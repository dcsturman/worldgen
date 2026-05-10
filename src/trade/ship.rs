//! # Ship configuration
//!
//! The `Ship` struct is the unified configuration record for a vessel used by
//! both the Trade Computer (`comms::TradeState`) and the Ship Simulator
//! (`simulator`). It captures the *static* shape of a ship — its capacity,
//! crew, hardware, and periodic costs — and is distinct from
//! [`crate::trade::ship_manifest::ShipManifest`], which records the cargo
//! and passengers actually loaded right now.
//!
//! `Ship` is shared across both build targets (WASM frontend and native
//! backend), which is why it lives under `src/trade/` rather than under
//! `src/backend/` or `src/simulator/`.

use serde::{Deserialize, Serialize};

/// Cost in credits to rent a stateroom for one jump.
///
/// One stateroom houses either one high-passage passenger, one
/// medium-passage passenger, or two basic-passage passengers. A single
/// basic-passage passenger still occupies (and pays for) a full
/// stateroom on its own.
pub const STATEROOM_COST: i64 = 1_000;

/// Per-jump life-support cost per crew member.
pub const CREW_LIFE_SUPPORT_PER_MEMBER: i64 = 1_000;

/// Unified ship configuration.
///
/// All fields describe the ship itself — capacity, crew, hardware, and
/// fixed periodic costs — not what it's currently carrying. The trade
/// computer (`comms::TradeState`) and the ship simulator
/// (`simulator::SimulationParams`) both build their behaviour from this
/// single record.
///
/// `#[serde(default)]` is applied at the struct level so newly-introduced
/// fields deserialize to their defaults when older clients send a partial
/// `TradeState` over the wire. This is important: `TradeState` carries a
/// `Ship`, and tolerating missing fields keeps the wire format
/// forward-compatible.
///
/// Note on terminology: `broker_skill` is the **Ship Broker skill** — the
/// skill of the broker aboard *this* ship (the player's broker). The
/// counterparty broker on the planet is treated separately (see
/// `simulator::types::SimulationParams::planetary_broker_skill`, which the
/// simulator UI exposes as "System broker skill"). Never refer to this
/// field as the "Player Broker skill".
///
/// `leadership_skill` and `weapons` are only consumed by the simulator;
/// the trade computer doesn't surface them in its UI. They still belong
/// on `Ship` so that a single record fully describes the vessel for both
/// tools.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Ship {
    /// Display name of the ship. Also used as the Firestore session key
    /// when this `Ship` is persisted server-side, so it must be unique
    /// per session.
    pub name: String,

    // -- Capacity --------------------------------------------------------
    /// Cargo hold capacity in tons.
    pub cargo_capacity: i32,
    /// Number of staterooms allocated to passengers (high / medium /
    /// basic occupancy follows the standard Traveller rules).
    pub passenger_staterooms: i32,
    /// Number of low berths aboard.
    pub low_berths: i32,
    /// Number of staterooms allocated to crew. Each crew stateroom
    /// incurs `STATEROOM_COST` per jump on top of per-crewmember life
    /// support.
    pub crew_staterooms: i32,

    // -- Crew ------------------------------------------------------------
    /// Number of crew aboard. Each crew member adds
    /// `CREW_LIFE_SUPPORT_PER_MEMBER` per jump to life-support cost.
    pub crew_size: i32,
    /// Ship Broker skill — the skill of the broker aboard *this* ship
    /// (the player's broker). The counterparty broker on the planet is
    /// modelled separately by the simulator.
    pub broker_skill: i16,
    /// Steward skill. Affects passenger recruitment.
    pub steward_skill: i16,
    /// Captain's leadership skill. Used by the simulator's incident
    /// system to reduce avoidance failures and shorten layovers; not
    /// surfaced in the trade-computer UI.
    pub leadership_skill: i16,

    // -- Hardware --------------------------------------------------------
    /// Ship's J-rating: the maximum jump distance it can make in one
    /// jump, in parsecs (typically `1..=6`). This is **not** a per-leg
    /// distance — the actual parsecs jumped on a given leg depends on
    /// origin/destination and is computed elsewhere.
    pub jump_rating: i16,
    /// Number of weapon turrets. Used by the simulator's piracy
    /// resolution; not surfaced in the trade-computer UI.
    pub weapons: i16,

    // -- Periodic costs (per 28-day period) ------------------------------
    /// Mortgage payment per 28-day period. Paid alongside maintenance
    /// and salary, but excluded from the simulator's end-of-voyage crew
    /// profit-share calculation.
    pub mortgage_per_period: i64,
    /// Maintenance cost per 28-day period.
    pub maintenance_per_period: i64,
    /// Total crew salary per 28-day period (sum across all crew).
    pub salary_per_period: i64,
}

impl Ship {
    /// Total credits paid every 28-day period: mortgage + maintenance +
    /// salary. Used by the trade computer's "Apply monthly expenses"
    /// button and by the simulator's periodic tick.
    pub fn monthly_expenses(&self) -> i64 {
        self.mortgage_per_period + self.maintenance_per_period + self.salary_per_period
    }

    /// Per-jump life-support cost for the crew:
    /// `crew_staterooms * STATEROOM_COST + crew_size * CREW_LIFE_SUPPORT_PER_MEMBER`.
    ///
    /// Replaces the standalone `simulator::economy::crew_cost(crew_staterooms, crew_size)`.
    /// Negative values for either field are clamped to zero.
    pub fn crew_life_support_per_jump(&self) -> i64 {
        let rooms = self.crew_staterooms.max(0) as i64;
        let crew = self.crew_size.max(0) as i64;
        rooms * STATEROOM_COST + crew * CREW_LIFE_SUPPORT_PER_MEMBER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monthly_expenses_sums_three_components() {
        let ship = Ship {
            mortgage_per_period: 200_000,
            maintenance_per_period: 50_000,
            salary_per_period: 30_000,
            ..Default::default()
        };
        assert_eq!(ship.monthly_expenses(), 280_000);
    }

    #[test]
    fn crew_life_support_per_jump_zero_when_empty() {
        let ship = Ship::default();
        assert_eq!(ship.crew_life_support_per_jump(), 0);
    }

    #[test]
    fn crew_life_support_per_jump_basic() {
        // 3 crew staterooms (3 * 1000 = 3000) + 4 crew (4 * 1000 = 4000) = 7000.
        let ship = Ship {
            crew_staterooms: 3,
            crew_size: 4,
            ..Default::default()
        };
        assert_eq!(ship.crew_life_support_per_jump(), 7_000);
    }

    #[test]
    fn crew_life_support_per_jump_clamps_negative() {
        let ship = Ship {
            crew_staterooms: -2,
            crew_size: -1,
            ..Default::default()
        };
        assert_eq!(ship.crew_life_support_per_jump(), 0);
    }

    #[test]
    fn ship_default_is_zeroed() {
        let ship = Ship::default();
        assert_eq!(ship.name, "");
        assert_eq!(ship.cargo_capacity, 0);
        assert_eq!(ship.jump_rating, 0);
        assert_eq!(ship.monthly_expenses(), 0);
    }

    #[test]
    fn ship_round_trips_through_serde() {
        let ship = Ship {
            name: "Beowulf".to_string(),
            cargo_capacity: 82,
            passenger_staterooms: 6,
            low_berths: 4,
            crew_staterooms: 4,
            crew_size: 4,
            broker_skill: 2,
            steward_skill: 1,
            leadership_skill: 1,
            jump_rating: 1,
            weapons: 1,
            mortgage_per_period: 187_654,
            maintenance_per_period: 5_433,
            salary_per_period: 12_000,
        };
        let json = serde_json::to_string(&ship).unwrap();
        let back: Ship = serde_json::from_str(&json).unwrap();
        assert_eq!(ship, back);
    }

    #[test]
    fn ship_deserializes_with_missing_fields_as_default() {
        // `serde(default)` at the struct level means an empty object
        // round-trips to Ship::default().
        let ship: Ship = serde_json::from_str("{}").unwrap();
        assert_eq!(ship, Ship::default());

        // A partial object fills in defaults for everything else.
        let ship: Ship = serde_json::from_str(r#"{"name":"Patrol"}"#).unwrap();
        assert_eq!(ship.name, "Patrol");
        assert_eq!(ship.cargo_capacity, 0);
        assert_eq!(ship.jump_rating, 0);
    }
}
