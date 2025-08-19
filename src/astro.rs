use crate::system::Star;
use crate::system_tables::{
    get_cloudiness, get_greenhouse, get_habitable, get_luminosity, get_orbital_distance,
    get_solar_mass, get_world_temp,
};
use crate::world::World;

const WATER_ALBEDO: f32 = 0.02;
const LAND_ALBEDO: f32 = 0.1;
const ICE_ALBEDO: f32 = 0.85;
const CLOUD_ALBEDO: f32 = 0.5;
const EARTH_TEMP: f32 = 288.0;

#[derive(Debug, Clone, PartialEq)]
pub struct AstroData {
    // Orbital period in Earth years
    orbital_period: f32,
    // Orbital distance in AU
    orbit_distance: f32,
    albedo: f32,
    temp: f32,
    gravity: f32,
    // Mass in Earth equivalents
    mass: f32,
    ice_cap_percent: f32,
    greenhouse: f32,
    luminosity: f32,
}

impl AstroData {
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

    pub fn compute(star: &Star, world: &World) -> AstroData {
        let mut astro = AstroData::new();
        astro.compute_orbital_period(star, world.position_in_system);
        astro.compute_mass_gravity(world.size);

        // Do this twice to try and estimate icecap
        astro.compute_albedo_temp(
            world.position_in_system,
            world.atmosphere,
            world.hydro,
            star,
        );
        // at temp = 273 then 0.5x of hydro is ice
        // at temp = 243 then 2x hydro is ice.
        // y(273) = 0.5 = m(273) + b
        // y(223) = 2 = m(223) + b
        // 0 = 273m + b - 0.5
        // 0 = 223m + b - 2
        // 0 = 50m + 1.5
        // m = -1.5/50 = -0.03
        // 0.5 = -0.03*273 + b
        // b = 0.5 + (-0.03*273) = 0.5 + 8.19 = 8.69
        // y = -0.03x + 8.69
        astro.ice_cap_percent =
            (world.hydro as f32 / 5.0 * (869.0 / 80.0 - 3.0 / 80.0 * astro.temp)).clamp(0.0, 1.0);

        //astro.compute_albedo_temp(world.atmosphere, world.hydro, star);
        astro
    }

    fn compute_orbital_period(&mut self, star: &Star, orbit: usize) {
        // P = sqrt(MD^3) OR T = 2pi sqrt(r^3/GM)
        let mass = get_solar_mass(star);
        // Convert Million KM to AU by dividing by 149.6
        self.orbit_distance = get_orbital_distance(orbit as i32) / 149.6;
        self.orbital_period = (mass * self.orbit_distance.powi(3)).sqrt();
    }

    fn compute_mass_gravity(&mut self, size: i32) {
        // Mass in Earth equivalents.
        self.mass = (size as f32 / 8.0).powi(3);
        // G = M(8/SIZE)^2
        // TODO: Validate this math is right!
        self.gravity = if size <= 0 { 0.0 } else { size as f32 / 8.0 };
    }

    fn compute_albedo_temp(&mut self, position: usize, atmosphere: i32, hydro: i32, star: &Star) {
        let cloud_percent = get_cloudiness(atmosphere) as f32 / 100.0;
        let mut water_percent = hydro as f32 / 10.0;
        let mut land_percent = 1.0 - water_percent;
        let mut ice_percent = self.ice_cap_percent;
        if water_percent >= ice_percent / 2.0 && land_percent >= ice_percent / 2.0 {
            land_percent -= 0.5 * self.ice_cap_percent;
            water_percent -= 0.5 * self.ice_cap_percent;
        } else if water_percent < land_percent {
            // remainder is how much additional we have to take out of the land
            let remainder = ice_percent / 2.0 - water_percent;
            land_percent -= ice_percent / 2.0 + remainder;
            water_percent = 0.0;
        } else {
            let remainder = ice_percent / 2.0 - land_percent;
            water_percent -= ice_percent / 2.0 + remainder;
            land_percent = 0.0;
        }

        ice_percent *= cloud_percent;
        water_percent *= cloud_percent;
        land_percent *= cloud_percent;

        self.albedo = cloud_percent * CLOUD_ALBEDO
            + water_percent * WATER_ALBEDO
            + land_percent * LAND_ALBEDO
            + ice_percent * ICE_ALBEDO;

        // T = KG(1-A)(L^0.25)/D^0.5 where
        // K = 374.02
        // G = 1 + greenhouse effect
        // A = Albedo (0-0.99)
        // L = Luminosity in solar units
        // D = Distance in AU from primary
        let k = 374.02;
        self.greenhouse = 1.0 + get_greenhouse(atmosphere);
        self.luminosity = get_luminosity(star);

        // One formula for habitable zone (trying to make the planet habitable) and
        // then another for other orbits.
        if position == get_habitable(star) as usize {
            let temp_modifier: i32 =
                ((self.greenhouse * (1.0 - self.albedo) - 1.0) / 0.05).trunc() as i32;
            // Convert from Celsius to Kelvin
            self.temp = get_world_temp(temp_modifier) + 273.0;
        } else {
            self.temp = k * self.greenhouse * (1.0 - self.albedo) * self.luminosity.powf(0.25)
                / self.orbit_distance.powf(0.5);
        }
    }

    pub fn get_astro_description(&self, world: &World) -> String {
        if world.atmosphere <= 1 {
            return "".to_string();
        }

        format!(
            "{:+0.2} °C, {:2.0}% ice, {:0.1}G, {:0.1} yrs",
            self.temp - EARTH_TEMP + 15.0,
            (self.ice_cap_percent * 100.0).round(),
            self.gravity,
            self.orbital_period
        )
    }
}
