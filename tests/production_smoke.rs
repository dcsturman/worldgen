//! Live smoke tests that hit the **deployed** `/api/system` and
//! `/api/world` endpoints at `tools.callistoflight.com` (or whatever
//! URL `WORLDGEN_BASE_URL` is set to) and verify the public contract
//! still holds end-to-end. Also includes a set of SPA-route survival
//! tests that catch nginx routing regressions.
//!
//! External consumers — notably the Traveller Map web client — call
//! `/api/system` and `/api/world` from a different origin and depend on
//! the response shape, CORS headers, and byte-level determinism. These
//! tests are the regression net: if a future change to worldgen, nginx,
//! or the Cloud Run deploy quietly breaks the contract, running this
//! test will surface it immediately.
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

/// Canonical Noricum query for `/api/system` — the spec example. If
/// this URL stops returning a valid PNG, external consumers (Traveller
/// Map) break.
const NORICUM_QUERY: &str =
    "sector=Trojan+Reach&hex=2018&name=Noricum&uwp=D8867BB-1&pbg=804&stellar=G2+V+M9+V+M6+V&worlds=14";

/// `/api/world` query — only needs the world identity (no PBG /
/// stellar / system worlds) because the planet renderer takes the UWP
/// directly.
const NORICUM_WORLD_QUERY: &str =
    "sector=Trojan+Reach&hex=2018&name=Noricum&uwp=D8867BB-1";

#[tokio::test]
#[ignore]
async fn live_system_endpoint_returns_valid_3200x1800_png() {
    let url = format!("{}/api/system?{NORICUM_QUERY}", base_url());
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
    let url = format!("{}/api/system?{NORICUM_QUERY}", base_url());
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
    let url = format!("{}/api/system", base_url());
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
        "{}/api/system?sector=x&hex=0000&name=x&uwp=X???????-?",
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
    let url = format!("{}/api/system?sector=x&name=x&uwp=A788899-A", base_url());
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
    let url = format!("{}/api/system?{NORICUM_QUERY}", base_url());
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
        "{}/api/system?sector=x&hex=0000&name=x&uwp=A788899-A&scale=1.0",
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

// ---------------------------------------------------------------------------
// /world endpoint smoke tests. These hit the same deployed backend as
// `/system` but exercise the planet renderer + GCS cache. The first
// call against a never-seen world is slow (~25 s cold); subsequent
// calls are sub-second cache hits.
// ---------------------------------------------------------------------------

/// Long client timeout — a cold-cache /world render can take ~25 s on
/// production. We use a separate client because the default `client()`
/// is tuned for fast endpoints.
fn slow_client() -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .user_agent("worldgen-production-smoke/1.0")
        .build()
        .expect("reqwest client builds")
}

#[tokio::test]
#[ignore]
async fn live_world_endpoint_returns_valid_png_with_cors_headers() {
    let url = format!("{}/api/world?{NORICUM_WORLD_QUERY}", base_url());
    let resp = slow_client().get(&url).send().await.expect("HTTP GET succeeds");

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
    assert!(ct.starts_with("image/png"), "content-type should be image/png, got {ct}");
    let headers = resp.headers().clone();
    assert_eq!(
        headers
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*"),
    );
    // X-Cache should be present; we don't assert HIT vs MISS because
    // the live state depends on whether this test ran before for this
    // world recently.
    assert!(
        headers.get("x-cache").is_some(),
        "expected X-Cache header on /world response"
    );

    let bytes = resp.bytes().await.expect("response body downloads").to_vec();
    assert!(bytes.len() > 10_000, "PNG suspiciously small: {} bytes", bytes.len());
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
}

#[tokio::test]
#[ignore]
async fn live_world_endpoint_caches_second_call_quickly() {
    // No hard threshold — we log timings via --nocapture so a human can
    // eyeball the cold vs warm ratio. If the second call is anywhere
    // near the first, the cache isn't working.
    use std::time::Instant;
    let url = format!("{}/api/world?{NORICUM_WORLD_QUERY}", base_url());

    let t0 = Instant::now();
    let r1 = slow_client().get(&url).send().await.unwrap();
    assert_eq!(r1.status().as_u16(), 200);
    let bytes1 = r1.bytes().await.unwrap().len();
    let cold = t0.elapsed();

    let t1 = Instant::now();
    let r2 = slow_client().get(&url).send().await.unwrap();
    assert_eq!(r2.status().as_u16(), 200);
    let bytes2 = r2.bytes().await.unwrap().len();
    let warm = t1.elapsed();

    eprintln!(
        "/world timings — call 1: {cold:?} ({bytes1} bytes); call 2: {warm:?} ({bytes2} bytes)"
    );
    // Cold call could be a cache HIT if this test ran recently; what we
    // really want to confirm is that the second call doesn't somehow
    // take longer than the first.
    assert!(
        warm <= cold + Duration::from_secs(2),
        "second call took longer than the first (cache regression?): cold={cold:?} warm={warm:?}"
    );
}

#[tokio::test]
#[ignore]
async fn live_world_endpoint_is_byte_deterministic() {
    // Two calls at default scale should return byte-identical PNGs.
    // This pins worldgen determinism + GCS round-trip + (if applicable)
    // the downsample step — bytes must be the same whether served from
    // cache or freshly generated.
    let url = format!("{}/api/world?{NORICUM_WORLD_QUERY}", base_url());
    let a = slow_client()
        .get(&url)
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap()
        .to_vec();
    let b = slow_client()
        .get(&url)
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap()
        .to_vec();
    assert_eq!(
        a, b,
        "/world bytes drifted between calls — determinism broken (len {} vs {})",
        a.len(),
        b.len()
    );
}

#[tokio::test]
#[ignore]
async fn live_world_endpoint_canonical_scale_is_byte_deterministic() {
    // Same as above but at scale=2.0 — bypasses the downsample branch
    // and pins that the cached canonical bytes are served verbatim.
    let url = format!("{}/api/world?{NORICUM_WORLD_QUERY}&scale=2.0", base_url());
    let a = slow_client()
        .get(&url)
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap()
        .to_vec();
    let b = slow_client()
        .get(&url)
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap()
        .to_vec();
    assert_eq!(a, b, "/world canonical bytes drifted between calls");
    // And verify the canonical dimensions while we're here.
    let w = u32::from_be_bytes([a[16], a[17], a[18], a[19]]);
    let h = u32::from_be_bytes([a[20], a[21], a[22], a[23]]);
    assert_eq!((w, h), (2000, 1310), "canonical /world should be 2000x1310");
}

#[tokio::test]
#[ignore]
async fn live_world_endpoint_rejects_bad_uwp_with_422() {
    let url = format!(
        "{}/api/world?sector=x&hex=0000&name=x&uwp=X???????-?",
        base_url()
    );
    let resp = slow_client().get(&url).send().await.expect("HTTP GET succeeds");
    assert_eq!(
        resp.status().as_u16(),
        422,
        "bad UWP should return 422; got {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// SPA route survival tests.
//
// These would have caught the prefix-match regression that shipped in the
// initial /system and /world deploys: nginx `location /world` was a
// prefix match that captured the SPA's /worldmap (404 "Unknown endpoint")
// AND the SPA's /world page (400 "missing required param: sector"). Each
// test below asserts a SPA route returns 200 + text/html — i.e. the
// nginx try_files fallback served index.html — and not a backend
// plain-text error.
//
// If you ever add another /api/<foo> route and accidentally drop the
// /api/ prefix, the same tests will fail loudly on the next push.
// ---------------------------------------------------------------------------

async fn assert_spa_route(path: &str) {
    let url = format!("{}{path}", base_url());
    let resp = client().get(&url).send().await.expect("HTTP GET succeeds");
    let status = resp.status();
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert_eq!(
        status.as_u16(),
        200,
        "SPA route {path} should return 200 (SPA fallback to index.html), got {status}"
    );
    assert!(
        ct.starts_with("text/html"),
        "SPA route {path} should return text/html, got {ct:?}. \
         This usually means an API location block in nginx is prefix-matching \
         the path and proxying to the backend."
    );
}

#[tokio::test]
#[ignore]
async fn live_spa_root_serves_html() {
    assert_spa_route("/").await;
}

#[tokio::test]
#[ignore]
async fn live_spa_world_serves_html() {
    // The system-generator page. Lives at /world in the SPA. This is the
    // route my initial /world API endpoint catastrophically collided
    // with — the API handler answered first and returned 400.
    assert_spa_route("/world").await;
}

#[tokio::test]
#[ignore]
async fn live_spa_worldmap_serves_html() {
    // The planet-surface viewer. Was the most visible casualty of the
    // nginx prefix-match bug — clicking a "Map" link from /world's
    // world list landed users on a 404 "Unknown endpoint" page.
    assert_spa_route("/worldmap").await;
}

#[tokio::test]
#[ignore]
async fn live_spa_trade_serves_html() {
    assert_spa_route("/trade").await;
}

#[tokio::test]
#[ignore]
async fn live_spa_simulator_serves_html() {
    assert_spa_route("/simulator").await;
}
