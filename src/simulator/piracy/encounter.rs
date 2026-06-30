//! The Prey Encounter table and its per-die modifiers.
//!
//! An encounter is a D66 roll (first die = tens, second die = units) with
//! per-die modifiers derived from the system's traits. Because the
//! modifiers can push a die outside the usual `1..=6`, the printed table
//! spans `00..=78`; we clamp the first digit to `0..=7` and the second to
//! `0..=8` (the table's edges).
//!
//! Per the design, a "Traveller" or "Unusual Vessel" cell is treated as no
//! encounter (the latter parked for a future, more interesting treatment).

use rand::Rng;

use super::roll_1d6;
use crate::simulator::types::EncounterType;
use crate::systems::world::{Facility, World};
use crate::trade::{PortCode, TradeClass, ZoneClassification};

/// Per-die modifiers for the encounter roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EncounterDms {
    /// Modifier on the first die (traffic: more prey).
    pub traffic: i32,
    /// Modifier on the second die (security: more defenders).
    pub security: i32,
}

/// Returns `true` if `world` has a naval base in-system.
pub fn naval_in_system(world: &World) -> bool {
    world.has_facility(Facility::Naval)
}

/// Build the encounter modifiers from the mainworld's traits.
///
/// Traffic (first die): high-traffic / on-the-trade-route `+1`, backwater `-1`.
/// Security (second die): naval base `+2`, else secure `+1`, dangerous `-1`,
/// plus a polity modifier — patrolled empires are more dangerous than
/// non-aligned space (Imperium / Aslan / K'kree / Hiver `+1`, Zhodani `+2`).
///
/// `allegiance` is the TravellerMap allegiance code (e.g. `"Im"`, `"AsMw"`,
/// `"Zh"`, `"NaHu"`).
pub fn encounter_dms(world: &World, allegiance: Option<&str>) -> EncounterDms {
    let port = world.port;
    let law = world.get_law_level();
    let classes = world.get_trade_classes();
    let has = |c: TradeClass| classes.contains(&c);

    let good_port = matches!(port, PortCode::A | PortCode::B);
    let high_traffic = good_port
        && (has(TradeClass::HighTech)
            || has(TradeClass::HighPopulation)
            || has(TradeClass::Industrial)
            || has(TradeClass::Agricultural)
            || has(TradeClass::Rich));
    let backwater = matches!(port, PortCode::X | PortCode::E | PortCode::Y);

    let mut traffic = 0;
    // High-traffic *or* on a charted trade route is a single +1 (the two routes
    // don't stack — being on either grants the one bonus).
    if high_traffic || on_trade_route(&world.name) {
        traffic += 1;
    }
    if backwater {
        traffic -= 1;
    }

    let naval = naval_in_system(world);
    // "Secure": Law 7+ and the technology to protect travellers.
    let secure = law >= 7 && (world.tech_level >= 12 || has(TradeClass::HighTech));
    // "Dangerous": Amber/Red travel zone, or Law 3 or less.
    let dangerous = !matches!(world.travel_zone, ZoneClassification::Green) || law <= 3;

    let mut security = 0;
    if naval {
        security += 2;
    } else if secure {
        security += 1;
    }
    if dangerous {
        security -= 1;
    }
    security += allegiance_security_dm(allegiance);

    EncounterDms { traffic, security }
}

/// Security modifier from a world's polity: a patrolled empire is more
/// dangerous than non-aligned space. Imperium / Aslan Hierate / K'kree / Hiver
/// Federation `+1`; Zhodani Consulate `+2`; non-aligned, client states, and
/// unknown `0`.
///
/// The prefix tests catch *every* code of each polity — `"As"` covers all the
/// Aslan clan codes (`AsMw`, `AsSc`, `AsT0`–`AsT9`, `AsWc`, …), `"Im"` all the
/// Imperial sub-codes, and so on.
fn allegiance_security_dm(allegiance: Option<&str>) -> i32 {
    match allegiance.map(str::trim) {
        Some(a) if a.starts_with("Zh") => 2,
        Some(a)
            if a.starts_with("Im")
                || a.starts_with("As")
                || a.starts_with("Kk")
                || a.starts_with("Hv") =>
        {
            1
        }
        _ => 0,
    }
}

/// Whether a world sits on a charted trade route — the Aslan–Imperium route
/// (the Trojan Reach corridor) or the Florian/Imperial route. Either grants the
/// single high-traffic bonus. Matched by world name (case-insensitive).
fn on_trade_route(world_name: &str) -> bool {
    const ROUTE_WORLDS: &[&str] = &[
        // Florian / Imperial trade route.
        "Yggdrasil",
        "Dpres",
        "Connaught",
        "291-540",
        "Janus",
        "Caldos",
        "Sagan",
        "Tyr",
        "Tktk",
        "Hecarda",
        "Acis",
        "Ace",
        "Number One",
        "Thebus",
        "Noricum",
        "Oghma",
        "Marduk",
        "Borite",
        "Torpol",
        "Blue",
        "Clarke",
        // Aslan–Imperium trade route (Trojan Reach corridor).
        "Asim",
        "Drinax",
        "Pourne",
        "Hilfer",
        "Paal",
        "Sink",
        "Exocet",
        "Iilgan",
        "Wildeman",
        "Pandora",
        "Tanith",
        "Arunisiir",
        "Acrid",
        "Cordan",
        "Umemii",
        "Argona",
        "Exe",
        "Inurin",
        "Sperle",
        "Tech-World",
        "Falcon",
        "Ergo",
        "Byrni",
        "The World",
    ];
    ROUTE_WORLDS
        .iter()
        .any(|&w| w.eq_ignore_ascii_case(world_name))
}

/// Roll the (clamped) D66 dice given the modifiers. Returns `(first, second)`
/// where `first ∈ 0..=7` and `second ∈ 0..=8` (the table edges).
pub fn roll_d66_clamped(dms: &EncounterDms, rng: &mut impl Rng) -> (u8, u8) {
    let d1 = rng.random_range(1..=6) + dms.traffic;
    let d2 = rng.random_range(1..=6) + dms.security;
    let first = d1.clamp(0, 7) as u8;
    let second = d2.clamp(0, 8) as u8;
    (first, second)
}

/// Look up the encounter for a clamped `(first, second)` pair, rolling any
/// sub-rolls the table cell calls for. "Traveller" and "Unusual Vessel"
/// cells return [`EncounterType::None`].
pub fn encounter_for(first: u8, second: u8, rng: &mut impl Rng) -> EncounterType {
    use EncounterType::*;

    // The units==8 column is the patrol column: mostly a "1-3 none / 4-6
    // naval patrol" sub-roll, with a few plain Naval Patrols.
    if second == 8 {
        return match first {
            0 | 6 | 7 => NavalPatrol,
            5 => {
                // 58: 1-2 No encounter; 3-6 Naval Patrol.
                if roll_1d6(rng) >= 3 { NavalPatrol } else { None }
            }
            _ => {
                // 1x..4x: 1-3 No encounter; 4-6 Naval Patrol.
                if roll_1d6(rng) >= 4 { NavalPatrol } else { None }
            }
        };
    }

    match (first, second) {
        // --- units 0: mostly Travellers (treated as no encounter) ---
        (0..=3 | 5, 0) => None,    // Traveller
        (4, 0) => SmallFreighter,
        (6 | 7, 0) => None,        // Traveller

        // --- units 1 ---
        (0..=2, 1) => None,
        (3, 1) => SmallFreighter,
        (4, 1) => None,            // Traveller
        (5 | 6, 1) => Convoy,
        (7, 1) => SmallFreighter,

        // --- units 2 ---
        (0 | 1, 2) => None,
        (2, 2) => None,
        (3, 2) => Convoy,
        (4, 2) => Convoy,
        (5..=7, 2) => SmallFreighter,

        // --- units 3 ---
        (0, 3) => None,
        (1, 3) => None,
        (2, 3) => SmallFreighter,
        (3, 3) => None,            // Unusual Vessel (parked)
        (4, 3) => HeavyFreighter,
        (5..=7, 3) => MediumFreighter,

        // --- units 4 ---
        (1, 4) => SmallFreighter,
        (2, 4) => MediumFreighter,
        (3, 4) => MediumFreighter,
        (0 | 4, 4) => None,
        (5, 4) => None,
        (6, 4) => Liner,
        (7, 4) => None,            // Unusual Vessel (parked)

        // --- units 5 ---
        (0, 5) => None,
        (1, 5) => None,
        (2, 5) => None,
        (3, 5) => None,
        (4, 5) => None,
        (5, 5) => HeavyFreighter,
        (6, 5) => Convoy,
        (7, 5) => Liner,

        // --- units 6 ---
        (0, 6) => None,
        (1, 6) => None,
        (2, 6) => None,            // Unusual Vessel (parked)
        (3, 6) => None,
        (4, 6) => Liner,
        (5, 6) => Liner,
        (6, 6) => RichFreighter,
        (7, 6) => RichFreighter,

        // --- units 7 ---
        (1, 7) => MediumFreighter,
        (2, 7) => SystemDefenceBoat,
        (3, 7) => RichFreighter,
        (4 | 5, 7) => SystemDefenceBoat,
        (6, 7) => SystemDefenceBoat,
        (7, 7) => SystemDefenceBoat,
        (0, 7) => None,            // Traveller (00..07 are travellers / none)

        // Defensive fallback for any unmapped cell.
        _ => None,
    }
}

/// Determine whether a System Defence Boat is actually a disguised q-ship
/// (a 1d6 of 6).
pub fn is_q_ship(rng: &mut impl Rng) -> bool {
    roll_1d6(rng) == 6
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::world::World;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    /// UWP columns: Port Size Atmo Hydro Pop Gov Law - Tech.
    fn world(uwp: &str) -> World {
        let mut w = World::from_uwp("Test", uwp, false, true).unwrap();
        w.gen_trade_classes();
        w
    }

    #[test]
    fn high_traffic_world_has_traffic_bonus() {
        // A-port, high pop → high-traffic; also on the trade route → +1.
        let w = world("A788999-A");
        let dms = encounter_dms(&w, Some("NaHu"));
        assert_eq!(dms.traffic, 1);
    }

    #[test]
    fn backwater_off_route_pushes_traffic_down() {
        // X-port backwater, not on a trade route → -1.
        let w = world("X788855-5");
        let dms = encounter_dms(&w, Some("NaHu"));
        assert_eq!(dms.traffic, -1);
    }

    #[test]
    fn trade_route_world_gets_traffic_bonus() {
        // A plain C-port world (not high-traffic) named for a trade-route world
        // still gets the +1.
        let mut on = World::from_uwp("Drinax", "C544540-8", false, true).unwrap();
        on.gen_trade_classes();
        assert_eq!(encounter_dms(&on, Some("NaHu")).traffic, 1);

        let mut off = World::from_uwp("Nowhere", "C544540-8", false, true).unwrap();
        off.gen_trade_classes();
        assert_eq!(encounter_dms(&off, Some("NaHu")).traffic, 0);
    }

    #[test]
    fn dangerous_low_law_pushes_security_down() {
        // Law 2 → dangerous; non-aligned, so no polity modifier.
        let w = world("C788852-5");
        let dms = encounter_dms(&w, Some("NaHu"));
        assert_eq!(dms.security, -1);
    }

    #[test]
    fn naval_base_pushes_security_up() {
        // A-port, law 9, tech D(13); a naval base sets security to +2.
        let mut w = world("A78889D-D");
        w.set_facilities(vec![Facility::Naval]);
        let dms = encounter_dms(&w, Some("NaHu"));
        assert_eq!(dms.security, 2);
    }

    #[test]
    fn patrolled_polities_raise_security() {
        // A quiet C-port, law 5 (not dangerous, not secure), so the polity
        // modifier is the whole security DM.
        let w = world("C788855-7");
        assert_eq!(encounter_dms(&w, Some("NaHu")).security, 0);
        assert_eq!(encounter_dms(&w, Some("Im")).security, 1);
        assert_eq!(encounter_dms(&w, Some("AsMw")).security, 1);
        assert_eq!(encounter_dms(&w, Some("Kk")).security, 1);
        assert_eq!(encounter_dms(&w, Some("Hv")).security, 1);
        assert_eq!(encounter_dms(&w, Some("Zh")).security, 2);
        assert_eq!(encounter_dms(&w, Some("CsIm")).security, 0);
    }

    #[test]
    fn clamp_keeps_dice_in_table_bounds() {
        let mut rng = StdRng::seed_from_u64(1);
        // Big positive DMs should never exceed the table edges.
        let dms = EncounterDms { traffic: 5, security: 5 };
        for _ in 0..100 {
            let (f, s) = roll_d66_clamped(&dms, &mut rng);
            assert!(f <= 7, "first digit {f} out of bounds");
            assert!(s <= 8, "second digit {s} out of bounds");
        }
    }

    #[test]
    fn travellers_are_no_encounter() {
        let mut rng = StdRng::seed_from_u64(2);
        // (0,0) is a Traveller cell.
        assert_eq!(encounter_for(0, 0, &mut rng), EncounterType::None);
    }

    #[test]
    fn small_freighter_cell() {
        let mut rng = StdRng::seed_from_u64(3);
        assert_eq!(encounter_for(4, 0, &mut rng), EncounterType::SmallFreighter);
    }
}
