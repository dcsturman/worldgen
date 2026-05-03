//! Plate-tectonics field: assigns each point on the unit sphere to one of
//! ~6–10 plates and produces an elevation offset and per-point rain-shadow
//! factor based on plate type and proximity to plate boundaries.
//!
//! Output is layered onto the simplex-fBm elevation in `noise.rs` so that
//! downstream consumers (biome, rivers, raster colormap) get tectonic-aware
//! elevation automatically without any code change.
//!
//! Domain warping: every plate-related lookup first perturbs `sphere_pos`
//! through a multi-octave 3D simplex fBm. This bends the otherwise-arc
//! Voronoi boundaries into wiggly, fractal-looking ones — so coastlines,
//! channels, and mountain chains lose their "Voronoi diagram" rigidity.

use ::noise::{Fbm, MultiFractal, NoiseFn, Simplex};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::Uwp;

/// Plate kind. Continental plates bias their interior toward land; oceanic
/// plates toward sea floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlateKind {
    #[default]
    Oceanic,
    Continental,
}

/// Boundary classification between two adjacent plates, derived from the
/// relative motion of their velocities at the boundary point. Drives
/// mountain ranges (convergent on land), trenches (subduction), and rifts
/// (divergent).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BoundaryKind {
    #[default]
    None,
    Convergent,
    Divergent,
    Transform,
    Subduction,
}

/// One tectonic plate seeded on the unit sphere.
#[derive(Clone, Debug, Default)]
pub struct Plate {
    pub id: u8,
    pub kind: PlateKind,
    /// Seed point (a unit-sphere vector); used for nearest-plate Voronoi.
    pub seed: [f64; 3],
    /// Angular velocity vector (rad / arbitrary-time, direction = rotation
    /// axis). Used to compute boundary motion at any point.
    pub velocity: [f64; 3],
}

/// Tectonic field. Sampled by the elevation pipeline to produce continental
/// vs oceanic biasing and orogenic uplift near convergent boundaries.
pub struct TectonicField {
    pub plates: Vec<Plate>,
    /// Domain-warp noise: shared by `plate_id_at`, `elevation_offset`, and
    /// `rain_shadow_at` so all three see the same warped point and stay
    /// self-consistent. Higher frequency than the elevation field — adds
    /// coast-scale detail without dissolving plate identity.
    warp: Fbm<Simplex>,
    /// Secondary high-frequency wobble added directly to `elevation_offset`'s
    /// output. Breaks up smoothstep level-sets so coastlines look fractal,
    /// not just "warped circles".
    wobble: Fbm<Simplex>,
}

/// Domain-warp amplitude (sphere units). Boundaries visibly bend on the
/// scale of a few hexes; continents stay recognizable.
const WARP_AMPLITUDE: f64 = 0.10;
/// Amplitude of the high-freq wobble added on top of the smoothstepped
/// interior bias. Small — just enough to fractal up coastlines without
/// drowning the plate-driven structure.
const WOBBLE_AMPLITUDE: f64 = 0.06;

impl TectonicField {
    pub fn from_uwp(uwp: &Uwp, seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let n: usize = 6 + (uwp.size() as usize % 5);

        // Fibonacci-lattice seeding — near-uniform distribution on the sphere
        // from a deterministic closed form, no rejection loop.
        let golden = std::f64::consts::PI * (1.0 + 5_f64.sqrt());
        let nf = n as f64;

        // Continental fraction biased by hydrographics (drier → more land plates).
        let hyd = uwp.hydrographics().min(15) as f64;
        let n_cont_raw = (nf * (1.0 - hyd / 15.0) * 0.6 + nf * 0.2).round() as isize;
        let n_cont = n_cont_raw.clamp(1, n as isize - 1) as usize;

        // Pick which plate ids are continental — random subset of size n_cont.
        let mut ids: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = rng.random_range(0..=i);
            ids.swap(i, j);
        }
        let mut is_cont = vec![false; n];
        for &k in ids.iter().take(n_cont) {
            is_cont[k] = true;
        }

        let mut plates = Vec::with_capacity(n);
        for (k, &cont) in is_cont.iter().enumerate() {
            let theta = (1.0 - 2.0 * (k as f64 + 0.5) / nf).acos();
            let phi = golden * k as f64;
            let (st, ct) = theta.sin_cos();
            let (sp, cp) = phi.sin_cos();
            let seed_pos = [st * cp, st * sp, ct];

            // Random angular velocity, direction uniform on sphere, mag ~0.5.
            let vx: f64 = rng.random_range(-1.0..1.0);
            let vy: f64 = rng.random_range(-1.0..1.0);
            let vz: f64 = rng.random_range(-1.0..1.0);
            let m = (vx * vx + vy * vy + vz * vz).sqrt().max(1e-9);
            let s = 0.5 / m;
            let velocity = [vx * s, vy * s, vz * s];

            plates.push(Plate {
                id: k as u8,
                kind: if cont {
                    PlateKind::Continental
                } else {
                    PlateKind::Oceanic
                },
                seed: seed_pos,
                velocity,
            });
        }

        // Derive distinct u32 seeds from the same RNG so warp/wobble are
        // deterministic per UWP+seed but uncorrelated with each other.
        let warp_seed: u32 = rng.random();
        let wobble_seed: u32 = rng.random();
        let warp = Fbm::<Simplex>::new(warp_seed)
            .set_octaves(4)
            .set_frequency(2.6)
            .set_lacunarity(2.05)
            .set_persistence(0.55);
        let wobble = Fbm::<Simplex>::new(wobble_seed)
            .set_octaves(3)
            .set_frequency(6.0)
            .set_lacunarity(2.1)
            .set_persistence(0.5);

        Self {
            plates,
            warp,
            wobble,
        }
    }

    /// Apply 3D domain warp to a unit-sphere point. Each output component
    /// is sampled at a slightly offset point so the three are uncorrelated
    /// (the standard simplex-warp trick), then renormalized back to the
    /// sphere.
    fn warp_sphere(&self, p: &[f64; 3]) -> [f64; 3] {
        let nx = self.warp.get([p[0], p[1], p[2]]);
        let ny = self.warp.get([p[0] + 17.3, p[1] - 4.1, p[2] + 9.7]);
        let nz = self.warp.get([p[0] - 31.5, p[1] + 22.6, p[2] - 13.2]);
        let q = [
            p[0] + WARP_AMPLITUDE * nx,
            p[1] + WARP_AMPLITUDE * ny,
            p[2] + WARP_AMPLITUDE * nz,
        ];
        let m = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt().max(1e-9);
        [q[0] / m, q[1] / m, q[2] / m]
    }

    /// Return (nearest_idx, second_idx, dot_nearest, dot_second). Plates sorted
    /// by descending seed·sphere_pos (greater dot = smaller angular distance).
    fn two_nearest(&self, p: &[f64; 3]) -> Option<(usize, usize, f64, f64)> {
        if self.plates.len() < 2 {
            return None;
        }
        let mut best = (-2.0_f64, usize::MAX);
        let mut second = (-2.0_f64, usize::MAX);
        for (i, plate) in self.plates.iter().enumerate() {
            let d = plate.seed[0] * p[0] + plate.seed[1] * p[1] + plate.seed[2] * p[2];
            if d > best.0 {
                second = best;
                best = (d, i);
            } else if d > second.0 {
                second = (d, i);
            }
        }
        Some((best.1, second.1, best.0, second.0))
    }

    pub fn elevation_offset(&self, sphere_pos: &[f64; 3]) -> f64 {
        // Warp first — every plate-related quantity below operates on the
        // warped point so they all stay consistent.
        let warped = self.warp_sphere(sphere_pos);
        self.elevation_offset_warped(&warped)
    }

    /// Body of `elevation_offset` operating on an already-warped point.
    /// Used by `rain_shadow_at` so we don't double-warp.
    fn elevation_offset_warped(&self, sphere_pos: &[f64; 3]) -> f64 {
        let Some((a, b, da, db)) = self.two_nearest(sphere_pos) else {
            return 0.0;
        };
        let plate_a = &self.plates[a];
        let plate_b = &self.plates[b];

        // Angular gap between the two nearest seed-distances. Small gap = near
        // the boundary; the value is in radians (small-angle, since da, db are
        // close to each other and to 1).
        let gap = (da - db).max(0.0);

        // Interior bias, smoothly fades from 0 at the boundary up to full at
        // gap >= boundary_fade. smoothstep keeps coastlines from being cliffs.
        let boundary_fade = 0.04;
        let t = (gap / boundary_fade).clamp(0.0, 1.0);
        let smooth = t * t * (3.0 - 2.0 * t);
        let interior_bias = match plate_a.kind {
            PlateKind::Continental => 0.35,
            PlateKind::Oceanic => -0.35,
        } * smooth;

        // Boundary contribution: examine relative motion at the boundary
        // midpoint. We approximate the midpoint as the sample point itself —
        // close enough near the boundary, which is where this term is nonzero.
        let mid = sphere_pos;
        let v_a = cross(&plate_a.velocity, mid);
        let v_b = cross(&plate_b.velocity, mid);
        let rel = sub(&v_a, &v_b);

        // Boundary normal direction (in the tangent plane, pointing from a to b):
        // project (seed_b - seed_a) onto the tangent plane at mid.
        let from_a_to_b = sub(&plate_b.seed, &plate_a.seed);
        let normal = normalize(&project_to_tangent(&from_a_to_b, mid));

        let conv = -dot(&rel, &normal); // positive = closing, negative = opening

        // Distance from boundary in sphere units: small-angle approximation,
        // gap ≈ 2 * sin(angle/2) ≈ angle, but the seed-dot difference is in
        // cos-units so just use it directly with an empirical scale.
        let scale = 0.05;
        let decay = (-gap / scale).exp();

        let boundary_term = if conv > 0.05 {
            // Convergent
            let peak = match (plate_a.kind, plate_b.kind) {
                (PlateKind::Continental, PlateKind::Continental) => 0.6,
                // Subduction: continental side gets uplift, oceanic side trench.
                (PlateKind::Continental, PlateKind::Oceanic) => 0.55,
                (PlateKind::Oceanic, PlateKind::Continental) => -0.45,
                (PlateKind::Oceanic, PlateKind::Oceanic) => 0.3, // island arc
            };
            peak * decay * conv.min(1.0)
        } else if conv < -0.05 {
            // Divergent — small rift on land, mid-ocean ridge in ocean.
            let peak = match plate_a.kind {
                PlateKind::Continental => -0.15,
                PlateKind::Oceanic => 0.15,
            };
            peak * decay * (-conv).min(1.0)
        } else {
            0.0
        };

        // High-freq elevation wobble — sampled on the *unwarped* point so it
        // adds independent fractal detail rather than echoing the warp. Tiny
        // amplitude; just breaks up smoothstep level-sets along coastlines.
        let w = self
            .wobble
            .get([sphere_pos[0], sphere_pos[1], sphere_pos[2]]);
        let wobble = WOBBLE_AMPLITUDE * w;

        (interior_bias + boundary_term + wobble).clamp(-0.7, 0.7)
    }

    pub fn plate_id_at(&self, sphere_pos: &[f64; 3]) -> u8 {
        if self.plates.is_empty() {
            return u8::MAX;
        }
        let warped = self.warp_sphere(sphere_pos);
        let mut best = (-2.0_f64, u8::MAX);
        for plate in &self.plates {
            let d = plate.seed[0] * warped[0]
                + plate.seed[1] * warped[1]
                + plate.seed[2] * warped[2];
            if d > best.0 {
                best = (d, plate.id);
            }
        }
        best.1
    }

    pub fn rain_shadow_at(&self, sphere_pos: &[f64; 3]) -> f64 {
        // Warp keeps rain-shadow stripes aligned with the (now wiggly) ridge.
        let warped = self.warp_sphere(sphere_pos);
        let sphere_pos = &warped;

        let Some((a, b, da, db)) = self.two_nearest(sphere_pos) else {
            return 0.0;
        };

        // Only meaningful within range of a real ridge.
        let gap = (da - db).max(0.0);
        if gap > 0.12 {
            return 0.0;
        }

        let plate_a = &self.plates[a];
        let plate_b = &self.plates[b];

        // Convergence test reused from elevation_offset.
        let v_a = cross(&plate_a.velocity, sphere_pos);
        let v_b = cross(&plate_b.velocity, sphere_pos);
        let rel = sub(&v_a, &v_b);
        let from_a_to_b = sub(&plate_b.seed, &plate_a.seed);
        let normal = normalize(&project_to_tangent(&from_a_to_b, sphere_pos));
        let conv = -dot(&rel, &normal);
        if conv < 0.05 {
            return 0.0;
        }
        // Ridge needs to actually rise — cheap sanity check. Use the
        // warped-point variant; sphere_pos here is already warped.
        let elev_here = self.elevation_offset_warped(sphere_pos);
        if elev_here < 0.3 {
            return 0.0;
        }

        // Latitude band → prevailing wind direction in geographic east-axis units.
        // sphere_pos = (x, y, z) with z = sin(lat).
        let lat = sphere_pos[2].clamp(-1.0, 1.0).asin();
        let lat_abs = lat.abs();
        let east_sign = if lat_abs < std::f64::consts::FRAC_PI_6 {
            -1.0 // tropical easterlies (wind blows toward west)
        } else if lat_abs < std::f64::consts::FRAC_PI_3 {
            1.0 // mid-latitude westerlies
        } else {
            -1.0 // polar easterlies
        };
        // East unit vector in sphere coords at this point: derivative of position
        // w.r.t. longitude, normalized. = (-sin φ, cos φ, 0).
        let r_xy = (sphere_pos[0] * sphere_pos[0] + sphere_pos[1] * sphere_pos[1]).sqrt();
        let east = if r_xy < 1e-6 {
            [1.0, 0.0, 0.0]
        } else {
            [-sphere_pos[1] / r_xy, sphere_pos[0] / r_xy, 0.0]
        };
        let wind = [east[0] * east_sign, east[1] * east_sign, east[2] * east_sign];

        // Boundary normal points from plate a (this side) toward plate b. If wind
        // has a component toward b, this point is upwind (wind hits ridge after
        // us → we're on the wet windward side). Otherwise downwind = dry.
        let along = dot(&wind, &normal);
        let mag = 0.3 + 0.3 * conv.min(1.0);
        if along > 0.0 {
            mag // upwind / wet
        } else {
            -mag // downwind / dry
        }
    }
}

fn cross(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn sub(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn normalize(v: &[f64; 3]) -> [f64; 3] {
    let m = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if m < 1e-9 {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / m, v[1] / m, v[2] / m]
    }
}
fn project_to_tangent(v: &[f64; 3], n: &[f64; 3]) -> [f64; 3] {
    let d = dot(v, n);
    [v[0] - d * n[0], v[1] - d * n[1], v[2] - d * n[2]]
}
