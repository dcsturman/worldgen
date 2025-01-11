use crate::system_tables::{get_cloudiness, get_greenhouse, get_luminosity, get_orbital_distance, get_solar_mass};
use crate::worldgen::{World, Star};

const WATER_ALBEDO: f32 = 0.02;
const LAND_ALBEDO: f32 = 0.1;
const ICE_ALBEDO: f32 = 0.85;
const CLOUD_ALBEDO: f32 = 0.5;
const EARTH_TEMP: f32 = 288.0;

#[derive(Debug, Clone)]
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
        }
    }

    pub fn compute(star: &Star, world: &World) -> AstroData {
        let mut astro = AstroData::new();
        astro.compute_orbital_period(star, world.position_in_system);
        astro.compute_mass_gravity(world.size);

        // Do this twice to try and estimate icecap
        astro.compute_albedo_temp(world.atmosphere, world.hydro, star);
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
        astro.ice_cap_percent = (2.0 * world.hydro as f32 / 10.0 * (-0.03 * astro.temp + 8.69)).max(0.0).min(1.0);
        astro.compute_albedo_temp(world.atmosphere, world.hydro, star);
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
        self.gravity = self.mass / (size as f32 / 8.0).powi(2);
    }

    fn compute_albedo_temp(&mut self, atmosphere: i32, hydro: i32, star: &Star) {
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

        ice_percent = ice_percent * cloud_percent;
        water_percent = water_percent * cloud_percent;
        land_percent = land_percent * cloud_percent;

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
        let gh_impact = 1.0 + get_greenhouse(atmosphere);
        let luminosity = get_luminosity(star);

        self.temp = k * gh_impact * (1.0 - self.albedo) * luminosity.powf(0.25) / self.orbit_distance.powf(0.5);
    }

    pub fn describe(&self, world: &World) -> String {
        if world.atmosphere <= 1 {  
            return "".to_string();
        }
    
        let temp_diff = self.temp - EARTH_TEMP;
        format!("{:+0.2} °K, {:2.2}% ice, {:0.1}G, {:0.1} yrs", temp_diff, (self.ice_cap_percent * 100.0).round(), self.gravity, self.orbital_period)
    }
}

