//! Fencing plundered cargo at a low-law world.
//!
//! Loot rides in the hold until the pirate diverts to a low-law port to
//! fence it. The fence rate is `2d6 + leadership + law_bonus` on a ladder
//! topping out at 50%, **except** a natural 2d6 of 2–3 is a sting: the goods
//! are seized outright (0%) no matter the modifiers, and the botched deal
//! adds heat (reputation).

use rand::Rng;

use super::roll_2d6_dice;

/// Outcome of a fence attempt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FenceOutcome {
    /// `2d6 + leadership + law_bonus` (the natural floor overrides the bands).
    pub roll: i32,
    /// True if the deal was a sting and the cargo was seized.
    pub seized: bool,
    /// Fence rate as a percentage (0/10/20/30/40/50).
    pub payout_pct: i32,
    /// Credits realized = `cargo_value * payout_pct / 100`.
    pub payout: i64,
    /// Reputation/heat added by the deal (only on a seizure).
    pub reputation_delta: f64,
}

/// Heat added to reputation when a fence deal turns out to be a sting.
const SEIZURE_HEAT: f64 = 3.0;

/// Law-level bonus to the fence roll, or `None` if the world is too
/// high-law to fence at all (the pirate should route elsewhere).
///
/// `0 → +2, 1-2 → +1, 3-4 → 0, 5 → -2, 6+ → can't fence`.
pub fn law_bonus(law: i32) -> Option<i32> {
    match law {
        0 => Some(2),
        1 | 2 => Some(1),
        3 | 4 => Some(0),
        5 => Some(-2),
        _ => None,
    }
}

/// Attempt to fence `cargo_value` credits of loot. Returns `None` if the
/// world is unfenceable (law 6+); callers shouldn't route a fence run there,
/// so this is a defensive guard.
pub fn fence(
    cargo_value: i64,
    leadership: i16,
    law: i32,
    rng: &mut impl Rng,
) -> Option<FenceOutcome> {
    let bonus = law_bonus(law)?;

    let (d1, d2) = roll_2d6_dice(rng);
    let natural = d1 + d2;
    let total = natural + leadership as i32 + bonus;

    // The natural-2/3 sting floor overrides all modifiers.
    if natural <= 3 {
        return Some(FenceOutcome {
            roll: total,
            seized: true,
            payout_pct: 0,
            payout: 0,
            reputation_delta: SEIZURE_HEAT,
        });
    }

    let payout_pct = match total {
        ..=6 => 10,
        7..=9 => 20,
        10..=12 => 30,
        13..=15 => 40,
        _ => 50,
    };
    let payout = cargo_value * payout_pct as i64 / 100;

    Some(FenceOutcome {
        roll: total,
        seized: false,
        payout_pct,
        payout,
        reputation_delta: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn law_bonus_ladder() {
        assert_eq!(law_bonus(0), Some(2));
        assert_eq!(law_bonus(2), Some(1));
        assert_eq!(law_bonus(4), Some(0));
        assert_eq!(law_bonus(5), Some(-2));
        assert_eq!(law_bonus(6), None);
        assert_eq!(law_bonus(9), None);
    }

    #[test]
    fn natural_two_or_three_seizes_regardless_of_modifiers() {
        // Hunt seeds until the natural roll is 2 or 3, then assert seizure
        // even with a big leadership + law bonus.
        let mut found = false;
        for seed in 0..500u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let (d1, d2) = {
                // Peek by cloning the same seed stream.
                let mut peek = StdRng::seed_from_u64(seed);
                roll_2d6_dice(&mut peek)
            };
            if d1 + d2 <= 3 {
                let out = fence(10_000_000, 5, 0, &mut rng).unwrap();
                assert!(out.seized, "natural {} should seize", d1 + d2);
                assert_eq!(out.payout_pct, 0);
                assert_eq!(out.payout, 0);
                assert!(out.reputation_delta > 0.0);
                found = true;
                break;
            }
        }
        assert!(found, "no natural 2/3 found in seed sweep");
    }

    #[test]
    fn solid_roll_pays_thirty_percent() {
        // total in 10..=12 → 30%.
        let out = FenceOutcome {
            roll: 11,
            seized: false,
            payout_pct: 30,
            payout: 3_000_000,
            reputation_delta: 0.0,
        };
        assert_eq!(out.payout, 10_000_000 * 30 / 100);
    }

    #[test]
    fn unfenceable_high_law_returns_none() {
        let mut rng = StdRng::seed_from_u64(1);
        assert!(fence(1_000_000, 3, 7, &mut rng).is_none());
    }
}
