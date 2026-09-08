//! 3D simplex fBm sampled on the unit sphere, used as the elevation field.
//!
//! When a `TectonicField` is attached, its per-point offset is added on top
//! of the noise sample, so every downstream consumer (biome assignment, the
//! per-pixel raster, sub-hex sampling) automatically sees tectonic-aware
//! elevation without code changes.

use ::noise::{Fbm, MultiFractal, NoiseFn, Simplex};

use super::Uwp;
use super::tectonics::TectonicField;

/// Amplitude of the high-frequency relief term, before the world's `scale`
/// factor. Small on purpose: this is meant to crenellate coastlines and
/// roughen mountainsides, not to move continents.
///
/// It lands unevenly by design, because `climate::amplify_elevation` is
/// piecewise: the beach band (n < 0.08) passes through 1:1, so a coastline —
/// where the terrain gradient is shallowest and a small vertical nudge moves
/// the shore a long way horizontally — gets the full effect; the plains band
/// compresses by 0.30, so flat country stays flat; and the mountain band
/// expands by 2.2, so ridges get the roughest. That is the right distribution
/// for free.
const RELIEF_AMP: f64 = 0.06;

pub struct ElevationField {
    fbm: Fbm<Simplex>,
    /// High-frequency detail added on top of `fbm`.
    ///
    /// A *separate* field rather than more octaves on `fbm`, for a
    /// non-obvious reason: `Fbm` normalizes its output by
    /// `1 / Σ persistence^k` over its octaves, and `set_octaves` recomputes
    /// that factor. Going 4 → 8 octaves at persistence 0.55 would rescale the
    /// whole field by ~0.92 — *including octave 1*. Sea level is a percentile
    /// so it partly follows, but the absolute thresholds downstream (rocky at
    /// 0.32, snow at 0.5) don't, so every existing world's coastlines and snow
    /// lines would shift. An additive term with its own amplitude leaves the
    /// large-scale shape exactly where it is.
    ///
    /// Frequency picks up where the base field runs out: `fbm` spans
    /// 1.4 → ~12 (4 octaves at lacunarity 2.05), this spans 9 → ~78, so the
    /// two overlap slightly and together cover about eight octaves. The top
    /// frequency is deliberately under the Nyquist limit of a 1024-wide
    /// equirectangular texture (~163 at the equator) so the detail resolves
    /// rather than aliasing.
    relief: Fbm<Simplex>,
    /// Atmosphere code shifts the elevation distribution: atmospheres ≤1 are
    /// near-vacuum so we flatten terrain (no liquid → coast lines mean less);
    /// dense atmospheres erode it slightly. We bake the bias into the field.
    bias: f64,
    scale: f64,
    /// Optional plate-tectonics field. When present its offset is added to
    /// each `sample()` result.
    tectonics: Option<TectonicField>,
}

impl ElevationField {
    pub fn from_uwp(uwp: &Uwp, seed: u64) -> Self {
        // Seed: derive a u32 from the u64 (noise crate uses u32 seeds).
        let seed_u32 = (seed ^ (seed >> 32)) as u32;
        let fbm = Fbm::<Simplex>::new(seed_u32)
            .set_octaves(4)
            .set_frequency(1.4)
            .set_lacunarity(2.05)
            .set_persistence(0.55);

        // Vacuum / trace atmosphere worlds get less relief variation
        // (cratered planetoid feel rather than rugged terrain).
        let atmo = uwp.atmosphere();
        let scale = if atmo <= 1 { 0.85 } else { 1.0 };
        // Size 0 (asteroid) — scale way down; large worlds slightly more rugged.
        let size = uwp.size();
        let scale = scale * (0.6 + (size as f64 / 15.0) * 0.6).min(1.2);

        // Offset seed so the detail doesn't correlate with the base field —
        // sharing it would make every ridge land on a continental crest.
        let relief = Fbm::<Simplex>::new(seed_u32.wrapping_add(0x7F4A_7C15))
            .set_octaves(4)
            .set_frequency(9.0)
            .set_lacunarity(2.05)
            .set_persistence(0.5);

        Self {
            fbm,
            relief,
            bias: 0.0,
            scale,
            tectonics: None,
        }
    }

    /// Attach a plate-tectonics field whose offset gets added to every
    /// `sample()` call. Builder-style for ergonomics inside `generate()`.
    pub fn with_tectonics(mut self, t: TectonicField) -> Self {
        self.tectonics = Some(t);
        self
    }

    /// Borrow the attached tectonic field, if any. Read-only access for the
    /// rivers / colormap passes that want plate or rain-shadow data.
    pub fn tectonics(&self) -> Option<&TectonicField> {
        self.tectonics.as_ref()
    }

    /// Sample the elevation field at a unit-sphere position. Returns a value
    /// roughly in [-1, 1] but not strictly bounded. If a tectonic field is
    /// attached, its offset is added in.
    ///
    /// The high-frequency [`relief`](Self::relief) term rides on the same
    /// `scale` as the base field, so an airless planetoid or a small world
    /// gets a proportionally smoother surface rather than a smooth shape
    /// wearing rough skin.
    pub fn sample(&self, sphere_pos: &[f64; 3]) -> f64 {
        match &self.tectonics {
            Some(t) => self.sample_prewarped(sphere_pos, &t.warp(sphere_pos)),
            None => self.sample_prewarped(sphere_pos, sphere_pos),
        }
    }

    /// The tectonic domain warp of `p`, or `p` unchanged when no tectonic
    /// field is attached. Pair with [`Self::sample_prewarped`] when a caller
    /// needs several plate-derived quantities at one point — the warp is the
    /// most expensive step in the path and there is no reason to pay for it
    /// more than once per sample site.
    pub fn warp(&self, sphere_pos: &[f64; 3]) -> [f64; 3] {
        match &self.tectonics {
            Some(t) => t.warp(sphere_pos),
            None => *sphere_pos,
        }
    }

    /// [`Self::sample`] given a `warped` point already obtained from
    /// [`Self::warp`]. The noise bands read the *unwarped* position (they add
    /// independent detail rather than echoing the warp); only the tectonic
    /// term uses the warped one.
    pub fn sample_prewarped(&self, sphere_pos: &[f64; 3], warped: &[f64; 3]) -> f64 {
        let raw = self.fbm.get([sphere_pos[0], sphere_pos[1], sphere_pos[2]]);
        let detail = self.relief.get(*sphere_pos) * RELIEF_AMP;
        let base = (raw + detail) * self.scale + self.bias;
        match &self.tectonics {
            Some(t) => base + t.elevation_offset_warped(warped),
            None => base,
        }
    }
}

pub fn compute_elevation(grid: &mut super::grid::Grid, field: &ElevationField) {
    for hex in &mut grid.hexes {
        hex.elevation = field.sample(&hex.sphere_pos);
    }
}

/// Fine-scale albedo detail, sampled on the unit sphere. Globe-only: the
/// flat map paints legend swatches and wants no per-pixel grain, but a
/// planet seen from orbit has no two square kilometres the same colour.
///
/// Two independent bands, because they do different jobs:
///
/// * `mottle` — continental-scale (a handful of features per hemisphere).
///   Perturbs the humidity/temperature a texel is classified at, so a
///   grassland region breaks up into patches of forest and steppe instead
///   of painting as one flat blob.
/// * `grain` — fine, near-texel scale. Modulates brightness only, which
///   reads as terrain texture rather than as a different biome.
///
/// Both are in roughly [-1, 1]. Deterministic from `(uwp, seed)` like every
/// other field, and derived from the same seed so no plumbing changes.
pub struct DetailField {
    mottle: Fbm<Simplex>,
    grain: Fbm<Simplex>,
}

impl DetailField {
    pub fn from_uwp(_uwp: &Uwp, seed: u64) -> Self {
        // Offset the seed so the detail bands don't correlate with the
        // elevation field (which would make grain track the coastlines).
        let base = seed ^ 0x5DEE_CE66_D9B4_1BA7;
        let mottle_seed = (base ^ (base >> 32)) as u32;
        let grain_seed = mottle_seed.wrapping_add(0x9E37_79B9);

        let mottle = Fbm::<Simplex>::new(mottle_seed)
            .set_octaves(3)
            .set_frequency(3.5)
            .set_lacunarity(2.1)
            .set_persistence(0.5);
        let grain = Fbm::<Simplex>::new(grain_seed)
            .set_octaves(4)
            .set_frequency(14.0)
            .set_lacunarity(2.2)
            .set_persistence(0.5);

        Self { mottle, grain }
    }

    /// Sample both bands at a unit-sphere position: `(mottle, grain)`.
    #[inline]
    pub fn sample(&self, p: &[f64; 3]) -> (f64, f64) {
        (self.mottle.get(*p), self.grain.get(*p))
    }
}
