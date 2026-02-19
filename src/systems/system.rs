//! # Star System Generation Module
//!
//! This module contains the core functionality for generating complete Traveller star systems,
//! including stellar mechanics, orbital dynamics, and system-wide coordination. It serves as
//! the primary orchestrator for creating realistic multi-star systems with worlds, gas giants,
//! and their satellites.
//!
//! ## Key Features
//!
//! - **Multi-Star Systems**: Supports primary, secondary, and tertiary star configurations
//! - **Orbital Mechanics**: Realistic orbital slot management and companion star placement
//! - **World Generation**: Coordinates placement of main worlds, gas giants, and planetoids
//! - **Satellite Systems**: Manages satellite generation for all orbital bodies
//! - **Zone Management**: Handles stellar zones and habitability calculations
//!
//! ## System Architecture
//!
//! The [`System`] struct represents a complete star system with:
//! - A primary star with its orbital slots
//! - Optional secondary and tertiary companion stars
//! - Various orbital contents (worlds, gas giants, blocked orbits)
//! - Hierarchical satellite systems
//!
//! ## Generation Process
//!
//! 1. **Star Generation**: Creates primary star and determines companions
//! 2. **Orbit Allocation**: Assigns orbital slots and blocks inappropriate orbits
//! 3. **Gas Giant Placement**: Generates gas giants in suitable orbits
//! 4. **Planetoid Generation**: Creates asteroid belts and planetoid systems
//! 5. **Main World Placement**: Places the system's primary inhabited world
//! 6. **World Generation**: Fills remaining orbits with generated worlds
//! 7. **Satellite Generation**: Creates moons for all applicable bodies
//! 8. **Companion Processing**: Recursively processes secondary/tertiary systems

use log::{debug, error, warn};
use rand::prelude::IndexedRandom;
use rand::Rng;
use reactive_stores::Store;
use std::fmt::Display;

use crate::systems::gas_giant::{GasGiant, GasGiantSize};
use crate::systems::has_satellites::HasSatellites;
use crate::systems::name_tables::gen_star_system_name;
use crate::systems::system_tables::get_zone;
use crate::systems::world::World;
use crate::util::{roll_10, roll_1d6, roll_2d6};

/// A complete star system with primary star and optional companions
///
/// Represents a Traveller star system containing a primary star, optional
/// secondary and tertiary companion stars, and all orbital bodies including
/// worlds, gas giants, and blocked orbits. The system manages orbital slots
/// and coordinates the generation of all system components.
///
/// ## System Hierarchy
///
/// - **Primary System**: The main star with its orbital slots
/// - **Secondary System**: Optional companion star (close, far, or orbital)
/// - **Tertiary System**: Optional third star (typically far orbit)
///
/// ## Orbital Management
///
/// Each system maintains a vector of orbital slots that can contain:
/// - Worlds (rocky planets with full UWP characteristics)
/// - Gas giants (with their own satellite systems)
/// - Secondary/tertiary star markers
/// - Blocked orbits (intentionally empty for realism)
#[derive(Debug, Clone, Store)]
pub struct System {
    pub name: String,
    pub star: Star,
    #[store]
    pub secondary: Option<Box<System>>,
    #[store]
    pub tertiary: Option<Box<System>>,
    pub orbit: StarOrbit,
    #[store]
    pub orbit_slots: Vec<Option<OrbitContent>>,
}

// Enums
/// Stellar spectral classification types
///
/// Represents the seven main stellar spectral classes in order from
/// hottest to coolest. Each type has distinct characteristics affecting
/// luminosity, habitable zones, and system generation.
///
/// ## Spectral Classes
///
/// - **O**: Blue supergiants, extremely hot (30,000-50,000K), very rare
/// - **B**: Blue giants, very hot (10,000-30,000K), short-lived
/// - **A**: White stars, hot (7,500-10,000K), rapid rotation
/// - **F**: Yellow-white stars, moderately hot (6,000-7,500K)
/// - **G**: Yellow stars like Sol (5,200-6,000K), stable main sequence
/// - **K**: Orange stars, cooler (3,700-5,200K), long-lived
/// - **M**: Red dwarfs, coolest (2,400-3,700K), most common, very long-lived
#[derive(Debug, Store, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub enum StarType {
    O,
    B,
    A,
    F,
    G,
    K,
    M,
}

/// Stellar subtype refinement (0-9)
///
/// Provides finer classification within each spectral type.
/// Lower numbers indicate hotter stars within the type.
/// For example, G0 is hotter than G9.
pub type StarSubType = u8;

/// Stellar luminosity class (size classification)
///
/// Indicates the star's luminosity and evolutionary stage.
/// Affects stellar zones, companion generation, and system characteristics.
///
/// ## Size Classes
///
/// - **Ia/Ib**: Supergiants, extremely luminous, short-lived
/// - **II**: Bright giants, evolved stars with extended zones
/// - **III**: Giants, evolved stars past main sequence
/// - **IV**: Subgiants, transitioning from main sequence
/// - **V**: Main sequence (dwarfs), stable hydrogen burning
/// - **VI**: Subdwarfs, metal-poor, lower luminosity
/// - **D**: White dwarfs, stellar remnants, very compact zones
#[derive(Debug, Store, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub enum StarSize {
    Ia,
    Ib,
    II,
    #[allow(clippy::upper_case_acronyms)]
    III,
    #[allow(clippy::upper_case_acronyms)]
    IV,
    V,
    #[allow(clippy::upper_case_acronyms)]
    VI,
    D,
}

/// Orbital relationship of companion stars
///
/// Defines how companion stars relate to the primary system,
/// affecting their orbital mechanics and zone interactions.
///
/// ## Orbit Types
///
/// - **Primary**: Contact binary or very close orbit
/// - **Far**: Distant orbit, independent zone system
/// - **System(n)**: Orbits within primary's zone system at position n
#[derive(Debug, Store, Clone, Copy, PartialEq, Eq)]
pub enum StarOrbit {
    Primary,
    Far,
    System(usize),
}

// Structs
/// Complete stellar classification
///
/// Combines spectral type, subtype, and size class to fully
/// specify a star's characteristics. Used for zone calculations,
/// luminosity lookup, and system generation parameters.
///
/// ## Examples
///
/// - Sol: G2V (G-type, subtype 2, main sequence)
/// - Rigel: B8Ia (B-type, subtype 8, supergiant)
/// - Proxima Centauri: M5.5V (M-type, subtype 5-6, main sequence)
#[derive(Debug, Store, Clone, Copy, PartialEq, Eq)]
pub struct Star {
    pub star_type: StarType,
    pub subtype: StarSubType,
    pub size: StarSize,
}

/// Contents of an orbital slot
///
/// Represents what occupies a specific orbital position in the system.
/// Each orbit can contain at most one type of content, though some
/// contents (like gas giants) can host their own satellite systems.
///
/// ## Content Types
///
/// - **Secondary/Tertiary**: Markers for companion star locations
/// - **World**: Rocky planets with full UWP characteristics
/// - **GasGiant**: Gas giants with satellite systems
/// - **Blocked**: Intentionally empty orbits for realism
#[derive(Debug, Store, Clone)]
pub enum OrbitContent {
    // This orbit contains the secondary star system of the primary.
    Secondary,
    // This orbit contains the tertiary star system of the primary.
    Tertiary,
    // This orbit contains a world
    #[store]
    World(World),
    // This orbit contains a gas giant
    #[store]
    GasGiant(GasGiant),
    // This orbit is intentionally empty and cannot be filled.
    Blocked,
}

impl System {
    pub fn new(
        star_type: StarType,
        subtype: StarSubType,
        size: StarSize,
        orbit: StarOrbit,
        max_orbits: usize,
    ) -> System {
        System {
            name: gen_star_system_name(),
            star: Star {
                star_type,
                subtype,
                size,
            },
            secondary: None,
            tertiary: None,
            orbit,
            orbit_slots: vec![None; max_orbits],
        }
    }

    pub fn count_stars(&self) -> i32 {
        let mut count = 1;
        if let Some(secondary) = &self.secondary {
            count += secondary.count_stars();
        }
        if let Some(tertiary) = &self.tertiary {
            count += tertiary.count_stars();
        }
        count
    }

    pub fn set_max_orbits(&mut self, max_orbits: usize) {
        self.orbit_slots.resize(max_orbits + 1, None);
    }

    pub fn get_max_orbits(&self) -> usize {
        self.orbit_slots.len()
    }

    pub fn is_slot_empty(&self, orbit: usize) -> bool {
        self.orbit_slots
            .get(orbit)
            .map(|body| body.is_none())
            .unwrap_or(true)
    }

    pub fn get_unused_orbits(&self) -> Vec<usize> {
        self.orbit_slots
            .iter()
            .enumerate()
            .filter_map(
                |(index, body)| {
                    if body.is_none() {
                        Some(index)
                    } else {
                        None
                    }
                },
            )
            .collect()
    }

    pub fn set_orbit_slot(&mut self, orbit: usize, content: OrbitContent) {
        if orbit >= self.orbit_slots.len() {
            self.set_max_orbits(orbit);
        }
        self.orbit_slots[orbit] = Some(content);
    }

    pub fn generate_system(mut main_world: World) -> System {
        let star_mod = if (main_world.atmosphere >= 4 && main_world.atmosphere <= 9)
            || main_world.get_population() >= 8
        {
            4
        } else {
            0
        };
        let mut system = gen_stars(star_mod, true);
        main_world.gen_trade_classes();
        system.fill_system(main_world, true);
        system
    }

    fn generate_companion(
        primary_type_roll: i32,
        primary_size_roll: i32,
        orbit: StarOrbit,
    ) -> System {
        let companion_type_roll = roll_2d6() + primary_type_roll;
        let companion_size_roll = roll_2d6() + primary_size_roll;
        let mut companion: System = System::new(
            gen_companion_star_type(companion_type_roll),
            roll_10() as StarSubType,
            gen_companion_star_size(companion_size_roll),
            orbit,
            0,
        );
        companion.set_max_orbits(gen_max_orbits(&companion.star));

        if companion.orbit == StarOrbit::Far {
            // If secondary is Far then it can have companions.
            if gen_num_stars() > 1 {
                // -4 to this as we're a secondary of a secondary.
                let orbit = gen_companion_orbit(roll_2d6() - 4);
                let mut secondary: Box<System> = Box::new(System::generate_companion(
                    companion_type_roll,
                    companion_size_roll,
                    orbit,
                ));

                // If the secondary of the secondary is also in a FAR orbit, then it can have a full range of
                // orbits itself.  Otherwise it is halved.
                if orbit == StarOrbit::Far {
                    secondary.set_max_orbits(gen_max_orbits(&secondary.star));
                } else {
                    secondary.set_max_orbits(gen_max_orbits(&secondary.star) / 2);
                }

                companion.secondary = Some(secondary);
                if let StarOrbit::System(orbit) = orbit {
                    companion.set_orbit_slot(orbit, OrbitContent::Secondary);
                }
            }
        }
        companion
    }

    fn gen_planetoids(&mut self, num_giants: i32, main_world: &World) {
        if roll_2d6() >= 7 {
            // No planetoids in system
            return;
        }
        let mut num_planetoids = match roll_2d6() - num_giants {
            1..=3 => 3,
            4..=6 => 2,
            _ => 1,
        };
        let mut viable_giants: Vec<usize> = self
            .orbit_slots
            .iter()
            .enumerate()
            .filter_map(|(index, body)| {
                if matches!(body, Some(OrbitContent::GasGiant(_))) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();

        let mut viable_other_orbits: Vec<usize> = self
            .orbit_slots
            .iter()
            .enumerate()
            .filter_map(
                |(index, body)| {
                    if body.is_none() {
                        Some(index)
                    } else {
                        None
                    }
                },
            )
            .collect();

        while viable_giants.len() + viable_other_orbits.len() > 0 && num_planetoids > 0 {
            let orbit = if !viable_giants.is_empty() {
                let pos = rand::rng().random_range(0..viable_giants.len());
                viable_giants.remove(pos);
                pos
            } else {
                let pos = rand::rng().random_range(0..viable_other_orbits.len());
                viable_other_orbits.remove(pos);
                pos
            };

            let population = (roll_2d6() - 2
                + if orbit as i32 <= get_zone(&self.star).inner {
                    -5
                } else {
                    0
                }
                + if orbit as i32 > get_zone(&self.star).habitable {
                    -5
                } else {
                    0
                })
            .clamp(0, main_world.get_population() - 1);

            let mut planetoid = World::new(
                "Planetoid Belt".to_string(),
                orbit,
                orbit,
                0,
                0,
                0,
                population,
                false,
                false,
            );

            planetoid.gen_subordinate_stats(main_world);
            planetoid.gen_trade_classes();
            planetoid.gen_subordinate_facilities(&get_zone(&self.star), orbit, main_world);
            planetoid.compute_astro_data(&self.star);
            self.set_orbit_slot(orbit, OrbitContent::World(planetoid));
            num_planetoids -= 1;
        }
    }

    fn gen_gas_giants(&mut self) -> i32 {
        if roll_2d6() >= 10 {
            // No gas giant in system
            return 0;
        }

        let mut num_giants = match roll_2d6() {
            1..=3 => 1,
            4..=5 => 2,
            6..=7 => 3,
            8..=10 => 4,
            _ => 5,
        };

        num_giants = num_giants.min(count_open_orbits(self));
        let original_num_giants = num_giants;

        let habitable = get_zone(&self.star).habitable;

        let mut viable_outer_orbits: Vec<usize> = self
            .orbit_slots
            .iter()
            .enumerate()
            .filter_map(|(index, body)| {
                if body.is_none() {
                    if index as i32 > habitable {
                        Some(index)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        let mut viable_inner_orbits: Vec<usize> = self
            .orbit_slots
            .iter()
            .enumerate()
            .filter_map(|(index, body)| {
                if body.is_none() {
                    if index as i32 <= habitable {
                        Some(index)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        while viable_outer_orbits.len() + viable_inner_orbits.len() > 0 && num_giants > 0 {
            let orbit = if !viable_outer_orbits.is_empty() {
                let pos = rand::rng().random_range(0..viable_outer_orbits.len());

                viable_outer_orbits.remove(pos)
            } else {
                let pos = rand::rng().random_range(0..viable_inner_orbits.len());

                viable_inner_orbits.remove(pos)
            };

            if roll_1d6() <= 3 {
                self.set_orbit_slot(
                    orbit,
                    OrbitContent::GasGiant(GasGiant::new(GasGiantSize::Small, orbit)),
                );
            } else {
                self.set_orbit_slot(
                    orbit,
                    OrbitContent::GasGiant(GasGiant::new(GasGiantSize::Large, orbit)),
                );
            }
            num_giants -= 1;
        }

        if num_giants > 0 {
            error!(
                "Not enough orbits for gas giants. Need {original_num_giants} in system {self:?}",
            );
        }
        original_num_giants - num_giants
    }

    fn fill_system(&mut self, main_world: World, is_primary: bool) {
        // First block appropriate orbits (just to have some number of empty orbits)
        self.gen_blocked_orbits();

        let main_world_copy = main_world.clone();
        let system_zones = get_zone(&self.star);
        let num_gas_giants = self.gen_gas_giants();

        // Next generate planetoids
        self.gen_planetoids(num_gas_giants, &main_world_copy);

        if is_primary {
            self.place_main_world(main_world);
        }

        for i in 0..=get_zone(&self.star).hot {
            self.set_orbit_slot(i as usize, OrbitContent::Blocked);
        }

        for i in (get_zone(&self.star).hot + 1)..self.get_max_orbits() as i32 {
            let i = i as usize;
            if self.is_slot_empty(i) {
                let mut new_world = World::generate(&self.star, i, &main_world_copy);
                new_world.gen_name(&self.name, i);
                new_world.compute_astro_data(&self.star);
                self.set_orbit_slot(i, OrbitContent::World(new_world));
            }
        }

        let zone_table = get_zone(&self.star);
        for i in 0..self.get_max_orbits() {
            match &mut self.orbit_slots[i] {
                Some(OrbitContent::World(world)) => {
                    let num_satellites = world.determine_num_satellites();
                    for _ in 0..num_satellites {
                        world.gen_satellite(&zone_table, &main_world_copy, &self.star);
                    }
                    world.clean_satellites();
                }
                Some(OrbitContent::GasGiant(gas_giant)) => {
                    let num_satellites = gas_giant.determine_num_satellites();
                    for _ in 0..num_satellites {
                        gas_giant.gen_satellite(&system_zones, &main_world_copy, &self.star);
                    }
                    gas_giant.clean_satellites();
                    gas_giant.gen_name(&self.name, i);
                }
                _ => continue,
            }
        }

        if let Some(secondary) = &mut self.secondary
            && secondary.orbit != StarOrbit::Primary {
                secondary.fill_system(main_world_copy.clone(), false);
            }

        if let Some(tertiary) = &mut self.tertiary
            && tertiary.orbit != StarOrbit::Primary {
                tertiary.fill_system(main_world_copy, false);
            }
    }

    fn place_main_world(&mut self, mut main_world: World) {
        let requires_habitable =
            main_world.atmosphere > 1 && main_world.atmosphere < 10 && main_world.size > 0;
        let mut habitable = get_zone(&self.star).habitable;
        if (habitable <= 0 || habitable == get_zone(&self.star).inner) && requires_habitable {
            warn!("No habitable zone for main world for system: {:?}. Habitable = {}. Inner = {}. Using orbit 0.", self, habitable, get_zone(&self.star).inner);
            habitable = get_zone(&self.star).inner.max(0);
        }

        debug!(
            "(place_main_world) habitable = {}, max_orbits = {}, requires_habitable = {}, star = {}",
            habitable,
            self.get_max_orbits(),
            requires_habitable,
            self.star
        );

        // Place the main world. After placing be sure and generate the astro data (cannot do it until its placed!)
        if requires_habitable {
            // Just place in the habitable
            match &mut self
                .orbit_slots
                .get_mut(habitable as usize)
                .unwrap_or(&mut None)
            {
                Some(OrbitContent::Secondary) => {
                    // If there happens to be a star in the habitable zone, place it in orbit there.
                    // Note the orbit of the main world in terms of primary first
                    // TODO: Is this correct when we have multiple stars?
                    main_world.position_in_system = habitable as usize;
                    // Safe to unwrap as if the orbital position is secondary but there is no secondary, thats bug.
                    self.secondary
                        .as_mut()
                        .unwrap()
                        .place_main_world(main_world);
                }
                Some(OrbitContent::Tertiary) => {
                    // If there happens to be a star in the habitable zone, place it in orbit there.
                    // Note the orbit of the main world in terms of primary first
                    // TODO: Is this correct when we have multiple stars?
                    main_world.position_in_system = habitable as usize;
                    // Safe to unwrap as if the orbital position is tertiary but there is no tertiary, thats bug.
                    self.tertiary.as_mut().unwrap().place_main_world(main_world);
                }
                Some(OrbitContent::GasGiant(gas_giant)) => {
                    let orbit = gas_giant.gen_satellite_orbit(main_world.size == 0);
                    main_world.orbit = orbit;
                    main_world.position_in_system = habitable as usize;
                    main_world.compute_astro_data(&self.star);
                    gas_giant.push_satellite(main_world);
                }
                Some(OrbitContent::Blocked) => {
                    main_world.position_in_system = habitable as usize;
                    main_world.orbit = habitable as usize;
                    main_world.compute_astro_data(&self.star);
                    self.set_orbit_slot(habitable as usize, OrbitContent::World(main_world));
                }
                Some(OrbitContent::World(_)) | None => {
                    main_world.position_in_system = habitable as usize;
                    main_world.orbit = habitable as usize;
                    main_world.compute_astro_data(&self.star);
                    self.set_orbit_slot(habitable as usize, OrbitContent::World(main_world));
                }
            }
        } else {
            let empty_orbits = self.get_unused_orbits();
            if !empty_orbits.is_empty() {
                let orbit = empty_orbits[rand::rng().random_range(0..empty_orbits.len())];
                main_world.position_in_system = orbit;
                main_world.orbit = orbit;
                main_world.compute_astro_data(&self.star);
                self.set_orbit_slot(orbit, OrbitContent::World(main_world));
            } else {
                // Just jam the world in somewhere.
                let pos = if self.get_max_orbits() == 0 {
                    0
                } else {
                    rand::rng().random_range(0..self.get_max_orbits())
                };
                main_world.orbit = pos;
                main_world.position_in_system = pos;
                self.set_orbit_slot(pos, OrbitContent::World(main_world));
            };
        }
    }

    fn gen_blocked_orbits(&mut self) {
        if roll_1d6() < 5 {
            // No Empty orbits
            return;
        }
        let roll = roll_1d6();
        let num_empty = match roll {
            1..=2 => 1,
            3 => 2,
            _ => 3,
        };

        let valid_orbits = self.get_unused_orbits();

        for _ in 0..num_empty {
            if let Some(pos) = valid_orbits.choose(&mut rand::rng()) {
                self.set_orbit_slot(*pos, OrbitContent::Blocked);
            }
        }
    }
}

impl Default for System {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            star: Star {
                star_type: StarType::G,
                subtype: 0,
                size: StarSize::V,
            },
            secondary: None,
            tertiary: None,
            orbit: StarOrbit::Primary,
            orbit_slots: Vec::new(),
        }
    }
}

impl Display for System {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} is a {} star.", self.name, self.star)?;
        write!(
            f,
            " Zones for {} are {:?}. ",
            self.name,
            get_zone(&self.star)
        )?;
        if let Some(secondary) = &self.secondary {
            if let StarOrbit::Primary = secondary.orbit {
                writeln!(
                    f,
                    "It has a secondary contact star {}:\n{}",
                    secondary.name, secondary
                )?;
            } else {
                writeln!(
                    f,
                    "It has a secondary star {} which is a {} star in {}.",
                    secondary.name, secondary.star, secondary.orbit
                )?;
            }
        }
        if let Some(tertiary) = &self.tertiary {
            if let StarOrbit::Primary = tertiary.orbit {
                writeln!(
                    f,
                    " It has a tertiary contact star {}:\n{}",
                    tertiary.name, tertiary
                )?;
            } else {
                writeln!(
                    f,
                    " It has a tertiary star {} which is a {} star in {}.",
                    tertiary.name, tertiary.star, tertiary.orbit
                )?;
            }
        }

        if self
            .orbit_slots
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.is_slot_empty(*index))
            .count()
            > 0
        {
            writeln!(
                f,
                "\n{:<7}{:<24}{:<12}{:<18}",
                "Orbit", "Name", "UPP", "Remarks"
            )?;
        }

        for body in self.orbit_slots.iter() {
            match body {
                Some(OrbitContent::Secondary) => {
                    if let Some(secondary) = &self.secondary
                        && let StarOrbit::System(orbit) = secondary.orbit {
                            writeln!(
                                f,
                                "{:<7}{:<24}{:<12}",
                                orbit, secondary.name, secondary.star
                            )?;
                        }
                }
                Some(OrbitContent::Tertiary) => {
                    if let Some(tertiary) = &self.tertiary
                        && let StarOrbit::System(orbit) = tertiary.orbit {
                            writeln!(f, "{:<7}{:<24}{:<12}", orbit, tertiary.name, tertiary.star)?;
                        }
                }
                Some(OrbitContent::World(world)) => {
                    writeln!(f, "{world}")?;
                }
                Some(OrbitContent::GasGiant(gas_giant)) => {
                    writeln!(f, "{gas_giant}")?;
                }
                Some(OrbitContent::Blocked) | None => {}
            }
        }

        if let Some(secondary) = &self.secondary {
            writeln!(f, "\n{secondary}")?;
        }

        if let Some(tertiary) = &self.tertiary {
            writeln!(f, "\n{tertiary}")?;
        }
        Ok(())
    }
}

impl Display for StarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Display for StarSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Display for Star {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{} {}",
            self.star_type, self.subtype as usize, self.size
        )
    }
}

impl Display for StarOrbit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StarOrbit::Primary => write!(f, "close orbit"),
            StarOrbit::Far => write!(f, "far orbit"),
            StarOrbit::System(orbit) => write!(f, "orbit {orbit}"),
        }
    }
}

// Functions not in a struct
fn gen_num_stars() -> i32 {
    let roll = roll_2d6();
    if roll <= 7 {
        1
    } else if roll < 12 {
        2
    } else {
        3
    }
}

fn gen_primary_star_type(roll: i32) -> StarType {
    match roll {
        x if x <= 1 => StarType::B,
        2 => StarType::A,
        3..=7 => StarType::M,
        8 => StarType::K,
        9 => StarType::G,
        10..=11 => StarType::F,
        _ => StarType::G,
    }
}

fn gen_primary_star_size(roll: i32, star_type: StarType, subtype: StarSubType) -> StarSize {
    let mut star_size = match roll {
        1 => StarSize::Ia,
        2 => StarSize::Ib,
        3 => StarSize::II,
        4 => StarSize::III,
        5..=10 => StarSize::V,
        11 => StarSize::VI,
        12 => StarSize::D,
        // EDITORIAL: Given bonuses on table want common case for populated world to be main sequence star.
        _ => StarSize::V,
    };
    if star_size == StarSize::IV
        && ((star_type == StarType::K && subtype >= 5)
            || star_type > StarType::K && star_type <= StarType::M)
    {
        star_size = StarSize::V;
    }
    if star_size == StarSize::VI
        && (star_type < StarType::F || (star_type == StarType::F && subtype <= 4))
    {
        star_size = StarSize::V;
    }
    star_size
}

fn gen_companion_star_type(roll: i32) -> StarType {
    match roll {
        x if x <= 1 => StarType::B,
        2 => StarType::A,
        3..=4 => StarType::F,
        5..=6 => StarType::G,
        7..=8 => StarType::K,
        _ => StarType::M,
    }
}

fn gen_companion_star_size(roll: i32) -> StarSize {
    match roll {
        1..=4 => match roll {
            1 => StarSize::Ia,
            2 => StarSize::Ib,
            3 => StarSize::II,
            4 => StarSize::III,
            _ => unreachable!(),
        },
        5..=6 => StarSize::D,
        7..=8 => StarSize::V,
        9 => StarSize::VI,
        _ => StarSize::D,
    }
}

fn gen_companion_orbit(roll: i32) -> StarOrbit {
    match roll {
        1..=3 => StarOrbit::Primary,
        4..=6 => StarOrbit::System((roll - 3) as usize),
        7..=11 => StarOrbit::System((roll - 3 + roll_1d6()) as usize),
        _ => StarOrbit::Far,
    }
}

fn gen_max_orbits(star: &Star) -> usize {
    let mut modifier = if star.size <= StarSize::II {
        8
    } else if star.size == StarSize::III {
        4
    } else {
        0
    };

    if star.star_type == StarType::M {
        modifier -= 4;
    } else if star.star_type == StarType::K {
        modifier -= 2;
    }

    let orbits = roll_2d6() + modifier;
    if orbits < 0 {
        0
    } else {
        orbits as usize
    }
}

fn empty_orbits_near_companion(system: &mut System, orbit: usize) {
    for i in (orbit / 2 + 1)..orbit {
        system.set_orbit_slot(i, OrbitContent::Blocked);
    }
    system.set_orbit_slot(orbit + 1, OrbitContent::Blocked);
    system.set_orbit_slot(orbit + 2, OrbitContent::Blocked);
}

fn gen_stars(world_mod: i32, companions_possible: bool) -> System {
    let num_stars = if companions_possible {
        gen_num_stars()
    } else {
        1
    };
    let primary_type_roll = roll_2d6();
    let primary_size_roll = roll_2d6();
    let star_type = gen_primary_star_type(primary_type_roll + world_mod);
    let star_subtype = roll_10() as StarSubType;
    let star_size = gen_primary_star_size(primary_size_roll, star_type, star_subtype);

    let mut system = System::new(star_type, star_subtype, star_size, StarOrbit::Primary, 0);
    let star = system.star;
    system.set_max_orbits(gen_max_orbits(&star));

    // Do this for a secondary, which we have with 2 or 3 stars.
    if num_stars >= 2 {
        let orbit = gen_companion_orbit(roll_2d6());
        match orbit {
            StarOrbit::Primary | StarOrbit::Far => {
                system.secondary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                )));
            }
            // If the companion has an orbit, but its inside the primary star, just treat it as the primary orbit.
            StarOrbit::System(position) if position as i32 <= get_zone(&star).inside => {
                system.secondary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    StarOrbit::Primary,
                )));
            }
            StarOrbit::System(position) => {
                system.secondary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                )));
                system.set_orbit_slot(position, OrbitContent::Secondary);
                empty_orbits_near_companion(&mut system, position);
            }
        }
    }

    // Do this for a tertiary, which we have with 3 stars.
    // TODO: This is a blatant copy of the code above; how do I DRY this?
    if num_stars == 3 {
        let orbit = gen_companion_orbit(roll_2d6() + 4);
        match orbit {
            StarOrbit::Primary | StarOrbit::Far => {
                system.tertiary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                )));
            }
            StarOrbit::System(position) if position as i32 <= get_zone(&star).inside => {
                system.tertiary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    StarOrbit::Primary,
                )));
            }
            StarOrbit::System(position) => {
                system.tertiary = Some(Box::new(System::generate_companion(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                )));
                system.set_orbit_slot(position, OrbitContent::Tertiary);
                empty_orbits_near_companion(&mut system, position);
            }
        }
    }
    system
}

fn count_open_orbits(system: &System) -> i32 {
    system
        .orbit_slots
        .iter()
        .filter(|body| body.is_none())
        .count() as i32
}

// Implement other functions...

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::arabic_to_roman;
    use std::collections::HashMap;

    #[test_log::test]
    fn test_roman_numerals() {
        assert_eq!(arabic_to_roman(1), "I");
        assert_eq!(arabic_to_roman(2), "II");
        assert_eq!(arabic_to_roman(3), "III");
        assert_eq!(arabic_to_roman(4), "IV");
        assert_eq!(arabic_to_roman(5), "V");
        assert_eq!(arabic_to_roman(6), "VI");
        assert_eq!(arabic_to_roman(7), "VII");
        assert_eq!(arabic_to_roman(8), "VIII");
        assert_eq!(arabic_to_roman(9), "IX");
        assert_eq!(arabic_to_roman(10), "X");
        assert_eq!(arabic_to_roman(11), "XI");
        assert_eq!(arabic_to_roman(12), "XII");
        assert_eq!(arabic_to_roman(13), "XIII");
        assert_eq!(arabic_to_roman(14), "XIV");
        assert_eq!(arabic_to_roman(15), "XV");
        assert_eq!(arabic_to_roman(16), "XVI");
        assert_eq!(arabic_to_roman(17), "XVII");
        assert_eq!(arabic_to_roman(18), "XVIII");
        assert_eq!(arabic_to_roman(19), "XIX");
        assert_eq!(arabic_to_roman(20), "XX");
    }

    #[test_log::test]
    fn test_generate_system() {
        let main_upp = "A788899-A";
        let main_world = World::from_upp("Main World", main_upp, false, true).unwrap();

        let system = System::generate_system(main_world);
        println!("{system}");
    }

    #[test_log::test]
    fn test_2d6_random() {
        let mut buckets = HashMap::new();
        for _ in 0..10000 {
            let roll = roll_2d6();
            *buckets.entry(roll).or_insert(0) += 1;
        }

        let mut count_vec: Vec<_> = buckets.iter().collect();
        count_vec.sort_by(|a, b| a.0.cmp(b.0));
        for (roll, count) in count_vec {
            println!("{}: {:2.2}%", roll, *count as f32 / 100.0);
        }
    }
}
