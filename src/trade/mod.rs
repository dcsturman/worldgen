use std::fmt::Display;

pub mod available_goods;
pub mod available_passengers;
pub mod ship_manifest;
pub mod table;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum TradeClass {
    Agricultural,
    Asteroid,
    Barren,
    Desert,
    FluidOceans,
    Garden,
    HighPopulation,
    HighTech,
    IceCapped,
    Industrial,
    LowPopulation,
    LowTech,
    NonAgricultural,
    NonIndustrial,
    Poor,
    Rich,
    Vacuum,
    WaterWorld,
    AmberZone,
    RedZone,
}

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

//const UPP_SPACEPORT: usize = 0;
const UPP_SIZE: usize = 1;
const UPP_ATMOSPHERE: usize = 2;
const UPP_HYDRO: usize = 3;
const UPP_POPULATION: usize = 4;
const UPP_GOVERNMENT: usize = 5;
const UPP_LAW_LEVEL: usize = 6;
const UPP_TECH_LEVEL: usize = 7;

pub fn upp_to_trade_classes(upp: &[char]) -> Vec<TradeClass> {
    assert!(
        upp.len() == 8,
        "Expected UPP to be 8 characters long, got {}",
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

    // Agricultural
    if (4..=9).contains(&atmosphere) && (4..=8).contains(&hydro) && (5..=7).contains(&population) {
        trade_classes.push(TradeClass::Agricultural);
    }

    // Asteroid
    // TODO: Should not be a ring system!
    if size == 0 && atmosphere == 0 && hydro == 0 {
        trade_classes.push(TradeClass::Asteroid);
    }

    // Barren
    if population == 0 && government == 0 && law_level == 0 {
        trade_classes.push(TradeClass::Barren);
    }

    // Desert
    if hydro <= 0 && atmosphere > 1 {
        trade_classes.push(TradeClass::Desert);
    }

    // Fluid
    if hydro >= 1 && atmosphere >= 10 {
        trade_classes.push(TradeClass::FluidOceans);
    }

    // Garden
    if (6..=8).contains(&size) && [5, 6, 8].contains(&atmosphere) && (5..=7).contains(&hydro) {
        trade_classes.push(TradeClass::Garden);
    }

    // High Population
    if population >= 9 {
        trade_classes.push(TradeClass::HighPopulation);
    }

    // High Tech
    if tech_level >= 12 {
        trade_classes.push(TradeClass::HighTech);
    }

    // Ice-Capped
    if atmosphere <= 1 && hydro >= 1 {
        trade_classes.push(TradeClass::IceCapped);
    }
    // Industrial
    if [0, 1, 2, 4, 7, 9, 10, 11, 12].contains(&atmosphere) && population >= 9 {
        trade_classes.push(TradeClass::Industrial);
    }

    // Low Population
    if (1..=3).contains(&population) {
        trade_classes.push(TradeClass::LowPopulation);
    }

    // Low Tech
    if population >= 1 && tech_level <= 5 {
        trade_classes.push(TradeClass::LowTech);
    }

    // Non-Agricultural
    if (0..=3).contains(&atmosphere) && (0..=3).contains(&hydro) && population >= 6 {
        trade_classes.push(TradeClass::NonAgricultural);
    }

    // Non-Industrial
    if (4..=6).contains(&population) {
        trade_classes.push(TradeClass::NonIndustrial);
    }

    // Poor
    // Population check is my addition.
    if population > 0 && (2..=5).contains(&atmosphere) && (0..=3).contains(&hydro) {
        trade_classes.push(TradeClass::Poor);
    }

    // Rich
    if [6, 8].contains(&atmosphere)
        && (6..=8).contains(&population)
        && (4..=9).contains(&government)
    {
        trade_classes.push(TradeClass::Rich);
    }

    // Vacuum
    if atmosphere <= 0 {
        trade_classes.push(TradeClass::Vacuum);
    }

    // Water World
    if ((3..=9).contains(&atmosphere) || atmosphere >= 13) && hydro >= 10 {
        trade_classes.push(TradeClass::WaterWorld);
    }

    trade_classes
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

#[derive(Debug, Clone, Copy)]
pub enum ZoneClassification {
    Amber,
    Red,
}

impl std::fmt::Display for ZoneClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoneClassification::Amber => write!(f, "Amber"),
            ZoneClassification::Red => write!(f, "Red"),
        }
    }
}
