import { AstroData } from "./astrodata";

export const FAR_ORBIT = 20;
export const PRIMARY_ORBIT = -1;

export class System {
  name: string;
  star: Star = new Star();
  secondary: System | null = null;
  tertiary: System | null = null;
  orbit: number = -1;
  max_orbits: number = 0;
  main_world: World | null = null;

  // Orbits can be 1) System itself, 2) a World 3) Planetoid 4) GasGiant
  // 5) Empty (intentionally empty and cannot be filled)
  // 6) null (not yet assigned)

  orbits: (World | GasGiant | System | Empty | null)[] = [];

  constructor(
    star_type: StarType,
    subtype: number,
    size: StarSize,
    orbit: number,
    max_orbits: number
  ) {
    this.name =
      starSystemNames[Math.floor(Math.random() * starSystemNames.length)];
    this.star.star_type = star_type;
    this.star.subtype = subtype;
    this.star.size = size;
    this.orbit = orbit;
    this.max_orbits = max_orbits;
  }

  setMaxOrbits(max_orbits: number) {
    this.max_orbits = max_orbits;
    this.orbits = new Array(max_orbits).fill(null);
  }
}

export enum PortCode {
  A,
  B,
  C,
  D,
  E,
  X,
  Y,
  H,
  G,
  F,
}

export enum StarType {
  O,
  B,
  A,
  F,
  G,
  K,
  M,
}

export enum StarSize {
  Ia,
  Ib,
  II,
  III,
  IV,
  V,
  VI,
  D,
}

enum Facility {
  Naval,
  Scout,
  Farming,
  Mining,
  Colony,
  Lab,
  Military,
}

enum TradeClass {
  Agricultural,
  NonAgricultural,
  Industrial,
  NonIndustrial,
  Rich,
  Poor,
  WaterWorld,
  DesertWorld,
  VacuumWorld,
  Icecapped,
}

export class World {
  name: string;
  // Orbits out from the primary, if a world in orbit, or orbit around a world/gas giant
  // if a satellite
  orbit: number = 0;
  // Orbits out from the primary star (even if a satellite)
  star_orbit: number = 0;

  is_satellite: boolean = false;
  is_mainworld: boolean = false;

  // UPP Data
  port: PortCode = PortCode.A;
  size: number = 0;
  atmosphere: number = 0;
  hydro: number = 0;
  population: number = 0;
  law_level: number = 0;
  government: number = 0;
  tech_level: number = 0;

  facilities: Facility[] = [];
  satellites: World[] = [];
  trade_classes: TradeClass[] = [];

  astro_data: AstroData = new AstroData();

  constructor(
    name: string,
    orbit: number,
    star_oribt: number,
    size: number,
    atmosphere: number,
    hydro: number,
    population: number,
    is_satellite: boolean,
    is_mainworld: boolean
  ) {
    this.name = name;
    this.size = size;
    this.atmosphere = atmosphere;
    this.hydro = hydro;
    this.satellites = [];
    this.orbit = orbit;
    this.star_orbit = star_oribt;
    this.population = population;
    this.is_satellite = is_satellite;
    this.is_mainworld = is_mainworld;
  }

  // Methods common with satellites
  set_subordinate_stats(
    port: PortCode,
    government: number,
    law_level: number,
    tech_level: number,
    facilities: Facility[]
  ) {
    this.port = port;
    this.government = government;
    this.law_level = law_level;
    this.tech_level = tech_level;
    this.facilities = facilities;
  }

  to_upp(): string {
    let size_digit = "0";

    if (
      (this.is_satellite && this.size === -1) ||
      (this.size <= 0 && !this.is_mainworld && !this.is_satellite && !this.name.includes("Planetoid"))
    ) {
      size_digit = "S";
    } else if (this.is_satellite && this.size === 0) {
      size_digit = "R";
    } else if (this.size === 0) {
      size_digit = "0";
    } else {
      size_digit = decimalToHex(this.size);
    }

    return `${PortCode[this.port]}${size_digit}${decimalToHex(
      this.atmosphere
    )}${decimalToHex(this.hydro)}${decimalToHex(this.population)}${decimalToHex(
      this.government
    )}${decimalToHex(this.law_level)}-${decimalToHex(this.tech_level)}`;
  }

  static from_upp(
    name: string,
    upp: string,
    is_satellite: boolean,
    is_mainworld: boolean
  ): World {
    let port = PortCode.Y;
    switch (upp[0]) {
      case "A":
        port = PortCode.A;
        break;
      case "B":
        port = PortCode.B;
        break;
      case "C":
        port = PortCode.C;
        break;
      case "D":
        port = PortCode.D;
        break;
      case "E":
        port = PortCode.E;
        break;
      case "X":
        port = PortCode.X;
        break;
      case "Y":
        port = PortCode.Y;
        break;
      case "H":
        port = PortCode.H;
        break;
      case "G":
        port = PortCode.G;
        break;
      case "F":
        port = PortCode.F;
        break;
    }

    let size = hexToDecimal(upp[1]);
    let atmosphere = hexToDecimal(upp[2]);
    let hydro = hexToDecimal(upp[3]);
    let population = hexToDecimal(upp[4]);
    let government = hexToDecimal(upp[5]);
    let law_level = hexToDecimal(upp[6]);
    if (upp[7] !== "-" && upp[7] !== " ") {
      console.log("Bad spacing character before tech level in " + upp);
    }
    let tech_level = hexToDecimal(upp[8]);

    let world = new World(
      name,
      0,
      0,
      size,
      atmosphere,
      hydro,
      population,
      is_satellite,
      is_mainworld
    );
    world.set_subordinate_stats(port, government, law_level, tech_level, []);
    return world;
  }

  get_population(): number {
    return this.population;
  }

  set_facilities(facilities: Facility[]) {
    this.facilities = facilities;
  }

  get_facilities(): Facility[] {
    return this.facilities;
  }

  facilities_string(): string {
    return this.facilities.map((x) => Facility[x]).join(", ");
  }

  trade_classes_string(): string {
    return this.trade_classes.map((x) => TradeClass[x]).join(", ");
  }

  compute_astro_data(system: System) {
    // TODO: This is not really correct as we need to take into account all stars in the system
    this.astro_data.compute(system, this);
    console.log(`${this.name} has astro data: ${JSON.stringify(this.astro_data)}`);
  }

  // Methods that have to be in common with Gas Giants for satellite generation
  get_orbit(): number {
    return this.orbit;
  }

  num_satellites(): number {
    return this.satellites.length;
  }
}

export enum GasGiantSize {
  Small,
  Large,
}

export class GasGiant {
  name: string;
  size: GasGiantSize = GasGiantSize.Small;
  satellites: World[] = [];
  orbit: number = 0;

  constructor(name: string, size: GasGiantSize, orbit: number) {
    this.name = name;
    this.size = size;
    this.satellites = [];
    this.orbit = orbit;
  }

  // Methods that have to be in common with World for satellite generation
  get_orbit(): number {
    return this.orbit;
  }

  num_satellites(): number {
    return this.satellites.length;
  }
}

export class Empty {
  orbit: number = 0;

  constructor(orbit: number) {
    this.orbit = orbit;
  }
}

class Star {
  star_type: StarType = StarType.O;
  subtype: number = 0;
  size: StarSize = StarSize.Ia;
}

type ZoneLimits = {
  inside: number;
  hot: number;
  inner: number;
  habitable: number;
  outer: number;
};

type ZoneTable = {
  [key in StarType]: {
    [key in 0 | 5]: ZoneLimits;
  };
};

function genNumStars(): number {
  let roll = roll2D();
  if (roll <= 7) {
    return 1;
  } else if (roll < 12) {
    return 2;
  } else {
    return 3;
  }
}

function genPrimaryStarType(roll: number): StarType {
  if (roll <= 1) {
    return StarType.B;
  } else if (roll === 2) {
    return StarType.A;
  } else if (roll <= 7) {
    return StarType.M;
  } else if (roll === 8) {
    return StarType.K;
  } else if (roll === 9) {
    return StarType.G;
  } else {
    return StarType.F;
  }
}

function genPrimaryStarSize(
  roll: number,
  star_type: StarType,
  subType: number
): StarSize {
  let star_size = StarSize.Ia;

  if (roll <= 4) {
    star_size = roll as StarSize;
  } else if (roll <= 10) {
    star_size = StarSize.V;
  } else if (roll === 11) {
    star_size = StarSize.VI;
  } else {
    star_size = StarSize.D;
  }

  let in_k5_to_m9 =
    ((star_type === StarType.K && subType >= 5) || star_type > StarType.K) &&
    star_type <= StarType.M;
  let in_b0_to_f4 =
    star_type < StarType.F || (star_type === StarType.F && subType <= 4);

  if (star_size === StarSize.IV && in_k5_to_m9) {
    star_size = StarSize.V;
  }

  if (star_size === StarSize.VI && in_b0_to_f4) {
    star_size = StarSize.V;
  }

  return star_size;
}

function genCompanionStarType(roll: number): StarType {
  if (roll <= 1) {
    return StarType.B;
  } else if (roll === 2) {
    return StarType.A;
  } else if (roll <= 4) {
    return StarType.F;
  } else if (roll <= 6) {
    return StarType.G;
  } else if (roll <= 8) {
    return StarType.K;
  } else {
    return StarType.M;
  }
}

function genCompanionStarSize(roll: number): StarSize {
  if (roll <= 4) {
    return roll as StarSize;
  } else if (roll <= 6) {
    return StarSize.D;
  } else if (roll <= 8) {
    return StarSize.V;
  } else if (roll === 9) {
    return StarSize.VI;
  } else {
    return StarSize.D;
  }
}

function genCompanionOrbit(roll: number): number {
  if (roll <= 3) {
    return -1;
  } else if (roll <= 6) {
    return roll - 3;
  } else if (roll <= 11) {
    return roll - 3 + roll1D();
  } else {
    return FAR_ORBIT;
  }
}

function genMaxOrbits(star: Star): number {
  let mod = 0;
  if (star.size <= StarSize.II) {
    mod += 8;
  } else if (star.size === StarSize.III) {
    mod += 4;
  }

  if (star.star_type === StarType.M) {
    mod -= 4;
  } else if (star.star_type === StarType.K) {
    mod -= 2;
  }

  let orbits = roll2D() + mod;
  if (orbits < 0) {
    orbits = 0;
  }

  return orbits;
}

// orbit_mod is the modifier for the orbit roll for this star's orbit position.
// Can be modified by +4 if its the tertiary star.
// Modified by -4 if a secondary star or tertiary of a secondary star or tertiary.
function genCompanionSystem(
  primary_type_roll: number,
  primary_size_roll: number,
  orbit: number
): System {
  let companion_type_roll = roll2D() + primary_type_roll;
  let companion_size_roll = roll2D() + primary_size_roll;
  let companion = new System(
    genCompanionStarType(companion_type_roll),
    roll10(),
    genCompanionStarSize(companion_size_roll),
    orbit,
    0
  );

  companion.setMaxOrbits(Math.min(Math.floor(orbit / 2), genMaxOrbits(companion.star)));

  // If secondary is Far then may need to generate companions.
  if (companion.orbit === FAR_ORBIT) {
    // If secondary is Far then it can have companions.
    let num_stars = genNumStars();
    if (num_stars > 1) {
      // -4 to this as we're a secondary of a secondary.
      let orbit = genCompanionOrbit(roll2D() - 4);
      companion.secondary = genCompanionSystem(
        companion_type_roll,
        companion_size_roll,
        orbit
      );
      companion.orbits[orbit] = companion.secondary;

      if (orbit === FAR_ORBIT) {
        companion.secondary.setMaxOrbits(
          genMaxOrbits(companion.secondary.star)
        );
      } else {
        companion.secondary.setMaxOrbits(
          Math.min(
            Math.floor(orbit / 2),
            genMaxOrbits(companion.secondary.star)
          )
        );
      }
    }
  }
  return companion;
}

function emptyOrbitsNearCompanion(system: System, orbit: number) {
  for (let i = Math.floor(orbit / 2) + 1; i < orbit; i++) {
    system.orbits[i] = new Empty(i);
  }

  system.orbits[orbit + 1] = new Empty(orbit + 1);
  system.orbits[orbit + 2] = new Empty(orbit + 2);
}

function genEmptyOrbits(system: System) {
  if (roll1D() < 5) {
    // No Empty orbits
    return;
  }

  let roll = roll1D();
  let num_empty = 0;
  if (roll <= 2) {
    num_empty = 1;
  } else if (roll === 3) {
    num_empty = 2;
  } else {
    num_empty = 3;
  }

  let valid_orbits = system.orbits
    .map((body, index) => {
      if (body === null) {
        return index;
      } else {
        return -1;
      }
    })
    .filter((x) => x >= 0);

  while (valid_orbits.length > 0 && num_empty > 0) {
    let pos = Math.floor(Math.random() * valid_orbits.length);
    let orbit = valid_orbits[pos];
    system.orbits[orbit] = new Empty(orbit);
    valid_orbits.splice(pos, 1);
    num_empty--;
  }
}

// companions_possible is a modifier to reduce possible orbits.  It should be -1, 0 or 4.  If it is -1 it means no companions possible.
function genStars(
  world_mod: number,
  companions_possible: boolean,
): System {
  let num_stars = 1;
  if (companions_possible) {
    num_stars = genNumStars();
  }

  let primary_type_roll = roll2D();
  let primary_size_roll = roll2D();

  let star_type = genPrimaryStarType(primary_type_roll + world_mod);
  let star_subtype = roll10();
  
  // EDITORIAL: Take out world mod for size of star, to shift away from size D a bit.
  let star_size = genPrimaryStarSize(
    primary_size_roll,
    star_type,
    star_subtype
  );

  let system = new System(star_type, star_subtype, star_size, PRIMARY_ORBIT, 0);

  system.setMaxOrbits(genMaxOrbits(system.star));

  if (num_stars > 1) {
    let orbit = genCompanionOrbit(roll2D());
    if (orbit <= getZone(system).inside) {
      orbit = 0;
    }
    system.secondary = genCompanionSystem(
      primary_type_roll,
      primary_size_roll,
      orbit
    );

    if (orbit >= 0) {
      system.orbits[orbit] = system.secondary;
    }
    emptyOrbitsNearCompanion(system, orbit);
  }

  if (num_stars === 3) {
    let orbit = genCompanionOrbit(roll2D() + 4);
    if (orbit <= getZone(system).inside) {
      orbit = 0;
    }

    system.tertiary = genCompanionSystem(
      primary_type_roll,
      primary_size_roll,
      orbit
    );

    if (orbit >= 0) {
      system.orbits[orbit] = system.tertiary;
    }
    emptyOrbitsNearCompanion(system, orbit);
  }

  return system;
}

function countOpenOrbits(system: System) {
  let acc = 0;

  for (let i = 1; i <= system.max_orbits; i++) {
    if (system.orbits[i] === null) {
      acc++;
    }
  }

  return acc;
}

function genGasGiants(system: System): number {
  if (roll2D() >= 10) {
    // No gas giant in system
    return 0;
  }

  let num_giants = 1;
  let roll = roll2D();
  if (roll <= 3) {
    num_giants = 1;
  } else if (roll <= 5) {
    num_giants = 2;
  } else if (roll <= 7) {
    num_giants = 3;
  } else if (roll <= 10) {
    num_giants = 4;
  } else {
    num_giants = 5;
  }

  // Cannot have more gas giants than open systems
  num_giants = Math.min(num_giants, countOpenOrbits(system));
  let original_num_giants = num_giants;

  let habitable = getZone(system).habitable;

  let viable_outer_orbits = system.orbits
    .map((body, index) => {
      if (body === null && index >= habitable) {
        return index;
      } else {
        return -1;
      }
    })
    .filter((x) => x >= 0);

  let viable_inner_orbits = system.orbits
    .map((body, index) => {
      if (body === null && index < habitable) {
        return index;
      } else {
        return -1;
      }
    })
    .filter((x) => x >= 0);

  while (
    viable_outer_orbits.length + viable_inner_orbits.length > 0 &&
    num_giants > 0
  ) {
    let orbit = 0;
    // Give priority to the outer orbits.
    if (viable_outer_orbits.length > 0) {
      let pos = Math.floor(Math.random() * viable_outer_orbits.length);
      orbit = viable_outer_orbits[pos];
      viable_outer_orbits.splice(pos, 1);
    } else {
      let pos = Math.floor(Math.random() * viable_inner_orbits.length);
      orbit = viable_inner_orbits[pos];
      viable_inner_orbits.splice(pos, 1);
    }

    if (roll1D() <= 3) {
      system.orbits[orbit] = new GasGiant(
        genPlanetName(system, orbit),
        GasGiantSize.Small,
        orbit
      );
    } else {
      system.orbits[orbit] = new GasGiant(
        genPlanetName(system, orbit),
        GasGiantSize.Large,
        orbit
      );
    }
    num_giants--;
  }

  if (num_giants > 0) {
    console.error(
      "Not enough orbits for gas giants. Need " +
        original_num_giants +
        " in system " +
        JSON.stringify(system) +
        "."
    );
  }

  // Return the number of gas giants actually placed.
  return original_num_giants - num_giants;
}

function genPlanetoids(system: System, num_giants: number, main_world: World) {
  if (roll2D() >= 7) {
    // No planetoids in system
    return;
  }
  let num_planetoids = 1;
  let roll = roll2D() - num_giants;
  if (roll <= 0) {
    num_planetoids = 3;
  } else if (roll <= 6) {
    num_planetoids = 2;
  } else {
    num_planetoids = 1;
  }

  // Here we can use a functional approach as we are finding non-empty orbits
  let viable_giants = system.orbits
    .map((body, index) => {
      if (body instanceof GasGiant && system.orbits[index - 1] === null) {
        return index - 1;
      } else {
        return -1;
      }
    })
    .filter((x) => x >= 0);

  let viable_other_orbits = system.orbits
    .map((body, index) => {
      if (body === null && !viable_giants.includes(index)) {
        return index;
      } else {
        return -1;
      }
    })
    .filter((x) => x >= 0);

  while (
    viable_giants.length + viable_other_orbits.length > 0 &&
    num_planetoids > 0
  ) {
    let pos = 0;
    let orbit = 0;
    if (viable_giants.length > 0) {
      pos = Math.floor(Math.random() * viable_giants.length);
      orbit = viable_giants[pos];
      viable_giants.splice(pos, 1);
    } else {
      pos = Math.floor(Math.random() * viable_other_orbits.length);
      orbit = viable_other_orbits[pos];
      viable_other_orbits.splice(pos, 1);
    }
    let population =
      roll2D() -
      2 +
      (orbit <= getZone(system).inner ? -5 : 0) +
      (orbit > getZone(system).habitable ? -5 : 0);

    if (population < 0) {
      population = 0;
    }

    if (population >= main_world.population) {
      population = main_world.population - 1;
    }

    let planetoid = new World(
      "Planetoid Belt",
      orbit,
      orbit,
      0,
      0,
      0,
      population,
      false,
      false
    );
    genSubordinateStats(planetoid, main_world);
    genSubordinateFacilities(system, planetoid, orbit, main_world);
    system.orbits[orbit] = planetoid;
    num_planetoids--;
  }
}

function placeMainWorld(system: System, world: World) {
  system.main_world = world;

  let requires_habitable =
    world.atmosphere > 1 && world.atmosphere < 10 && world.size > 0;

  let habitable = getZone(system).habitable;

  // Error check if there is no habitable zone and requires_habitable
  if (
    (habitable === -1 || habitable === getZone(system).inner) &&
    requires_habitable
  ) {
    console.warn(
      "No habitable zone for main world for system: " +
        JSON.stringify(system) +
        ". Habitable = " +
        habitable +
        ". Inner = " +
        getZone(system).inner +
        ". Using orbit 0."
    );
    habitable = Math.max(getZone(system).inner, 0);
  }

  if (requires_habitable) {
    // Just place in the habitable
    let body = system.orbits[habitable];
    if (body instanceof System) {
      // If there happens to be a star in the habitable zone, place it in orbit there.
      // Note the orbit of the main world in terms of primary first
      // TODO: Is this correct when we have multiple stars?
      world.star_orbit = habitable;
      placeMainWorld(body, world);
    } else if (body instanceof GasGiant) {
      world.orbit = genSatelliteOrbit(body, world.size === 0);
      world.star_orbit = habitable;
      body.satellites.push(world);
    } else {
      // Just overwrite whatever is there in this case.
      system.orbits[habitable] = world;
      world.star_orbit = habitable;
      world.orbit = habitable;
    }
  } else {
    // Otherwise just find an empty orbit and put it there.
    let empty_orbits = system.orbits
      .map((body, index) => {
        if (body === null) {
          return index;
        } else {
          return -1;
        }
      })
      .filter((x) => x > 0);

    if (empty_orbits.length > 0) {
      let pos = Math.floor(Math.random() * empty_orbits.length);
      let orbit = empty_orbits[pos];
      system.orbits[orbit] = world;
      world.orbit = orbit;
    } else {
      // Just jam the world in somewhere.
      let pos = Math.floor(Math.random() * system.max_orbits);
      system.orbits[pos] = world;
      world.orbit = pos;
    }
  }
}

function genWorld(
  name: string,
  system: System,
  orbit: number,
  main_world: World
): World {
  let mod = 0;
  if (orbit === 0) {
    mod = -5;
  } else if (orbit === 1) {
    mod = -4;
  } else if (orbit === 2) {
    mod = -2;
  }
  if (system.star.star_type === StarType.M) {
    mod -= 2;
  }

  let size = roll2D() - 2 + mod;
  if (size < 0) {
    size = 0;
  }

  let atmosphere = 0;
  let roll = roll2D();
  atmosphere =
    roll2D() -
    7 +
    size +
    (orbit <= getZone(system).inner ? -2 : 0) +
    (orbit > getZone(system).habitable ? -2 : 0);

  // Special case where atmosphere is 1 or less.
  if (atmosphere <= 0) {
    atmosphere = 0;
  }

  // Special case for a type A atmosphere.
  if (roll === 12 && orbit > getZone(system).habitable) {
    atmosphere = 10;
  }

  if (atmosphere >= 10) {
    atmosphere = 10;
  }

  let hydro =
    roll2D() -
    7 +
    size +
    (orbit > getZone(system).habitable ? -4 : 0) +
    (atmosphere <= 1 || atmosphere >= 10 ? -4 : 0);
  if (size <= 0 || orbit <= getZone(system).inner) {
    hydro = 0;
  }

  if (hydro < 0) {
    hydro = 0;
  }

  if (hydro >= 10) {
    hydro = 10;
  }

  let population =
    roll2D() -
    2 +
    (orbit <= getZone(system).inner ? -5 : 0) +
    (orbit > getZone(system).habitable ? -5 : 0) +
    (![0, 5, 6, 8].includes(atmosphere) ? -2 : 0);

  if (population < 0) {
    population = 0;
  }

  if (population >= main_world.population) {
    population = main_world.population - 1;
  }

  // Give a real name to populated planets.
  if (population > 0) {
    name = planetNames[Math.floor(Math.random() * planetNames.length)];
  }

  let world = new World(
    name,
    orbit,
    orbit,
    size,
    atmosphere,
    hydro,
    population,
    false,
    false
  );
  genSubordinateStats(world, main_world);
  genSubordinateFacilities(system, world, orbit, main_world);

  return world;
}

function numSatellites(system: System, primary: World | GasGiant) {
  if (primary instanceof World && primary.size <= 0) {
    return 0;
  } else if (primary instanceof World) {
    return Math.max(0,roll1D() - 3);
  } else if (
    primary instanceof GasGiant &&
    primary.size === GasGiantSize.Small
  ) {
    return Math.max(0,roll2D() - 4);
  } else {
    return roll2D();
  }
}

function clean_satellites(world: World | GasGiant) {
  world.satellites.sort((a, b) => {
    return a.orbit - b.orbit;
  });
  // Clean up rings.
  let ring_indices = world.satellites
    .map((satellite, index) => {
      if (satellite.size === 0) {
        return index;
      } else {
        return -1;
      }
    })
    .filter((x) => x >= 0).sort((a, b) => a - b);
  if (ring_indices.length > 0) {
    for (let i = 1; i < ring_indices.length; i++) {
      world.satellites.splice(ring_indices[i], 1);
    }
    world.satellites[ring_indices[0]].name = "Ring System";
  }
}

function genSatelliteOrbit(primary: World | GasGiant, is_ring: boolean) {
  let orbit = 0;
  if (is_ring) {
    switch (roll1D()) {
      case 1:
      case 2:
      case 3:
        orbit = 1;
        break;
      case 4:
      case 5:
        orbit = 2;
        break;
      case 6:
        orbit = 3;
        break;
    }
  } else {
    let orbit_type_roll = roll2D();
    let mod = -primary.num_satellites();
    orbit_type_roll += mod;
    if (orbit_type_roll <= 7) {
      // Close orbit
      orbit = roll12() + 3;
    } else if (orbit_type_roll === 12 && primary instanceof GasGiant) {
      // Extreme orbit (only for Gas Giants)
      orbit = (roll12() + 3) * 25;
    } else {
      // Far orbit
      orbit = (roll12() + 3) * 5;
    }
  }
  const duplicate_orbit = (orbit: number) => (satellite: World) =>
    satellite.orbit === orbit;

  while (primary.satellites.some(duplicate_orbit(orbit))) {
    orbit += 1;
  }
  return orbit;
}

function genSatellite(
  system: System,
  primary: World | GasGiant,
  main_world: World
) {
  let size = 0;
  if (primary instanceof World) {
    size = primary.size - roll1D();
  } else if (
    primary instanceof GasGiant &&
    primary.size === GasGiantSize.Small
  ) {
    size = roll2D() - 6;
  } else {
    size = roll2D() - 4;
  }

  // Anything less than 0 is size S; make them all -1 to keep it
  // straightforward.
  if (size < 0) {
    size = -1;
  }


  let orbit = genSatelliteOrbit(primary, size === 0);

  // Size 0 is a ring so nothing else can be 0.
  if (size === 0) {
    let ring = new World("Ring System", orbit, primary.orbit,0, 0, 0, 0, true, false);  
    ring.port = PortCode.Y;
    primary.satellites.push(ring);
    return;
  }
  
  let atmosphere = 0;
  let roll = roll2D();
  atmosphere =
    roll2D() -
    7 +
    size +
    (primary.get_orbit() <= getZone(system).inner ? -4 : 0) +
    (primary.get_orbit() > getZone(system).habitable ? -4 : 0);

  // Special case where atmosphere is 1 or less.
  if (atmosphere <= 1) {
    atmosphere = 0;
  }

  // Special case for a type A atmosphere.
  if (roll === 12 && primary.get_orbit() > getZone(system).habitable) {
    atmosphere = 10;
  }

  let hydro =
    roll2D() -
    7 +
    size +
    (primary.get_orbit() > getZone(system).habitable ? -4 : 0) +
    (atmosphere <= 1 || atmosphere >= 10 ? -4 : 0);

  if (hydro < 0) {
    hydro = 0;
  }

  if (size <= 0 || primary.get_orbit() <= getZone(system).inner) {
    hydro = 0;
  }

  let population =
    roll2D() -
    2 +
    (primary.get_orbit() <= getZone(system).inner ? -5 : 0) +
    (primary.get_orbit() > getZone(system).habitable ? -4 : 0) +
    (![5, 6, 8].includes(atmosphere) ? -2 : 0);

  if (population < 0) {
    population = 0;
  }

  let satellite_name = moonNames[Math.floor(Math.random() * moonNames.length)];
  let satellite = new World(
    satellite_name,
    orbit,
    primary.orbit,
    size,
    atmosphere,
    hydro,
    population,
    true,
    false
  );
  genSubordinateStats(satellite, main_world);
  genSubordinateFacilities(system, satellite, orbit, main_world);
  satellite.compute_astro_data(system);
  primary.satellites.push(satellite);
}

/*
 * Add subordinate facilities.
 *
 * system is the System this body is in.
 * world is the body itself that we are adding these to.
 * orbit is the orbit in the system that this body is in. Not necessarily the orbit of this body itself,
 * i.e. in the case of a satellite its orbit around the primary.
 * main_world is the main world of the system.
 */
function genSubordinateFacilities(
  system: System,
  world: World,
  orbit: number,
  main_world: World
) {
  // Mining?
  if (
    main_world.trade_classes.includes(TradeClass.Industrial) &&
    world.population >= 2
  ) {
    world.facilities.push(Facility.Mining);
  }
  // Farming?
  if (
    orbit === getZone(system).habitable &&
    orbit > getZone(system).inner &&
    world.atmosphere >= 4 &&
    world.atmosphere <= 9 &&
    world.hydro >= 4 &&
    world.hydro <= 8 &&
    world.population >= 2
  ) {
    world.facilities.push(Facility.Farming);
  }
  // Colony?
  if (world.government === 6 && world.get_population() >= 5) {
    world.facilities.push(Facility.Colony);
  }

  // Research Lab?
  // EDITORIAL: reduce likelihood of pop is 0.
  if (
    main_world.population > 0 &&
    main_world.tech_level > 8 &&
    roll2D() + (main_world.tech_level >= 10 ? 2 : 0) 
    + (world.population === 0 ? -2: 0) >= 12
  ) {
    world.facilities.push(Facility.Lab);
    // Fix tech level if there is a lab.  Not ideal but we need to gen most of a world/satellite
    // before facilities, but tech level is impacted by having a lab.
    if (world.tech_level === main_world.tech_level -1) {
      world.tech_level = main_world.tech_level;
    }
  }

  // Military Base?
  let mod =
    (main_world.get_population() >= 8 ? 1 : 0) +
    (main_world.atmosphere === world.atmosphere ? 2 : 0) +
    (main_world.facilities.includes(Facility.Naval) ||
    main_world.facilities.includes(Facility.Scout)
      ? 1
      : 0);

  let roll = roll2D();
  if (!main_world.trade_classes.includes(TradeClass.Poor) && (world.population > 0) && roll + mod >= 12) {
    world.facilities.push(Facility.Military);
  }
}

function genSubordinateStats(world: World, main_world: World) {
  let population = world.get_population();
  let government = 0;
  let mod = 0;
  if (main_world.government === 6) {
    mod = population;
  } else if (main_world.government >= 7) {
    mod = 1;
  }

  switch (roll1D() + mod) {
    case 1:
      government = 0;
      break;
    case 2:
      government = 1;
      break;
    case 3:
      government = 2;
      break;
    case 4:
      government = 3;
      break;
    default:
      government = 6;
      break;
  }

  if (population <= 0) {
    government = 0;
  }

  let law_level = roll1D() - 3 + main_world.law_level;
  if (law_level <= 0 || population <= 0) {
    law_level = 0;
  }

  let tech_level = main_world.tech_level - 1;
  if (tech_level <= 0 || population <= 0) {
    tech_level = 0;
  }

  if (
    population > 0 &&
    ![5, 6, 8].includes(world.atmosphere) &&
    tech_level < 7
  ) {
    tech_level = 7;
  }

  let port = PortCode.Y;
  mod = 0;
  switch (population) {
    case 0:
      mod = -3;
      break;
    case 1:
      mod = -2;
      break;
    case 2:
    case 3:
    case 4:
    case 5:
      mod = 0;
      break;
    default:
      mod = 2;
      break;
  }

  let roll = roll1D() + mod;
  if (roll <= 2) {
    port = PortCode.Y;
  } else if (roll === 3) {
    port = PortCode.H;
  } else if (roll <= 5) {
    port = PortCode.G;
  } else {
    port = PortCode.F;
  }

  world.set_subordinate_stats(port, government, law_level, tech_level, []);
}

function genPlanetName(system: System, orbit: number): string {
  return `${system.name} ${arabicToRoman(orbit + 1)}`;
}

function genTradeClasses(world: World) {
  if (
    world.atmosphere >= 4 &&
    world.atmosphere <= 9 &&
    world.hydro >= 4 &&
    world.hydro <= 8 &&
    world.population >= 5 &&
    world.population <= 7
  ) {
    world.trade_classes.push(TradeClass.Agricultural);
  }

  if (world.atmosphere <= 3 && world.hydro <= 3 && world.population >= 6) {
    world.trade_classes.push(TradeClass.NonAgricultural);
  }

  if ([0, 1, 2, 4, 7, 9].includes(world.atmosphere) && world.population >= 9) {
    world.trade_classes.push(TradeClass.Industrial);
  }

  if (world.population > 0 && world.population <= 6) {
    world.trade_classes.push(TradeClass.NonIndustrial);
  }

  if (
    [6, 8].includes(world.atmosphere) &&
    [6, 7, 8].includes(world.population) &&
    world.government >= 4 &&
    world.government <= 9
  ) {
    world.trade_classes.push(TradeClass.Rich);
  }

  if (
    world.population >= 0 &&
    world.atmosphere >= 2 &&
    world.atmosphere <= 5 &&
    world.hydro <= 3
  ) {
    world.trade_classes.push(TradeClass.Poor);
  }

  if (world.hydro >= 10) {
    world.trade_classes.push(TradeClass.WaterWorld);
  }
  if (world.hydro <= 0 && world.atmosphere > 1) {
    world.trade_classes.push(TradeClass.DesertWorld);
  }
  /*
  if (world.atmosphere <= 0) {
    world.trade_classes.push(TradeClass.VacuumWorld);
  }
  */
  if (world.atmosphere <= 1 && world.hydro >= 10) {
    world.trade_classes.push(TradeClass.Icecapped);
  }
}
export function generateSystem(main_world: World): System {
  let star_mod =
    (main_world.atmosphere >= 4 && main_world.atmosphere <= 9) ||
    main_world.population >= 8
      ? 4
      : 0;
  let system = genStars(star_mod, true);
  genTradeClasses(main_world);

  fillSystem(system, main_world, true);
  main_world.compute_astro_data(system);
  return system;
}

function fillSystem(system: System, main_world: World, is_primary: boolean) {
  genEmptyOrbits(system);
  let num_gas_giants = genGasGiants(system);
  genPlanetoids(system, num_gas_giants, main_world);
  if (is_primary) {
    placeMainWorld(system, main_world);
  }

  for (let i = 0; i <= getZone(system).hot; i++) {
    system.orbits[i] = new Empty(i);
  }
  for (let i = getZone(system).hot + 1; i <= system.max_orbits; i++) {
    if (system.orbits[i] === null) {
      let name = genPlanetName(system, i);
      let new_world = genWorld(name, system, i, main_world);
      new_world.compute_astro_data(system);
      system.orbits[i] = new_world;
    }
  }

  for (let i = 1; i <= system.max_orbits; i++) {
    let body = system.orbits[i];
    if (body instanceof World || body instanceof GasGiant) {
      let num_satellites = numSatellites(system, body);
      for (let j = 0; j < num_satellites; j++) {
        genSatellite(system, body, main_world);
      }
      clean_satellites(body);
    }
  }

  if (system.secondary !== null && system.secondary.orbit > 0) {
    fillSystem(system.secondary, main_world, false);
  }
  if (system.tertiary !== null && system.tertiary.orbit > 0) {
    fillSystem(system.tertiary, main_world, false);
  }
}

export function arabicToRoman(num: number): string {
  if (num < 0 || num > 20 || !Number.isInteger(num)) {
    throw new Error("Input must be an integer between 0 and 20");
  }

  const romanNumerals: [number, string][] = [
    [20, "XX"],
    [19, "XIX"],
    [18, "XVIII"],
    [17, "XVII"],
    [16, "XVI"],
    [15, "XV"],
    [14, "XIV"],
    [13, "XIII"],
    [12, "XII"],
    [11, "XI"],
    [10, "X"],
    [9, "IX"],
    [8, "VIII"],
    [7, "VII"],
    [6, "VI"],
    [5, "V"],
    [4, "IV"],
    [3, "III"],
    [2, "II"],
    [1, "I"],
    [0, "N"],
  ];

  for (const [value, symbol] of romanNumerals) {
    if (num >= value) {
      return symbol;
    }
  }

  return ""; // This should never be reached given the input constraints
}

function decimalToHex(decimal: number): string {
  const hexDigit = "0123456789ABCDEF";
  if (decimal < 0 || decimal > 15 || !Number.isInteger(decimal)) {
    throw new Error(
      "Input must be an integer between 0 and 15, not " + decimal
    );
  }
  return hexDigit[decimal];
}

function hexToDecimal(hex: string): number {
  if (hex.length !== 1) {
    throw new Error("Input must be a single character");
  }

  const hexDigit = hex.toLowerCase();
  const hexValues: { [key: string]: number } = {
    "0": 0,
    "1": 1,
    "2": 2,
    "3": 3,
    "4": 4,
    "5": 5,
    "6": 6,
    "7": 7,
    "8": 8,
    "9": 9,
    a: 10,
    b: 11,
    c: 12,
    d: 13,
    e: 14,
    f: 15,
  };

  if (hexDigit in hexValues) {
    return hexValues[hexDigit];
  } else {
    throw new Error(`Invalid hexadecimal digit '${hexDigit}'`);
  }
}

// Random number generators
function roll2D(): number {
  // Generate a random roll of two 6-sided dies
  return Math.floor(Math.random() * 6) + Math.floor(Math.random() * 6) + 2;
}

function roll1D(): number {
  // Generate a random roll of one 6-sided die
  return Math.floor(Math.random() * 6) + 1;
}

function roll10(): number {
  // Generate a number between 0 and 9
  return Math.floor(Math.random() * 10);
}

function roll12(): number {
  // Generate a number between 0 and 11
  return Math.floor(Math.random() * 12);
}

// ZoneTable and support for them
export function roundSubType(subtype: number): 0 | 5 {
  switch (Math.floor(subtype / 5) * 5) {
    case 0:
      return 0;
    case 5:
      return 5;
    default:
      throw new Error("Invalid subtype");
  }
}

function getZone(system: System): ZoneLimits {
  return zoneTables[system.star.size][system.star.star_type][
    roundSubType(system.star.subtype)
  ];
}

export function getHabitable(system: System): number {
  let habitable = getZone(system).habitable;
  if (habitable > getZone(system).inner) {
    return habitable;
  } else {
    return -1;
  }
}

export function getCloudiness(atmosphere: number): number {
  return cloudiness[atmosphere];
}

export function getGreenhouse(atmosphere: number): number {
  return greenhouse[atmosphere];
}

export function getOrbitalDistance(orbit: number): number {
  return orbitalDistance[orbit];
}

export function getSolarMass(system: System): number {
  return massTables[system.star.star_type][roundSubType(system.star.subtype)][
    system.star.size
  ];
}

export function getLuminosity(system: System): number {
  return luminosityTables[system.star.star_type][roundSubType(system.star.subtype)][
    system.star.size
  ];
}

const zoneTables: { [key in StarSize]: ZoneTable } = {
  [StarSize.Ia]: {
    [StarType.O]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.B]: {
      0: { inside: 0, hot: 7, inner: 12, habitable: 13, outer: 14 },
      5: { inside: 0, hot: 7, inner: 12, habitable: 13, outer: 14 },
    },
    [StarType.A]: {
      0: { inside: 1, hot: 6, inner: 11, habitable: 12, outer: 14 },
      5: { inside: 1, hot: 6, inner: 11, habitable: 12, outer: 14 },
    },
    [StarType.F]: {
      0: { inside: 2, hot: 5, inner: 11, habitable: 12, outer: 14 },
      5: { inside: 2, hot: 5, inner: 10, habitable: 11, outer: 14 },
    },
    [StarType.G]: {
      0: { inside: 3, hot: 6, inner: 11, habitable: 12, outer: 14 },
      5: { inside: 4, hot: 6, inner: 11, habitable: 12, outer: 14 },
    },
    [StarType.K]: {
      0: { inside: 5, hot: 6, inner: 11, habitable: 12, outer: 14 },
      5: { inside: 5, hot: 6, inner: 11, habitable: 12, outer: 14 },
    },
    [StarType.M]: {
      0: { inside: 6, hot: 6, inner: 11, habitable: 12, outer: 14 },
      5: { inside: 0, hot: 6, inner: 11, habitable: 12, outer: 14 },
    },
  },
  [StarSize.Ib]: {
    [StarType.O]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.B]: {
      0: { inside: 0, hot: 7, inner: 12, habitable: 13, outer: 14 },
      5: { inside: 0, hot: 5, inner: 10, habitable: 11, outer: 14 },
    },
    [StarType.A]: {
      0: { inside: 0, hot: 4, inner: 10, habitable: 11, outer: 14 },
      5: { inside: 0, hot: 4, inner: 9, habitable: 10, outer: 14 },
    },
    [StarType.F]: {
      0: { inside: 0, hot: 4, inner: 9, habitable: 10, outer: 14 },
      5: { inside: 0, hot: 3, inner: 9, habitable: 10, outer: 14 },
    },
    [StarType.G]: {
      0: { inside: 0, hot: 3, inner: 9, habitable: 10, outer: 14 },
      5: { inside: 1, hot: 4, inner: 9, habitable: 10, outer: 14 },
    },
    [StarType.K]: {
      0: { inside: 3, hot: 4, inner: 9, habitable: 10, outer: 14 },
      5: { inside: 4, hot: 5, inner: 10, habitable: 11, outer: 14 },
    },
    [StarType.M]: {
      0: { inside: 5, hot: 5, inner: 10, habitable: 11, outer: 14 },
      5: { inside: 6, hot: 6, inner: 11, habitable: 12, outer: 14 },
    },
  },
  [StarSize.II]: {
    [StarType.O]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.B]: {
      0: { inside: 0, hot: 6, inner: 11, habitable: 12, outer: 13 },
      5: { inside: 0, hot: 4, inner: 10, habitable: 11, outer: 13 },
    },
    [StarType.A]: {
      0: { inside: 0, hot: 2, inner: 8, habitable: 9, outer: 13 },
      5: { inside: 0, hot: 1, inner: 7, habitable: 8, outer: 13 },
    },
    [StarType.F]: {
      0: { inside: 0, hot: 1, inner: 7, habitable: 8, outer: 13 },
      5: { inside: 0, hot: 1, inner: 7, habitable: 8, outer: 13 },
    },
    [StarType.G]: {
      0: { inside: 0, hot: 1, inner: 7, habitable: 8, outer: 13 },
      5: { inside: 0, hot: 1, inner: 7, habitable: 8, outer: 13 },
    },
    [StarType.K]: {
      0: { inside: 0, hot: 1, inner: 8, habitable: 9, outer: 13 },
      5: { inside: 1, hot: 2, inner: 8, habitable: 9, outer: 13 },
    },
    [StarType.M]: {
      0: { inside: 3, hot: 3, inner: 9, habitable: 10, outer: 13 },
      5: { inside: 5, hot: 5, inner: 10, habitable: 11, outer: 13 },
    },
  },
  [StarSize.III]: {
    [StarType.O]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.B]: {
      0: { inside: 0, hot: 6, inner: 11, habitable: 12, outer: 13 },
      5: { inside: 0, hot: 4, inner: 9, habitable: 10, outer: 13 },
    },
    [StarType.A]: {
      0: { inside: 0, hot: 0, inner: 7, habitable: 8, outer: 13 },
      5: { inside: 0, hot: 0, inner: 6, habitable: 7, outer: 13 },
    },
    [StarType.F]: {
      0: { inside: 0, hot: 0, inner: 5, habitable: 6, outer: 13 },
      5: { inside: 0, hot: 0, inner: 5, habitable: 6, outer: 13 },
    },
    [StarType.G]: {
      0: { inside: 0, hot: 0, inner: 5, habitable: 6, outer: 13 },
      5: { inside: 0, hot: 0, inner: 6, habitable: 7, outer: 13 },
    },
    [StarType.K]: {
      0: { inside: 0, hot: 0, inner: 6, habitable: 7, outer: 13 },
      5: { inside: 0, hot: 0, inner: 7, habitable: 8, outer: 13 },
    },
    [StarType.M]: {
      0: { inside: 0, hot: 1, inner: 7, habitable: 8, outer: 13 },
      5: { inside: 3, hot: 3, inner: 8, habitable: 9, outer: 13 },
    },
  },
  [StarSize.IV]: {
    [StarType.O]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.B]: {
      0: { inside: 0, hot: 6, inner: 11, habitable: 12, outer: 13 },
      5: { inside: 0, hot: 2, inner: 8, habitable: 9, outer: 13 },
    },
    [StarType.A]: {
      0: { inside: 0, hot: 0, inner: 6, habitable: 7, outer: 13 },
      5: { inside: -1, hot: -1, inner: 5, habitable: 6, outer: 13 },
    },
    [StarType.F]: {
      0: { inside: -1, hot: -1, inner: 5, habitable: 6, outer: 13 },
      5: { inside: -1, hot: -1, inner: 4, habitable: 5, outer: 13 },
    },
    [StarType.G]: {
      0: { inside: -1, hot: -1, inner: 4, habitable: 5, outer: 13 },
      5: { inside: -1, hot: -1, inner: 4, habitable: 5, outer: 13 },
    },
    [StarType.K]: {
      0: { inside: -1, hot: -1, inner: 3, habitable: 4, outer: 13 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.M]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
  },
  [StarSize.V]: {
    [StarType.O]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.B]: {
      0: { inside: 0, hot: 5, inner: 11, habitable: 12, outer: 14 },
      5: { inside: 0, hot: 2, inner: 8, habitable: 9, outer: 14 },
    },
    [StarType.A]: {
      0: { inside: -1, hot: -1, inner: 6, habitable: 7, outer: 14 },
      5: { inside: -1, hot: -1, inner: 5, habitable: 6, outer: 14 },
    },
    [StarType.F]: {
      0: { inside: -1, hot: -1, inner: 4, habitable: 5, outer: 14 },
      5: { inside: -1, hot: -1, inner: 3, habitable: 4, outer: 14 },
    },
    [StarType.G]: {
      0: { inside: -1, hot: -1, inner: 2, habitable: 3, outer: 14 },
      5: { inside: -1, hot: -1, inner: 2, habitable: 3, outer: 14 },
    },
    [StarType.K]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: 0, outer: 14 },
      5: { inside: -1, hot: -1, inner: -1, habitable: 0, outer: 14 },
    },
    [StarType.M]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 14 },
      5: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 14 },
    },
  },
  [StarSize.VI]: {
    [StarType.O]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.B]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.A]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.F]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: -1, hot: -1, inner: 2, habitable: 3, outer: 4 },
    },
    [StarType.G]: {
      0: { inside: -1, hot: -1, inner: 1, habitable: 2, outer: 4 },
      5: { inside: -1, hot: -1, inner: 0, habitable: 1, outer: 4 },
    },
    [StarType.K]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: 1, outer: 4 },
      5: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
    },
    [StarType.M]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
      5: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
    },
  },
  [StarSize.D]: {
    [StarType.O]: {
      0: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
      5: { inside: 0, hot: 0, inner: 0, habitable: 0, outer: 0 },
    },
    [StarType.B]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: 0, outer: 4 },
      5: { inside: -1, hot: -1, inner: -1, habitable: 0, outer: 4 },
    },
    [StarType.A]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
      5: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
    },
    [StarType.F]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
      5: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
    },
    [StarType.G]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
      5: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
    },
    [StarType.K]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
      5: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
    },
    [StarType.M]: {
      0: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
      5: { inside: -1, hot: -1, inner: -1, habitable: -1, outer: 4 },
    },
  },
};

const orbitalDistance: number[] = [
  29.9, 59.8, 104.7, 149.6, 239.3, 418.9, 777.9, 1495.9, 2932, 5804, 11548,
  23038, 46016, 91972, 183885, 367711, 735363, 1470666, 2941274, 5882488,
];

const cloudiness: number[] = [0, 0, 10, 10, 20, 30, 40, 50, 60, 70, 70];

const greenhouse: number[] = [0, 0, 0, 0, 0.05, 0.05, 0.1, 0.1, 0.15, 0.15, 0.5, 0.5, 0.5, 0.15, 0.10, 0];

const luminosityTables: {
  [key in StarType]: { [key in 0 | 5]: { [key in StarSize]: number } };
} = {
    [StarType.O]: {
        0: {
            [StarSize.Ia]: 0,
            [StarSize.Ib]: 0,
            [StarSize.II]: 0,
            [StarSize.III]: 0,
            [StarSize.IV]: 0,
            [StarSize.V]: 0,
            [StarSize.VI]: 0,
            [StarSize.D]: 0,
        },
        5: {
            [StarSize.Ia]: 0,
            [StarSize.Ib]: 0,
            [StarSize.II]: 0,
            [StarSize.III]: 0,
            [StarSize.IV]: 0,
            [StarSize.V]: 0,
            [StarSize.VI]: 0,
            [StarSize.D]: 0,
        },
    },
    [StarType.B]: {
        0: {
            [StarSize.Ia]: 560_000,
            [StarSize.Ib]: 270_000,
            [StarSize.II]: 170_000,
            [StarSize.III]: 107_000,
            [StarSize.IV]: 81_000,
            [StarSize.V]: 56_000,
            [StarSize.VI]: 0,
            [StarSize.D]: 0.46,
        },
        5: {
            [StarSize.Ia]: 204_000,
            [StarSize.Ib]: 46_700,
            [StarSize.II]: 18_600,
            [StarSize.III]: 6_700,
            [StarSize.IV]: 2_000,
            [StarSize.V]: 1_400,
            [StarSize.VI]: 0,
            [StarSize.D]: 0.46,
        },
    },
    [StarType.A]: {
        0: {
            [StarSize.Ia]: 107_000,
            [StarSize.Ib]: 15_000,
            [StarSize.II]: 2_200,
            [StarSize.III]: 280,
            [StarSize.IV]: 156,
            [StarSize.V]: 90,
            [StarSize.VI]: 0,
            [StarSize.D]: 0.005,
        },
        5: {
            [StarSize.Ia]: 81_000,
            [StarSize.Ib]: 11_700,
            [StarSize.II]: 850,
            [StarSize.III]: 90,
            [StarSize.IV]: 37,
            [StarSize.V]: 16,
            [StarSize.VI]: 0,
            [StarSize.D]: 0.005,
        },
    },
    [StarType.F]: {
        0: {
            [StarSize.Ia]: 61_000,
            [StarSize.Ib]: 7_400,
            [StarSize.II]: 600,
            [StarSize.III]: 53,
            [StarSize.IV]: 19,
            [StarSize.V]: 8.1,
            [StarSize.VI]: 0,
            [StarSize.D]: 0.0003,
        },
        5: {
            [StarSize.Ia]: 51_000,
            [StarSize.Ib]: 5_100,
            [StarSize.II]: 510,
            [StarSize.III]: 43,
            [StarSize.IV]: 12,
            [StarSize.V]: 3.5,
            [StarSize.VI]: 0.977,
            [StarSize.D]: 0.0003,
        },
    },
    [StarType.G]: {
        0: {
            [StarSize.Ia]: 67_000,
            [StarSize.Ib]: 6_100,
            [StarSize.II]: 560,
            [StarSize.III]: 50,
            [StarSize.IV]: 6.5,
            [StarSize.V]: 1.21,
            [StarSize.VI]: 0.322,
            [StarSize.D]: 0.00006,
        },
        5: {
            [StarSize.Ia]: 89_000,
            [StarSize.Ib]: 8_100,
            [StarSize.II]: 740,
            [StarSize.III]: 75,
            [StarSize.IV]: 4.9,
            [StarSize.V]: 0.67,
            [StarSize.VI]: 0.186,
            [StarSize.D]: 0.00006,
        },
    },
    [StarType.K]: {
        0: {
            [StarSize.Ia]: 97_000,
            [StarSize.Ib]: 11_700,
            [StarSize.II]: 890,
            [StarSize.III]: 95,
            [StarSize.IV]: 4.67,
            [StarSize.V]: 0.42,
            [StarSize.VI]: 0.117,
            [StarSize.D]: 0.00004,
        },
        5: {
            [StarSize.Ia]: 107_000,
            [StarSize.Ib]: 20_400,
            [StarSize.II]: 2_450,
            [StarSize.III]: 320,
            [StarSize.IV]: 0,
            [StarSize.V]: 0.08,
            [StarSize.VI]: 0.025,
            [StarSize.D]: 0.00004,
        },
    },
    [StarType.M]: {
        0: {
            [StarSize.Ia]: 117_000,
            [StarSize.Ib]: 46_000,
            [StarSize.II]: 4_600,
            [StarSize.III]: 470,
            [StarSize.IV]: 0,
            [StarSize.V]: 0.04,
            [StarSize.VI]: 0.011,
            [StarSize.D]: 0.00003,
        },
        5: {
            [StarSize.Ia]: 129_000,
            [StarSize.Ib]: 89_000,
            [StarSize.II]: 14_900,
            [StarSize.III]: 2_280,
            [StarSize.IV]: 0,
            [StarSize.V]: 0.007,
            [StarSize.VI]: 0.002,
            [StarSize.D]: 0.00003,
        },
    },
};

const massTables: {
  [key in StarType]: { [key in 0 | 5]: { [key in StarSize]: number } };
} = {
  [StarType.O]: {
    0: {
      [StarSize.Ia]: 0,
      [StarSize.Ib]: 0,
      [StarSize.II]: 0,
      [StarSize.III]: 0,
      [StarSize.IV]: 0,
      [StarSize.V]: 0,
      [StarSize.VI]: 0,
      [StarSize.D]: 0,
    },
    5: {
      [StarSize.Ia]: 0,
      [StarSize.Ib]: 0,
      [StarSize.II]: 0,
      [StarSize.III]: 0,
      [StarSize.IV]: 0,
      [StarSize.V]: 0,
      [StarSize.VI]: 0,
      [StarSize.D]: 0,
    },
  },
  [StarType.B]: {
    0: {
      [StarSize.Ia]: 60,
      [StarSize.Ib]: 50,
      [StarSize.II]: 30,
      [StarSize.III]: 25,
      [StarSize.IV]: 20,
      [StarSize.V]: 18,
      [StarSize.VI]: 0,
      [StarSize.D]: 0.26,
    },
    5: {
      [StarSize.Ia]: 30,
      [StarSize.Ib]: 25,
      [StarSize.II]: 20,
      [StarSize.III]: 15,
      [StarSize.IV]: 10,
      [StarSize.V]: 6.5,
      [StarSize.VI]: 0,
      [StarSize.D]: 0.26,
    },
  },
  [StarType.A]: {
    0: {
      [StarSize.Ia]: 18,
      [StarSize.Ib]: 16,
      [StarSize.II]: 14,
      [StarSize.III]: 12,
      [StarSize.IV]: 6,
      [StarSize.V]: 3.2,
      [StarSize.VI]: 0,
      [StarSize.D]: 0.36,
    },
    5: {
      [StarSize.Ia]: 15,
      [StarSize.Ib]: 13,
      [StarSize.II]: 11,
      [StarSize.III]: 9,
      [StarSize.IV]: 4,
      [StarSize.V]: 2.1,
      [StarSize.VI]: 0,
      [StarSize.D]: 0.36,
    },
  },
  [StarType.F]: {
    0: {
      [StarSize.Ia]: 13,
      [StarSize.Ib]: 12,
      [StarSize.II]: 10,
      [StarSize.III]: 8,
      [StarSize.IV]: 2.5,
      [StarSize.V]: 1.7,
      [StarSize.VI]: 0,
      [StarSize.D]: 0.42,
    },
    5: {
      [StarSize.Ia]: 12,
      [StarSize.Ib]: 10,
      [StarSize.II]: 8.1,
      [StarSize.III]: 5,
      [StarSize.IV]: 2,
      [StarSize.V]: 1.3,
      [StarSize.VI]: 0.8,
      [StarSize.D]: 0.42,
    },
  },
  [StarType.G]: {
    0: {
      [StarSize.Ia]: 12,
      [StarSize.Ib]: 10,
      [StarSize.II]: 8.1,
      [StarSize.III]: 2.5,
      [StarSize.IV]: 1.75,
      [StarSize.V]: 1.04,
      [StarSize.VI]: 0.6,
      [StarSize.D]: 0.63,
    },
    5: {
      [StarSize.Ia]: 13,
      [StarSize.Ib]: 12,
      [StarSize.II]: 10,
      [StarSize.III]: 3.2,
      [StarSize.IV]: 2,
      [StarSize.V]: 0.94,
      [StarSize.VI]: 0.528,
      [StarSize.D]: 0.63,
    },
  },
  [StarType.K]: {
    0: {
      [StarSize.Ia]: 14,
      [StarSize.Ib]: 13,
      [StarSize.II]: 11,
      [StarSize.III]: 4,
      [StarSize.IV]: 2.3,
      [StarSize.V]: 0.825,
      [StarSize.VI]: 0.43,
      [StarSize.D]: 0.83,
    },
    5: {
      [StarSize.Ia]: 18,
      [StarSize.Ib]: 16,
      [StarSize.II]: 14,
      [StarSize.III]: 5,
      [StarSize.IV]: 0,
      [StarSize.V]: 0.57,
      [StarSize.VI]: 0.33,
      [StarSize.D]: 0.83,
    },
  },
  [StarType.M]: {
    0: {
      [StarSize.Ia]: 20,
      [StarSize.Ib]: 16,
      [StarSize.II]: 14,
      [StarSize.III]: 6.3,
      [StarSize.IV]: 0,
      [StarSize.V]: 0.489,
      [StarSize.VI]: 0.154,
      [StarSize.D]: 1.11,
    },
    5: {
      [StarSize.Ia]: 25,
      [StarSize.Ib]: 20,
      [StarSize.II]: 16,
      [StarSize.III]: 7.4,
      [StarSize.IV]: 0,
      [StarSize.V]: 0.331,
      [StarSize.VI]: 0.104,
      [StarSize.D]: 1.11,
    },
  },
};

const starSystemNames: string[] = [
  "Aegis Prime",
  "Bellatrix Nebula",
  "Cygnus Reach",
  "Draco's Claw",
  "Eridanus Expanse",
  "Fornax Frontier",
  "Gliese",
  "Hydra's Heart",
  "Ixion Corridor",
  "Juno's Veil",
  "Kepler's Keep",
  "Lyra's Light",
  "Mizar Maze",
  "Nova Nexus",
  "Orion's Forge",
  "Polaris Point",
  "Quantum Quasar",
  "Rigel Rift",
  "Sirius Sector",
  "Taurus Tides",
  "Ursa Ultima",
  "Vega Void",
  "Wolf's Wisp",
  "Xena Crossroads",
  "Yggdrasil Yard",
  "Zenith Zone",
  "Altair Abyss",
  "Betelgeuse Beacon",
  "Cassiopeia Cluster",
  "Deneb Depths",
  "Epsilon Enigma",
  "Fomalhaut Fringes",
  "Gemini Gates",
  "Hercules Haven",
  "Io Isthmus",
  "Jupiter's Jewel",
  "Kochab Keystone",
  "Leo's Lair",
  "Merak Mists",
  "Nemesis Nexus",
  "Oberon Oasis",
  "Procyon Passage",
  "Quintessa",
  "Regulus Reach",
  "Spica Spiral",
  "Theta Threshold",
  "Umbra Utopia",
  "Virgo Venture",
  "Wezen Warp",
  "Xanadu Xing",
  "Yakima Yards",
  "Zosma Zone",
  "Andromeda Anchorage",
  "Boötes Borderlands",
  "Canis Corridor",
  "Dorado Dominion",
  "Eridanus Expanse",
  "Fornax Frontier",
  "Grus Gateway",
  "Hydrus Horizon",
  "Indus Inlet",
  "Lacerta Labyrinth",
  "Mensa Meridian",
  "Norma Nebula",
  "Octans Odyssey",
  "Pavo Paradise",
  "Reticulum Realm",
  "Sagitta Sanctuary",
  "Tucana Traverse",
  "Ursa Utopia",
  "Vela Voyage",
  "Volans Vista",
  "Vulpecula Valley",
  "Carina Crossroads",
  "Centaurus Citadel",
  "Cetus Confluence",
  "Chamaeleon Channel",
  "Columba Colony",
  "Corvus Crest",
  "Crater Cradle",
  "Crux Crucible",
  "Delphinus Delta",
  "Dorado Domain",
  "Draco Drifts",
  "Equuleus Equator",
  "Hercules Hinterlands",
  "Horologium Halo",
  "Hydra Highlands",
  "Indus Inlet",
  "Lepus Leap",
  "Lupus Labyrinth",
  "Lynx Ledge",
  "Microscopium Maze",
  "Monoceros Meadow",
  "Musca Mists",
  "Ophiuchus Orbit",
  "Pegasus Passage",
  "Phoenix Frontier",
  "Pictor Plateau",
  "Pisces Pools",
];

const moonNames: string[] = [
  "Aether",
  "Borealis",
  "Calypso",
  "Deimos",
  "Echo",
  "Frostbite",
  "Ganymede",
  "Hyperion",
  "Io",
  "Janus",
  "Kerberos",
  "Luna",
  "Mimas",
  "Nix",
  "Oberon",
  "Phobos",
  "Quaoar",
  "Rhea",
  "Styx",
  "Titan",
  "Umbriel",
  "Vesta",
  "Wyvern",
  "Xena",
  "Ymir",
  "Zephyr",
  "Aegaeon",
  "Belinda",
  "Charon",
  "Dione",
  "Europa",
  "Fenrir",
  "Galatea",
  "Hydra",
  "Iapetus",
  "Juliet",
  "Kale",
  "Larissa",
  "Miranda",
  "Naiad",
  "Ophelia",
  "Pandora",
  "Quincy",
  "Rosalind",
  "Sedna",
  "Tethys",
  "Uranus",
  "Vanth",
  "Weywot",
  "Xanthus",
  "Yalode",
  "Zelinda",
  "Adrastea",
  "Bianca",
  "Callisto",
  "Despina",
  "Elara",
  "Fenrir",
  "Galatea",
  "Halimede",
  "Isonoe",
  "Japet",
  "Kari",
  "Leda",
  "Metis",
  "Neso",
  "Orthosie",
  "Pasiphae",
  "Qilin",
  "Rhea",
  "Sinope",
  "Thebe",
  "Umbiel",
  "Valetudo",
  "Wanda",
  "Xerxes",
  "Ymir",
  "Zoe",
  "Ananke",
  "Bellatrix",
  "Carme",
  "Dysnomia",
  "Erinome",
  "Fjorgyn",
  "Gaspra",
  "Herse",
  "Iocaste",
  "Jarnsaxa",
  "Kallichore",
  "Lysithea",
  "Megaclite",
  "Nix",
  "Orius",
  "Praxidike",
  "Quetzal",
  "Rhadamanthys",
  "Sponde",
  "Taygete",
  "Ursula",
  "Valeska",
  "Wezen",
  "Xolotl",
  "Yvaga",
  "Zircon",
  "Aoede",
  "Beira",
  "Cyllene",
  "Daphnis",
  "Euanthe",
  "Fenrir",
  "Greip",
  "Hati",
  "Ijiraq",
  "Janus",
  "Kiviuq",
  "Loge",
  "Mundilfari",
  "Narvi",
  "Odin",
  "Pallene",
  "Quaoar",
  "Ravn",
  "Skoll",
  "Thrymr",
  "Ull",
  "Vidarr",
  "Waldron",
  "Xena",
  "Ymir",
  "Zephyr",
  "Albiorix",
  "Bestla",
  "Callirrhoe",
  "Daphnis",
  "Erriapus",
  "Farbauti",
  "Gerd",
  "Hyrrokkin",
  "Idunn",
  "Jarnsaxa",
  "Kari",
  "Loge",
  "Mimas",
  "Narvi",
  "Odin",
  "Paaliaq",
  "Quaoar",
  "Rhea",
  "Skadi",
  "Tarvos",
  "Ull",
  "Vali",
  "Wezen",
  "Xena",
  "Ymir",
  "Zephyr",
  "Aegir",
  "Bebhionn",
  "Carpo",
  "Dia",
  "Erinome",
  "Fornjot",
  "Geirrod",
  "Hati",
  "Ijiraq",
  "Janus",
  "Kale",
  "Leda",
  "Methone",
  "Neso",
  "Orthosie",
  "Pallene",
  "Quaoar",
  "Rhadamanthys",
  "Siarnaq",
  "Tethys",
  "Uranus",
  "Valeska",
  "Wanda",
  "Xolotl",
  "Yvaga",
  "Zircon",
  "Anthe",
  "Bergelmir",
  "Carme",
  "Dione",
  "Eukelade",
  "Fenrir",
  "Greip",
  "Helene",
  "Iocaste",
  "Jarnsaxa",
  "Kiviuq",
  "Lysithea",
  "Megaclite",
  "Nix",
  "Orius",
  "Praxidike",
  "Quetzal",
  "Rhea",
  "Sponde",
  "Taygete",
  "Ursula",
  "Valeska",
  "Wezen",
  "Xolotl",
  "Yvaga",
  "Zircon",
  "Aoede",
  "Beira",
  "Cyllene",
  "Daphnis",
  "Euanthe",
  "Fenrir",
  "Greip",
  "Hati",
  "Ijiraq",
  "Janus",
  "Kiviuq",
  "Loge",
  "Mundilfari",
  "Narvi",
  "Odin",
  "Pallene",
  "Quaoar",
  "Ravn",
  "Skoll",
  "Thrymr",
  "Ull",
  "Vidarr",
  "Waldron",
  "Xena",
  "Ymir",
  "Zephyr",
  "Albiorix",
  "Bestla",
  "Callirrhoe",
  "Daphnis",
  "Erriapus",
  "Farbauti",
  "Gerd",
  "Hyrrokkin",
  "Idunn",
  "Jarnsaxa",
  "Kari",
  "Loge",
  "Mimas",
  "Narvi",
  "Odin",
  "Paaliaq",
  "Quaoar",
  "Rhea",
  "Skadi",
  "Tarvos",
  "Ull",
  "Vali",
  "Wezen",
  "Xena",
  "Ymir",
  "Zephyr",
  "Aegir",
  "Bebhionn",
  "Carpo",
  "Dia",
  "Erinome",
  "Fornjot",
  "Geirrod",
  "Hati",
  "Ijiraq",
  "Janus",
  "Kale",
  "Leda",
  "Methone",
  "Neso",
  "Orthosie",
  "Pallene",
  "Quaoar",
  "Rhadamanthys",
  "Siarnaq",
  "Tethys",
  "Uranus",
  "Valeska",
  "Wanda",
  "Xolotl",
  "Yvaga",
  "Zircon",
  "Anthe",
  "Bergelmir",
  "Carme",
  "Dione",
  "Eukelade",
  "Fenrir",
  "Greip",
  "Helene",
  "Iocaste",
  "Jarnsaxa",
  "Kiviuq",
  "Lysithea",
  "Megaclite",
  "Nix",
  "Orius",
  "Praxidike",
  "Quetzal",
  "Rhea",
  "Sponde",
  "Taygete",
  "Ursula",
  "Valeska",
  "Wezen",
  "Xolotl",
  "Yvaga",
  "Zircon",
  "Aoede",
  "Beira",
  "Cyllene",
  "Daphnis",
  "Euanthe",
  "Fenrir",
  "Greip",
  "Hati",
  "Ijiraq",
  "Janus",
  "Kiviuq",
  "Loge",
  "Mundilfari",
  "Narvi",
  "Odin",
  "Pallene",
  "Quaoar",
  "Ravn",
  "Skoll",
  "Thrymr",
  "Ull",
  "Vidarr",
  "Waldron",
  "Xena",
  "Ymir",
  "Zephyr",
];

const planetNames: string[] = [
  "Zephyria",
  "Novacron",
  "Lumina",
  "Aethoria",
  "Celestis",
  "Thalassia",
  "Chronos",
  "Nebulos",
  "Helixia",
  "Quasar",
  "Vortexia",
  "Stellaris",
  "Novalux",
  "Astralis",
  "Galaxia",
  "Pulsara",
  "Cosmora",
  "Novastra",
  "Solara",
  "Lunara",
  "Auroris",
  "Nebulox",
  "Heliosia",
  "Quantara",
  "Novatron",
  "Stellara",
  "Pulsaris",
  "Galaxara",
  "Chronara",
  "Vortexis",
  "Aetheron",
  "Celestara",
  "Thalassos",
  "Novastria",
  "Luminara",
  "Nebulon",
  "Helixara",
  "Quasara",
  "Stellarix",
  "Pulsaron",
  "Galaxora",
  "Chronosia",
  "Vortexara",
  "Aethorix",
  "Celestron",
  "Thalassara",
  "Novastra",
  "Luminaris",
  "Nebulara",
  "Helixion",
  "Quasaris",
  "Stellaron",
  "Pulsaria",
  "Galaxion",
  "Chronara",
  "Vortexion",
  "Aethoron",
  "Celestara",
  "Thalassion",
  "Novastron",
  "Luminara",
  "Nebuloris",
  "Helixara",
  "Quasaron",
  "Stellaris",
  "Pulsarion",
  "Galaxara",
  "Chronorix",
  "Vortexara",
  "Aethoris",
  "Celestris",
  "Thalassara",
  "Novastris",
  "Luminaris",
  "Nebularis",
  "Helixoris",
  "Quasaris",
  "Stellaron",
  "Pulsara",
  "Galaxoris",
  "Chronara",
  "Vortexaris",
  "Aethoron",
  "Celestara",
  "Thalassion",
  "Novastron",
  "Luminara",
  "Nebuloris",
  "Helixara",
  "Quasaron",
  "Stellaris",
  "Pulsarion",
  "Galaxara",
  "Chronorix",
  "Vortexara",
  "Aethoris",
  "Celestris",
  "Thalassara",
  "Novastris",
  "Luminaris",
  "Zephyros",
  "Novacrys",
  "Luminos",
  "Aethoros",
  "Celestos",
  "Thalassos",
  "Chronos",
  "Nebulos",
  "Helixos",
  "Quasaros",
  "Vortexos",
  "Stellaros",
  "Novaluxos",
  "Astralos",
  "Galaxos",
  "Pulsaros",
  "Cosmoros",
  "Novastros",
  "Solaros",
  "Lunaros",
  "Auroros",
  "Nebuloxos",
  "Heliosios",
  "Quantaros",
  "Novatronos",
  "Stellaros",
  "Pulsaros",
  "Galaxaros",
  "Chronaros",
  "Vortexos",
  "Aetheronos",
  "Celestaros",
  "Thalassos",
  "Novastrios",
  "Luminaros",
  "Nebulonos",
  "Helixaros",
  "Quasaros",
  "Stellarixos",
  "Pulsaronos",
  "Galaxoros",
  "Chronosios",
  "Vortexaros",
  "Aethorixos",
  "Celestronos",
  "Thalassaros",
  "Novastros",
  "Luminaros",
  "Nebularos",
  "Helixionos",
  "Quasaris",
  "Stellaronos",
  "Pulsarios",
  "Galaxionos",
  "Chronaros",
  "Vortexionos",
  "Aethoronos",
  "Celestaros",
  "Thalassionos",
  "Novastronos",
  "Luminaros",
  "Nebuloris",
  "Helixaros",
  "Quasaronos",
  "Stellaris",
  "Pulsarionos",
  "Galaxaros",
  "Chronorixos",
  "Vortexaros",
  "Aethoris",
  "Celestris",
  "Thalassaros",
  "Novastris",
  "Luminaros",
  "Nebularis",
  "Helixoris",
  "Quasaris",
  "Stellaronos",
  "Pulsaros",
  "Galaxoris",
  "Chronaros",
  "Vortexaris",
  "Aethoronos",
  "Celestaros",
  "Thalassionos",
  "Novastronos",
  "Luminaros",
  "Nebuloris",
  "Helixaros",
  "Quasaronos",
  "Stellaris",
  "Pulsarionos",
  "Galaxaros",
  "Chronorixos",
  "Vortexaros",
  "Aethoris",
  "Celestris",
  "Thalassaros",
  "Novastris",
  "Luminaros",
  "Zephyria",
  "Novacronia",
  "Luminara",
  "Aethoria",
  "Celestia",
  "Thalassia",
  "Chronia",
  "Nebulia",
  "Helixia",
  "Quasaria",
  "Vortexia",
  "Stellaria",
  "Novaluxia",
  "Astralia",
  "Galaxia",
  "Pulsaria",
  "Cosmoria",
  "Novastria",
  "Solaria",
  "Lunaria",
  "Auroria",
  "Nebuloxia",
  "Heliosia",
  "Quantaria",
  "Novatronia",
  "Stellaria",
  "Pulsaria",
  "Galaxaria",
  "Chronia",
  "Vortexia",
  "Aetheronia",
  "Celestaria",
  "Thalassia",
  "Novastria",
  "Luminaria",
  "Nebulia",
  "Helixaria",
  "Quasaria",
  "Stellarixia",
  "Pulsaronia",
  "Galaxoria",
  "Chronosia",
  "Vortexaria",
  "Aethorixia",
  "Celestronia",
  "Thalassaria",
  "Novastria",
  "Luminaria",
  "Nebularia",
  "Helixionia",
  "Quasaria",
  "Stellaronia",
  "Pulsaria",
  "Galaxionia",
  "Chronaria",
  "Vortexionia",
  "Aethoronia",
  "Celestaria",
  "Thalassionia",
  "Novastronia",
  "Luminaria",
  "Nebuloris",
  "Helixaria",
  "Quasaronia",
  "Stellaria",
  "Pulsarionia",
  "Galaxaria",
  "Chronorixia",
  "Vortexaria",
  "Aethoria",
  "Celestria",
  "Thalassaria",
  "Novastria",
  "Luminaria",
  "Nebularia",
  "Helixoria",
  "Quasaria",
  "Stellaronia",
  "Pulsaria",
  "Galaxoria",
  "Chronaria",
  "Vortexaria",
  "Aethoronia",
  "Celestaria",
  "Thalassionia",
  "Novastronia",
  "Luminaria",
  "Nebuloris",
  "Helixaria",
  "Quasaronia",
  "Stellaria",
  "Pulsarionia",
  "Galaxaria",
  "Chronorixia",
  "Vortexaria",
  "Aethoria",
  "Celestria",
  "Thalassaria",
  "Novastria",
  "Luminaria",
  "Zephyron",
  "Novacron",
  "Luminon",
  "Aethoron",
  "Celeston",
  "Thalassion",
  "Chronon",
  "Nebulon",
  "Helixon",
  "Quasaron",
  "Vortexon",
  "Stellaron",
  "Novaluxon",
  "Astralon",
  "Galaxon",
  "Pulsaron",
  "Cosmoron",
  "Novastron",
  "Solaron",
  "Lunaron",
  "Auroron",
  "Nebuloxon",
  "Heliosion",
  "Quantaron",
  "Novatronon",
  "Stellaron",
  "Pulsaron",
  "Galaxaron",
  "Chronaron",
  "Vortexon",
  "Aetheron",
  "Celestaron",
  "Thalassion",
  "Novastrion",
  "Luminaron",
  "Nebulon",
  "Helixaron",
  "Quasaron",
  "Stellarixon",
  "Pulsaron",
  "Galaxoron",
  "Chronosion",
  "Vortexaron",
  "Aethorixon",
  "Celestronon",
  "Thalassaron",
  "Novastron",
  "Luminaron",
  "Nebularon",
  "Helixionon",
  "Quasaron",
  "Stellaron",
  "Pulsarion",
  "Galaxionon",
  "Chronaron",
  "Vortexionon",
  "Aethoron",
  "Celestaron",
  "Thalassionon",
  "Novastronon",
  "Luminaron",
  "Nebuloron",
  "Helixaron",
  "Quasaron",
  "Stellaron",
  "Pulsarionon",
  "Galaxaron",
  "Chronorixon",
  "Vortexaron",
  "Aethoron",
  "Celestron",
  "Thalassaron",
  "Novastron",
  "Luminaron",
  "Nebularon",
  "Helixoron",
  "Quasaron",
  "Stellaron",
  "Pulsaron",
  "Galaxoron",
  "Chronaron",
  "Vortexaron",
  "Aethoron",
  "Celestaron",
  "Thalassionon",
  "Novastronon",
  "Luminaron",
  "Nebuloron",
  "Helixaron",
  "Quasaron",
  "Stellaron",
  "Pulsarionon",
  "Galaxaron",
  "Chronorixon",
  "Vortexaron",
  "Aethoron",
  "Celestron",
  "Thalassaron",
  "Novastron",
  "Luminaron",
];
