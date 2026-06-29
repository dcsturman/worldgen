//! Reputation (notoriety): grows with acts of piracy, cools over quiet months.
//!
//! Modelled on Traveller's "Standing" table, inverted to a positive
//! notoriety. Each incident's deeds stack, and the bigger categories are a
//! fresh 1d6 each — so one ugly job can spike a pirate's name a long way:
//!
//! - **+1** stealing cargo worth more than Cr 100,000.
//! - **+1D** an infamous incident: a large haul (>1 MCr), capturing a ship,
//!   or crippling one.
//! - **+1D** an atrocity: destroying a ship or murdering its crew.
//! - **+1D** interference: hitting a registered convoy.
//! - **−1 per quiet month** (28-day period with no act of piracy): notoriety
//!   trends back toward 0.

use rand::Rng;

use super::roll_1d6;

/// Reputation ceiling.
pub const REP_MAX: f64 = 100.0;

/// Notoriety lost per quiet 28-day period (trends back toward 0).
pub const REP_DECAY_PER_PERIOD: f64 = 1.0;

/// Clamp reputation into `0..=REP_MAX`.
pub fn clamp(rep: f64) -> f64 {
    rep.clamp(0.0, REP_MAX)
}

/// What a single incident did, for scoring notoriety. Categories are
/// independent and stack.
#[derive(Debug, Clone, Copy, Default)]
pub struct IncidentDeeds {
    /// Cargo value taken this incident (Cr).
    pub cargo_value: i64,
    /// Crippled a ship (boarded after a fight, not destroyed).
    pub crippled: bool,
    /// Destroyed a ship / murdered its crew (atrocity).
    pub destroyed: bool,
    /// Seized a ship whole as a prize.
    pub captured_prize: bool,
    /// The target was a registered convoy (interference with trade).
    pub convoy: bool,
}

/// Notoriety gained from one incident, per the Standing-style table.
pub fn rep_gain_for_incident(deeds: &IncidentDeeds, rng: &mut impl Rng) -> f64 {
    let mut gain = 0.0;
    // Stealing cargo.
    if deeds.cargo_value > 100_000 {
        gain += 1.0;
    }
    // Infamous incident.
    if deeds.cargo_value > 1_000_000 || deeds.captured_prize || deeds.crippled {
        gain += roll_1d6(rng) as f64;
    }
    // Atrocity.
    if deeds.destroyed {
        gain += roll_1d6(rng) as f64;
    }
    // Interference with trade.
    if deeds.convoy {
        gain += roll_1d6(rng) as f64;
    }
    gain
}

/// Move reputation one step toward 0 for a quiet month.
pub fn decay_one_period(rep: f64) -> f64 {
    clamp(rep - REP_DECAY_PER_PERIOD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn small_cargo_theft_is_plus_one() {
        let mut rng = StdRng::seed_from_u64(1);
        let deeds = IncidentDeeds {
            cargo_value: 200_000,
            ..Default::default()
        };
        assert_eq!(rep_gain_for_incident(&deeds, &mut rng), 1.0);
    }

    #[test]
    fn tiny_theft_gains_nothing() {
        let mut rng = StdRng::seed_from_u64(1);
        let deeds = IncidentDeeds {
            cargo_value: 50_000,
            ..Default::default()
        };
        assert_eq!(rep_gain_for_incident(&deeds, &mut rng), 0.0);
    }

    #[test]
    fn big_ugly_job_stacks_categories() {
        // Big haul + capture + destroy + convoy: +1 (cargo) + 3D, so at least
        // 1 + 3 and at most 1 + 18.
        let mut rng = StdRng::seed_from_u64(7);
        let deeds = IncidentDeeds {
            cargo_value: 5_000_000,
            crippled: false,
            destroyed: true,
            captured_prize: true,
            convoy: true,
        };
        let g = rep_gain_for_incident(&deeds, &mut rng);
        assert!((4.0..=19.0).contains(&g), "got {g}");
    }

    #[test]
    fn decay_trends_toward_zero_and_clamps() {
        let mut rep = 1.5;
        rep = decay_one_period(rep); // 0.5
        rep = decay_one_period(rep); // clamps at 0
        assert_eq!(rep, 0.0);
    }

    #[test]
    fn clamp_caps_at_max() {
        assert_eq!(clamp(250.0), REP_MAX);
        assert_eq!(clamp(-5.0), 0.0);
    }
}
