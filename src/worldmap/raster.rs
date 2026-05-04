//! Per-pixel terrain rasterizer.
//!
//! Each output pixel inside the icosahedron silhouette is mapped to a 3D
//! sphere position via a global equirectangular projection — `y` → latitude,
//! `x` → longitude — independent of which icosahedron face contains it.
//! This is essential because the Mongoose 5+10+5 unfolding has stylized
//! seams (horizontal at y=h and y=2h, plus alternating diagonals in the
//! equator zigzag) where the same 2D point maps to *different* 3D vertices
//! per-face; sampling per-face barycentric there causes hard land/ocean
//! discontinuities. A single global mapping makes terrain flow smoothly
//! across every seam. The face triangles are kept only as the silhouette
//! mask so the icosahedron shape still reads.
//!
//! Output is a flat RGBA8 buffer in row-major order (width × height pixels).

use super::WorldMap;
use super::climate;
use super::colormap;
use super::grid::{Face, SHEET_HEIGHT, SHEET_WIDTH, xy_to_sphere};
use super::noise::ElevationField;

/// Background page color — a soft warm gray so the icosahedron silhouette
/// reads as floating on a printed page rather than pasted on space.
/// Was nearly-black (8, 10, 18) which made the map feel like a screenshot
/// of a planetarium; the light page tone makes it look like a published
/// map.
pub const SPACE_RGB: (u8, u8, u8) = (231, 232, 230);

/// 2D ring radius (sheet px) sampled around each land point to estimate
/// continentality. ~3.5% of SHEET_WIDTH ≈ 13° lon ≈ 1400 km on Earth.
const CONT_OFFSET_PX: f64 = 35.0;
/// Max humidity drop applied at full interior (all 6 ring samples land).
const CONT_DRYING: f64 = 0.18;

/// 4 diagonal offsets — fewer than the original 6 to halve continentality
/// compute (this is the dominant cost of PNG generation). The diagonal
/// rotation still gives reasonable rotational symmetry without doubling
/// up on cardinal directions.
const CONT_OFFSETS: [(f64, f64); 4] = [
    (0.7, 0.7),
    (-0.7, 0.7),
    (0.7, -0.7),
    (-0.7, -0.7),
];

/// Fraction of a small ring of 2D offsets that are also land.
/// 0 = surrounded by ocean, 1 = deep interior. Land-only; gate at caller.
pub fn continentality(elev_field: &ElevationField, sea_level: f64, sx: f64, sy: f64) -> f64 {
    let mut land = 0u32;
    for (dx, dy) in CONT_OFFSETS {
        let nx = sx + dx * CONT_OFFSET_PX;
        let ny = sy + dy * CONT_OFFSET_PX;
        let sphere = xy_to_sphere(nx, ny);
        if elev_field.sample(&sphere) > sea_level {
            land += 1;
        }
    }
    land as f64 / CONT_OFFSETS.len() as f64
}

/// Apply continentality drying to a humidity value. No-op for ocean callers
/// (they should gate on `above > 0.0` before calling).
pub fn apply_continentality(h: f64, cont: f64) -> f64 {
    (h - CONT_DRYING * cont).clamp(0.0, 1.0)
}

/// CONT_OFFSETS expressed in raster-pixel coordinates for the current
/// raster's scale. Computed once per render and reused per land pixel so
/// the inner loop is just integer arithmetic.
fn continentality_pixel_offsets(scale_x: f64, scale_y: f64) -> [(i32, i32); 4] {
    let mut out = [(0, 0); 4];
    for (i, (dx, dy)) in CONT_OFFSETS.iter().enumerate() {
        let nx = (dx * CONT_OFFSET_PX / scale_x).round() as i32;
        let ny = (dy * CONT_OFFSET_PX / scale_y).round() as i32;
        out[i] = (nx, ny);
    }
    out
}

/// Continentality computed from the precomputed elev[] grid: the fraction
/// of the four ring-offset samples that fall on land. Used in place of
/// `continentality` when we have a populated elevation grid (i.e. inside
/// `render_terrain`'s Pass 1B). Each lookup is O(1) — no noise sampling —
/// which is a ~4× reduction in per-pixel noise work for land pixels.
fn continentality_from_grid(
    elev: &[f32],
    width: i32,
    height: i32,
    px: i32,
    py: i32,
    offsets: &[(i32, i32); 4],
) -> f64 {
    let mut land = 0u32;
    for (dx, dy) in offsets {
        let nx = px + dx;
        let ny = py + dy;
        if nx < 0 || nx >= width || ny < 0 || ny >= height {
            continue;
        }
        let ni = (ny * width + nx) as usize;
        let e = elev[ni];
        if e.is_finite() && e > 0.0 {
            land += 1;
        }
    }
    land as f64 / offsets.len() as f64
}

/// Resumable per-pixel rasterizer. Splits `render_terrain` into four
/// callable steps so callers running on the WASM main thread can yield
/// back to the browser between phases — without that, a single
/// regenerate pegs the main thread for 1–2s on a fast machine and well
/// past Chrome's 5s "Page Unresponsive" threshold on slow ones.
///
/// Phases (each can stand alone in a setTimeout(0) tick):
///   1. `step_elevation` — populate `elev[i]` for every silhouette pixel.
///   2. `step_color` — turn elevation + climate into a base color per pixel.
///   3. `step_postprocess` — hillshade + coastline + drop shadow + paper.
///   4. `into_rgba` — give back the RGBA8 buffer; consumes self.
///
/// The synchronous `render_terrain` helper still exists and just runs all
/// four in a row, so existing callers (tests, native code) don't change.
pub struct RasterJob {
    width: u32,
    height: u32,
    bounded: Vec<BoundedTri>,
    elev: Vec<f32>,
    color: Vec<(u8, u8, u8)>,
    rgba: Vec<u8>,
    scale_x: f64,
    scale_y: f64,
    cont_offsets: [(i32, i32); 4],
}

impl RasterJob {
    pub fn new(map: &WorldMap, width: u32, height: u32) -> Self {
        let n = (width as usize) * (height as usize);
        let scale_x = SHEET_WIDTH / width as f64;
        let scale_y = SHEET_HEIGHT / height as f64;
        Self {
            width,
            height,
            bounded: collect_faces(&map.grid.faces),
            elev: vec![f32::NAN; n],
            color: vec![SPACE_RGB; n],
            rgba: vec![0u8; n * 4],
            scale_x,
            scale_y,
            cont_offsets: continentality_pixel_offsets(scale_x, scale_y),
        }
    }

    /// Phase 1A: populate `elev[i]` with above-sea elevation for every
    /// silhouette pixel; space pixels stay NaN. Doing the whole grid first
    /// lets Phase 1B answer continentality with grid lookups instead of
    /// four fresh `elev_field.sample` calls per land pixel — the dominant
    /// cost in the original implementation.
    pub fn step_elevation(&mut self, map: &WorldMap) {
        for py in 0..self.height {
            let sy = (py as f64 + 0.5) * self.scale_y;
            for px in 0..self.width {
                let sx = (px as f64 + 0.5) * self.scale_x;
                if !point_in_silhouette(&self.bounded, sx, sy) {
                    continue;
                }
                let sphere = xy_to_sphere(sx, sy);
                let e = map.elev_field.sample(&sphere);
                // Non-linear amplification of above-sea elevation: keeps
                // coastal plains flat while exaggerating mountain ranges
                // so they push past lapse/colormap thresholds into varied
                // biomes.
                let above = climate::amplify_elevation(e - map.sea_level, map.uwp.hydrographics());
                let i = (py * self.width + px) as usize;
                self.elev[i] = above as f32;
            }
        }
    }

    /// Phase 1B: per silhouette pixel, derive temperature + humidity (with
    /// rain-shadow, altitude drying, and grid-driven continentality) and
    /// fold them through the colormap into a base biome color.
    pub fn step_color(&mut self, map: &WorldMap) {
        let tectonics = map.elev_field.tectonics();
        let w_i32 = self.width as i32;
        let h_i32 = self.height as i32;
        for py in 0..self.height {
            let sy = (py as f64 + 0.5) * self.scale_y;
            for px in 0..self.width {
                let i = (py * self.width + px) as usize;
                let above = self.elev[i];
                if !above.is_finite() {
                    continue;
                }
                let sx = (px as f64 + 0.5) * self.scale_x;
                let sphere = xy_to_sphere(sx, sy);
                let above = above as f64;

                let raw_t = climate::temperature_at_wobbled(&sphere, &map.temp_field);
                let t = climate::apply_lapse(climate::adjust_temperature(raw_t, &map.uwp), above, &map.uwp);

                let mut h = map.humidity_field.sample(&sphere, &map.uwp);
                if let Some(tec) = tectonics {
                    h = colormap::rain_shadow_adjustment(h, tec.rain_shadow_at(&sphere));
                }
                h = climate::apply_altitude_drying(h, above);
                if above > 0.0 {
                    let cont = continentality_from_grid(
                        &self.elev, w_i32, h_i32, px as i32, py as i32, &self.cont_offsets,
                    );
                    h = apply_continentality(h, cont);
                }
                self.color[i] = colormap::elevation_color(above, t, h);
            }
        }
    }

    /// Phase 2: hillshade + coastline tide + page-side drop shadow + paper
    /// texture. All passes are whole-buffer per-pixel scans over the
    /// already-populated `color`/`elev` grids — no noise sampling, so this
    /// step is markedly cheaper than the two before it.
    pub fn step_postprocess(&mut self) {
        let w = self.width as usize;
        let h = self.height as usize;

        // Pass 2: hillshade land pixels + faint tide-line on ocean pixels
        // adjacent to land.
        for py in 0..h {
            for px in 0..w {
                let i = py * w + px;
                let mut c = self.color[i];

                if !self.elev[i].is_nan() {
                    let il = if px > 0 { i - 1 } else { i };
                    let ir = if px + 1 < w { i + 1 } else { i };
                    let iu = if py > 0 { i - w } else { i };
                    let id = if py + 1 < h { i + w } else { i };

                    let el = nan_or(self.elev[il], self.elev[i]);
                    let er = nan_or(self.elev[ir], self.elev[i]);
                    let eu = nan_or(self.elev[iu], self.elev[i]);
                    let ed = nan_or(self.elev[id], self.elev[i]);

                    if self.elev[i] > 0.0 {
                        const SHADE_GAIN: f64 = 30.0;
                        let dx = (er - el) as f64 * SHADE_GAIN;
                        let dy = (ed - eu) as f64 * SHADE_GAIN;
                        const FLAT_LIMIT: f64 = 0.20;
                        const FULL_LIMIT: f64 = 0.50;
                        let slope = (dx * dx + dy * dy).sqrt();
                        let tt = ((slope - FLAT_LIMIT) / (FULL_LIMIT - FLAT_LIMIT))
                            .clamp(0.0, 1.0);
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
                        let any_land = el > 0.0 || er > 0.0 || eu > 0.0 || ed > 0.0;
                        if any_land {
                            const TIDE: (u8, u8, u8) = (180, 198, 220);
                            c = (
                                lerp_byte(c.0, TIDE.0, 0.45),
                                lerp_byte(c.1, TIDE.1, 0.45),
                                lerp_byte(c.2, TIDE.2, 0.45),
                            );
                        }
                    }
                }

                let pi = i * 4;
                self.rgba[pi] = c.0;
                self.rgba[pi + 1] = c.1;
                self.rgba[pi + 2] = c.2;
                self.rgba[pi + 3] = 255;
            }
        }

        // Pass 3: drop shadow on page-side pixels offset (-3, -3) from a
        // silhouette pixel.
        const SHADOW_OFFSET: usize = 3;
        const SHADOW_DARKEN: f64 = 0.10;
        for py in SHADOW_OFFSET..h {
            for px in SHADOW_OFFSET..w {
                let i = py * w + px;
                if !self.elev[i].is_nan() {
                    continue;
                }
                let si = (py - SHADOW_OFFSET) * w + (px - SHADOW_OFFSET);
                if self.elev[si].is_nan() {
                    continue;
                }
                let pi = i * 4;
                self.rgba[pi] = (self.rgba[pi] as f64 * (1.0 - SHADOW_DARKEN)) as u8;
                self.rgba[pi + 1] = (self.rgba[pi + 1] as f64 * (1.0 - SHADOW_DARKEN)) as u8;
                self.rgba[pi + 2] = (self.rgba[pi + 2] as f64 * (1.0 - SHADOW_DARKEN)) as u8;
            }
        }

        // Pass 4: deterministic ±2 LSB paper jitter on page pixels.
        for py in 0..h {
            for px in 0..w {
                let i = py * w + px;
                if !self.elev[i].is_nan() {
                    continue;
                }
                let hash = (px as u32)
                    .wrapping_mul(0x9E37_79B9)
                    ^ (py as u32).wrapping_mul(0xC2B2_AE35);
                let n = ((hash % 5) as i32) - 2;
                let pi = i * 4;
                self.rgba[pi] = (self.rgba[pi] as i32 + n).clamp(0, 255) as u8;
                self.rgba[pi + 1] = (self.rgba[pi + 1] as i32 + n).clamp(0, 255) as u8;
                self.rgba[pi + 2] = (self.rgba[pi + 2] as i32 + n).clamp(0, 255) as u8;
            }
        }
    }

    /// Take ownership of the finished RGBA8 buffer.
    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }
}

pub fn render_terrain(map: &WorldMap, width: u32, height: u32) -> Vec<u8> {
    let mut job = RasterJob::new(map, width, height);
    job.step_elevation(map);
    job.step_color(map);
    job.step_postprocess();
    job.into_rgba()
}

fn nan_or(v: f32, fallback: f32) -> f32 {
    if v.is_nan() { fallback } else { v }
}

fn lerp_byte(a: u8, b: u8, t: f64) -> u8 {
    (a as f64 + (b as f64 - a as f64) * t).clamp(0.0, 255.0) as u8
}

struct BoundedTri {
    aabb: (f64, f64, f64, f64),
    tri: [(f64, f64); 3],
}

fn collect_faces(faces: &[Face]) -> Vec<BoundedTri> {
    faces
        .iter()
        .flat_map(|f| {
            f.unfolded_positions
                .iter()
                .map(|tri| BoundedTri {
                    aabb: aabb_of(tri),
                    tri: *tri,
                })
        })
        .collect()
}

fn point_in_silhouette(bounded: &[BoundedTri], x: f64, y: f64) -> bool {
    for bt in bounded {
        let (xmin, ymin, xmax, ymax) = bt.aabb;
        if x < xmin || x > xmax || y < ymin || y > ymax {
            continue;
        }
        if barycentric_2d(&bt.tri, x, y).is_some() {
            return true;
        }
    }
    false
}

fn aabb_of(tri: &[(f64, f64); 3]) -> (f64, f64, f64, f64) {
    let xs = [tri[0].0, tri[1].0, tri[2].0];
    let ys = [tri[0].1, tri[1].1, tri[2].1];
    (
        xs.iter().cloned().fold(f64::INFINITY, f64::min),
        ys.iter().cloned().fold(f64::INFINITY, f64::min),
        xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    )
}

fn barycentric_2d(tri: &[(f64, f64); 3], x: f64, y: f64) -> Option<[f64; 3]> {
    let (x0, y0) = tri[0];
    let (x1, y1) = tri[1];
    let (x2, y2) = tri[2];
    let denom = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2);
    if denom.abs() < 1e-12 {
        return None;
    }
    let a = ((y1 - y2) * (x - x2) + (x2 - x1) * (y - y2)) / denom;
    let b = ((y2 - y0) * (x - x2) + (x0 - x2) * (y - y2)) / denom;
    let c = 1.0 - a - b;
    if a >= -1e-9 && b >= -1e-9 && c >= -1e-9 {
        Some([a.max(0.0), b.max(0.0), c.max(0.0)])
    } else {
        None
    }
}
