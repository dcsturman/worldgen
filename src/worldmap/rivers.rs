//! Major-river detection and rendering paths.
//!
//! Builds a downhill flow graph from the finalized elevation, accumulates
//! drainage area downstream, and thresholds for "visible-from-orbit" rivers.
//! Output is a list of `RiverPath`s in unfolded sheet coordinates so the
//! renderer can stroke them as polylines on top of the terrain raster.
//!
//! Algorithm (terse):
//!   1. Sample sphere via per-face barycentric subdivision (`SUB_PER_EDGE`
//!      points per face edge → land-eligible candidates).
//!   2. For each LAND sample, find steepest-downhill neighbor among nearby
//!      samples (angular cutoff ≈ 1.6× sample spacing).
//!   3. Topo-sort by elevation desc; push unit drainage downstream.
//!   4. Threshold against a high percentile to keep only "major" rivers.
//!   5. Walk downstream from each thresholded source (whose upstream isn't
//!      thresholded) until ocean or basin; emit polyline per face crossed.

use ::noise::{Fbm, MultiFractal, NoiseFn, Simplex};

use super::WorldMap;
use super::grid::{Face, HEXES_PER_EDGE, SHEET_WIDTH, sphere_to_xy};

/// One river polyline in unfolded sheet coordinates. Multi-segment because
/// the same logical river may cross the equator-zigzag seam and need to be
/// drawn twice (once on each side of the wrap).
#[derive(Clone, Debug, Default)]
pub struct RiverPath {
    /// One polyline per visible copy of the river. For a non-seam river,
    /// one entry; for a river that crosses the seam, two.
    pub strokes: Vec<Vec<(f64, f64)>>,
    /// Drainage area at the mouth, in sphere-area units. Drives stroke
    /// width — major rivers stroked fatter than tributaries.
    pub mouth_drainage: f64,
}

/// Sub-samples per face edge. 2× hex resolution gives ~ N(N+1)/2 = 105 per
/// face × 20 faces = 2100 samples — plenty of detail without blowing the
/// 500ms budget on the O(N²) neighbor search.
const SUB_PER_EDGE: usize = HEXES_PER_EDGE * 2;

/// Source sample on the sphere with attached flow data.
#[derive(Clone, Debug)]
struct Sample {
    face_idx: usize,
    sphere_pos: [f64; 3],
    elev: f64,
    flow_to: Option<usize>,
    drainage: f64,
}

/// Compute major rivers for a finished map. Reads `map.elev_field` and
/// `map.sea_level`; everything tectonics has done is already baked into
/// the elevation samples.
pub fn compute_rivers(map: &WorldMap) -> Vec<RiverPath> {
    // Hyd 0/1 worlds (Mars-like deserts) don't have flowing surface
    // water. Whatever liquid exists is in scattered lakes; no rivers.
    if map.uwp.hydrographics() < 2 {
        return Vec::new();
    }

    let samples = build_samples(map);
    if samples.is_empty() {
        return Vec::new();
    }

    // Quick exit if there's negligible land (waterworld). Also skips
    // worlds where stray peaks above sea_level form micro-islands too
    // small to support a true river.
    let land_count = samples.iter().filter(|s| s.elev > map.sea_level).count();
    let land_frac = land_count as f64 / samples.len() as f64;
    if land_frac < 0.05 {
        return Vec::new();
    }

    let mut samples = compute_flow(samples, map.sea_level);
    accumulate_drainage(&mut samples);

    let threshold = pick_threshold(&samples, map.sea_level);
    if threshold.is_none() {
        return Vec::new();
    }
    let threshold = threshold.unwrap();

    trace_rivers(
        &samples,
        &map.grid.faces,
        map.sea_level,
        threshold,
        map.seed,
        &map.elev_field,
    )
}

// ---------- 1. sphere sampling ----------

/// Generate samples on a regular barycentric subdivision per face. Skips
/// face vertex/edge duplicates by jittering inward (sub-triangle centroids).
fn build_samples(map: &WorldMap) -> Vec<Sample> {
    let n = SUB_PER_EDGE;
    let mut out = Vec::with_capacity(map.grid.faces.len() * n * (n + 1) / 2);

    for face in &map.grid.faces {
        let canon = &face.unfolded_positions[0];
        for (_, _, bary) in face_subgrid_barycentric(n) {
            // Project barycentric → canonical 2D, then through the same
            // equirectangular mapping the rasterizer uses, so river
            // elevation samples agree with the visible terrain. Sampling
            // via per-face 3D vertices would put river paths on land that
            // is ocean in the rendered pixmap.
            let canon_2d = (
                bary[0] * canon[0].0 + bary[1] * canon[1].0 + bary[2] * canon[2].0,
                bary[0] * canon[0].1 + bary[1] * canon[1].1 + bary[2] * canon[2].1,
            );
            let sphere = super::grid::xy_to_sphere(canon_2d.0, canon_2d.1);
            let elev = map.elev_field.sample(&sphere);
            out.push(Sample {
                face_idx: face.idx,
                sphere_pos: sphere,
                elev,
                flow_to: None,
                drainage: 0.0,
            });
        }
    }
    out
}

/// Same shape as `iter_face_hex_barycentric` but for arbitrary `n`. We
/// keep a private copy to decouple from the hex-grid module.
fn face_subgrid_barycentric(n: usize) -> impl Iterator<Item = (usize, usize, [f64; 3])> {
    (0..n).flat_map(move |i| {
        (0..=i).map(move |j| {
            let nf = n as f64;
            let c_apex = (nf - i as f64 - 0.5) / nf;
            let split = (i as f64 + 0.5) / nf;
            let row_pos = (j as f64 + 0.5) / (i as f64 + 1.0);
            let a = (1.0 - row_pos) * split;
            let b = row_pos * split;
            (i, j, [a, b, c_apex])
        })
    })
}

// ---------- 2. flow graph ----------

/// For each land sample, pick the steepest-descent neighbor within an
/// angular cutoff. Ocean samples have flow_to = None (terminal).
fn compute_flow(mut samples: Vec<Sample>, sea_level: f64) -> Vec<Sample> {
    // Angular spacing for sub-grid samples is roughly the icosahedron-edge
    // arc / SUB_PER_EDGE. Edge arc ≈ 1.107 rad. Use 1.6× as cutoff.
    let edge_arc = 1.107_148_717_794_090_5_f64;
    let cutoff = 1.6 * edge_arc / SUB_PER_EDGE as f64;
    // Compare via dot products: cos(theta) > cos(cutoff) ⇔ within range.
    let cos_cutoff = cutoff.cos();

    let n = samples.len();
    for i in 0..n {
        if samples[i].elev <= sea_level {
            continue;
        }
        let here = samples[i].sphere_pos;
        let here_elev = samples[i].elev;
        let mut best: Option<usize> = None;
        let mut best_drop = 0.0;
        for j in 0..n {
            if i == j {
                continue;
            }
            let sj = &samples[j];
            let dot = here[0] * sj.sphere_pos[0]
                + here[1] * sj.sphere_pos[1]
                + here[2] * sj.sphere_pos[2];
            if dot < cos_cutoff {
                continue;
            }
            let drop = here_elev - sj.elev;
            if drop > best_drop {
                best_drop = drop;
                best = Some(j);
            }
        }
        samples[i].flow_to = best;
    }
    samples
}

// ---------- 3. drainage accumulation ----------

/// Push unit weight from each land sample into its downstream target,
/// processed in elevation-descending order so contributions cascade.
/// `drainage[i]` ends up as total area passing through i (including i's
/// own unit contribution) — conventional contributing-area definition.
fn accumulate_drainage(samples: &mut [Sample]) {
    let mut order: Vec<usize> = (0..samples.len()).collect();
    order.sort_by(|&a, &b| {
        samples[b]
            .elev
            .partial_cmp(&samples[a].elev)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let unit = 1.0 / samples.len() as f64;
    for &i in &order {
        samples[i].drainage += unit;
        let d = samples[i].drainage;
        if let Some(target) = samples[i].flow_to {
            samples[target].drainage += d;
        }
    }
}

// ---------- 4. threshold ----------

/// Pick a drainage threshold that keeps a healthy population of rivers
/// per land mass — empirically 3-5× more than the previous very-conservative
/// cutoff. Heuristic: 0.30 × the 90th-percentile drainage among terminal
/// land samples (river mouths / basin outlets). The river-cap in
/// `trace_rivers` keeps very rivery worlds from looking cluttered.
fn pick_threshold(samples: &[Sample], sea_level: f64) -> Option<f64> {
    // Drainages of land cells whose flow target is either None or ocean —
    // i.e., river mouths and basin outlets.
    let mut mouth_drains: Vec<f64> = samples
        .iter()
        .filter(|s| s.elev > sea_level)
        .filter(|s| match s.flow_to {
            Some(t) => samples[t].elev <= sea_level,
            None => true,
        })
        .map(|s| s.drainage)
        .collect();
    if mouth_drains.is_empty() {
        return None;
    }
    mouth_drains.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p_idx = ((mouth_drains.len() as f64) * 0.90) as usize;
    let p_idx = p_idx.min(mouth_drains.len() - 1);
    let p90 = mouth_drains[p_idx];
    if p90 <= 0.0 {
        return None;
    }
    Some(0.30 * p90)
}

// ---------- 5. trace + project ----------

/// Walk every river head (thresholded land sample whose upstream graph
/// has no thresholded contributor) downstream to ocean / basin, emitting
/// one `RiverPath` per logical river.
fn trace_rivers(
    samples: &[Sample],
    faces: &[Face],
    sea_level: f64,
    threshold: f64,
    seed: u64,
    elev_field: &super::noise::ElevationField,
) -> Vec<RiverPath> {
    // Compute upstream presence: a sample has any-thresholded-upstream if
    // there exists a sample whose flow_to chain reaches it AND has drainage
    // > threshold. Easier: mark inverse — for each thresholded sample,
    // increment a counter on its flow_to target if itself is thresholded.
    let mut upstream_thresholded = vec![false; samples.len()];
    for s in samples.iter() {
        if s.elev <= sea_level || s.drainage < threshold {
            continue;
        }
        if let Some(t) = s.flow_to {
            // t has at least one above-threshold contributor → not a head.
            upstream_thresholded[t] = true;
        }
    }

    // Coherent low-frequency noise drives the meander displacement. Seeding
    // from `seed` keeps re-renders identical for the same world. Folding
    // into u32 is fine — Fbm seeds the underlying Simplex.
    let warp_seed = ((seed ^ (seed >> 32)) as u32).wrapping_add(0x5EED_5EED);
    let warp = Fbm::<Simplex>::new(warp_seed)
        .set_octaves(4)
        .set_frequency(1.0)
        .set_lacunarity(2.05)
        .set_persistence(0.55);

    let mut rivers = Vec::new();
    for (i, s) in samples.iter().enumerate() {
        if s.elev <= sea_level || s.drainage < threshold {
            continue;
        }
        if upstream_thresholded[i] {
            continue; // not a river head
        }
        // Walk downstream collecting (face_idx, sphere_pos, drainage) tuples.
        // Stop at the last LAND sample. If the next step would land in ocean,
        // append a coastline point found by binary-searching the elevation
        // field along the (last-land, ocean-mouth) segment, so the river
        // visibly reaches the shore instead of ending one sample short.
        let mut path: Vec<(usize, [f64; 3], f64)> = Vec::new();
        let mut cur = i;
        let mut last_land_drain;
        loop {
            let s = &samples[cur];
            path.push((s.face_idx, s.sphere_pos, s.drainage));
            last_land_drain = s.drainage;
            match s.flow_to {
                None => {
                    // The downhill walk hit a sample with no in-cutoff
                    // downhill neighbor. If that sample is still above
                    // sea level it visually reads as a river dying in
                    // the middle of land. Most of these aren't true
                    // basins — they're just samples where the steepest-
                    // descent search didn't find an ocean within the
                    // narrow angular cutoff used by `compute_flow`.
                    // Search globally for the nearest ocean sample and,
                    // if one is reasonably close, extend the polyline to
                    // a binary-searched coast point on that bearing.
                    if s.elev > sea_level
                        && let Some(ocean_idx) = nearest_ocean(samples, cur, sea_level)
                    {
                        let coast = find_coast(
                            elev_field,
                            sea_level,
                            s.sphere_pos,
                            samples[ocean_idx].sphere_pos,
                        );
                        path.push((s.face_idx, coast, s.drainage));
                    }
                    break;
                }
                Some(t) => {
                    if samples[t].elev <= sea_level {
                        let coast = find_coast(
                            elev_field,
                            sea_level,
                            s.sphere_pos,
                            samples[t].sphere_pos,
                        );
                        path.push((s.face_idx, coast, s.drainage));
                        break;
                    }
                    cur = t;
                }
            }
        }
        // Require a substantive river: at least MIN_PATH_LEN samples on
        // the path. Filters out blip-paths from elevation noise.
        const MIN_PATH_LEN: usize = 4;
        if path.len() < MIN_PATH_LEN {
            continue;
        }
        let strokes = project_path_to_strokes(&path, faces);
        if strokes.is_empty() {
            continue;
        }
        // Apply fractal midpoint-displacement to give the river an organic
        // meander instead of straight chunks between ~14-unit samples.
        let meandered: Vec<Vec<(f64, f64)>> = strokes
            .into_iter()
            .map(|s| meander_polyline(&s, &warp, elev_field, sea_level))
            .filter(|s| s.len() >= 2)
            .collect();
        if meandered.is_empty() {
            continue;
        }
        rivers.push(RiverPath {
            strokes: meandered,
            mouth_drainage: last_land_drain,
        });
    }
    // Cap at MAX_RIVERS, keeping the largest by drainage so very rivery
    // worlds stay legible. Sort descending then truncate.
    const MAX_RIVERS: usize = 30;
    rivers.sort_by(|a, b| {
        b.mouth_drainage
            .partial_cmp(&a.mouth_drainage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rivers.truncate(MAX_RIVERS);
    rivers
}

// ---------- 6. fractal meander ----------

/// Maximum perpendicular displacement as a fraction of segment length.
/// 0.30 gives expressive bends without producing self-intersections at the
/// scales this polyline is rendered.
const MEANDER_AMPLITUDE: f64 = 0.30;
/// Stop subdividing when sub-segments are this short. ~2.5 sheet units
/// ≈ 1 pixel at PNG-2x scale, so finer detail isn't visible anyway.
const MIN_SEG_LEN: f64 = 2.5;
/// Hard cap on subdivision depth so a single ~14-unit segment expands to
/// at most 2^6 = 64 sub-segments.
const MAX_DEPTH: u32 = 6;

/// Apply midpoint displacement recursively to a polyline. Uses a coherent
/// 3D simplex-FBm sampled at (mx, my, depth) so the same input produces
/// the same meander each render. Displacements that would push the river
/// off-land are clamped against the elevation field so coastlines hold.
fn meander_polyline(
    poly: &[(f64, f64)],
    warp: &Fbm<Simplex>,
    elev_field: &super::noise::ElevationField,
    sea_level: f64,
) -> Vec<(f64, f64)> {
    if poly.len() < 2 {
        return poly.to_vec();
    }
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(poly.len() * 8);
    out.push(poly[0]);
    for win in poly.windows(2) {
        subdivide(win[0], win[1], warp, 0, &mut out, elev_field, sea_level);
        out.push(win[1]);
    }
    out
}

/// Recursive midpoint-displacement worker. Pushes intermediate points into
/// `out` between (but not including) `a` and `b`.
fn subdivide(
    a: (f64, f64),
    b: (f64, f64),
    warp: &Fbm<Simplex>,
    depth: u32,
    out: &mut Vec<(f64, f64)>,
    elev_field: &super::noise::ElevationField,
    sea_level: f64,
) {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len = (dx * dx + dy * dy).sqrt();
    if depth >= MAX_DEPTH || len < MIN_SEG_LEN {
        return;
    }
    let mx = (a.0 + b.0) * 0.5;
    let my = (a.1 + b.1) * 0.5;
    // Perpendicular unit vector to (b - a). Sign comes from the noise.
    let perp = (-dy / len, dx / len);
    // Coherent displacement: low-freq sample of x,y plus a depth-derived
    // z offset so successive levels of subdivision aren't correlated.
    let n = warp.get([
        mx * 0.05,
        my * 0.05,
        (depth as f64) * 0.73 + 0.17,
    ]);
    let amp = len * MEANDER_AMPLITUDE * n;
    let mut m = (mx + perp.0 * amp, my + perp.1 * amp);
    // Guard against the meander pushing the river across a coastline. The
    // elevation field is queried via the same equirectangular projection
    // the rasterizer uses, so "land" here matches what the user sees as
    // green pixels. Back off to half displacement, then to the unperturbed
    // midpoint, if the proposed point would land in ocean.
    if !is_land(elev_field, sea_level, m.0, m.1) {
        m = (mx + perp.0 * amp * 0.5, my + perp.1 * amp * 0.5);
        if !is_land(elev_field, sea_level, m.0, m.1) {
            m = (mx, my);
        }
    }
    subdivide(a, m, warp, depth + 1, out, elev_field, sea_level);
    out.push(m);
    subdivide(m, b, warp, depth + 1, out, elev_field, sea_level);
}

fn is_land(
    elev_field: &super::noise::ElevationField,
    sea_level: f64,
    x: f64,
    y: f64,
) -> bool {
    let sphere = super::grid::xy_to_sphere(x, y);
    elev_field.sample(&sphere) > sea_level
}

/// Locate the coastline (elev ≈ sea_level) on the segment between a known-
/// land 3D point and a known-ocean 3D point via binary-search slerp. Returns
/// the last point still on land, so the river polyline ends right at the
/// shore rather than ~one sub-grid cell short of it.
fn find_coast(
    elev_field: &super::noise::ElevationField,
    sea_level: f64,
    land: [f64; 3],
    ocean: [f64; 3],
) -> [f64; 3] {
    const ITERS: u32 = 8; // 2^-8 of segment length ≈ sub-pixel precision
    let mut a = land;
    let mut b = ocean;
    for _ in 0..ITERS {
        let m = sphere_midpoint(a, b);
        if elev_field.sample(&m) > sea_level {
            a = m;
        } else {
            b = m;
        }
    }
    a
}

/// Find the index of the nearest ocean sample (elev <= sea_level) to
/// `from`, capped at one icosahedron-edge of arc. Returns `None` if no
/// ocean sample is within the cap — in that case the river is treated
/// as a genuine inland basin (e.g., Caspian-style endorheic drainage on
/// a small isolated island chain). Used to extend rivers that hit a
/// `flow_to = None` dead-end above sea level so the polyline reaches
/// the visible coastline instead of fading mid-land.
///
/// Implementation uses `1 - dot(from, sample)` as a monotonic proxy for
/// angular distance (cheaper than acos and order-preserving). Cap value
/// `1 - cos(edge_arc)` corresponds to one full icosahedron-edge of arc
/// (~63°), which comfortably covers any failure of the narrow-cutoff
/// downhill search (only 1.6 sub-grid spacings ≈ 0.09 of an edge)
/// without bridging across whole ocean basins on tiny island worlds.
fn nearest_ocean(samples: &[Sample], from: usize, sea_level: f64) -> Option<usize> {
    let here = samples[from].sphere_pos;
    let mut best: Option<(usize, f64)> = None;
    for (j, s) in samples.iter().enumerate() {
        if j == from || s.elev > sea_level {
            continue;
        }
        let dot = here[0] * s.sphere_pos[0]
            + here[1] * s.sphere_pos[1]
            + here[2] * s.sphere_pos[2];
        let dist = 1.0 - dot;
        if best.is_none_or(|(_, d)| dist < d) {
            best = Some((j, dist));
        }
    }
    // edge_arc ≈ 1.107 rad; 1 - cos(edge_arc) ≈ 0.553. Half of that
    // (~0.30 in dot-distance, ~45° of arc) is the cap: any basin further
    // than this from any ocean is a genuine inland drain.
    // edge_arc ≈ 1.107 rad ≈ 63.4°. Cap at one icosahedron-edge of arc.
    // The binary-searched coast point lands at the FIRST sea-level
    // crossing along the bearing, so even a generous cap doesn't
    // bridge entire ocean basins on rivery worlds.
    let edge_arc = 1.107_148_717_794_090_5_f64;
    let cap = 1.0 - edge_arc.cos();
    best.filter(|(_, d)| *d < cap).map(|(idx, _)| idx)
}

fn sphere_midpoint(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let m = [
        (a[0] + b[0]) * 0.5,
        (a[1] + b[1]) * 0.5,
        (a[2] + b[2]) * 0.5,
    ];
    let len = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
    [m[0] / len, m[1] / len, m[2] / len]
}

/// Project each sphere position to (x, y) via the global equirectangular
/// mapping, then split the polyline at seam crossings. A jump of more than
/// half the sheet width between consecutive points means the river crossed
/// the longitude wrap — emit the stroke up to the seam (with an
/// interpolated y), then start a fresh stroke on the other side.
fn project_path_to_strokes(
    path: &[(usize, [f64; 3], f64)],
    _faces: &[Face],
) -> Vec<Vec<(f64, f64)>> {
    if path.is_empty() {
        return Vec::new();
    }
    let mut strokes = Vec::new();
    let mut current: Vec<(f64, f64)> = Vec::new();
    let mut prev: Option<(f64, f64)> = None;
    for (_, sphere, _) in path {
        let (x, y) = sphere_to_xy(sphere);
        if let Some((px, py)) = prev {
            let dx = x - px;
            if dx.abs() > SHEET_WIDTH / 2.0 {
                let (cross_end, cross_start, x_eff) = if dx > 0.0 {
                    // Path wraps west: previous on left side, current
                    // unwrapped to its negative-x equivalent.
                    (0.0, SHEET_WIDTH, x - SHEET_WIDTH)
                } else {
                    // Path wraps east: previous on right side, current
                    // unwrapped past SHEET_WIDTH.
                    (SHEET_WIDTH, 0.0, x + SHEET_WIDTH)
                };
                let t = (cross_end - px) / (x_eff - px);
                let y_seam = py + t * (y - py);
                current.push((cross_end, y_seam));
                if current.len() >= 2 {
                    strokes.push(std::mem::take(&mut current));
                }
                current.push((cross_start, y_seam));
            }
        }
        current.push((x, y));
        prev = Some((x, y));
    }
    if current.len() >= 2 {
        strokes.push(current);
    }
    strokes
}

#[cfg(test)]
mod tests {
    use crate::worldmap::{biome::Biome, generate};

    #[test]
    fn garden_world_has_rivers() {
        let map = generate("A788899-A", 0xDEADBEEF).unwrap();
        let n = map.rivers.len();
        assert!(
            n >= 1,
            "expected at least one river on a Hyd-8 garden world, got {n}"
        );
        // Each river should have at least one polyline with ≥ 2 points.
        for r in &map.rivers {
            assert!(!r.strokes.is_empty());
            for s in &r.strokes {
                assert!(s.len() >= 2);
            }
            assert!(r.mouth_drainage > 0.0);
        }
    }

    #[test]
    fn desert_world_has_no_rivers() {
        // Hyd 0/1 worlds bail out of `compute_rivers` early; whatever dark
        // wavy lines a user might see on a Mars-like map come from
        // hillshade or tectonic shading, never from this module.
        for seed in 1..=10u64 {
            let map = generate("A780799-A", seed).unwrap();
            assert_eq!(map.uwp.hydrographics(), 0);
            assert!(
                map.rivers.is_empty(),
                "Hyd 0 desert at seed {seed} produced {} rivers",
                map.rivers.len(),
            );
        }
    }

    #[test]
    fn waterworld_has_no_rivers() {
        let map = generate("A78A899-A", 1).unwrap();
        // Sanity: it's actually a water world.
        let land = map
            .grid
            .hexes
            .iter()
            .filter(|h| {
                !matches!(
                    h.biome,
                    Biome::DeepOcean | Biome::ShallowOcean
                )
            })
            .count();
        assert!(
            land < map.grid.hexes.len() / 10,
            "waterworld should be ≥90% ocean, got {land} land hexes"
        );
        assert!(
            map.rivers.is_empty(),
            "expected no rivers on a waterworld, got {}",
            map.rivers.len()
        );
    }

    #[test]
    fn rivers_stay_within_sheet_bounds() {
        let map = generate("A788899-A", 0xCAFE).unwrap();
        let w = super::super::grid::SHEET_WIDTH;
        let h = super::super::grid::SHEET_HEIGHT;
        // Allow modest slack for seam-wrapped copies (which can be at -ve x).
        for r in &map.rivers {
            for s in &r.strokes {
                for (x, y) in s {
                    assert!(*x > -w && *x < 2.0 * w, "river x out of range: {x}");
                    assert!(*y > -0.1 * h && *y < 1.1 * h, "river y out of range: {y}");
                }
            }
        }
    }

    /// Diagnostic: classifies each river's terminus as "reaches sea" or
    /// "ends inland" by sampling the elevation field at the polyline's
    /// final point. Useful for regressing the basin-extension fix in
    /// `trace_rivers` (see the `None` arm). Ignored by default; run with
    /// `cargo test --lib worldmap::rivers::tests::river_termini_reach_sea
    ///  -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn river_termini_reach_sea() {
        use crate::worldmap::grid::xy_to_sphere;
        let cases: &[(&str, &str, u64)] = &[
            ("garden", "A788899-A", 1),
            ("earth-s1", "C886977-8", 1),
            ("earth-s2", "C886977-8", 2),
            ("earth-s7", "C886977-8", 7),
            ("desert", "A780899-A", 1),
            ("ice", "A300077-A", 1),
        ];
        for &(name, uwp, seed) in cases {
            let map = generate(uwp, seed).unwrap();
            let mut reach = 0;
            let mut inland = 0;
            for r in &map.rivers {
                // The last stroke is the one ending at the river's
                // mouth — `project_path_to_strokes` pushes strokes in
                // path order, so the binary-searched coast point (or
                // the basin terminus) sits at the tail of the final
                // stroke.
                let stroke = r.strokes.last().unwrap();
                let (x, y) = *stroke.last().unwrap();
                let sphere = xy_to_sphere(x, y);
                let elev = map.elev_field.sample(&sphere);
                // Binary-searched coast points sit ON the last-land
                // sample (elev just above sea_level). Treat anything
                // very close to sea level as reaching the coast.
                let near_sea = (elev - map.sea_level).abs() < 0.01;
                if elev <= map.sea_level || near_sea {
                    reach += 1;
                } else {
                    // Step further along the stroke's tail direction —
                    // meander displacement can push the recorded end
                    // a few sheet units off the binary-searched coast
                    // point. Try several step sizes.
                    let mut found = false;
                    if stroke.len() >= 2 {
                        let (px, py) = stroke[stroke.len() - 2];
                        let dx = x - px;
                        let dy = y - py;
                        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                        for step in [1.0, 3.0, 6.0, 12.0, 24.0] {
                            let nx = x + dx / len * step;
                            let ny = y + dy / len * step;
                            let sphere2 = xy_to_sphere(nx, ny);
                            if map.elev_field.sample(&sphere2) <= map.sea_level {
                                found = true;
                                break;
                            }
                        }
                    }
                    if found {
                        reach += 1;
                    } else {
                        inland += 1;
                    }
                }
            }
            eprintln!(
                "{name} ({uwp}): {} rivers, reach-sea={reach}, end-inland={inland}",
                map.rivers.len()
            );
        }
    }
}
