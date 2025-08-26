//! # World System Module
//!
//! This module defines the core World structure and related types for representing
//! planets and worlds in the Traveller universe, including their physical characteristics,
//! facilities, and trade classifications.
//! use log::debug;
use reactive_stores::Store;
use std::fmt::Display;

#[allow(unused_imports)]
use log::debug;

use crate::systems::astro::AstroData;
use crate::systems::has_satellites::HasSatellites;
use crate::systems::name_tables::{gen_moon_name, gen_planet_name};
use crate::systems::system::{Star, StarType};
use crate::systems::system_tables::{get_zone, ZoneTable};
use crate::util::{arabic_to_roman, roll_1d6, roll_2d6};

use crate::trade::PortCode;
use crate::trade::TradeClass;
use crate::trade::ZoneClassification;

/// Container for world satellites
///
/// Stores a vector of satellite worlds with a key based on the parent world's name.
#[derive(Debug, Clone, Store, PartialEq)]
pub struct World {
    pub name: String,
    pub orbit: usize,
    pub(crate) position_in_system: usize,
    is_satellite: bool,
    is_mainworld: bool,
    pub port: PortCode,
    pub(crate) size: i32,
    pub(crate) atmosphere: i32,
    pub(crate) hydro: i32,
    population: i32,
    law_level: i32,
    government: i32,
    pub tech_level: i32,
    facilities: Vec<Facility>,
    pub satellites: Satellites,
    trade_classes: Vec<TradeClass>,
    pub travel_zone: ZoneClassification,
    astro_data: AstroData,
    pub coordinates: Option<(i32, i32)>,
}

/// Enum for facilities that can be present on a world
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facility {
    Naval,
    Scout,
    Farming,
    Mining,
    Colony,
    Lab,
    Military,
}

/// Container for world (or gas giant) satellites
/// each satellite is in its own right a world, though
/// orbit numbering uses a different system than in the main system.
#[derive(Debug, Clone, Store, PartialEq)]
pub struct Satellites {
    #[store(key: String = |world| world.name.clone())]
    pub sats: Vec<World>,
}

impl World {
    /// Creates a new world with the specified characteristics
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the world
    /// * `orbit` - Orbital position around the primary star
    /// * `position_in_system` - Position within the star system for ordering
    /// * `size` - World size (0-A in hex)
    /// * `atmosphere` - Atmospheric composition and density (0-F in hex)
    /// * `hydro` - Hydrographic percentage (0-A in hex)
    /// * `population` - Population level (0-A+ in hex)
    /// * `is_satellite` - Whether this world is a satellite of another body
    /// * `is_mainworld` - Whether this is the main world of the system
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        orbit: usize,
        position_in_system: usize,
        size: i32,
        atmosphere: i32,
        hydro: i32,
        population: i32,
        is_satellite: bool,
        is_mainworld: bool,
    ) -> World {
        World {
            name,
            orbit,
            position_in_system,
            is_satellite,
            is_mainworld,
            port: PortCode::A,
            size,
            atmosphere,
            hydro,
            population,
            law_level: 0,
            government: 0,
            tech_level: 0,
            facilities: Vec::new(),
            satellites: Satellites { sats: Vec::new() },
            trade_classes: Vec::new(),
            travel_zone: ZoneClassification::Green,
            astro_data: AstroData::new(),
            coordinates: None,
        }
    }
    /// Generates a name for the world based on system name and orbital position
    ///
    /// If the world has population, generates a random planet name.
    /// Otherwise, uses the system name with a Roman numeral for the orbit.
    ///
    /// # Arguments
    ///
    /// * `system_name` - Name of the parent star system
    /// * `orbit` - Orbital position (0-based index)
    pub fn gen_name(&mut self, system_name: &str, orbit: usize) {
        if self.population > 0 {
            self.name = gen_planet_name()
        } else {
            self.name = format!("{} {}", system_name, arabic_to_roman(orbit + 1))
        }
    }

    /// Returns the population level of the world
    pub fn get_population(&self) -> i32 {
        self.population
    }

    /// Sets the starport code for the world
    ///
    /// # Arguments
    ///
    /// * `port` - The starport quality code to set
    pub(crate) fn set_port(&mut self, port: PortCode) {
        self.port = port;
    }

    /// Sets multiple subordinate statistics for the world
    ///
    /// # Arguments
    ///
    /// * `port` - Starport quality code
    /// * `government` - Government type
    /// * `law_level` - Law level restrictions
    /// * `tech_level` - Technology level
    /// * `facilities` - List of facilities present
    pub fn set_subordinate_stats(
        &mut self,
        port: PortCode,
        government: i32,
        law_level: i32,
        tech_level: i32,
        facilities: Vec<Facility>,
    ) {
        self.port = port;
        self.government = government;
        self.law_level = law_level;
        self.tech_level = tech_level;
        self.facilities = facilities;
    }

    /// Converts the world's characteristics to a Traveller UWP string
    pub fn to_uwp(&self) -> String {
        let size_digit = if self.is_satellite && self.size == -1 {
            "S"
        } else if self.is_satellite && self.size == 0 {
            "R"
        } else if self.size <= 0
            && !self.is_mainworld
            && !self.is_satellite
            && !self.name.contains("Planetoid")
        {
            "S"
        } else if self.size == 0 {
            "0"
        } else {
            &self.size.to_string()
        };
        format!(
            "{}{}{:X}{:01X}{:01X}{:01X}{:01X}-{:01X}",
            self.port,
            size_digit,
            self.atmosphere,
            self.hydro,
            self.population,
            self.government,
            self.law_level,
            self.tech_level
        )
    }

    /// Creates a new world from a Traveller UWP string.  
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the world
    /// * `upp` - The Traveller UWP string
    /// * `is_satellite` - Whether this world is a satellite of another body used to just record in the returned world. It does not
    ///   impact parsing of the UWP.
    /// * `is_mainworld` - Whether this is the main world of the system used to just record in the returned world. It does not
    ///   impact parsing of the UWP.
    pub fn from_upp(
        name: String,
        upp: &str,
        is_satellite: bool,
        is_mainworld: bool,
    ) -> Result<World, Box<dyn std::error::Error>> {
        let port = PortCode::from_upp(upp);
        let size = i32::from_str_radix(&upp[1..2], 16)?;
        let atmosphere = i32::from_str_radix(&upp[2..3], 16)?;
        let hydro = i32::from_str_radix(&upp[3..4], 16)?;
        let population = i32::from_str_radix(&upp[4..5], 16)?;
        let government = i32::from_str_radix(&upp[5..6], 16)?;
        let law_level = i32::from_str_radix(&upp[6..7], 16)?;
        let tech_level = i32::from_str_radix(&upp[8..9], 16)?;
        let mut world = World::new(
            name,
            0,
            0,
            size,
            atmosphere,
            hydro,
            population,
            is_satellite,
            is_mainworld,
        );
        world.set_subordinate_stats(port, government, law_level, tech_level, Vec::new());
        Ok(world)
    }

    /// Generates facilities based on the world's characteristics and the main world's characteristics
    ///
    /// # Arguments
    ///
    /// * `system_zones` - Reference to the star system's zone table
    /// * `orbit` - The world's orbital position
    /// * `main_world` - Reference to the system's main world
    pub fn gen_subordinate_facilities(
        &mut self,
        system_zones: &ZoneTable,
        orbit: usize,
        main_world: &World,
    ) {
        // Mining?
        if main_world.trade_classes.contains(&TradeClass::Industrial) && self.population >= 2 {
            self.facilities.push(Facility::Mining);
        }

        // Farming?
        if orbit as i32 == system_zones.habitable
            && orbit as i32 > system_zones.inner
            && self.atmosphere >= 4
            && self.atmosphere <= 9
            && self.hydro >= 4
            && self.hydro <= 8
            && self.population >= 2
        {
            self.facilities.push(Facility::Farming);
        }

        // Colony?
        if self.government == 6 && self.get_population() >= 5 {
            self.facilities.push(Facility::Colony);
        }

        // Research Lab?
        if main_world.population > 0
            && main_world.tech_level > 8
            && roll_2d6()
                + if main_world.tech_level >= 10 { 2 } else { 0 }
                + if self.population == 0 { -2 } else { 0 }
                >= 12
        {
            self.facilities.push(Facility::Lab);
            // Fix tech level if there is a lab.  Not ideal but we need to gen most of a world/satellite
            // before facilities, but tech level is impacted by having a lab.
            if self.tech_level == main_world.tech_level - 1 {
                self.tech_level = main_world.tech_level;
            }
        }

        // Military Base?
        let modifier = if main_world.get_population() >= 8 {
            1
        } else {
            0
        } + if main_world.atmosphere == self.atmosphere {
            2
        } else {
            0
        } + if main_world.facilities.contains(&Facility::Naval)
            || main_world.facilities.contains(&Facility::Scout)
        {
            1
        } else {
            0
        };
        if !main_world.trade_classes.contains(&TradeClass::Poor)
            && self.get_population() > 0
            && roll_2d6() + modifier >= 12
        {
            self.facilities.push(Facility::Military);
        }
    }

    /// Generates subordinate statistics based on the main world's characteristics
    ///
    /// Calculates government, law level, technology level, and starport quality
    /// based on the world's population and the main world's statistics.
    ///
    /// # Arguments
    ///
    /// * `main_world` - Reference to the system's main world
    pub fn gen_subordinate_stats(&mut self, main_world: &World) {
        let population = self.get_population();
        let modifier = if main_world.government == 6 {
            population
        } else if main_world.government >= 7 {
            1
        } else {
            0
        };

        let government = if population <= 0 {
            0
        } else {
            match roll_1d6() + modifier {
                1 => 0,
                2 => 1,
                3 => 2,
                4 => 3,
                _ => 6,
            }
        };

        let law_level = if population <= 0 {
            0
        } else {
            (roll_1d6() - 3 + main_world.law_level).max(0)
        };

        let tech_level = if population <= 0 {
            0
        } else if population > 0
            && ![5, 6, 8].contains(&self.atmosphere)
            && main_world.tech_level <= 7
        {
            7
        } else {
            (main_world.tech_level - 1).max(0)
        };

        let roll = roll_1d6()
            + match population {
                0 => -3,
                1 => -2,
                2..=5 => 0,
                _ => 2,
            };

        let port = match roll {
            -2..=2 => PortCode::Y,
            3 => PortCode::H,
            4..=5 => PortCode::G,
            _ => PortCode::F,
        };
        self.set_subordinate_stats(port, government, law_level, tech_level, Vec::new());
    }

    /// Returns a copy of the world's trade classifications
    pub fn get_trade_classes(&self) -> Vec<TradeClass> {
        self.trade_classes.clone()
    }

    /// Generates trade classifications based on the world's UWP characteristics
    ///
    /// Analyzes the world's atmosphere, hydrographics, population, size, and other
    /// factors to determine applicable trade classifications like Agricultural,
    /// Non-Agricultural, Industrial, etc.
    pub fn gen_trade_classes(&mut self) {
        self.trade_classes = Vec::new();
        if self.atmosphere >= 4
            && self.atmosphere <= 9
            && self.hydro >= 4
            && self.hydro <= 8
            && self.population >= 5
            && self.population <= 7
        {
            self.trade_classes.push(TradeClass::Agricultural);
        }
        if self.atmosphere <= 3 && self.hydro <= 3 && self.population >= 6 {
            self.trade_classes.push(TradeClass::NonAgricultural);
        }

        if self.size == 0 && self.atmosphere == 0 && self.hydro == 0 && self.is_mainworld {
            self.trade_classes.push(TradeClass::Asteroid);
        }

        if self.population == 0 && self.government == 0 && self.law_level == 0 {
            self.trade_classes.push(TradeClass::Barren);
        }

        if self.atmosphere >= 10 && self.hydro >= 1 {
            self.trade_classes.push(TradeClass::FluidOceans);
        }
        if (6..=8).contains(&self.size)
            && [5, 6, 8].contains(&self.atmosphere)
            && (5..=7).contains(&self.hydro)
        {
            self.trade_classes.push(TradeClass::Garden);
        }
        if self.population >= 9 {
            self.trade_classes.push(TradeClass::HighPopulation);
        }
        if self.tech_level >= 12 {
            self.trade_classes.push(TradeClass::HighTech);
        }
        if [0, 1, 2, 4, 7, 9].contains(&self.atmosphere) && self.population >= 9 {
            self.trade_classes.push(TradeClass::Industrial);
        }
        if (1..=6).contains(&self.population) {
            self.trade_classes.push(TradeClass::NonIndustrial);
        }

        if self.population >= 1 && self.tech_level <= 5 {
            self.trade_classes.push(TradeClass::LowTech);
        }

        if [6, 8].contains(&self.atmosphere)
            && [6, 7, 8].contains(&self.population)
            && self.government >= 4
            && self.government <= 9
        {
            self.trade_classes.push(TradeClass::Rich);
        }

        if self.population > 0 && self.atmosphere >= 2 && self.atmosphere <= 5 && self.hydro <= 3 {
            self.trade_classes.push(TradeClass::Poor);
        }
        if self.hydro >= 10 {
            self.trade_classes.push(TradeClass::WaterWorld);
        }
        if self.hydro <= 0 && self.atmosphere > 1 {
            self.trade_classes.push(TradeClass::Desert);
        }

        if self.atmosphere <= 1 && self.hydro >= 10 {
            self.trade_classes.push(TradeClass::IceCapped);
        }
        if self.atmosphere <= 0 && self.population > 1 {
            self.trade_classes.push(TradeClass::Vacuum);
        }
    }

    /// Sets the facilities present on the world
    ///
    /// # Arguments
    ///
    /// * `facilities` - Vector of facilities to set
    #[allow(dead_code)]
    pub fn set_facilities(&mut self, facilities: Vec<Facility>) {
        self.facilities = facilities;
    }

    /// Returns a formatted string of all facilities on the world
    pub fn facilities_string(&self) -> String {
        self.facilities
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    }

    /// Returns a formatted string of all trade classifications
    pub fn trade_classes_string(&self) -> String {
        let res = self
            .trade_classes
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        res
    }

    /// Computes astronomical data for the world based on its star
    ///
    /// # Arguments
    ///
    /// * `star` - Reference to the star this world orbits
    pub fn compute_astro_data(&mut self, star: &Star) {
        let astro = AstroData::compute(star, self);
        self.astro_data = astro;
    }

    /// Returns a formatted description of the world's astronomical characteristics
    pub fn get_astro_description(&self) -> String {
        self.astro_data.get_astro_description(self)
    }

    pub fn generate(star: &Star, orbit: usize, main_world: &World) -> World {
        let mut modifier = if orbit == 0 {
            -5
        } else if orbit == 1 {
            -4
        } else if orbit == 2 {
            -2
        } else {
            0
        };

        if star.star_type == StarType::M {
            modifier -= 2;
        }

        let size = (roll_2d6() - 2 + modifier).min(0);

        let roll = roll_2d6();
        let signed_orbit = orbit as i32;
        let mut atmosphere = (roll_2d6() - 7
            + size
            + if signed_orbit <= get_zone(star).inner {
                -2
            } else {
                0
            }
            + if signed_orbit > get_zone(star).habitable {
                -2
            } else {
                0
            })
        .clamp(0, 10);

        // Special case for a type A atmosphere. Possible if 2 zones out
        // or more from the habitable zone.
        if roll == 12 && signed_orbit > get_zone(star).habitable + 1 {
            atmosphere = 10;
        }

        let mut hydro = (roll_2d6() - 7
            + size
            + if signed_orbit > get_zone(star).habitable {
                -4
            } else {
                0
            }
            + if atmosphere <= 1 || atmosphere >= 10 {
                -4
            } else {
                0
            })
        .clamp(0, 10);
        if size <= 0 || signed_orbit <= get_zone(star).inner {
            hydro = 0;
        }

        let population = (roll_2d6() - 2
            + if signed_orbit <= get_zone(star).inner {
                -5
            } else {
                0
            }
            + if signed_orbit > get_zone(star).habitable {
                -5
            } else {
                0
            }
            + if ![5, 6, 8].contains(&atmosphere) {
                -2
            } else {
                0
            })
        .clamp(0, main_world.get_population() - 1);

        let mut world = World::new(
            "Unknown".to_string(),
            orbit,
            orbit,
            size,
            atmosphere,
            hydro,
            population,
            false,
            false,
        );
        world.gen_name(&main_world.name, orbit);
        world.gen_subordinate_stats(main_world);
        world.gen_trade_classes();
        world.gen_subordinate_facilities(&get_zone(star), orbit, main_world);
        world
    }
}

/// Implements Display for World.  Used extensively in displaying the output from the app.
impl Display for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{:<7}{:<24}{:<12}{:<18}",
            self.orbit,
            self.name,
            self.to_uwp(),
            self.facilities_string()
        )?;
        for satellite in self.satellites.sats.iter() {
            writeln!(f, "\t{satellite}")?;
        }
        Ok(())
    }
}

/// Implements the HasSatellites trait for World.  Worlds and GasGiants can both have satellites.
impl HasSatellites for World {
    /// Returns the number of satellites orbiting this world
    fn get_num_satellites(&self) -> usize {
        self.satellites.sats.len()
    }

    /// Returns a mutable reference to the satellites
    fn get_satellites_mut(&mut self) -> &mut Satellites {
        &mut self.satellites
    }

    /// Adds a satellite onto the world.  Note that the orbit of the satellite is stored in the satellite description
    /// and satellites may be added out of order.  This method also does not check for duplicate orbits.
    fn push_satellite(&mut self, satellite: World) {
        self.satellites.sats.push(satellite);
    }

    /// Returns a reference to a satellite by its orbit
    ///
    /// # Arguments
    ///
    /// * `orbit` - Orbit of the satellite to return
    ///
    /// # Returns
    ///
    /// * `Option<&World>` - Reference to the satellite if it exists. None otherwise.
    fn get_satellite(&self, orbit: usize) -> Option<&World> {
        self.satellites.sats.iter().find(|&x| x.orbit == orbit)
    }

    fn gen_satellite_orbit(&self, is_ring: bool) -> usize {
        let mut orbit: usize = if is_ring {
            match roll_1d6() {
                1..=3 => 1,
                4..=5 => 2,
                6 => 3,
                _ => unreachable!(),
            }
        } else {
            let orbit_type_roll = roll_2d6() - self.get_num_satellites() as i32;
            debug!(
                "(World.gen_satellite_orbit) Orbit type roll is {}. num_sat = {}",
                orbit_type_roll,
                self.get_num_satellites()
            );
            if orbit_type_roll <= 7 {
                // Close orbit
                (roll_2d6() + 3) as usize
            } else {
                // Far orbit
                ((roll_2d6() + 3) * 5) as usize
            }
        };

        while self.get_satellite(orbit).is_some() {
            debug!("(World.gen_satellite_orbit) Having to bump orbit up by 1.");
            orbit += 1;
        }

        debug!("(World.gen_satellite_orbit) Orbit is {orbit}");
        orbit
    }

    fn determine_num_satellites(&self) -> i32 {
        if self.size <= 0 {
            0
        } else {
            (roll_1d6() - 3).max(0)
        }
    }

    fn gen_satellite(&mut self, system_zones: &ZoneTable, main_world: &World, star: &Star) {
        // Anything less than 0 is size S; make them all -1 to keep it
        // straightforward.
        let size = (self.size - roll_1d6()).max(-1);

        let orbit = self.gen_satellite_orbit(size == 0);

        // Size 0 is a ring so nothing else can be 0.
        if size == 0 {
            let mut ring = World::new(
                "Ring System".to_string(),
                orbit,
                self.position_in_system,
                0,
                0,
                0,
                0,
                true,
                false,
            );
            ring.port = PortCode::Y;
            self.satellites.sats.push(ring);
            return;
        }

        let roll = roll_2d6();
        let mut atmosphere = (roll - 7
            + size
            + if orbit as i32 <= system_zones.inner || orbit as i32 > system_zones.habitable {
                -4
            } else {
                0
            })
        .clamp(0, 10);

        // Special case where size is 1 or less.
        if size <= 1 {
            atmosphere = 0;
        }

        // Special case for a type A atmosphere.
        if roll == 12 && orbit as i32 > system_zones.habitable {
            atmosphere = 10;
        }

        let mut hydro = (roll_2d6() - 7
            + size
            + if orbit as i32 > system_zones.habitable {
                -4
            } else {
                0
            }
            + if atmosphere <= 1 || atmosphere >= 10 {
                -4
            } else {
                0
            })
        .clamp(0, 10);

        if size <= 0 || orbit as i32 <= system_zones.inner {
            hydro = 0;
        }

        let population = (roll_2d6() - 2
            + if orbit as i32 <= system_zones.inner {
                -5
            } else if orbit as i32 > system_zones.habitable {
                -4
            } else {
                0
            }
            + if ![5, 6, 8].contains(&atmosphere) {
                -2
            } else {
                0
            })
        .clamp(0, 10);

        let satellite_name = gen_moon_name();
        let mut satellite = World::new(
            satellite_name,
            orbit,
            self.position_in_system,
            size,
            atmosphere,
            hydro,
            population,
            true,
            false,
        );
        satellite.gen_subordinate_stats(main_world);
        satellite.gen_trade_classes();
        satellite.gen_subordinate_facilities(system_zones, orbit, main_world);
        satellite.compute_astro_data(star);
        self.satellites.sats.push(satellite);
    }
}

impl Display for Facility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Facility::Naval => write!(f, "Naval"),
            Facility::Military => write!(f, "Military"),
            Facility::Scout => write!(f, "Scout"),
            Facility::Farming => write!(f, "Farming"),
            Facility::Mining => write!(f, "Mining"),
            Facility::Colony => write!(f, "Colony"),
            Facility::Lab => write!(f, "Lab"),
        }
    }
}
