//! Climate fields: temperature (latitude-driven) and humidity (noise +
//! atmosphere bias). Outputs are roughly normalized to [0, 1].

use ::noise::{Fbm, MultiFractal, NoiseFn, Simplex};

use super::Uwp;

/// Temperature variation amplitude added on top of the latitude curve.
/// Used as a multiplier on the spatial wobble field so threshold bands
/// (ice/sea-ice/temperate/hot) don't render as perfectly horizontal
/// stripes in the equirectangular projection.
pub const TEMP_AMPLITUDE: f64 = 0.06;

/// Low-frequency spatial noise added to the latitude-driven temperature.
/// Without it, every temperature threshold (ice/tundra/etc.) renders as a
/// perfectly horizontal line because temp = f(lat) is independent of lon.
/// With it, the latitude bands wobble like real Earth's climate zones.
pub struct TempField {
    fbm: Fbm<Simplex>,
}

impl TempField {
    pub fn from_uwp(_uwp: &Uwp, seed: u64) -> Self {
        let seed_u32 = (seed ^ (seed >> 32)) as u32;
        let fbm = Fbm::<Simplex>::new(seed_u32 ^ 0xA17E_5EED)
            .set_octaves(2)
            .set_frequency(0.7)
            .set_lacunarity(2.0)
            .set_persistence(0.5);
        Self { fbm }
    }

    /// Spatial wobble in roughly [-1, 1]. Caller multiplies by an amplitude.
    pub fn wobble(&self, sphere_pos: &[f64; 3]) -> f64 {
        self.fbm.get([sphere_pos[0] * 1.1, sphere_pos[1] * 1.1, sphere_pos[2] * 1.1])
    }
}

/// Latitude-driven temperature, calibrated to Earth's climate zones.
/// Output is roughly [0, 1] where 0 = polar/frozen and 1 = equatorial/hot.
///
/// Calibration targets (matched to colormap.rs biome thresholds):
///   - lat 0°  (equator) → temp ≈ 0.70 (hot zone, jungle if humid)
///   - lat 30° (subtropics) → temp = 0.60 (hot/temperate threshold)
///   - lat 60° (mid-latitude) → temp = 0.33 (temperate/cold threshold)
///   - lat 75° (sub-polar) → temp = 0.15 (ice/tundra threshold)
///   - lat 90° (pole) → temp ≈ 0.0
/// Maps `(cos(lat) - 0.05)` linearly through 0.735 so that lat 30° lands
/// exactly on 0.60 — the previous shape clipped at 1.0 across a 32°-wide
/// band, putting most of the polar caps' wide bottoms into hot biomes
/// even though they're really temperate latitudes.
pub fn temperature_at(sphere_pos: &[f64; 3], _amplitude: f64) -> f64 {
    let lat = sphere_pos[2].clamp(-1.0, 1.0).asin();
    let base = lat.cos();
    ((base - 0.05) * 0.735).clamp(0.0, 1.0)
}

/// Latitude curve plus a spatial wobble so threshold bands don't render
/// as perfectly horizontal lines. The wobble is small (≤ TEMP_AMPLITUDE)
/// so latitudinal climate zones remain recognizable; it just makes the
/// boundaries irregular like real Earth.
pub fn temperature_at_wobbled(sphere_pos: &[f64; 3], temp_field: &TempField) -> f64 {
    let base = temperature_at(sphere_pos, 0.0);
    let wobble = temp_field.wobble(sphere_pos) * TEMP_AMPLITUDE;
    (base + wobble).clamp(0.0, 1.0)
}

/// Per-unit-elevation temperature drop (lapse rate). Tuned so mid-elevation
/// terrain (post-amplification) crosses biome thresholds — too low and
/// tropical peaks stay hot, too high and low-hydrographics worlds (whose
/// `elev_above_sea` distribution is wider) snow-bomb everywhere.
pub const LAPSE_RATE: f64 = 0.45;

/// Apply elevation-driven cooling to a latitude+UWP temperature. Ocean
/// (negative `elev_above_sea`) passes through unchanged. Pair with
/// `amplify_elevation` to push the non-linear elevation tail into this.
///
/// Atmospheric heat retention puts a floor on lapse-driven cooling: a
/// pixel that started warm cannot be cooled BELOW the floor by altitude
/// alone (dense atmo on a hyd-0 world doesn't freeze its mid-latitude
/// peaks just because they're high — the air keeps them moderate). The
/// floor only applies when the floor would actually clamp; polar regions
/// (cold by latitude, no lapse involved) remain icy.
pub fn apply_lapse(temp: f64, elev_above_sea: f64, uwp: &Uwp) -> f64 {
    let cooled = (temp - LAPSE_RATE * elev_above_sea.max(0.0)).clamp(0.0, 1.0);
    let floor = atmo_temp_floor(uwp);
    if temp >= floor && cooled < floor {
        floor
    } else {
        cooled
    }
}

/// Minimum temperature a pixel can reach via lapse cooling alone, set by
/// the world's atmosphere. Thin/none atmos (Mars-like) have no floor —
/// peaks freely cool to ice. Earth-like and dense atmos retain enough
/// heat that a high mountain on a dense-atmo desert world stays in the
/// tundra/cool band rather than freezing solid.
fn atmo_temp_floor(uwp: &Uwp) -> f64 {
    match uwp.atmosphere() {
        0 | 1 => 0.00,    // trace / vacuum — no floor
        2 | 3 => 0.05,    // very thin
        4 | 5 => 0.08,    // thin
        6 | 7 => 0.10,    // standard / Earth-like
        8 | 9 => 0.13,    // dense
        _ => 0.08,        // exotic / corrosive — partial retention
    }
}

/// Per-unit-elevation humidity drop. Above the cloud layer it gets dry —
/// combined with rain-shadow this gives proper alpine deserts.
pub const ALTITUDE_DRYING: f64 = 0.35;

pub fn apply_altitude_drying(humidity: f64, elev_above_sea: f64) -> f64 {
    (humidity - ALTITUDE_DRYING * elev_above_sea.max(0.0)).clamp(0.0, 1.0)
}

/// Bimodal hypsometric stretch — most land sits flat (continental shelves,
/// plains) with a sharp rise to mountains, mirroring Earth's hypsometric
/// curve. Four bands: pass-through coast, compressed plains, ramped
/// foothills, then exploded mountains with soft cap. Lets tropical
/// lowlands stay hot (negligible lapse cooling) while mountains still
/// blow past snowline.
///
/// `hyd` divides the raw input by a hydrographics-dependent scale: on a
/// hyd-0 world `sea_level` sits at the 2.5%ile of the noise distribution
/// so median land ends up ~1.5 above-sea, which would otherwise saturate
/// the rocky-highland threshold and turn the whole planet into mountains.
/// Compressing low-hyd input by 5× makes a desert world's median land
/// read as plains, with only its true peaks reaching highland status.
pub fn amplify_elevation(elev_above_sea: f64, hyd: u8) -> f64 {
    if elev_above_sea <= 0.0 {
        return elev_above_sea;
    }
    let scale = relief_scale_for_hyd(hyd);
    let n = elev_above_sea / scale;
    // 0..0.08 — beach / immediate coast — pass through.
    if n < 0.08 {
        return n;
    }
    // 0.08..0.22 — plains — compress so most low land stays visually flat.
    if n < 0.22 {
        return 0.08 + (n - 0.08) * 0.30;
    }
    // 0.22..0.45 — foothills/hills — moderate ramp; transition to mountains
    // is the visible inflection.
    if n < 0.45 {
        return 0.122 + (n - 0.22) * 1.6; // ends at 0.490
    }
    // 0.45+ — mountains — sharp expansion with soft saturation around 0.65.
    let stretched = 0.490 + (n - 0.45) * 2.2;
    if stretched < 0.65 {
        stretched
    } else {
        0.65 + (stretched - 0.65) * 0.05
    }
}

/// Compress the above-sea range based on hydrographics so worlds with
/// extremely low sea_level don't have most of their surface saturated
/// past the rocky-highland threshold.
fn relief_scale_for_hyd(hyd: u8) -> f64 {
    match hyd {
        0 => 5.0,        // Mars-like — most land is "above sea" but flat
        1 => 3.0,
        2 | 3 => 2.0,
        4 | 5 => 1.4,
        6 | 7 => 1.1,
        _ => 1.0,        // Earth-like / water-world — full amplification
    }
}

/// UWP-driven temperature offset. Two contributions:
///   * Atmosphere — thin/no atmos can't retain heat; dense atmos run warmer.
///   * Hydrographics × atmo — low-hyd worlds with thick atmos run as
///     greenhouse environments (Venus-like / Sahara-like) because there
///     are no oceans to absorb solar input. High-hyd worlds with thick
///     atmos cool slightly via ocean heat sinks.
/// Hyd modifier only applies for atmo ≥ 6 — you need actual atmosphere to
/// retain heat. A Mars-like (atmo 1, hyd 0) gets no greenhouse boost.
fn temperature_bias(uwp: &Uwp) -> f64 {
    let atmo_bias = match uwp.atmosphere() {
        0 => -0.45,
        1 => -0.30,
        2 | 3 => -0.15,
        4 | 5 => -0.05,
        6 | 7 => 0.0,
        8 | 9 => 0.02,
        _ => 0.05,
    };
    let hyd_bias = if uwp.atmosphere() >= 6 {
        match uwp.hydrographics() {
            0 => 0.20,        // dry greenhouse — Venus / hot Sahara world
            1 => 0.12,
            2 | 3 => 0.05,
            4 | 5 => 0.0,
            6 | 7 => -0.02,
            _ => -0.05,       // water world heat sink
        }
    } else {
        0.0
    };
    atmo_bias + hyd_bias
}

pub struct HumidityField {
    fbm: Fbm<Simplex>,
}

impl HumidityField {
    pub fn from_uwp(_uwp: &Uwp, seed: u64) -> Self {
        let seed_u32 = (seed ^ (seed >> 32)) as u32;
        let fbm = Fbm::<Simplex>::new(seed_u32)
            .set_octaves(3)
            .set_frequency(1.0)
            .set_lacunarity(2.0)
            .set_persistence(0.5);
        Self { fbm }
    }

    /// Humidity at a sphere position, biased by atmosphere and hydrographics.
    /// Atmosphere 0 → desiccated (no free water); 8+ → wet/jungle bias.
    /// Hydrographics 0 → desert (large negative pull); 10 → water world (positive).
    pub fn sample(&self, sphere_pos: &[f64; 3], uwp: &Uwp) -> f64 {
        let raw = self
            .fbm
            .get([sphere_pos[0] * 1.7, sphere_pos[1] * 1.7, sphere_pos[2] * 1.7]);
        // raw is roughly [-1, 1]; remap to [0, 1].
        let h = (raw + 1.0) * 0.5;
        // Bias arms intentionally leave room for the FBM noise (~[-1,1]/2) to
        // dominate, so a garden world lands across a wider humidity spread
        // (steppe → grassland → forest → jungle) instead of pinning above the
        // jungle cutoff. Atmo 0..=2 still skew strongly dry; 8+ only nudge wet.
        let atmo_bias = match uwp.atmosphere() {
            0 => -0.50,
            1 => -0.25,
            2 | 3 => -0.10,
            4 | 5 => -0.02,
            6 | 7 => 0.0,
            8 | 9 => 0.05,
            _ => 0.10,
        };
        // Hydrographics is the strongest signal for surface moisture, but we
        // still ease off the wet end so noise variation comes through.
        let hydro_bias = match uwp.hydrographics() {
            0 => -0.55,
            1 => -0.30,
            2 | 3 => -0.15,
            4 | 5 => -0.05,
            6 | 7 => 0.05,
            8 | 9 => 0.10,
            _ => 0.20,
        };
        (h + atmo_bias + hydro_bias).clamp(0.0, 1.0)
    }
}

pub fn compute_climate(grid: &mut super::grid::Grid, uwp: &Uwp, humidity: &HumidityField) {
    for hex in &mut grid.hexes {
        hex.temperature = adjust_temperature(temperature_at(&hex.sphere_pos, TEMP_AMPLITUDE), uwp);
        hex.humidity = humidity.sample(&hex.sphere_pos, uwp);
    }
}

/// Apply the UWP-driven temperature bias to a raw latitude-derived temp.
/// Useful for callers (e.g. the rasterizer) that sample per-pixel without
/// going through `compute_climate`. Currently unused by raster.rs (out of
/// scope here); wire it up when the rasterizer learns about the UWP.
pub fn adjust_temperature(raw: f64, uwp: &Uwp) -> f64 {
    (raw + temperature_bias(uwp)).clamp(0.0, 1.0)
}
