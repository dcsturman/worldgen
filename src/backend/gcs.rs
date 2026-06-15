//! Tiny GCS (Google Cloud Storage) client used by the `/world` endpoint
//! to cache the canonical-scale planet PNGs.
//!
//! Hand-rolled REST (one GET, one POST) over the same `gcp_auth` +
//! `reqwest` stack `vertex_client.rs` uses — no new crate. Two
//! operations:
//!
//! - [`GcsClient::get`]  — `GET …/storage/v1/b/{bucket}/o/{object}?alt=media`,
//!   returns `Ok(None)` on `404` (cache miss), `Ok(Some(bytes))` on `200`.
//! - [`GcsClient::put`]  — `POST …/upload/storage/v1/b/{bucket}/o?uploadType=media&name={object}`,
//!   stores the bytes under `object` with the given `Content-Type`.
//!
//! ## Disabled mode
//!
//! `GCS_BUCKET=debug` (or the env var unset) initializes a disabled
//! client. `get` always returns `Ok(None)`, `put` is a no-op. Mirrors
//! the `FIRESTORE_DATABASE_ID=debug` convention so local dev works
//! without GCP creds — the `/world` endpoint naturally degrades to
//! "always regenerate" (no caching) instead of failing.
//!
//! ## Auth
//!
//! Uses Application Default Credentials via [`gcp_auth::provider`].
//! Tokens are minted per-request (the underlying provider caches them
//! itself, so repeated calls are cheap) and attached as
//! `Authorization: Bearer …`.

use std::sync::Arc;
use std::time::Duration;

use gcp_auth::TokenProvider;
use tokio::sync::OnceCell;

/// GCS scope. Read-write because the `/world` endpoint also writes
/// on cache-miss.
const GCS_SCOPE: &[&str] = &["https://www.googleapis.com/auth/devstorage.read_write"];

/// Sentinel value for [`GcsClient::init`] that means "no caching" —
/// matches the existing `FIRESTORE_DATABASE_ID=debug` pattern.
const DISABLED_BUCKET: &str = "debug";

/// Default request timeout. GCS is fast; if a single GET/PUT is taking
/// more than 30 s, something's wrong and we'd rather error out than
/// pin the request handler waiting.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Cached auth provider — same pattern as `vertex_client.rs`. Cloned
/// `Arc` returned per call; the provider caches tokens internally.
static AUTH_PROVIDER: OnceCell<Arc<dyn TokenProvider>> = OnceCell::const_new();

async fn auth_provider() -> Result<Arc<dyn TokenProvider>, GcsError> {
    AUTH_PROVIDER
        .get_or_try_init(|| async {
            gcp_auth::provider()
                .await
                .map_err(|e| GcsError::Auth(e.to_string()))
        })
        .await
        .cloned()
}

/// All the ways a GCS call can fail. Disabled-mode short-circuits
/// don't produce errors — they just return `Ok(None)` / `Ok(())`.
#[derive(Debug)]
pub enum GcsError {
    Auth(String),
    Network(String),
    /// Non-success HTTP status from GCS. `404` on `get` is **not** an
    /// error — it's surfaced as `Ok(None)`. Anything else (403, 5xx,
    /// etc.) lands here with the status code and (truncated) body so
    /// the caller can log a usable diagnostic.
    Storage { status: u16, body: String },
}

impl std::fmt::Display for GcsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GcsError::Auth(s) => write!(f, "GCS auth failed: {s}"),
            GcsError::Network(s) => write!(f, "GCS network error: {s}"),
            GcsError::Storage { status, body } => {
                write!(f, "GCS storage error {status}: {body}")
            }
        }
    }
}

impl std::error::Error for GcsError {}

/// GCS client.
///
/// `bucket = None` is the disabled mode — `get` returns `Ok(None)` and
/// `put` returns `Ok(())` without touching the network. Construct with
/// [`GcsClient::init`].
#[derive(Clone)]
pub struct GcsClient {
    bucket: Option<String>,
    http: reqwest::Client,
}

impl GcsClient {
    /// Construct from `GCS_BUCKET`. `"debug"` or an unset env var
    /// initializes the disabled client.
    pub async fn init() -> Result<Self, GcsError> {
        let bucket = std::env::var("GCS_BUCKET").ok();
        let bucket = match bucket.as_deref() {
            None | Some("") | Some(DISABLED_BUCKET) => None,
            Some(name) => Some(name.to_string()),
        };
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| GcsError::Network(e.to_string()))?;
        if bucket.is_some() {
            // Eagerly initialize the auth provider so a misconfigured
            // service account fails at boot rather than on the first
            // cache miss. Failure here is non-fatal — we log and fall
            // back to disabled mode so the rest of the backend can
            // still serve traffic.
            if let Err(e) = auth_provider().await {
                log::warn!(
                    "GCS auth provider init failed; falling back to disabled mode: {e}"
                );
                return Ok(Self { bucket: None, http });
            }
        }
        Ok(Self { bucket, http })
    }

    pub fn is_disabled(&self) -> bool {
        self.bucket.is_none()
    }

    /// Read `object` from the bucket. `Ok(None)` on `404` (cache miss),
    /// `Ok(Some(bytes))` on `200`. Other statuses become
    /// [`GcsError::Storage`]. Always returns `Ok(None)` when disabled.
    pub async fn get(&self, object: &str) -> Result<Option<Vec<u8>>, GcsError> {
        let Some(bucket) = self.bucket.as_ref() else {
            return Ok(None);
        };
        let url = format!(
            "https://storage.googleapis.com/storage/v1/b/{}/o/{}?alt=media",
            urlencode(bucket),
            urlencode(object),
        );
        let token = self.bearer_token().await?;
        let resp = self
            .http
            .get(&url)
            .bearer_auth(token.as_str())
            .send()
            .await
            .map_err(|e| GcsError::Network(e.to_string()))?;
        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(GcsError::Storage {
                status: status.as_u16(),
                body: truncate(body, 512),
            });
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| GcsError::Network(e.to_string()))?
            .to_vec();
        Ok(Some(bytes))
    }

    /// Write `body` to `object` with the given `Content-Type`. No-op
    /// when disabled. Overwrites any existing object at that path.
    pub async fn put(
        &self,
        object: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<(), GcsError> {
        let Some(bucket) = self.bucket.as_ref() else {
            return Ok(());
        };
        let url = format!(
            "https://storage.googleapis.com/upload/storage/v1/b/{}/o?uploadType=media&name={}",
            urlencode(bucket),
            urlencode(object),
        );
        let token = self.bearer_token().await?;
        let resp = self
            .http
            .post(&url)
            .bearer_auth(token.as_str())
            .header("Content-Type", content_type)
            .body(body)
            .send()
            .await
            .map_err(|e| GcsError::Network(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(GcsError::Storage {
                status: status.as_u16(),
                body: truncate(body, 512),
            });
        }
        Ok(())
    }

    async fn bearer_token(&self) -> Result<Arc<gcp_auth::Token>, GcsError> {
        let provider = auth_provider().await?;
        provider
            .token(GCS_SCOPE)
            .await
            .map_err(|e| GcsError::Auth(e.to_string()))
    }
}

/// Minimal URL-encoder that percent-encodes everything outside the
/// unreserved set plus `/` (so e.g. `world/v1/<hex>.png` round-trips).
/// GCS object names can contain almost any byte; we don't need a
/// general-purpose encoder, just one that survives our cache keys
/// (alphanumerics + `/` + `.` + `_` + `-`) verbatim and escapes
/// anything else defensively.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn truncate(mut s: String, max: usize) -> String {
    if s.len() > max {
        s.truncate(max);
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disabled_client() -> GcsClient {
        GcsClient {
            bucket: None,
            http: reqwest::Client::new(),
        }
    }

    #[tokio::test]
    async fn disabled_get_returns_none() {
        let c = disabled_client();
        assert!(c.is_disabled());
        let r = c.get("world/v1/abc.png").await.unwrap();
        assert!(r.is_none(), "disabled get should return None, got {:?}", r);
    }

    #[tokio::test]
    async fn disabled_put_is_noop() {
        let c = disabled_client();
        c.put("world/v1/abc.png", vec![1, 2, 3], "image/png")
            .await
            .expect("disabled put should be Ok(())");
    }

    #[test]
    fn urlencode_passes_through_safe_chars() {
        assert_eq!(urlencode("world/v1/abc123.png"), "world/v1/abc123.png");
        assert_eq!(urlencode("Trojan_Reach"), "Trojan_Reach");
    }

    #[test]
    fn urlencode_escapes_unsafe_chars() {
        assert_eq!(urlencode("Trojan Reach"), "Trojan%20Reach");
        assert_eq!(urlencode("a&b"), "a%26b");
    }

    #[test]
    fn truncate_appends_ellipsis_when_over_max() {
        let r = truncate("abcdefghij".to_string(), 5);
        assert_eq!(r, "abcde…");
    }

    #[test]
    fn truncate_returns_as_is_when_under_max() {
        let r = truncate("abc".to_string(), 5);
        assert_eq!(r, "abc");
    }
}
