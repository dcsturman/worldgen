//! Wire protocol for the ship simulator WebSocket endpoint.
//!
//! The simulator uses a WebSocket separate from the trade tool. Lifecycle:
//! 1. Client opens `/ws/simulator`.
//! 2. Client sends one [`ClientMessage::RunSimulation`].
//! 3. Server streams zero or more [`ServerMessage::Step`] frames.
//! 4. Server sends exactly one of [`ServerMessage::Done`] or
//!    [`ServerMessage::Error`] and closes the connection.
//!
//! Both enums are tagged via `#[serde(tag = "type")]` to keep the wire format
//! self-describing and forward-compatible.

use serde::{Deserialize, Serialize};

use crate::simulator::types::{SimulationParams, SimulationResult, SimulationStep};

/// Messages sent from the simulator client to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Begin a simulation with the given parameters.
    RunSimulation(SimulationParams),
}

/// Messages sent from the simulator server to the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// One step in the running simulation. The server may send many of these
    /// before sending `Done` or `Error`.
    Step(SimulationStep),

    /// The simulation finished (whether it returned home, ran out of time,
    /// or aborted on overflow). Carries the final tally.
    Done(SimulationResult),

    /// The simulation could not run or aborted with an error before producing
    /// a result. The `message` is human-readable.
    Error { message: String },
}
