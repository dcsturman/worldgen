//! What Callisto knows about a body beyond its UWP, and the system's fuel.
//!
//! Carried on [`World::callisto`](crate::systems::world::World) and
//! [`GasGiant::callisto`](crate::systems::gas_giant::GasGiant), both `None`
//! on a Book 6 system. On `World` it is serialized, so it follows the
//! pattern `decorations` set: `#[serde(default)]` and skipped when absent,
//! and a stored Book 6 world round-trips byte-identically.

use serde::{Deserialize, Serialize};

use crate::callisto::orbits::Zone;

/// What sort of body fills an orbit (Table 22). Giants are
/// [`GasGiant`](crate::systems::gas_giant::GasGiant)s, not worlds, and carry
/// [`GiantKind`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyClass {
    /// A terrestrial planet, from an airless rock to a garden world.
    World,
    /// A planetoid belt: rock and metal inside, ice further out.
    Belt,
    /// Between Earth and Neptune in size, under a thick envelope. Not
    /// landable, not usefully skimmable.
    SubNeptune,
    /// A small ice body like Pluto.
    IcyDwarf,
}

impl BodyClass {
    pub fn name(self) -> &'static str {
        match self {
            BodyClass::World => "World",
            BodyClass::Belt => "Belt",
            BodyClass::SubNeptune => "Sub-Neptune",
            BodyClass::IcyDwarf => "Icy dwarf",
        }
    }
}

/// What a world is made of (Section 7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Composition {
    /// A large metal core, like Mercury.
    IronRich,
    /// Like Earth or Venus.
    Rocky,
    /// Like Ganymede or Callisto.
    IceRock,
}

impl Composition {
    pub fn name(self) -> &'static str {
        match self {
            Composition::IronRich => "iron-rich",
            Composition::Rocky => "rocky",
            Composition::IceRock => "ice-rock",
        }
    }
}

/// A world's Callisto facts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Physics {
    pub class: BodyClass,
    pub zone: Zone,
    /// Position around its star, HD.
    pub position_hd: f32,
    /// Terrestrial worlds only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition: Option<Composition>,
    /// Surface gravity in g (Table 23); terrestrial worlds only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity: Option<f32>,
    /// The hydrographics digit is ice, not liquid (Cold and Outer zones,
    /// icy dwarfs).
    #[serde(default)]
    pub hydro_is_ice: bool,
    /// A place to land and melt ice for fuel (Section 5.5).
    #[serde(default)]
    pub ice_source: bool,
}

/// The kinds of giant planet (Table 17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GiantKind {
    /// Uranus and Neptune: 45,000 to 55,000 km.
    IceGiant,
    /// 60,000 to 100,000 km.
    SaturnClass,
    /// 120,000 to 160,000 km.
    JupiterClass,
}

impl GiantKind {
    pub fn name(self) -> &'static str {
        match self {
            GiantKind::IceGiant => "Ice giant",
            GiantKind::SaturnClass => "Gas giant, Saturn-class",
            GiantKind::JupiterClass => "Gas giant, Jupiter-class",
        }
    }

    /// Diameter range in km, from Table 17.
    pub fn diameter_km(self) -> std::ops::RangeInclusive<u32> {
        match self {
            GiantKind::IceGiant => 45_000..=55_000,
            GiantKind::SaturnClass => 60_000..=100_000,
            GiantKind::JupiterClass => 120_000..=160_000,
        }
    }
}

/// How much ice a system has (Table 19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceAvailability {
    /// Scattered small bodies that have to be searched for.
    Sparse,
    /// A charted ice belt, or a giant's icy moons.
    Charted,
    /// Charted, and every Outer-zone body has surface ice.
    Rich,
}

/// The best fuel a ship passing through can use without visiting the main
/// world (Section 5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitFuel {
    GiantPlanet,
    ChartedIce,
    UnchartedIce,
}

impl TransitFuel {
    pub fn name(self) -> &'static str {
        match self {
            TransitFuel::GiantPlanet => "giant planet",
            TransitFuel::ChartedIce => "charted ice",
            TransitFuel::UnchartedIce => "uncharted ice",
        }
    }
}

/// The system's refuelling line (Section 5.6).
#[derive(Debug, Clone, PartialEq)]
pub struct Fuel {
    pub ice: IceAvailability,
    pub transit: TransitFuel,
    /// Days at thrust 1 from the main world to the nearest fuel source.
    /// `None` without a main world, or with no charted source.
    pub local_days: Option<f32>,
    /// That source's name.
    pub local_source: Option<String>,
}

impl Fuel {
    /// "convenient" under a week, "reachable" under three, else "not
    /// reachable" on a normal fuel load.
    pub fn local_reach(&self) -> Option<&'static str> {
        self.local_days.map(|d| {
            if d < 7.0 {
                "convenient"
            } else if d < 21.0 {
                "reachable"
            } else {
                "not reachable"
            }
        })
    }
}
