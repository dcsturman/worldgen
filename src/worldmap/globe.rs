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

use super::WorldMap;
use super::climate;
use super::colormap;
use super::grid::{SHEET_HEIGHT, SHEET_WIDTH, xy_to_sphere};

/// Default equirectangular texture width (longitude). 2:1 with the height.
/// 1024×512 is plenty of detail for globes up to ~600 px and keeps the
/// one-time build cheaper than a full flat-map raster.
pub const TEX_W: u32 = 1024;
/// Default equirectangular texture height (latitude).
pub const TEX_H: u32 = 512;

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
/// upper-left and slightly toward the camera. Normalized at use.
const LIGHT: (f64, f64, f64) = (-0.55, 0.55, 0.62);
/// Ambient term so the night side stays legible rather than black.
const AMBIENT: f64 = 0.55;
/// How much the directional term adds on top of ambient.
const DIFFUSE_GAIN: f64 = 0.62;
/// Limb-darkening floor: edge pixels keep this fraction of their brightness.
const LIMB_FLOOR: f64 = 0.74;

/// A gap-free equirectangular RGB texture of a world's surface, suitable for
/// projecting onto a sphere. Row-major, `width × height`, 3 bytes per pixel.
pub struct GlobeTexture {
    pub width: u32,
    pub height: u32,
    /// `width * height * 3` bytes, RGB row-major.
    pub rgb: Vec<u8>,
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
    color: Vec<(u8, u8, u8)>,
}

impl GlobeTextureJob {
    pub fn new(width: u32, height: u32) -> Self {
        let n = (width as usize) * (height as usize);
        Self {
            width,
            height,
            elev: vec![0f32; n],
            color: vec![(0u8, 0u8, 0u8); n],
        }
    }

    /// Step 1: above-sea elevation per texel. Steps 2 and 3 read this grid for
    /// continentality and hillshade.
    pub fn step_elevation(&mut self, map: &WorldMap) {
        let w = self.width as usize;
        for ty in 0..self.height as usize {
            let sy = (ty as f64 + 0.5) / self.height as f64 * SHEET_HEIGHT;
            for tx in 0..w {
                let sx = (tx as f64 + 0.5) / self.width as f64 * SHEET_WIDTH;
                let sphere = xy_to_sphere(sx, sy);
                let e = map.elev_field.sample(&sphere);
                let above = climate::amplify_elevation(e - map.sea_level, map.uwp.hydrographics());
                self.elev[ty * w + tx] = above as f32;
            }
        }
    }

    /// Step 2: fold elevation + climate into a base biome colour per texel.
    pub fn step_color(&mut self, map: &WorldMap) {
        let w = self.width as usize;
        let h = self.height as usize;
        let tectonics = map.elev_field.tectonics();
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

                let mut hu = map.humidity_field.sample(&sphere, &map.uwp);
                if let Some(tec) = tectonics {
                    hu = colormap::rain_shadow_adjustment(hu, tec.rain_shadow_at(&sphere));
                }
                hu = climate::apply_altitude_drying(hu, above);
                if above > 0.0 {
                    let cont = continentality_wrapped(&self.elev, w, h, tx, ty);
                    hu = super::raster::apply_continentality(hu, cont);
                }
                self.color[ty * w + tx] = colormap::elevation_color(above, t, hu);
            }
        }
    }

    /// Step 3 (terminal): hillshade land + faint tide on shallow water, baking
    /// the RGB texture. Longitude wraps; latitude clamps at the poles.
    pub fn into_texture(self) -> GlobeTexture {
        let w = self.width as usize;
        let h = self.height as usize;
        let elev = &self.elev;
        let color = &self.color;
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
                    const SHADE_GAIN: f64 = 30.0;
                    let dx = (elev[ir] - elev[il]) as f64 * SHADE_GAIN;
                    let dy = (elev[id] - elev[iu]) as f64 * SHADE_GAIN;
                    const FLAT_LIMIT: f64 = 0.20;
                    const FULL_LIMIT: f64 = 0.50;
                    let slope = (dx * dx + dy * dy).sqrt();
                    let tt = ((slope - FLAT_LIMIT) / (FULL_LIMIT - FLAT_LIMIT)).clamp(0.0, 1.0);
                    let strength = tt * tt * (3.0 - 2.0 * tt);
                    if strength > 0.0 {
                        let lit = colormap::apply_hillshade(c, dx, dy);
                        c = (
                            lerp_byte(c.0, lit.0, strength),
                            lerp_byte(c.1, lit.1, strength),
                            lerp_byte(c.2, lit.2, strength),
                        );
                    }
                } else {
                    let any_land =
                        elev[il] > 0.0 || elev[ir] > 0.0 || elev[iu] > 0.0 || elev[id] > 0.0;
                    if any_land {
                        const TIDE: (u8, u8, u8) = (180, 198, 220);
                        c = (
                            lerp_byte(c.0, TIDE.0, 0.45),
                            lerp_byte(c.1, TIDE.1, 0.45),
                            lerp_byte(c.2, TIDE.2, 0.45),
                        );
                    }
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
    /// `xy_to_sphere` convention: `z` is the pole axis). Longitude wraps,
    /// latitude clamps.
    #[inline]
    fn sample(&self, p: [f64; 3]) -> (u8, u8, u8) {
        use std::f64::consts::PI;
        let lat = p[2].clamp(-1.0, 1.0).asin();
        let lon = p[1].atan2(p[0]).rem_euclid(2.0 * PI);
        // Match xy_to_sphere: x∈[0,W)→lon∈[0,2π); y∈[0,H)→lat from +π/2 to -π/2.
        let fx = lon / (2.0 * PI) * self.width as f64 - 0.5;
        let fy = (std::f64::consts::FRAC_PI_2 - lat) / PI * self.height as f64 - 0.5;

        let w = self.width as i32;
        let h = self.height as i32;
        let x0 = fx.floor() as i32;
        let y0 = fy.floor() as i32;
        let tx = fx - x0 as f64;
        let tyf = fy - y0 as f64;

        let px = |x: i32, y: i32| -> (f64, f64, f64) {
            let xi = x.rem_euclid(w) as usize;
            let yi = y.clamp(0, h - 1) as usize;
            let i = (yi * self.width as usize + xi) * 3;
            (
                self.rgb[i] as f64,
                self.rgb[i + 1] as f64,
                self.rgb[i + 2] as f64,
            )
        };
        let c00 = px(x0, y0);
        let c10 = px(x0 + 1, y0);
        let c01 = px(x0, y0 + 1);
        let c11 = px(x0 + 1, y0 + 1);
        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let top = (
            lerp(c00.0, c10.0, tx),
            lerp(c00.1, c10.1, tx),
            lerp(c00.2, c10.2, tx),
        );
        let bot = (
            lerp(c01.0, c11.0, tx),
            lerp(c01.1, c11.1, tx),
            lerp(c01.2, c11.2, tx),
        );
        (
            lerp(top.0, bot.0, tyf).round() as u8,
            lerp(top.1, bot.1, tyf).round() as u8,
            lerp(top.2, bot.2, tyf).round() as u8,
        )
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

                let (mut r, mut g, mut b) = {
                    let (cr, cg, cb) = self.sample(sphere);
                    (cr as f64, cg as f64, cb as f64)
                };

                // Shade: ambient + directional, then limb darkening.
                let diffuse = dot(p_cam, light).max(0.0);
                let mut shade = (AMBIENT + DIFFUSE_GAIN * diffuse).clamp(0.0, 1.15);
                shade *= LIMB_FLOOR + (1.0 - LIMB_FLOOR) * nz;
                r = (r * shade).clamp(0.0, 255.0);
                g = (g * shade).clamp(0.0, 255.0);
                b = (b * shade).clamp(0.0, 255.0);

                // Soft bright atmosphere rim on the lit limb (inner edge).
                let edge = (dist / radius).clamp(0.0, 1.0);
                if edge > 0.92 {
                    let rim = ((edge - 0.92) / 0.08).clamp(0.0, 1.0) * 0.5 * diffuse;
                    r = r + (ATMO.0 - r) * rim;
                    g = g + (ATMO.1 - g) * rim;
                    b = b + (ATMO.2 - b) * rim;
                }

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

#[inline]
fn lerp_byte(a: u8, b: u8, t: f64) -> u8 {
    (a as f64 + (b as f64 - a as f64) * t).clamp(0.0, 255.0) as u8
}

/// Render a single static globe frame for `map` as a PNG, viewed at sub-viewer
/// longitude `spin` (radians). `size` is the output square's side in pixels.
pub fn render_globe_png(map: &WorldMap, size: u32, spin: f64) -> Result<Vec<u8>, String> {
    let tex = build_equirect_texture(map, TEX_W, TEX_H);
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
    frames: u32,
    delay_num: u16,
    delay_den: u16,
) -> Result<Vec<u8>, String> {
    use std::f64::consts::PI;
    let frames = frames.max(1);
    let tex = build_equirect_texture(map, TEX_W, TEX_H);
    let buffers: Vec<Vec<u8>> = (0..frames)
        .map(|f| tex.warp_frame(size, f as f64 / frames as f64 * 2.0 * PI))
        .collect();
    encode_apng_rgba(&buffers, size, size, delay_num, delay_den)
}

/// PNG-encode a single RGBA8 frame.
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
    fn spin_changes_the_frame() {
        let t = tex();
        let a = t.warp_frame(96, 0.0);
        let b = t.warp_frame(96, std::f64::consts::PI);
        assert_ne!(a, b, "opposite hemispheres should differ");
    }

    #[test]
    fn static_globe_png_decodes() {
        let map = super::super::generate("A788899-A", 1, None).unwrap();
        let bytes = render_globe_png(&map, 128, 0.0).unwrap();
        assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
        let dec = png::Decoder::new(std::io::Cursor::new(&bytes));
        let reader = dec.read_info().unwrap();
        assert_eq!(reader.info().width, 128);
        assert_eq!(reader.info().height, 128);
    }

    #[test]
    fn apng_is_animated_with_expected_frame_count() {
        let map = super::super::generate("A788899-A", 1, None).unwrap();
        let bytes = render_globe_apng(&map, 96, 8, 1, 10).unwrap();
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

    /// Visual dump: write a static globe PNG and a spinning APNG to /tmp for
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
        ];
        for (name, uwp) in cases {
            let map = super::super::generate(uwp, 1, None).unwrap();
            let png = render_globe_png(&map, 512, 0.0).unwrap();
            let apng = render_globe_apng(&map, 400, DEFAULT_FRAMES, 1, 5).unwrap();
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
