//! 3D simplex fBm sampled on the unit sphere, used as the elevation field.
//!
//! When a `TectonicField` is attached, its per-point offset is added on top
//! of the noise sample, so every downstream consumer (biome assignment, the
//! per-pixel raster, sub-hex sampling) automatically sees tectonic-aware
//! elevation without code changes.

use ::noise::{Fbm, MultiFractal, NoiseFn, Simplex};

use super::Uwp;
use super::tectonics::TectonicField;

pub struct ElevationField {
    fbm: Fbm<Simplex>,
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

        Self {
            fbm,
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
    pub fn sample(&self, sphere_pos: &[f64; 3]) -> f64 {
        let raw = self.fbm.get([sphere_pos[0], sphere_pos[1], sphere_pos[2]]);
        let base = raw * self.scale + self.bias;
        match &self.tectonics {
            Some(t) => base + t.elevation_offset(sphere_pos),
            None => base,
        }
    }
}

pub fn compute_elevation(grid: &mut super::grid::Grid, field: &ElevationField) {
    for hex in &mut grid.hexes {
        hex.elevation = field.sample(&hex.sphere_pos);
    }
}
