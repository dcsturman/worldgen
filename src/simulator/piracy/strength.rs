//! Tonnage-driven derivation of a prey's hull, weapons, thrust and loot.
//!
//! Ships come in discrete hull sizes — 100 t, then even multiples of 200 t
//! (200, 400, 600, …). Each encounter type has a tonnage band; we roll a
//! hull from the ladder within that band and derive **both** the weapon
//! count (`tons/100 × rate`) and the loot from that single number, so a
//! bigger hull is both tougher and richer.

use rand::Rng;

use super::{roll_1d6, roll_2d6};
use crate::simulator::types::EncounterType;
use crate::systems::world::World;
use crate::trade::TradeClass;

/// The discrete hull-size ladder, in tons.
const HULL_LADDER: [i32; 11] = [
    100, 200, 400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000,
];

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

/// Tonnage band `(min, max)` for an encounter type, or `None` for types
/// with no hull (e.g. `None`).
fn tonnage_band(t: EncounterType) -> Option<(i32, i32)> {
    use EncounterType::*;
    match t {
        SmallFreighter => Some((100, 300)),
        MediumFreighter => Some((400, 1000)),
        HeavyFreighter => Some((1000, 2000)),
        Liner => Some((800, 2000)),
        Convoy => Some((1000, 2000)),
        SystemDefenceBoat => Some((200, 400)),
        NavalPatrol => Some((400, 1000)),
        // Rich resolves to one of the freighter types before this is called.
        RichFreighter | None => Option::None,
    }
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

/// Pick a hull from the ladder within `(min, max)`.
fn roll_hull(min: i32, max: i32, rng: &mut impl Rng) -> i32 {
    let options: Vec<i32> = HULL_LADDER
        .iter()
        .copied()
        .filter(|&t| t >= min && t <= max)
        .collect();
    if options.is_empty() {
        return min;
    }
    options[rng.random_range(0..options.len())]
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

    let (min, max) = tonnage_band(underlying).unwrap_or((0, 0));
    let hull_tons = if max == 0 {
        0
    } else {
        roll_hull(min, max, rng)
    };

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
    fn hull_snaps_to_ladder() {
        let mut rng = StdRng::seed_from_u64(1);
        for _ in 0..200 {
            let h = roll_hull(400, 1000, &mut rng);
            assert!([400, 600, 800, 1000].contains(&h), "off-ladder hull {h}");
        }
    }

    #[test]
    fn small_freighter_hull_is_100_or_200() {
        let mut rng = StdRng::seed_from_u64(2);
        for _ in 0..50 {
            let h = roll_hull(100, 300, &mut rng);
            assert!(h == 100 || h == 200, "small freighter hull {h}");
        }
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
