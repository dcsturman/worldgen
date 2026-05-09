//! WebSocket handler for `/ws/captains-log`.
//!
//! Lifecycle (mirrors `simulator_server.rs`):
//!
//! 1. Receive exactly one [`ClientMessage::RunSummary`] frame. Malformed
//!    JSON is rejected with `internal_error`.
//! 2. Run two pre-checks fail-fast (global rate limit, prompt size cap)
//!    — each rejects with the appropriate [`ServerMessage::Error`] and
//!    a clean Close. There is no per-connection rate limit: each
//!    connection is one-shot (one `RunSummary`, then close), so a
//!    per-connection gate would never have a prior request to gate
//!    against. The global 1/sec limiter is the real defense.
//! 3. Stream Vertex AI's `streamGenerateContent` SSE response,
//!    forwarding each text delta as a [`ServerMessage::Delta`] frame.
//! 4. On clean stream end, send one [`ServerMessage::Done`] with the
//!    usage counters Vertex reported, then perform the
//!    "send Close + drain_until_close" handshake to avoid the 1006
//!    issue.
//! 5. On any Vertex error, log the full body + URL at `warn!` level,
//!    send a [`ServerMessage::Error`] with `code: "vertex_error"`,
//!    then close cleanly.
//!
//! Authorization headers and the prompt are never logged.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use crate::backend::vertex_client::{self, VertexError};
use crate::comms::captains_log::{ClientMessage, MAX_PROMPT_BYTES, ServerMessage};

/// Minimum gap between accepted requests across the entire process.
const GLOBAL_RATE_GAP: Duration = Duration::from_secs(1);

/// Type alias matching the construction in `bin/server.rs`.
pub type GlobalRateLimiter = Arc<Mutex<Option<Instant>>>;

/// Handle a single captain's-log WebSocket connection from start to finish.
///
/// `project` is the GCP project ID (`GCP_PROJECT` env at server
/// startup). `global_rate_limiter` is shared with every other captain's
/// log connection — it stores the [`Instant`] of the last accepted
/// request across the process.
pub async fn handle_captains_log_ws(
    ws_stream: WebSocketStream<TcpStream>,
    peer_addr: SocketAddr,
    project: Arc<String>,
    global_rate_limiter: GlobalRateLimiter,
) {
    log::info!("captains_log: connection from {}", peer_addr);

    let (ws_sender, mut ws_receiver) = ws_stream.split();

    // Bridge: every outbound frame is funneled through this mpsc so
    // the executor closure (which runs synchronously for each delta)
    // can still ship frames without holding the WS sender mutably.
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let send_task = tokio::spawn(async move {
        let mut ws_sender = ws_sender;
        while let Some(msg) = rx.recv().await {
            let is_close = matches!(msg, Message::Close(_));
            if ws_sender.send(msg).await.is_err() {
                return ws_sender;
            }
            if is_close {
                return ws_sender;
            }
        }
        ws_sender
    });

    // Read the first text frame.
    let first = match ws_receiver.next().await {
        Some(Ok(Message::Text(t))) => t,
        Some(Ok(Message::Close(_))) => {
            log::info!("captains_log: client closed before sending request");
            drop(tx);
            let _ = send_task.await;
            return;
        }
        Some(Ok(_)) => {
            send_internal_error(&tx, "expected text frame, got non-text");
            close_clean(tx, send_task, &mut ws_receiver).await;
            return;
        }
        Some(Err(e)) => {
            log::warn!("captains_log: ws receive error: {}", e);
            drop(tx);
            let _ = send_task.await;
            return;
        }
        None => {
            log::info!("captains_log: client disconnected before sending request");
            drop(tx);
            let _ = send_task.await;
            return;
        }
    };

    // Parse the client message. Malformed JSON is a generic
    // `internal_error` since the schema is fixed and the client
    // controls it.
    let parsed: Result<ClientMessage, _> = serde_json::from_str(&first);
    let prompt = match parsed {
        Ok(ClientMessage::RunSummary { prompt }) => prompt,
        Err(e) => {
            send_error(
                &tx,
                "internal_error",
                &format!("could not parse RunSummary: {}", e),
                None,
                None,
            );
            close_clean(tx, send_task, &mut ws_receiver).await;
            return;
        }
    };

    // ---- Check 1: global rate limit ----
    {
        let mut guard = global_rate_limiter.lock().await;
        if let Some(prev) = *guard {
            let elapsed = prev.elapsed();
            if elapsed < GLOBAL_RATE_GAP {
                let retry_ms = (GLOBAL_RATE_GAP - elapsed).as_millis() as u32;
                drop(guard);
                send_error(
                    &tx,
                    "rate_limit_global",
                    "global rate limit reached; please retry shortly",
                    None,
                    Some(retry_ms),
                );
                close_clean(tx, send_task, &mut ws_receiver).await;
                return;
            }
        }
        // Tentatively claim the slot. We update before the size
        // check would also be reasonable, but the spec says "in
        // order"; we only stamp here once we've decided to accept.
        *guard = Some(Instant::now());
    }

    // ---- Check 2: prompt size ----
    if prompt.len() > MAX_PROMPT_BYTES {
        send_error(
            &tx,
            "prompt_too_large",
            &format!(
                "prompt exceeds max size ({} > {} bytes)",
                prompt.len(),
                MAX_PROMPT_BYTES
            ),
            None,
            None,
        );
        close_clean(tx, send_task, &mut ws_receiver).await;
        return;
    }

    log::info!(
        "captains_log: accepted request from {} ({} bytes)",
        peer_addr,
        prompt.len()
    );

    // ---- Stream from Vertex ----
    // The closure clones `tx` so it can outlive any single call;
    // each delta is queued to the WS via the mpsc bridge.
    let tx_for_deltas = tx.clone();
    let result = vertex_client::stream_generate(project.as_ref(), &prompt, |text: &str| {
        let msg = ServerMessage::Delta {
            text: text.to_string(),
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = tx_for_deltas.send(Message::Text(json.into()));
        }
    })
    .await;

    match result {
        Ok(usage) => {
            // Anything other than `STOP` means Vertex truncated or
            // suppressed output. Surface at warn so it shows up in the
            // backend log without RUST_LOG=debug.
            let reason_str = usage.finish_reason.as_deref().unwrap_or("<none>");
            if usage.finish_reason.as_deref() == Some("STOP") {
                log::info!(
                    "captains_log: completed for {} (prompt_tokens={}, output_tokens={}, finish_reason={})",
                    peer_addr,
                    usage.prompt_tokens,
                    usage.output_tokens,
                    reason_str
                );
            } else {
                log::warn!(
                    "captains_log: non-STOP finish for {} (prompt_tokens={}, output_tokens={}, finish_reason={})",
                    peer_addr,
                    usage.prompt_tokens,
                    usage.output_tokens,
                    reason_str
                );
            }
            let done = ServerMessage::Done {
                prompt_tokens: usage.prompt_tokens,
                output_tokens: usage.output_tokens,
                finish_reason: usage.finish_reason.clone(),
            };
            if let Ok(json) = serde_json::to_string(&done) {
                let _ = tx.send(Message::Text(json.into()));
            }
        }
        Err(e) => {
            // Log full detail server-side. NEVER log the prompt or
            // any Authorization header — only the URL and Vertex's
            // response body.
            let url = format!(
                "https://aiplatform.googleapis.com/v1/projects/{}/locations/global/publishers/google/models/gemini-3-flash-preview:streamGenerateContent?alt=sse",
                project.as_ref()
            );
            let (status, body, short) = match &e {
                VertexError::Status { status, body } => {
                    (Some(*status), body.clone(), truncate(body, 200))
                }
                VertexError::Auth(s) => (None, s.clone(), truncate(s, 200)),
                VertexError::Network(s) => (None, s.clone(), truncate(s, 200)),
                VertexError::Sse(s) => (None, s.clone(), truncate(s, 200)),
            };
            log::warn!(
                "captains_log: vertex error from {}: url={} body={}",
                url,
                url,
                body
            );
            send_error(&tx, "vertex_error", &short, status, None);
        }
    }

    close_clean(tx, send_task, &mut ws_receiver).await;
}

/// Queue a `ServerMessage::Error` frame on the bridge.
fn send_error(
    tx: &mpsc::UnboundedSender<Message>,
    code: &str,
    message: &str,
    vertex_status: Option<u16>,
    retry_after_ms: Option<u32>,
) {
    let err = ServerMessage::Error {
        code: code.to_string(),
        message: message.to_string(),
        vertex_status,
        retry_after_ms,
    };
    match serde_json::to_string(&err) {
        Ok(json) => {
            let _ = tx.send(Message::Text(json.into()));
        }
        Err(e) => {
            log::error!("captains_log: failed to serialize Error: {}", e);
        }
    }
}

/// Convenience: send a generic `internal_error` Error frame.
fn send_internal_error(tx: &mpsc::UnboundedSender<Message>, message: &str) {
    send_error(tx, "internal_error", message, None, None);
}

/// Drive the clean-close handshake: queue a Close frame, then drop
/// the bridge so the send task drains and exits, then read until the
/// client sends its Close back (or we time out).
async fn close_clean(
    tx: mpsc::UnboundedSender<Message>,
    send_task: tokio::task::JoinHandle<
        futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
    >,
    ws_receiver: &mut SplitStream<WebSocketStream<TcpStream>>,
) {
    let _ = tx.send(Message::Close(None));
    drop(tx);
    let _ = send_task.await;
    drain_until_close(ws_receiver, Duration::from_secs(2)).await;
}

async fn drain_until_close(
    ws_receiver: &mut SplitStream<WebSocketStream<TcpStream>>,
    timeout: Duration,
) {
    let _ = tokio::time::timeout(timeout, async {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Close(_)) | Err(_) => break,
                _ => continue,
            }
        }
    })
    .await;
}

/// Truncate a string to `max` bytes at a UTF-8 boundary, suffixing
/// with `…` if any content was dropped.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Find the largest boundary index <= max.
    let mut idx = max;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    let mut out = s[..idx].to_string();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_under_limit_unchanged() {
        assert_eq!(truncate("hello", 200), "hello");
    }

    #[test]
    fn truncate_over_limit_appends_ellipsis() {
        let s = "a".repeat(300);
        let t = truncate(&s, 200);
        // 200 ASCII bytes + the 3-byte ellipsis.
        assert!(t.ends_with('…'));
        assert_eq!(t.chars().filter(|c| *c == 'a').count(), 200);
    }

    #[test]
    fn truncate_respects_utf8_boundary() {
        // Multi-byte chars: 'é' is 2 bytes in UTF-8. With a limit
        // of 1, we must back off to 0 to avoid splitting it.
        let s = "é";
        let t = truncate(s, 1);
        // We dropped everything; result is "…".
        assert_eq!(t, "…");
    }

    #[test]
    fn truncate_does_not_panic_on_short_multibyte() {
        // Sanity: should never panic regardless of input.
        let s = "日本語のテキスト";
        let _ = truncate(s, 5);
        let _ = truncate(s, 100);
    }

    #[test]
    fn server_message_error_serializes() {
        let err = ServerMessage::Error {
            code: "rate_limit_global".to_string(),
            message: "slow down".to_string(),
            vertex_status: None,
            retry_after_ms: Some(500),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"code\":\"rate_limit_global\""));
        assert!(json.contains("\"retry_after_ms\":500"));
    }

    #[test]
    fn server_message_done_serializes() {
        let done = ServerMessage::Done {
            prompt_tokens: 100,
            output_tokens: 200,
            finish_reason: Some("STOP".to_string()),
        };
        let json = serde_json::to_string(&done).unwrap();
        assert!(json.contains("\"prompt_tokens\":100"));
        assert!(json.contains("\"output_tokens\":200"));
        assert!(json.contains("\"finish_reason\":\"STOP\""));
    }

    #[test]
    fn server_message_delta_serializes() {
        let d = ServerMessage::Delta {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"text\":\"hello\""));
    }

    #[test]
    fn rate_limit_constants() {
        assert_eq!(GLOBAL_RATE_GAP, Duration::from_secs(1));
    }

    #[test]
    fn max_prompt_bytes_matches_comms_module() {
        // Sanity check the constant is the one we expect (256 KiB).
        assert_eq!(MAX_PROMPT_BYTES, 256 * 1024);
    }
}
