//! The Morale (MOR) surrender mechanic.
//!
//! A target's *will to resist* is a morale pool the pirate's *menace* pushes
//! against. The gap grades the outcome from instant full surrender, through
//! a partial-cargo "safe passage" deal, to a standoff the pirate can either
//! break off or press into a firefight — or, when badly outmatched, walk
//! away from. A small pirate facing a heavy freighter or a convoy comes out
//! with a deeply negative weapon margin and leaves them alone, exactly as
//! the design intends.

use rand::Rng;

use super::strength::Prey;
use super::{roll_1d6, roll_2d6};
use crate::simulator::types::{ActTier, Attitude, EncounterType};

/// Reputation's coefficient and cap on the menace score. A feared name
/// intimidates prey, but only modestly — reputation's main job is getting
/// you *recognized* by patrols, which is bad.
const REP_MENACE_COEF: f64 = 0.2;
const REP_MENACE_CAP: f64 = 5.0;

/// The attacking pirate's relevant stats.
#[derive(Debug, Clone, Copy)]
pub struct Pirate {
    pub weapons: i16,
    pub attitude: Attitude,
    pub reputation: f64,
    pub leadership: i16,
}

/// Outcome of resolving a prey encounter.
#[derive(Debug, Clone)]
pub struct Resolution {
    /// 1d6 morale roll and the total (+ class base).
    pub mor_roll: i32,
    pub mor_total: i32,
    /// Pirate menace and the resulting surrender margin.
    pub menace: i32,
    pub surrender_margin: i32,
    /// Act tier committed, if any take was made.
    pub act_tier: Option<ActTier>,
    /// Tons taken into the hold (before any hold-space cap the caller applies).
    pub loot_tons: i32,
    /// Gross value of that take.
    pub loot_value: i64,
    /// Whether the encounter went loud (the pirate pressed the attack).
    pub went_loud: bool,
    /// Credits of return-fire damage the pirate took.
    pub pirate_damage_credits: i64,
    /// True if the prey broke contact / jumped out before being looted.
    pub target_escaped: bool,
}

/// Base morale by target class: merchant `+3`, armed `+6`, naval `+8`.
pub fn mor_base(kind: EncounterType) -> i32 {
    use EncounterType::*;
    match kind {
        SmallFreighter | MediumFreighter | RichFreighter => 3,
        HeavyFreighter | Liner | Convoy | SystemDefenceBoat => 6,
        NavalPatrol => 8,
        None => 0,
    }
}

/// Resolve a prey encounter. `damage_unit` scales return-fire damage (the
/// executor passes the ship's `maintenance_per_period`, mirroring the
/// merchant repair model).
pub fn resolve_encounter(
    p: &Pirate,
    prey: &Prey,
    damage_unit: i64,
    rng: &mut impl Rng,
) -> Resolution {
    let mor_roll = roll_1d6(rng);
    let mor_total = mor_roll + mor_base(prey.kind);

    let rep_bonus = (p.reputation * REP_MENACE_COEF)
        .round()
        .clamp(0.0, REP_MENACE_CAP) as i32;
    let menace = (p.weapons - prey.weapons) as i32
        + p.attitude.bonus()
        + rep_bonus
        + p.leadership as i32
        + roll_1d6(rng);
    let margin = menace - mor_total;

    let mut went_loud = false;
    let mut pirate_damage = 0i64;
    let mut target_escaped = false;
    let mut base_fraction = 0.0f64;

    if margin >= 4 {
        // Instant full surrender — no shots fired.
        base_fraction = 1.0;
    } else if margin >= 1 {
        // Partial surrender: cargo for safe passage.
        base_fraction = 0.45;
    } else if margin >= -3 && p.attitude >= Attitude::Aggressive {
        // Standoff the pirate is willing to press into a firefight.
        went_loud = true;
        base_fraction = 1.0;
        let d = roll_2d6(rng) as i64; // 2..=12
        pirate_damage = (prey.weapons.max(0) as i64) * damage_unit * d / 60;
    } else {
        // Break off, or the prey jumps out — nothing taken.
        target_escaped = true;
    }

    // Attitude throttles how much of the available cargo is actually grabbed.
    let attitude_take = match p.attitude {
        Attitude::Chill => 0.35,
        Attitude::Hungry => 0.75,
        Attitude::Aggressive | Attitude::Bloodthirsty => 1.0,
    };
    let take_fraction = (base_fraction * attitude_take).min(1.0);

    let loot_tons = ((prey.cargo_tons as f64) * take_fraction).round() as i32;
    let loot_value = ((prey.cargo_value as f64) * take_fraction).round() as i64;

    let act_tier = if loot_tons <= 0 && !went_loud {
        None
    } else {
        let computed = if went_loud {
            if p.attitude == Attitude::Bloodthirsty {
                ActTier::DestroyOrMurder
            } else {
                ActTier::DamageShip
            }
        } else if loot_value >= 1_000_000 {
            ActTier::ExtortLot
        } else {
            ActTier::ExtortLittle
        };
        Some(computed.min(p.attitude.max_tier()))
    };

    Resolution {
        mor_roll,
        mor_total,
        menace,
        surrender_margin: margin,
        act_tier,
        loot_tons,
        loot_value,
        went_loud,
        pirate_damage_credits: pirate_damage,
        target_escaped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn prey(weapons: i16, cargo_tons: i32, cargo_value: i64) -> Prey {
        Prey {
            kind: EncounterType::SmallFreighter,
            hull_tons: 200,
            weapons,
            thrust: 1,
            cargo_tons,
            cargo_value,
        }
    }

    #[test]
    fn outgunned_small_freighter_surrenders() {
        // Strong pirate, weak prey: should take a haul most of the time.
        let p = Pirate {
            weapons: 8,
            attitude: Attitude::Hungry,
            reputation: 0.0,
            leadership: 2,
        };
        let mut takes = 0;
        let mut rng = StdRng::seed_from_u64(11);
        for _ in 0..200 {
            let r = resolve_encounter(&p, &prey(2, 100, 500_000), 50_000, &mut rng);
            if r.act_tier.is_some() {
                takes += 1;
            }
        }
        assert!(takes > 150, "expected mostly takes, got {takes}/200");
    }

    #[test]
    fn badly_outgunned_pirate_breaks_off() {
        // Weak pirate vs a heavily armed target: should walk away.
        let p = Pirate {
            weapons: 2,
            attitude: Attitude::Hungry,
            reputation: 0.0,
            leadership: 1,
        };
        let mut escapes = 0;
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..200 {
            let r = resolve_encounter(&p, &prey(20, 200, 2_000_000), 50_000, &mut rng);
            if r.target_escaped {
                escapes += 1;
            }
        }
        assert!(escapes > 150, "expected mostly break-offs, got {escapes}/200");
    }

    #[test]
    fn chill_never_exceeds_extort_little() {
        let p = Pirate {
            weapons: 12,
            attitude: Attitude::Chill,
            reputation: 0.0,
            leadership: 3,
        };
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..200 {
            let r = resolve_encounter(&p, &prey(1, 400, 5_000_000), 50_000, &mut rng);
            if let Some(t) = r.act_tier {
                assert!(t <= ActTier::ExtortLittle, "chill committed {t:?}");
            }
        }
    }
}
