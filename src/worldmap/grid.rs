//! Icosahedral hex grid laid out as a 5 + 10 + 5 strip.
//!
//! The 12 icosahedron vertices live on a unit sphere (north pole, 5-vertex
//! upper pentagon at lat ≈ +26.57°, 5-vertex lower pentagon at lat ≈ −26.57°,
//! south pole). The 20 faces are each filled with a triangular hex array of
//! `HEXES_PER_EDGE` cells per edge, giving N(N+1)/2 hexes per face. Each hex
//! carries its barycentric coords inside its face, its 3D unit-sphere
//! position (via barycentric → linear-combine of face vertices, then
//! normalize), and its 2D unfolded-sheet center + polygon. The cyclic seam
//! in the equator zigzag (`ED_4`) is handled by storing two unfolded
//! positions for that face's hexes — left edge and right edge.

use std::f64::consts::{FRAC_PI_2, PI};

use super::biome::{Biome, SubSample};
use super::features::Feature;

pub const HEXES_PER_EDGE: usize = 7;
pub const TRIANGLE_SIDE: f64 = 200.0;

const SQRT_3_OVER_2: f64 = 0.866_025_403_784_438_6;
pub const TRIANGLE_HEIGHT: f64 = TRIANGLE_SIDE * SQRT_3_OVER_2;
pub const SHEET_WIDTH: f64 = 5.0 * TRIANGLE_SIDE;
pub const SHEET_HEIGHT: f64 = 3.0 * TRIANGLE_HEIGHT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceCategory {
    NorthPolar,
    EquatorUp,
    EquatorDown,
    SouthPolar,
}

#[derive(Clone, Debug)]
pub struct Face {
    pub idx: usize,
    pub category: FaceCategory,
    /// 3 vertices of the face on the unit sphere, in the order matching
    /// `unfolded_positions[*]`.
    pub vertices_3d: [[f64; 3]; 3],
    /// Each entry is one rendering of this face in the unfolded sheet:
    /// 3 (x, y) corners in the same order as `vertices_3d`. Most faces
    /// have one entry; the seam face has two.
    pub unfolded_positions: Vec<[(f64, f64); 3]>,
}

#[derive(Clone, Debug)]
pub struct Hex {
    pub face_idx: usize,
    /// Barycentric coords (a, b, c) within the parent face,
    /// matching the same vertex order as `Face::vertices_3d`.
    pub barycentric: [f64; 3],
    pub sphere_pos: [f64; 3],
    /// Unfolded 2D centers — one entry per `Face::unfolded_positions` entry.
    pub centers_2d: Vec<(f64, f64)>,
    /// Hex polygon vertices for each unfolded position.
    pub polygons_2d: Vec<[(f64, f64); 6]>,

    pub elevation: f64,
    pub temperature: f64,
    pub humidity: f64,
    pub biome: Biome,
    pub sub_samples: Vec<SubSample>,
    pub features: Vec<Feature>,
}

pub struct Grid {
    pub faces: Vec<Face>,
    pub hexes: Vec<Hex>,
    pub width: f64,
    pub height: f64,
}

impl Grid {
    pub fn build() -> Self {
        let verts_3d = icosahedron_vertices();
        let faces = build_faces(&verts_3d);
        let hexes = generate_hexes(&faces);
        Self {
            faces,
            hexes,
            width: SHEET_WIDTH,
            height: SHEET_HEIGHT,
        }
    }
}

// ---------- icosahedron geometry ----------

/// 12 vertices on the unit sphere:
/// 0  = north pole
/// 1..=5 = upper pentagon at latitude +arctan(1/2), longitudes 72k°
/// 6..=10 = lower pentagon at latitude −arctan(1/2), longitudes 36 + 72k°
/// 11 = south pole
fn icosahedron_vertices() -> [[f64; 3]; 12] {
    let lat_n = (0.5_f64).atan();
    let lat_s = -lat_n;
    let cos_n = lat_n.cos();
    let sin_n = lat_n.sin();
    let cos_s = lat_s.cos();
    let sin_s = lat_s.sin();

    let mut v = [[0.0; 3]; 12];
    v[0] = [0.0, 0.0, 1.0];
    for k in 0..5 {
        let lon = 2.0 * PI * (k as f64) / 5.0;
        v[1 + k] = [cos_n * lon.cos(), cos_n * lon.sin(), sin_n];
    }
    for k in 0..5 {
        let lon = 2.0 * PI * ((k as f64) + 0.5) / 5.0;
        v[6 + k] = [cos_s * lon.cos(), cos_s * lon.sin(), sin_s];
    }
    v[11] = [0.0, 0.0, -1.0];
    v
}

fn build_faces(verts: &[[f64; 3]; 12]) -> Vec<Face> {
    let mut faces = Vec::with_capacity(20);
    let s = TRIANGLE_SIDE;
    let h = TRIANGLE_HEIGHT;

    // Indices: N=0, U_k = 1+k, L_k = 6+k, S=11.
    let u = |k: usize| 1 + (k % 5);
    let l = |k: usize| 6 + (k % 5);

    // North polar (5): vertices (U_k, U_{k+1}, N).
    // Apex N at the top of the sheet, wide base at y=h so it sits flush
    // against the wide top edge of equator-down k. Mirrors the south polar
    // layout (wide base shared with equator-up k at y=2h).
    // ND_4 spans x = 4.5s..5.5s, so it's rendered twice — once at the right
    // edge and once wrapped to the left edge — to cover the seam.
    for k in 0..5 {
        let canon = [
            ((k as f64 + 0.5) * s, h),
            ((k as f64 + 1.5) * s, h),
            ((k as f64 + 1.0) * s, 0.0),
        ];
        let mut positions = vec![canon];
        if k == 4 {
            positions.push([
                ((k as f64 + 0.5) * s - SHEET_WIDTH, h),
                ((k as f64 + 1.5) * s - SHEET_WIDTH, h),
                ((k as f64 + 1.0) * s - SHEET_WIDTH, 0.0),
            ]);
        }
        faces.push(Face {
            idx: faces.len(),
            category: FaceCategory::NorthPolar,
            vertices_3d: [verts[u(k)], verts[u(k + 1)], verts[0]],
            unfolded_positions: positions,
        });
    }

    // Equator up (5): vertices (U_k, L_k, U_{k+1}).
    // Unfolded: U_k at (k*s, 2h) [bot-left], L_k at ((k+1)*s, 2h) [bot-right],
    //           U_{k+1} at ((k+0.5)*s, h) [apex].
    for k in 0..5 {
        faces.push(Face {
            idx: faces.len(),
            category: FaceCategory::EquatorUp,
            vertices_3d: [verts[u(k)], verts[l(k)], verts[u(k + 1)]],
            unfolded_positions: vec![[
                (k as f64 * s, 2.0 * h),
                ((k + 1) as f64 * s, 2.0 * h),
                ((k as f64 + 0.5) * s, h),
            ]],
        });
    }

    // Equator down (5): vertices (U_{k+1}, L_k, L_{k+1}).
    // Unfolded: U_{k+1} at ((k+0.5)*s, h) [top-left], L_{k+1} at ((k+1.5)*s, h) [top-right],
    //           L_k at ((k+1)*s, 2h) [bot-apex].
    // ED_4 spans x = 4.5s..5.5s, so we render it twice — once at right edge
    // and once wrapped to the left edge — to cover both halves of the seam.
    for k in 0..5 {
        let canon = [
            ((k as f64 + 0.5) * s, h),
            ((k as f64 + 1.5) * s, h),
            ((k as f64 + 1.0) * s, 2.0 * h),
        ];
        let mut positions = vec![canon];
        if k == 4 {
            positions.push([
                ((k as f64 + 0.5) * s - SHEET_WIDTH, h),
                ((k as f64 + 1.5) * s - SHEET_WIDTH, h),
                ((k as f64 + 1.0) * s - SHEET_WIDTH, 2.0 * h),
            ]);
        }
        faces.push(Face {
            idx: faces.len(),
            category: FaceCategory::EquatorDown,
            vertices_3d: [verts[u(k + 1)], verts[l(k + 1)], verts[l(k)]],
            unfolded_positions: positions,
        });
    }

    // South polar (5): vertices (L_{k+1}, L_k, S).
    // Unfolded: L_{k+1} at (k*s, 2h), L_k at ((k+1)*s, 2h), S at ((k+0.5)*s, 3h).
    // Lower pentagon offset means the strip starts at L_1 on the left.
    for k in 0..5 {
        faces.push(Face {
            idx: faces.len(),
            category: FaceCategory::SouthPolar,
            vertices_3d: [verts[l(k + 1)], verts[l(k)], verts[11]],
            unfolded_positions: vec![[
                (k as f64 * s, 2.0 * h),
                ((k + 1) as f64 * s, 2.0 * h),
                ((k as f64 + 0.5) * s, 3.0 * h),
            ]],
        });
    }

    debug_assert_eq!(faces.len(), 20);
    faces
}

// ---------- hex placement inside each face ----------

fn generate_hexes(faces: &[Face]) -> Vec<Hex> {
    let n = HEXES_PER_EDGE;
    let mut hexes = Vec::with_capacity(faces.len() * n * (n + 1) / 2);

    for face in faces {
        for (i, j, bary) in iter_face_hex_barycentric(n) {
            let _ = (i, j); // i,j only used internally for placement
            let mut centers = Vec::with_capacity(face.unfolded_positions.len());
            let mut polys = Vec::with_capacity(face.unfolded_positions.len());
            for tri2d in &face.unfolded_positions {
                let center = barycentric_to_2d(&bary, tri2d);
                centers.push(center);
                polys.push(hex_polygon(center));
            }
            // Derive 3D position from the canonical 2D center via the same
            // equirectangular mapping the rasterizer uses, so a hex's biome
            // (and any features placed on it) match the visible terrain at
            // its center pixel — instead of disagreeing across stylized
            // seams the way per-face barycentric did.
            let sphere = xy_to_sphere(centers[0].0, centers[0].1);
            hexes.push(Hex {
                face_idx: face.idx,
                barycentric: bary,
                sphere_pos: sphere,
                centers_2d: centers,
                polygons_2d: polys,
                elevation: 0.0,
                temperature: 0.0,
                humidity: 0.0,
                biome: Biome::Unassigned,
                sub_samples: Vec::new(),
                features: Vec::new(),
            });
        }
    }
    hexes
}

/// Yield (row, col, [a, b, c]) for each hex inside a triangle of side N.
///
/// Vertex ordering convention (matches `Face::vertices_3d` / `unfolded_positions`
/// for ALL face categories): the first two vertices form the "base" edge, the
/// third vertex is the "apex" (alone on its row). For up-pointing faces the
/// apex is at smaller screen-y; for down-pointing faces it's at larger
/// screen-y. Either way the hex layout is symmetric in barycentric space.
///
/// Row 0 sits at the apex (1 hex), row N−1 at the base (N hexes). Total
/// N(N+1)/2 hexes per face.
fn iter_face_hex_barycentric(n: usize) -> impl Iterator<Item = (usize, usize, [f64; 3])> {
    (0..n).flat_map(move |i| {
        (0..=i).map(move |j| {
            let nf = n as f64;
            // Apex weight (vertex index 2 — the "apex"):
            let c_apex = (nf - i as f64 - 0.5) / nf;
            // The remainder splits between vertex 0 and vertex 1 along the row.
            let split = (i as f64 + 0.5) / nf;
            let row_pos = (j as f64 + 0.5) / (i as f64 + 1.0);
            let a = (1.0 - row_pos) * split;
            let b = row_pos * split;
            (i, j, [a, b, c_apex])
        })
    })
}

fn barycentric_to_sphere(bary: &[f64; 3], verts: &[[f64; 3]; 3]) -> [f64; 3] {
    let mut p = [0.0_f64; 3];
    for k in 0..3 {
        for d in 0..3 {
            p[d] += bary[k] * verts[k][d];
        }
    }
    let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    [p[0] / len, p[1] / len, p[2] / len]
}

fn barycentric_to_2d(bary: &[f64; 3], tri: &[(f64, f64); 3]) -> (f64, f64) {
    let x = bary[0] * tri[0].0 + bary[1] * tri[1].0 + bary[2] * tri[2].0;
    let y = bary[0] * tri[0].1 + bary[1] * tri[1].1 + bary[2] * tri[2].1;
    (x, y)
}

/// Pointy-top hexagon centered on `(cx, cy)`. `flat_to_flat` is the
/// distance between the two parallel side edges (i.e. the hex's width;
/// height is `flat_to_flat * 2 / sqrt(3)`).
pub fn pointy_top_hex(center: (f64, f64), flat_to_flat: f64) -> [(f64, f64); 6] {
    let r = flat_to_flat / (3.0_f64).sqrt(); // apex distance for pointy-top
    let (cx, cy) = center;
    let dx = r * 3.0_f64.sqrt() / 2.0;
    let dy = r / 2.0;
    [
        (cx, cy - r),
        (cx + dx, cy - dy),
        (cx + dx, cy + dy),
        (cx, cy + r),
        (cx - dx, cy + dy),
        (cx - dx, cy - dy),
    ]
}

fn hex_polygon(center: (f64, f64)) -> [(f64, f64); 6] {
    // Per-face data hexes sized to the triangular subdivision. Note: these
    // do NOT tessellate cleanly across face boundaries — the renderer uses
    // a separate global honeycomb for the visible grid.
    pointy_top_hex(center, TRIANGLE_SIDE / HEXES_PER_EDGE as f64)
}

/// Compute a 3D sphere position from a face's barycentric coordinates and
/// face vertices. Public for use by sub-hex sampling.
pub fn bary_to_sphere(bary: &[f64; 3], face: &Face) -> [f64; 3] {
    barycentric_to_sphere(bary, &face.vertices_3d)
}

/// Latitude in radians, derived from a unit-sphere position.
pub fn latitude(sphere_pos: &[f64; 3]) -> f64 {
    sphere_pos[2].clamp(-1.0, 1.0).asin()
}

/// Global (x, y) on the unfolded sheet → unit-sphere position via
/// equirectangular projection. y maps linearly to latitude across the full
/// sheet height (y=0 → north pole, y=SHEET_HEIGHT → south pole), x maps
/// to longitude over [0, 2π). Independent of which icosahedron face the
/// point falls in, so adjacent points straddling a stylized seam map to
/// neighboring sphere positions and terrain stays continuous.
pub fn xy_to_sphere(x: f64, y: f64) -> [f64; 3] {
    let lat = FRAC_PI_2 - (y / SHEET_HEIGHT) * PI;
    let lon = (x / SHEET_WIDTH) * 2.0 * PI;
    let cos_lat = lat.cos();
    [cos_lat * lon.cos(), cos_lat * lon.sin(), lat.sin()]
}

/// Inverse of `xy_to_sphere`. Wraps longitude into [0, SHEET_WIDTH).
pub fn sphere_to_xy(sphere: &[f64; 3]) -> (f64, f64) {
    let lat = sphere[2].clamp(-1.0, 1.0).asin();
    let mut lon = sphere[1].atan2(sphere[0]);
    if lon < 0.0 {
        lon += 2.0 * PI;
    }
    let x = (lon / (2.0 * PI)) * SHEET_WIDTH;
    let y = (FRAC_PI_2 - lat) / PI * SHEET_HEIGHT;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icosahedron_has_unit_vertices() {
        let v = icosahedron_vertices();
        for p in &v {
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-12, "vertex not unit length: {p:?}");
        }
    }

    #[test]
    fn icosahedron_edges_are_uniform() {
        // All connected vertex pairs should have the same chord length.
        // Use the canonical icosahedron edge length: chord = 2 * sin(arccos(sqrt(5)/5) / 2).
        let v = icosahedron_vertices();
        let target = 2.0 * (((5.0_f64).sqrt() / 5.0).acos() / 2.0).sin();

        // For each vertex, check that exactly 5 others are at the target distance.
        for (i, a) in v.iter().enumerate() {
            let mut neighbors = 0;
            for (j, b) in v.iter().enumerate() {
                if i == j {
                    continue;
                }
                let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                if (d - target).abs() < 1e-6 {
                    neighbors += 1;
                }
            }
            assert_eq!(neighbors, 5, "vertex {i} has {neighbors} neighbors, expected 5");
        }
    }

    #[test]
    fn grid_has_expected_hex_count() {
        let g = Grid::build();
        let n = HEXES_PER_EDGE;
        assert_eq!(g.faces.len(), 20);
        assert_eq!(g.hexes.len(), 20 * n * (n + 1) / 2);
    }

    #[test]
    fn all_hexes_are_on_unit_sphere() {
        let g = Grid::build();
        for h in &g.hexes {
            let p = h.sphere_pos;
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-9, "hex sphere_pos not unit: {p:?}");
        }
    }

    #[test]
    fn seam_faces_have_two_renderings() {
        let g = Grid::build();
        let seam_count = g
            .faces
            .iter()
            .filter(|f| f.unfolded_positions.len() > 1)
            .count();
        // North polar k=4 and equator-down k=4 both wrap across the seam.
        assert_eq!(seam_count, 2, "expected exactly two seam faces");
    }

    #[test]
    fn barycentric_sums_to_one() {
        for (_, _, b) in iter_face_hex_barycentric(HEXES_PER_EDGE) {
            let sum = b[0] + b[1] + b[2];
            assert!((sum - 1.0).abs() < 1e-12, "barycentric does not sum to 1: {b:?}");
            assert!(b.iter().all(|x| *x >= -1e-12 && *x <= 1.0 + 1e-12));
        }
    }
}
