//! # Astronomical Data Module
//!
//! This module provides astronomical calculations and data structures for worlds in the Traveller universe.
//! It handles orbital mechanics, planetary physics, atmospheric modeling, and temperature calculations
//! to generate realistic astronomical data for worlds based on their star system characteristics.
//!
//! ## Key Features
//!
//! - **Orbital Mechanics**: Calculates orbital periods and distances using Kepler's laws
//! - **Planetary Physics**: Computes mass, gravity, and surface conditions
//! - **Climate Modeling**: Determines temperature, albedo, and ice cap coverage
//! - **Atmospheric Effects**: Models greenhouse effects and cloud coverage
//! - **Stellar Interactions**: Accounts for star type, luminosity, and habitable zones
//!
//! ## Constants
//!
//! The module defines several physical constants used in calculations:
//! - Surface albedo values for different terrain types (water, land, ice, clouds)
//! - Earth's average temperature as a reference point for climate calculations
//!
//! ## Usage
//!
//! The primary interface is the [`AstroData`] struct, which can be computed from
//! a star and world combination to generate comprehensive astronomical data.
//!
//! ```rust,ignore
//! # use worldgen::systems::astro::AstroData;
//! # use worldgen::systems::{system::Star, world::World};
//! let astro_data = AstroData::compute(&star, &world);
//! let description = astro_data.get_astro_description(&world);
//! ```
use serde::{Deserialize, Serialize};

use crate::systems::system::{Star, StarSize};
use crate::systems::system_tables::{
    get_cloudiness, get_greenhouse, get_habitable, get_luminosity, get_orbital_distance,
    get_solar_mass, get_world_temp,
};
use crate::systems::world::World;

/// Albedo (reflectivity) constant for water surfaces
const WATER_ALBEDO: f32 = 0.02;

/// Albedo (reflectivity) constant for land surfaces  
const LAND_ALBEDO: f32 = 0.1;

/// Albedo (reflectivity) constant for ice surfaces
const ICE_ALBEDO: f32 = 0.85;

/// Albedo (reflectivity) constant for cloud cover
const CLOUD_ALBEDO: f32 = 0.5;

/// Earth's average temperature in Kelvin, used as reference
const EARTH_TEMP: f32 = 288.0;

/// For stating a tide-locked world's day in days.
const DAYS_PER_YEAR: f32 = 365.25;

/// Lock radius, in AU, around a one-solar-mass star; see [`tidal_lock_radius_au`].
///
/// 0.38 rather than a round 0.4 on purpose. The mass table rounds G2 V to
/// G0 V (1.04 M☉), which gives 0.385 AU — just inside orbit 1 (0.3997 AU).
/// At 0.4 a Sun-like star would lock orbit 1, and Mercury, at 0.39 AU, isn't
/// locked (it's in a 3:2 spin-orbit resonance). Orbit 0 (0.1999 AU) locks
/// around every main-sequence star in the table.
pub const TIDAL_LOCK_COEFFICIENT_AU: f32 = 0.38;

/// How far out a main-sequence star tide-locks its planets:
/// `0.38 · (M / M☉)^(1/3)` AU.
///
/// The time to tidally lock scales roughly as a⁶/M². Holding that time fixed
/// (at "much shorter than the system's age") and solving for the distance
/// gives a ∝ M^(1/3): a heavier star locks further out, but only weakly. With
/// the table's masses that's orbit 0 for M, K and G dwarfs, and orbits 0–1
/// from F5 V (1.3 M☉, 0.41 AU) upward. M5–M9 V all read 0.331 M☉, because the
/// table rounds subtypes to 0 or 5.
pub fn tidal_lock_radius_au(star: &Star) -> f32 {
    TIDAL_LOCK_COEFFICIENT_AU * get_solar_mass(star).cbrt()
}

/// Whether physics alone would tide-lock a body orbiting `star` directly at
/// `orbit_distance_au`.
///
/// **Not applied automatically, anywhere.** A world is tide-locked only when
/// `data/overrides.json` says so. TravellerMap's UWPs were generated with no
/// notion of tidal locking, and a red-dwarf main world only lands at orbit 0
/// because Traveller's orbit grid is too coarse to place it anywhere closer
/// — so "M-dwarf main world at orbit 0" is an artifact of the rules, not
/// evidence of a lock. Measured against Trojan Reach, this rule locked 64 of
/// 327 main worlds, Drinax among them; even restricted to M5–M9 V it locked
/// 19, six of them with hydrographics 5–9 (Forandin, Gor, Aohfeau,
/// Aiuiktiyr, Szirp, Janus) — worlds the published data describes as wet and
/// temperate. Locking them would decide facts about the setting that belong
/// to whoever runs it. The physics stays here, tested, for an opt-in later.
///
/// Size V only: giants and white dwarfs never auto-lock. Their table masses
/// say nothing useful about a planet's history — a giant has swollen through
/// its inner orbits and a white dwarf's planets survived a giant phase — so
/// the scaling argument doesn't apply. Callers must not apply this to moons
/// — a moon locks to its planet, not its star, so its substellar point sweeps
/// round once per month and the fixed-hot-spot climate model is wrong for it.
///
/// Pure table lookups, no dice, so a future opt-in can't perturb the
/// generator's random stream.
pub fn auto_tide_locked(star: &Star, orbit_distance_au: f32) -> bool {
    star.size == StarSize::V && orbit_distance_au <= tidal_lock_radius_au(star)
}

/// Comprehensive astronomical data for a world
///
/// Contains all calculated astronomical and physical properties of a world,
/// including orbital characteristics, surface conditions, and atmospheric data.
/// This data is used for generating realistic world descriptions and determining
/// habitability and environmental conditions.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AstroData {
    /// Orbital period in Earth years
    orbital_period: f32,
    /// Orbital distance in AU (Astronomical Units)
    orbit_distance: f32,
    /// Surface albedo (reflectivity) from 0.0 to 1.0
    albedo: f32,
    /// Surface temperature in Kelvin
    temp: f32,
    /// Surface gravity in Earth gravities
    gravity: f32,
    /// Planetary mass in Earth equivalents
    mass: f32,
    /// Percentage of surface covered by ice caps (0.0 to 1.0)
    ice_cap_percent: f32,
    /// Greenhouse effect multiplier
    greenhouse: f32,
    /// Stellar luminosity in solar units
    luminosity: f32,
}

impl AstroData {
    /// Creates a new `AstroData` instance with default values
    ///
    /// All values are initialized to zero except `ice_cap_percent` which defaults to 0.1 (10%).
    /// This provides a baseline for subsequent calculations.
    pub fn new() -> AstroData {
        AstroData {
            orbital_period: 0.0,
            orbit_distance: 0.0,
            albedo: 0.0,
            temp: 0.0,
            gravity: 0.0,
            mass: 0.0,
            ice_cap_percent: 0.1,
            greenhouse: 0.0,
            luminosity: 0.0,
        }
    }

    /// Computes comprehensive astronomical data for a world
    ///
    /// This is the main calculation method that determines all astronomical properties
    /// of a world based on its star system and physical characteristics. The calculation
    /// process includes orbital mechanics, mass/gravity relationships, and climate modeling.
    ///
    /// # Arguments
    ///
    /// * `star` - The primary star of the system
    /// * `world` - The world to calculate data for
    ///
    /// # Returns
    ///
    /// Complete `AstroData` with all calculated values
    ///
    /// # Climate Model
    ///
    /// The ice cap calculation uses a linear relationship between temperature and ice coverage:
    /// - At 273K (0°C): 50% of hydrographics becomes ice
    /// - At 223K (-50°C): 200% of hydrographics becomes ice
    /// - Formula: y = -0.03x + 8.69 (where x is temperature, y is ice multiplier)
    pub fn compute(star: &Star, world: &World) -> AstroData {
        let mut astro = AstroData::new();
        astro.compute_orbital_period(star, world.position_in_system, world.orbit_distance_mkm);
        astro.compute_mass_gravity(world.size);

        // Initial temperature calculation for ice cap estimation
        astro.compute_albedo_temp(
            // A stated distance means the orbit slot no longer says where the
            // world is, so the slot-keyed habitable-zone lookup doesn't apply.
            // `None`, not a sentinel slot: an earlier `usize::MAX` collided
            // with `get_habitable` returning -1 (no habitable orbit) cast to
            // usize, and sent Hilfer to the table's +5 °C.
            world
                .orbit_distance_mkm
                .is_none()
                .then_some(world.position_in_system),
            world.atmosphere,
            world.hydro,
            star,
        );

        // Ice cap calculation based on temperature and hydrographics
        // Linear relationship: at 273K = 0.5x hydro, at 223K = 2x hydro
        astro.ice_cap_percent =
            (world.hydro as f32 / 5.0 * (869.0 / 80.0 - 3.0 / 80.0 * astro.temp)).clamp(0.0, 1.0);

        astro
    }

    /// Orbital period in Earth years, as computed by
    /// [`compute_orbital_period`](Self::compute_orbital_period).
    pub fn orbital_period_years(&self) -> f32 {
        self.orbital_period
    }

    /// Distance from the star in AU.
    pub fn orbit_distance_au(&self) -> f32 {
        self.orbit_distance
    }

    /// Surface temperature in kelvin, as the description reports it.
    #[cfg(test)]
    pub(crate) fn temperature_k(&self) -> f32 {
        self.temp
    }

    /// Calculates orbital period and distance using Kepler's third law.
    ///
    /// P = sqrt(D³ / M), with P in years, D in AU and M in solar masses. This
    /// used to be `sqrt(M · D³)`, which agrees only for a one-solar-mass star
    /// and is wrong by a factor of M everywhere else — too short around small
    /// stars, too long around big ones. Hilfer, an M6 V world at orbit 0,
    /// read a year of 18.8 days where the physics gives ~57.
    ///
    /// `stated_mkm` overrides the orbit slot's distance when a source gives
    /// one (see `World::orbit_distance_mkm`).
    ///
    /// # Arguments
    ///
    /// * `star` - The primary star
    /// * `orbit` - Orbital position index in the system
    /// * `stated_mkm` - A stated distance in millions of km, if any
    fn compute_orbital_period(&mut self, star: &Star, orbit: usize, stated_mkm: Option<f32>) {
        let mass = get_solar_mass(star);
        // Convert from million km to AU (1 AU = 149.6 million km)
        let mkm = stated_mkm.unwrap_or_else(|| get_orbital_distance(orbit as i32));
        self.orbit_distance = mkm / 149.6;
        // O-class rows in the mass table are 0.0 placeholders, not data.
        // Dividing by them would give an infinite year; report none, which is
        // what the old multiply-by-mass formula happened to produce.
        self.orbital_period = if mass > 0.0 {
            (self.orbit_distance.powi(3) / mass).sqrt()
        } else {
            0.0
        };
    }

    /// Calculates planetary mass and surface gravity
    ///
    /// Mass scales with the cube of size ratio to Earth (size 8).
    /// Gravity scales linearly with size ratio.
    ///
    /// # Arguments
    ///
    /// * `size` - World size code (0-10+ in Traveller system)
    ///
    /// # Note
    ///
    /// The gravity calculation may need validation - currently uses simple linear scaling.
    fn compute_mass_gravity(&mut self, size: i32) {
        // Mass scales as cube of radius (size/8)³
        self.mass = (size as f32 / 8.0).powi(3);
        // Gravity scales linearly with size ratio
        self.gravity = if size <= 0 { 0.0 } else { size as f32 / 8.0 };
    }

    /// Calculates surface albedo and temperature
    ///
    /// This complex calculation models:
    /// - Surface composition (water, land, ice percentages)
    /// - Cloud coverage effects
    /// - Greenhouse warming from atmosphere
    /// - Stellar heating based on luminosity and distance
    ///
    /// Uses different temperature formulas for habitable zone vs. other orbits.
    ///
    /// # Arguments
    ///
    /// * `position` - Orbital position in system
    /// * `atmosphere` - Atmospheric density code
    /// * `hydro` - Hydrographics percentage
    /// * `star` - Primary star characteristics
    fn compute_albedo_temp(
        &mut self,
        position: Option<usize>,
        atmosphere: i32,
        hydro: i32,
        star: &Star,
    ) {
        let cloud_percent = get_cloudiness(atmosphere) as f32 / 100.0;
        let mut water_percent = hydro as f32 / 10.0;
        let mut land_percent = 1.0 - water_percent;
        let mut ice_percent = self.ice_cap_percent;

        // Distribute ice caps between water and land surfaces
        if water_percent >= ice_percent / 2.0 && land_percent >= ice_percent / 2.0 {
            land_percent -= 0.5 * self.ice_cap_percent;
            water_percent -= 0.5 * self.ice_cap_percent;
        } else if water_percent < land_percent {
            let remainder = ice_percent / 2.0 - water_percent;
            land_percent -= ice_percent / 2.0 + remainder;
            water_percent = 0.0;
        } else {
            let remainder = ice_percent / 2.0 - land_percent;
            water_percent -= ice_percent / 2.0 + remainder;
            land_percent = 0.0;
        }

        // Apply cloud coverage to surface percentages
        ice_percent *= cloud_percent;
        water_percent *= cloud_percent;
        land_percent *= cloud_percent;

        // Calculate weighted average albedo
        self.albedo = cloud_percent * CLOUD_ALBEDO
            + water_percent * WATER_ALBEDO
            + land_percent * LAND_ALBEDO
            + ice_percent * ICE_ALBEDO;

        // Temperature calculation: T = K*G*(1-A)*L^0.25/D^0.5
        let k = 374.02; // Scaling constant
        self.greenhouse = 1.0 + get_greenhouse(atmosphere);
        self.luminosity = get_luminosity(star);

        // Different formulas for habitable zone vs other orbits
        // Compared as i32: `get_habitable` is -1 when the star has no
        // habitable orbit, and must then match nothing.
        if position.is_some_and(|p| p as i32 == get_habitable(star)) {
            // Habitable zone: use lookup table with greenhouse modifier
            let temp_modifier: i32 =
                ((self.greenhouse * (1.0 - self.albedo) - 1.0) / 0.05).trunc() as i32;
            self.temp = get_world_temp(temp_modifier) + 273.0; // Convert C to K
        } else {
            // Other orbits: use direct stellar heating formula
            self.temp = k * self.greenhouse * (1.0 - self.albedo) * self.luminosity.powf(0.25)
                / self.orbit_distance.powf(0.5);
        }
    }

    /// Generates a human-readable description of the world's astronomical data
    ///
    /// Returns a formatted string containing temperature (in Celsius relative to Earth),
    /// ice cap percentage, surface gravity, and orbital period, plus the day
    /// length for a tide-locked world. Worlds with very thin atmospheres (≤1)
    /// lack meaningful climate data, so they get only the tide lock, if any,
    /// and otherwise an empty string.
    ///
    /// # Arguments
    ///
    /// * `world` - The world to describe
    ///
    /// # Returns
    ///
    /// Formatted string like "+15.23 °C, 25% ice, 1.2G, 1.1 yrs" or empty string
    ///
    /// # Example Output
    ///
    /// ```text
    /// "+12.50 °C, 15% ice, 0.8G, 2.3 yrs"
    /// ```
    pub fn get_astro_description(&self, world: &World) -> String {
        // A tidal lock is a fact about the world's rotation, not its weather,
        // so it's worth saying even for a world thin enough that the climate
        // figures are suppressed. Given in days: at orbit 0 of a red dwarf
        // the period is a few weeks, which "0.1 yrs" would hide.
        let lock = world.day_length_years().map(|day| {
            format!("tide-locked (day = year = {:0.1} days)", day * DAYS_PER_YEAR)
        });
        if world.atmosphere <= 1 {
            return lock.unwrap_or_default();
        }

        let climate = format!(
            "{:+0.2} °C, {:2.0}% ice, {:0.1}G, {:0.1} yrs",
            self.temp - EARTH_TEMP + 15.0, // Temperature relative to Earth
            // `+ 0.0` turns -0.0 into 0.0. A world with no water computes its
            // ice as 0 × (a negative factor) = -0.0, which `clamp` keeps and
            // the formatter printed as "-0%".
            (self.ice_cap_percent * 100.0).round() + 0.0, // Ice coverage percentage
            self.gravity,
            self.orbital_period
        );
        // Appended only when locked, so every unlocked world's description
        // is exactly what it was.
        match lock {
            Some(lock) => format!("{climate}, {lock}"),
            None => climate,
        }
    }
}
