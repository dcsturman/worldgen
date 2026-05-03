//! Biome classification + sub-hex terrain mixing.
//!
//! For each hex we sample N sub-points in its parent face's barycentric
//! coordinates (jittered around the hex center), evaluate elevation /
//! temperature / humidity at each sub-point's sphere position, and run them
//! through a `(elevation_band, temp_band, humidity_band) → Biome` lookup.
//! The hex's `biome` field is the dominant biome among its sub-samples; the
//! full `sub_samples` list drives the stipple-marks render that gives the
//! "mixed terrain" look.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use super::Uwp;
use super::climate::{self, HumidityField};
use super::grid::{Grid, xy_to_sphere};
use super::noise::ElevationField;
use super::raster::{apply_continentality, continentality};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Biome {
    #[default]
    Unassigned,
    DeepOcean,
    ShallowOcean,
    IceCap,
    Tundra,
    Taiga,
    TemperateForest,
    Grassland,
    Steppe,
    Desert,
    SavannaScrub,
    Jungle,
    Highland,
    Mountain,
    Barren,
}

impl Biome {
    /// Base RGB fill color for this biome (sRGB, 0..255 per channel).
    pub fn color(self) -> (u8, u8, u8) {
        match self {
            Biome::Unassigned => (200, 0, 200),
            Biome::DeepOcean => (28, 60, 110),
            Biome::ShallowOcean => (52, 110, 158),
            Biome::IceCap => (235, 240, 245),
            Biome::Tundra => (170, 175, 165),
            Biome::Taiga => (78, 110, 82),
            Biome::TemperateForest => (66, 120, 70),
            Biome::Grassland => (140, 168, 92),
            Biome::Steppe => (180, 174, 120),
            Biome::Desert => (212, 188, 132),
            Biome::SavannaScrub => (188, 170, 100),
            Biome::Jungle => (52, 100, 56),
            Biome::Highland => (130, 110, 90),
            Biome::Mountain => (110, 95, 82),
            Biome::Barren => (140, 130, 118),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SubSample {
    /// 2D unfolded position (one entry per hex's centers_2d slot).
    pub pos_2d: (f64, f64),
    pub biome: Biome,
}

const SUB_SAMPLES_PER_HEX: usize = 12;

pub fn assign_biomes(
    grid: &mut Grid,
    uwp: &Uwp,
    elev: &ElevationField,
    humidity_field: &HumidityField,
    sea_level: f64,
    base_seed: u64,
) {
    let temp_amplitude = climate::TEMP_AMPLITUDE;

    let faces = grid.faces.clone();

    for hex in &mut grid.hexes {
        let face = &faces[hex.face_idx];
        let mut sub_rng = ChaCha8Rng::seed_from_u64(
            base_seed
                .wrapping_add((hex.face_idx as u64) << 32)
                .wrapping_add(
                    ((hex.barycentric[0] * 1e6) as u64) ^ ((hex.barycentric[1] * 1e9) as u64),
                ),
        );

        let mut counts: [u32; BIOME_COUNT] = [0; BIOME_COUNT];
        hex.sub_samples.clear();

        for _ in 0..SUB_SAMPLES_PER_HEX {
            let bary = jitter_barycentric(&hex.barycentric, &mut sub_rng);
            // Project barycentric into the canonical 2D position, then map
            // through the same equirectangular projection the rasterizer
            // uses. Keeps sub-sample biomes consistent with visible pixels.
            let canon = &face.unfolded_positions[0];
            let canon_2d = (
                bary[0] * canon[0].0 + bary[1] * canon[1].0 + bary[2] * canon[2].0,
                bary[0] * canon[0].1 + bary[1] * canon[1].1 + bary[2] * canon[2].1,
            );
            let sphere = xy_to_sphere(canon_2d.0, canon_2d.1);
            let elev_v = elev.sample(&sphere);
            let above = climate::amplify_elevation(elev_v - sea_level);
            let raw_t = climate::temperature_at(&sphere, temp_amplitude);
            let temp_v = climate::apply_lapse(climate::adjust_temperature(raw_t, uwp), above);
            let mut hum_v = humidity_field.sample(&sphere, uwp);
            if let Some(tec) = elev.tectonics() {
                hum_v = super::colormap::rain_shadow_adjustment(hum_v, tec.rain_shadow_at(&sphere));
            }
            hum_v = climate::apply_altitude_drying(hum_v, above);
            // Match the raster's continentality drying so per-hex biomes
            // line up with visible pixels in continental interiors.
            if above > 0.0 {
                let cont = continentality(elev, sea_level, canon_2d.0, canon_2d.1);
                hum_v = apply_continentality(hum_v, cont);
            }

            let biome = classify(elev_v, sea_level, temp_v, hum_v);
            counts[biome as usize] += 1;

            for tri2d in &face.unfolded_positions {
                let pos_2d = (
                    bary[0] * tri2d[0].0 + bary[1] * tri2d[1].0 + bary[2] * tri2d[2].0,
                    bary[0] * tri2d[0].1 + bary[1] * tri2d[1].1 + bary[2] * tri2d[2].1,
                );
                hex.sub_samples.push(SubSample { pos_2d, biome });
            }
        }

        // hex.sphere_pos is already the equirectangular projection of the
        // canonical 2D center (set in grid.rs::generate_hexes), so these
        // sample at the same point the raster will color.
        hex.elevation = elev.sample(&hex.sphere_pos);
        let above = climate::amplify_elevation(hex.elevation - sea_level);
        let raw_t = climate::temperature_at(&hex.sphere_pos, temp_amplitude);
        hex.temperature = climate::apply_lapse(climate::adjust_temperature(raw_t, uwp), above);
        let mut hum = humidity_field.sample(&hex.sphere_pos, uwp);
        if let Some(tec) = elev.tectonics() {
            hum = super::colormap::rain_shadow_adjustment(hum, tec.rain_shadow_at(&hex.sphere_pos));
        }
        hex.humidity = climate::apply_altitude_drying(hum, above);
        hex.biome = dominant_biome(&counts);
    }
}

fn jitter_barycentric(center: &[f64; 3], rng: &mut ChaCha8Rng) -> [f64; 3] {
    // Sample a uniform point inside a small triangle around the hex center.
    // Hex inscribed circle has radius ≈ 0.5 * (TRIANGLE_SIDE / N) in pixels;
    // in barycentric units inside the parent face, that's 0.5 / N.
    let radius = 0.5 / super::grid::HEXES_PER_EDGE as f64;
    // Pick a random offset in barycentric space.
    let mut offset = [0.0_f64; 3];
    let r1: f64 = rng.random();
    let r2: f64 = rng.random();
    let r3: f64 = rng.random();
    let total = r1 + r2 + r3;
    offset[0] = (r1 / total - 1.0 / 3.0) * radius * 2.0;
    offset[1] = (r2 / total - 1.0 / 3.0) * radius * 2.0;
    offset[2] = (r3 / total - 1.0 / 3.0) * radius * 2.0;
    let mut b = [
        (center[0] + offset[0]).max(0.0),
        (center[1] + offset[1]).max(0.0),
        (center[2] + offset[2]).max(0.0),
    ];
    let s = b[0] + b[1] + b[2];
    b[0] /= s;
    b[1] /= s;
    b[2] /= s;
    b
}

/// Compute the sea-level elevation threshold from the grid's per-hex
/// elevation field (assumed already populated in `hex.elevation`) and the
/// UWP's hydrographics digit. Hexes with elevation strictly below this
/// threshold are water.
pub fn compute_sea_level(grid: &Grid, uwp: &Uwp) -> f64 {
    let mut values: Vec<f64> = grid.hexes.iter().map(|h| h.elevation).collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // Map Hyd 0..=10 to fraction-water 0.025..=0.975 (midpoint of each band).
    let hydro = uwp.hydrographics().min(15);
    let frac_water = (hydro as f64).clamp(0.0, 10.0) / 10.0 * 0.95 + 0.025;
    let idx = ((values.len() as f64) * frac_water).round() as usize;
    let idx = idx.min(values.len().saturating_sub(1));
    values[idx]
}

const BIOME_COUNT: usize = 16; // covers every variant; trailing slots unused

fn dominant_biome(counts: &[u32; BIOME_COUNT]) -> Biome {
    let (idx, _) = counts
        .iter()
        .enumerate()
        .max_by_key(|(_, c)| **c)
        .unwrap_or((Biome::Unassigned as usize, &0));
    biome_from_index(idx)
}

fn biome_from_index(i: usize) -> Biome {
    match i {
        x if x == Biome::DeepOcean as usize => Biome::DeepOcean,
        x if x == Biome::ShallowOcean as usize => Biome::ShallowOcean,
        x if x == Biome::IceCap as usize => Biome::IceCap,
        x if x == Biome::Tundra as usize => Biome::Tundra,
        x if x == Biome::Taiga as usize => Biome::Taiga,
        x if x == Biome::TemperateForest as usize => Biome::TemperateForest,
        x if x == Biome::Grassland as usize => Biome::Grassland,
        x if x == Biome::Steppe as usize => Biome::Steppe,
        x if x == Biome::Desert as usize => Biome::Desert,
        x if x == Biome::SavannaScrub as usize => Biome::SavannaScrub,
        x if x == Biome::Jungle as usize => Biome::Jungle,
        x if x == Biome::Highland as usize => Biome::Highland,
        x if x == Biome::Mountain as usize => Biome::Mountain,
        x if x == Biome::Barren as usize => Biome::Barren,
        _ => Biome::Unassigned,
    }
}

/// Map (elevation, sea_level, temperature, humidity) → Biome. Temperature
/// and humidity are roughly normalized to [0, 1].
pub fn classify(elev: f64, sea_level: f64, temp: f64, humidity: f64) -> Biome {
    if elev < sea_level {
        let depth = sea_level - elev;
        // Frozen ocean at very cold latitudes regardless of depth.
        if temp < 0.15 {
            return Biome::IceCap;
        }
        return if depth < 0.05 {
            Biome::ShallowOcean
        } else {
            Biome::DeepOcean
        };
    }

    let above = elev - sea_level;
    if above > 0.40 {
        return Biome::Mountain;
    }
    if above > 0.20 {
        return Biome::Highland;
    }

    if temp < 0.18 {
        return Biome::IceCap;
    }
    if temp < 0.30 {
        return Biome::Tundra;
    }
    if temp < 0.45 {
        if humidity > 0.55 {
            Biome::Taiga
        } else {
            Biome::Tundra
        }
    } else if temp < 0.65 {
        if humidity > 0.70 {
            Biome::TemperateForest
        } else if humidity > 0.45 {
            Biome::Grassland
        } else if humidity > 0.20 {
            Biome::Steppe
        } else {
            Biome::Desert
        }
    } else {
        // Hot
        if humidity > 0.65 {
            Biome::Jungle
        } else if humidity > 0.40 {
            Biome::SavannaScrub
        } else {
            Biome::Desert
        }
    }
}
