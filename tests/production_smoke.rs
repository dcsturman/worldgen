//! Live smoke tests that hit the **deployed** `/system` endpoint at
//! `tools.callistoflight.com` (or whatever URL `WORLDGEN_BASE_URL` is
//! set to) and verify the public contract still holds end-to-end.
//!
//! External consumers — notably the Traveller Map web client — call
//! `/system` from a different origin and depend on the response shape,
//! CORS headers, and byte-level determinism. These tests are the
//! regression net: if a future change to worldgen, nginx, or the Cloud
//! Run deploy quietly breaks the contract, running this test will
//! surface it immediately.
//!
//! All tests are `#[ignore]` by default so `cargo test` doesn't hit
//! production on every run. Invoke them manually before pushing a
//! change that could affect the public surface:
//!
//! ```text
//! cargo test --features backend --test production_smoke -- --ignored --nocapture
//! ```
//!
//! Add `WORLDGEN_BASE_URL=https://...` to point at a staging deploy or
//! a local backend instead of production.

#![cfg(feature = "backend")]

use std::time::Duration;

/// Production URL the Traveller Map client (and any other external
/// consumer) targets. Override with `WORLDGEN_BASE_URL` to test a
/// staging deploy or a local instance.
const DEFAULT_BASE_URL: &str = "https://tools.callistoflight.com";

fn base_url() -> String {
    std::env::var("WORLDGEN_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
}

fn client() -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("worldgen-production-smoke/1.0")
        .build()
        .expect("reqwest client builds")
}

/// Canonical Noricum query — the spec example. If this URL stops
/// returning a valid PNG, external consumers (Traveller Map) break.
const NORICUM_QUERY: &str =
    "sector=Trojan+Reach&hex=2018&name=Noricum&uwp=D8867BB-1&pbg=804&stellar=G2+V+M9+V+M6+V&worlds=14";

#[tokio::test]
#[ignore]
async fn live_system_endpoint_returns_valid_3200x1800_png() {
    let url = format!("{}/system?{NORICUM_QUERY}", base_url());
    let resp = client().get(&url).send().await.expect("HTTP GET succeeds");

    assert_eq!(
        resp.status().as_u16(),
        200,
        "expected 200 OK from {url}; got {}",
        resp.status()
    );
    let ct = resp
        .headers()
        .get("content-type")
        .expect("content-type header present")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        ct.starts_with("image/png"),
        "content-type should be image/png, got {ct}"
    );

    let bytes = resp.bytes().await.expect("response body downloads").to_vec();
    assert!(bytes.len() > 10_000, "PNG suspiciously small: {} bytes", bytes.len());
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "missing PNG magic header");

    // PNG IHDR width/height — big-endian u32s at byte offsets 16..24.
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    assert_eq!(
        (w, h),
        (3200, 1800),
        "default scale=2.0 should yield 3200x1800; got {w}x{h}"
    );
}

#[tokio::test]
#[ignore]
async fn live_system_endpoint_emits_cors_headers() {
    // External consumers (browser-based Traveller Map client) must be
    // able to read the response from a different origin. Loss of any
    // CORS header breaks them silently.
    let url = format!("{}/system?{NORICUM_QUERY}", base_url());
    let resp = client().get(&url).send().await.expect("HTTP GET succeeds");
    let headers = resp.headers();
    assert_eq!(
        headers
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*"),
        "Access-Control-Allow-Origin must be *"
    );
    let methods = headers
        .get("access-control-allow-methods")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        methods.contains("GET") && methods.contains("OPTIONS"),
        "Access-Control-Allow-Methods should include GET + OPTIONS; got {methods:?}"
    );
}

#[tokio::test]
#[ignore]
async fn live_system_endpoint_options_preflight_returns_204() {
    // Browsers fire OPTIONS before the real GET when the origin
    // differs. If this stops returning 204 (or stops emitting CORS
    // headers), every cross-origin GET breaks.
    let url = format!("{}/system", base_url());
    let resp = client()
        .request(reqwest::Method::OPTIONS, &url)
        .header("Origin", "https://travellermap.com")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .expect("OPTIONS request succeeds");

    assert_eq!(
        resp.status().as_u16(),
        204,
        "OPTIONS preflight should return 204; got {}",
        resp.status()
    );
    assert_eq!(
        resp.headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*"),
        "OPTIONS preflight must include Access-Control-Allow-Origin: *"
    );
}

#[tokio::test]
#[ignore]
async fn live_system_endpoint_rejects_bad_uwp_with_422() {
    let url = format!(
        "{}/system?sector=x&hex=0000&name=x&uwp=X???????-?",
        base_url()
    );
    let resp = client().get(&url).send().await.expect("HTTP GET succeeds");
    assert_eq!(
        resp.status().as_u16(),
        422,
        "bad UWP should return 422; got {}",
        resp.status()
    );
    // Body should be plain text naming the failure mode so callers can
    // surface it. We just check it's non-empty here.
    let body = resp.text().await.unwrap_or_default();
    assert!(!body.is_empty(), "422 body should explain the failure");
}

#[tokio::test]
#[ignore]
async fn live_system_endpoint_rejects_missing_required_with_400() {
    // Drop the `hex` param — required, so the handler should return
    // 400 naming which one is missing.
    let url = format!("{}/system?sector=x&name=x&uwp=A788899-A", base_url());
    let resp = client().get(&url).send().await.expect("HTTP GET succeeds");
    assert_eq!(
        resp.status().as_u16(),
        400,
        "missing required param should return 400; got {}",
        resp.status()
    );
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.contains("hex"),
        "400 body should name the missing param 'hex': {body:?}"
    );
}

#[tokio::test]
#[ignore]
async fn live_system_endpoint_is_byte_deterministic() {
    // The headline contract — same query, same bytes, forever. If this
    // ever fails, *something* in the determinism chain broke between
    // worldgen versions: seed derivation, ChaCha8Rng plumbing, sysmap
    // renderer, or even an nginx-level transform (gzip, image
    // optimization, etc.). All would silently break external caching.
    let url = format!("{}/system?{NORICUM_QUERY}", base_url());
    let a = client()
        .get(&url)
        .send()
        .await
        .expect("first call succeeds")
        .bytes()
        .await
        .unwrap()
        .to_vec();
    let b = client()
        .get(&url)
        .send()
        .await
        .expect("second call succeeds")
        .bytes()
        .await
        .unwrap()
        .to_vec();
    assert_eq!(
        a, b,
        "same query produced different PNG bytes (len {} vs {}) — determinism contract broken",
        a.len(),
        b.len()
    );
}

#[tokio::test]
#[ignore]
async fn live_system_endpoint_scale_1_returns_1600x900() {
    // scale=1.0 is the byte-identity contract with the legacy
    // `generate_system_png` — also confirms the scale parameter is
    // actually being read from the query string.
    let url = format!(
        "{}/system?sector=x&hex=0000&name=x&uwp=A788899-A&scale=1.0",
        base_url()
    );
    let resp = client().get(&url).send().await.expect("HTTP GET succeeds");
    assert_eq!(resp.status().as_u16(), 200);
    let bytes = resp.bytes().await.unwrap().to_vec();
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    assert_eq!(
        (w, h),
        (1600, 900),
        "scale=1.0 should yield 1600x900; got {w}x{h}"
    );
}
