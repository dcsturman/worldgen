//! Settlement of the other worlds: rulebook Section 11, Tables 36 to 39.
//!
//! Every world, moon of size 1 or more, and belt in a system whose main world
//! has people gets a population, government, law level, facilities, tech
//! level and spaceport, in that order. The rolls lean on the physics: warmth,
//! air and water decide how many settle a surface; a belt's people live in
//! habitats, so only the main world's technology matters there.
//!
//! Table 36's main-world tech-level rows, the research station's DM, and the
//! fuel depot's roll are the user's (2026-10-14), not the rulebook's first
//! draft: it listed tech level 7 under two rows, gave a research station +2
//! from tech level 10 and +1 on a "remarkable" world, and put a depot on
//! every settled moon of a giant fuel source.

use crate::callisto::dice::Roller;
use crate::callisto::temperature::TempBand;
use crate::systems::constraint::PartialUwp;
use crate::systems::world::{Facility, World};
use crate::trade::{PortCode, TradeClass};

/// What the main world brings to every other body's rolls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainWorld {
    pub population: i32,
    pub government: i32,
    pub law: i32,
    pub tech: i32,
    pub atmosphere: i32,
    pub industrial: bool,
    pub rich: bool,
    pub poor: bool,
}

impl MainWorld {
    pub fn of(w: &World) -> MainWorld {
        let has = |t: TradeClass| w.get_trade_classes().contains(&t);
        MainWorld {
            population: w.get_population(),
            government: w.get_government(),
            law: w.get_law_level(),
            tech: w.tech_level,
            atmosphere: w.atmosphere,
            industrial: has(TradeClass::Industrial),
            rich: has(TradeClass::Rich),
            poor: has(TradeClass::Poor),
        }
    }

    /// Does anyone live off the main world (Table 36's last row)? Not with
    /// nobody there, nor at tech level 6 or less: no spaceflight, and no way
    /// to support such a world.
    pub fn settles(&self) -> bool {
        self.population >= 1 && self.tech >= 7
    }
}

/// The body being settled, as the tables read it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Body {
    /// A belt skips Table 36's surface rows.
    pub belt: bool,
    pub size: i32,
    pub atmosphere: i32,
    pub hydro: i32,
    pub band: Option<TempBand>,
    pub gravity: Option<f32>,
    pub iron_rich: bool,
    /// A moon in the Close band of a gas giant: inside its radiation belts.
    pub radiation: bool,
    /// The system's nearest fuel source (a giant's moon counts as one).
    pub fuel_source: bool,
    /// A moon of the nearest fuel source.
    pub moon_of_fuel_source: bool,
}

/// A body's settlement: the extended profile, spaceport first.
#[derive(Debug, Clone, PartialEq)]
pub struct Settlement {
    pub port: PortCode,
    /// Table 39's modified 1D, kept so a fuel depot placed afterwards can add
    /// its +1 (see [`with_fuel_depot`]). `None` when a source states the port.
    pub port_roll: Option<i32>,
    pub population: i32,
    pub government: i32,
    pub law: i32,
    pub tech: i32,
    pub facilities: Vec<Facility>,
}

/// Table 36's DM on a body's 2D − 2 population roll.
pub fn population_dm(b: &Body, mw: &MainWorld) -> i32 {
    let surface = if b.belt {
        -4
    } else {
        let band = match b.band {
            Some(TempBand::Temperate) | None => 0,
            Some(TempBand::Hot | TempBand::Cold) => -2,
            Some(TempBand::Roasting | TempBand::Frozen) => -4,
        };
        let air = match b.atmosphere {
            5 | 6 | 8 => 0,
            4 | 7 | 9 => -1,
            0 | 1 => -3,
            11 | 12 => -4,
            _ => -2,
        };
        band + air
            - i32::from(b.hydro == 0)
            - i32::from((1..=2).contains(&b.size))
            - i32::from(b.gravity.is_some_and(|g| g > 1.5))
            - 2 * i32::from(b.radiation)
    };
    let tech = match mw.tech {
        12.. => 2,
        10 | 11 => 1,
        7 | 8 => -2,
        _ => 0,
    };
    let people = match mw.population {
        8.. => 1,
        ..=3 => -2,
        _ => 0,
    };
    surface
        + i32::from(b.fuel_source || b.moon_of_fuel_source)
        + tech
        + i32::from(mw.industrial || mw.rich)
        + people
}

/// Settle one body (Section 11). A column a source states is kept and not
/// rolled; the rest follow from it.
pub fn settle(b: &Body, mw: &MainWorld, source: Option<&PartialUwp>, roller: &mut impl Roller) -> Settlement {
    let stated = |f: fn(&PartialUwp) -> Option<u8>| source.and_then(f).map(i32::from);

    // 11.1: never more than the main world's population minus 1.
    let population = stated(|u| u.population).unwrap_or_else(|| {
        (roller.d2() - 2 + population_dm(b, mw)).clamp(0, (mw.population - 1).max(0))
    });

    // 11.2 and Table 37.
    let government = stated(|u| u.government).unwrap_or_else(|| match population {
        0 => 0,
        1 | 2 => i32::from(roller.d1() >= 4),
        _ if mw.government == 6 => 6,
        _ => match roller.d1() + i32::from(mw.government >= 7) {
            1 => 0,
            2 => 1,
            3 => 2,
            4 => 3,
            _ => 6,
        },
    });

    // 11.3.
    let law = stated(|u| u.law).unwrap_or_else(|| {
        if government == 0 {
            0
        } else {
            (roller.d1() - 3 + mw.law).max(0)
        }
    });

    // 11.4 and Table 38, each line in order.
    let mut facilities = Vec::new();
    if b.band == Some(TempBand::Temperate)
        && (4..=9).contains(&b.atmosphere)
        && (4..=8).contains(&b.hydro)
        && population >= 2
    {
        facilities.push(Facility::Farming);
    }
    if population >= 2 && (b.belt || b.iron_rich || mw.industrial) {
        facilities.push(Facility::Mining);
    }
    // A fuel depot (Table 38's third line) is one per system, so it is
    // placed once every body is settled: see `fuel_depot`.
    if government == 6 && population >= 5 {
        facilities.push(Facility::Colony);
    }
    if mw.tech >= 9 && population >= 1 && roller.d2() + 2 * i32::from(mw.tech >= 12) >= 11 {
        facilities.push(Facility::Lab);
    }
    if population >= 1 && !mw.poor {
        let dm = i32::from(mw.population >= 8) + 2 * i32::from(b.atmosphere == mw.atmosphere);
        if roller.d2() + dm >= 12 {
            facilities.push(Facility::Military);
        }
    }

    // 11.5: one behind the main world, level with it for a research station
    // or a base, and at least 7 wherever people need life support.
    let tech = stated(|u| u.tech).unwrap_or_else(|| {
        if population == 0 {
            return 0;
        }
        let level = facilities.iter().any(|f| matches!(f, Facility::Lab | Facility::Military));
        let tech = if level { mw.tech } else { mw.tech - 1 };
        let life_support =
            b.belt || !matches!(b.atmosphere, 5 | 6 | 8) || b.band.is_some_and(|t| t != TempBand::Temperate);
        if life_support { tech.max(7) } else { tech.max(0) }
    });

    // 11.6 and Table 39.
    let port_roll = source.and_then(|u| u.port).is_none().then(|| {
        let dm = match population {
            0 => -3,
            1 => -2,
            6.. => 2,
            _ => 0,
        } + i32::from(facilities.contains(&Facility::Colony));
        roller.d1() + dm
    });
    let port = source.and_then(|u| u.port).unwrap_or_else(|| table_39(port_roll.unwrap_or(0)));

    Settlement {
        port,
        port_roll,
        population,
        government,
        law,
        tech,
        facilities,
    }
}

/// Table 39: a spaceport from its modified 1D.
pub fn table_39(roll: i32) -> PortCode {
    match roll {
        ..=2 => PortCode::Y,
        3 => PortCode::H,
        4 | 5 => PortCode::G,
        _ => PortCode::F,
    }
}

/// Does the system's fuel source have a depot (Table 38)? Rolled once per
/// system, for its most populous settled body at the source: none below
/// main-world tech level 9, then 2D for 8 or more, DM +2 at tech level 12
/// or more.
pub fn fuel_depot(mw: &MainWorld, roller: &mut impl Roller) -> bool {
    mw.tech >= 9 && roller.d2() + 2 * i32::from(mw.tech >= 12) >= 8
}

/// A settlement with a fuel depot: the facility in Table 38's order, and its
/// +1 on the spaceport roll, unless a colony has already given it.
pub fn with_fuel_depot(facilities: &mut Vec<Facility>, port: &mut PortCode, port_roll: Option<i32>) {
    let colony = facilities.contains(&Facility::Colony);
    let at = facilities
        .iter()
        .position(|f| matches!(f, Facility::Colony | Facility::Lab | Facility::Military))
        .unwrap_or(facilities.len());
    facilities.insert(at, Facility::FuelDepot);
    if let (Some(roll), false) = (port_roll, colony) {
        *port = table_39(roll + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callisto::dice::{Kind::*, Scripted};
    use crate::callisto::tables;

    /// A TL 12, population 9, industrial main world with law 4 and
    /// government 4.
    fn regina_like() -> MainWorld {
        MainWorld {
            population: 9,
            government: 4,
            law: 4,
            tech: 12,
            atmosphere: 7,
            industrial: true,
            rich: false,
            poor: false,
        }
    }

    fn garden() -> Body {
        Body {
            size: 7,
            atmosphere: 6,
            hydro: 6,
            band: Some(TempBand::Temperate),
            gravity: Some(0.9),
            ..Body::default()
        }
    }

    fn row(table: u32, prefix: &str) -> String {
        tables::table(table)
            .rows
            .iter()
            .find(|r| r[0].starts_with(prefix))
            .unwrap_or_else(|| panic!("Table {table} has no row starting {prefix:?}"))
            .join(" | ")
    }

    /// Table 36's DMs are code; each must still be printed in its row.
    #[test]
    fn table_36_figures_are_still_printed() {
        for (prefix, dm) in [
            ("Temperature band Temperate", "0"),
            ("Temperature band Hot or Cold", "−2"),
            ("Temperature band Roasting or Frozen", "−4"),
            ("Atmosphere 5, 6 or 8", "0"),
            ("Atmosphere 4, 7 or 9", "−1"),
            ("Atmosphere 2, 3, A, D, E or F", "−2"),
            ("Atmosphere 0 or 1", "−3"),
            ("Atmosphere B or C", "−4"),
            ("Hydrographics 0", "−1"),
            ("Size 1 or 2", "−1"),
            ("Gravity above 1.5", "−1"),
            ("A moon in the Close band of a gas giant", "−2"),
            ("Belt", "−4"),
            ("The system's nearest fuel source", "+1"),
            ("Main world tech level 12 or more", "+2"),
            ("Main world tech level 10 or 11", "+1"),
            ("Main world tech level 7 or 8", "−2"),
            ("Main world industrial (In) or rich (Ri)", "+1"),
            ("Main world population 8 or more", "+1"),
            ("Main world population 3 or less", "−2"),
            ("Main world population 0, or main world tech level 6 or less", "population 0, no roll"),
        ] {
            let printed = row(36, prefix);
            assert!(printed.ends_with(&format!("| {dm}")), "Table 36 {prefix}: {printed}");
        }
    }

    /// Tables 37 to 39 are code; their figures must still be printed.
    #[test]
    fn tables_37_to_39_are_still_printed() {
        let t37 = tables::table(37);
        assert!(t37.note.contains("7 or higher") && t37.note.contains("government is 6, the result is 6"));
        for (d, code) in [("1", "0"), ("2", "1"), ("3", "2"), ("4", "3"), ("5 or more", "6")] {
            assert!(t37.rows.iter().any(|r| r[0] == d && r[1] == code), "Table 37 {d} → {code}");
        }
        assert!(row(38, "Farming").contains("atmosphere 4 to 9, hydrographics 4 to 8, population 2 or more"));
        assert!(row(38, "Research station").contains("11 or more"));
        assert!(row(38, "Research station").contains("DM +2 if the main world's tech level is 12 or more"));
        assert!(row(38, "Fuel depot").contains("One per system"));
        assert!(row(38, "Fuel depot").contains("tech level is 8 or less"));
        assert!(row(38, "Fuel depot").contains("2D for 8 or more, DM +2 if the main world's tech level is 12 or more"));
        assert!(row(38, "Research station").contains("tech level is 8 or less"));
        assert!(row(38, "Military base").contains("12 or more"));
        assert!(row(38, "Colony").contains("Government 6 and population 5 or more"));
        let t39 = tables::table(39);
        for figure in ["+2 if population 6", "−2 if population 1", "−3 if population 0", "+1 for a fuel depot or a colony"] {
            assert!(t39.note.contains(figure), "Table 39 note: {figure}");
        }
        for (d, code) in [("2 or less", "Y"), ("3", "H"), ("4 or 5", "G"), ("6 or more", "F")] {
            assert!(t39.rows.iter().any(|r| r[0] == d && r[1] == code), "Table 39 {d} → {code}");
        }
    }

    #[test]
    fn no_one_leaves_a_low_tech_or_empty_main_world() {
        let mw = regina_like();
        assert!(mw.settles());
        assert!(!MainWorld { tech: 6, ..mw }.settles());
        assert!(MainWorld { tech: 7, ..mw }.settles());
        assert!(!MainWorld { population: 0, ..mw }.settles());
    }

    #[test]
    fn population_dms() {
        let mw = regina_like();
        // A garden world: TL 12 +2, industrial +1, population 9 +1.
        assert_eq!(population_dm(&garden(), &mw), 4);
        // A belt skips the surface rows: −4 + 4.
        let belt = Body { belt: true, ..Body::default() };
        assert_eq!(population_dm(&belt, &mw), 0);
        // An airless, frozen, dry little moon in a giant's radiation belts,
        // around a TL 8 world of a thousand people: −4 −3 −1 −1 −2, −2 TL,
        // −2 population.
        let moon = Body {
            size: 2,
            band: Some(TempBand::Frozen),
            radiation: true,
            ..Body::default()
        };
        let small = MainWorld { tech: 8, population: 3, industrial: false, ..mw };
        assert_eq!(population_dm(&moon, &small), -15);
        // TL 9 is neither help nor hindrance; TL 10 and 11 help a little.
        assert_eq!(population_dm(&belt, &MainWorld { tech: 9, ..mw }), -2);
        assert_eq!(population_dm(&belt, &MainWorld { tech: 11, ..mw }), -1);
        // The nearest fuel source, or a moon of it, +1.
        let depot = Body { fuel_source: true, ..belt };
        assert_eq!(population_dm(&depot, &mw), 1);
    }

    /// Never more than the main world's population minus 1.
    #[test]
    fn population_is_capped_by_the_main_world() {
        let mw = MainWorld { population: 4, ..regina_like() };
        // 2D 12 − 2 + 4 → 14, capped at 3; then government (1D 3 = 2),
        // law (1D 4), research (2D 2), military (2D 2), spaceport (1D 3).
        let mut r = Scripted::new(&[(D2, 12), (D1, 3), (D1, 4), (D2, 2), (D2, 2), (D1, 3)]);
        let s = settle(&garden(), &mw, None, &mut r);
        assert_eq!(r.remaining(), 0);
        assert_eq!((s.population, s.government, s.law), (3, 2, 5));
        // Temperate, breathable: no life support, so one behind the main world.
        assert_eq!(s.tech, 11);
        assert_eq!(s.port, PortCode::H);
        // Farming (temperate, air, water, 2+), mining (an industrial main world).
        assert_eq!(s.facilities, vec![Facility::Farming, Facility::Mining]);
    }

    /// Nobody there: government, law and tech level 0 without a roll, and
    /// only the spaceport is rolled (−3).
    #[test]
    fn an_empty_world_rolls_only_its_spaceport() {
        let mut r = Scripted::new(&[(D2, 2), (D1, 6)]);
        let rock = Body { band: Some(TempBand::Roasting), ..Body::default() };
        let s = settle(&rock, &regina_like(), None, &mut r);
        assert_eq!(r.remaining(), 0);
        assert_eq!((s.population, s.government, s.law, s.tech), (0, 0, 0, 0));
        assert_eq!(s.port, PortCode::H);
        assert!(s.facilities.is_empty());
    }

    /// A belt needs life support: at least tech level 7, here one behind a
    /// TL 8 main world would be 7 anyway; behind TL 7 it stays 7.
    #[test]
    fn life_support_needs_tech_level_7() {
        let mw = MainWorld { tech: 7, ..regina_like() };
        let belt = Body { belt: true, ..Body::default() };
        // 2D 12 − 2 − 4 − 2 + 1 + 1 = 6; government 1D 5 (+0) = 6, so no
        // law roll skipped; law 1D 3; no research below TL 9; military 2D 2;
        // spaceport 1D 4 + 2.
        let mut r = Scripted::new(&[(D2, 12), (D1, 5), (D1, 3), (D2, 2), (D1, 4)]);
        let s = settle(&belt, &mw, None, &mut r);
        assert_eq!(r.remaining(), 0);
        assert_eq!(s.population, 6);
        assert_eq!(s.government, 6);
        assert_eq!(s.tech, 7);
        assert_eq!(s.port, PortCode::F);
        assert_eq!(s.facilities, vec![Facility::Mining, Facility::Colony]);
    }

    /// Population 1 or 2: a few families (1D 1–3) or a company (4–6).
    #[test]
    fn a_handful_of_people() {
        let mw = MainWorld { industrial: false, ..regina_like() };
        let rock = Body { band: Some(TempBand::Cold), ..Body::default() };
        // Cold −2, airless −3, dry −1, TL 12 +2, population 9 +1: DM −3.
        // 2D 6 − 2 − 3 = 1; government 1D 4: a company.
        let mut r = Scripted::new(&[(D2, 6), (D1, 4), (D1, 1), (D2, 2), (D2, 2), (D1, 6)]);
        let s = settle(&rock, &mw, None, &mut r);
        assert_eq!(r.remaining(), 0);
        assert_eq!((s.population, s.government), (1, 1));
        // Law 1D 1 − 3 + 4 = 2; airless, so life support: TL 11 all the same.
        assert_eq!((s.law, s.tech), (2, 11));
        // Spaceport 1D 6 − 2.
        assert_eq!(s.port, PortCode::G);
    }

    /// A column a source states is kept, and its roll isn't made.
    #[test]
    fn stated_columns_are_kept() {
        let stated = PartialUwp {
            port: Some(PortCode::B),
            population: Some(5),
            government: Some(1),
            ..PartialUwp::default()
        };
        // Only law, research, military: no population, government or port roll.
        let mut r = Scripted::new(&[(D1, 3), (D2, 2), (D2, 2)]);
        let s = settle(&garden(), &regina_like(), Some(&stated), &mut r);
        assert_eq!(r.remaining(), 0);
        assert_eq!((s.port, s.population, s.government, s.law), (PortCode::B, 5, 1, 4));
    }

    /// A research station: 2D 11+, +2 only from main-world tech level 12.
    #[test]
    fn research_stations() {
        let lab = |tech: i32, d2: i32| {
            let mw = MainWorld { tech, ..regina_like() };
            // Population (2D 8), government (1D 2), law (1D 3), research,
            // military (2D 2), spaceport (1D 3).
            let mut script = vec![(D2, 8), (D1, 2), (D1, 3)];
            if tech >= 9 {
                script.push((D2, d2));
            }
            script.extend([(D2, 2), (D1, 3)]);
            let mut r = Scripted::new(&script);
            let s = settle(&garden(), &mw, None, &mut r);
            assert_eq!(r.remaining(), 0);
            s.facilities.contains(&Facility::Lab)
        };
        assert!(!lab(8, 0), "none at tech level 8 or less");
        assert!(!lab(11, 10) && lab(11, 11), "11+ without a DM below tech level 12");
        assert!(!lab(12, 8) && lab(12, 9), "+2 from tech level 12");
    }

    /// A fuel depot: none below main-world tech level 9, then 2D 8+, +2 from
    /// tech level 12.
    #[test]
    fn fuel_depots() {
        let depot = |tech: i32, script: &[(crate::callisto::dice::Kind, i32)]| {
            let mut r = Scripted::new(script);
            let d = fuel_depot(&MainWorld { tech, ..regina_like() }, &mut r);
            assert_eq!(r.remaining(), 0);
            d
        };
        assert!(!depot(8, &[]));
        assert!(!depot(9, &[(D2, 7)]) && depot(9, &[(D2, 8)]));
        assert!(!depot(12, &[(D2, 5)]) && depot(12, &[(D2, 6)]));
    }

    /// A depot goes in Table 38's order and adds +1 to the spaceport roll,
    /// unless a colony has already given it.
    #[test]
    fn a_depot_improves_the_spaceport() {
        let mut f = vec![Facility::Mining, Facility::Lab];
        let mut port = PortCode::G;
        with_fuel_depot(&mut f, &mut port, Some(5));
        assert_eq!(f, vec![Facility::Mining, Facility::FuelDepot, Facility::Lab]);
        assert_eq!(port, PortCode::F);
        let mut f = vec![Facility::Colony];
        let mut port = PortCode::G;
        with_fuel_depot(&mut f, &mut port, Some(5));
        assert_eq!(port, PortCode::G, "a colony's +1 already counted");
        let mut port = PortCode::B;
        with_fuel_depot(&mut vec![], &mut port, None);
        assert_eq!(port, PortCode::B, "a stated port stays");
    }

    /// The main world's government 6 makes every larger settlement captive.
    #[test]
    fn a_captive_government() {
        let mw = MainWorld { government: 6, ..regina_like() };
        let mut r = Scripted::new(&[(D2, 8), (D1, 3), (D2, 2), (D2, 2), (D1, 3)]);
        let s = settle(&garden(), &mw, None, &mut r);
        assert_eq!(r.remaining(), 0);
        assert_eq!(s.government, 6);
        assert!(s.facilities.contains(&Facility::Colony));
    }
}
