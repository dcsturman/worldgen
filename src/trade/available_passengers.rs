//! # Available Passengers Module
//!
//! This module handles the generation of available passengers and freight
//! for interstellar travel between worlds in the Traveller universe.
use crate::trade::{PortCode, ZoneClassification};
use crate::util::{roll_1d6, roll_2d6};
use serde::{Deserialize, Serialize};

#[allow(unused_imports)]
use log::debug;

/// Represents a lot of freight available for shipping at a specific world
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FreightLot {
    /// Size in tons (1-60)
    pub size: i32,
    /// Raw 1d6 roll used to generate size (saved for recalculation)
    pub size_roll: i32,
}

/// Represents available passengers (by class of passage) and freight for a route between two worlds
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AvailablePassengers {
    /// Number of high passage passengers
    pub high: i32,
    /// Number of medium passage passengers
    pub medium: i32,
    /// Number of basic passage passengers
    pub basic: i32,
    /// Number of low passage passengers
    pub low: i32,
    /// List of available freight lots
    pub freight_lots: Vec<FreightLot>,

    // Saved dice rolls for recalculation
    /// Raw 2d6 roll for high passage (saved for recalculation)
    pub high_roll: Option<i32>,
    /// Raw 2d6 roll for medium passage (saved for recalculation)
    pub medium_roll: Option<i32>,
    /// Raw 2d6 roll for basic passage (saved for recalculation)
    pub basic_roll: Option<i32>,
    /// Raw 2d6 roll for low passage (saved for recalculation)
    pub low_roll: Option<i32>,
    /// Individual 1d6 rolls for high passage count (saved for recalculation)
    pub high_dice_rolls: Vec<i32>,
    /// Individual 1d6 rolls for medium passage count (saved for recalculation)
    pub medium_dice_rolls: Vec<i32>,
    /// Individual 1d6 rolls for basic passage count (saved for recalculation)
    pub basic_dice_rolls: Vec<i32>,
    /// Individual 1d6 rolls for low passage count (saved for recalculation)
    pub low_dice_rolls: Vec<i32>,

    /// Raw 2d6 roll for major cargo (saved for recalculation)
    pub major_cargo_roll: Option<i32>,
    /// Broker skill check roll for major cargo (saved for recalculation)
    pub major_cargo_check_roll: Option<i32>,
    /// Rolls for size of individual major cargo lots
    pub major_cargo_size_rolls: Vec<i32>,

    /// Rolls for size of individual minor cargo lots///
    pub minor_cargo_roll: Option<i32>,
    /// Broker skill check roll for minor cargo (saved for recalculation)
    pub minor_cargo_check_roll: Option<i32>,
    /// Raw 2d6 roll for incidental cargo (saved for recalculation)
    pub minor_cargo_size_rolls: Vec<i32>,

    /// Raw 2d6 roll for minor cargo (saved for recalculation)
    pub incidental_cargo_roll: Option<i32>,
    /// Broker skill check roll for incidental cargo (saved for recalculation)
    pub incidental_cargo_check_roll: Option<i32>,
    /// Rolls for size of individual incidental cargo lots
    pub incidental_cargo_size_rolls: Vec<i32>,
}

impl AvailablePassengers {
    /// Generates available passengers and freight for a route between two worlds
    ///
    /// # Arguments
    ///
    /// * `origin_population` - Population level of the origin world
    /// * `origin_port` - Starport quality of the origin world
    /// * `origin_zone` - Travel zone classification of the origin world
    /// * `origin_tech_level` - Technology level of the origin world
    /// * `destination_population` - Population level of the destination world
    /// * `destination_port` - Starport quality of the destination world
    /// * `destination_zone` - Travel zone classification of the destination world
    /// * `destination_tech_level` - Technology level of the destination world
    /// * `distance_parsecs` - Distance between worlds in parsecs
    /// * `steward_skill` - Steward skill level of the ship's crew
    ///
    /// # Returns
    ///
    /// A new `AvailablePassengers` instance with generated passengers and freight
    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        &mut self,
        origin_population: i32,
        origin_port: PortCode,
        origin_zone: ZoneClassification,
        origin_tech_level: i32,
        destination_population: i32,
        destination_port: PortCode,
        destination_zone: ZoneClassification,
        destination_tech_level: i32,
        distance_parsecs: i32,
        steward_skill: i32,
        player_broker_skill: i32,
    ) {
        // Generate passengers
        self.generate_passengers(
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
        );

        // Generate freight lots
        self.generate_freight(
            origin_population,
            origin_port,
            origin_zone,
            origin_tech_level,
            destination_population,
            destination_port,
            destination_zone,
            destination_tech_level,
            distance_parsecs,
            player_broker_skill,
        );
    }

    pub fn reset_die_rolls(&mut self) {
        self.high_roll = None;
        self.medium_roll = None;
        self.basic_roll = None;
        self.low_roll = None;
        self.high_dice_rolls.clear();
        self.medium_dice_rolls.clear();
        self.basic_dice_rolls.clear();
        self.low_dice_rolls.clear();
        self.major_cargo_roll = None;
        self.major_cargo_check_roll = None;
        self.minor_cargo_roll = None;
        self.minor_cargo_check_roll = None;
        self.incidental_cargo_roll = None;
        self.incidental_cargo_check_roll = None;
    }

    /// Generates passengers for all passenger classes
    ///
    /// # Arguments
    ///
    /// * `passengers` - Mutable reference to the passengers structure to populate
    /// * `origin_population` - Population level of the origin world
    /// * `origin_port` - Starport quality of the origin world
    /// * `origin_zone` - Travel zone classification of the origin world
    /// * `destination_population` - Population level of the destination world
    /// * `destination_port` - Starport quality of the destination world
    /// * `destination_zone` - Travel zone classification of the destination world
    /// * `distance_parsecs` - Distance between worlds in parsecs
    /// * `steward_skill` - Steward skill level of the ship's crew
    #[allow(clippy::too_many_arguments)]
    fn generate_passengers(
        &mut self,
        origin_population: i32,
        origin_port: PortCode,
        origin_zone: ZoneClassification,
        destination_population: i32,
        destination_port: PortCode,
        destination_zone: ZoneClassification,
        distance_parsecs: i32,
        steward_skill: i32,
    ) {
        self.high = Self::generate_passenger_class(
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            &mut self.high_roll,
            &mut self.high_dice_rolls,
            PassengerClass::High,
        );

        self.medium = Self::generate_passenger_class(
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            &mut self.medium_roll,
            &mut self.medium_dice_rolls,
            PassengerClass::Medium,
        );

        self.basic = Self::generate_passenger_class(
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            &mut self.basic_roll,
            &mut self.basic_dice_rolls,
            PassengerClass::Basic,
        );

        self.low = Self::generate_passenger_class(
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            &mut self.low_roll,
            &mut self.low_dice_rolls,
            PassengerClass::Low,
        );
    }

    /// Generates freight lots for all cargo classes
    ///
    /// # Arguments
    ///
    /// * `passengers` - Mutable reference to the passengers structure to populate
    /// * `origin_population` - Population level of the origin world
    /// * `origin_port` - Starport quality of the origin world
    /// * `origin_zone` - Travel zone classification of the origin world
    /// * `origin_tech_level` - Technology level of the origin world
    /// * `destination_population` - Population level of the destination world
    /// * `destination_port` - Starport quality of the destination world
    /// * `destination_zone` - Travel zone classification of the destination world
    /// * `destination_tech_level` - Technology level of the destination world
    /// * `distance_parsecs` - Distance between worlds in parsecs
    #[allow(clippy::too_many_arguments)]
    fn generate_freight(
        &mut self,
        origin_population: i32,
        origin_port: PortCode,
        origin_zone: ZoneClassification,
        origin_tech_level: i32,
        destination_population: i32,
        destination_port: PortCode,
        destination_zone: ZoneClassification,
        destination_tech_level: i32,
        distance_parsecs: i32,
        player_broker_skill: i32,
    ) {
        // Clear out all existing freight lots
        self.freight_lots.clear();

        // Generate major cargo
        let num_major_lots = Self::generate_cargo_class(
            origin_population,
            origin_port,
            origin_zone,
            origin_tech_level,
            destination_population,
            destination_port,
            destination_zone,
            destination_tech_level,
            distance_parsecs,
            player_broker_skill,
            &mut self.major_cargo_roll,
            &mut self.major_cargo_check_roll,
            &mut self.major_cargo_size_rolls,
            CargoClass::Major,
        );

        // Generate minor cargo
        let num_minor_lots = Self::generate_cargo_class(
            origin_population,
            origin_port,
            origin_zone,
            origin_tech_level,
            destination_population,
            destination_port,
            destination_zone,
            destination_tech_level,
            distance_parsecs,
            player_broker_skill,
            &mut self.minor_cargo_roll,
            &mut self.minor_cargo_check_roll,
            &mut self.minor_cargo_size_rolls,
            CargoClass::Minor,
        );

        // Generate incidental cargo
        let num_incidental_lots = Self::generate_cargo_class(
            origin_population,
            origin_port,
            origin_zone,
            origin_tech_level,
            destination_population,
            destination_port,
            destination_zone,
            destination_tech_level,
            distance_parsecs,
            player_broker_skill,
            &mut self.incidental_cargo_roll,
            &mut self.incidental_cargo_check_roll,
            &mut self.incidental_cargo_size_rolls,
            CargoClass::Incidental,
        );

        // Collect all freight lots using iterator chains
        self.freight_lots.extend(
            [
                (&self.major_cargo_size_rolls, num_major_lots),
                (&self.minor_cargo_size_rolls, num_minor_lots),
                (&self.incidental_cargo_size_rolls, num_incidental_lots),
            ]
            .iter()
            .flat_map(|(rolls, count)| {
                rolls
                    .iter()
                    .take(*count as usize)
                    .map(|&size_roll| FreightLot {
                        size: size_roll * 10,
                        size_roll,
                    })
            }),
        );

        // Sort freight lots by size (largest first)
        self.freight_lots.sort_by(|a, b| b.size.cmp(&a.size));
    }

    /// Generates the number of cargo lots for a specific cargo class
    ///
    /// # Arguments
    ///
    /// * `origin_population` - Population level of the origin world
    /// * `origin_port` - Starport quality of the origin world
    /// * `origin_zone` - Travel zone classification of the origin world
    /// * `origin_tech_level` - Technology level of the origin world
    /// * `destination_population` - Population level of the destination world
    /// * `destination_port` - Starport quality of the destination world
    /// * `destination_zone` - Travel zone classification of the destination world
    /// * `destination_tech_level` - Technology level of the destination world
    /// * `distance_parsecs` - Distance between worlds in parsecs
    /// * `cargo_class` - The class of cargo to generate
    ///
    /// # Returns
    ///
    /// Tuple of (number of lots, base 2d6 roll, broker check roll)
    #[allow(clippy::too_many_arguments)]
    fn generate_cargo_class(
        origin_population: i32,
        origin_port: PortCode,
        origin_zone: ZoneClassification,
        origin_tech_level: i32,
        destination_population: i32,
        destination_port: PortCode,
        destination_zone: ZoneClassification,
        destination_tech_level: i32,
        distance_parsecs: i32,
        player_broker_skill: i32,
        roll: &mut Option<i32>,
        check_roll: &mut Option<i32>,
        size_rolls: &mut Vec<i32>,
        cargo_class: CargoClass,
    ) -> i32 {
        let base_roll = roll.get_or_insert_with(roll_2d6);

        // Modify by effect of player broker skill check
        let check_roll_value = *check_roll.get_or_insert_with(roll_2d6);
        let check = player_broker_skill + check_roll_value - 8;
        let mut num_lots = *base_roll + check;

        // Cargo class modifiers
        match cargo_class {
            CargoClass::Major => num_lots -= 4,
            CargoClass::Incidental => num_lots += 2,
            _ => {}
        }

        // Population modifiers
        for pop in [origin_population, destination_population] {
            if pop <= 1 {
                num_lots -= 4;
            } else if pop >= 8 {
                num_lots += 4;
            } else if pop >= 6 {
                num_lots += 2;
            }
        }

        // Starport modifiers
        for port in [origin_port, destination_port] {
            match port {
                PortCode::A => num_lots += 2,
                PortCode::B => num_lots += 1,
                PortCode::E => num_lots -= 1,
                PortCode::X => num_lots -= 3,
                _ => {}
            }
        }

        // Tech level modifiers
        for tech_level in [origin_tech_level, destination_tech_level] {
            if tech_level <= 6 {
                num_lots -= 1;
            } else if tech_level >= 9 {
                num_lots += 2;
            }
        }

        // Zone modifiers
        for zone in [origin_zone, destination_zone] {
            match zone {
                ZoneClassification::Green => continue,
                ZoneClassification::Amber => num_lots -= 2,
                ZoneClassification::Red => num_lots -= 6,
            }
        }

        // Distance modifier
        if distance_parsecs > 1 {
            num_lots -= distance_parsecs - 1;
        }

        // Generate additional size rolls if needed
        let num_new = num_lots - size_rolls.len() as i32;
        for _ in 0..num_new {
            size_rolls.push(roll_1d6());
        }

        num_lots
    }

    /// Generates the number of passengers for a specific passenger class
    ///
    /// # Arguments
    ///
    /// * `origin_population` - Population level of the origin world
    /// * `origin_port` - Starport quality of the origin world
    /// * `origin_zone` - Travel zone classification of the origin world
    /// * `destination_population` - Population level of the destination world
    /// * `destination_port` - Starport quality of the destination world
    /// * `destination_zone` - Travel zone classification of the destination world
    /// * `distance_parsecs` - Distance between worlds in parsecs
    /// * `steward_skill` - Steward skill level of the ship's crew
    /// * `passenger_class` - The class of passenger to generate
    ///
    /// # Returns
    ///
    /// Tuple of (passenger count, initial 2d6 roll, individual dice rolls)
    #[allow(clippy::too_many_arguments)]
    fn generate_passenger_class(
        origin_population: i32,
        origin_port: PortCode,
        origin_zone: ZoneClassification,
        destination_population: i32,
        destination_port: PortCode,
        destination_zone: ZoneClassification,
        distance_parsecs: i32,
        steward_skill: i32,
        roll: &mut Option<i32>,
        dice_rolls: &mut Vec<i32>,
        passenger_class: PassengerClass,
    ) -> i32 {
        // Get prior roll or roll now if we didn't have one
        // Note, we explicitly copy as we don't want modifications to
        // roll to lose the base roll.
        let mut roll = *roll.get_or_insert_with(roll_2d6);

        // Apply modifiers
        roll += steward_skill;
        // Class-specific modifiers
        match passenger_class {
            PassengerClass::High => roll -= 4,
            PassengerClass::Low => roll += 1,
            _ => {}
        }

        // Population modifiers
        if origin_population <= 1 {
            roll -= 4;
        }
        if destination_population <= 1 {
            roll -= 4;
        }
        // +1 for population 6-7, +3 for population 8+
        for pop in [origin_population, destination_population] {
            if pop >= 8 {
                roll += 3;
            } else if pop >= 6 {
                roll += 1;
            }
        }

        // Starport modifiers
        for port in [origin_port, destination_port] {
            match port {
                PortCode::A => roll += 2,
                PortCode::B => roll += 1,
                PortCode::E => roll -= 1,
                PortCode::X => roll -= 3,
                _ => {}
            }
        }

        // Zone modifiers
        for zone in [origin_zone, destination_zone] {
            match zone {
                ZoneClassification::Green => continue,
                ZoneClassification::Amber => roll += 1,
                ZoneClassification::Red => roll -= 4,
            }
        }

        // Distance modifier - for each parsec past the 1st, apply -1 modifier
        if distance_parsecs > 1 {
            roll -= distance_parsecs - 1;
        }

        // Determine number of dice to roll based on modified result
        let dice_count = match roll {
            i32::MIN..=1 => 0,
            2..=3 => 1,
            4..=6 => 2,
            7..=10 => 3,
            11..=13 => 4,
            14..=15 => 5,
            16 => 6,
            17 => 7,
            18 => 8,
            19 => 9,
            20..=i32::MAX => 10,
        };

        // If we haven't rolled possible passenger counts, roll them all now.
        // We then only use the number we need (dice_count).
        if dice_rolls.is_empty() {
            for _ in 0..10 {
                dice_rolls.push(roll_1d6());
            }
        }

        // Calculate the current passenger count using only the dice we need
        let total: i32 = dice_rolls.iter().take(dice_count as usize).sum();

        total
    }
}
/// Enum representing the class of passenger
#[derive(Debug, Clone, Copy)]
enum PassengerClass {
    High,
    Medium,
    Basic,
    Low,
}

/// Enum representing the class of cargo
#[derive(Debug, Clone, Copy)]
enum CargoClass {
    Major,
    Minor,
    Incidental,
}
