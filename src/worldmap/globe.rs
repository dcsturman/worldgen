//! Spinning-globe projection of a world map.
//!
//! The flat world map ([`super::render`]) is an *equirectangular* unfolding
//! of a sphere — the whole generation pipeline samples elevation, climate,
//! and biome on a 3D unit sphere via [`grid::xy_to_sphere`] and only flattens
//! at the very end. A globe is therefore not a new generator, just a new
//! *projection* of the same data.
//!
//! The pipeline here is two stages:
//!
//! 1. [`build_equirect_texture`] — render a gap-free equirectangular RGB
//!    texture (every longitude/latitude filled, no icosahedron silhouette,
//!    no page background or drop shadow). This is the one expensive
//!    field-sampling pass and it is done exactly **once** per globe.
//! 2. [`GlobeTexture::warp_frame`] — orthographically project that texture
//!    onto a sphere as seen from a fixed camera, rotated by a spin longitude
//!    and a gentle axial tilt. Cheap (per output pixel: an inverse projection
//!    plus a bilinear texture lookup), so a smooth spin is just re-warping the
//!    same texture at an advancing angle.
//!
//! The browser animates by re-warping live (one frame per `requestAnimation
//! Frame` tick — see the `WorldMap` component); the server/download path
//! pre-warps `N` frames over a full turn and encodes them as an animated PNG
//! ([`render_globe_apng`]). Both share the same texture + warp, so the live
//! preview and the saved file look identical.
//!
//! Everything here is deterministic from the underlying `(uwp, seed)` — no
//! RNG, no clock — so the same world always produces the same globe.

use std::f64::consts::{FRAC_PI_2, PI};

use super::WorldMap;
use super::climate;
use super::clouds::CloudField;
use super::colormap;
use super::features::{CityTier, Feature};
use super::grid::{SHEET_HEIGHT, SHEET_WIDTH, xy_to_sphere};
use super::noise::DetailField;
use super::orbital;

/// Default equirectangular texture width (longitude). 2:1 with the height.
/// 1024×512 is plenty of detail for globes up to ~600 px and keeps the
/// one-time build cheaper than a full flat-map raster.
pub const TEX_W: u32 = 1024;
/// Default equirectangular texture height (latitude).
pub const TEX_H: u32 = 512;

/// Equirectangular texture dimensions to build a globe at.
///
/// Explicit at every call site rather than a single global constant, because
/// the two consumers want different answers and neither should silently
/// inherit the other's. The server caches its renders, so it can afford
/// [`TexSize::HIGH`]; the frontend builds the texture in WASM on the user's
/// machine every time a world is generated, where 4× the texels is 4× the
/// wait, and its 460-px canvas cannot resolve them anyway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TexSize {
    pub w: u32,
    pub h: u32,
}

impl TexSize {
    /// 1024×512 — what every consumer got before, and still what the
    /// in-browser path uses.
    pub const STANDARD: Self = Self {
        w: TEX_W,
        h: TEX_H,
    };
    /// 2048×1024. A globe disc of side `n` shows a hemisphere across `n`
    /// pixels, i.e. half the texture width, so 1024 gives only about one texel
    /// per pixel at the sub-viewer point and less toward the limb — which was
    /// throwing away the fractal coastline and fine relief detail before it
    /// ever reached a viewer.
    pub const HIGH: Self = Self { w: 2048, h: 1024 };
}

/// Animation timing for a spinning-globe APNG: how many frames make up the
/// full turn, and how long each is held (`delay_num / delay_den` seconds).
///
/// Grouped rather than passed as three loose numbers because they only ever
/// mean anything together — and two adjacent `u16`s with no names between
/// them at a call site is a swap waiting to happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApngTiming {
    pub frames: u32,
    pub delay_num: u16,
    pub delay_den: u16,
}

impl ApngTiming {
    /// [`DEFAULT_FRAMES`] frames at 1/5 s each — one full turn in about seven
    /// seconds.
    pub const DEFAULT: Self = Self {
        frames: DEFAULT_FRAMES,
        delay_num: 1,
        delay_den: 5,
    };
}

/// Default number of frames in a full-rotation flipbook. 36 frames = one
/// frame per 10° of spin — smooth enough to read as continuous rotation
/// while keeping the APNG small.
pub const DEFAULT_FRAMES: u32 = 36;

/// Axial tilt of the rendered globe, radians. Tips the visible pole slightly
/// toward the viewer so the planet reads as a 3D ball rather than a disc.
const AXIAL_TILT: f64 = 0.41; // ~23.5°, Earth-like

/// Fraction of the output half-size taken up by the globe disc; the rest is
/// margin for the atmosphere glow.
const DISC_FILL: f64 = 0.92;
/// Atmosphere glow thickness as a fraction of the disc radius (outside it).
const GLOW: f64 = 0.06;
/// Atmosphere glow tint.
const ATMO: (f64, f64, f64) = (130.0, 175.0, 255.0);

/// Directional "sun" in camera space (x right, y up, z toward viewer):
/// lower-left and toward the camera. The negative `y` drops the sun below
/// the horizon enough that the terminator sweeps up *through* the (viewer-
/// tilted) north pole, while the `x` term keeps the day/night line at a
/// diagonal rather than a vertical edge. Normalized at use; fixed in camera
/// space, so terrain rotates through day and night as the planet spins.
const LIGHT: (f64, f64, f64) = (-0.5, -0.3, 0.6);
/// Brightness of the night side relative to the fully-lit day side. The dark
/// half stays just readable (~18%) without going fully black; set to 1.0 to
/// disable the day/night effect entirely.
const NIGHT_LEVEL: f64 = 0.18;
/// Half-width (in Lambert-cosine units) of the soft day→night transition band
/// around the terminator. Larger = wider, softer twilight.
const TERM_WIDTH: f64 = 0.18;
/// Limb-darkening floor: edge pixels keep this fraction of their brightness.
const LIMB_FLOOR: f64 = 0.74;

/// Cloud colour at full opacity, and the greyer tone thin cloud takes.
/// Real cloud from orbit is not paper-white, and letting thin cover read
/// grey while thick cover reads bright is the only depth cue available to a
/// layer that can't cast a directional shadow.
const CLOUD_BRIGHT: (f64, f64, f64) = (243.0, 246.0, 250.0);
const CLOUD_THIN: (f64, f64, f64) = (186.0, 194.0, 205.0);
/// How much cloud darkens the ground beneath it before it's composited over.
///
/// This is the honest half of a cloud shadow. A directional drop-shadow would
/// need a sun direction, and the consumer warps this texture under a sun we
/// don't control — so it would look right at one angle and wrong everywhere
/// else. Ambient darkening under the deck is true regardless of where the sun
/// is, and it stops the clouds reading as decals laid on the surface.
const CLOUD_OCCLUSION: f64 = 0.35;
/// How thoroughly cloud hides the city lights underneath it. Not total: a
/// major conurbation glows through thin overcast, which is exactly what the
/// real Black Marble imagery shows.
const CLOUD_LIGHT_DIMMING: f64 = 0.80;

/// Warm sodium-vapor tint of night-side city lights (added, not multiplied,
/// on the dark side only).
const CITY_LIGHT: (f64, f64, f64) = (255.0, 185.0, 110.0);
/// Overall gain on the city-light contribution — keeps dense clusters from
/// blowing straight out to white.
const CITY_LIGHT_GAIN: f64 = 0.95;

/// Saturated red of the starport beacon. Unlike city lights it shows in
/// daylight too, and it's *blended* toward (not added) so the hot core reads
/// as true red over any terrain. Rotates out of view with the planet.
const BEACON_COLOR: (f64, f64, f64) = (255.0, 45.0, 35.0);
/// Beacon glow radius as a fraction of texture width.
const BEACON_RADIUS_FRAC: f64 = 0.011;
/// Gentle beacon pulses per full rotation. Integer so the spinning animation
/// loops seamlessly (the pulse phase returns to its start after 2π of spin).
const BEACON_PULSES: f64 = 8.0;

/// A gap-free equirectangular texture of a world's surface, suitable for
/// projecting onto a sphere. Row-major, `width × height`.
pub struct GlobeTexture {
    pub width: u32,
    pub height: u32,
    /// `width * height * 3` bytes, RGB row-major — the daylight surface.
    pub rgb: Vec<u8>,
    /// `width * height` bytes — a "Black Marble" emissive channel: the
    /// intensity of artificial light (city glow) at each texel, scaled by
    /// settlement size. Sampled and shown only on the night side during the
    /// warp. All-zero for unpopulated worlds.
    pub emissive: Vec<u8>,
    /// `width * height` bytes — the starport-beacon channel: a single red
    /// marker at the world's starport, shown day and night. All-zero for
    /// worlds with no starport (class X / Y, or unpopulated).
    pub beacon: Vec<u8>,
}

/// Build a full equirectangular surface texture for `map` in one call.
///
/// Mirrors the flat rasterizer's per-pixel colour pipeline (elevation
/// amplification → temperature with lapse → humidity with rain-shadow,
/// altitude drying, and continentality → colormap → hillshade + tide) but
/// over a complete lon/lat rectangle: every pixel is filled (no silhouette
/// mask) and the longitude axis *wraps* so the seam at lon=0/2π is seamless
/// on the globe.
///
/// On the WASM main thread, prefer [`GlobeTextureJob`] and yield between its
/// steps so a build can't block long enough to trip the browser's
/// "Page Unresponsive" dialog.
pub fn build_equirect_texture(map: &WorldMap, width: u32, height: u32) -> GlobeTexture {
    let mut job = GlobeTextureJob::new(width, height);
    job.step_elevation(map);
    job.step_color(map);
    job.populate_clouds(map);
    job.populate_city_lights(map);
    job.into_texture()
}

/// Resumable equirectangular-texture builder. Splits the surface render into
/// three callable steps so a WASM caller can `setTimeout(0)`-yield between
/// them — the same pattern [`super::RasterJob`] uses for the flat map. The
/// synchronous [`build_equirect_texture`] just runs all three back to back.
pub struct GlobeTextureJob {
    width: u32,
    height: u32,
    elev: Vec<f32>,
    /// Per-texel tectonic rain-shadow, computed in [`Self::step_elevation`]
    /// and consumed by [`Self::step_color`].
    ///
    /// It lives here rather than being sampled where it's used because both
    /// it and the elevation need the same expensive thing: the tectonic
    /// domain warp, which evaluates two fBm bands three times over (once per
    /// axis) and was previously computed twice per texel — once inside
    /// `elevation_offset`, then again inside `rain_shadow_at`. Warping once
    /// and carrying the f32 result forward costs 2 MB at 1024×512 and removes
    /// the single largest term in the sampling path.
    ///
    /// Zero when the map has no tectonic field, which makes
    /// `rain_shadow_adjustment` a no-op — so `step_color` needs no branch.
    rain_shadow: Vec<f32>,
    /// Per-texel cloud opacity, 0–255. Zero everywhere unless
    /// [`Self::populate_clouds`] ran, and always zero on worlds with
    /// atmosphere 0 or 1, which get no cloud field at all.
    clouds: Vec<u8>,
    color: Vec<(u8, u8, u8)>,
    emissive: Vec<u8>,
    beacon: Vec<u8>,
}

impl GlobeTextureJob {
    pub fn new(width: u32, height: u32) -> Self {
        let n = (width as usize) * (height as usize);
        Self {
            width,
            height,
            elev: vec![0f32; n],
            rain_shadow: vec![0f32; n],
            clouds: vec![0u8; n],
            color: vec![(0u8, 0u8, 0u8); n],
            emissive: vec![0u8; n],
            beacon: vec![0u8; n],
        }
    }

    /// Step 1: above-sea elevation per texel, plus the tectonic rain-shadow
    /// that step 2 needs. Steps 2 and 3 read this grid for continentality and
    /// hillshade.
    ///
    /// Both outputs come from one [`ElevationField::warp`] call — see
    /// [`Self::rain_shadow`] for why they're computed together.
    pub fn step_elevation(&mut self, map: &WorldMap) {
        let w = self.width as usize;
        let tectonics = map.elev_field.tectonics();
        for ty in 0..self.height as usize {
            let sy = (ty as f64 + 0.5) / self.height as f64 * SHEET_HEIGHT;
            for tx in 0..w {
                let sx = (tx as f64 + 0.5) / self.width as f64 * SHEET_WIDTH;
                let sphere = xy_to_sphere(sx, sy);
                let warped = map.elev_field.warp(&sphere);
                let e = map.elev_field.sample_prewarped(&sphere, &warped);
                let above = climate::amplify_elevation(e - map.sea_level, map.uwp.hydrographics());
                let i = ty * w + tx;
                self.elev[i] = above as f32;
                if let Some(tec) = tectonics {
                    self.rain_shadow[i] = tec.rain_shadow_at_warped(&warped) as f32;
                }
            }
        }
    }

    /// Step 2: fold elevation + climate into a base biome colour per texel.
    ///
    /// Unlike the flat map's [`super::raster::RasterJob::step_color`], which
    /// paints legend swatches via [`colormap::elevation_color`], this uses the
    /// continuous [`orbital`] path: the same climate inputs and the same
    /// palette, but blended, mottled by a detail field and tone-curved. The
    /// flat map stays a precision instrument; the globe gets to be a photo.
    pub fn step_color(&mut self, map: &WorldMap) {
        let w = self.width as usize;
        let h = self.height as usize;
        let detail = DetailField::from_uwp(&map.uwp, map.seed);
        for ty in 0..h {
            let sy = (ty as f64 + 0.5) / self.height as f64 * SHEET_HEIGHT;
            for tx in 0..w {
                let sx = (tx as f64 + 0.5) / self.width as f64 * SHEET_WIDTH;
                let sphere = xy_to_sphere(sx, sy);
                let above = self.elev[ty * w + tx] as f64;

                let raw_t = climate::temperature_at_wobbled(&sphere, &map.temp_field);
                let t = climate::apply_lapse(
                    climate::adjust_temperature(raw_t, &map.uwp),
                    above,
                    &map.uwp,
                );

                // Rain shadow was computed in step_elevation, which already
                // had the warped point in hand.
                let mut hu = map.humidity_field.sample(&sphere, &map.uwp);
                hu = colormap::rain_shadow_adjustment(hu, self.rain_shadow[ty * w + tx] as f64);
                hu = climate::apply_altitude_drying(hu, above);
                if above > 0.0 {
                    let cont = continentality_wrapped(&self.elev, w, h, tx, ty);
                    hu = super::raster::apply_continentality(hu, cont);
                }
                let (mottle, grain) = detail.sample(&sphere);
                self.color[ty * w + tx] = orbital::surface_color(above, t, hu, mottle, grain);
            }
        }
    }

    /// Optional step (run before [`Self::into_texture`]): rasterize the
    /// world's cloud deck. Coverage comes from the UWP's atmosphere and
    /// hydrographics — see [`super::clouds`] — so the layer reports something
    /// about the world rather than just prettying it up, and worlds that
    /// can't have weather get nothing rather than a faint haze.
    ///
    /// Skipping it leaves a cloudless sky.
    pub fn populate_clouds(&mut self, map: &WorldMap) {
        let Some(field) = CloudField::from_uwp(&map.uwp, map.seed) else {
            return;
        };
        let w = self.width as usize;
        for ty in 0..self.height as usize {
            let sy = (ty as f64 + 0.5) / self.height as f64 * SHEET_HEIGHT;
            for tx in 0..w {
                let sx = (tx as f64 + 0.5) / self.width as f64 * SHEET_WIDTH;
                let a = field.opacity_at(&xy_to_sphere(sx, sy));
                self.clouds[ty * w + tx] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Optional step (run after [`Self::step_color`], before
    /// [`Self::into_texture`]): rasterize a "Black Marble" emissive map from
    /// the world's settlements. Each city splats a soft warm glow at its
    /// lon/lat whose radius and peak scale with [`CityTier`], accumulating
    /// where settlements cluster. Longitude wraps. Cheap — O(cities × radius²).
    /// Skipping it leaves the emissive channel zero (no night lights).
    pub fn populate_city_lights(&mut self, map: &WorldMap) {
        let w = self.width as i32;
        let h = self.height as i32;
        let width = self.width as f64;
        for hex in &map.grid.hexes {
            let Some((tier, is_port)) = hex.features.iter().find_map(|f| match f {
                Feature::City { tier, starport } => Some((*tier, *starport)),
                _ => None,
            }) else {
                continue;
            };
            let p = hex.sphere_pos;
            let lat = p[2].clamp(-1.0, 1.0).asin();
            let lon = p[1].atan2(p[0]).rem_euclid(2.0 * PI);
            let cx = (lon / (2.0 * PI) * width) as i32;
            let cy = ((FRAC_PI_2 - lat) / PI * self.height as f64) as i32;

            // Warm city glow scaled by settlement size.
            let (rfrac, peak) = city_light_params(tier);
            splat(&mut self.emissive, w, h, cx, cy, (rfrac * width).max(1.5), peak);

            // The single starport also drops a red beacon marker.
            if is_port {
                splat(
                    &mut self.beacon,
                    w,
                    h,
                    cx,
                    cy,
                    (BEACON_RADIUS_FRAC * width).max(1.5),
                    255.0,
                );
            }
        }
    }

    /// Step 3 (terminal): hillshade land + faint tide on shallow water, baking
    /// the RGB texture. Longitude wraps; latitude clamps at the poles. The
    /// emissive channel (if [`Self::populate_city_lights`] ran) passes through.
    pub fn into_texture(self) -> GlobeTexture {
        let w = self.width as usize;
        let h = self.height as usize;
        let elev = &self.elev;
        let color = &self.color;
        let mut emissive = self.emissive;
        let mut rgb = vec![0u8; w * h * 3];
        for ty in 0..h {
            for tx in 0..w {
                let i = ty * w + tx;
                let mut c = color[i];

                let il = i - tx + ((tx + w - 1) % w); // wrap left
                let ir = i - tx + ((tx + 1) % w); // wrap right
                let iu = if ty > 0 { i - w } else { i };
                let id = if ty + 1 < h { i + w } else { i };

                if elev[i] > 0.0 {
                    // Ungated, and at a far higher gain than the flat map's
                    // equivalent in `raster::step_postprocess`. Both choices
                    // are deliberate, and they're the difference between
                    // terrain you can see and terrain you can't.
                    //
                    // The flat map gates shading below a slope threshold so
                    // level ground paints its legend swatch unmodified — the
                    // key has to mean something. The globe has no key, and
                    // that gate was silently discarding all the fine relief:
                    // typical per-texel deltas here are ~0.002, which at the
                    // old gain of 30 gives a slope of ~0.06, well under the
                    // old 0.20 floor. Every subtle slope on the planet
                    // rounded to "perfectly flat", which is most of why the
                    // surface read as moulded clay rather than landscape.
                    //
                    // The gain is set so an ordinary hillside lands in the
                    // meat of the Lambert response instead of pinned at its
                    // flat-ground value. `colormap::apply_hillshade` clamps
                    // shade to [0.40, 1.20], so steep ground self-limits and
                    // no amount of gain can blow it out.
                    const SHADE_GAIN: f64 = 140.0;
                    let dx = (elev[ir] - elev[il]) as f64 * SHADE_GAIN;
                    let dy = (elev[id] - elev[iu]) as f64 * SHADE_GAIN;
                    c = colormap::apply_hillshade(c, dx, dy);
                }
                // No tide band on the globe. The flat map lightens the texel
                // ring next to land so a coastline is legible at a glance;
                // here that fixed 45% lerp to a pale blue was drawing a hard
                // one-texel fringe *on top of* the continuous shelf ramp —
                // the staircased cyan outline around every island.
                // `orbital::ocean_color` already brightens shallow water, and
                // does it as a gradient over real depth.

                // Clouds composite last: they sit above the terrain, so they
                // go over the hillshade rather than under it.
                let cloud = self.clouds[i];
                if cloud > 0 {
                    let a = cloud as f64 / 255.0;
                    let shaded = 1.0 - CLOUD_OCCLUSION * a;
                    let ground = (
                        c.0 as f64 * shaded,
                        c.1 as f64 * shaded,
                        c.2 as f64 * shaded,
                    );
                    // Thin cloud greys, thick cloud brightens.
                    let tint = (
                        CLOUD_THIN.0 + (CLOUD_BRIGHT.0 - CLOUD_THIN.0) * a,
                        CLOUD_THIN.1 + (CLOUD_BRIGHT.1 - CLOUD_THIN.1) * a,
                        CLOUD_THIN.2 + (CLOUD_BRIGHT.2 - CLOUD_THIN.2) * a,
                    );
                    c = (
                        (ground.0 + (tint.0 - ground.0) * a).round() as u8,
                        (ground.1 + (tint.1 - ground.1) * a).round() as u8,
                        (ground.2 + (tint.2 - ground.2) * a).round() as u8,
                    );
                    // The deck is above the cities too.
                    let e = &mut emissive[i];
                    *e = (*e as f64 * (1.0 - CLOUD_LIGHT_DIMMING * a)).round() as u8;
                }

                rgb[i * 3] = c.0;
                rgb[i * 3 + 1] = c.1;
                rgb[i * 3 + 2] = c.2;
            }
        }
        GlobeTexture {
            width: self.width,
            height: self.height,
            rgb,
            emissive,
            beacon: self.beacon,
        }
    }
}

/// City-light splat parameters by settlement size: `(radius as a fraction of
/// texture width, peak intensity 0–255)`. Bigger, brighter for larger tiers.
fn city_light_params(tier: CityTier) -> (f64, f64) {
    match tier {
        CityTier::Megacity => (0.0100, 255.0),
        CityTier::Major => (0.0072, 210.0),
        CityTier::Minor => (0.0050, 160.0),
        CityTier::Small => (0.0034, 110.0),
    }
}

/// Accumulate a soft radial glow of radius `r` and centre intensity `peak`
/// into a single-channel `width × height` map at `(cx, cy)`. Longitude (x)
/// wraps; latitude (y) clamps. Quadratic falloff, saturating at 255.
fn splat(channel: &mut [u8], width: i32, height: i32, cx: i32, cy: i32, r: f64, peak: f64) {
    let ri = r.ceil() as i32;
    for dy in -ri..=ri {
        let ny = (cy + dy).clamp(0, height - 1);
        for dx in -ri..=ri {
            let d2 = (dx * dx + dy * dy) as f64;
            let fall = (1.0 - d2 / (r * r)).max(0.0);
            if fall <= 0.0 {
                continue;
            }
            let nx = (cx + dx).rem_euclid(width);
            let idx = (ny as usize) * (width as usize) + nx as usize;
            let val = peak * fall * fall;
            channel[idx] = (channel[idx] as f64 + val).min(255.0) as u8;
        }
    }
}

/// Continentality from the texel elevation grid, wrapping in longitude and
/// clamping in latitude. Fraction of four diagonal ring samples that are
/// land. Mirrors `raster::continentality_from_grid` but seam-aware so the
/// globe's drying doesn't gain a visible meridian stripe.
fn continentality_wrapped(elev: &[f32], w: usize, h: usize, tx: usize, ty: usize) -> f64 {
    // ~3.5% of width in texels, matching the flat map's CONT_OFFSET_PX ratio.
    let r = ((w as f64) * 0.035).round().max(1.0) as i32;
    const DIAG: [(i32, i32); 4] = [(1, 1), (-1, 1), (1, -1), (-1, -1)];
    let mut land = 0u32;
    for (dx, dy) in DIAG {
        let nx = (tx as i32 + dx * r).rem_euclid(w as i32) as usize;
        let ny = (ty as i32 + dy * r).clamp(0, h as i32 - 1) as usize;
        if elev[ny * w + nx] > 0.0 {
            land += 1;
        }
    }
    land as f64 / DIAG.len() as f64
}

impl GlobeTexture {
    /// Bilinearly sample the texture at a 3D unit-sphere position (in the
    /// `xy_to_sphere` convention: `z` is the pole axis). Returns the surface
    /// RGB, the emissive (city-light) intensity, and the starport-beacon
    /// intensity at that point. Longitude wraps, latitude clamps.
    #[inline]
    fn sample(&self, p: [f64; 3]) -> ((f64, f64, f64), f64, f64) {
        let lat = p[2].clamp(-1.0, 1.0).asin();
        let lon = p[1].atan2(p[0]).rem_euclid(2.0 * PI);
        // Match xy_to_sphere: x∈[0,W)→lon∈[0,2π); y∈[0,H)→lat from +π/2 to -π/2.
        let fx = lon / (2.0 * PI) * self.width as f64 - 0.5;
        let fy = (FRAC_PI_2 - lat) / PI * self.height as f64 - 0.5;

        let w = self.width as i32;
        let h = self.height as i32;
        let x0 = fx.floor() as i32;
        let y0 = fy.floor() as i32;
        let tx = fx - x0 as f64;
        let tyf = fy - y0 as f64;

        // Sample one texel as (r, g, b, emissive, beacon), all f64.
        let px = |x: i32, y: i32| -> [f64; 5] {
            let xi = x.rem_euclid(w) as usize;
            let yi = y.clamp(0, h - 1) as usize;
            let flat = yi * self.width as usize + xi;
            let i = flat * 3;
            [
                self.rgb[i] as f64,
                self.rgb[i + 1] as f64,
                self.rgb[i + 2] as f64,
                self.emissive[flat] as f64,
                self.beacon[flat] as f64,
            ]
        };
        let c00 = px(x0, y0);
        let c10 = px(x0 + 1, y0);
        let c01 = px(x0, y0 + 1);
        let c11 = px(x0 + 1, y0 + 1);
        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let mut out = [0.0f64; 5];
        for k in 0..5 {
            let top = lerp(c00[k], c10[k], tx);
            let bot = lerp(c01[k], c11[k], tx);
            out[k] = lerp(top, bot, tyf);
        }
        ((out[0], out[1], out[2]), out[3], out[4])
    }

    /// Orthographically project the texture onto a sphere into a fresh
    /// `size × size` RGBA buffer. `spin` is the sub-viewer longitude in
    /// radians (advance it to rotate the planet). Pixels outside the disc are
    /// transparent except a thin atmosphere glow. See [`Self::warp_into`].
    pub fn warp_frame(&self, size: u32, spin: f64) -> Vec<u8> {
        let mut buf = vec![0u8; (size as usize) * (size as usize) * 4];
        self.warp_into(&mut buf, size, spin);
        buf
    }

    /// Warp into an existing `size × size` RGBA buffer (reused across frames
    /// so the live animation loop allocates nothing per tick). The buffer
    /// must be exactly `size*size*4` bytes.
    pub fn warp_into(&self, buf: &mut [u8], size: u32, spin: f64) {
        debug_assert_eq!(buf.len(), (size as usize) * (size as usize) * 4);
        let s = size as f64;
        let c = s / 2.0;
        let radius = c * DISC_FILL;
        let glow_outer = radius * (1.0 + GLOW);

        // Planet basis in camera space (see module docs / tests): `east` is
        // screen-right, `north` (pole) tips toward the viewer by AXIAL_TILT,
        // `front` is the equator point facing the camera at spin=0.
        let (ca, sa) = (AXIAL_TILT.cos(), AXIAL_TILT.sin());
        let east = [1.0, 0.0, 0.0];
        let north = [0.0, ca, sa];
        let front = [0.0, -sa, ca];

        // Normalized light direction.
        let ll = (LIGHT.0 * LIGHT.0 + LIGHT.1 * LIGHT.1 + LIGHT.2 * LIGHT.2).sqrt();
        let light = [LIGHT.0 / ll, LIGHT.1 / ll, LIGHT.2 / ll];

        // Beacon pulse — a gentle throb tied to the spin phase (so it loops
        // seamlessly). Constant across the frame, so compute it once here.
        let beacon_pulse = 0.65 + 0.35 * (0.5 + 0.5 * (spin * BEACON_PULSES).sin());

        for oy in 0..size {
            for ox in 0..size {
                let i = ((oy * size + ox) as usize) * 4;
                let dx = (ox as f64 + 0.5) - c;
                let dy = (oy as f64 + 0.5) - c;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist > glow_outer {
                    buf[i] = 0;
                    buf[i + 1] = 0;
                    buf[i + 2] = 0;
                    buf[i + 3] = 0;
                    continue;
                }
                if dist > radius {
                    // Atmosphere glow ring: fade alpha from rim outward.
                    let tt = ((glow_outer - dist) / (glow_outer - radius)).clamp(0.0, 1.0);
                    let a = (tt * tt * 150.0).round() as u8;
                    buf[i] = ATMO.0 as u8;
                    buf[i + 1] = ATMO.1 as u8;
                    buf[i + 2] = ATMO.2 as u8;
                    buf[i + 3] = a;
                    continue;
                }

                // Inverse orthographic: screen → near-hemisphere normal.
                // nx right, ny up (screen y is down), nz toward viewer.
                let nx = dx / radius;
                let ny = -dy / radius;
                let nz2 = 1.0 - nx * nx - ny * ny;
                let nz = if nz2 > 0.0 { nz2.sqrt() } else { 0.0 };
                let p_cam = [nx, ny, nz];

                // Project onto the planet basis → (lat, lon-relative).
                let de = dot(p_cam, east);
                let dn = dot(p_cam, north);
                let df = dot(p_cam, front);
                let lat = dn.clamp(-1.0, 1.0).asin();
                let lon_rel = de.atan2(df);
                let lon = lon_rel + spin;
                let cl = lat.cos();
                let sphere = [cl * lon.cos(), cl * lon.sin(), lat.sin()];

                let ((cr, cg, cb), emissive, beacon) = self.sample(sphere);
                let (mut r, mut g, mut b) = (cr, cg, cb);

                // Day/night: `lambert` is the sun cosine at this point; a
                // smoothstep across the terminator gives `day` ∈ [0,1] (1 =
                // full daylight, 0 = night). The night side keeps NIGHT_LEVEL
                // of its brightness so it stays readable. Then limb-darken.
                let lambert = dot(p_cam, light);
                let day = smoothstep(-TERM_WIDTH, TERM_WIDTH, lambert);
                let mut shade = NIGHT_LEVEL + (1.0 - NIGHT_LEVEL) * day;
                shade *= LIMB_FLOOR + (1.0 - LIMB_FLOOR) * nz;
                r *= shade;
                g *= shade;
                b *= shade;

                // City lights ("Black Marble"): warm sodium glow, additive and
                // only on the night side, fading out through the terminator.
                let night = 1.0 - day;
                if night > 0.0 && emissive > 0.0 {
                    let lit = (emissive / 255.0) * night * CITY_LIGHT_GAIN;
                    r += CITY_LIGHT.0 * lit;
                    g += CITY_LIGHT.1 * lit;
                    b += CITY_LIGHT.2 * lit;
                }

                // Soft bright atmosphere rim on the lit limb (inner edge).
                let edge = (dist / radius).clamp(0.0, 1.0);
                if edge > 0.92 {
                    let rim = ((edge - 0.92) / 0.08).clamp(0.0, 1.0) * 0.5 * day;
                    r += (ATMO.0 - r) * rim;
                    g += (ATMO.1 - g) * rim;
                    b += (ATMO.2 - b) * rim;
                }

                // Starport beacon: blended toward red (so the hot core overrides
                // terrain), shown in daylight as well as night, pulsing gently.
                if beacon > 0.0 {
                    let bo = (beacon / 255.0) * beacon_pulse;
                    r += (BEACON_COLOR.0 - r) * bo;
                    g += (BEACON_COLOR.1 - g) * bo;
                    b += (BEACON_COLOR.2 - b) * bo;
                }

                let r = r.clamp(0.0, 255.0);
                let g = g.clamp(0.0, 255.0);
                let b = b.clamp(0.0, 255.0);

                buf[i] = r.round() as u8;
                buf[i + 1] = g.round() as u8;
                buf[i + 2] = b.round() as u8;
                buf[i + 3] = 255;
            }
        }
    }
}

#[inline]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Hermite smoothstep: 0 below `e0`, 1 above `e1`, smooth in between.
#[inline]
fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Render a single static globe frame for `map` as a PNG, viewed at sub-viewer
/// longitude `spin` (radians). `size` is the output square's side in pixels.
pub fn render_globe_png(
    map: &WorldMap,
    size: u32,
    spin: f64,
    tex_size: TexSize,
) -> Result<Vec<u8>, String> {
    let tex = build_equirect_texture(map, tex_size.w, tex_size.h);
    let frame = tex.warp_frame(size, spin);
    encode_png_rgba(&frame, size, size)
}

/// Render a full-rotation spinning globe for `map` as an animated PNG (APNG).
///
/// `frames` evenly-spaced frames over 360°, each shown for `delay_num/delay_den`
/// seconds, looping forever. APNG is served as `image/png` and animates
/// natively in every modern browser. `size` is the output square's side.
pub fn render_globe_apng(
    map: &WorldMap,
    size: u32,
    timing: ApngTiming,
    tex_size: TexSize,
) -> Result<Vec<u8>, String> {
    use std::f64::consts::PI;
    let ApngTiming {
        frames,
        delay_num,
        delay_den,
    } = timing;
    let frames = frames.max(1);
    let tex = build_equirect_texture(map, tex_size.w, tex_size.h);
    let buffers: Vec<Vec<u8>> = (0..frames)
        .map(|f| tex.warp_frame(size, f as f64 / frames as f64 * 2.0 * PI))
        .collect();
    encode_apng_rgba(&buffers, size, size, delay_num, delay_den)
}

/// PNG-encode a single RGBA8 frame.
/// Render the world's surface as a single equirectangular **texture** PNG for
/// client-side (e.g. WebGL) globe rendering: RGB carries the day-side surface
/// colour, the alpha channel carries the night-side city-light emissive
/// intensity. If the world has a starport, its texture coordinates
/// `(lon, lat)` in radians are embedded as a `Starport` tEXt chunk so the
/// consumer can draw the beacon. The image is `tex_size.w`×`tex_size.h`.
///
/// This is the payload for `/api/world?projection=globe&format=texture`: the
/// expensive generation happens here (server-side, cached); the client only
/// warps this texture per frame. Since the consumer warps it onto a sphere at
/// whatever size it likes, the texture's resolution sets the ceiling on how
/// sharp that render can be — see [`TexSize::HIGH`].
pub fn render_globe_texture(
    map: &WorldMap,
    tex_size: TexSize,
    clouds: bool,
) -> Result<Vec<u8>, String> {
    let (tex_w, tex_h) = (tex_size.w, tex_size.h);
    let tex = if clouds {
        build_equirect_texture(map, tex_w, tex_h)
    } else {
        // Same pipeline minus the cloud step. Consumers that composite their
        // own weather, or want the bare surface, shouldn't be stuck with ours
        // baked into the pixels — we can't ship it as a separate layer, so the
        // only honest alternative is not shipping it.
        let mut job = GlobeTextureJob::new(tex_w, tex_h);
        job.step_elevation(map);
        job.step_color(map);
        job.populate_city_lights(map);
        job.into_texture()
    };
    let n = (tex_w as usize) * (tex_h as usize);
    let mut rgba = vec![0u8; n * 4];
    for i in 0..n {
        rgba[i * 4] = tex.rgb[i * 3];
        rgba[i * 4 + 1] = tex.rgb[i * 3 + 1];
        rgba[i * 4 + 2] = tex.rgb[i * 3 + 2];
        rgba[i * 4 + 3] = tex.emissive[i];
    }

    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, tex_w, tex_h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Best);
        if let Some((lon, lat)) = starport_lonlat(map) {
            enc.add_text_chunk("Starport".to_string(), format!("{lon},{lat}"))
                .map_err(|e| format!("png text chunk: {e}"))?;
        }
        let mut writer = enc.write_header().map_err(|e| format!("png header: {e}"))?;
        writer
            .write_image_data(&rgba)
            .map_err(|e| format!("png data: {e}"))?;
    }
    Ok(out)
}

/// The starport's texture coordinates `(lon, lat)` in radians (matching the
/// equirectangular convention: `lon ∈ [0, 2π)`, `lat ∈ [-π/2, π/2]`), or
/// `None` if the world has no starport (class X/Y, or unpopulated).
pub fn starport_lonlat(map: &WorldMap) -> Option<(f64, f64)> {
    map.grid.hexes.iter().find_map(|hex| {
        hex.features
            .iter()
            .any(|f| matches!(f, Feature::City { starport: true, .. }))
            .then(|| {
                let p = hex.sphere_pos;
                let lat = p[2].clamp(-1.0, 1.0).asin();
                let lon = p[1].atan2(p[0]).rem_euclid(2.0 * PI);
                (lon, lat)
            })
    })
}

fn encode_png_rgba(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|e| format!("png header: {e}"))?;
        writer
            .write_image_data(rgba)
            .map_err(|e| format!("png data: {e}"))?;
    }
    Ok(out)
}

/// APNG-encode a sequence of RGBA8 frames into one looping animated PNG.
fn encode_apng_rgba(
    frames: &[Vec<u8>],
    width: u32,
    height: u32,
    delay_num: u16,
    delay_den: u16,
) -> Result<Vec<u8>, String> {
    if frames.is_empty() {
        return Err("apng: no frames".into());
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        // Best compression: the flipbook is cached in GCS (server) or held in
        // memory for download, so the one-time encode cost buys a much smaller
        // payload on every subsequent serve (~36 full frames otherwise).
        enc.set_compression(png::Compression::Best);
        // num_plays = 0 → loop forever.
        enc.set_animated(frames.len() as u32, 0)
            .map_err(|e| format!("apng actl: {e}"))?;
        enc.set_frame_delay(delay_num, delay_den)
            .map_err(|e| format!("apng delay: {e}"))?;
        let mut writer = enc.write_header().map_err(|e| format!("apng header: {e}"))?;
        for frame in frames {
            writer
                .write_image_data(frame)
                .map_err(|e| format!("apng frame: {e}"))?;
        }
        writer.finish().map_err(|e| format!("apng finish: {e}"))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tex() -> GlobeTexture {
        let map = super::super::generate("A788899-A", 1, None).unwrap();
        build_equirect_texture(&map, 256, 128)
    }

    #[test]
    fn equirect_texture_is_full_and_nonblank() {
        let t = tex();
        assert_eq!(t.rgb.len(), 256 * 128 * 3);
        // A garden world should show more than one colour (land + ocean),
        // i.e. the texture isn't a flat fill or all-page-colour.
        let first = (t.rgb[0], t.rgb[1], t.rgb[2]);
        let varied = t
            .rgb
            .chunks_exact(3)
            .any(|c| (c[0], c[1], c[2]) != first);
        assert!(varied, "equirect texture should contain varied terrain");
    }

    #[test]
    fn equirect_texture_is_deterministic() {
        let map = super::super::generate("A788899-A", 7, None).unwrap();
        let a = build_equirect_texture(&map, 128, 64).rgb;
        let b = build_equirect_texture(&map, 128, 64).rgb;
        assert_eq!(a, b);
    }

    #[test]
    fn warp_disc_is_bounded_and_corners_transparent() {
        let t = tex();
        let size = 120u32;
        let frame = t.warp_frame(size, 0.0);
        assert_eq!(frame.len(), (size * size * 4) as usize);
        // The four corners are well outside the disc+glow → fully transparent.
        let corner = |x: u32, y: u32| frame[((y * size + x) * 4 + 3) as usize];
        assert_eq!(corner(0, 0), 0);
        assert_eq!(corner(size - 1, 0), 0);
        assert_eq!(corner(0, size - 1), 0);
        assert_eq!(corner(size - 1, size - 1), 0);
        // The centre pixel is on the disc → opaque.
        let mid = (size / 2) * size + (size / 2);
        assert_eq!(frame[(mid * 4 + 3) as usize], 255);
    }

    #[test]
    fn populated_world_emits_city_lights() {
        // A high-population world (pop A) places many cities, so its emissive
        // channel must be non-zero somewhere.
        let map = super::super::generate("A8888AA-A", 1, None).unwrap();
        let t = build_equirect_texture(&map, 256, 128);
        assert_eq!(t.emissive.len(), 256 * 128);
        let lit = t.emissive.iter().filter(|&&e| e > 0).count();
        assert!(lit > 0, "populated world should have lit texels");
    }

    #[test]
    fn unpopulated_world_has_no_city_lights() {
        // Pop 0 → no settlements → emissive channel is entirely dark.
        let map = super::super::generate("A8800A0-8", 1, None).unwrap();
        assert_eq!(map.uwp.population(), 0);
        let t = build_equirect_texture(&map, 256, 128);
        assert!(
            t.emissive.iter().all(|&e| e == 0),
            "unpopulated world must have no city lights"
        );
    }

    #[test]
    fn starport_world_has_a_beacon() {
        // An A-port populated world places a starport, so the beacon channel
        // must light up.
        let map = super::super::generate("A788899-A", 1, None).unwrap();
        let t = build_equirect_texture(&map, 256, 128);
        assert_eq!(t.beacon.len(), 256 * 128);
        assert!(
            t.beacon.iter().any(|&v| v > 0),
            "A-port world should have a starport beacon"
        );
    }

    #[test]
    fn portless_world_has_no_beacon() {
        // Class-X worlds have no starport — no beacon, even though they may
        // still have cities (and thus city lights).
        let map = super::super::generate("X788899-A", 1, None).unwrap();
        let t = build_equirect_texture(&map, 256, 128);
        assert!(
            t.beacon.iter().all(|&v| v == 0),
            "X-port world must have no beacon"
        );
        assert!(
            t.emissive.iter().any(|&v| v > 0),
            "but a populated X world still has city lights"
        );
    }

    #[test]
    fn spin_changes_the_frame() {
        let t = tex();
        let a = t.warp_frame(96, 0.0);
        let b = t.warp_frame(96, std::f64::consts::PI);
        assert_ne!(a, b, "opposite hemispheres should differ");
    }

    #[test]
    fn static_globe_png_decodes() {
        let map = super::super::generate("A788899-A", 1, None).unwrap();
        let bytes = render_globe_png(&map, 128, 0.0, TexSize::STANDARD).unwrap();
        assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
        let dec = png::Decoder::new(std::io::Cursor::new(&bytes));
        let reader = dec.read_info().unwrap();
        assert_eq!(reader.info().width, 128);
        assert_eq!(reader.info().height, 128);
    }

    #[test]
    fn apng_is_animated_with_expected_frame_count() {
        let map = super::super::generate("A788899-A", 1, None).unwrap();
        let bytes = render_globe_apng(
            &map,
            96,
            ApngTiming {
                frames: 8,
                delay_num: 1,
                delay_den: 10,
            },
            TexSize::STANDARD,
        )
        .unwrap();
        assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
        let dec = png::Decoder::new(std::io::Cursor::new(&bytes));
        let reader = dec.read_info().unwrap();
        let actl = reader
            .info()
            .animation_control
            .expect("APNG must carry an acTL chunk");
        assert_eq!(actl.num_frames, 8, "frame count should match request");
        assert_eq!(actl.num_plays, 0, "0 plays = infinite loop");
    }

    #[test]
    fn frame_zero_matches_static_render() {
        // The first APNG frame and the static PNG at spin=0 are warped from
        // the same texture at the same angle, so their pixels agree.
        let map = super::super::generate("A788899-A", 3, None).unwrap();
        let tex = build_equirect_texture(&map, TEX_W, TEX_H);
        let static_frame = tex.warp_frame(96, 0.0);
        let apng_first = tex.warp_frame(96, 0.0);
        assert_eq!(static_frame, apng_first);
    }

    /// The texture comes out at whatever [`TexSize`] was asked for, and both
    /// sizes carry the same metadata — the server renders `HIGH` while the
    /// in-browser path stays on `STANDARD`, so neither may quietly lose the
    /// starport chunk or change colour type.
    #[test]
    fn globe_texture_is_rgba_at_requested_size_with_starport_chunk() {
        let map = super::super::generate("A788899-A", 1, None).unwrap();
        for tex_size in [TexSize::STANDARD, TexSize::HIGH] {
            let bytes = render_globe_texture(&map, tex_size, true).unwrap();
            assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
            let dec = png::Decoder::new(std::io::Cursor::new(&bytes));
            let reader = dec.read_info().unwrap();
            let info = reader.info();
            assert_eq!((info.width, info.height), (tex_size.w, tex_size.h));
            assert_eq!(info.color_type, png::ColorType::Rgba);
            // An A-port world embeds its starport coords as a tEXt chunk, and
            // starport_lonlat reports the same presence.
            assert!(starport_lonlat(&map).is_some());
            assert!(
                info.uncompressed_latin1_text
                    .iter()
                    .any(|c| c.keyword == "Starport"),
                "A-port texture should carry a Starport chunk at {tex_size:?}"
            );
        }
    }

    #[test]
    fn portless_globe_texture_has_no_starport_chunk() {
        let map = super::super::generate("X788899-A", 1, None).unwrap();
        assert!(starport_lonlat(&map).is_none());
        let bytes = render_globe_texture(&map, TexSize::STANDARD, true).unwrap();
        let dec = png::Decoder::new(std::io::Cursor::new(&bytes));
        let reader = dec.read_info().unwrap();
        assert!(
            !reader
                .info()
                .uncompressed_latin1_text
                .iter()
                .any(|c| c.keyword == "Starport"),
            "X-port texture must not carry a Starport chunk"
        );
    }

    /// Visual dump: write a static globe PNG and a spinning APNG to /tmp for
    /// Visual dump: write the raw equirectangular surface texture to /tmp,
    /// opaque, for judging detail at 1:1. Ignored by default.
    ///
    /// The globe dumps below are the wrong tool for that: a 512-px disc shows
    /// the 1024-wide texture at roughly one texel per pixel *at the sub-viewer
    /// point* and far worse toward the limb, so fine relief reads as mush
    /// there whether or not it's present. Look at the texture to decide
    /// whether detail exists; look at the globe to decide whether it reads.
    ///
    /// Note this is the surface RGB only — the emissive (city-light) channel
    /// `render_globe_texture` puts in alpha is dropped, because an image
    /// viewer composites that as transparency and shows a near-blank sheet.
    ///
    /// `cargo test --lib --release worldmap::globe::tests::dump_globe_texture -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dump_globe_texture() {
        for (name, uwp) in [("garden", "A788899-A"), ("earth", "C886977-8")] {
            let map = super::super::generate(uwp, 1, None).unwrap();
            let ts = TexSize::HIGH;
            let tex = build_equirect_texture(&map, ts.w, ts.h);
            let mut rgba = Vec::with_capacity(tex.rgb.len() / 3 * 4);
            for px in tex.rgb.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            let png = encode_png_rgba(&rgba, ts.w, ts.h).unwrap();
            let path = format!("/tmp/globe_tex_{name}.png");
            std::fs::write(&path, &png).unwrap();
            // Also report what the endpoint actually ships: same pixels, but
            // the real emissive alpha and Compression::Best. That number is
            // the one third-party consumers pay for on every cache miss, so
            // it's worth seeing next to the size bump whenever TexSize moves.
            //
            // Note this builds the texture a second time, so don't read this
            // test's wall time as the cost of one render — halve it.
            let served = render_globe_texture(&map, ts, true).unwrap();
            eprintln!(
                "wrote {path} ({} B opaque) — endpoint payload at {}x{}: {} B",
                png.len(),
                ts.w,
                ts.h,
                served.len()
            );
        }
    }

    /// eyeballing. Ignored by default.
    /// `cargo test --lib worldmap::globe::tests::dump_globe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dump_globe() {
        let cases = [
            ("garden", "A788899-A"),
            ("earth", "C886977-8"),
            ("waterworld", "A78A899-A"),
            ("desert", "A780899-A"),
            ("urban", "A8888AA-A"), // pop A — lots of night-side city lights
        ];
        for (name, uwp) in cases {
            let map = super::super::generate(uwp, 1, None).unwrap();
            let png = render_globe_png(&map, 512, 0.0, TexSize::HIGH).unwrap();
            let apng = render_globe_apng(&map, 400, ApngTiming::DEFAULT, TexSize::HIGH).unwrap();
            std::fs::write(format!("/tmp/globe_{name}.png"), &png).unwrap();
            std::fs::write(format!("/tmp/globe_{name}.apng.png"), &apng).unwrap();
            eprintln!(
                "wrote /tmp/globe_{name}.png ({} B) and /tmp/globe_{name}.apng.png ({} B)",
                png.len(),
                apng.len()
            );
        }
    }
}
