//! Core data types for the ship simulator.
//!
//! These types form the shared API between the route planner, the
//! server-side executor, and the WebSocket protocol. Everything serdes,
//! so the same shapes flow over the wire to the WASM frontend.

use serde::{Deserialize, Serialize};

use crate::trade::ZoneClassification;

/// Imperial date: day in `0..=364` plus year. Day `365` wraps to
/// `(0, year + 1)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Date {
    /// Day of year, `0..=364`.
    pub day: u16,
    /// Imperial year.
    pub year: u16,
}

/// Number of days in a (simulator) year. The Traveller Imperial calendar
/// is exactly 365 days; we don't model leap years.
const DAYS_PER_YEAR: u32 = 365;

impl Date {
    /// Construct a new `Date`.
    ///
    /// # Panics
    /// Panics if `day > 364`.
    pub fn new(day: u16, year: u16) -> Self {
        assert!(day < 365, "day must be in 0..=364, got {day}");
        Date { day, year }
    }

    /// Add `n` days, wrapping the year as needed.
    ///
    /// Multi-year adds work: e.g. `add_days(800)` advances ~2 years and
    /// 70 days.
    pub fn add_days(self, n: u32) -> Date {
        let total = self.day as u32 + n;
        let years = total / DAYS_PER_YEAR;
        let day = (total % DAYS_PER_YEAR) as u16;
        let year = self.year as u32 + years;
        Date {
            day,
            year: year as u16,
        }
    }

    /// Days from `self` to `other`. Negative if `other` is in the past.
    pub fn days_until(self, other: Date) -> i64 {
        let a = self.year as i64 * DAYS_PER_YEAR as i64 + self.day as i64;
        let b = other.year as i64 * DAYS_PER_YEAR as i64 + other.day as i64;
        b - a
    }

    /// Format as `"DDD-YYYY"` with the day zero-padded to three digits.
    pub fn format(self) -> String {
        format!("{:03}-{:04}", self.day, self.year)
    }
}

/// Lightweight world identifier carried in steps and history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldRef {
    /// Display name.
    pub name: String,
    /// 9-character UWP, e.g. `"A788899-A"`.
    pub uwp: String,
    /// Sector name (used together with hex for identity).
    pub sector: String,
    /// Sector-relative hex column.
    pub hex_x: i32,
    /// Sector-relative hex row.
    pub hex_y: i32,
    /// Travel zone classification.
    pub zone: ZoneClassification,
}

/// Inputs to the simulation, supplied by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationParams {
    /// Player-side broker skill, applied on both buy and sell sides. The
    /// counterparty (the planet's broker) is treated as a constant
    /// `economy::PLANETARY_BROKER_SKILL`.
    pub broker_skill: i16,
    /// Steward skill (affects passenger recruitment).
    pub steward_skill: i16,
    /// Captain's leadership skill. Reduces incident likelihood and
    /// shortens crew-loss / trade-scam layovers. Form caps input at 5;
    /// runtime accepts any non-negative value.
    pub leadership_skill: i16,
    /// Number of weapon turrets aboard, `0..=24`. Counterweights piracy
    /// cargo loss.
    pub weapons: i16,

    /// Cargo capacity in tons.
    pub cargo_capacity: i32,
    /// Number of staterooms (passenger + crew accommodations).
    pub staterooms: i32,
    /// Number of low berths.
    pub low_berths: i32,
    /// Ship's jump capability in parsecs.
    pub jump: i32,

    /// Maintenance cost in credits per 28-day period.
    pub maintenance_per_period: i64,
    /// Crew salary cost in credits per 28-day period.
    pub crew_salary_per_period: i64,
    /// Fuel cost in credits per parsec jumped.
    pub fuel_cost_per_parsec: i64,
    /// Fraction of profit shared with the crew, in `0.0..=1.0`.
    pub crew_profit_share: f32,
    /// Starting cash budget in credits.
    pub starting_budget: i64,

    /// World the simulation starts at and tries to return to.
    pub home_world: WorldRef,
    /// Date the simulation begins.
    pub start_date: Date,
    /// Target completion date — the executor pushes the planner to head
    /// home as this approaches and aborts if it slips too far past.
    pub target_completion_date: Date,

    /// Whether illegal goods are allowed in market generation.
    pub illegal_goods: bool,
}

/// Per-step record. Streamed to the UI as the simulation runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationStep {
    /// Date this step occurred on.
    pub date: Date,
    /// Where the action happened.
    pub location: WorldRef,
    /// Budget after this action settled.
    pub budget_after: i64,
    /// What happened.
    pub action: Action,
}

/// Tagged enum, one variant per kind of thing that happens in a step.
///
/// The frontend renders each variant as its own card.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Action {
    /// Ship arrived at a world after a jump.
    Arrive {
        /// World we just left.
        from: WorldRef,
        /// Distance jumped, in parsecs.
        distance: i32,
        /// Fuel cost paid for the jump.
        fuel_cost: i64,
    },
    /// Sold a speculative-cargo good.
    SellGood {
        /// Good name.
        good: String,
        /// Tons sold.
        qty: i32,
        /// Per-ton sell price.
        sell_price: i32,
        /// Per-ton cost previously paid.
        paid: i32,
        /// Total profit on this lot (sell_price - paid) * qty.
        profit: i64,
    },
    /// Couldn't profitably sell a good and held it for the next world.
    HoldGood {
        /// Good name.
        good: String,
        /// Tons held.
        qty: i32,
        /// What it would have sold for here.
        would_sell_at: i32,
        /// What was originally paid.
        paid: i32,
        /// Why we held (e.g. "below cost", "no buyer DM").
        reason: String,
    },
    /// Bought a speculative-cargo good.
    BuyGood {
        /// Good name.
        good: String,
        /// Tons bought.
        qty: i32,
        /// Per-ton purchase price.
        unit_cost: i32,
        /// Total cost (`unit_cost` * `qty`).
        total_cost: i64,
    },
    /// Loaded freight lots in the cargo hold.
    LoadFreight {
        /// Total tons of freight loaded.
        tons: i32,
        /// Number of distinct freight lots.
        lots: u32,
        /// Revenue that will be paid on arrival.
        revenue_pending: i64,
    },
    /// Boarded passengers.
    BoardPax {
        /// High-passage passengers boarded.
        high: i32,
        /// Middle-passage passengers boarded.
        medium: i32,
        /// Basic-passage passengers boarded.
        basic: i32,
        /// Low-passage passengers boarded.
        low: i32,
        /// Revenue that will be paid on arrival.
        revenue_pending: i64,
    },
    /// Paid life-support costs for the upcoming jump.
    PayLifeSupport {
        /// Stateroom rental cost.
        stateroom_cost: i64,
        /// Per-passenger life-support cost.
        ls_cost: i64,
        /// Low-berth life-support cost.
        low_cost: i64,
    },
    /// Jumped to the next world.
    Jump {
        /// Destination world.
        to: WorldRef,
        /// Distance jumped, in parsecs.
        distance: i32,
        /// Fuel cost paid for the jump.
        fuel_cost: i64,
    },
    /// Paid the periodic maintenance + crew salary tick.
    PayPeriodic {
        /// Maintenance paid this tick.
        maintenance: i64,
        /// Crew salary paid this tick.
        salary: i64,
        /// 0-based index of which 28-day period this is.
        period_index: u32,
    },
    /// Soft warning, e.g. budget is low.
    BudgetWarning {
        /// Human-readable note.
        note: String,
    },
    /// Route planner found no acceptable destination.
    NoCandidate {
        /// Human-readable note.
        note: String,
    },
    /// Run aborted because we're too far past the target date.
    AbortOverflow {
        /// How many days past the target we are.
        days_past_target: i64,
    },

    // ---- Incident variants -------------------------------------------------
    // The five incident kinds plus a successful-avoidance variant. All
    // share `avoidance_*` and `table_*` roll fields so the log can show
    // the saving throw inline. The frontend's renderer skips
    // `IncidentAvoided` rather than emit a row for it.
    /// Avoidance roll passed; nothing happened. Carried for analytics.
    IncidentAvoided {
        avoidance_roll: i32,
        leadership: i16,
        port_mod: i32,
        zone_mod: i32,
        law_mod: i32,
        modifier_total: i32,
        avoidance_total: i32,
    },
    /// Pirates struck. Three sub-effects: cargo destroyed/stolen, credits
    /// for repairs, weeks delayed.
    IncidentPiracy {
        avoidance_roll: i32,
        leadership: i16,
        avoidance_modifier_total: i32,
        avoidance_total: i32,
        table_roll: i32,
        table_modifier_total: i32,
        table_total: i32,
        weapons: i16,
        /// Total tons removed from the manifest.
        cargo_lost_tons: i32,
        /// Per-good `(name, tons_lost)` for the log.
        cargo_lost_breakdown: Vec<(String, i32)>,
        /// Sum of `buy_cost * tons_lost` — sunk; not refunded.
        buy_cost_sunk: i64,
        /// Credits for repair after attack
        credits_lost: i64,
        /// Weeks added to the schedule (1d6, no leadership reduction).
        weeks_lost: u32,
    },
    /// A trade scam relieves the captain of credits and time.
    IncidentTradeScam {
        avoidance_roll: i32,
        leadership: i16,
        avoidance_modifier_total: i32,
        avoidance_total: i32,
        table_roll: i32,
        table_modifier_total: i32,
        table_total: i32,
        broker: i16,
        credits_lost: i64,
        weeks_lost: u32,
    },
    /// Crew member lost; the layover for hiring and paperwork delays the
    /// ship. No credit penalty.
    IncidentCrewLoss {
        avoidance_roll: i32,
        leadership: i16,
        avoidance_modifier_total: i32,
        avoidance_total: i32,
        table_roll: i32,
        table_modifier_total: i32,
        table_total: i32,
        weeks_lost: u32,
    },
    /// Mechanical accident; pure credit cost.
    IncidentAccident {
        avoidance_roll: i32,
        leadership: i16,
        avoidance_modifier_total: i32,
        avoidance_total: i32,
        table_roll: i32,
        table_modifier_total: i32,
        table_total: i32,
        repair_cost: i64,
    },
    /// Government complication: fines and weeks lost.
    IncidentGovernment {
        avoidance_roll: i32,
        leadership: i16,
        avoidance_modifier_total: i32,
        avoidance_total: i32,
        table_roll: i32,
        table_modifier_total: i32,
        table_total: i32,
        fine_credits: i64,
        weeks_lost: u32,
    },

    /// Terminal: the end-of-port-stay budget check failed. The run ends
    /// here; a help message will reach the home port after `rescue_eta_days`.
    Marooned {
        budget: i64,
        total_parsecs_jumped: u32,
        rescue_eta_days: u32,
        rescue_arrives_on: Date,
    },
}

/// Final result delivered after the last step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    /// Final cash budget.
    pub final_budget: i64,
    /// Total gross profit (final_budget − starting_budget, before crew share).
    pub gross_profit: i64,
    /// Crew's share of the profit.
    pub crew_share: i64,
    /// Owner's share of the profit (after crew share).
    pub owner_profit: i64,
    /// Date the simulation ended.
    pub end_date: Date,
    /// Number of jumps performed.
    pub jumps: u32,
    /// True if the simulation ran to a clean end (didn't abort).
    pub completed_normally: bool,
    /// True if the ship made it back to the home world.
    pub returned_home: bool,
    /// True if the budget went negative at any point.
    pub went_negative: bool,
    /// True if the run terminated because the ship couldn't pay to leave
    /// a port. When true, the four `marooned_*` fields below are populated.
    pub marooned: bool,
    /// World where the ship was marooned.
    pub marooned_at: Option<WorldRef>,
    /// Date the ship was marooned.
    pub marooned_on: Option<Date>,
    /// Date a rescue is expected to arrive (one week per 4 parsecs of the
    /// path actually travelled, rounded up).
    pub rescue_arrives_on: Option<Date>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_new_panics_on_overflow() {
        let result = std::panic::catch_unwind(|| Date::new(365, 1105));
        assert!(result.is_err(), "Date::new(365, _) should panic");
    }

    #[test]
    fn add_days_within_year() {
        let d = Date::new(100, 1105).add_days(50);
        assert_eq!(d, Date::new(150, 1105));
    }

    #[test]
    fn add_days_at_year_boundary() {
        // 364 + 1 → day 0, year+1
        let d = Date::new(364, 1105).add_days(1);
        assert_eq!(d, Date::new(0, 1106));

        // 360 + 10 → day 5, year+1
        let d = Date::new(360, 1105).add_days(10);
        assert_eq!(d, Date::new(5, 1106));
    }

    #[test]
    fn add_days_multi_year() {
        // 800 days from (0, 1100) → 2 years + 70 days = (70, 1102)
        let d = Date::new(0, 1100).add_days(800);
        assert_eq!(d, Date::new(70, 1102));

        // From a non-zero day, multi-year still works.
        let d = Date::new(100, 1100).add_days(800);
        assert_eq!(d, Date::new(170, 1102));

        // Exactly one year jump.
        let d = Date::new(0, 1100).add_days(365);
        assert_eq!(d, Date::new(0, 1101));
    }

    #[test]
    fn days_until_basic() {
        let a = Date::new(100, 1105);
        let b = Date::new(150, 1105);
        assert_eq!(a.days_until(b), 50);
    }

    #[test]
    fn days_until_negative_when_other_in_past() {
        let a = Date::new(150, 1105);
        let b = Date::new(100, 1105);
        assert_eq!(a.days_until(b), -50);
    }

    #[test]
    fn days_until_across_years() {
        let a = Date::new(360, 1105);
        let b = Date::new(5, 1106);
        // 5 days to year end (360→365 is 5) + 5 = 10
        assert_eq!(a.days_until(b), 10);
    }

    #[test]
    fn format_zero_pads_day() {
        assert_eq!(Date::new(0, 1105).format(), "000-1105");
        assert_eq!(Date::new(7, 1105).format(), "007-1105");
        assert_eq!(Date::new(70, 1105).format(), "070-1105");
        assert_eq!(Date::new(364, 1105).format(), "364-1105");
    }
}
