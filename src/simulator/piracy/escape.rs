//! Escaping defenders, and the prey that gets away.
//!
//! When a pirate meets a defender (System Defence Boat, Naval Patrol, or a
//! disguised q-ship) it first tries to **play it cool** — a social/bluff
//! check on leadership offset by reputation. A notorious pirate gets
//! recognized; an unknown one talks past the patrol. If recognized, the only
//! way out is a **graded thrust escape**: a bigger thrust advantage means a
//! cleaner getaway, while a small deficit still escapes but takes damage, and
//! only a large deficit gets the ship caught.

use rand::Rng;

use super::{COMBAT_DAMAGE_UNIT, roll_2d6};
use crate::simulator::types::EncounterType;

/// Threshold at/above which a patrol recognizes the pirate.
const RECOGNITION_THRESHOLD: i32 = 8;
/// How strongly reputation feeds the recognition roll.
const RECOGNITION_REP_COEF: f64 = 0.5;

/// The result of a thrust escape attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeOutcome {
    /// Got away clean.
    Clean,
    /// Escaped, but took damage / lost time.
    WithDamage { credits: i64, weeks: u32 },
    /// Run down: heavy damage, lost time, possibly marooned.
    Caught {
        credits: i64,
        weeks: u32,
        maroon: bool,
    },
}

/// Play-it-cool / recognition check. Returns `true` if the defender
/// recognizes the pirate (forcing a fight or a flight); `false` means the
/// pirate bluffs past and the encounter is a free pass.
pub fn play_it_cool(leadership: i16, reputation: f64, rng: &mut impl Rng) -> (i32, bool) {
    let score = roll_2d6(rng) + (reputation * RECOGNITION_REP_COEF).round() as i32
        - leadership as i32;
    (score, score >= RECOGNITION_THRESHOLD)
}

/// Graded thrust escape. A messy getaway costs a fixed
/// [`COMBAT_DAMAGE_UNIT`]-scaled repair bill, independent of the ship's
/// maintenance setting.
pub fn thrust_escape(
    ship_thrust: i16,
    encounter_thrust: i16,
    rng: &mut impl Rng,
) -> (i32, EscapeOutcome) {
    // Randomness centered on zero, in -5..=5.
    let luck = roll_2d6(rng) - 7;
    let margin = (ship_thrust - encounter_thrust) as i32 + luck;

    let outcome = if margin >= 2 {
        EscapeOutcome::Clean
    } else if margin >= -3 {
        // Closer thrust → escape, but with damage that grows as the margin
        // shrinks.
        let severity = (3 - margin).max(1) as i64; // 2..=6
        EscapeOutcome::WithDamage {
            credits: severity * COMBAT_DAMAGE_UNIT / 2,
            weeks: (severity as u32 / 2).max(1),
        }
    } else {
        // Badly out-thrust: run down.
        let severity = (-margin) as i64; // >= 4
        EscapeOutcome::Caught {
            credits: severity * COMBAT_DAMAGE_UNIT,
            weeks: (severity as u32 / 2).max(1),
            maroon: margin <= -7,
        }
    };
    (margin, outcome)
}

/// "The one that got away": a fuelled prey may jump out before it can be
/// looted. Higher prey thrust → likelier escape.
pub fn prey_escape(prey_thrust: i16, rng: &mut impl Rng) -> bool {
    roll_2d6(rng) + prey_thrust as i32 >= 11
}

/// Convenience: the MOR base class string for a threat, for logs.
pub fn threat_label(threat: EncounterType, q_ship: bool) -> &'static str {
    if q_ship {
        "q-ship"
    } else {
        threat.label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn unknown_pirate_usually_bluffs_past() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut recognized = 0;
        for _ in 0..200 {
            let (_s, r) = play_it_cool(2, 0.0, &mut rng);
            if r {
                recognized += 1;
            }
        }
        assert!(recognized < 80, "unknown pirate recognized too often: {recognized}/200");
    }

    #[test]
    fn notorious_pirate_usually_recognized() {
        let mut rng = StdRng::seed_from_u64(2);
        let mut recognized = 0;
        for _ in 0..200 {
            let (_s, r) = play_it_cool(1, 25.0, &mut rng);
            if r {
                recognized += 1;
            }
        }
        assert!(recognized > 150, "notorious pirate slipped by too often: {recognized}/200");
    }

    #[test]
    fn big_thrust_advantage_escapes_clean() {
        // A +8 thrust advantage clears the Clean threshold even on the
        // worst luck roll (margin = 8 + (2..12 - 7) >= 3).
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..50 {
            let (_m, o) = thrust_escape(9, 1, &mut rng);
            assert_eq!(o, EscapeOutcome::Clean);
        }
    }

    #[test]
    fn small_deficit_escapes_with_damage_not_caught() {
        let mut rng = StdRng::seed_from_u64(4);
        let mut caught = 0;
        for _ in 0..200 {
            let (_m, o) = thrust_escape(3, 4, &mut rng);
            if matches!(o, EscapeOutcome::Caught { .. }) {
                caught += 1;
            }
        }
        // A one-G deficit should mostly escape (with damage); only a bad-luck
        // tail (~17%) gets caught.
        assert!(caught < 70, "small deficit caught too often: {caught}/200");
    }
}
