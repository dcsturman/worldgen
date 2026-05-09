//! Vertex AI streaming client used by the captain's-log endpoint.
//!
//! This is a thin REST + SSE wrapper around Vertex AI's
//! `streamGenerateContent` API for `gemini-3-flash-preview` on the
//! `global` location. We hand-roll the SSE splitter (Vertex's SSE shape
//! is well-defined: each event is a single `data: <JSON>` line with a
//! blank line terminator) rather than pulling in an extra dep.
//!
//! Auth: a single [`gcp_auth`] provider is cached in a [`OnceCell`]
//! across the process and reused for every request. It uses the same
//! `GOOGLE_APPLICATION_CREDENTIALS` service account that Firestore
//! authenticates with. The token itself is not cached here — the
//! provider does that internally.
//!
//! Error path: every failure mode produces a [`VertexError`] variant
//! that carries enough information for the WS handler to log the full
//! body (so the user can debug Vertex-side issues) and to forward a
//! truncated message to the browser via `ServerMessage::Error`.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use gcp_auth::TokenProvider;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::OnceCell;

/// Vertex AI scope required for `streamGenerateContent`.
const VERTEX_SCOPE: &[&str] = &["https://www.googleapis.com/auth/cloud-platform"];

/// Model ID. Pinned here because the URL shape includes the model and
/// because the request body's generation config is calibrated for
/// Gemini 3 Flash specifically.
const MODEL: &str = "gemini-3-flash-preview";

/// HTTP timeout for the streaming POST. Generation can be slow
/// (several seconds) so this is generous; the SSE stream itself can
/// run longer because reqwest's timeout is for the request as a
/// whole, not per-byte. We set it loose enough to let a long
/// captain's log finish.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Cached auth provider. `OnceCell` lets us await the provider once
/// per process; subsequent callers just clone the `Arc<dyn ...>`.
static AUTH_PROVIDER: OnceCell<Arc<dyn TokenProvider>> = OnceCell::const_new();

/// Token usage and stop reason reported by Vertex.
///
/// `finish_reason` is the value of the last candidate's `finishReason`
/// field seen during streaming. Vertex documents these values as
/// `STOP` (clean end), `MAX_TOKENS` (hit the output cap), `SAFETY`
/// (content filter), `RECITATION`, `LANGUAGE`, `BLOCKLIST`,
/// `PROHIBITED_CONTENT`, `SPII`, `MALFORMED_FUNCTION_CALL`, `OTHER`.
/// Anything other than `STOP` is a sign the output was truncated or
/// suppressed.
#[derive(Debug, Clone, Default)]
pub struct UsageMetadata {
    pub prompt_tokens: u32,
    pub output_tokens: u32,
    pub finish_reason: Option<String>,
}

/// All the ways a Vertex call can fail.
#[derive(Debug)]
pub enum VertexError {
    /// `gcp_auth` couldn't mint a token.
    Auth(String),
    /// `reqwest` transport-level error (connection reset, DNS, etc.).
    Network(String),
    /// Vertex returned a non-2xx status. Body is captured for logging.
    Status { status: u16, body: String },
    /// SSE framing or JSON parse error mid-stream.
    Sse(String),
}

impl std::fmt::Display for VertexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VertexError::Auth(s) => write!(f, "auth error: {}", s),
            VertexError::Network(s) => write!(f, "network error: {}", s),
            VertexError::Status { status, body } => {
                write!(f, "vertex status {}: {}", status, body)
            }
            VertexError::Sse(s) => write!(f, "sse parse error: {}", s),
        }
    }
}

impl std::error::Error for VertexError {}

/// Vertex's `streamGenerateContent` returns a stream of these. Most
/// fields are optional because early events often carry only metadata
/// (e.g. `responseId`) with empty `candidates`.
#[derive(Debug, Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
    #[serde(default, rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadataRaw>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    #[serde(default)]
    content: Option<Content>,
    #[serde(default, rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Debug, Deserialize)]
struct Part {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageMetadataRaw {
    #[serde(default, rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(default, rename = "candidatesTokenCount")]
    candidates_token_count: u32,
}

/// Get (and lazily initialize) the cached auth provider.
async fn auth_provider() -> Result<Arc<dyn TokenProvider>, VertexError> {
    AUTH_PROVIDER
        .get_or_try_init(|| async {
            gcp_auth::provider()
                .await
                .map_err(|e| VertexError::Auth(e.to_string()))
        })
        .await
        .cloned()
}

/// Pick a `maxOutputTokens` budget that scales with prompt size.
///
/// Two reasons this needs to be generous:
///
/// 1. **Visible output scales with the voyage.** A 23-port trip needs
///    way more narrative than a 3-port hop, and we don't want a long
///    voyage to truncate at the same hardcoded cap as a short one.
/// 2. **Gemini 3 reasoning tokens count toward `maxOutputTokens` but
///    are not surfaced in `output_tokens`.** Empirically a ~50 KB prompt
///    can chew ~7K hidden reasoning tokens before any visible output
///    starts, so an apparent 8192 budget really only buys ~1000 visible
///    tokens. We budget generously to leave room for both.
///
/// Formula: `prompt_chars / 4 + 4096`, clamped to `[16384, 65536]`.
/// 65536 is the documented `maxOutputTokens` ceiling on Gemini 3 Flash
/// when reasoning is enabled.
fn compute_max_output_tokens(prompt: &str) -> u32 {
    let prompt_chars = prompt.len() as u32;
    (prompt_chars / 4)
        .saturating_add(4096)
        .clamp(16_384, 65_536)
}

/// Build the Vertex `streamGenerateContent` URL for the given project.
fn build_url(project: &str) -> String {
    format!(
        "https://aiplatform.googleapis.com/v1/projects/{}/locations/global/publishers/google/models/{}:streamGenerateContent?alt=sse",
        project, MODEL
    )
}

/// Stream a generation. `on_delta` is called for each text chunk
/// extracted from the SSE stream (skipping events whose `parts` are
/// empty or have no `text` field). Returns the final
/// [`UsageMetadata`] on clean EOF.
pub async fn stream_generate(
    project: &str,
    prompt: &str,
    mut on_delta: impl FnMut(&str),
) -> Result<UsageMetadata, VertexError> {
    let provider = auth_provider().await?;
    let token = provider
        .token(VERTEX_SCOPE)
        .await
        .map_err(|e| VertexError::Auth(e.to_string()))?;

    let url = build_url(project);
    let max_output_tokens = compute_max_output_tokens(prompt);
    log::info!(
        "vertex: request — prompt_chars={}, max_output_tokens={}",
        prompt.len(),
        max_output_tokens
    );
    let body = json!({
        "contents": [{
            "role": "user",
            "parts": [{ "text": prompt }]
        }],
        "generationConfig": {
            "maxOutputTokens": max_output_tokens,
            "temperature": 0.9
        }
    });

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| VertexError::Network(e.to_string()))?;

    let response = client
        .post(&url)
        .bearer_auth(token.as_str())
        .header("Accept", "text/event-stream")
        .json(&body)
        .send()
        .await
        .map_err(|e| VertexError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<failed to read error body: {}>", e));
        return Err(VertexError::Status {
            status: status.as_u16(),
            body,
        });
    }

    // SSE splitter: accumulate bytes from the byte stream into a
    // String buffer, then peel off complete events delimited by a
    // blank line ("\n\n", and tolerate "\r\n\r\n"). For each event,
    // the spec allows multiple lines but Vertex only emits a single
    // `data:` line per event in practice — we still join multi-line
    // `data:` payloads with newlines per the SSE spec.
    let mut byte_stream = response.bytes_stream();
    let mut buf = String::new();
    let mut usage = UsageMetadata::default();
    let mut total_bytes: usize = 0;
    let mut event_count: usize = 0;

    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk.map_err(|e| VertexError::Network(e.to_string()))?;
        total_bytes += chunk.len();
        let s = std::str::from_utf8(&chunk)
            .map_err(|e| VertexError::Sse(format!("invalid utf-8 in stream: {}", e)))?;
        buf.push_str(s);

        while let Some((event_text, rest)) = split_one_event(&buf) {
            // Move `rest` into a new owned String *before* we drop
            // `event_text`'s borrow on `buf`.
            let rest_owned = rest.to_string();
            let data = collect_data_payload(&event_text);
            buf = rest_owned;

            if data.is_empty() {
                continue;
            }
            // Vertex sometimes emits "[DONE]" sentinels on other
            // SSE-based products; for streamGenerateContent it does
            // not, but be defensive.
            if data.trim() == "[DONE]" {
                continue;
            }
            let parsed: GenerateContentResponse = serde_json::from_str(&data).map_err(|e| {
                VertexError::Sse(format!(
                    "failed to parse event JSON: {} (data: {})",
                    e, data
                ))
            })?;
            for cand in &parsed.candidates {
                if let Some(content) = &cand.content {
                    for part in &content.parts {
                        if let Some(text) = &part.text
                            && !text.is_empty()
                        {
                            on_delta(text);
                        }
                    }
                }
                if let Some(reason) = &cand.finish_reason {
                    usage.finish_reason = Some(reason.clone());
                }
            }
            if let Some(u) = parsed.usage_metadata {
                usage.prompt_tokens = u.prompt_token_count;
                usage.output_tokens = u.candidates_token_count;
            }
            event_count += 1;
        }
    }

    // Anything left in `buf` after the stream closed is a partial
    // event — usually just a trailing newline. If it has content but
    // no terminating blank line, surface it once at end-of-stream.
    let tail = buf.trim();
    if !tail.is_empty() {
        let data = collect_data_payload(tail);
        if !data.is_empty()
            && data.trim() != "[DONE]"
            && let Ok(parsed) = serde_json::from_str::<GenerateContentResponse>(&data)
        {
            for cand in &parsed.candidates {
                if let Some(content) = &cand.content {
                    for part in &content.parts {
                        if let Some(text) = &part.text
                            && !text.is_empty()
                        {
                            on_delta(text);
                        }
                    }
                }
                if let Some(reason) = &cand.finish_reason {
                    usage.finish_reason = Some(reason.clone());
                }
            }
            if let Some(u) = parsed.usage_metadata {
                usage.prompt_tokens = u.prompt_token_count;
                usage.output_tokens = u.candidates_token_count;
            }
        }
    }

    log::info!(
        "vertex: stream complete — total_bytes={}, events={}, prompt_tokens={}, output_tokens={}, finish_reason={:?}",
        total_bytes,
        event_count,
        usage.prompt_tokens,
        usage.output_tokens,
        usage.finish_reason
    );

    Ok(usage)
}

/// If `buf` contains at least one complete SSE event (delimited by
/// `\n\n` or `\r\n\r\n`), return `(event_text, rest)`. Otherwise
/// return None.
fn split_one_event(buf: &str) -> Option<(String, &str)> {
    // Search for the earliest of "\n\n" / "\r\n\r\n".
    let lf = buf.find("\n\n");
    let crlf = buf.find("\r\n\r\n");
    let (idx, sep_len) = match (lf, crlf) {
        (Some(a), Some(b)) => {
            if a <= b {
                (a, 2)
            } else {
                (b, 4)
            }
        }
        (Some(a), None) => (a, 2),
        (None, Some(b)) => (b, 4),
        (None, None) => return None,
    };
    let event = buf[..idx].to_string();
    let rest = &buf[idx + sep_len..];
    Some((event, rest))
}

/// Per SSE spec, an event may have multiple `data:` lines that the
/// client joins with `\n`. We also tolerate a leading space after
/// the colon (`data: {...}`) which is how Vertex formats it.
fn collect_data_payload(event_text: &str) -> String {
    let mut out = String::new();
    for line in event_text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("data:") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(rest);
        }
        // Other SSE field lines (event:, id:, retry:, comments) are
        // ignored — Vertex doesn't use them for generateContent.
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_one_event_lf() {
        let buf = "data: hello\n\nrest";
        let (event, rest) = split_one_event(buf).unwrap();
        assert_eq!(event, "data: hello");
        assert_eq!(rest, "rest");
    }

    #[test]
    fn split_one_event_crlf() {
        let buf = "data: hello\r\n\r\nrest";
        let (event, rest) = split_one_event(buf).unwrap();
        assert_eq!(event, "data: hello");
        assert_eq!(rest, "rest");
    }

    #[test]
    fn split_one_event_incomplete() {
        let buf = "data: partial";
        assert!(split_one_event(buf).is_none());
    }

    #[test]
    fn split_one_event_picks_earliest_delim() {
        // LF delim earlier than CRLF.
        let buf = "data: a\n\ndata: b\r\n\r\nrest";
        let (event, rest) = split_one_event(buf).unwrap();
        assert_eq!(event, "data: a");
        assert_eq!(rest, "data: b\r\n\r\nrest");
    }

    #[test]
    fn collect_data_payload_single_line() {
        let event = "data: {\"foo\":1}";
        assert_eq!(collect_data_payload(event), "{\"foo\":1}");
    }

    #[test]
    fn collect_data_payload_no_leading_space() {
        let event = "data:{\"foo\":1}";
        assert_eq!(collect_data_payload(event), "{\"foo\":1}");
    }

    #[test]
    fn collect_data_payload_multi_line_joins_with_newline() {
        let event = "data: line1\ndata: line2";
        assert_eq!(collect_data_payload(event), "line1\nline2");
    }

    #[test]
    fn collect_data_payload_ignores_other_fields() {
        let event = "event: message\nid: 1\ndata: {\"x\":2}\nretry: 5000";
        assert_eq!(collect_data_payload(event), "{\"x\":2}");
    }

    #[test]
    fn collect_data_payload_strips_trailing_cr() {
        let event = "data: hello\r";
        assert_eq!(collect_data_payload(event), "hello");
    }

    #[test]
    fn compute_max_output_tokens_floor() {
        // Tiny prompt → clamped to the floor.
        assert_eq!(compute_max_output_tokens(""), 16_384);
        let small = "x".repeat(1_000);
        assert_eq!(compute_max_output_tokens(&small), 16_384);
    }

    #[test]
    fn compute_max_output_tokens_scales() {
        // 50 KB prompt → ~12.5K + 4096 = ~16,596 (just above floor).
        let mid = "x".repeat(50_000);
        let v = compute_max_output_tokens(&mid);
        assert!(v > 16_384 && v < 65_536, "got {v}");
    }

    #[test]
    fn compute_max_output_tokens_ceiling() {
        // Pathological prompt → clamped to ceiling.
        let huge = "x".repeat(300_000);
        assert_eq!(compute_max_output_tokens(&huge), 65_536);
    }

    #[test]
    fn build_url_shape() {
        let url = build_url("my-project");
        assert!(url.contains("/projects/my-project/"));
        assert!(url.contains("/locations/global/"));
        assert!(url.ends_with(":streamGenerateContent?alt=sse"));
        assert!(url.contains(MODEL));
    }

    #[test]
    fn parse_response_with_text_and_usage() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [{ "text": "hello world" }]
                }
            }],
            "usageMetadata": {
                "promptTokenCount": 42,
                "candidatesTokenCount": 7
            }
        }"#;
        let r: GenerateContentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.candidates.len(), 1);
        let parts = &r.candidates[0].content.as_ref().unwrap().parts;
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].text.as_deref(), Some("hello world"));
        let u = r.usage_metadata.unwrap();
        assert_eq!(u.prompt_token_count, 42);
        assert_eq!(u.candidates_token_count, 7);
    }

    #[test]
    fn parse_response_with_empty_candidates() {
        // Early events from Vertex often look like this.
        let json = r#"{ "candidates": [] }"#;
        let r: GenerateContentResponse = serde_json::from_str(json).unwrap();
        assert!(r.candidates.is_empty());
        assert!(r.usage_metadata.is_none());
    }

    #[test]
    fn parse_response_candidate_without_content() {
        // Final/safety events sometimes carry no content.
        let json = r#"{ "candidates": [{ "finishReason": "STOP" }] }"#;
        let r: GenerateContentResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.candidates.len(), 1);
        assert!(r.candidates[0].content.is_none());
    }
}
