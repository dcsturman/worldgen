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

use super::escape::prey_escape;
use super::strength::Prey;
use super::{COMBAT_DAMAGE_UNIT, roll_1d6};
use crate::simulator::types::{ActTier, Attitude, EncounterOutcome, EncounterType};

/// Reputation's coefficient and cap on the menace score. A feared name
/// intimidates prey, but only modestly — reputation's main job is getting
/// you *recognized* by patrols, which is bad.
const REP_MENACE_COEF: f64 = 0.2;
const REP_MENACE_CAP: f64 = 5.0;

/// Resistance (morale shortfall) at/below which a doctrine will wade into a
/// firefight. Chill never fights; Bloodthirsty fights anything. Hungry's
/// stomach reaches just past the destroy threshold, so a Hungry pirate *can*
/// (rarely) end up destroying a target in its toughest fights.
fn stomach(attitude: Attitude) -> i32 {
    match attitude {
        Attitude::Chill => -1, // never engages a resisting target
        Attitude::Hungry => 4,
        Attitude::Aggressive => 8,
        Attitude::Bloodthirsty => i32::MAX,
    }
}

/// Damage (in [`COMBAT_DAMAGE_UNIT`] units) a doctrine will absorb before
/// breaking off a fight, mauled. Bloodthirsty never bails — it wins or dies.
fn breaking_point(attitude: Attitude) -> i64 {
    match attitude {
        Attitude::Chill => 0,
        Attitude::Hungry => 5,
        Attitude::Aggressive => 12,
        Attitude::Bloodthirsty => i64::MAX,
    }
}

/// Fraction of the available cargo a doctrine actually grabs on a take.
fn attitude_take(attitude: Attitude) -> f64 {
    match attitude {
        Attitude::Chill => 0.35,
        Attitude::Hungry => 0.75,
        Attitude::Aggressive | Attitude::Bloodthirsty => 1.0,
    }
}

/// Resistance at/above which a won battle ends with the target *destroyed*
/// rather than merely crippled. Battle intensity drives this, not the
/// pirate's temperament — except Bloodthirsty reaches the killing point
/// sooner. Set below Hungry's stomach (4) so even Hungry can destroy in its
/// hardest fights.
fn destroy_threshold(attitude: Attitude) -> i32 {
    match attitude {
        Attitude::Bloodthirsty => 2,
        _ => 4,
    }
}

/// Divisor tuning battle damage: `target_weapons × (resistance + 1d6) ×
/// COMBAT_DAMAGE_UNIT / DAMAGE_K`.
const DAMAGE_K: i64 = 12;

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
    /// Morale shortfall driving battle intensity (`max(0, -margin)`).
    pub resistance: i32,
    /// How the encounter ended.
    pub outcome: EncounterOutcome,
    /// Act tier committed, if any take was made.
    pub act_tier: Option<ActTier>,
    /// Tons taken into the hold (before any hold-space cap the caller applies).
    pub loot_tons: i32,
    /// Gross value of that take.
    pub loot_value: i64,
    /// Credits of battle damage the pirate must repair (zero unless a fight
    /// actually happened — overwhelmed targets surrender without a shot).
    pub pirate_damage_credits: i64,
    /// Weeks lost to a protracted battle.
    pub weeks_lost: u32,
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

/// Resolve a prey encounter. Battle damage is a fixed cost
/// ([`COMBAT_DAMAGE_UNIT`]) scaled by the enemy's weapons and the fight's
/// intensity — independent of the ship's maintenance setting.
pub fn resolve_encounter(p: &Pirate, prey: &Prey, rng: &mut impl Rng) -> Resolution {
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
    let resistance = (-margin).max(0);

    let outcome;
    let mut act_tier: Option<ActTier> = None;
    let mut pirate_damage = 0i64;
    let mut weeks_lost = 0u32;
    let mut take_fraction = 0.0f64;

    if margin >= 4 {
        // Completely overwhelmed — instant full surrender, not a shot fired.
        outcome = EncounterOutcome::Surrendered;
        take_fraction = attitude_take(p.attitude);
    } else if margin >= 1 {
        // Partial surrender — cargo for safe passage; still no fight.
        outcome = EncounterOutcome::Surrendered;
        take_fraction = 0.45 * attitude_take(p.attitude);
    } else if prey_escape(prey.thrust, rng) {
        // The target had the legs and made jump before we closed.
        outcome = EncounterOutcome::PreyJumpedClear;
    } else if resistance > stomach(p.attitude) {
        // Chill never fights; others decline a brawl too tough for them.
        outcome = EncounterOutcome::BrokeOffOutgunned;
    } else {
        // A real battle. Even a win costs repair damage; only an overwhelmed
        // target (handled above) yields without bloodying us.
        let d = roll_1d6(rng) as i64;
        pirate_damage =
            (prey.weapons.max(0) as i64) * (resistance as i64 + d) * COMBAT_DAMAGE_UNIT / DAMAGE_K;
        weeks_lost = (resistance / 3).clamp(0, 4) as u32;

        if p.attitude != Attitude::Bloodthirsty
            && pirate_damage > breaking_point(p.attitude).saturating_mul(COMBAT_DAMAGE_UNIT)
        {
            // Beaten back before we could board: damage, no take.
            outcome = EncounterOutcome::DrivenOffMauled;
        } else {
            // Won the firefight. Battle intensity — not the pirate's initial
            // temperament — decides whether the ship is crippled or destroyed.
            outcome = EncounterOutcome::FoughtAndWon;
            take_fraction = attitude_take(p.attitude);
            act_tier = Some(if resistance >= destroy_threshold(p.attitude) {
                ActTier::DestroyOrMurder
            } else {
                ActTier::DamageShip
            });
        }
    }

    let loot_tons = ((prey.cargo_tons as f64) * take_fraction).round() as i32;
    let loot_value = ((prey.cargo_value as f64) * take_fraction).round() as i64;

    // A peaceful take is extortion; its tier is set by the haul size.
    if outcome == EncounterOutcome::Surrendered {
        act_tier = Some(if loot_value >= 1_000_000 {
            ActTier::ExtortLot
        } else {
            ActTier::ExtortLittle
        });
    }

    Resolution {
        mor_roll,
        mor_total,
        menace,
        surrender_margin: margin,
        resistance,
        outcome,
        act_tier,
        loot_tons,
        loot_value,
        pirate_damage_credits: pirate_damage,
        weeks_lost,
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
            let r = resolve_encounter(&p, &prey(2, 100, 500_000), &mut rng);
            if r.act_tier.is_some() {
                takes += 1;
            }
        }
        assert!(takes > 150, "expected mostly takes, got {takes}/200");
    }

    #[test]
    fn badly_outgunned_pirate_takes_nothing() {
        // Weak pirate vs a heavily armed target: breaks off (or the prey
        // jumps clear) — either way, no take and no battle.
        let p = Pirate {
            weapons: 2,
            attitude: Attitude::Hungry,
            reputation: 0.0,
            leadership: 1,
        };
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..200 {
            let r = resolve_encounter(&p, &prey(20, 200, 2_000_000), &mut rng);
            assert!(r.act_tier.is_none(), "weak pirate should never take this prey");
            assert!(matches!(
                r.outcome,
                EncounterOutcome::BrokeOffOutgunned | EncounterOutcome::PreyJumpedClear
            ));
        }
    }

    #[test]
    fn surrender_is_free_but_every_battle_costs_repair() {
        let p = Pirate {
            weapons: 7,
            attitude: Attitude::Aggressive,
            reputation: 4.0,
            leadership: 2,
        };
        let mut rng = StdRng::seed_from_u64(21);
        for _ in 0..400 {
            let r = resolve_encounter(&p, &prey(6, 200, 800_000), &mut rng);
            match r.outcome {
                EncounterOutcome::Surrendered => {
                    assert_eq!(r.pirate_damage_credits, 0, "surrender drew blood")
                }
                EncounterOutcome::FoughtAndWon | EncounterOutcome::DrivenOffMauled => {
                    assert!(r.pirate_damage_credits > 0, "a battle dealt no damage")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn chill_never_fights() {
        // Chill takes surrenders (even big ones), but never joins a battle
        // and never commits violence.
        let p = Pirate {
            weapons: 12,
            attitude: Attitude::Chill,
            reputation: 0.0,
            leadership: 3,
        };
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..300 {
            let r = resolve_encounter(&p, &prey(8, 400, 5_000_000), &mut rng);
            assert!(
                !matches!(
                    r.outcome,
                    EncounterOutcome::FoughtAndWon | EncounterOutcome::DrivenOffMauled
                ),
                "chill should never battle, got {:?}",
                r.outcome
            );
            if let Some(t) = r.act_tier {
                assert!(
                    matches!(t, ActTier::ExtortLittle | ActTier::ExtortLot),
                    "chill committed violence: {t:?}"
                );
            }
        }
    }

    #[test]
    fn aggressive_can_destroy_in_a_hard_fight() {
        // A mid-armed Aggressive pirate against a stout medium freighter sees
        // some fights protracted enough to end with the target destroyed.
        let p = Pirate {
            weapons: 5,
            attitude: Attitude::Aggressive,
            reputation: 0.0,
            leadership: 2,
        };
        let mut destroyed = 0;
        let mut fought = 0;
        let mut rng = StdRng::seed_from_u64(31);
        for _ in 0..600 {
            let r = resolve_encounter(&p, &prey(9, 300, 1_500_000), &mut rng);
            if r.outcome == EncounterOutcome::FoughtAndWon {
                fought += 1;
                if r.act_tier == Some(ActTier::DestroyOrMurder) {
                    destroyed += 1;
                }
            }
        }
        assert!(fought > 0, "expected some battles");
        assert!(destroyed > 0, "expected some destroyed targets in hard fights");
    }
}
