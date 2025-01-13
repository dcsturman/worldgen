use log::debug;
use reactive_stores::Store;
use std::fmt::Display;

use crate::astro::AstroData;
use crate::has_satellites::HasSatellites;
use crate::name_tables::{gen_moon_name, gen_planet_name};
use crate::system::Star;
use crate::system_tables::ZoneTable;
use crate::util::{arabic_to_roman, roll_1d6, roll_2d6};

#[derive(Debug, Clone, Store)]
pub struct World {
    pub name: String,
    pub orbit: usize,
    pub(crate) position_in_system: usize,
    is_satellite: bool,
    is_mainworld: bool,
    port: PortCode,
    pub(crate) size: i32,
    pub(crate) atmosphere: i32,
    pub(crate) hydro: i32,
    population: i32,
    law_level: i32,
    government: i32,
    tech_level: i32,
    facilities: Vec<Facility>,
    pub satellites: Satellites,
    trade_classes: Vec<TradeClass>,
    astro_data: AstroData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortCode {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeClass {
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

#[derive(Debug, Clone, Store)]
pub struct Satellites {
    #[store(key: String = |world| world.name.clone())]
    pub sats: Vec<World>,
}

impl World {
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
            astro_data: AstroData::new(),
        }
    }
    pub fn gen_name(&mut self, system_name: &str, orbit: usize) {
        if self.population > 0 {
            self.name = gen_planet_name()
        } else {
            self.name = format!("{} {}", system_name, arabic_to_roman(orbit + 1))
        }
    }

    pub fn get_population(&self) -> i32 {
        self.population
    }

    pub(crate) fn set_port(&mut self, port: PortCode) {
        self.port = port;
    }

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
    pub fn to_upp(&self) -> String {
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
    pub fn from_upp(name: String, upp: &str, is_satellite: bool, is_mainworld: bool) -> World {
        let port = PortCode::from_upp(upp);
        let size = i32::from_str_radix(&upp[1..2], 16).unwrap();
        let atmosphere = i32::from_str_radix(&upp[2..3], 16).unwrap();
        let hydro = i32::from_str_radix(&upp[3..4], 16).unwrap();
        let population = i32::from_str_radix(&upp[4..5], 16).unwrap();
        let government = i32::from_str_radix(&upp[5..6], 16).unwrap();
        let law_level = i32::from_str_radix(&upp[6..7], 16).unwrap();
        let tech_level = i32::from_str_radix(&upp[8..9], 16).unwrap();
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
        world
    }

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

    pub fn gen_trade_classes(&mut self) {
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
        if [0, 1, 2, 4, 7, 9].contains(&self.atmosphere) && self.population >= 9 {
            self.trade_classes.push(TradeClass::Industrial);
        }
        if (1..=6).contains(&self.population) {
            self.trade_classes.push(TradeClass::NonIndustrial);
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
            self.trade_classes.push(TradeClass::DesertWorld);
        }

        if self.atmosphere <= 1 && self.hydro >= 10 {
            self.trade_classes.push(TradeClass::Icecapped);
        }
        if self.atmosphere <= 0 && self.population > 1 {
            self.trade_classes.push(TradeClass::VacuumWorld);
        }
    }

    #[allow(dead_code)]
    pub fn set_facilities(&mut self, facilities: Vec<Facility>) {
        self.facilities = facilities;
    }

    pub fn facilities_string(&self) -> String {
        self.facilities
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    }

    pub fn trade_classes_string(&self) -> String {
        self.trade_classes
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    }

    pub fn compute_astro_data(&mut self, star: &Star) {
        let astro = AstroData::compute(star, self);
        self.astro_data = astro;
    }

    pub fn get_astro_description(&self) -> String {
        self.astro_data.describe(self)
    }
    
}

impl Display for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{:<7}{:<24}{:<12}{:<18}",
            self.orbit,
            self.name,
            self.to_upp(),
            self.facilities_string()
        )?;
        for satellite in self.satellites.sats.iter() {
            writeln!(f, "\t{}", satellite)?;
        }
        Ok(())
    }
}

impl HasSatellites for World {
    fn get_num_satellites(&self) -> usize {
        self.satellites.sats.len()
    }

    fn get_satellites_mut(&mut self) -> &mut Satellites {
        &mut self.satellites
    }

    fn push_satellite(&mut self, satellite: World) {
        self.satellites.sats.push(satellite);
    }

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

        debug!("(World.gen_satellite_orbit) Orbit is {}", orbit);
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

impl PortCode {
    pub fn from_upp(upp: &str) -> PortCode {
        match upp.chars().next() {
            Some('A') => PortCode::A,
            Some('B') => PortCode::B,
            Some('C') => PortCode::C,
            Some('D') => PortCode::D,
            Some('E') => PortCode::E,
            Some('X') => PortCode::X,
            _ => PortCode::A,
        }
    }
}

impl Display for PortCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortCode::A => write!(f, "A"),
            PortCode::B => write!(f, "B"),
            PortCode::C => write!(f, "C"),
            PortCode::D => write!(f, "D"),
            PortCode::E => write!(f, "E"),
            PortCode::X => write!(f, "X"),
            PortCode::Y => write!(f, "Y"),
            PortCode::H => write!(f, "H"),
            PortCode::G => write!(f, "G"),
            PortCode::F => write!(f, "F"),
        }
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

impl Display for TradeClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeClass::Agricultural => write!(f, "Ag"),
            TradeClass::NonAgricultural => write!(f, "Na"),
            TradeClass::Industrial => write!(f, "In"),
            TradeClass::NonIndustrial => write!(f, "Ni"),
            TradeClass::Rich => write!(f, "Ri"),
            TradeClass::Poor => write!(f, "Po"),
            TradeClass::WaterWorld => write!(f, "Wa"),
            TradeClass::DesertWorld => write!(f, "De"),
            TradeClass::Icecapped => write!(f, "Ic"),
            TradeClass::VacuumWorld => write!(f, "Va"),
        }
    }
}
