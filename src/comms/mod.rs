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
}
