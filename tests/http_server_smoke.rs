//! End-to-end smoke test for `worldgen::backend::http_server`.
//!
//! Each test binds a real `TcpListener` on a free port, spawns the
//! dispatcher in a tokio task, sends a hand-rolled HTTP/1.1 request,
//! and asserts the response. This exercises the full path the
//! deployed binary takes: HTTP parse → query decode → library API →
//! PNG response with CORS headers.
//!
//! Only compiled under `--features backend` (the only mode where
//! `worldgen::backend` exists).

#![cfg(feature = "backend")]

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use worldgen::backend::gcs::GcsClient;

/// Spawn a one-shot accept loop on a free port, return the bound
/// address. Each accepted connection is handed to
/// `http_server::handle_http`. The GCS client is initialized in
/// disabled mode (`GCS_BUCKET=debug` is set before each test) so the
/// tests don't need GCP creds and the `/world` endpoint always
/// regenerates instead of caching.
async fn spawn_http_server() -> std::net::SocketAddr {
    // SAFETY: this is a test process. Setting an env var here is fine
    // because the runtime's std::env access is single-threaded inside
    // a single tokio test binary.
    unsafe {
        std::env::set_var("GCS_BUCKET", "debug");
    }
    let gcs = Arc::new(GcsClient::init().await.expect("disabled GCS init"));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((stream, peer)) = listener.accept().await {
            let gcs = gcs.clone();
            tokio::spawn(async move {
                let _ = worldgen::backend::http_server::handle_http(stream, peer, gcs).await;
            });
        }
    });
    addr
}

async fn send_request(addr: std::net::SocketAddr, request: &str) -> Vec<u8> {
    let mut sock = TcpStream::connect(addr).await.unwrap();
    sock.write_all(request.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    // 120 s budget — system PNGs at scale=2.0 finish in under a
    // second, but /world renders take ~30 s in debug builds (the
    // raster loop is the slow path and is unoptimized without
    // --release). The timeout is the safety net for a wedged test,
    // not the expected duration.
    timeout(Duration::from_secs(120), sock.read_to_end(&mut buf))
        .await
        .expect("response read timed out")
        .unwrap();
    buf
}

fn split_response(buf: &[u8]) -> (String, Vec<u8>) {
    let split = buf
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("no header/body boundary");
    let head = String::from_utf8_lossy(&buf[..split]).into_owned();
    let body = buf[split + 4..].to_vec();
    (head, body)
}

#[tokio::test]
async fn get_system_returns_png_with_cors_headers() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /system?sector=Trojan+Reach&hex=2018&name=Noricum&uwp=D8867BB-1&pbg=804&stellar=G2+V+M9+V+M6+V&worlds=14 \
         HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let (head, body) = split_response(&buf);

    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "head:\n{head}");
    assert!(head.contains("Content-Type: image/png"));
    assert!(head.contains("Access-Control-Allow-Origin: *"));
    assert!(head.contains("Access-Control-Allow-Methods: GET, HEAD, OPTIONS"));

    // PNG magic + the response should be the 3200x1800 default (scale=2.0)
    assert_eq!(&body[..8], b"\x89PNG\r\n\x1a\n");
    let w = u32::from_be_bytes([body[16], body[17], body[18], body[19]]);
    let h = u32::from_be_bytes([body[20], body[21], body[22], body[23]]);
    assert_eq!((w, h), (3200, 1800), "default scale=2.0 should be 3200x1800");
}

#[tokio::test]
async fn get_system_is_byte_identical_across_calls() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /system?sector=Trojan+Reach&hex=2018&name=Noricum&uwp=D8867BB-1&pbg=804&stellar=G2+V+M9+V+M6+V&worlds=14 \
         HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf1 = send_request(addr, &req).await;
    let buf2 = send_request(addr, &req).await;
    let (_, body1) = split_response(&buf1);
    let (_, body2) = split_response(&buf2);
    assert_eq!(body1, body2, "HTTP determinism contract broken");
}

#[tokio::test]
async fn get_system_with_scale_1_returns_1600x900() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /system?sector=Trojan+Reach&hex=2018&name=Noricum&uwp=D8867BB-1&scale=1.0 \
         HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let (head, body) = split_response(&buf);
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
    let w = u32::from_be_bytes([body[16], body[17], body[18], body[19]]);
    let h = u32::from_be_bytes([body[20], body[21], body[22], body[23]]);
    assert_eq!((w, h), (1600, 900));
}

#[tokio::test]
async fn get_system_with_bad_uwp_returns_422() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /system?sector=x&hex=0000&name=x&uwp=NOT-A-UWP-WAY-TOO-LONG \
         HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let head = split_response(&buf).0;
    assert!(
        head.starts_with("HTTP/1.1 422 Unprocessable Entity\r\n"),
        "head:\n{head}"
    );
    assert!(head.contains("Access-Control-Allow-Origin: *"));
}

#[tokio::test]
async fn get_system_with_missing_required_param_returns_400() {
    let addr = spawn_http_server().await;
    // Missing `hex` parameter.
    let req = format!(
        "GET /system?sector=x&name=x&uwp=A788899-A HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let (head, body) = split_response(&buf);
    assert!(head.starts_with("HTTP/1.1 400 Bad Request\r\n"), "head:\n{head}");
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("hex"), "body should name the missing param: {body}");
}

#[tokio::test]
async fn options_returns_204_with_cors_headers() {
    let addr = spawn_http_server().await;
    let req = format!(
        "OPTIONS /system HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let head = split_response(&buf).0;
    assert!(head.starts_with("HTTP/1.1 204 No Content\r\n"), "head:\n{head}");
    assert!(head.contains("Access-Control-Allow-Origin: *"));
    assert!(head.contains("Access-Control-Allow-Methods: GET, HEAD, OPTIONS"));
}

#[tokio::test]
async fn unknown_path_returns_404() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /not-a-real-endpoint HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let head = split_response(&buf).0;
    assert!(head.starts_with("HTTP/1.1 404 Not Found\r\n"), "head:\n{head}");
}

#[tokio::test]
async fn invalid_hex_quad_returns_400() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /system?sector=x&hex=20A1&name=x&uwp=A788899-A \
         HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let head = split_response(&buf).0;
    assert!(head.starts_with("HTTP/1.1 400 Bad Request\r\n"), "head:\n{head}");
}

// ---------------------------------------------------------------------------
// /world endpoint smoke tests. These exercise the same dispatch path as
// /system but go through the planet renderer + cache machinery. The
// `spawn_http_server` helper puts the GCS client into disabled mode so
// each /world request regenerates from scratch (~30 s in debug builds)
// — slow but reproducible without GCP creds.
// ---------------------------------------------------------------------------

const NORICUM_WORLD_QUERY: &str =
    "sector=Trojan+Reach&hex=2018&name=Noricum&uwp=D8867BB-1";

#[tokio::test]
async fn get_world_returns_png_with_cors_and_x_cache_headers() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /world?{NORICUM_WORLD_QUERY} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let (head, body) = split_response(&buf);

    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "head:\n{head}");
    assert!(head.contains("Content-Type: image/png"));
    assert!(head.contains("Access-Control-Allow-Origin: *"));
    // Disabled GCS short-circuits to regenerate; the handler reports
    // that explicitly so a downstream consumer can tell apart "cache
    // hit" from "no cache at all".
    assert!(
        head.contains("X-Cache: DISABLED"),
        "expected X-Cache: DISABLED with debug bucket, head:\n{head}"
    );

    assert!(body.len() > 10_000, "PNG suspiciously small: {} bytes", body.len());
    assert_eq!(&body[..8], b"\x89PNG\r\n\x1a\n");
}

#[tokio::test]
async fn get_world_at_canonical_scale_matches_no_scale_param_byte_for_byte() {
    // Default scale=1.0 and explicit scale=1.0 should produce the same
    // bytes. They also exercise different code paths: default lands in
    // the downsample branch (output_scale=1.0 < CANONICAL=2.0); explicit
    // scale=1.0 hits the same branch. Same output regardless.
    let addr = spawn_http_server().await;
    let req_default = format!(
        "GET /world?{NORICUM_WORLD_QUERY} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let req_explicit = format!(
        "GET /world?{NORICUM_WORLD_QUERY}&scale=1.0 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let a = split_response(&send_request(addr, &req_default).await).1;
    let b = split_response(&send_request(addr, &req_explicit).await).1;
    assert_eq!(a, b, "default and explicit scale=1.0 must produce identical bytes");
}

#[tokio::test]
async fn get_world_at_scale_2_is_canonical_2000x1310() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /world?{NORICUM_WORLD_QUERY}&scale=2.0 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let (head, body) = split_response(&buf);
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
    let w = u32::from_be_bytes([body[16], body[17], body[18], body[19]]);
    let h = u32::from_be_bytes([body[20], body[21], body[22], body[23]]);
    assert_eq!(
        (w, h),
        (2000, 1310),
        "scale=2.0 should yield the canonical 2000x1310"
    );
}

#[tokio::test]
async fn get_world_is_byte_deterministic_across_calls() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /world?{NORICUM_WORLD_QUERY} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let a = split_response(&send_request(addr, &req).await).1;
    let b = split_response(&send_request(addr, &req).await).1;
    assert_eq!(a, b, "determinism contract: same query → same bytes");
}

#[tokio::test]
async fn get_world_with_bad_uwp_returns_422() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /world?sector=x&hex=0000&name=x&uwp=X???????-? HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let head = split_response(&buf).0;
    assert!(
        head.starts_with("HTTP/1.1 422 Unprocessable Entity\r\n"),
        "head:\n{head}"
    );
    assert!(head.contains("Access-Control-Allow-Origin: *"));
}

#[tokio::test]
async fn get_world_with_missing_required_param_returns_400() {
    let addr = spawn_http_server().await;
    // Drop the `hex` param.
    let req = format!(
        "GET /world?sector=x&name=x&uwp=A788899-A HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let (head, body) = split_response(&buf);
    assert!(head.starts_with("HTTP/1.1 400 Bad Request\r\n"), "head:\n{head}");
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("hex"));
}

#[tokio::test]
async fn get_world_with_scale_above_canonical_is_clamped() {
    // Request scale=4.0 — that's > CANONICAL (2.0), so the handler
    // clamps to canonical rather than upsampling (which would just
    // give a blurry larger image) or regenerating fresh (which would
    // defeat the cache).
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /world?{NORICUM_WORLD_QUERY}&scale=4.0 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let (head, body) = split_response(&buf);
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"));
    let w = u32::from_be_bytes([body[16], body[17], body[18], body[19]]);
    let h = u32::from_be_bytes([body[20], body[21], body[22], body[23]]);
    assert_eq!(
        (w, h),
        (2000, 1310),
        "scale=4.0 should clamp to canonical 2000x1310, not 4000x2620"
    );
}

#[tokio::test]
async fn get_world_with_scale_below_1_returns_400() {
    let addr = spawn_http_server().await;
    let req = format!(
        "GET /world?{NORICUM_WORLD_QUERY}&scale=0.5 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    );
    let buf = send_request(addr, &req).await;
    let head = split_response(&buf).0;
    assert!(head.starts_with("HTTP/1.1 400 Bad Request\r\n"), "head:\n{head}");
}
