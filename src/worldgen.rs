mod name_tables;
mod system_tables;

use crate::name_tables::{MOON_NAMES, STAR_SYSTEM_NAMES};
use crate::system_tables::{get_zone, ZoneTable};
use log::{debug, error, warn};
use rand::seq::SliceRandom;
use rand::Rng;
use std::fmt::Display;

// Traits

trait HasSatellites {
    fn get_position_in_system(&self) -> usize;
    fn get_num_satellites(&self) -> usize;
    fn get_satellite(&self, orbit: usize) -> Option<&World>;
    fn get_satellites_mut(&mut self) -> &mut Satellites;
    fn sort_satellites(&mut self) {
        self.get_satellites_mut()
            .sort_by(|a, b| a.orbit.cmp(&b.orbit));
    }

    fn clean_satellites(&mut self) {
        self.sort_satellites();
        let ring_indices: Vec<usize> = self
            .get_satellites_mut()
            .iter()
            .enumerate()
            .filter(|(_, satellite)| satellite.size == 0)
            .map(|(index, _)| index)
            .collect();

        if ring_indices.len() > 0 {
            for i in 1..ring_indices.len() {
                self.get_satellites_mut().remove(ring_indices[i]);
            }
            self.get_satellites_mut()[ring_indices[0]].name = "Ring System".to_string();
        }
    }

    fn determine_num_satellites(&self) -> i32;

    fn gen_satellite_orbit(&self, is_ring: bool) -> usize;

    fn gen_satellite(&mut self, system_zones: &ZoneTable, main_world: &World);
}

// Enums
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub enum StarType {
    O,
    B,
    A,
    F,
    G,
    K,
    M,
}

pub type StarSubType = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub enum StarSize {
    Ia,
    Ib,
    II,
    III,
    IV,
    V,
    VI,
    D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarOrbit {
    Primary,
    Far,
    System(usize),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasGiantSize {
    Small,
    Large,
}

// Structs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Star {
    star_type: StarType,
    subtype: StarSubType,
    size: StarSize,
}
#[derive(Debug)]
pub struct System {
    name: String,
    star: Star,
    secondary: Option<Box<System>>,
    tertiary: Option<Box<System>>,
    orbit: StarOrbit,
    orbit_slots: Vec<Option<OrbitContent>>,
}

#[derive(Debug, Clone)]
pub enum OrbitContent {
    // This orbit contains the secondary star system of the primary.
    Secondary,
    // This orbit contains the tertiary star system of the primary.
    Tertiary,
    // This orbit contains a world
    World(World),
    // This orbit contains a gas giant
    GasGiant(GasGiant),
    // This orbit is intentionally empty and cannot be filled.
    Blocked,
}

type Satellites = Vec<World>;

#[derive(Debug, Clone)]
pub struct World {
    name: String,
    orbit: usize,
    position_in_system: usize,
    is_satellite: bool,
    is_mainworld: bool,
    port: PortCode,
    size: i32,
    atmosphere: i32,
    hydro: i32,
    population: i32,
    law_level: i32,
    government: i32,
    tech_level: i32,
    facilities: Vec<Facility>,
    satellites: Satellites,
    trade_classes: Vec<TradeClass>,
    // astro_data: AstroData,
}

#[derive(Debug, Clone)]
pub struct GasGiant {
    name: String,
    size: GasGiantSize,
    satellites: Satellites,
    orbit: usize,
}

impl System {
    pub fn new(
        star_type: StarType,
        subtype: StarSubType,
        size: StarSize,
        orbit: StarOrbit,
        max_orbits: usize,
    ) -> System {
        let mut rng = rand::thread_rng();
        System {
            name: STAR_SYSTEM_NAMES[rng.gen_range(0..STAR_SYSTEM_NAMES.len())].to_string(),
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
    pub fn set_max_orbits(&mut self, max_orbits: usize) {
        self.orbit_slots.resize(max_orbits, None);
    }

    pub fn get_max_orbits(&self) -> usize {
        self.orbit_slots.len()
    }

    pub fn is_slot_empty(&self, orbit: usize) -> bool {
        self.orbit_slots.get(orbit as usize).is_none()
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
            self.orbit_slots.resize(orbit + 1, None);
        }
        self.orbit_slots[orbit] = Some(content);
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
        write!(f, " Zones for {} are {:?}. ", self.name, get_zone(self))?;
        if let Some(secondary) = &self.secondary {
            if let StarOrbit::Primary = secondary.orbit {
                write!(f, "It has a secondary contact star {}:\n{}\n", secondary.name, secondary)?;
            } else {
                write!(f, "It has a secondary star {} which is a {} star in {}.\n", secondary.name, secondary.star, secondary.orbit)?;
            }
        }
        if let Some(tertiary) = &self.tertiary {
            if let StarOrbit::Primary = tertiary.orbit {
                write!(f, " It has a tertiary contact star {}:\n{}\n", tertiary.name, tertiary)?;
            } else {
                write!(f, " It has a tertiary star {} which is a {} star in {}.\n", tertiary.name, tertiary.star, tertiary.orbit)?;
            }
        }

        if self.orbit_slots.iter().enumerate().filter(|(index, _)| !self.is_slot_empty(*index)).count() > 0 {
            write!(f, "\n{:<7}{:<24}{:<12}{:<18}\n", "Orbit","Name", "UPP", "Remarks")?;
        }

        for body in self.orbit_slots.iter() {
            match body {
                Some(OrbitContent::Secondary) => {
                    if let Some(secondary) = &self.secondary {
                        if let StarOrbit::System(orbit) = secondary.orbit {
                                write!(f, "{:<7}{:<24}{:<12}\n", orbit, secondary.name, secondary.star)?;
                        }
                    }
                }
                Some(OrbitContent::Tertiary) => {
                    if let Some(tertiary) = &self.tertiary {
                        if let StarOrbit::System(orbit) = tertiary.orbit {
                                write!(f, "{:<7}{:<24}{:<12}\n", orbit, tertiary.name, tertiary.star)?;
                        }
                    }
                }
                Some(OrbitContent::World(world)) => {
                    write!(f, "{}\n", world)?;
                }
                Some(OrbitContent::GasGiant(gas_giant)) => {
                    write!(f, "{}\n", gas_giant)?;
                }
                Some(OrbitContent::Blocked) | None => {
                }
            }
        }

        if let Some(secondary) = &self.secondary {
            write!(f, "\n{}\n", secondary)?;
        }

        if let Some(tertiary) = &self.tertiary {
            write!(f, "\n{}\n", tertiary)?;
        }
        Ok(())
    }
}

impl World {
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
            satellites: Vec::new(),
            trade_classes: Vec::new(),
        }
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
            self.port.to_string(),
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

    pub fn get_population(&self) -> i32 {
        self.population
    }

    pub fn set_facilities(&mut self, facilities: Vec<Facility>) {
        self.facilities = facilities;
    }

    pub fn get_facilities(&self) -> Vec<Facility> {
        self.facilities.clone()
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

    pub fn compute_astro_data(&mut self, _system: &System) {
        // self.astro_data.compute(system, self);
        unimplemented!("compute_astro_data");
    }
}

impl Display for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:<7}{:<24}{:<12}{:<18}\n",
            self.orbit,
            self.name,
            self.to_upp(),
            self.facilities_string()
        )?;
        for satellite in self.satellites.iter() {
            write!(f, "\t{}\n", satellite)?;
        }
        Ok(())
    }
}

impl Display for GasGiantSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GasGiantSize::Small => write!(f, "Small GG"),
            GasGiantSize::Large => write!(f, "Large GG"),
        }
    }
}

impl Display for GasGiant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:<7}{:<24}{:<12}\n", self.orbit, self.name, self.size)?;
        for satellite in self.satellites.iter() {
            write!(f, "\t{}\n", satellite)?;
        }
        Ok(())
    }
}

impl HasSatellites for World {
    fn get_num_satellites(&self) -> usize {
        self.satellites.len()
    }

    fn get_satellites_mut(&mut self) -> &mut Satellites {
        &mut self.satellites
    }

    fn get_satellite(&self, orbit: usize) -> Option<&World> {
        self.satellites.iter().find(|&x| x.orbit == orbit)
    }

    fn get_position_in_system(&self) -> usize {
        self.position_in_system
    }

    fn gen_satellite_orbit(&self, is_ring: bool) -> usize {
        let mut orbit: usize = if is_ring {
            match roll_1d6() {
                1 | 2 | 3 => 1,
                4 | 5 => 2,
                6 => 3,
                _ => unreachable!(),
            }
        } else {
            let orbit_type_roll = roll_2d6() - self.get_num_satellites() as i32;
            debug!("(World.gen_satellite_orbit) Orbit type roll is {}. num_sat = {}", orbit_type_roll, self.get_num_satellites());
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


    fn gen_satellite(&mut self, system_zones: &ZoneTable, main_world: &World) {
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
            self.satellites.push(ring);
            return;
        }

        let roll = roll_2d6();
        let mut atmosphere = (roll - 7
            + size
            + if orbit as i32 <= system_zones.inner || orbit as i32 > system_zones.habitable {
                -4
            } else {
                0
            }).clamp(0, 10);        
        
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
            }).clamp(0, 10);

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
            }).clamp(0, 10);

        let satellite_name =
            MOON_NAMES[rand::thread_rng().gen_range(0..MOON_NAMES.len())].to_string();
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
        gen_subordinate_stats(&mut satellite, main_world);
        gen_subordinate_facilities(system_zones, &mut satellite, orbit, main_world);
        // TODO: Astro data
        //satellite.compute_astro_data(system);
        self.satellites.push(satellite);
    }
}
impl GasGiant {
    pub fn new(name: String, size: GasGiantSize, orbit: usize) -> GasGiant {
        GasGiant {
            name,
            size,
            satellites: Vec::new(),
            orbit,
        }
    }
}

impl HasSatellites for GasGiant {
    fn get_num_satellites(&self) -> usize {
        self.satellites.len()
    }

    fn get_satellite(&self, orbit: usize) -> Option<&World> {
        self.satellites.iter().find(|&x| x.orbit == orbit)
    }

    fn get_satellites_mut(&mut self) -> &mut Satellites {
        &mut self.satellites
    }

    fn get_position_in_system(&self) -> usize {
        self.orbit
    }

    fn gen_satellite_orbit(&self, is_ring: bool) -> usize {
        let mut orbit: usize = if is_ring {
            match roll_1d6() {
                1 | 2 | 3 => 1,
                4 | 5 => 2,
                6 => 3,
                _ => unreachable!(),
            }
        } else {
            let orbit_type_roll = roll_2d6() - self.get_num_satellites() as i32;
            if orbit_type_roll <= 7 {
                // Close orbit
                (roll_2d6() + 3) as usize
            } else if orbit_type_roll == 12 {
                // Extreme orbit: only possible in Gas Giants
                ((roll_2d6() + 3) * 25) as usize
            } else {
                // Far orbit
                ((roll_2d6() + 3) * 5) as usize
            }
        };

        while self.get_satellite(orbit).is_some() {
            debug!("(GasGiant.gen_satellite_orbit) Having to bump orbit up by 1.");
            orbit += 1;
        }
        debug!("(GasGiant.gen_satellite_orbit) Orbit is {}", orbit);
        orbit
    }

    fn determine_num_satellites(&self) -> i32 {
        match self.size {
            GasGiantSize::Small => (roll_2d6() - 4).max(0),
            GasGiantSize::Large => roll_2d6(),
        }
    }


    fn gen_satellite(&mut self, system_zones: &ZoneTable, main_world: &World) {
        // Anything less than 0 is size S; make them all -1 to keep it
        // straightforward
        let size = (match self.size {
            GasGiantSize::Small => roll_2d6() - 6,
            GasGiantSize::Large => roll_2d6() - 4,
        })
        .max(-1);

        let orbit = self.gen_satellite_orbit(size == 0);

        // Size 0 is a ring so nothing else can be 0.
        if size == 0 {
            let mut ring = World::new(
                "Ring System".to_string(),
                orbit,
                self.orbit,
                0,
                0,
                0,
                0,
                true,
                false,
            );
            ring.port = PortCode::Y;
            self.satellites.push(ring);
            return;
        }

        let roll = roll_2d6();
        let mut atmosphere = (roll - 7
            + size
            + if orbit as i32 <= system_zones.inner || orbit as i32 > system_zones.habitable {
                -4
            } else {
                0
            }).clamp(0, 10);        
        
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
            }).clamp(0, 10);

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
            }).clamp(0, 10);

        let satellite_name =
            MOON_NAMES[rand::thread_rng().gen_range(0..MOON_NAMES.len())].to_string();
        let mut satellite = World::new(
            satellite_name,
            orbit,
            self.orbit,
            size,
            atmosphere,
            hydro,
            population,
            true,
            false,
        );

        gen_subordinate_stats(&mut satellite, main_world);
        gen_subordinate_facilities(system_zones, &mut satellite, self.orbit, main_world);
        // TODO: Astro data
        //satellite.compute_astro_data(system);
        self.satellites.push(satellite);
    }
}

impl Display for StarType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Display for StarSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Display for Star {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{} {}", self.star_type, self.subtype as usize, self.size)
    }
}

impl Display for StarOrbit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StarOrbit::Primary => write!(f, "close orbit"),
            StarOrbit::Far => write!(f, "far orbit"),
            StarOrbit::System(orbit) => write!(f, "orbit {}", orbit),
        }
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
    pub fn to_string(&self) -> String {
        match self {
            PortCode::A => "A".to_string(),
            PortCode::B => "B".to_string(),
            PortCode::C => "C".to_string(),
            PortCode::D => "D".to_string(),
            PortCode::E => "E".to_string(),
            PortCode::X => "X".to_string(),
            PortCode::Y => "Y".to_string(),
            PortCode::H => "H".to_string(),
            PortCode::G => "G".to_string(),
            PortCode::F => "F".to_string(),
        }
    }
}

impl Facility {
    pub fn to_string(&self) -> String {
        match self {
            Facility::Naval => "Naval Base".to_string(),
            Facility::Scout => "Scout Base".to_string(),
            Facility::Farming => "Farming".to_string(),
            Facility::Mining => "Mining".to_string(),
            Facility::Colony => "Colony".to_string(),
            Facility::Lab => "Research Lab".to_string(),
            Facility::Military => "Military Base".to_string(),
        }
    }
}

impl TradeClass {
    pub fn to_string(&self) -> String {
        match self {
            TradeClass::Agricultural => "Agricultural".to_string(),
            TradeClass::NonAgricultural => "Non-Agricultural".to_string(),
            TradeClass::Industrial => "Industrial".to_string(),
            TradeClass::NonIndustrial => "Non-Industrial".to_string(),
            TradeClass::Rich => "Rich".to_string(),
            TradeClass::Poor => "Poor".to_string(),
            TradeClass::WaterWorld => "Water World".to_string(),
            TradeClass::DesertWorld => "Desert World".to_string(),
            TradeClass::VacuumWorld => "Vacuum World".to_string(),
            TradeClass::Icecapped => "Icecapped".to_string(),
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
        _ => StarType::F,
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

fn gen_companion_system(
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
            let mut secondary: Box<System> = Box::new(gen_companion_system(
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

fn empty_orbits_near_companion(system: &mut System, orbit: usize) {
    for i in (orbit / 2 + 1)..orbit {
        system.set_orbit_slot(i, OrbitContent::Blocked);
    }
    system.set_orbit_slot(orbit + 1, OrbitContent::Blocked);
    system.set_orbit_slot(orbit + 2, OrbitContent::Blocked);
}

fn gen_blocked_orbits(system: &mut System) {
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

    let valid_orbits = system.get_unused_orbits();

    for _ in 0..num_empty {
        if let Some(pos) = valid_orbits.choose(&mut rand::thread_rng()) {
            system.set_orbit_slot(*pos, OrbitContent::Blocked);
        }
    }
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
    system.set_max_orbits(gen_max_orbits(&system.star));

    // Do this for a secondary, which we have with 2 or 3 stars.
    if num_stars >= 2 {
        let orbit = gen_companion_orbit(roll_2d6());
        match orbit {
            StarOrbit::Primary | StarOrbit::Far => {
                system.secondary = Some(Box::new(gen_companion_system(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                )));
            }
            // If the companion has an orbit, but its inside the primary star, just treat it as the primary orbit.
            StarOrbit::System(position) if position as i32 <= get_zone(&system).inside => {
                system.secondary = Some(Box::new(gen_companion_system(
                    primary_type_roll,
                    primary_size_roll,
                    StarOrbit::Primary,
                )));
            }
            StarOrbit::System(position) => {
                system.secondary = Some(Box::new(gen_companion_system(
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
                system.tertiary = Some(Box::new(gen_companion_system(
                    primary_type_roll,
                    primary_size_roll,
                    orbit,
                )));
            }
            StarOrbit::System(position) if position as i32 <= get_zone(&system).inside => {
                system.tertiary = Some(Box::new(gen_companion_system(
                    primary_type_roll,
                    primary_size_roll,
                    StarOrbit::Primary,
                )));
            }
            StarOrbit::System(position) => {
                system.tertiary = Some(Box::new(gen_companion_system(
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
        .filter(|&body| body.is_none())
        .count() as i32
}

fn gen_gas_giants(system: &mut System) -> i32 {
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

    num_giants = num_giants.min(count_open_orbits(system));
    let original_num_giants = num_giants;

    let habitable = get_zone(system).habitable;

    let mut viable_outer_orbits: Vec<i32> = system
        .orbit_slots
        .iter()
        .enumerate()
        .filter_map(|(index, body)| {
            if body.is_none() {
                if habitable <= 0 || index as i32 > habitable {
                    Some(index as i32)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    let mut viable_inner_orbits: Vec<i32> = system
        .orbit_slots
        .iter()
        .enumerate()
        .filter_map(|(index, body)| {
            if body.is_none() {
                if habitable <= 0 || index as i32 <= habitable {
                    Some(index as i32)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    while viable_outer_orbits.len() + viable_inner_orbits.len() > 0 && num_giants > 0 {
        let orbit = if viable_outer_orbits.len() > 0 {
            let pos = rand::thread_rng().gen_range(0..viable_outer_orbits.len());
            viable_outer_orbits.remove(pos);
            pos
        } else {
            let pos = rand::thread_rng().gen_range(0..viable_inner_orbits.len());
            viable_inner_orbits.remove(pos);
            pos
        };

        if roll_1d6() <= 3 {
            system.set_orbit_slot(
                orbit,
                OrbitContent::GasGiant(GasGiant::new(
                    gen_planet_name(system, orbit),
                    GasGiantSize::Small,
                    orbit,
                )),
            );
        } else {
            system.set_orbit_slot(
                orbit,
                OrbitContent::GasGiant(GasGiant::new(
                    gen_planet_name(system, orbit),
                    GasGiantSize::Large,
                    orbit,
                )),
            );
        }
        num_giants -= 1;
    }

    if num_giants > 0 {
        error!(
            "Not enough orbits for gas giants. Need {} in system {:?}",
            original_num_giants, system
        );
    }
    original_num_giants - num_giants
}

fn gen_planetoids(system: &mut System, num_giants: i32, main_world: &World) {
    if roll_2d6() >= 7 {
        // No planetoids in system
        return;
    }
    let mut num_planetoids = match roll_2d6() - num_giants {
        1..=3 => 3,
        4..=6 => 2,
        _ => 1,
    };
    let mut viable_giants: Vec<usize> = system
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

    let mut viable_other_orbits: Vec<usize> = system
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
        let orbit = if viable_giants.len() > 0 {
            let pos = rand::thread_rng().gen_range(0..viable_giants.len());
            viable_giants.remove(pos);
            pos
        } else {
            let pos = rand::thread_rng().gen_range(0..viable_other_orbits.len());
            viable_other_orbits.remove(pos);
            pos
        };

        let population = (roll_2d6() - 2
            + if orbit as i32 <= get_zone(system).inner {
                -5
            } else {
                0
            }
            + if orbit as i32 > get_zone(system).habitable {
                -5
            } else {
                0
            })
        .clamp(0, main_world.population - 1);

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

        gen_subordinate_stats(&mut planetoid, main_world);
        gen_subordinate_facilities(&get_zone(system), &mut planetoid, orbit, main_world);
        system.set_orbit_slot(orbit, OrbitContent::World(planetoid));
        num_planetoids -= 1;
    }
}

fn place_main_world(system: &mut System, mut main_world: World) {
    let requires_habitable =
        main_world.atmosphere > 1 && main_world.atmosphere < 10 && main_world.size > 0;
    let mut habitable = get_zone(system).habitable;
    if (habitable < 0 || habitable == get_zone(system).inner) && requires_habitable {
        warn!("No habitable zone for main world for system: {:?}. Habitable = {}. Inner = {}. Using orbit 0.", system, habitable, get_zone(system).inner);
        habitable = get_zone(system).inner.max(0);
    }

    debug!("(place_main_world) habitable = {}, max_orbits = {}, requires_habitable = {}, star = {}", habitable, system.get_max_orbits(), requires_habitable, system.star);
    if requires_habitable {
        // Just place in the habitable
        match &mut system.orbit_slots.get_mut(habitable as usize).unwrap_or(&mut None) {
            Some(OrbitContent::Secondary) => {
                // If there happens to be a star in the habitable zone, place it in orbit there.
                // Note the orbit of the main world in terms of primary first
                // TODO: Is this correct when we have multiple stars?
                main_world.position_in_system = habitable as usize;
                // Safe to unwrap as if the orbital position is secondary but there is no secondary, thats bug.
                place_main_world(system.secondary.as_mut().unwrap(), main_world);
            }
            Some(OrbitContent::Tertiary) => {
                // If there happens to be a star in the habitable zone, place it in orbit there.
                // Note the orbit of the main world in terms of primary first
                // TODO: Is this correct when we have multiple stars?
                main_world.position_in_system = habitable as usize;
                // Safe to unwrap as if the orbital position is tertiary but there is no tertiary, thats bug.
                place_main_world(system.tertiary.as_mut().unwrap(), main_world);
            }
            Some(OrbitContent::GasGiant(gas_giant)) => {
                main_world.orbit = gas_giant.gen_satellite_orbit(main_world.size == 0);
                main_world.position_in_system = habitable as usize;
                gas_giant.satellites.push(main_world);
            }
            Some(OrbitContent::Blocked) => {
                main_world.position_in_system = habitable as usize;
                main_world.orbit = habitable as usize;
                system.set_orbit_slot(habitable as usize, OrbitContent::World(main_world));
            }
            Some(OrbitContent::World(_)) | None => {
                main_world.position_in_system = habitable as usize;
                main_world.orbit = habitable as usize;
                system.set_orbit_slot(habitable as usize, OrbitContent::World(main_world));
            }
        }
    } else {
        let empty_orbits = system.get_unused_orbits();
        if empty_orbits.len() > 0 {
            let orbit = empty_orbits[rand::thread_rng().gen_range(0..empty_orbits.len())];
            main_world.position_in_system = orbit;
            main_world.orbit = orbit;
            system.set_orbit_slot(orbit, OrbitContent::World(main_world));
        } else {
            // Just jam the world in somewhere.
            let pos = rand::thread_rng().gen_range(0..system.get_max_orbits());
            main_world.orbit = pos;
            system.set_orbit_slot(pos, OrbitContent::World(main_world));
        }
    }
}

pub fn generate_system(main_world: World) -> System {
    let star_mod = if (main_world.atmosphere >= 4 && main_world.atmosphere <= 9)
        || main_world.population >= 8
    {
        4
    } else {
        0
    };
    debug!("(generate_system) star_mod = {}. Now generate stars.", star_mod);
    let mut system = gen_stars(star_mod, true);
    debug!("(generate_system) Generated stars. Now fill system.");
    fill_system(&mut system, main_world, true);
    debug!("(generate_system) Filled system. Now compute astro data.");
    // TODO: add astro data
    //main_world.compute_astro_data(&system);
    debug!("(generate_system) All complete. Return system: {:?}", system);
    system
}

pub fn gen_world(name: String, system: &System, orbit: usize, main_world: &World) -> World {
    let mut modifier = if orbit == 0 {
        -5
    } else if orbit == 1 {
        -4
    } else if orbit == 2 {
        -2
    } else {
        0
    };

    if system.star.star_type == StarType::M {
        modifier -= 2;
    }

    let size = (roll_2d6() - 2 + modifier).min(0);

    let roll = roll_2d6();
    let signed_orbit = orbit as i32;
    let mut atmosphere = (roll_2d6() - 7
        + size
        + if signed_orbit <= get_zone(system).inner {
            -2
        } else {
            0
        }
        + if signed_orbit > get_zone(system).habitable {
            -2
        } else {
            0
        })
    .clamp(0, 10);

    // Special case for a type A atmosphere.
    if roll == 12 && signed_orbit > get_zone(system).habitable {
        atmosphere = 10;
    }

    let mut hydro = (roll_2d6() - 7
        + size
        + if signed_orbit > get_zone(system).habitable {
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
    if size <= 0 || signed_orbit <= get_zone(system).inner {
        hydro = 0;
    }

    let population = (roll_2d6() - 2
        + if signed_orbit <= get_zone(system).inner {
            -5
        } else {
            0
        }
        + if signed_orbit > get_zone(system).habitable {
            -5
        } else {
            0
        }
        + if ![5, 6, 8].contains(&atmosphere) {
            -2
        } else {
            0
        })
    .clamp(0, main_world.population - 1);

    let mut world = World::new(
        name, orbit, orbit, size, atmosphere, hydro, population, false, false,
    );
    gen_subordinate_stats(&mut world, main_world);
    gen_subordinate_facilities(&get_zone(system), &mut world, orbit, main_world);
    world
}

fn fill_system(system: &mut System, main_world: World, is_primary: bool) {
    gen_blocked_orbits(system);
    let main_world_copy = main_world.clone();
    let system_zones = get_zone(system);
    let num_gas_giants = gen_gas_giants(system);

    gen_planetoids(system, num_gas_giants, &main_world_copy);

    if is_primary {
        debug!("(fill_system) Place main world...");
        place_main_world(system, main_world);
        debug!("(fill_system) Placed main world.");
    }

    for i in 0..=get_zone(system).hot {
        system.set_orbit_slot(i as usize, OrbitContent::Blocked);
    }

    for i in (get_zone(system).hot + 1)..system.get_max_orbits() as i32 {
        debug!("(fill_system) Fill orbit {}", i);
        let i = i as usize;
        if system.is_slot_empty(i) {
            let name = gen_planet_name(system, i);
            let new_world = gen_world(name, system, i, &main_world_copy);
            //new_world.compute_astro_data(system);
            system.set_orbit_slot(i, OrbitContent::World(new_world));
        }
    }

    let zone_table = get_zone(system).clone();
    for i in 0..system.get_max_orbits() {
        match &mut system.orbit_slots[i] {
            Some(OrbitContent::World(world)) => {
                let num_satellites = world.determine_num_satellites();
                for _ in 0..num_satellites {
                    world.gen_satellite(&zone_table, &main_world_copy);
                }
                world.clean_satellites();
            }
            Some(OrbitContent::GasGiant(gas_giant)) => {
                let num_satellites = gas_giant.determine_num_satellites();
                for _ in 0..num_satellites {
                    gas_giant.gen_satellite(&system_zones, &main_world_copy);
                }
                gas_giant.clean_satellites();
            }
            _ => continue,
        }
    }

    if let Some(secondary) = &mut system.secondary {
        if secondary.orbit != StarOrbit::Primary {
            fill_system(secondary, main_world_copy.clone(), false);
        }
    }

    if let Some(tertiary) = &mut system.tertiary {
        if tertiary.orbit != StarOrbit::Primary {
            fill_system(tertiary, main_world_copy, false);
        }
    }

    debug!("(fill_system) System filled.");
}

fn gen_subordinate_facilities(
    system_zones: &ZoneTable,
    world: &mut World,
    orbit: usize,
    main_world: &World,
) {
    // Mining?
    if main_world.trade_classes.contains(&TradeClass::Industrial) && world.population >= 2 {
        world.facilities.push(Facility::Mining);
    }

    // Farming?
    if orbit as i32 == system_zones.habitable
        && orbit as i32 > system_zones.inner
        && world.atmosphere >= 4
        && world.atmosphere <= 9
        && world.hydro >= 4        && world.hydro <= 8
        && world.population >= 2
    {
        world.facilities.push(Facility::Farming);
    }

    // Colony?
    if world.government == 6 && world.get_population() >= 5 {
        world.facilities.push(Facility::Colony);
    }

    // Research Lab?
    if main_world.population > 0
        && main_world.tech_level > 8
        && roll_2d6()
            + if main_world.tech_level >= 10 { 2 } else { 0 }
            + if world.population == 0 { -2 } else { 0 }
            >= 12
    {
        world.facilities.push(Facility::Lab);
        // Fix tech level if there is a lab.  Not ideal but we need to gen most of a world/satellite
        // before facilities, but tech level is impacted by having a lab.
        if world.tech_level == main_world.tech_level - 1 {
            world.tech_level = main_world.tech_level;
        }
    }

    // Military Base?
    let modifier = if main_world.get_population() >= 8 {
        1
    } else {
        0
    } + if main_world.atmosphere == world.atmosphere {
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
        && world.get_population() > 0
        && roll_2d6() + modifier >= 12
    {
        world.facilities.push(Facility::Military);
    }
}

fn gen_subordinate_stats(world: &mut World, main_world: &World) {
    let population = world.get_population();
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
    } else if population > 0 && ![5, 6, 8].contains(&world.atmosphere) && main_world.tech_level <= 7
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
    world.set_subordinate_stats(port, government, law_level, tech_level, Vec::new());
}

// Implement other functions...
fn gen_planet_name(system: &System, orbit: usize) -> String {
    format!("{} {}", system.name, arabic_to_roman(orbit + 1))
}

fn arabic_to_roman(num: usize) -> String {
    if num > 20 {
        panic!("Input must be an integer between 0 and 20");
    }
    let roman_numerals: [(usize, &str); 21] = [
        (20, "XX"),
        (19, "XIX"),
        (18, "XVIII"),
        (17, "XVII"),
        (16, "XVI"),
        (15, "XV"),
        (14, "XIV"),
        (13, "XIII"),
        (12, "XII"),
        (11, "XI"),
        (10, "X"),
        (9, "IX"),
        (8, "VIII"),
        (7, "VII"),
        (6, "VI"),
        (5, "V"),
        (4, "IV"),
        (3, "III"),
        (2, "II"),
        (1, "I"),
        (0, "N"),
    ];
    for (value, symbol) in roman_numerals {
        if num >= value {
            return symbol.to_string();
        }
    }
    "".to_string()
}

// Functions
fn roll_2d6() -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(1..=6) + rng.gen_range(1..=6)
}

fn roll_1d6() -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(1..=6)
}

fn roll_10() -> i32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(0..=9)
}


#[cfg(test)]
mod tests {
    use super::*;
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
        let main_world = World::from_upp("Main World".to_string(), main_upp, false, true);

        let system = generate_system(main_world);
        println!("{}", system);
    }

    #[test_log::test]
    fn test_2d6_random() {
        let mut buckets = HashMap::new();
        for _ in 0..10000 {
            let roll = roll_2d6();
            *buckets.entry(roll).or_insert(0) += 1;
        }

        let mut count_vec: Vec<_> = buckets.iter().collect();
        count_vec.sort_by(|a, b| a.0.cmp(&b.0));
        for (roll, count) in count_vec {
            println!("{}: {:2.2}%", roll, *count as f32 / 100.0);
        }

    }
}
