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

/// Background "space" color matching the existing renderer.
pub const SPACE_RGB: (u8, u8, u8) = (8, 10, 18);

/// 2D ring radius (sheet px) sampled around each land point to estimate
/// continentality. ~3.5% of SHEET_WIDTH ≈ 13° lon ≈ 1400 km on Earth.
const CONT_OFFSET_PX: f64 = 35.0;
/// Max humidity drop applied at full interior (all 6 ring samples land).
const CONT_DRYING: f64 = 0.18;

/// 6 cardinal+diagonal offsets — cheap and rotationally fair.
const CONT_OFFSETS: [(f64, f64); 6] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (0.7, 0.7),
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

pub fn render_terrain(map: &WorldMap, width: u32, height: u32) -> Vec<u8> {
    let n = (width as usize) * (height as usize);
    let mut rgba = vec![0u8; n * 4];

    let mut elev = vec![f32::NAN; n];
    let mut color = vec![SPACE_RGB; n];

    let bounded = collect_faces(&map.grid.faces);

    let scale_x = SHEET_WIDTH / width as f64;
    let scale_y = SHEET_HEIGHT / height as f64;

    // Pass 1: classify each pixel via the global equirectangular mapping.
    // Per-pixel pipeline (in order):
    //   sphere ← xy_to_sphere(sx, sy)
    //   elevation ← elev_field.sample (already includes tectonic uplift + warp)
    //   temperature ← latitude-curve → atmosphere bias → lapse-rate cooling
    //   humidity ← noise+UWP bias → tectonic rain-shadow → altitude drying
    //   color ← colormap(elev, temp, humidity)
    let tectonics = map.elev_field.tectonics();
    for py in 0..height {
        let sy = (py as f64 + 0.5) * scale_y;
        for px in 0..width {
            let sx = (px as f64 + 0.5) * scale_x;
            if !point_in_silhouette(&bounded, sx, sy) {
                continue;
            }
            let sphere = xy_to_sphere(sx, sy);
            let e = map.elev_field.sample(&sphere);
            // Non-linear amplification of above-sea elevation: keeps
            // coastal plains flat while exaggerating mountain ranges so
            // they push past lapse/colormap thresholds into varied biomes.
            let above = climate::amplify_elevation(e - map.sea_level);

            let raw_t = climate::temperature_at(&sphere, climate::TEMP_AMPLITUDE);
            let t = climate::apply_lapse(climate::adjust_temperature(raw_t, &map.uwp), above);

            let mut h = map.humidity_field.sample(&sphere, &map.uwp);
            if let Some(tec) = tectonics {
                h = colormap::rain_shadow_adjustment(h, tec.rain_shadow_at(&sphere));
            }
            h = climate::apply_altitude_drying(h, above);
            // Continentality: deep interiors are drier than coasts. Land only.
            if above > 0.0 {
                let cont = continentality(&map.elev_field, map.sea_level, sx, sy);
                h = apply_continentality(h, cont);
            }

            let i = (py * width + px) as usize;
            elev[i] = above as f32;
            color[i] = colormap::elevation_color(above, t, h);
        }
    }

    // Pass 2: hillshade land pixels using neighbor elevation differences.
    let w = width as usize;
    let h = height as usize;
    for py in 0..h {
        for px in 0..w {
            let i = py * w + px;
            let mut c = color[i];

            if !elev[i].is_nan() && elev[i] > 0.0 {
                let il = if px > 0 { i - 1 } else { i };
                let ir = if px + 1 < w { i + 1 } else { i };
                let iu = if py > 0 { i - w } else { i };
                let id = if py + 1 < h { i + w } else { i };

                let el = nan_or(elev[il], elev[i]);
                let er = nan_or(elev[ir], elev[i]);
                let eu = nan_or(elev[iu], elev[i]);
                let ed = nan_or(elev[id], elev[i]);

                const SHADE_GAIN: f64 = 80.0;
                let dx = (er - el) as f64 * SHADE_GAIN;
                let dy = (ed - eu) as f64 * SHADE_GAIN;
                c = colormap::apply_hillshade(c, dx, dy);
            }

            let pi = i * 4;
            rgba[pi] = c.0;
            rgba[pi + 1] = c.1;
            rgba[pi + 2] = c.2;
            rgba[pi + 3] = 255;
        }
    }

    rgba
}

fn nan_or(v: f32, fallback: f32) -> f32 {
    if v.is_nan() { fallback } else { v }
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
