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
//! ```rust
//! use worldgen::systems::{astro::AstroData, system::Star, world::World};
//! 
//! let astro_data = AstroData::compute(&star, &world);
//! let description = astro_data.get_astro_description(&world);
//! ```

use crate::systems::system::Star;
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

/// Comprehensive astronomical data for a world
/// 
/// Contains all calculated astronomical and physical properties of a world,
/// including orbital characteristics, surface conditions, and atmospheric data.
/// This data is used for generating realistic world descriptions and determining
/// habitability and environmental conditions.
#[derive(Debug, Clone, PartialEq, Default)]
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
        astro.compute_orbital_period(star, world.position_in_system);
        astro.compute_mass_gravity(world.size);

        // Initial temperature calculation for ice cap estimation
        astro.compute_albedo_temp(
            world.position_in_system,
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

    /// Calculates orbital period and distance using Kepler's laws
    /// 
    /// Uses the formula P = sqrt(M * D³) where:
    /// - P = orbital period in years
    /// - M = stellar mass in solar masses  
    /// - D = orbital distance in AU
    /// 
    /// # Arguments
    /// 
    /// * `star` - The primary star
    /// * `orbit` - Orbital position index in the system
    fn compute_orbital_period(&mut self, star: &Star, orbit: usize) {
        let mass = get_solar_mass(star);
        // Convert from million km to AU (1 AU = 149.6 million km)
        self.orbit_distance = get_orbital_distance(orbit as i32) / 149.6;
        self.orbital_period = (mass * self.orbit_distance.powi(3)).sqrt();
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
    fn compute_albedo_temp(&mut self, position: usize, atmosphere: i32, hydro: i32, star: &Star) {
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
        if position == get_habitable(star) as usize {
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
    /// ice cap percentage, surface gravity, and orbital period. Returns empty string
    /// for worlds with very thin atmospheres (≤1) as they lack meaningful climate data.
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
        if world.atmosphere <= 1 {
            return "".to_string();
        }

        format!(
            "{:+0.2} °C, {:2.0}% ice, {:0.1}G, {:0.1} yrs",
            self.temp - EARTH_TEMP + 15.0, // Temperature relative to Earth
            (self.ice_cap_percent * 100.0).round(), // Ice coverage percentage
            self.gravity,
            self.orbital_period
        )
    }
}
