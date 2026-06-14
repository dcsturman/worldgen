//! # World System Module
//!
//! This module defines the core World structure and related types for representing
//! planets and worlds in the Traveller universe, including their physical characteristics,
//! facilities, and trade classifications.
//! use log::debug;
#[cfg(feature = "frontend")]
use reactive_stores::Store;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[allow(unused_imports)]
use log::debug;

use crate::systems::astro::AstroData;
use crate::systems::constraint::PartialUwp;
use crate::systems::has_satellites::HasSatellites;
use crate::systems::name_tables::{gen_moon_name, gen_planet_name};
use crate::systems::system::{Star, StarType};
use crate::systems::system_tables::{ZoneTable, get_zone};
use crate::util::{arabic_to_roman, roll_1d6, roll_2d6};

use crate::trade::PortCode;
use crate::trade::TradeClass;
use crate::trade::ZoneClassification;

/// Container for world satellites
///
/// Stores a vector of satellite worlds with a key based on the parent world's name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "frontend", derive(Store))]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "frontend", derive(Store))]
pub struct Satellites {
    #[cfg_attr(feature = "frontend", store(key: String = |world| world.name.clone()))]
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

    /// Whether this world was flagged as the main world for the system.
    pub fn is_mainworld(&self) -> bool {
        self.is_mainworld
    }

    /// Returns the law level of the world
    pub fn get_law_level(&self) -> i32 {
        self.law_level
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
    /// * `uwp` - The Traveller UWP string
    /// * `is_satellite` - Whether this world is a satellite of another body used to just record in the returned world. It does not
    ///   impact parsing of the UWP.
    /// * `is_mainworld` - Whether this is the main world of the system used to just record in the returned world. It does not
    ///   impact parsing of the UWP.
    pub fn from_uwp(
        name: &str,
        uwp: &str,
        is_satellite: bool,
        is_mainworld: bool,
    ) -> Result<World, Box<dyn std::error::Error>> {
        let port = PortCode::from_uwp(uwp);
        let size = i32::from_str_radix(&uwp[1..2], 16)?;
        let atmosphere = i32::from_str_radix(&uwp[2..3], 16)?;
        let hydro = i32::from_str_radix(&uwp[3..4], 16)?;
        let population = i32::from_str_radix(&uwp[4..5], 16)?;
        let government = i32::from_str_radix(&uwp[5..6], 16)?;
        let law_level = i32::from_str_radix(&uwp[6..7], 16)?;
        let tech_level = i32::from_str_radix(&uwp[8..9], 16)?;
        let mut world = World::new(
            name.to_string(),
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

        let tech_level = subordinate_tech_level(population, self.atmosphere, main_world);

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
        self.trade_classes
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(", ")
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

    /// Constraint-aware version of [`World::generate`]: every UWP
    /// column is either taken from `partial` (if specified) or rolled
    /// using the same per-orbit / per-zone modifier table the legacy
    /// `generate` path uses. Already-resolved digits flow into the
    /// rolls for later digits, so e.g. user-specified size feeds the
    /// hydrographics roll exactly the way fully-rolled size would.
    ///
    /// Pass `partial = None` and `name = None` to recreate the
    /// classic "everything rolled" behavior — `generate` itself is
    /// just a thin wrapper.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_with_partial(
        star: &Star,
        orbit: usize,
        main_world: &World,
        partial: Option<&PartialUwp>,
        name: Option<&str>,
        is_satellite: bool,
        is_mainworld: bool,
    ) -> World {
        let signed_orbit = orbit as i32;

        // Size
        let size = match partial.and_then(|p| p.size) {
            Some(s) => s as i32,
            None => {
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
                (roll_2d6() - 2 + modifier).min(0)
            }
        };

        // Atmosphere
        let atmosphere = match partial.and_then(|p| p.atmosphere) {
            Some(a) => a as i32,
            None => {
                let roll = roll_2d6();
                let mut atm = (roll_2d6() - 7
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
                if roll == 12 && signed_orbit > get_zone(star).habitable + 1 {
                    atm = 10;
                }
                atm
            }
        };

        // Hydro
        let hydro = match partial.and_then(|p| p.hydro) {
            Some(h) => h as i32,
            None => {
                let mut h = (roll_2d6() - 7
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
                    h = 0;
                }
                h
            }
        };

        // Population
        let population = match partial.and_then(|p| p.population) {
            Some(p) => p as i32,
            None if force_lifeless(main_world, atmosphere) => 0,
            None => (roll_2d6() - 2
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
            .clamp(0, main_world.get_population() - 1),
        };

        let world_name = name
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let mut world = World::new(
            world_name,
            orbit,
            orbit,
            size,
            atmosphere,
            hydro,
            population,
            is_satellite,
            is_mainworld,
        );
        if name.is_none() {
            world.gen_name(&main_world.name, orbit);
        }
        if let Some(p) = partial {
            world.gen_subordinate_stats_with_partial(main_world, p);
        } else {
            world.gen_subordinate_stats(main_world);
        }
        world.gen_trade_classes();
        world.gen_subordinate_facilities(&get_zone(star), orbit, main_world);
        world
    }

    /// Constraint-aware version of [`World::gen_subordinate_stats`]:
    /// each of port / government / law / tech is either taken from
    /// `partial` (if specified) or rolled using the same logic as the
    /// classic call.
    pub fn gen_subordinate_stats_with_partial(&mut self, main_world: &World, partial: &PartialUwp) {
        let population = self.get_population();

        let government = match partial.government {
            Some(g) => g as i32,
            None => {
                let modifier = if main_world.government == 6 {
                    population
                } else if main_world.government >= 7 {
                    1
                } else {
                    0
                };
                if population <= 0 {
                    0
                } else {
                    match roll_1d6() + modifier {
                        1 => 0,
                        2 => 1,
                        3 => 2,
                        4 => 3,
                        _ => 6,
                    }
                }
            }
        };

        let law_level = match partial.law {
            Some(l) => l as i32,
            None => {
                if population <= 0 {
                    0
                } else {
                    (roll_1d6() - 3 + main_world.law_level).max(0)
                }
            }
        };

        let tech_level = match partial.tech {
            Some(t) => t as i32,
            None => subordinate_tech_level(population, self.atmosphere, main_world),
        };

        let port = match partial.port {
            Some(p) => p,
            None => {
                let roll = roll_1d6()
                    + match population {
                        0 => -3,
                        1 => -2,
                        2..=5 => 0,
                        _ => 2,
                    };
                match roll {
                    -2..=2 => PortCode::Y,
                    3 => PortCode::H,
                    4..=5 => PortCode::G,
                    _ => PortCode::F,
                }
            }
        };

        self.set_subordinate_stats(port, government, law_level, tech_level, Vec::new());
    }

    pub fn generate(star: &Star, orbit: usize, main_world: &World) -> World {
        Self::generate_with_partial(star, orbit, main_world, None, None, false, false)
    }
}

/// A "real atmosphere" — Traveller atmosphere code 2..=8 — is one that
/// can sustain a population without continuous life-support tech. 0–1
/// require vacc suits, A+ require sealed habitats / corrosion handling,
/// and 9 (dense, tainted) is technically breathable with a filter but
/// the user's threshold for the population rule is "> 1 and < 9".
pub(crate) fn has_real_atmosphere(atmosphere: i32) -> bool {
    (2..=8).contains(&atmosphere)
}

/// House rule: a TL < 7 main world lacks the life-support tech to
/// sustain populations on bodies without a real atmosphere, so any
/// unconstrained such body is forced empty. The override is the partial
/// UWP — if the user pinned a population (constraint `Planet`/`Belt`/
/// `Moon` with an explicit pop digit), they've stated a reason and we
/// keep it. Used by both planet and satellite generators.
pub(crate) fn force_lifeless(main_world: &World, atmosphere: i32) -> bool {
    !has_real_atmosphere(atmosphere) && main_world.tech_level < 7
}

/// Subordinate-world tech level rule. Population 0 always yields TL 0.
/// Otherwise the cap is main-world TL minus one (Book 6, p.24), with one
/// house exception: a populated body without a real atmosphere in a
/// TL-7 system stays at TL 7, because TL 7 is the floor for the life
/// support / vacc-suits / fusion plants such a body relies on — bumping
/// it to TL 6 would imply it can't function. Higher-TL systems don't
/// need the exception (TL ≥ 7 is already met after subtracting one),
/// and lower-TL systems should already have had the population zeroed
/// out by the caller via [`force_lifeless`].
pub(crate) fn subordinate_tech_level(population: i32, atmosphere: i32, main_world: &World) -> i32 {
    if population <= 0 {
        0
    } else if !has_real_atmosphere(atmosphere) && main_world.tech_level == 7 {
        7
    } else {
        (main_world.tech_level - 1).max(0)
    }
}

/// Construct a satellite (`is_satellite = true`) at a chosen orbit
/// around a parent body, honoring a partial UWP. The caller decides
/// the satellite's size — World parents use `parent.size - 1d6`,
/// GasGiant parents use their own `2d6-6`/`2d6-4` rolls — and just
/// passes the resolved size in. Wild columns roll using the same
/// per-zone modifiers the random satellite generator uses; specified
/// columns flow into rolls for later columns just as in
/// [`World::generate_with_partial`].
///
/// Size 0 produces a Ring System with the standard `Y` starport and
/// zero atmo/hydro/pop unless those columns are explicitly pinned.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_partial_satellite(
    star: &Star,
    parent_position_in_system: usize,
    sat_orbit: usize,
    size: i32,
    system_zones: &ZoneTable,
    main_world: &World,
    partial: Option<&PartialUwp>,
    name: Option<&str>,
) -> World {
    if size == 0 {
        let atmosphere = partial
            .and_then(|p| p.atmosphere)
            .map(|a| a as i32)
            .unwrap_or(0);
        let hydro = partial.and_then(|p| p.hydro).map(|h| h as i32).unwrap_or(0);
        let population = partial
            .and_then(|p| p.population)
            .map(|p| p as i32)
            .unwrap_or(0);
        let sat_name = name
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Ring System".to_string());
        let mut ring = World::new(
            sat_name,
            sat_orbit,
            parent_position_in_system,
            0,
            atmosphere,
            hydro,
            population,
            true,
            false,
        );
        let port = partial.and_then(|p| p.port).unwrap_or(PortCode::Y);
        ring.set_port(port);
        ring.compute_astro_data(star);
        return ring;
    }

    let atmosphere = match partial.and_then(|p| p.atmosphere) {
        Some(a) => a as i32,
        None => {
            let roll = roll_2d6();
            let mut atm = (roll - 7
                + size
                + if sat_orbit as i32 <= system_zones.inner
                    || sat_orbit as i32 > system_zones.habitable
                {
                    -4
                } else {
                    0
                })
            .clamp(0, 10);
            if size <= 1 {
                atm = 0;
            }
            if roll == 12 && sat_orbit as i32 > system_zones.habitable {
                atm = 10;
            }
            atm
        }
    };

    let hydro = match partial.and_then(|p| p.hydro) {
        Some(h) => h as i32,
        None => {
            let mut h = (roll_2d6() - 7
                + size
                + if sat_orbit as i32 > system_zones.habitable {
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
            if size <= 0 || sat_orbit as i32 <= system_zones.inner {
                h = 0;
            }
            h
        }
    };

    let population = match partial.and_then(|p| p.population) {
        Some(p) => p as i32,
        None if force_lifeless(main_world, atmosphere) => 0,
        None => (roll_2d6() - 2
            + if sat_orbit as i32 <= system_zones.inner {
                -5
            } else if sat_orbit as i32 > system_zones.habitable {
                -4
            } else {
                0
            }
            + if ![5, 6, 8].contains(&atmosphere) {
                -2
            } else {
                0
            })
        .clamp(0, 10),
    };

    let sat_name = name.map(|n| n.to_string()).unwrap_or_else(gen_moon_name);
    let mut satellite = World::new(
        sat_name,
        sat_orbit,
        parent_position_in_system,
        size,
        atmosphere,
        hydro,
        population,
        true,
        false,
    );
    if let Some(p) = partial {
        satellite.gen_subordinate_stats_with_partial(main_world, p);
    } else {
        satellite.gen_subordinate_stats(main_world);
    }
    satellite.gen_trade_classes();
    satellite.gen_subordinate_facilities(system_zones, sat_orbit, main_world);
    satellite.compute_astro_data(star);
    satellite
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

        let population = if force_lifeless(main_world, atmosphere) {
            0
        } else {
            (roll_2d6() - 2
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
            .clamp(0, 10)
        };

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

#[cfg(test)]
mod tests {
    use super::*;

    fn main_with_tl(tl: i32) -> World {
        let mut w = World::new("Main".to_string(), 0, 0, 8, 7, 8, 8, false, true);
        w.set_subordinate_stats(PortCode::A, 9, 9, tl, Vec::new());
        w
    }

    #[test]
    fn real_atmosphere_range() {
        for atm in 2..=8 {
            assert!(has_real_atmosphere(atm), "atmosphere {atm} should be real");
        }
        for atm in [0, 1, 9, 10, 12, 15] {
            assert!(
                !has_real_atmosphere(atm),
                "atmosphere {atm} should not be real"
            );
        }
    }

    #[test]
    fn lifeless_low_tech_system_forces_empty() {
        // Main TL < 7 + lifeless atmosphere = no population.
        let main = main_with_tl(4);
        assert!(force_lifeless(&main, 0)); // vacuum
        assert!(force_lifeless(&main, 10)); // exotic
        assert!(force_lifeless(&main, 9)); // dense tainted (just outside threshold)
        // Real atmosphere keeps its population even in low-TL system.
        assert!(!force_lifeless(&main, 5));
        assert!(!force_lifeless(&main, 8));
    }

    #[test]
    fn tl7_system_does_not_force_empty() {
        let main = main_with_tl(7);
        assert!(!force_lifeless(&main, 0));
        assert!(!force_lifeless(&main, 10));
    }

    #[test]
    fn tl7_lifeless_subordinate_keeps_tl7() {
        // Main TL == 7 and atmosphere is non-real: subordinate stays
        // at 7 instead of falling to TL 6.
        let main = main_with_tl(7);
        assert_eq!(subordinate_tech_level(5, 0, &main), 7);
        assert_eq!(subordinate_tech_level(5, 10, &main), 7);
        // Real atmosphere falls to main - 1.
        assert_eq!(subordinate_tech_level(5, 6, &main), 6);
    }

    #[test]
    fn higher_tl_subordinate_uses_main_minus_one_everywhere() {
        let main = main_with_tl(12);
        assert_eq!(subordinate_tech_level(5, 0, &main), 11);
        assert_eq!(subordinate_tech_level(5, 6, &main), 11);
        assert_eq!(subordinate_tech_level(5, 10, &main), 11);
    }

    #[test]
    fn empty_population_collapses_tl_to_zero() {
        let main = main_with_tl(7);
        assert_eq!(subordinate_tech_level(0, 0, &main), 0);
        let main = main_with_tl(12);
        assert_eq!(subordinate_tech_level(0, 6, &main), 0);
    }
}
