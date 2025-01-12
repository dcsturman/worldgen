import { System, World, getCloudiness, getGreenhouse, getOrbitalDistance, getSolarMass, getLuminosity} from "./worldgen";

const WATER_ALBEDO = 0.02;
const LAND_ALBEDO = 0.1;
const ICE_ALBEDO = 0.85;
const CLOUD_ALBEDO = 0.5;
const EARTH_TEMP = 288;

export class AstroData {
  // Orbital period in Earth years
  orbital_period: number = 0;
  // Orbital distance in AU
  orbit_distance: number = 0;
  albedo: number = 0;
  temp: number = 0;
  gravity: number = 0;
  // Mass in Earth equivalents
  mass: number = 0;
  ice_cap_percent: number = 0.1;

  constructor() {
    this.orbital_period = 0;
    this.albedo = 0;
    this.mass = 0;
  }

  compute(primary: System, world: World) {
    this.computeOrbitalPeriod(primary, world.star_orbit);
    this.computeMassGravity(world.size);

    // Do this twice to try and estimate icecap
    this.computeAlbedoTemp(world.atmosphere, world.hydro, primary);
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
    this.ice_cap_percent = Math.max(Math.min(2.0 * world.hydro/10.0*(-0.03*this.temp + 8.69), 1.0), 0.0);
    this.computeAlbedoTemp(world.atmosphere, world.hydro, primary);
  }

  computeOrbitalPeriod(primary: System, orbit: number) {
    // P = sqrt(MD^3) OR T = 2pi sqrt(r^3/GM)
    let mass = getSolarMass(primary);
    // Convert Million KM to AU by dividing by 149.6
    this.orbit_distance = getOrbitalDistance(orbit) / 149.6;
    console.group("Orbit calc");
    console.log("Orbit: " + orbit);
    console.log("Orbit in AU: " + this.orbit_distance);
    console.groupEnd();
    this.orbital_period = Math.sqrt(mass * Math.pow(this.orbit_distance, 3));
  }

  computeMassGravity(size: number) {
    // Mass in Earth equivalents.
    this.mass = Math.pow(size / 8, 3);
    // G = M(8/SIZE)^2
    this.gravity = this.mass / Math.pow(size/8, 2);
  }

  computeAlbedoTemp(atmosphere: number, hydro: number, primary: System) {
    let cloud_percent = getCloudiness(atmosphere)/100;
    let water_percent = hydro/10;
    let land_percent = (1 - water_percent);
    let ice_percent = this.ice_cap_percent;
    if (water_percent >= ice_percent/2 && land_percent >= ice_percent/2
    ) {
        land_percent -= 0.5 * this.ice_cap_percent;
        water_percent -= 0.5 * this.ice_cap_percent;
    } else if (water_percent < land_percent) {
        // remainder is how much additional we have to take out of the land
        let remainder = ice_percent/2 - water_percent;
        land_percent -= ice_percent/2 + remainder;
        water_percent = 0;
    } else {
        let remainder = ice_percent/2 - land_percent;
        water_percent -= ice_percent/2 + remainder;
        land_percent = 0;
    }
    
    ice_percent = ice_percent * cloud_percent;
    water_percent = water_percent * cloud_percent;
    land_percent = land_percent * cloud_percent;

    console.group("Albedo Data");
    console.log("Atmosphere: " + atmosphere);
    console.log("Hydro: " + hydro);
    console.log("Cloud percent: " + cloud_percent);
    console.log("Water percent: " + water_percent);
    console.log("Land percent: " + land_percent);
    console.log("Ice percent: " + ice_percent);
    console.groupEnd();

    this.albedo = cloud_percent * CLOUD_ALBEDO + water_percent * WATER_ALBEDO + land_percent * LAND_ALBEDO + ice_percent * ICE_ALBEDO;

    // T = KG(1-A)(L^0.25)/D^0.5 where
    // K = 374.02
    // G = 1 + greenhouse effect
    // A = Albedo (0-0.99)
    // L = Luminosity in solar units
    // D = Distance in AU from primary
    const K = 374.02;
    let gh_impact = 1 + getGreenhouse(atmosphere);
    let luminosity = getLuminosity(primary);

    this.temp = K * gh_impact * (1 - this.albedo) * Math.pow(luminosity, 0.25) / Math.pow(this.orbit_distance, 0.5);
    console.group("Temp Data");
    console.log("Temp: " + this.temp);
    console.log("K: " + K);
    console.log("GH: " +  gh_impact);
    console.log("A: " + this.albedo);
    console.log("Lum: " + luminosity);
    console.log("Orbit distance: " + this.orbit_distance);
    console.groupEnd()
  }

  describe(world: World): string {
    if (world === null || world.atmosphere <= 1) {
      return "";
    }

    let temp_diff = this.temp - EARTH_TEMP
    let desc = `${temp_diff > 0 ? "+"+temp_diff.toFixed(1) : temp_diff.toFixed(1)} °K, ${(this.ice_cap_percent*100.0).toFixed(0)}% ice, ${this.gravity.toFixed(1)}G, ${this.orbital_period.toFixed(1)} yrs`;
    console.log(desc);
    return desc;
  }
}