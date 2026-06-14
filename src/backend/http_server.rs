//! HTTP endpoint serving deterministic system-map PNGs.
//!
//! The deployed binary runs one TCP listener (`bin/server.rs`) that
//! dispatches each accepted connection to either a WebSocket handler
//! (`/ws/trade`, `/ws/simulator`, `/ws/captains-log`) or this HTTP
//! handler. Dispatch happens before any handshake — `bin/server.rs`
//! peeks the first bytes of the stream and routes plain HTTP here.
//!
//! Currently exposes one route:
//!
//! - `GET /system?sector=…&hex=CCRR&name=…&uwp=…&pbg=…&stellar=…&worlds=…&scale=…`
//!   → `200 image/png` of the system-map render. See [`handle_system`]
//!   for the parameter semantics and error mapping.
//!
//! All responses include permissive CORS headers (`*` origin, GET + OPTIONS
//! allowed) so a browser client served from a different origin (e.g. the
//! Traveller Map web client) can call this without preflight failure.
//!
//! No new heavy dependency is pulled in for this — the implementation
//! hand-rolls an HTTP/1.1 request line and header parser plus minimal
//! response writers. Everything beyond that funnels through the existing
//! public library API (`system_seed`, `parse_stellar`, `build_constraints`,
//! `generate_system_png_scaled`).

use std::collections::HashMap;
use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::api::{build_constraints, generate_system_png_scaled, parse_stellar};
use crate::seed::system_seed;

/// Soft cap on the request bytes we read before bailing out. The only
/// endpoint we expose is a `GET` so the headers should be well under a
/// kilobyte; we cap at 8 KiB so a wedged or hostile client can't keep
/// us reading forever.
const MAX_HEADER_BYTES: usize = 8 * 1024;

/// Top-level HTTP entry point. Called by the dispatch loop in
/// `bin/server.rs` after it has peeked the stream and determined this
/// is an HTTP request rather than a WebSocket upgrade.
pub async fn handle_http(
    stream: TcpStream,
    peer_addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = BufReader::new(stream);
    let request_line = match read_line(&mut reader, MAX_HEADER_BYTES).await {
        Ok(l) => l,
        Err(e) => {
            log::warn!("HTTP read failed from {peer_addr}: {e}");
            return Ok(());
        }
    };

    // Drain headers (we don't need their values for these routes).
    let mut consumed = request_line.len();
    loop {
        let line = read_line(&mut reader, MAX_HEADER_BYTES - consumed).await?;
        consumed += line.len();
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
    }

    let (method, target) = match parse_request_line(&request_line) {
        Some(p) => p,
        None => {
            return write_simple(reader.get_mut(), 400, "Bad Request", "Malformed request line")
                .await;
        }
    };

    let (path, query) = split_path_query(target);

    log::info!("HTTP {} {} from {}", method, target, peer_addr);

    // Universal CORS preflight: every endpoint accepts OPTIONS by
    // returning 204 + permissive headers. Browsers fire this before
    // the actual GET when the origin differs from the server.
    if method.eq_ignore_ascii_case("OPTIONS") {
        return write_options(reader.get_mut()).await;
    }

    if !(method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("HEAD")) {
        return write_simple(reader.get_mut(), 405, "Method Not Allowed", "Use GET").await;
    }

    let head_only = method.eq_ignore_ascii_case("HEAD");

    match path {
        "/system" => handle_system(reader.get_mut(), query, head_only).await,
        _ => write_simple(reader.get_mut(), 404, "Not Found", "Unknown endpoint").await,
    }
}

/// Handler for `GET /system`. Parses required + optional query params,
/// derives a deterministic seed from the world identity, builds a
/// constraint set, renders a PNG at the requested scale, and writes
/// the response.
///
/// Error mapping:
/// - Missing or malformed required params → `400 text/plain`
/// - `build_constraints` returning `Err` (invalid / partial / contradictory
///   UWP) → `422 text/plain`
/// - Render failure (e.g. scale < 1.0, NaN, tiny-skia OOM) → `500 text/plain`
///
/// `same (sector, hex, name, uwp, pbg, stellar, worlds, scale)` always
/// yields byte-identical output — `scale` does not feed any RNG.
async fn handle_system(
    stream: &mut TcpStream,
    query: &str,
    head_only: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let params = parse_query(query);

    let sector = match params.get("sector") {
        Some(s) if !s.is_empty() => s.as_str(),
        _ => return write_simple(stream, 400, "Bad Request", "missing required param: sector").await,
    };
    let hex = match params.get("hex") {
        Some(h) if !h.is_empty() => h.as_str(),
        _ => return write_simple(stream, 400, "Bad Request", "missing required param: hex").await,
    };
    let name = match params.get("name") {
        Some(n) if !n.is_empty() => n.as_str(),
        _ => return write_simple(stream, 400, "Bad Request", "missing required param: name").await,
    };
    let uwp = match params.get("uwp") {
        Some(u) if !u.is_empty() => u.as_str(),
        _ => return write_simple(stream, 400, "Bad Request", "missing required param: uwp").await,
    };

    // `hex` is a 4-character "CCRR" sub-sector hex location. We treat
    // each pair as a u8 — Traveller hexes can run up to 32×40 per
    // sector, well within u8.
    let (hex_x, hex_y) = match parse_hex_quad(hex) {
        Some(h) => h,
        None => {
            return write_simple(
                stream,
                400,
                "Bad Request",
                "hex must be a 4-digit string like \"2018\"",
            )
            .await;
        }
    };

    let pbg = params.get("pbg").cloned().unwrap_or_default();
    let belts = digit_at(&pbg, 1).unwrap_or(0) as usize;
    let giants = digit_at(&pbg, 2).unwrap_or(0) as usize;

    let stellar = params.get("stellar").map(|s| s.as_str()).unwrap_or("");
    let stars = parse_stellar(stellar);

    // `worlds` is the system's `W` digit (total body count) from
    // Traveller Map. We back out the main world, the belts and the
    // gas giants to leave just the extra rocky planets the caller
    // wants placed. Clamps to 0 if the math goes negative.
    let worlds = params
        .get("worlds")
        .and_then(|s| s.trim().parse::<i32>().ok());
    let planets = match worlds {
        Some(w) => (w - 1 - belts as i32 - giants as i32).max(0) as usize,
        None => 0,
    };

    let scale = params
        .get("scale")
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(2.0);

    let seed = system_seed(sector, hex_x, hex_y);
    let constraints = match build_constraints(name, uwp, &stars, giants, belts, planets) {
        Ok(c) => c,
        Err(e) => {
            return write_simple(stream, 422, "Unprocessable Entity", &format!("{e}")).await;
        }
    };

    let png = match generate_system_png_scaled(seed, constraints, scale) {
        Ok(b) => b,
        Err(e) => {
            return write_simple(stream, 500, "Internal Server Error", &format!("{e}")).await;
        }
    };

    write_png(stream, &png, head_only).await
}

// ---------------------------------------------------------------------------
// Request parsing
// ---------------------------------------------------------------------------

/// Read one CRLF-terminated line into a `String`. Caps at `max` bytes
/// so a wedged client can't grow our buffer indefinitely.
async fn read_line(
    reader: &mut BufReader<TcpStream>,
    max: usize,
) -> Result<String, std::io::Error> {
    let mut out = String::new();
    let mut total = 0usize;
    loop {
        let mut buf = [0u8; 1];
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        out.push(buf[0] as char);
        total += n;
        if buf[0] == b'\n' {
            break;
        }
        if total >= max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request header line exceeded max bytes",
            ));
        }
    }
    Ok(out)
}

/// Parse `GET /path?query HTTP/1.1` → `("GET", "/path?query")`.
/// Ignores the version segment.
fn parse_request_line(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let mut parts = trimmed.splitn(3, ' ');
    let method = parts.next()?;
    let target = parts.next()?;
    Some((method, target))
}

fn split_path_query(target: &str) -> (&str, &str) {
    match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    }
}

/// Decode a URL-encoded query string into a `HashMap`. The last value
/// wins for repeated keys; bare keys without `=` are treated as `""`.
fn parse_query(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if query.is_empty() {
        return out;
    }
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        out.insert(percent_decode(k), percent_decode(v));
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'+' {
            out.push(b' ');
            i += 1;
        } else if b == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            match (hi, lo) {
                (Some(h), Some(l)) => {
                    out.push(((h << 4) | l) as u8);
                    i += 3;
                }
                _ => {
                    out.push(b);
                    i += 1;
                }
            }
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_hex_quad(s: &str) -> Option<(u8, u8)> {
    if s.len() != 4 {
        return None;
    }
    let bytes = s.as_bytes();
    if !bytes.iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let x: u8 = s[0..2].parse().ok()?;
    let y: u8 = s[2..4].parse().ok()?;
    Some((x, y))
}

fn digit_at(s: &str, idx: usize) -> Option<u32> {
    s.chars().nth(idx).and_then(|c| c.to_digit(10))
}

// ---------------------------------------------------------------------------
// Response writers
// ---------------------------------------------------------------------------

const CORS_HEADERS: &str = "Access-Control-Allow-Origin: *\r\n\
     Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
     Access-Control-Allow-Headers: *\r\n";

async fn write_simple(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    body: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let response = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         {cors}\
         \r\n\
         {body}",
        len = body.len(),
        cors = CORS_HEADERS,
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await.ok();
    Ok(())
}

async fn write_options(
    stream: &mut TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let response = format!(
        "HTTP/1.1 204 No Content\r\n\
         Content-Length: 0\r\n\
         Connection: close\r\n\
         {cors}\
         \r\n",
        cors = CORS_HEADERS,
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await.ok();
    Ok(())
}

async fn write_png(
    stream: &mut TcpStream,
    bytes: &[u8],
    head_only: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: image/png\r\n\
         Content-Length: {len}\r\n\
         Cache-Control: public, max-age=31536000, immutable\r\n\
         Connection: close\r\n\
         {cors}\
         \r\n",
        len = bytes.len(),
        cors = CORS_HEADERS,
    );
    stream.write_all(headers.as_bytes()).await?;
    if !head_only {
        stream.write_all(bytes).await?;
    }
    stream.shutdown().await.ok();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests (the request parsing — endpoint flow is exercised end-to-end
// from `tests/http_server_smoke.rs`).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_request_line_basic() {
        let (m, t) = parse_request_line("GET /system?foo=bar HTTP/1.1\r\n").unwrap();
        assert_eq!(m, "GET");
        assert_eq!(t, "/system?foo=bar");
    }

    #[test]
    fn split_path_query_no_question_mark() {
        let (p, q) = split_path_query("/system");
        assert_eq!(p, "/system");
        assert_eq!(q, "");
    }

    #[test]
    fn split_path_query_with_query() {
        let (p, q) = split_path_query("/system?a=1&b=2");
        assert_eq!(p, "/system");
        assert_eq!(q, "a=1&b=2");
    }

    #[test]
    fn parse_query_basic() {
        let m = parse_query("sector=Trojan%20Reach&hex=2018&uwp=D8867BB-1");
        assert_eq!(m.get("sector").unwrap(), "Trojan Reach");
        assert_eq!(m.get("hex").unwrap(), "2018");
        assert_eq!(m.get("uwp").unwrap(), "D8867BB-1");
    }

    #[test]
    fn parse_query_plus_decodes_to_space() {
        let m = parse_query("sector=Trojan+Reach");
        assert_eq!(m.get("sector").unwrap(), "Trojan Reach");
    }

    #[test]
    fn parse_query_handles_bare_key() {
        let m = parse_query("a&b=1");
        assert_eq!(m.get("a").unwrap(), "");
        assert_eq!(m.get("b").unwrap(), "1");
    }

    #[test]
    fn parse_hex_quad_valid() {
        assert_eq!(parse_hex_quad("2018"), Some((20, 18)));
        assert_eq!(parse_hex_quad("3128"), Some((31, 28)));
        assert_eq!(parse_hex_quad("0000"), Some((0, 0)));
    }

    #[test]
    fn parse_hex_quad_rejects_short_long_or_nondigit() {
        assert_eq!(parse_hex_quad("201"), None);
        assert_eq!(parse_hex_quad("20180"), None);
        assert_eq!(parse_hex_quad("20A1"), None);
    }

    #[test]
    fn digit_at_extracts_pbg_digits() {
        // Noricum PBG is "804" — pop=8, belts=0, giants=4.
        assert_eq!(digit_at("804", 0), Some(8));
        assert_eq!(digit_at("804", 1), Some(0));
        assert_eq!(digit_at("804", 2), Some(4));
        assert_eq!(digit_at("804", 3), None);
        // Non-digit char yields None.
        assert_eq!(digit_at("8X4", 1), None);
    }
}
