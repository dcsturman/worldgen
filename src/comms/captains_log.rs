//! Wire protocol for the captain's-log summary WebSocket endpoint.
//!
//! Lifecycle:
//! 1. Client opens `/ws/captains-log`.
//! 2. Client sends one [`ClientMessage::RunSummary`] with a fully-built
//!    prompt string (assembled by
//!    [`crate::components::captains_log_prompt::build_prompt`]).
//! 3. Server streams zero or more [`ServerMessage::Delta`] frames as
//!    Vertex AI generates text.
//! 4. Server sends exactly one terminal frame — either
//!    [`ServerMessage::Done`] on success or [`ServerMessage::Error`] on
//!    any failure — and closes the connection.
//!
//! This module compiles for both wasm (frontend) and native (backend)
//! since both sides serde the same types.

use serde::{Deserialize, Serialize};

/// Messages the client sends to the captain's-log server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Generate a captain's-log summary for the supplied prompt.
    ///
    /// The prompt is fully-assembled on the frontend; the backend
    /// treats it as opaque text and forwards it to Vertex AI as the
    /// user message of `streamGenerateContent`.
    RunSummary { prompt: String },
}

/// Messages the server sends back over the WebSocket.
///
/// Multiple `Delta`s precede exactly one terminal `Done` or `Error`.
/// After the terminal frame the server closes the connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// One streamed chunk of generated text. Append to whatever's
    /// already rendered.
    Delta { text: String },
    /// Generation finished. Carries usage counters and the Vertex
    /// `finishReason` so the UI can flag truncated / safety-blocked
    /// responses. `STOP` is the clean case; `MAX_TOKENS`, `SAFETY`,
    /// `RECITATION` etc. mean the output was cut off.
    Done {
        /// Tokens in the assembled prompt (including the instruction
        /// header), as reported by Vertex.
        prompt_tokens: u32,
        /// Tokens in the generated response, as reported by Vertex.
        output_tokens: u32,
        /// Last `finishReason` Vertex reported, verbatim. `None` if
        /// none of the streamed events carried one (rare; usually
        /// indicates an upstream protocol change worth investigating).
        finish_reason: Option<String>,
    },
    /// Terminal error. Closes the connection.
    Error {
        /// Stable machine-readable code. One of: `"rate_limit_global"`,
        /// `"prompt_too_large"`, `"vertex_error"`, `"internal_error"`.
        code: String,
        /// Short human-readable description, suitable for inline UI
        /// display. For `vertex_error` this includes the first ~200
        /// characters of Vertex's error body so you can debug; the
        /// full body is logged server-side at `warn!` level.
        message: String,
        /// HTTP status from Vertex when `code == "vertex_error"`.
        /// `None` for purely client-side errors.
        vertex_status: Option<u16>,
        /// Set when `code` is a rate-limit; the client may retry after
        /// this many milliseconds.
        retry_after_ms: Option<u32>,
    },
}

/// Maximum size of the assembled prompt the server will accept, in
/// bytes. Calibrated to a worst-case 5-year voyage with full per-port
/// detail (~64 KB) plus headroom; anything larger is almost certainly
/// malicious or a bug.
pub const MAX_PROMPT_BYTES: usize = 256 * 1024;
