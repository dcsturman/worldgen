//! What fills the orbits: rulebook Sections 5 to 7.4.
//!
//! Giant planets (Tables 15 to 18), ice (Table 19), what fills each remaining
//! orbit (Tables 21 and 22), and each world's size, composition, gravity,
//! atmosphere and hydrographics (Section 7, Table 23). The rolls live here;
//! [`crate::callisto::generate`] decides which orbit each lands in.

use crate::callisto::body::{BodyClass, Composition, GiantKind, IceAvailability};
use crate::callisto::dice::Roller;
use crate::callisto::orbits::Zone;
use crate::callisto::tables;

/// Is there at least one giant planet (Table 15)? Traveller's rate: 2D of 9
/// or less. (Section 13.1's physical rate, 7 or less, is an option for
/// later.)
pub fn giants_present(roller: &mut impl Roller) -> bool {
    let t = tables::table(15).dice().expect("Table 15 is a dice table");
    t.lookup(roller.d2())[0].starts_with("At least one")
}

/// How many giants (Table 16).
pub fn number_of_giants(roller: &mut impl Roller) -> usize {
    let t = tables::table(16).dice().expect("Table 16 is a dice table");
    t.lookup(roller.d2())[0].parse().expect("Table 16 counts are numbers")
}

/// A giant's kind (Table 17): 2D, then 1D for a gas giant's class.
pub fn giant_kind(roller: &mut impl Roller) -> GiantKind {
    let t = tables::table(17).dice().expect("Table 17 is a dice table");
    if t.lookup(roller.d2())[0] == "Ice giant" {
        GiantKind::IceGiant
    } else if roller.d1() <= 4 {
        GiantKind::SaturnClass
    } else {
        GiantKind::JupiterClass
    }
}

/// Where the first giant goes (Table 18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstGiant {
    /// The first free orbit in this zone.
    Zone(Zone),
    /// Migrated inward: the first free Temperate or Hot orbit.
    Migrated,
    /// A hot Jupiter: the first free Inner orbit, clearing everything inside.
    HotJupiter,
}

pub fn first_giant(roller: &mut impl Roller) -> FirstGiant {
    match roller.d1() {
        1..=4 => FirstGiant::Zone(Zone::Outer),
        5 => FirstGiant::Zone(Zone::Cold),
        _ => {
            if roller.d1() <= 5 {
                FirstGiant::Migrated
            } else {
                FirstGiant::HotJupiter
            }
        }
    }
}

/// Ice in the system (Table 19).
pub fn ice(roller: &mut impl Roller) -> IceAvailability {
    let t = tables::table(19).dice().expect("Table 19 is a dice table");
    let cell = &t.lookup(roller.d2())[0];
    if cell.starts_with("Sparse") {
        IceAvailability::Sparse
    } else if cell.starts_with("Rich") {
        IceAvailability::Rich
    } else {
        IceAvailability::Charted
    }
}

/// What Table 21 put in an orbit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filled {
    Empty,
    Body(BodyClass),
}

/// Fill one orbit (Table 21). DM −1 if the star's mass is below 0.3. Next to
/// a giant a result of 5 is Belt: giants shepherd belts.
pub fn fill(zone: Zone, small_star: bool, next_to_giant: bool, roller: &mut impl Roller) -> Filled {
    let t = tables::table(21).dice().expect("Table 21 is a dice table");
    let col = zone as usize;
    let dm = if small_star { -1 } else { 0 };
    let roll = roller.d2() + dm;
    if next_to_giant && roll == 5 {
        return Filled::Body(BodyClass::Belt);
    }
    match t.lookup(roll)[col].as_str() {
        "Empty" => Filled::Empty,
        "Belt" => Filled::Body(BodyClass::Belt),
        "Sub-Neptune" => Filled::Body(BodyClass::SubNeptune),
        "Icy dwarf" => Filled::Body(BodyClass::IcyDwarf),
        _ => Filled::Body(BodyClass::World),
    }
}

/// What kind of body a published world is (Section 6, "Published counts"):
/// Table 21 for its zone, an Empty or Belt result read as World.
pub fn published_kind(zone: Zone, small_star: bool, roller: &mut impl Roller) -> BodyClass {
    match fill(zone, small_star, false, roller) {
        Filled::Body(BodyClass::Belt) | Filled::Empty => BodyClass::World,
        Filled::Body(class) => class,
    }
}

/// A world's size, atmosphere and hydrographics, and what it's made of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Codes {
    pub size: i32,
    pub atmosphere: i32,
    pub hydro: i32,
    pub composition: Option<Composition>,
    pub gravity: Option<f32>,
    pub hydro_is_ice: bool,
}

/// Columns a source already gave, which are kept rather than rolled.
#[derive(Debug, Clone, Copy, Default)]
pub struct Known {
    pub size: Option<i32>,
    pub atmosphere: Option<i32>,
    pub hydro: Option<i32>,
}

/// Table 22's fixed codes for a body that isn't a terrestrial world, and a
/// full set of rolls (Section 7) for one that is.
pub fn codes(
    class: BodyClass,
    zone: Zone,
    small_star: bool,
    known: Known,
    roller: &mut impl Roller,
) -> Codes {
    let cold = matches!(zone, Zone::Cold | Zone::Outer);
    match class {
        BodyClass::Belt => Codes {
            size: 0,
            atmosphere: 0,
            hydro: 0,
            composition: None,
            gravity: None,
            hydro_is_ice: false,
        },
        // Size 1D: B, C, D, E, F, F. Atmosphere A on 1 to 4 else B.
        // Hydrographics 0.
        BodyClass::SubNeptune => Codes {
            size: known.size.unwrap_or_else(|| (roller.d1() + 10).min(15)),
            atmosphere: known
                .atmosphere
                .unwrap_or_else(|| if roller.d1() <= 4 { 10 } else { 11 }),
            hydro: known.hydro.unwrap_or(0),
            composition: None,
            gravity: None,
            hydro_is_ice: false,
        },
        // Size D3, atmosphere 0, hydrographics 1D + 4 as ice.
        BodyClass::IcyDwarf => Codes {
            size: known.size.unwrap_or_else(|| roller.d3()),
            atmosphere: known.atmosphere.unwrap_or(0),
            hydro: known.hydro.unwrap_or_else(|| roller.d1() + 4),
            composition: None,
            gravity: None,
            hydro_is_ice: true,
        },
        BodyClass::World => {
            let size = known.size.unwrap_or_else(|| world_size(zone, small_star, roller));
            let composition = composition(zone, 0, roller);
            let atmosphere = known
                .atmosphere
                .unwrap_or_else(|| atmosphere(size, composition, zone, roller));
            let hydro = known
                .hydro
                .unwrap_or_else(|| hydrographics(size, atmosphere, zone, roller));
            Codes {
                size,
                atmosphere,
                hydro,
                composition: Some(composition),
                gravity: Some(gravity(size, composition)),
                hydro_is_ice: cold && hydro >= 1,
            }
        }
    }
}

/// Size (Section 7.1): 2D − 2, DM −2 Inner, DM −1 small star, at least 1.
pub fn world_size(zone: Zone, small_star: bool, roller: &mut impl Roller) -> i32 {
    let dm = if zone == Zone::Inner { -2 } else { 0 } + if small_star { -1 } else { 0 };
    (roller.d2() - 2 + dm).max(1)
}

/// Composition (Section 7.2): 1D with the zone's DM, plus `extra_dm` (a
/// giant's moon gets +1). 1 or less iron-rich, 2 to 4 rocky, 5 or more
/// ice-rock.
pub fn composition(zone: Zone, extra_dm: i32, roller: &mut impl Roller) -> Composition {
    let dm = match zone {
        Zone::Inner | Zone::Hot => -2,
        Zone::Temperate => -1,
        Zone::Cold => 1,
        Zone::Outer => 2,
    };
    match roller.d1() + dm + extra_dm {
        i32::MIN..=1 => Composition::IronRich,
        2..=4 => Composition::Rocky,
        _ => Composition::IceRock,
    }
}

/// Surface gravity (Table 23), in g. Sizes the table doesn't print come from
/// its formula, size ÷ 8 × density.
pub fn gravity(size: i32, composition: Composition) -> f32 {
    let col = match composition {
        Composition::IceRock => 2,
        Composition::Rocky => 3,
        Composition::IronRich => 4,
    };
    tables::table(23)
        .rows
        .iter()
        .find(|r| crate::util::ehex_to_value(r[0].chars().next().unwrap_or(' ')) == Some(size.max(0) as u32))
        .and_then(|r| r[col].parse().ok())
        .unwrap_or_else(|| size.max(0) as f32 / 8.0 * density(composition))
}

/// Density relative to Earth behind Table 23.
fn density(composition: Composition) -> f32 {
    match composition {
        Composition::IceRock => 0.6,
        Composition::Rocky => 1.0,
        Composition::IronRich => 1.3,
    }
}

/// Atmosphere (Section 7.3): 2D − 7 + size, DM +1 iron-rich, −1 ice-rock,
/// −2 Inner, −1 Outer, −2 size 2. Size 1 has none. Clamped 0 to 15.
pub fn atmosphere(size: i32, composition: Composition, zone: Zone, roller: &mut impl Roller) -> i32 {
    if size <= 1 {
        return 0;
    }
    let dm = match composition {
        Composition::IronRich => 1,
        Composition::IceRock => -1,
        Composition::Rocky => 0,
    } + match zone {
        Zone::Inner => -2,
        Zone::Outer => -1,
        _ => 0,
    } + if size == 2 { -2 } else { 0 };
    (roller.d2() - 7 + size + dm).clamp(0, 15)
}

/// Hydrographics (Section 7.4): 2D − 7 + size, DM −4 if the atmosphere is
/// 0–1 or A+, −2 Hot. Size 1 and every Inner world have none. Clamped 0 to
/// 10.
pub fn hydrographics(size: i32, atmosphere: i32, zone: Zone, roller: &mut impl Roller) -> i32 {
    if size <= 1 || zone == Zone::Inner {
        return 0;
    }
    let dm = if atmosphere <= 1 || atmosphere >= 10 { -4 } else { 0 }
        + if zone == Zone::Hot { -2 } else { 0 };
    (roller.d2() - 7 + size + dm).clamp(0, 10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callisto::dice::{Kind::*, Scripted};

    #[test]
    fn table_23_is_its_formula() {
        for size in 1..=10 {
            for c in [Composition::IceRock, Composition::Rocky, Composition::IronRich] {
                let printed = gravity(size, c);
                let formula = size as f32 / 8.0 * density(c);
                assert!(
                    (printed - formula).abs() <= 0.0051,
                    "size {size} {c:?}: printed {printed}, formula {formula}"
                );
            }
        }
    }

    /// Example 12.1's inner world: 2D = 11 size, 1D = 1 composition, 2D = 10
    /// atmosphere, Inner zone.
    #[test]
    fn example_12_1_venus() {
        let mut r = Scripted::new(&[(D2, 11), (D1, 1), (D2, 10)]);
        let c = codes(BodyClass::World, Zone::Inner, false, Known::default(), &mut r);
        assert_eq!((c.size, c.atmosphere, c.hydro), (7, 9, 0));
        assert_eq!(c.composition, Some(Composition::IronRich));
        assert_eq!(c.gravity, Some(1.14));
    }

    /// Noricum's outermost world: Outer zone, 2D = 5 size, 1D ice-rock,
    /// 2D = 8 atmosphere, 2D = 3 hydrographics.
    #[test]
    fn noricum_icy_world() {
        let mut r = Scripted::new(&[(D2, 5), (D1, 3), (D2, 8), (D2, 3)]);
        let c = codes(BodyClass::World, Zone::Outer, false, Known::default(), &mut r);
        assert_eq!((c.size, c.atmosphere, c.hydro), (3, 2, 0));
        assert_eq!(c.composition, Some(Composition::IceRock));
    }

    #[test]
    fn sub_neptunes_stop_at_f() {
        let size = |d| {
            codes(BodyClass::SubNeptune, Zone::Cold, false, Known::default(), &mut Scripted::new(&[(D1, d), (D1, 1)])).size
        };
        assert_eq!((1..=6).map(size).collect::<Vec<_>>(), [11, 12, 13, 14, 15, 15]);
    }

    #[test]
    fn giants_read_their_tables() {
        assert!(giants_present(&mut Scripted::new(&[(D2, 9)])));
        assert!(!giants_present(&mut Scripted::new(&[(D2, 10)])));
        assert_eq!(number_of_giants(&mut Scripted::new(&[(D2, 8)])), 4);
        assert_eq!(giant_kind(&mut Scripted::new(&[(D2, 4)])), GiantKind::IceGiant);
        assert_eq!(giant_kind(&mut Scripted::new(&[(D2, 9), (D1, 1)])), GiantKind::SaturnClass);
        assert_eq!(first_giant(&mut Scripted::new(&[(D1, 6), (D1, 6)])), FirstGiant::HotJupiter);
    }

    #[test]
    fn fill_reads_the_zone_column() {
        let f = |z, roll| fill(z, false, false, &mut Scripted::new(&[(D2, roll)]));
        assert_eq!(f(Zone::Outer, 12), Filled::Body(BodyClass::IcyDwarf));
        assert_eq!(f(Zone::Inner, 12), Filled::Empty);
        assert_eq!(f(Zone::Hot, 10), Filled::Body(BodyClass::SubNeptune));
        // Next to a giant, a 5 is Belt.
        let near = fill(Zone::Cold, false, true, &mut Scripted::new(&[(D2, 5)]));
        assert_eq!(near, Filled::Body(BodyClass::Belt));
        let far = fill(Zone::Cold, false, false, &mut Scripted::new(&[(D2, 5)]));
        assert_eq!(far, Filled::Body(BodyClass::World));
    }
}
