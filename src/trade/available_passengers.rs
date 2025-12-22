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
    pub high_roll: i32,
    /// Raw 2d6 roll for medium passage (saved for recalculation)
    pub medium_roll: i32,
    /// Raw 2d6 roll for basic passage (saved for recalculation)
    pub basic_roll: i32,
    /// Raw 2d6 roll for low passage (saved for recalculation)
    pub low_roll: i32,
    /// Individual 1d6 rolls for high passage count (saved for recalculation)
    pub high_dice_rolls: Vec<i32>,
    /// Individual 1d6 rolls for medium passage count (saved for recalculation)
    pub medium_dice_rolls: Vec<i32>,
    /// Individual 1d6 rolls for basic passage count (saved for recalculation)
    pub basic_dice_rolls: Vec<i32>,
    /// Individual 1d6 rolls for low passage count (saved for recalculation)
    pub low_dice_rolls: Vec<i32>,

    /// Raw 2d6 roll for major cargo (saved for recalculation)
    pub major_cargo_roll: i32,
    /// Broker skill check roll for major cargo (saved for recalculation)
    pub major_cargo_check_roll: i32,
    /// Raw 2d6 roll for minor cargo (saved for recalculation)
    pub minor_cargo_roll: i32,
    /// Broker skill check roll for minor cargo (saved for recalculation)
    pub minor_cargo_check_roll: i32,
    /// Raw 2d6 roll for incidental cargo (saved for recalculation)
    pub incidental_cargo_roll: i32,
    /// Broker skill check roll for incidental cargo (saved for recalculation)
    pub incidental_cargo_check_roll: i32,
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
    ) -> Self {
        let mut passengers = Self::default();

        // Generate passengers
        Self::generate_passengers(
            &mut passengers,
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
        Self::generate_freight(
            &mut passengers,
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

        passengers
    }

    /// Recalculates passengers and freight using saved dice rolls
    ///
    /// This allows skill and world parameter changes to update counts without re-rolling
    #[allow(clippy::too_many_arguments)]
    pub fn recalculate(
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
        // Recalculate passengers
        Self::recalc_passengers(
            self,
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
        );

        // Recalculate freight
        Self::recalc_freight(
            self,
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
        passengers: &mut AvailablePassengers,
        origin_population: i32,
        origin_port: PortCode,
        origin_zone: ZoneClassification,
        destination_population: i32,
        destination_port: PortCode,
        destination_zone: ZoneClassification,
        distance_parsecs: i32,
        steward_skill: i32,
    ) {
        let (count, roll, dice_rolls) = Self::generate_passenger_class(
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            PassengerClass::High,
        );
        passengers.high = count;
        passengers.high_roll = roll;
        passengers.high_dice_rolls = dice_rolls;

        let (count, roll, dice_rolls) = Self::generate_passenger_class(
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            PassengerClass::Medium,
        );
        passengers.medium = count;
        passengers.medium_roll = roll;
        passengers.medium_dice_rolls = dice_rolls;

        let (count, roll, dice_rolls) = Self::generate_passenger_class(
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            PassengerClass::Basic,
        );
        passengers.basic = count;
        passengers.basic_roll = roll;
        passengers.basic_dice_rolls = dice_rolls;

        let (count, roll, dice_rolls) = Self::generate_passenger_class(
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            PassengerClass::Low,
        );
        passengers.low = count;
        passengers.low_roll = roll;
        passengers.low_dice_rolls = dice_rolls;
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
        passengers: &mut AvailablePassengers,
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
        // Generate major cargo
        let (num_lots, base_roll, check_roll) = Self::generate_cargo_class(
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
            CargoClass::Major,
        );
        passengers.major_cargo_roll = base_roll;
        passengers.major_cargo_check_roll = check_roll;
        for _ in 0..num_lots {
            let size_roll = roll_1d6();
            let size = size_roll * 10;
            passengers.freight_lots.push(FreightLot { size, size_roll });
        }

        // Generate minor cargo
        let (num_lots, base_roll, check_roll) = Self::generate_cargo_class(
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
            CargoClass::Minor,
        );
        passengers.minor_cargo_roll = base_roll;
        passengers.minor_cargo_check_roll = check_roll;
        for _ in 0..num_lots {
            let size_roll = roll_1d6();
            let size = size_roll * 5;
            passengers.freight_lots.push(FreightLot { size, size_roll });
        }

        // Generate incidental cargo
        let (num_lots, base_roll, check_roll) = Self::generate_cargo_class(
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
            CargoClass::Incidental,
        );
        passengers.incidental_cargo_roll = base_roll;
        passengers.incidental_cargo_check_roll = check_roll;
        for _ in 0..num_lots {
            let size_roll = roll_1d6();
            let size = size_roll;
            passengers.freight_lots.push(FreightLot { size, size_roll });
        }

        // Sort freight lots by size (largest first)
        passengers.freight_lots.sort_by(|a, b| b.size.cmp(&a.size));
    }

    /// Recalculates passengers using saved dice rolls
    #[allow(clippy::too_many_arguments)]
    fn recalc_passengers(
        passengers: &mut AvailablePassengers,
        origin_population: i32,
        origin_port: PortCode,
        origin_zone: ZoneClassification,
        destination_population: i32,
        destination_port: PortCode,
        destination_zone: ZoneClassification,
        distance_parsecs: i32,
        steward_skill: i32,
    ) {
        passengers.high = Self::recalc_passenger_class(
            passengers.high_roll,
            &passengers.high_dice_rolls,
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            PassengerClass::High,
        );

        passengers.medium = Self::recalc_passenger_class(
            passengers.medium_roll,
            &passengers.medium_dice_rolls,
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            PassengerClass::Medium,
        );

        passengers.basic = Self::recalc_passenger_class(
            passengers.basic_roll,
            &passengers.basic_dice_rolls,
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            PassengerClass::Basic,
        );

        passengers.low = Self::recalc_passenger_class(
            passengers.low_roll,
            &passengers.low_dice_rolls,
            origin_population,
            origin_port,
            origin_zone,
            destination_population,
            destination_port,
            destination_zone,
            distance_parsecs,
            steward_skill,
            PassengerClass::Low,
        );
    }

    /// Recalculates freight using saved dice rolls
    #[allow(clippy::too_many_arguments)]
    fn recalc_freight(
        passengers: &mut AvailablePassengers,
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
        // Save the existing freight lots with their size rolls
        let saved_lots = passengers.freight_lots.clone();
        passengers.freight_lots.clear();

        let mut saved_index = 0;

        // Recalculate major cargo
        let num_lots = Self::recalc_cargo_class(
            passengers.major_cargo_roll,
            passengers.major_cargo_check_roll,
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
            CargoClass::Major,
        );
        for _ in 0..num_lots {
            // Reuse saved size roll if available, otherwise generate new one
            let size_roll = if saved_index < saved_lots.len() {
                let roll = saved_lots[saved_index].size_roll;
                saved_index += 1;
                roll
            } else {
                roll_1d6()
            };
            let size = size_roll * 10;
            passengers.freight_lots.push(FreightLot { size, size_roll });
        }

        // Recalculate minor cargo
        let num_lots = Self::recalc_cargo_class(
            passengers.minor_cargo_roll,
            passengers.minor_cargo_check_roll,
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
            CargoClass::Minor,
        );
        for _ in 0..num_lots {
            // Reuse saved size roll if available, otherwise generate new one
            let size_roll = if saved_index < saved_lots.len() {
                let roll = saved_lots[saved_index].size_roll;
                saved_index += 1;
                roll
            } else {
                roll_1d6()
            };
            let size = size_roll * 5;
            passengers.freight_lots.push(FreightLot { size, size_roll });
        }

        // Recalculate incidental cargo
        let num_lots = Self::recalc_cargo_class(
            passengers.incidental_cargo_roll,
            passengers.incidental_cargo_check_roll,
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
            CargoClass::Incidental,
        );
        for _ in 0..num_lots {
            // Reuse saved size roll if available, otherwise generate new one
            let size_roll = if saved_index < saved_lots.len() {
                let roll = saved_lots[saved_index].size_roll;
                saved_index += 1;
                roll
            } else {
                roll_1d6()
            };
            let size = size_roll;
            passengers.freight_lots.push(FreightLot { size, size_roll });
        }

        // Sort freight lots by size (largest first)
        passengers.freight_lots.sort_by(|a, b| b.size.cmp(&a.size));
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
        cargo_class: CargoClass,
    ) -> (i32, i32, i32) {
        // Initial 2d6 roll - SAVE THIS
        let base_roll = roll_2d6();
        let mut roll = base_roll;

        // Modify by effect of player broker skill check - SAVE THIS
        let check_roll = roll_2d6();
        let check = player_broker_skill + check_roll - 8;
        roll += check;

        // Cargo class modifiers
        match cargo_class {
            CargoClass::Major => roll -= 4,
            CargoClass::Incidental => roll += 2,
            _ => {}
        }

        // Population modifiers
        for pop in [origin_population, destination_population] {
            if pop <= 1 {
                roll -= 4;
            } else if pop >= 8 {
                roll += 4;
            } else if pop >= 6 {
                roll += 2;
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

        // Tech level modifiers
        for tech_level in [origin_tech_level, destination_tech_level] {
            if tech_level <= 6 {
                roll -= 1;
            } else if tech_level >= 9 {
                roll += 2;
            }
        }

        // Zone modifiers
        for zone in [origin_zone, destination_zone] {
            match zone {
                ZoneClassification::Green => continue,
                ZoneClassification::Amber => roll -= 2,
                ZoneClassification::Red => roll -= 6,
            }
        }

        // Distance modifier
        if distance_parsecs > 1 {
            roll -= distance_parsecs - 1;
        }

        (roll, base_roll, check_roll)
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
        passenger_class: PassengerClass,
    ) -> (i32, i32, Vec<i32>) {
        // Initial 2d6 roll - SAVE THIS
        let base_roll = roll_2d6();
        let mut roll = base_roll;

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

        // ALWAYS roll and save 10 dice (the maximum possible)
        // This allows us to recalculate with different skills later
        // without losing information when skills increase
        let mut dice_rolls = Vec::new();
        for _ in 0..10 {
            dice_rolls.push(roll_1d6());
        }

        // Calculate the current passenger count using only the dice we need
        let total: i32 = dice_rolls.iter().take(dice_count as usize).sum();

        (total, base_roll, dice_rolls)
    }

    /// Recalculates passenger count using saved dice rolls
    #[allow(clippy::too_many_arguments)]
    fn recalc_passenger_class(
        base_roll: i32,
        dice_rolls: &[i32],
        origin_population: i32,
        origin_port: PortCode,
        origin_zone: ZoneClassification,
        destination_population: i32,
        destination_port: PortCode,
        destination_zone: ZoneClassification,
        distance_parsecs: i32,
        steward_skill: i32,
        passenger_class: PassengerClass,
    ) -> i32 {
        // Start with saved base roll
        let mut roll = base_roll;

        // Apply modifiers (same as generate_passenger_class)
        roll += steward_skill;

        match passenger_class {
            PassengerClass::High => roll -= 4,
            PassengerClass::Low => roll += 1,
            _ => {}
        }

        if origin_population <= 1 {
            roll -= 4;
        }
        if destination_population <= 1 {
            roll -= 4;
        }

        for pop in [origin_population, destination_population] {
            if pop >= 8 {
                roll += 3;
            } else if pop >= 6 {
                roll += 1;
            }
        }

        for port in [origin_port, destination_port] {
            match port {
                PortCode::A => roll += 2,
                PortCode::B => roll += 1,
                PortCode::E => roll -= 1,
                PortCode::X => roll -= 3,
                _ => {}
            }
        }

        for zone in [origin_zone, destination_zone] {
            match zone {
                ZoneClassification::Green => continue,
                ZoneClassification::Amber => roll += 1,
                ZoneClassification::Red => roll -= 4,
            }
        }

        if distance_parsecs > 1 {
            roll -= distance_parsecs - 1;
        }

        // Determine number of dice based on modified result
        let dice_count = match roll {
            i32::MIN..=1 => return 0,
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

        // Use saved dice rolls, sum only the number we need
        dice_rolls.iter().take(dice_count as usize).sum()
    }

    /// Recalculates cargo lot count using saved dice rolls
    #[allow(clippy::too_many_arguments)]
    fn recalc_cargo_class(
        base_roll: i32,
        check_roll: i32,
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
        cargo_class: CargoClass,
    ) -> i32 {
        // Start with saved base roll
        let mut roll = base_roll;

        // Apply broker skill check using saved check roll
        let check = player_broker_skill + check_roll - 8;
        roll += check;

        // Apply all modifiers (same as generate_cargo_class)
        match cargo_class {
            CargoClass::Major => roll -= 4,
            CargoClass::Incidental => roll += 2,
            _ => {}
        }

        for pop in [origin_population, destination_population] {
            if pop <= 1 {
                roll -= 4;
            } else if pop >= 8 {
                roll += 4;
            } else if pop >= 6 {
                roll += 2;
            }
        }

        for port in [origin_port, destination_port] {
            match port {
                PortCode::A => roll += 2,
                PortCode::B => roll += 1,
                PortCode::E => roll -= 1,
                PortCode::X => roll -= 3,
                _ => {}
            }
        }

        for tech_level in [origin_tech_level, destination_tech_level] {
            if tech_level <= 6 {
                roll -= 1;
            } else if tech_level >= 9 {
                roll += 2;
            }
        }

        for zone in [origin_zone, destination_zone] {
            match zone {
                ZoneClassification::Green => continue,
                ZoneClassification::Amber => roll -= 2,
                ZoneClassification::Red => roll -= 6,
            }
        }

        if distance_parsecs > 1 {
            roll -= distance_parsecs - 1;
        }

        roll.max(0)
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
