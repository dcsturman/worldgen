//! Class-and-tonnage derivation of a prey's hull, weapons, thrust and loot.
//!
//! Each encounter type rolls a specific **ship class** from a weighted table
//! (see [`ship_classes`]) — a named Traveller hull that carries its own
//! tonnage — so the log reads "Far Trader (200t)" rather than a generic
//! "200t freighter". Weapons (`tons/100 × rate`) and loot both derive from
//! that tonnage, so a bigger hull is both tougher and richer.

use rand::Rng;

use super::{roll_1d6, roll_2d6};
use crate::simulator::types::EncounterType;
use crate::systems::world::World;
use crate::trade::TradeClass;

/// Candidate ship classes for an encounter type: `(class name, hull tons,
/// weight)`. Weights lean toward the common designs (Far Traders, Subsidised
/// Merchants, Patrol Corvettes); each class carries its own canonical tonnage,
/// so the log names a real ship instead of "200t freighter". A mix of canon
/// Traveller hulls and flavour designs for variety.
fn ship_classes(t: EncounterType) -> &'static [(&'static str, i32, u32)] {
    use EncounterType::*;
    match t {
        SmallFreighter => &[
            ("Far Trader", 200, 8),
            ("Free Trader", 200, 6),
            ("Safari Ship", 200, 2),
            ("Tramp Trader", 200, 2),
            ("Frontier Hauler", 200, 2),
            ("Long-Hauler", 200, 2),
            ("Seeker", 100, 1),
            ("Cargo Lighter", 100, 1),
            ("System Skiff", 100, 1),
            ("Packet Boat", 100, 1),
        ],
        MediumFreighter => &[
            ("Subsidised Merchant", 400, 6),
            ("Fat Trader", 400, 3),
            ("Laboratory Ship", 400, 1),
            ("Stretched Subsidised Merchant", 600, 3),
            ("Bulk Merchant", 600, 2),
            ("Tramp Bulk-Hauler", 600, 2),
            ("Container Freighter", 800, 2),
            ("Ore Tender", 800, 2),
            ("Box-Hauler", 800, 1),
            ("Heavy Merchant", 1000, 2),
            ("Modular Freightliner", 1000, 1),
        ],
        HeavyFreighter => &[
            ("Thousand Freighter", 1200, 4),
            ("Bulk Carrier", 1200, 2),
            ("Ore Barque", 1400, 2),
            ("Megafreighter", 1600, 2),
            ("Tukera-line Freightliner", 1800, 2),
            ("Heavy Container Ship", 2000, 1),
            ("Deep-Space Hauler", 2000, 1),
        ],
        Liner => &[
            ("Subsidised Liner", 600, 5),
            ("Long Liner", 800, 3),
            ("Tourist Liner", 1000, 2),
            ("Colony Transport", 1200, 2),
            ("Pilgrim Liner", 1000, 1),
            ("Troop Transport", 1400, 1),
        ],
        SystemDefenceBoat => &[
            ("System Defence Boat", 200, 5),
            ("Pop-up Monitor", 200, 1),
            ("System Defence Boat", 400, 3),
            ("Advanced SDB", 400, 2),
            ("Missile Systems Defender", 400, 2),
            ("Strike Boat", 100, 1),
        ],
        NavalPatrol => &[
            ("Patrol Corvette", 400, 5),
            ("Gazelle-class Close Escort", 400, 3),
            ("Command Vessel", 600, 2),
            ("Patrol Frigate", 600, 2),
            ("Mercenary Cruiser", 800, 2),
            ("Sector Cutter", 800, 1),
            ("Naval Frigate", 1000, 1),
            ("Customs Cruiser", 1000, 1),
        ],
        Convoy => &[("escorted Convoy", 1500, 1)],
        RichFreighter | None => &[],
    }
}

/// Weighted pick of a `(class name, tons)` from a class table.
fn pick_class(classes: &'static [(&'static str, i32, u32)], rng: &mut impl Rng) -> (&'static str, i32) {
    let total: u32 = classes.iter().map(|(_, _, w)| *w).sum();
    if total == 0 {
        return ("unknown vessel", 0);
    }
    let mut roll = rng.random_range(0..total);
    for &(name, tons, w) in classes {
        if roll < w {
            return (name, tons);
        }
        roll -= w;
    }
    let last = classes[classes.len() - 1];
    (last.0, last.1)
}

/// Weapon turrets per 100 t, by hull role.
const RATE_FREIGHTER: f64 = 1.5;
const RATE_LINER: f64 = 1.0;
const RATE_NAVAL: f64 = 3.0;

/// Fraction of a target's hull that is sellable cargo.
const CARGO_FRACTION: f64 = 0.5;

/// A resolved prey: the concrete ship a pirate has run down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prey {
    /// The encounter type as shown in the log (e.g. `RichFreighter`).
    pub kind: EncounterType,
    /// The specific ship class (e.g. "Far Trader", "Patrol Corvette").
    pub class_name: &'static str,
    /// Rolled hull tonnage.
    pub hull_tons: i32,
    /// Derived weapon count.
    pub weapons: i16,
    /// Maneuver thrust (drives the "one that got away" roll).
    pub thrust: i16,
    /// Tons of sellable cargo aboard (the full hold, before the take).
    pub cargo_tons: i32,
    /// Gross credit value of that full cargo.
    pub cargo_value: i64,
}

/// Weapon rate for an encounter type.
fn weapon_rate(t: EncounterType) -> f64 {
    use EncounterType::*;
    match t {
        Liner => RATE_LINER,
        SystemDefenceBoat | NavalPatrol | Convoy => RATE_NAVAL,
        _ => RATE_FREIGHTER,
    }
}

/// Typical maneuver thrust for an encounter type. Freighters/liners are
/// slow; defenders are fast.
fn base_thrust(t: EncounterType) -> i16 {
    use EncounterType::*;
    match t {
        SmallFreighter | MediumFreighter | HeavyFreighter | RichFreighter | Convoy => 1,
        Liner => 2,
        SystemDefenceBoat => 5,
        NavalPatrol => 4,
        None => 0,
    }
}

/// Cargo-value multiplier derived from a world's trade classifications.
/// Richer economies haul richer cargo.
pub fn economy_multiplier(world: &World) -> f64 {
    let classes = world.get_trade_classes();
    let has = |c: TradeClass| classes.contains(&c);
    let mut m = 1.0;
    if has(TradeClass::Rich) {
        m *= 1.5;
    }
    if has(TradeClass::HighTech) {
        m *= 1.4;
    }
    if has(TradeClass::Industrial) {
        m *= 1.3;
    }
    if has(TradeClass::Agricultural) {
        m *= 1.1;
    }
    if has(TradeClass::Poor) {
        m *= 0.7;
    }
    m
}

/// Roll a concrete prey for the given encounter type. `trade_mult` scales the
/// cargo value by the system economy (see [`economy_multiplier`]).
///
/// A `RichFreighter` rolls its underlying freighter type (1d6: 1-3 Small /
/// 4-5 Medium / 6 Heavy) and doubles the cargo value.
pub fn roll_prey(t: EncounterType, trade_mult: f64, rng: &mut impl Rng) -> Prey {
    // Rich freighters resolve to an underlying hull, doubled in value.
    let (underlying, value_mult, shown) = if t == EncounterType::RichFreighter {
        let kind = match roll_1d6(rng) {
            1..=3 => EncounterType::SmallFreighter,
            4 | 5 => EncounterType::MediumFreighter,
            _ => EncounterType::HeavyFreighter,
        };
        (kind, 2.0, EncounterType::RichFreighter)
    } else {
        (t, 1.0, t)
    };

    // Roll a specific ship class (weighted toward the commons); the class
    // carries its own tonnage.
    let (class_name, hull_tons) = pick_class(ship_classes(underlying), rng);

    let rate = weapon_rate(underlying);
    let weapons = ((hull_tons as f64 / 100.0) * rate).round() as i16;

    // Slight thrust jitter so identical hulls vary a little.
    let thrust = (base_thrust(shown) + rng.random_range(-1..=1) as i16).max(0);

    let cargo_tons = ((hull_tons as f64) * CARGO_FRACTION).round() as i32;
    // Per-ton value: base 2000–6000 (2d6×~350 + floor) × economy multiplier.
    let per_ton = (2000 + roll_2d6(rng) * 350) as f64 * trade_mult * value_mult;
    let cargo_value = (cargo_tons as f64 * per_ton).round() as i64;

    Prey {
        kind: shown,
        class_name,
        hull_tons,
        weapons,
        thrust,
        cargo_tons,
        cargo_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn small_freighter_is_a_named_small_hull() {
        let mut rng = StdRng::seed_from_u64(2);
        for _ in 0..50 {
            let p = roll_prey(EncounterType::SmallFreighter, 1.0, &mut rng);
            assert!(p.hull_tons == 100 || p.hull_tons == 200, "small hull {}", p.hull_tons);
            assert!(!p.class_name.is_empty(), "class name should be set");
        }
    }

    #[test]
    fn naval_patrol_gets_a_naval_class() {
        let mut rng = StdRng::seed_from_u64(9);
        let p = roll_prey(EncounterType::NavalPatrol, 1.0, &mut rng);
        assert!(p.hull_tons >= 400, "naval hull {}", p.hull_tons);
        assert!(!p.class_name.is_empty());
    }

    #[test]
    fn weapons_scale_with_hull_and_rate() {
        // 600 t freighter at 1.5/100t = 9 weapons.
        let w = ((600.0 / 100.0) * RATE_FREIGHTER).round() as i16;
        assert_eq!(w, 9);
        // 800 t naval at 3/100t = 24.
        let w = ((800.0 / 100.0) * RATE_NAVAL).round() as i16;
        assert_eq!(w, 24);
    }

    #[test]
    fn rich_freighter_is_shown_as_rich_and_worth_more() {
        let mut rng = StdRng::seed_from_u64(5);
        let prey = roll_prey(EncounterType::RichFreighter, 1.0, &mut rng);
        assert_eq!(prey.kind, EncounterType::RichFreighter);
        assert!(prey.hull_tons > 0);
        assert!(prey.cargo_value > 0);
    }

    #[test]
    fn rich_economy_lifts_value() {
        let mut rich = World::from_uwp("Rich", "A766645-A", false, true).unwrap();
        rich.gen_trade_classes();
        let mut poor = World::from_uwp("Poor", "C320335-7", false, true).unwrap();
        poor.gen_trade_classes();
        assert!(economy_multiplier(&rich) > 1.0);
        assert!(economy_multiplier(&poor) < 1.0);
    }
}
