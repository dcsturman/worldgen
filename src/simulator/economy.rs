//! Per-jump cost constants and helpers for the ship simulator.
//!
//! These are the fixed-rate ship-running costs the executor charges as
//! the simulation proceeds: stateroom rentals, passenger life support,
//! low-berth life support, and the simulation's day-counter constants
//! used by the periodic-maintenance/salary tick.

/// Cost in credits to rent a stateroom for one jump.
///
/// One stateroom houses either one high-passage passenger, one
/// medium-passage passenger, or two basic-passage passengers. A single
/// basic-passage passenger still occupies (and pays for) a full
/// stateroom on its own.
pub const STATEROOM_COST: i64 = 1_000;

/// Per-jump life-support cost for a high-passage passenger.
pub const HIGH_LIFE_SUPPORT: i64 = 2_000;
/// Per-jump life-support cost for a medium-passage passenger.
pub const MEDIUM_LIFE_SUPPORT: i64 = 1_000;
/// Per-jump life-support cost for a basic-passage passenger.
pub const BASIC_LIFE_SUPPORT: i64 = 500;
/// Per-jump life-support cost for a low-berth passenger.
pub const LOW_BERTH_COST: i64 = 100;

/// Days of in-jump time per jump (Traveller standard).
pub const DAYS_PER_JUMP: u32 = 7;
/// Days spent in port per turn (loading/selling/refueling).
pub const DAYS_IN_PORT: u32 = 7;
/// Total days per turn — one jump plus the port stay before it.
pub const TURN_DAYS: u32 = DAYS_PER_JUMP + DAYS_IN_PORT;
/// Days between periodic ship-cost (maintenance + salary) ticks.
pub const PERIOD_DAYS: u32 = 28;
/// How many days past the target completion date the executor tolerates
/// before it gives up trying to head home and just aborts the run.
pub const ABORT_OVERFLOW_DAYS: i64 = 100;

/// Compute how many staterooms are needed to house `high` high-passage,
/// `medium` medium-passage, and `basic` basic-passage passengers.
///
/// High and medium passengers use one stateroom each. Basic passengers
/// share staterooms two-to-a-room (a single odd basic still uses a full
/// stateroom).
pub fn staterooms_used(high: i32, medium: i32, basic: i32) -> i32 {
    // ceil(basic / 2) without using the not-yet-stable signed
    // `i32::div_ceil`.
    let basic_rooms = if basic > 0 { (basic + 1) / 2 } else { 0 };
    high + medium + basic_rooms
}

/// Return `(stateroom_cost, life_support_cost, low_berth_cost)` for a
/// turn carrying `high`/`medium`/`basic`/`low` passengers.
pub fn passenger_costs(high: i32, medium: i32, basic: i32, low: i32) -> (i64, i64, i64) {
    let staterooms = staterooms_used(high, medium, basic) as i64;
    let stateroom_cost = staterooms * STATEROOM_COST;
    let ls_cost = high as i64 * HIGH_LIFE_SUPPORT
        + medium as i64 * MEDIUM_LIFE_SUPPORT
        + basic as i64 * BASIC_LIFE_SUPPORT;
    let low_cost = low as i64 * LOW_BERTH_COST;
    (stateroom_cost, ls_cost, low_cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staterooms_used_no_passengers() {
        assert_eq!(staterooms_used(0, 0, 0), 0);
    }

    #[test]
    fn staterooms_used_only_high() {
        assert_eq!(staterooms_used(3, 0, 0), 3);
    }

    #[test]
    fn staterooms_used_only_medium() {
        assert_eq!(staterooms_used(0, 4, 0), 4);
    }

    #[test]
    fn staterooms_used_basic_share_pairs() {
        // 1 basic still uses a full stateroom.
        assert_eq!(staterooms_used(0, 0, 1), 1);
        // 2 basics share 1 stateroom.
        assert_eq!(staterooms_used(0, 0, 2), 1);
        // 3 basics need 2 staterooms (one shared, one solo).
        assert_eq!(staterooms_used(0, 0, 3), 2);
        // 4 basics need 2 staterooms.
        assert_eq!(staterooms_used(0, 0, 4), 2);
    }

    #[test]
    fn staterooms_used_mixed() {
        // 2 high + 1 medium + 5 basic → 2 + 1 + ceil(5/2)=3 → 6.
        assert_eq!(staterooms_used(2, 1, 5), 6);
    }

    #[test]
    fn passenger_costs_zero() {
        let (sr, ls, low) = passenger_costs(0, 0, 0, 0);
        assert_eq!(sr, 0);
        assert_eq!(ls, 0);
        assert_eq!(low, 0);
    }

    #[test]
    fn passenger_costs_basic_pair() {
        // 2 basics share 1 stateroom (1 * 1000 = 1000), each costs 500
        // basic LS = 1000.
        let (sr, ls, low) = passenger_costs(0, 0, 2, 0);
        assert_eq!(sr, 1_000);
        assert_eq!(ls, 1_000);
        assert_eq!(low, 0);
    }

    #[test]
    fn passenger_costs_mixed() {
        // 2 high + 1 medium + 3 basic + 4 low.
        // staterooms: 2 + 1 + ceil(3/2)=2 → 5 * 1000 = 5000.
        // ls: 2*2000 + 1*1000 + 3*500 = 4000 + 1000 + 1500 = 6500.
        // low: 4 * 100 = 400.
        let (sr, ls, low) = passenger_costs(2, 1, 3, 4);
        assert_eq!(sr, 5_000);
        assert_eq!(ls, 6_500);
        assert_eq!(low, 400);
    }
}
