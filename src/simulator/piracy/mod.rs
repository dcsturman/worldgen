//! Pure piracy mechanics for the pirate simulator.
//!
//! Everything in this module is deliberately **pure** — no async, no
//! network, no global RNG. Every randomized function takes a
//! `&mut impl rand::Rng`, so the executor can thread a single seedable
//! [`rand::rngs::StdRng`] through the whole pirate path for reproducible
//! runs, and the unit tests stay hermetic (matching the `pirate_cargo`
//! pattern in [`crate::simulator::incidents`]).
//!
//! The submodules map one-to-one to the design's resolution stages:
//!
//! - [`encounter`] — the Prey Encounter D66 table and its per-die modifiers.
//! - [`strength`] — tonnage-driven hull / weapons / loot derivation.
//! - [`resolution`] — the Morale (MOR) surrender mechanic.
//! - [`escape`] — play-it-cool and the graded thrust escape.
//! - [`fencing`] — the fence-rate ladder (with the natural-2/3 seizure floor).
//! - [`reputation`] — reputation gain and quiet-time decay.
//! - [`route`] — HUNT / FENCE-RUN candidate scoring and the mode trigger.

use rand::Rng;

/// Fixed credit cost unit for battle and escape damage, independent of the
/// ship's upkeep. A fight's repair bill scales on this, the enemy's weapons,
/// and the fight's intensity — *not* on `maintenance_per_period`, so combat
/// always bites the same whatever a captain sets their maintenance to.
pub const COMBAT_DAMAGE_UNIT: i64 = 25_000;

pub mod encounter;
pub mod escape;
pub mod fencing;
pub mod reputation;
pub mod resolution;
pub mod route;
pub mod strength;

/// Roll 2d6, returning the two individual dice. Callers that need the
/// *natural* roll (e.g. fencing's snake-eyes seizure floor) inspect the
/// pair; most just sum it.
pub(crate) fn roll_2d6_dice(rng: &mut impl Rng) -> (i32, i32) {
    (rng.random_range(1..=6), rng.random_range(1..=6))
}

/// Roll 2d6 and sum.
pub(crate) fn roll_2d6(rng: &mut impl Rng) -> i32 {
    let (a, b) = roll_2d6_dice(rng);
    a + b
}

/// Roll 1d6.
pub(crate) fn roll_1d6(rng: &mut impl Rng) -> i32 {
    rng.random_range(1..=6)
}

/// Roll a uniform `0.0..1.0` — used for probability checks (e.g. prize capture).
pub(crate) fn roll_unit(rng: &mut impl Rng) -> f64 {
    rng.random::<f64>()
}
