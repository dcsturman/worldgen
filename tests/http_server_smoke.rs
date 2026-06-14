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

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

/// Spawn a one-shot accept loop on a free port, return the bound
/// address. Each accepted connection is handed to
/// `http_server::handle_http`.
async fn spawn_http_server() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((stream, peer)) = listener.accept().await {
            tokio::spawn(async move {
                let _ = worldgen::backend::http_server::handle_http(stream, peer).await;
            });
        }
    });
    addr
}

async fn send_request(addr: std::net::SocketAddr, request: &str) -> Vec<u8> {
    let mut sock = TcpStream::connect(addr).await.unwrap();
    sock.write_all(request.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    // 10 s is plenty: rendering a system PNG at scale=2.0 takes well
    // under a second on a modern dev box.
    timeout(Duration::from_secs(10), sock.read_to_end(&mut buf))
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
