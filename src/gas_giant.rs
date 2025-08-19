use log::debug;
use reactive_stores::Store;

use crate::has_satellites::HasSatellites;
use crate::name_tables::{gen_moon_name, gen_planet_name};
use crate::system::Star;
use crate::system_tables::ZoneTable;
use crate::util::{arabic_to_roman, roll_1d6, roll_2d6};
use crate::world::{Satellites, World};
use trade::PortCode;
use std::fmt::Display;

#[derive(Debug, Clone, Store)]
pub struct GasGiant {
    pub name: String,
    pub size: GasGiantSize,
    satellites: Satellites,
    pub orbit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GasGiantSize {
    Small,
    Large,
}

impl GasGiant {
    pub fn new(size: GasGiantSize, orbit: usize) -> GasGiant {
        GasGiant {
            name: "".to_string(),
            size,
            satellites: Satellites { sats: Vec::new() },
            orbit,
        }
    }

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
    fn get_num_satellites(&self) -> usize {
        self.satellites.sats.len()
    }

    fn get_satellite(&self, orbit: usize) -> Option<&World> {
        self.satellites.sats.iter().find(|&x| x.orbit == orbit)
    }

    fn get_satellites_mut(&mut self) -> &mut Satellites {
        &mut self.satellites
    }

    fn push_satellite(&mut self, satellite: World) {
        self.satellites.sats.push(satellite);
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

    fn determine_num_satellites(&self) -> i32 {
        match self.size {
            GasGiantSize::Small => (roll_2d6() - 4).max(0),
            GasGiantSize::Large => roll_2d6(),
        }
    }

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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GasGiantSize::Small => write!(f, "Small GG"),
            GasGiantSize::Large => write!(f, "Large GG"),
        }
    }
}

impl Display for GasGiant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:<7}{:<24}{:<12}", self.orbit, self.name, self.size)?;
        for satellite in self.satellites.sats.iter() {
            writeln!(f, "\t{satellite}")?;
        }
        Ok(())
    }
}
