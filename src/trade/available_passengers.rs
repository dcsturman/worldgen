//! # Available Passengers Module
//! 
//! This module handles the generation of available passengers and freight
//! for interstellar travel between worlds in the Traveller universe.

use crate::util::{roll_1d6, roll_2d6};
use crate::trade::{PortCode, ZoneClassification};

/// Represents a lot of freight available for shipping at a specific world
#[derive(Debug, Clone)]
pub struct FreightLot {
    /// Size in tons (1-60)
    pub size: i32,
}

/// Represents available passengers (by class of passage) and freight for a route between two worlds
#[derive(Debug, Clone, Default)]
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
        );

        passengers
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
        passengers.high = Self::generate_passenger_class(
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

        passengers.medium = Self::generate_passenger_class(
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

        passengers.basic = Self::generate_passenger_class(
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

        passengers.low = Self::generate_passenger_class(
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
    ) {
        // Generate each cargo class
        for cargo_class in [CargoClass::Major, CargoClass::Minor, CargoClass::Incidental] {
            let num_lots = Self::generate_cargo_class(
                origin_population,
                origin_port,
                origin_zone,
                origin_tech_level,
                destination_population,
                destination_port,
                destination_zone,
                destination_tech_level,
                distance_parsecs,
                cargo_class,
            );

            // Generate individual lots
            for _ in 0..num_lots {
                let size = match cargo_class {
                    CargoClass::Major => roll_1d6() * 10,
                    CargoClass::Minor => roll_1d6() * 5,
                    CargoClass::Incidental => roll_1d6(),
                };
                passengers.freight_lots.push(FreightLot { size });
            }
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
    /// The number of cargo lots available for the specified class
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
        cargo_class: CargoClass,
    ) -> i32 {
        // Initial 2d6 roll
        let mut roll = roll_2d6();

        // Cargo class modifiers
        match cargo_class {
            CargoClass::Major => roll -= 4,
            CargoClass::Incidental => roll += 2,
            _ => {}
        }

        // Population modifiers
        if origin_population <= 1 {
            roll -= 4;
        }
        if destination_population <= 1 {
            roll -= 4;
        }

        // Population bonuses
        for pop in [origin_population, destination_population] {
            if pop >= 8 {
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

        // Determine number of dice to roll based on modified result
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

        // Roll the determined number of d6
        let mut total = 0;
        for _ in 0..dice_count {
            total += roll_1d6();
        }

        total
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
    /// The number of passengers available for the specified class
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
    ) -> i32 {
        // Initial 2d6 roll
        let mut roll = roll_2d6();

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

        // Roll the determined number of d6
        let mut total = 0;
        for _ in 0..dice_count {
            total += roll_1d6();
        }

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
