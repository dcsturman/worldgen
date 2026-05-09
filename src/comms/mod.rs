//! # Communications Module
//!
//! This module provides client-side WebSocket communication for syncing
//! trade state between multiple clients through the trade server.

pub mod client;
mod state;

pub use client::Client;
pub use state::TradeState;

use serde::{Deserialize, Serialize};

/// Messages that can be sent between client and server
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ServerMessage {
    /// A trade state update from a client
    StateUpdate(TradeState),
    /// A command to the server
    Command(ServerCommand),
}

/// Commands that clients can send to the server
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "command")]
pub enum ServerCommand {
    /// Request the server to regenerate prices and passengers
    #[serde(rename = "regenerate")]
    Regenerate,
    /// Switch the client to a particular ship's session. The server replies
    /// with that ship's persisted TradeState (loading from Firestore on
    /// first request) and from then on scopes this client's state-updates
    /// and broadcasts to that ship.
    #[serde(rename = "select_ship")]
    SelectShip { ship_name: String },
    /// Subtract one 28-day period of fixed expenses (mortgage +
    /// maintenance + salary) from the ship-manifest profit and persist
    /// the result. The server uses [`crate::trade::Ship::monthly_expenses`]
    /// from the currently-selected ship to compute the deduction, then
    /// broadcasts the updated state to all clients viewing that ship.
    #[serde(rename = "apply_monthly_expenses")]
    ApplyMonthlyExpenses,
}
