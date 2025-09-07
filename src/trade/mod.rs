//! # Trade Module
//!
//! This module provides core trade-related functionality for the Traveller universe,
//! including trade classifications, starport codes, zone classifications, and
//! utilities for generating trade data from Universal World Profiles (UWPs).

use serde::{Deserialize, Serialize};
use std::fmt::Display;
pub mod available_goods;
pub mod available_passengers;
pub mod ship_manifest;
pub mod table;

/// Trade classifications that determine a world's economic characteristics
///
/// These classifications affect trade good availability, pricing modifiers,
/// and economic relationships between worlds. A world can have multiple
/// trade classifications based on its physical and social characteristics.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeClass {
    /// Agricultural world - produces food and organic materials
    ///
    /// Requirements: Atmosphere 4-9, Hydrographics 4-8, Population 5-7
    Agricultural,

    /// Asteroid belt or planetoid - mining and low-gravity manufacturing
    ///
    /// Requirements: Size 0, Atmosphere 0, Hydrographics 0
    Asteroid,

    /// Barren world - no permanent population or government
    ///
    /// Requirements: Population 0, Government 0, Law Level 0
    Barren,

    /// Desert world - arid conditions with minimal water
    ///
    /// Requirements: Hydrographics 0, Atmosphere 2+
    Desert,

    /// Fluid oceans world - exotic liquid oceans
    ///
    /// Requirements: Hydrographics 1+, Atmosphere 10+
    FluidOceans,

    /// Garden world - ideal living conditions
    ///
    /// Requirements: Size 6-8, Atmosphere 5/6/8, Hydrographics 5-7
    Garden,

    /// High population world - densely populated
    ///
    /// Requirements: Population 9+
    HighPopulation,

    /// High technology world - advanced manufacturing
    ///
    /// Requirements: Tech Level 12+
    HighTech,

    /// Ice-capped world - frozen water at poles or surface
    ///
    /// Requirements: Atmosphere 0-1, Hydrographics 1+
    IceCapped,

    /// Industrial world - heavy manufacturing and processing
    ///
    /// Requirements: Atmosphere 0/1/2/4/7/9/10/11/12, Population 9+
    Industrial,

    /// Low population world - sparsely populated
    ///
    /// Requirements: Population 1-3
    LowPopulation,

    /// Low technology world - primitive technology
    ///
    /// Requirements: Population 1+, Tech Level 0-5
    LowTech,

    /// Non-agricultural world - cannot produce sufficient food
    ///
    /// Requirements: Atmosphere 0-3, Hydrographics 0-3, Population 6+
    NonAgricultural,

    /// Non-industrial world - limited manufacturing capability
    ///
    /// Requirements: Population 4-6
    NonIndustrial,

    /// Poor world - limited economic development
    ///
    /// Requirements: Population 1+, Atmosphere 2-5, Hydrographics 0-3
    Poor,

    /// Rich world - prosperous economy
    ///
    /// Requirements: Atmosphere 6/8, Population 6-8, Government 4-9
    Rich,

    /// Vacuum world - no atmosphere
    ///
    /// Requirements: Atmosphere 0
    Vacuum,

    /// Water world - extensive water coverage
    ///
    /// Requirements: Atmosphere 3-9/13+, Hydrographics 10
    WaterWorld,

    /// Amber zone - travel advisory in effect
    ///
    /// Dangerous conditions requiring caution
    AmberZone,

    /// Red zone - travel prohibited
    ///
    /// Extremely dangerous or interdicted world
    RedZone,
}

/// Display the two-character trade code for a TradeClass
impl Display for TradeClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeClass::Agricultural => write!(f, "Ag"),
            TradeClass::Asteroid => write!(f, "As"),
            TradeClass::Barren => write!(f, "Ba"),
            TradeClass::Desert => write!(f, "De"),
            TradeClass::FluidOceans => write!(f, "Fl"),
            TradeClass::Garden => write!(f, "Ga"),
            TradeClass::HighPopulation => write!(f, "Hi"),
            TradeClass::HighTech => write!(f, "Ht"),
            TradeClass::IceCapped => write!(f, "Ic"),
            TradeClass::Industrial => write!(f, "In"),
            TradeClass::LowPopulation => write!(f, "Lo"),
            TradeClass::LowTech => write!(f, "Lt"),
            TradeClass::NonAgricultural => write!(f, "Na"),
            TradeClass::NonIndustrial => write!(f, "Ni"),
            TradeClass::Poor => write!(f, "Po"),
            TradeClass::Rich => write!(f, "Ri"),
            TradeClass::Vacuum => write!(f, "Va"),
            TradeClass::WaterWorld => write!(f, "Wa"),
            TradeClass::AmberZone => write!(f, "Az"),
            TradeClass::RedZone => write!(f, "Rz"),
        }
    }
}

/// Converts a two-character trade code string to a TradeClass enum
///
/// # Arguments
///
/// * `code` - Two-character trade classification code (e.g., "Ag", "Hi", "In")
///
/// # Returns
///
/// Some(TradeClass) if the code is recognized, None otherwise
///
/// # Examples
///
/// ```
/// use worldgen::trade::{string_to_trade_class, TradeClass};
///
/// assert_eq!(string_to_trade_class("Ag"), Some(TradeClass::Agricultural));
/// assert_eq!(string_to_trade_class("Hi"), Some(TradeClass::HighPopulation));
/// assert_eq!(string_to_trade_class("XX"), None);
/// ```
pub fn string_to_trade_class(code: &str) -> Option<TradeClass> {
    match code {
        "Ag" => Some(TradeClass::Agricultural),
        "As" => Some(TradeClass::Asteroid),
        "Ba" => Some(TradeClass::Barren),
        "De" => Some(TradeClass::Desert),
        "Fl" => Some(TradeClass::FluidOceans),
        "Ga" => Some(TradeClass::Garden),
        "Hi" => Some(TradeClass::HighPopulation),
        "Ht" => Some(TradeClass::HighTech),
        "Ic" => Some(TradeClass::IceCapped),
        "In" => Some(TradeClass::Industrial),
        "Lo" => Some(TradeClass::LowPopulation),
        "Lt" => Some(TradeClass::LowTech),
        "Na" => Some(TradeClass::NonAgricultural),
        "Ni" => Some(TradeClass::NonIndustrial),
        "Po" => Some(TradeClass::Poor),
        "Ri" => Some(TradeClass::Rich),
        "Va" => Some(TradeClass::Vacuum),
        "Wa" => Some(TradeClass::WaterWorld),
        "Az" => Some(TradeClass::AmberZone),
        "Rz" => Some(TradeClass::RedZone),
        _ => None,
    }
}

// UWP (Universal World Profile) field indices
/// Index for starport quality in UWP string
//const UPP_SPACEPORT: usize = 0;
/// Index for world size in UWP string
const UPP_SIZE: usize = 1;
/// Index for atmosphere type in UWP string
const UPP_ATMOSPHERE: usize = 2;
/// Index for hydrographics percentage in UWP string
const UPP_HYDRO: usize = 3;
/// Index for population level in UWP string
const UPP_POPULATION: usize = 4;
/// Index for government type in UWP string
const UPP_GOVERNMENT: usize = 5;
/// Index for law level in UWP string
const UPP_LAW_LEVEL: usize = 6;
/// Index for technology level in UWP string
const UPP_TECH_LEVEL: usize = 7;

/// Converts a Universal World Profile (UWP) to applicable trade classifications
///
/// Analyzes the physical and social characteristics encoded in a UWP string
/// to determine which trade classifications apply to the world.
///
/// # Arguments
///
/// * `upp` - Array of 8 characters representing the UWP (e.g., ['A','7','8','8','8','9','9','A'])
///
/// # Returns
///
/// Vector of applicable TradeClass enums
///
/// # Panics
///
/// Panics if the UWP is not exactly 8 characters long
///
/// # Examples
///
/// ```
/// use worldgen::trade::{upp_to_trade_classes, TradeClass};
///
/// let upp: Vec<char> = "A788899A".chars().collect();
/// let trade_classes = upp_to_trade_classes(&upp);
/// // Returns applicable trade classes based on the UWP characteristics
/// ```
pub fn upp_to_trade_classes(upp: &[char]) -> Vec<TradeClass> {
    assert!(
        upp.len() == 8,
        "Expected UWP to be 8 characters long, got {}",
        upp.len()
    );
    let mut trade_classes = Vec::new();
    let size = upp[UPP_SIZE].to_digit(16).unwrap() as i32;
    let atmosphere = upp[UPP_ATMOSPHERE].to_digit(16).unwrap() as i32;
    let hydro = upp[UPP_HYDRO].to_digit(16).unwrap() as i32;
    let population = upp[UPP_POPULATION].to_digit(16).unwrap() as i32;
    let government = upp[UPP_GOVERNMENT].to_digit(16).unwrap() as i32;
    let law_level = upp[UPP_LAW_LEVEL].to_digit(16).unwrap() as i32;
    let tech_level = upp[UPP_TECH_LEVEL].to_digit(16).unwrap() as i32;

    // Agricultural: Atmosphere 4-9, Hydrographics 4-8, Population 5-7
    if (4..=9).contains(&atmosphere) && (4..=8).contains(&hydro) && (5..=7).contains(&population) {
        trade_classes.push(TradeClass::Agricultural);
    }

    // Asteroid: Size 0, Atmosphere 0, Hydrographics 0
    // TODO: Should not be a ring system!
    if size == 0 && atmosphere == 0 && hydro == 0 {
        trade_classes.push(TradeClass::Asteroid);
    }

    // Barren: Population 0, Government 0, Law Level 0
    if population == 0 && government == 0 && law_level == 0 {
        trade_classes.push(TradeClass::Barren);
    }

    // Desert: Hydrographics 0, Atmosphere 2+
    if hydro <= 0 && atmosphere > 1 {
        trade_classes.push(TradeClass::Desert);
    }

    // Fluid Oceans: Hydrographics 1+, Atmosphere 10+
    if hydro >= 1 && atmosphere >= 10 {
        trade_classes.push(TradeClass::FluidOceans);
    }

    // Garden: Size 6-8, Atmosphere 5/6/8, Hydrographics 5-7
    if (6..=8).contains(&size) && [5, 6, 8].contains(&atmosphere) && (5..=7).contains(&hydro) {
        trade_classes.push(TradeClass::Garden);
    }

    // High Population: Population 9+
    if population >= 9 {
        trade_classes.push(TradeClass::HighPopulation);
    }

    // High Tech: Tech Level 12+
    if tech_level >= 12 {
        trade_classes.push(TradeClass::HighTech);
    }

    // Ice-Capped: Atmosphere 0-1, Hydrographics 1+
    if atmosphere <= 1 && hydro >= 1 {
        trade_classes.push(TradeClass::IceCapped);
    }

    // Industrial: Atmosphere 0/1/2/4/7/9/10/11/12, Population 9+
    if [0, 1, 2, 4, 7, 9, 10, 11, 12].contains(&atmosphere) && population >= 9 {
        trade_classes.push(TradeClass::Industrial);
    }

    // Low Population: Population 1-3
    if (1..=3).contains(&population) {
        trade_classes.push(TradeClass::LowPopulation);
    }

    // Low Tech: Population 1+, Tech Level 0-5
    if population >= 1 && tech_level <= 5 {
        trade_classes.push(TradeClass::LowTech);
    }

    // Non-Agricultural: Atmosphere 0-3, Hydrographics 0-3, Population 6+
    if (0..=3).contains(&atmosphere) && (0..=3).contains(&hydro) && population >= 6 {
        trade_classes.push(TradeClass::NonAgricultural);
    }

    // Non-Industrial: Population 4-6
    if (4..=6).contains(&population) {
        trade_classes.push(TradeClass::NonIndustrial);
    }

    // Poor: Population 1+, Atmosphere 2-5, Hydrographics 0-3
    // Population check is my addition.
    if population > 0 && (2..=5).contains(&atmosphere) && (0..=3).contains(&hydro) {
        trade_classes.push(TradeClass::Poor);
    }

    // Rich: Atmosphere 6/8, Population 6-8, Government 4-9
    if [6, 8].contains(&atmosphere)
        && (6..=8).contains(&population)
        && (4..=9).contains(&government)
    {
        trade_classes.push(TradeClass::Rich);
    }

    // Vacuum: Atmosphere 0
    if atmosphere <= 0 {
        trade_classes.push(TradeClass::Vacuum);
    }

    // Water World: Atmosphere 3-9/13+, Hydrographics 10
    if ((3..=9).contains(&atmosphere) || atmosphere >= 13) && hydro >= 10 {
        trade_classes.push(TradeClass::WaterWorld);
    }

    trade_classes
}

/// Starport quality classifications
///
/// Represents the quality and capabilities of a world's starport facilities,
/// affecting trade, refueling, maintenance, and shipyard services.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PortCode {
    /// Excellent starport - Full shipyard, refined fuel, all services
    A,
    /// Good starport - Shipyard (non-starships), refined fuel, most services
    B,
    /// Routine starport - Minor repairs, unrefined fuel, basic services
    C,
    /// Poor starport - Limited repairs, unrefined fuel, minimal services
    D,
    /// Frontier starport - No repairs, no fuel, landing area only
    E,
    /// No starport - No facilities, dangerous landing
    #[default]
    X,
    /// No starport - No facilities, no landing possible
    Y,
    /// Primitive starport - No fuel, basic landing area
    H,
    /// Poor starport variant - Limited facilities
    G,
    /// Routine starport variant - Basic facilities
    F,
}

impl PortCode {
    /// Creates a PortCode from the first character of a UWP string
    ///
    /// # Arguments
    ///
    /// * `upp` - UWP string starting with the starport code
    ///
    /// # Returns
    ///
    /// PortCode enum value, defaults to A if character is not recognized
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

impl std::fmt::Display for PortCode {
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

/// Travel zone classifications for worlds
///
/// Indicates the safety level and travel restrictions for a world,
/// affecting passenger traffic, trade, and insurance rates.
#[derive(Debug, Clone, PartialEq, Copy, Eq, Serialize, Deserialize, Default)]
pub enum ZoneClassification {
    #[default]
    /// Green zone - Safe for travel, no restrictions
    Green,
    /// Amber zone - Caution advised, potential dangers
    Amber,
    /// Red zone - Travel prohibited, extreme danger or interdiction
    Red,
}

impl std::fmt::Display for ZoneClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoneClassification::Green => write!(f, "Green"),
            ZoneClassification::Amber => write!(f, "Amber"),
            ZoneClassification::Red => write!(f, "Red"),
        }
    }
}

impl From<&str> for ZoneClassification {
    fn from(s: &str) -> Self {
        match s {
            "Amber" => ZoneClassification::Amber,
            "Red" => ZoneClassification::Red,
            _ => ZoneClassification::Green,
        }
    }
}
