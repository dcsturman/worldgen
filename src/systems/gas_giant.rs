//! # Gas Giant Generation Module
//!
//! This module handles the generation and management of gas giants in Traveller solar systems.
//! Gas giants are major planetary bodies that can host multiple satellites (moons) and play
//! important roles in system dynamics, trade, and exploration.
//!
//! ## Key Features
//!
//! - **Size Classification**: Small and Large gas giants with different characteristics
//! - **Satellite Generation**: Automatic generation of moons, rings, and orbital bodies
//! - **Naming System**: Intelligent naming based on population and system importance
//! - **Orbital Mechanics**: Realistic satellite orbit generation with collision avoidance
//! - **World Generation**: Full UWP generation for satellites including atmosphere, hydrographics, and population
//!
//! ## Gas Giant Types
//!
//! - **Small Gas Giants**: Fewer satellites, smaller overall system influence
//! - **Large Gas Giants**: More satellites, can support extreme orbits, greater system presence
//!
//! ## Satellite Types
//!
//! - **Ring Systems**: Size 0 satellites representing planetary rings
//! - **Rocky Moons**: Various sizes with full world characteristics
//! - **Close Orbits**: Satellites in tight orbits around the gas giant
//! - **Far Orbits**: Distant satellites with different environmental conditions
//! - **Extreme Orbits**: Very distant satellites only possible around large gas giants
//!
//! ## Usage
//!
//! ```rust
//! use worldgen::systems::gas_giant::{GasGiant, GasGiantSize};
//!
//! let mut gas_giant = GasGiant::new(GasGiantSize::Large, 5);
//! gas_giant.gen_name("Sol", 5);
//! // Generate satellites using HasSatellites trait methods
//! ```

use log::debug;
use reactive_stores::Store;

use crate::systems::has_satellites::HasSatellites;
use crate::systems::name_tables::{gen_moon_name, gen_planet_name};
use crate::systems::system::Star;
use crate::systems::system_tables::ZoneTable;
use crate::systems::world::{Satellites, World};
use crate::trade::PortCode;
use crate::util::{arabic_to_roman, roll_1d6, roll_2d6};
use std::fmt::Display;

/// Represents a gas giant in a solar system
///
/// Gas giants are major planetary bodies that can host multiple satellites and
/// play important roles in system trade and exploration. They are classified
/// by size which affects their satellite generation characteristics.
#[derive(Debug, Clone, Store)]
pub struct GasGiant {
    /// Display name of the gas giant
    pub name: String,
    /// Size classification affecting satellite generation
    pub size: GasGiantSize,
    /// Collection of satellite worlds orbiting this gas giant
    satellites: Satellites,
    /// Orbital position within the star system
    pub orbit: usize,
}

/// Size classification for gas giants
///
/// Determines the number and types of satellites that can be generated,
/// as well as the maximum orbital distances possible for satellites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasGiantSize {
    /// Smaller gas giant with fewer satellites and limited orbital ranges
    Small,
    /// Larger gas giant with more satellites and extreme orbital possibilities
    Large,
}

impl GasGiant {
    /// Creates a new gas giant with the specified size and orbital position
    ///
    /// The gas giant starts with an empty name and no satellites. Use `gen_name()`
    /// to assign an appropriate name and satellite generation methods to populate
    /// the system.
    ///
    /// # Arguments
    ///
    /// * `size` - Size classification (Small or Large)
    /// * `orbit` - Orbital position within the star system
    ///
    /// # Returns
    ///
    /// New `GasGiant` instance ready for further configuration
    pub fn new(size: GasGiantSize, orbit: usize) -> GasGiant {
        GasGiant {
            name: "".to_string(),
            size,
            satellites: Satellites { sats: Vec::new() },
            orbit,
        }
    }

    /// Generates an appropriate name for the gas giant
    ///
    /// Uses intelligent naming logic based on the population of the satellite system:
    /// - **Named**: Gas giants with significant population (≥100,000 residents) get proper names
    /// - **Designated**: Low-population systems get systematic names using Roman numerals
    ///
    /// Population threshold of 5 on the Traveller scale represents approximately 100,000 residents.
    ///
    /// # Arguments
    ///
    /// * `system_name` - Name of the parent star system
    /// * `orbit` - Orbital position for systematic naming (converted to Roman numerals)
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut gas_giant = GasGiant::new(GasGiantSize::Large, 4);
    /// gas_giant.gen_name("Sol", 4);
    /// // Result: either a proper name like "Jupiter" or "Sol V"
    /// ```
    pub fn gen_name(&mut self, system_name: &str, orbit: usize) {
        // Gas giants with more than 100,000 residents in their system get a name.
        // i.e. if you're not just a remote outpost with mining, etc, you get a name.
        let pop = self.satellites.sats.iter().any(|x| x.get_population() >= 5);

        if pop {
            self.name = gen_planet_name()
        } else {
            self.name = format!("{} {}", system_name, arabic_to_roman(orbit + 1))
        }
    }
}

impl HasSatellites for GasGiant {
    /// Returns the current number of satellites orbiting this gas giant
    fn get_num_satellites(&self) -> usize {
        self.satellites.sats.len()
    }

    /// Retrieves a satellite at the specified orbital position
    ///
    /// # Arguments
    ///
    /// * `orbit` - Orbital position to search for
    ///
    /// # Returns
    ///
    /// Reference to the satellite world if found, None otherwise
    fn get_satellite(&self, orbit: usize) -> Option<&World> {
        self.satellites.sats.iter().find(|&x| x.orbit == orbit)
    }

    /// Returns a mutable reference to the satellite collection
    fn get_satellites_mut(&mut self) -> &mut Satellites {
        &mut self.satellites
    }

    /// Adds a new satellite to this gas giant's system
    ///
    /// # Arguments
    ///
    /// * `satellite` - World to add as a satellite
    fn push_satellite(&mut self, satellite: World) {
        self.satellites.sats.push(satellite);
    }

    /// Generates an orbital position for a new satellite
    ///
    /// Uses different algorithms based on whether the satellite is a ring system
    /// or a regular moon. Automatically avoids orbital collisions by incrementing
    /// the orbit until a free position is found.
    ///
    /// # Orbital Types
    ///
    /// ## Ring Systems (is_ring = true)
    /// - **Orbit 1**: 50% chance (rolls 1-3 on 1d6)
    /// - **Orbit 2**: 33% chance (rolls 4-5 on 1d6)  
    /// - **Orbit 3**: 17% chance (rolls 6 on 1d6)
    ///
    /// ## Regular Satellites (is_ring = false)
    /// - **Close Orbit**: 2d6+3 (rolls ≤7 after satellite count modifier)
    /// - **Far Orbit**: (2d6+3)×5 (rolls 8-11 after modifier)
    /// - **Extreme Orbit**: (2d6+3)×25 (roll 12 after modifier, Large gas giants only)
    ///
    /// # Arguments
    ///
    /// * `is_ring` - Whether this satellite is a ring system
    ///
    /// # Returns
    ///
    /// Available orbital position for the new satellite
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
        debug!("(GasGiant.gen_satellite_orbit) Orbit is {orbit}");
        orbit
    }

    /// Determines the number of satellites this gas giant should have
    ///
    /// Uses different probability distributions based on gas giant size:
    /// - **Small Gas Giants**: (2d6-4), minimum 0 satellites
    /// - **Large Gas Giants**: 2d6 satellites (2-12 range)
    ///
    /// # Returns
    ///
    /// Number of satellites to generate for this gas giant
    fn determine_num_satellites(&self) -> i32 {
        match self.size {
            GasGiantSize::Small => (roll_2d6() - 4).max(0),
            GasGiantSize::Large => roll_2d6(),
        }
    }

    /// Generates a complete satellite world for this gas giant
    ///
    /// Creates a fully detailed satellite with all UWP characteristics including
    /// size, atmosphere, hydrographics, population, and supporting infrastructure.
    /// Special handling for ring systems (size 0) which get minimal characteristics.
    ///
    /// # Generation Process
    ///
    /// 1. **Size Determination**: Based on gas giant size with random variation
    /// 2. **Orbit Assignment**: Uses `gen_satellite_orbit()` for positioning
    /// 3. **Ring Check**: Size 0 creates ring systems with Y-class starports
    /// 4. **Atmosphere**: Modified by size, orbit, and system zones
    /// 5. **Hydrographics**: Affected by orbit, atmosphere, and temperature
    /// 6. **Population**: Influenced by orbit position and atmospheric conditions
    /// 7. **Infrastructure**: Generates subordinate stats, trade classes, and facilities
    /// 8. **Astronomy**: Computes orbital mechanics and environmental data
    ///
    /// # Size Generation
    /// - **Small Gas Giants**: 2d6-6 (minimum -1, representing size S)
    /// - **Large Gas Giants**: 2d6-4 (minimum -1, representing size S)
    ///
    /// # Environmental Modifiers
    /// - **Inner Zone**: -4 to atmosphere, hydro = 0
    /// - **Outer Zone**: -4 to atmosphere and hydrographics
    /// - **Small Worlds**: Size ≤1 forces atmosphere = 0
    /// - **Extreme Atmospheres**: Size ≤0 or inner zone forces hydro = 0
    ///
    /// # Arguments
    ///
    /// * `system_zones` - Zone boundaries for environmental calculations
    /// * `main_world` - Primary world for trade and facility generation
    /// * `star` - Primary star for astronomical calculations
    fn gen_satellite(&mut self, system_zones: &ZoneTable, main_world: &World, star: &Star) {
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
            ring.set_port(PortCode::Y);
            self.push_satellite(ring);
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
            self.orbit,
            size,
            atmosphere,
            hydro,
            population,
            true,
            false,
        );

        satellite.gen_subordinate_stats(main_world);
        satellite.gen_trade_classes();
        satellite.gen_subordinate_facilities(system_zones, self.orbit, main_world);
        satellite.compute_astro_data(star);
        self.satellites.sats.push(satellite);
    }
}

impl Display for GasGiantSize {
    /// Formats gas giant size for display
    ///
    /// # Returns
    ///
    /// - "Small GG" for Small gas giants
    /// - "Large GG" for Large gas giants
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GasGiantSize::Small => write!(f, "Small GG"),
            GasGiantSize::Large => write!(f, "Large GG"),
        }
    }
}

impl Display for GasGiant {
    /// Formats the gas giant and all its satellites for display
    ///
    /// Outputs the gas giant's orbital position, name, and size classification,
    /// followed by an indented list of all satellite worlds with their details.
    ///
    /// # Format
    ///
    /// ```text
    /// 5      Jupiter              Large GG
    ///     1   Io               Ring System    Y
    ///     15  Europa           A867A98-C      Hi In
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:<7}{:<24}{:<12}", self.orbit, self.name, self.size)?;
        for satellite in self.satellites.sats.iter() {
            writeln!(f, "\t{satellite}")?;
        }
        Ok(())
    }
}
