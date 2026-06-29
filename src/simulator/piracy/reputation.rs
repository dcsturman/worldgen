//! Reputation: gain by act of piracy, decay by lying low.
//!
//! Reputation starts at 0 and climbs with each raid — more severe acts add
//! more. It gates whether patrols recognize the pirate (see
//! [`super::escape::play_it_cool`]) and lends a small intimidation bonus to
//! prey (see [`super::resolution`]). It decays over quiet stretches, so
//! laying low cools you off.

use crate::simulator::types::ActTier;

/// Reputation gained per act tier.
pub fn rep_gain(tier: ActTier) -> f64 {
    match tier {
        ActTier::ExtortLittle => 1.0,
        ActTier::ExtortLot => 2.0,
        ActTier::DamageShip => 4.0,
        ActTier::DestroyOrMurder => 8.0,
    }
}

/// Reputation lost per quiet jump (a jump with no act of piracy).
pub const REP_DECAY_PER_QUIET_JUMP: f64 = 0.5;

/// Reputation ceiling.
pub const REP_MAX: f64 = 100.0;

/// Clamp reputation into `0..=REP_MAX`.
pub fn clamp(rep: f64) -> f64 {
    rep.clamp(0.0, REP_MAX)
}

/// Apply one quiet-jump's worth of decay.
pub fn decay_one_quiet_jump(rep: f64) -> f64 {
    clamp(rep - REP_DECAY_PER_QUIET_JUMP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_is_monotonic_in_tier() {
        assert!(rep_gain(ActTier::ExtortLittle) < rep_gain(ActTier::ExtortLot));
        assert!(rep_gain(ActTier::ExtortLot) < rep_gain(ActTier::DamageShip));
        assert!(rep_gain(ActTier::DamageShip) < rep_gain(ActTier::DestroyOrMurder));
    }

    #[test]
    fn decay_cools_off_but_never_negative() {
        let mut rep = 1.2;
        rep = decay_one_quiet_jump(rep); // 0.7
        rep = decay_one_quiet_jump(rep); // 0.2
        rep = decay_one_quiet_jump(rep); // clamps at 0
        assert_eq!(rep, 0.0);
    }

    #[test]
    fn clamp_caps_at_max() {
        assert_eq!(clamp(250.0), REP_MAX);
        assert_eq!(clamp(-5.0), 0.0);
    }
}
