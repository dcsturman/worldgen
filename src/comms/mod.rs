//! # Communications Module
//!
//! This module provides client-side WebSocket communication for syncing
//! trade state between multiple clients through the trade server.

mod state;
pub mod client;

pub use client::Client;
pub use state::TradeState;

