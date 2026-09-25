//! Climate fields: temperature (latitude-driven) and humidity (noise +
//! atmosphere bias). Outputs are roughly normalized to [0, 1].
//!
//! A tidally locked world replaces latitude with `theta`, the great-circle
//! angle from the substellar point — see [`ClimateModel`]. Every consumer
//! asks the model rather than calling the latitude functions directly, so the
//! per-hex biomes, the flat raster and the globe texture all read one rule.

use ::noise::{Fbm, MultiFractal, NoiseFn, Simplex};

use super::Uwp;
use super::grid::Grid;
use crate::decorations::WorldDecorations;

/// Temperature variation amplitude added on top of the latitude curve.
/// Used as a multiplier on the spatial wobble field so threshold bands
/// (ice/sea-ice/temperate/hot) don't render as perfectly horizontal
/// stripes in the equirectangular projection.
pub const TEMP_AMPLITUDE: f64 = 0.06;

/// Amplitude of a *higher-frequency*, pole-weighted temperature roughness.
/// The low-frequency wobble above barely varies across a small polar cap, so
/// the ice-cap boundary stays a near-perfect latitude circle (an unrealistic
/// round "sticker"). This term adds finer fingers at high latitudes. It is
/// zero-mean, so it ragged the *edge* of the cap without changing its average
/// extent — the cap-size calibration in `temperature_at` is untouched.
pub const POLAR_ROUGH_AMPLITUDE: f64 = 0.05;
/// Frequency multiplier for the polar roughness (vs. the smooth wobble).
/// Higher = finer, more numerous fingers around the ice edge.
const POLAR_ROUGH_FREQ: f64 = 8.0;

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
        self.fbm.get([
            sphere_pos[0] * 1.1,
            sphere_pos[1] * 1.1,
            sphere_pos[2] * 1.1,
        ])
    }

    /// Higher-frequency sample of the same field, in roughly [-1, 1], used to
    /// ragged high-latitude (ice/sub-polar) boundaries. Offset so it
    /// decorrelates from [`Self::wobble`].
    pub fn polar_roughness(&self, sphere_pos: &[f64; 3]) -> f64 {
        self.fbm.get([
            sphere_pos[0] * POLAR_ROUGH_FREQ + 11.3,
            sphere_pos[1] * POLAR_ROUGH_FREQ - 7.1,
            sphere_pos[2] * POLAR_ROUGH_FREQ + 4.7,
        ])
    }
}

/// Hermite smoothstep: 0 below `e0`, 1 above `e1`, smooth in between.
#[inline]
fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
///
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
    // Pole-weighted high-frequency roughness so the ice cap and sub-polar bands
    // get a ragged, fractal edge instead of a perfect latitude circle. The
    // weight (|sin lat|) is ~0 in the tropics — leaving those biomes
    // untouched — and ramps to 1 by ~70° latitude. Zero-mean, so the cap's
    // average extent is preserved.
    let polar = smoothstep(0.55, 0.95, sphere_pos[2].abs());
    let rough = temp_field.polar_roughness(sphere_pos) * POLAR_ROUGH_AMPLITUDE * polar;
    (base + wobble + rough).clamp(0.0, 1.0)
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
        0 | 1 => 0.00, // trace / vacuum — no floor
        2 | 3 => 0.05, // very thin
        4 | 5 => 0.08, // thin
        6 | 7 => 0.10, // standard / Earth-like
        8 | 9 => 0.13, // dense
        _ => 0.08,     // exotic / corrosive — partial retention
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
        0 => 5.0, // Mars-like — most land is "above sea" but flat
        1 => 3.0,
        2 | 3 => 2.0,
        4 | 5 => 1.4,
        6 | 7 => 1.1,
        _ => 1.0, // Earth-like / water-world — full amplification
    }
}

/// UWP-driven temperature offset. Two contributions:
///   * Atmosphere — thin/no atmos can't retain heat; dense atmos run warmer.
///   * Hydrographics × atmo — low-hyd worlds with thick atmos run as
///     greenhouse environments (Venus-like / Sahara-like) because there
///     are no oceans to absorb solar input. High-hyd worlds with thick
///     atmos cool slightly via ocean heat sinks.
///
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
            0 => 0.20, // dry greenhouse — Venus / hot Sahara world
            1 => 0.12,
            2 | 3 => 0.05,
            4 | 5 => 0.0,
            6 | 7 => -0.02,
            _ => -0.05, // water world heat sink
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
        let raw = self.fbm.get([
            sphere_pos[0] * 1.7,
            sphere_pos[1] * 1.7,
            sphere_pos[2] * 1.7,
        ]);
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

pub fn compute_climate(
    grid: &mut super::grid::Grid,
    uwp: &Uwp,
    model: &ClimateModel,
    humidity: &HumidityField,
) {
    for hex in &mut grid.hexes {
        hex.temperature = model.base_temperature(&hex.sphere_pos, uwp);
        hex.humidity = model.humidity(&hex.sphere_pos, humidity, uwp);
    }
}

/// Apply the UWP-driven temperature bias to a raw latitude-derived temp.
/// Useful for callers (e.g. the rasterizer) that sample per-pixel without
/// going through `compute_climate`. Currently unused by raster.rs (out of
/// scope here); wire it up when the rasterizer learns about the UWP.
pub fn adjust_temperature(raw: f64, uwp: &Uwp) -> f64 {
    (raw + temperature_bias(uwp)).clamp(0.0, 1.0)
}

// ---- Tidally locked climate ------------------------------------------------

/// Which climate a map is painted with.
///
/// Every temperature, water, humidity, wind, cloud and settlement decision
/// goes through this, so the rule that decides a hex's biome is the same one
/// that colours its pixels on the flat map and the globe. The `Rotating` arm
/// of every method calls exactly the code that existed before this type did,
/// in the same order — a rotating world's cached render must stay
/// byte-identical to a fresh one (pinned by `worldmap::golden`).
#[derive(Clone, Debug, Default)]
pub enum ClimateModel {
    /// Latitude-banded climate: every world that isn't tidally locked.
    #[default]
    Rotating,
    /// One face permanently toward the star: climate is a function of the
    /// angle from the substellar point, not latitude.
    TideLocked(TideLockedClimate),
}

/// Parameters of a locked world's climate. Built without touching the map's
/// RNG (the substellar point comes from the seed by pure arithmetic), so a
/// lock re-climates the same terrain rather than generating a new world.
#[derive(Clone, Debug)]
pub struct TideLockedClimate {
    /// Unit vector to the substellar point.
    pub substellar: [f64; 3],
    /// Temperature at the substellar point, before wobble and clamping. On
    /// the same normalized scale as the rotating model (0 ≈ frozen, 1 ≈ the
    /// hottest desert), and may exceed 1 — the per-point clamp absorbs it.
    pub t_sub: f64,
    /// Temperature at the antistellar point. May be negative on an airless
    /// world, which just means the whole night side clamps to frozen.
    pub t_night: f64,
    /// Angle (radians) beyond which the antistellar ice sheet lies, before
    /// its edge is roughened. Infinite until [`Self::settle_water`] runs, and
    /// stays infinite on a world with no water to trap.
    pub sheet_theta: f64,
    /// Hydrographics digit the *liquid* water alone amounts to. Relief is
    /// scaled by it (see [`amplify_elevation`]) because that scale exists to
    /// stop a dry world's land all reading as mountains — and a locked
    /// world whose water is locked up as ice is, everywhere else, a dry world.
    pub relief_hyd: u8,
    /// Lowest water potential any point may have: on a world with no liquid
    /// water, just above sea level, so the deepest basins flatten into dry
    /// playas instead of pooling; `-∞` (no floor) once there is liquid.
    pub dry_floor: f64,
}

/// Area-weighted mean of [`temperature_at`] over the sphere:
/// `0.735 · (π/4 − 0.05)`. The rotating model's global temperature budget
/// before the UWP bias; the clamp at the poles changes it by under 0.001.
///
/// A locked world gets no stellar data here — only its UWP — so this plus
/// [`temperature_bias`] *is* its insolation: the same star and greenhouse
/// that would warm the rotating version of this world, with the heat piled
/// onto one hemisphere instead of spread by rotation. Conserving the mean is
/// what keeps a locked garden world's terminator temperate rather than the
/// whole map sliding hot or cold.
const ROTATING_MEAN_TEMP: f64 = 0.735 * (std::f64::consts::FRAC_PI_4 - 0.05);

/// Substellar-to-antistellar temperature spread by atmosphere code.
///
/// Thin air moves little heat, so the day side bakes and the night side
/// freezes; a thick atmosphere carries heat round to the night side and
/// narrows the gap. Anchors: an airless body's full span (the Moon's
/// ~390 K noon vs ~100 K night is more than this whole scale covers); a
/// standard atmosphere keeps its terminator temperate; the exotic and
/// corrosive codes are Venus-like, nearly isothermal.
fn locked_spread(atmo: u8) -> f64 {
    match atmo {
        0 => 1.60,
        1 => 1.40,
        2 | 3 => 0.95,
        4 | 5 => 0.90,
        6 | 7 => 0.80,
        8 | 9 => 0.60,
        _ => 0.40,
    }
}

/// Share of the temperature curve carried by atmospheric transport rather
/// than local insolation.
///
/// SPEC's curve, `T_night + ΔT · max(0, cos θ)^¼`, is pure radiative
/// balance: it is flat across the whole night side, so the frost zone
/// (105–140°) and the permanent-night zone (140–180°) could never differ, and
/// with its vertical slope at 90° the habitable band would be a degree wide.
/// Real air carries heat across the terminator and loses it with distance,
/// so the curve here is a blend of SPEC's term and a transport term that
/// falls linearly from the substellar point to the antistellar one. At zero
/// this is SPEC's formula exactly. The spread is still set per atmosphere by
/// [`locked_spread`]; this only sets the curve's shape.
const TERMINATOR_TRANSPORT: f64 = 0.65;

/// Half-width (degrees) of the twilight band the insolation term is softened
/// over, either side of the geometric terminator.
///
/// `max(0, cos θ)^¼` falls from 0.45 to nothing in the last degree before
/// 90°, which the ±0.06 temperature wobble can't bend: the terminator drew as
/// a ruler-straight meridian across the map. A real one is a band —
/// refraction and scattering carry light past the geometric edge, and a red
/// dwarf seen from 0.2 AU is a disc a degree or two wide, not a point. The
/// width is generous for the physics on purpose; at hex scale a narrower
/// band is still a line.
const TWILIGHT_DEG: f64 = 8.0;

/// `max(0, x)` with the corner rounded over `±sin(TWILIGHT_DEG)`: a quadratic
/// blend that meets both straight pieces with matching slope, so the curve
/// stays monotone and SPEC's insolation term is untouched outside the band.
fn twilight(x: f64) -> f64 {
    let d = TWILIGHT_DEG.to_radians().sin();
    if x >= d {
        x
    } else if x <= -d {
        0.0
    } else {
        (x + d) * (x + d) / (4.0 * d)
    }
}

/// Area-weighted mean of the blended curve's shape over the sphere: ⅖ for
/// the insolation term (`∫cos^¼θ sinθ dθ / 2` over the day side) and ½ for
/// the linear transport term. Used to place `t_night` so the locked world's
/// mean temperature matches [`ROTATING_MEAN_TEMP`] plus its UWP bias. The
/// twilight rounding ([`TWILIGHT_DEG`]) raises the true mean by under 0.01,
/// well inside the temperature wobble, so it isn't worth a numeric integral.
const LOCKED_CURVE_MEAN: f64 = (1.0 - TERMINATOR_TRANSPORT) * 0.4 + TERMINATOR_TRANSPORT * 0.5;

/// Largest fraction of the surface the antistellar cold trap holds before
/// water spills over as liquid. 15% is the top of hydrographics 1's band
/// (see `biome::draw_water_fraction`), and SPEC says hydrographics 0–1 is
/// trace water with *no* liquid anywhere — so the trap has to hold all of
/// it. A cap of that area reaches θ ≈ 134°: the permanent-night zone plus
/// the coldest edge of the frost zone.
pub const TRAP_CAPACITY: f64 = 0.15;

/// Temperature under the ice sheet. Below both the land ice threshold
/// (0.10) and the sea-ice one (0.15), so the sheet renders as ice whatever
/// the air above it is doing — a thick atmosphere's mild night doesn't melt a
/// sheet that exists because the cold trap put it there.
const SHEET_TEMP: f64 = 0.05;

/// Highest amplified relief that shows through the ice sheet. Just under the
/// Highland threshold (0.20): an ice sheet kilometres thick buries hills and
/// planes the surface into a plateau, so what lies under it classifies and
/// colours as ice rather than as bare highland with a cold temperature.
const SHEET_RELIEF_CAP: f64 = 0.18;

/// Degrees the sheet edge moves per unit of the roughness noise. The same
/// zero-mean noise that frays the cold zone's temperature, so the ice edge
/// and the frost around it wander together, and the sheet's mean extent stays
/// what the water budget says.
const SHEET_EDGE_ROUGH_DEG: f64 = 10.0;

/// Where liquid water collects once the trap is full: the inner (dayside)
/// edge of the terminator ring, where it is warm enough to stay liquid and
/// cool enough not to boil off. The bias is added to a hex's claim on the
/// water budget (see [`TideLockedClimate::water_potential`]), so low ground
/// near this ring floods first and the rest of the world only after.
///
/// A peaked bias rather than anything monotone in θ: a bias that simply grew
/// toward the night side would put the first liquid water on the *outer*,
/// freezing edge of the ring — the opposite of what the ring is.
const RING_THETA_DEG: f64 = 78.0;
/// Width (σ, degrees) of the liquid-water ring's bias.
const RING_WIDTH_DEG: f64 = 18.0;
/// Height of the liquid-water ring's bias, in raw elevation units. Plate
/// interiors sit on a ±0.35 plateau, so this is enough to fill the ring's
/// basins well before the plains elsewhere, but not so much that a seabed
/// near the ring reads as abyss.
const RING_BIAS: f64 = 0.45;

/// Fraction of the open (un-iced) surface whose floor defines the relief
/// datum on a world with no liquid water; those basins flatten to dry pans.
///
/// The rotating model's dry branch puts sea level a whole unit below the
/// lowest hex, which lifts every hex a unit off the datum and turns most of
/// a desert world into Highland and Mountain. A locked world with its water
/// in the cold trap is exactly that desert, so it takes the datum from its
/// own basin floors instead — SPEC's "residual dayside basins read as dry or
/// hypersaline, not open water" — and its plains read as plains.
const DRY_BASIN_FRACTION: f64 = 0.03;
/// Height above sea level of a dry basin floor. Positive so every classifier
/// and the hillshade treat it as land (they test `above > 0`), and small
/// enough to read as dead flat.
const DRY_BASIN_FLOOR: f64 = 1e-3;

/// Humidity removed at the substellar point. SPEC calls θ < 30° hyperarid:
/// whatever water reaches the hot spot is driven off to the cold trap. Fades
/// out by [`DAYSIDE_DRY_EDGE_DEG`], so the terminator keeps its moisture.
const DAYSIDE_DRYING: f64 = 0.60;
/// θ (degrees) inside which dayside drying is at full strength.
const DAYSIDE_DRY_CORE_DEG: f64 = 25.0;
/// θ (degrees) beyond which there is no dayside drying.
const DAYSIDE_DRY_EDGE_DEG: f64 = 75.0;

impl ClimateModel {
    /// The model for a map with these decorations. `seed` is the map seed
    /// (not mixed with the UWP) — it resolves a lock with no stated
    /// substellar point. Draws nothing from any RNG.
    pub fn from_decorations(decorations: &WorldDecorations, uwp: &Uwp, seed: u64) -> Self {
        let Some(lock) = &decorations.tide_locked else {
            return Self::Rotating;
        };
        let spread = locked_spread(uwp.atmosphere());
        let mean = ROTATING_MEAN_TEMP + temperature_bias(uwp);
        let t_night = mean - spread * LOCKED_CURVE_MEAN;
        Self::TideLocked(TideLockedClimate {
            substellar: lock.resolve(seed).to_unit_vector(),
            t_sub: t_night + spread,
            t_night,
            sheet_theta: f64::INFINITY,
            relief_hyd: uwp.hydrographics(),
            dry_floor: f64::NEG_INFINITY,
        })
    }

    pub fn is_tide_locked(&self) -> bool {
        matches!(self, Self::TideLocked(_))
    }

    /// Unit vector to the substellar point, if the world is locked.
    pub fn substellar(&self) -> Option<[f64; 3]> {
        match self {
            Self::Rotating => None,
            Self::TideLocked(tl) => Some(tl.substellar),
        }
    }

    /// Angle from the substellar point in radians (0..=π); `None` for a
    /// rotating world, which has no such point.
    pub fn theta(&self, p: &[f64; 3]) -> Option<f64> {
        match self {
            Self::Rotating => None,
            Self::TideLocked(tl) => Some(tl.theta(p)),
        }
    }

    /// Smooth temperature (UWP bias included, no wobble, no lapse) — the
    /// provisional per-hex value [`compute_climate`] writes before biome
    /// assignment overwrites it.
    pub fn base_temperature(&self, p: &[f64; 3], uwp: &Uwp) -> f64 {
        match self {
            Self::Rotating => adjust_temperature(temperature_at(p, TEMP_AMPLITUDE), uwp),
            Self::TideLocked(tl) => tl.curve(tl.theta(p)).clamp(0.0, 1.0),
        }
    }

    /// Pre-lapse surface temperature, UWP bias included. The one function
    /// every per-hex and per-pixel caller uses, so the biome a city is placed
    /// on is the colour drawn under it.
    pub fn surface_temperature(&self, p: &[f64; 3], tf: &TempField, uwp: &Uwp) -> f64 {
        match self {
            Self::Rotating => adjust_temperature(temperature_at_wobbled(p, tf), uwp),
            Self::TideLocked(tl) => tl.surface_temperature(p, tf),
        }
    }

    /// Humidity at `p`. A locked world's day side is dried toward the
    /// substellar point; a rotating world's is the field as sampled.
    pub fn humidity(&self, p: &[f64; 3], field: &HumidityField, uwp: &Uwp) -> f64 {
        match self {
            Self::Rotating => field.sample(p, uwp),
            Self::TideLocked(tl) => tl.humidity(p, field.sample(p, uwp)),
        }
    }

    /// Elevation as the water sees it: identical to `e` on a rotating world,
    /// lowered near a locked world's liquid-water ring so that ring floods
    /// first. A point is liquid water iff this is below `sea_level`.
    pub fn water_potential(&self, e: f64, p: &[f64; 3]) -> f64 {
        match self {
            Self::Rotating => e,
            Self::TideLocked(tl) => tl.water_potential(e, p),
        }
    }

    /// Whether `p` lies under a locked world's antistellar ice sheet. Always
    /// false on a rotating world, whose ice is purely a matter of temperature.
    pub fn under_ice_sheet(&self, p: &[f64; 3], tf: &TempField) -> bool {
        match self {
            Self::Rotating => false,
            Self::TideLocked(tl) => tl.under_sheet(tl.theta(p), tf.polar_roughness(p)),
        }
    }

    /// Amplified signed height above the local water surface — the `above`
    /// every classifier and colormap reads (negative = depth of water).
    /// `hyd` is the UWP's hydrographics, used as-is on a rotating world; `tf`
    /// locates a locked world's ice sheet, which buries the relief under it.
    pub fn above_sea(&self, e: f64, p: &[f64; 3], sea_level: f64, hyd: u8, tf: &TempField) -> f64 {
        match self {
            Self::Rotating => amplify_elevation(e - sea_level, hyd),
            Self::TideLocked(tl) => {
                let theta = tl.theta(p);
                let above = amplify_elevation(tl.potential_at(e, theta) - sea_level, tl.relief_hyd);
                if tl.under_sheet(theta, tf.polar_roughness(p)) {
                    above.min(SHEET_RELIEF_CAP)
                } else {
                    above
                }
            }
        }
    }
}

impl TideLockedClimate {
    /// θ in radians. `p` is a unit sphere position; the clamp absorbs the
    /// rounding that would otherwise hand `acos` a 1.0000000000000002.
    pub fn theta(&self, p: &[f64; 3]) -> f64 {
        let s = &self.substellar;
        (p[0] * s[0] + p[1] * s[1] + p[2] * s[2])
            .clamp(-1.0, 1.0)
            .acos()
    }

    /// Unclamped temperature curve in θ (radians): SPEC's insolation term
    /// blended with a linear transport term, see [`TERMINATOR_TRANSPORT`].
    /// Strictly decreasing from `t_sub` at θ = 0 to `t_night` at θ = π.
    pub fn curve(&self, theta: f64) -> f64 {
        let insolation = twilight(theta.cos()).powf(0.25);
        let transport = 1.0 - theta / std::f64::consts::PI;
        let shape = (1.0 - TERMINATOR_TRANSPORT) * insolation + TERMINATOR_TRANSPORT * transport;
        self.t_night + (self.t_sub - self.t_night) * shape
    }

    /// Wobbled, roughened, sheet-capped temperature at `p`.
    ///
    /// Mirrors [`temperature_at_wobbled`] with θ in place of latitude: the
    /// same low-frequency wobble so the rings aren't perfect circles, and the
    /// same high-frequency roughness — weighted toward the *antistellar*
    /// point rather than the poles — so the ice edge is ragged where a
    /// locked world's ice actually is.
    fn surface_temperature(&self, p: &[f64; 3], tf: &TempField) -> f64 {
        let theta = self.theta(p);
        let base = self.curve(theta);
        let wobble = tf.wobble(p) * TEMP_AMPLITUDE;
        let rough_raw = tf.polar_roughness(p);
        // -cos θ is the antistellar analogue of the rotating model's |sin lat|.
        let night = smoothstep(0.55, 0.95, -theta.cos());
        let rough = rough_raw * POLAR_ROUGH_AMPLITUDE * night;
        let t = (base + wobble + rough).clamp(0.0, 1.0);
        if self.under_sheet(theta, rough_raw) {
            t.min(SHEET_TEMP)
        } else {
            t
        }
    }

    /// Whether the antistellar ice sheet covers a point at `theta` whose
    /// roughness sample is `rough_raw`.
    fn under_sheet(&self, theta: f64, rough_raw: f64) -> bool {
        theta + SHEET_EDGE_ROUGH_DEG.to_radians() * rough_raw > self.sheet_theta
    }

    fn humidity(&self, p: &[f64; 3], sampled: f64) -> f64 {
        let dry = smoothstep(
            DAYSIDE_DRY_EDGE_DEG.to_radians().cos(),
            DAYSIDE_DRY_CORE_DEG.to_radians().cos(),
            self.theta(p).cos(),
        );
        (sampled - DAYSIDE_DRYING * dry).clamp(0.0, 1.0)
    }

    fn water_potential(&self, e: f64, p: &[f64; 3]) -> f64 {
        self.potential_at(e, self.theta(p))
    }

    fn potential_at(&self, e: f64, theta: f64) -> f64 {
        let d = (theta.to_degrees() - RING_THETA_DEG) / RING_WIDTH_DEG;
        (e - RING_BIAS * (-d * d).exp()).max(self.dry_floor)
    }

    /// Split the world's water fraction between the antistellar ice trap and
    /// liquid water, and return the sea level for the liquid part.
    ///
    /// SPEC's order: the cold trap takes water first, up to
    /// [`TRAP_CAPACITY`]; only the excess is liquid, and it goes where
    /// [`Self::water_potential`] is lowest — the basins of the inner
    /// terminator ring, then everywhere else. The liquid fraction is counted
    /// over the whole grid but placed only outside the sheet, so ice plus
    /// water together cover about `frac_water` of the world.
    ///
    /// The sheet is a cap of the right area around the antistellar point
    /// (`area = (1 + cos θ) / 2`), so its edge is analytic and every pixel can
    /// test it without the hex list; the returned sea level is a percentile of
    /// hex water potentials, which is what the rotating model does with raw
    /// elevation.
    pub fn settle_water(&mut self, frac_water: f64, grid: &Grid, tf: &TempField) -> f64 {
        let ice = frac_water.min(TRAP_CAPACITY);
        let liquid = frac_water - ice;
        self.sheet_theta = if ice > 0.0 {
            (2.0 * ice - 1.0).acos()
        } else {
            f64::INFINITY
        };
        self.relief_hyd = (liquid * 10.0).round() as u8;

        self.dry_floor = f64::NEG_INFINITY;
        let mut open: Vec<f64> = grid
            .hexes
            .iter()
            .filter(|h| {
                let theta = self.theta(&h.sphere_pos);
                !self.under_sheet(theta, tf.polar_roughness(&h.sphere_pos))
            })
            .map(|h| self.water_potential(h.elevation, &h.sphere_pos))
            .collect();
        open.sort_by(f64::total_cmp);
        let Some(&highest) = open.last() else {
            // Everything is under the sheet (not reachable at TRAP_CAPACITY,
            // but a finite level keeps `above` finite if it ever is).
            return grid.hexes.iter().map(|h| h.elevation).fold(0.0, f64::min) - 1.0;
        };
        let wet = (grid.hexes.len() as f64 * liquid).round() as usize;
        if wet == 0 {
            // No liquid: the datum is the floor of the deepest basins, and
            // the floor clamp keeps anything below it (a pixel can dip under
            // every hex centre) dry. See DRY_BASIN_FRACTION.
            let idx = (open.len() as f64 * DRY_BASIN_FRACTION) as usize;
            let datum = open[idx.min(open.len() - 1)];
            self.dry_floor = datum + DRY_BASIN_FLOOR;
            datum
        } else if wet >= open.len() {
            highest + 1.0
        } else {
            open[wet]
        }
    }
}
