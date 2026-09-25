//! HTTP endpoint serving deterministic system-map and planet renders.
//!
//! The deployed binary runs one TCP listener (`bin/server.rs`) that
//! dispatches each accepted connection to either a WebSocket handler
//! (`/ws/trade`, `/ws/simulator`, `/ws/captains-log`) or this HTTP
//! handler. Dispatch happens before any handshake — `bin/server.rs`
//! peeks the first bytes of the stream and routes plain HTTP here.
//!
//! Routes:
//!
//! - `GET /api/system?sector=…&hex=CCRR&name=…&uwp=…&pbg=…&stellar=…&worlds=…&scale=…`
//!   → `200 image/png` of the system-map render. See [`handle_system`].
//! - `GET /api/system_svg?…` (same query params) → `200 image/svg+xml` of
//!   the same render as vectors, with each body wrapped in a
//!   `<g class="sysmap-body" data-…>` group so consumers can make bodies
//!   clickable. `scale` is accepted but ignored (SVG is
//!   resolution-independent). See [`handle_system_svg`]. Both share
//!   [`parse_system_request`] for parsing/validation.
//! - `GET /api/world?…` → `200 image/png` of a planet surface (GCS-cached).
//!   An optional `deco=…` (e.g. `deco=tl`) renders the world with
//!   decorations beyond its UWP; see [`resolve_decorations`].
//!
//! All responses include permissive CORS headers (`*` origin, GET + OPTIONS
//! allowed) so a browser client served from a different origin (e.g. the
//! Traveller Map web client) can call this without preflight failure.
//!
//! No new heavy dependency is pulled in for this — the implementation
//! hand-rolls an HTTP/1.1 request line and header parser plus minimal
//! response writers. Everything beyond that funnels through the existing
//! public library API (`system_seed`, `parse_stellar`, `build_constraints`,
//! `generate_system_png_scaled`, `generate_system_svg`).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use siphasher::sip::SipHasher24;
use std::hash::Hasher;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::api::{
    generate_globe_apng, generate_globe_png, generate_globe_texture,
    generate_planet_png_scaled_decorated, generate_system_png_scaled, generate_system_svg,
    parse_hex_quad,
};
use crate::backend::gcs::GcsClient;
use crate::decorations::{DecorationError, WorldDecorations};
use crate::worldmap::{ApngTiming, TexSize};
use crate::seed::{planet_seed, system_seed};
use crate::systems::constraint::SystemConstraints;
use crate::systems::overrides;

/// Always render planet PNGs at this scale, regardless of the request's
/// `scale` query param. On cache-hit we decode the cached PNG and
/// downsample to the requested scale. This way the GCS bucket holds
/// **one** PNG per world rather than N (one per requested scale), and
/// the 20–30 s compute cost is paid exactly once per world per
/// worldgen version.
///
/// 2.0 is also the default scale of `/api/system`, so a request to either
/// endpoint with no explicit scale produces a comparably-sized image.
const PLANET_CANONICAL_SCALE: f32 = 2.0;

/// GCS object-path prefix for cached planet PNGs. The version segment
/// (`v2`) lets us bust the cache on a worldgen version bump by
/// changing the prefix instead of deleting objects.
///
/// The cache key is `(seed, uwp, name, deco)` — pure world *identity*, with nothing
/// about the generator that produced the image — so a change to worldgen does
/// not invalidate anything on its own. Without a bump, a world someone had
/// already viewed would keep serving its old terrain indefinitely while a
/// world nobody had opened yet would render with the new: the same world
/// showing up as two different planets depending on view history.
///
/// Bumped to `v2` for the terrain changes of 2026-09-06 — continuous globe
/// colormap, high-frequency relief, fractal coastlines via a fine domain-warp
/// band, and the plate-bias fix that removed the straight channels along
/// continental plate boundaries. Old `world/v1/` objects are orphaned rather
/// than deleted, so this is reversible by putting the prefix back.
const PLANET_CACHE_PREFIX: &str = "world/v2";

/// Extra object-path segment for renders of a *decorated* world (e.g. a
/// tidal lock), placed after the variant: `world/v2/deco-v1/…`,
/// `world/v2/globe/deco-v1/…`. Undecorated worlds keep their old paths.
///
/// The decorations are already in the cache key, so this isn't needed to
/// keep decorated and undecorated renders apart. It exists so that locked
/// renders can be invalidated on their own: bump it whenever the decorated
/// climate model changes what a world looks like, and only decorated worlds
/// re-render — bumping `PLANET_CACHE_PREFIX` would throw away every
/// undecorated world in the bucket too.
const DECO_CACHE_VERSION: &str = "deco-v1";

/// Globe (orthographic projection) render parameters for `?projection=globe`.
/// Fixed server-side so the cache key stays `(seed, uwp, name, deco)` per variant
/// rather than fanning out over arbitrary sizes. The static PNG renders a bit
/// larger than the animation, which is kept smaller to bound the APNG size
/// (one full RGBA frame per `GLOBE_FRAMES`).
const GLOBE_PNG_SIZE: u32 = 512;
const GLOBE_APNG_SIZE: u32 = 400;
const GLOBE_FRAMES: u32 = 36;
/// Per-frame hold = `GLOBE_DELAY_NUM / GLOBE_DELAY_DEN` seconds. 1/5 s × 36
/// frames ≈ 7.2 s per rotation — a slow, readable spin.
const GLOBE_DELAY_NUM: u16 = 1;
const GLOBE_DELAY_DEN: u16 = 5;

/// SipHash key for cache-key derivation. Separate from the keys in
/// `src/seed.rs` so a future change to one doesn't accidentally
/// invalidate the other. Pinned forever — change these and every
/// cached object becomes orphaned.
const CACHE_SIP_KEY_0: u64 = 0x776f_726c_645f_6361; // "world_ca"
const CACHE_SIP_KEY_1: u64 = 0x6368_655f_7631_5f00; // "che_v1_\0"

/// Soft cap on the request bytes we read before bailing out. The only
/// endpoint we expose is a `GET` so the headers should be well under a
/// kilobyte; we cap at 8 KiB so a wedged or hostile client can't keep
/// us reading forever.
const MAX_HEADER_BYTES: usize = 8 * 1024;

/// Top-level HTTP entry point. Called by the dispatch loop in
/// `bin/server.rs` after it has peeked the stream and determined this
/// is an HTTP request rather than a WebSocket upgrade.
///
/// `gcs` is shared across every request — it's a `reqwest::Client`
/// internally (already cheap to clone) plus a possibly-`None` bucket
/// name (disabled mode). Disabled clients short-circuit cache I/O so
/// local dev runs without GCP creds.
pub async fn handle_http(
    stream: TcpStream,
    peer_addr: SocketAddr,
    gcs: Arc<GcsClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = BufReader::new(stream);
    let request_line = match read_line(&mut reader, MAX_HEADER_BYTES).await {
        Ok(l) => l,
        Err(e) => {
            log::warn!("HTTP read failed from {peer_addr}: {e}");
            return Ok(());
        }
    };

    // Drain headers, keeping the one we act on.
    //
    // `If-None-Match` matters for the system endpoints: their ETag is built
    // from the request's *inputs* rather than its output, so a match can be
    // answered with a 304 without generating or rendering anything.
    let mut consumed = request_line.len();
    let mut if_none_match: Option<String> = None;
    loop {
        let line = read_line(&mut reader, MAX_HEADER_BYTES - consumed).await?;
        consumed += line.len();
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("if-none-match")
        {
            if_none_match = Some(value.trim().to_string());
        }
    }

    let (method, target) = match parse_request_line(&request_line) {
        Some(p) => p,
        None => {
            return write_simple(
                reader.get_mut(),
                400,
                "Bad Request",
                "Malformed request line",
            )
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

    // All HTTP API routes live under `/api/` to keep them out of the way
    // of the SPA's path-based routing (`/world`, `/worldmap`, `/trade`,
    // `/simulator`, `/` — see `src/bin/main.rs`). Without the prefix,
    // nginx's `location /world { proxy_pass … }` block prefix-matched
    // `/worldmap`, broke the SPA planet-viewer page, and silently
    // intercepted bare `/world` system-generator navigation.
    match path {
        // Readiness, not liveness. Cloud Run's startup probe defaults to a
        // TCP check on port 80 — which nginx satisfies the moment it binds,
        // roughly two seconds before this server binds 8081. In that window
        // the instance is "healthy" and receiving traffic, and every request
        // gets nginx's 502 from a refused upstream connect. That is not
        // hypothetical: it is what a scale-out event did to a production
        // smoke test, whose byte-comparison then reported a 2.4 MB PNG and a
        // 157-byte error page as "output drifted".
        //
        // Answering here means a probe against this path only passes once
        // *both* processes are up, which is the actual condition for the
        // instance being able to serve. Point the Cloud Run startup probe at
        // it (httpGet /api/health) — a TCP probe on 80 cannot express this.
        "/api/health" => write_simple(reader.get_mut(), 200, "OK", "ok").await,
        "/api/system" => {
            handle_system(reader.get_mut(), query, head_only, if_none_match.as_deref()).await
        }
        "/api/system_svg" => {
            handle_system_svg(reader.get_mut(), query, head_only, if_none_match.as_deref()).await
        }
        "/api/world" => handle_world(reader.get_mut(), query, head_only, gcs).await,
        _ => write_simple(reader.get_mut(), 404, "Not Found", "Unknown endpoint").await,
    }
}

/// How long to wait for a cache upload before giving up and serving anyway.
///
/// The upload used to be a detached `tokio::spawn` so the response could ship
/// first. On Cloud Run that is precisely backwards: CPU is throttled to near
/// zero the moment a response completes, so the upload was being scheduled at
/// the instant the instance lost the ability to perform it, and it died with
/// "error sending request". Every such failure means the next viewer of that
/// world waits for a full render again — and a cold globe is ~10 s, so the
/// cost of a lost write is far larger than the cost of waiting for it.
///
/// Awaiting it inline costs a fraction of a second on a path that already
/// took seconds to render. The timeout is there so a wedged GCS degrades to
/// "slow once" rather than holding the client open.
const CACHE_PUT_TIMEOUT: Duration = Duration::from_secs(10);

/// Write a freshly rendered planet into the cache, logging rather than
/// failing the request: a lost cache write costs the next viewer a re-render,
/// which is not a reason to deny this one the image it already has.
async fn cache_put(gcs: &Arc<GcsClient>, key: &str, bytes: Vec<u8>) {
    match tokio::time::timeout(CACHE_PUT_TIMEOUT, gcs.put(key, bytes, "image/png")).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => log::warn!("GCS put failed for {key}: {e}"),
        Err(_) => log::warn!(
            "GCS put for {key} exceeded {CACHE_PUT_TIMEOUT:?}; serving anyway, \
             the next request for this world will re-render"
        ),
    }
}

/// The parsed, validated inputs shared by `/api/system` (PNG) and
/// `/api/system_svg` (SVG). Both endpoints take the identical query string
/// and derive the same `(seed, constraints)`; only the render target and the
/// `scale` use differ, so the parsing lives in one place.
struct SystemRequest {
    seed: u64,
    constraints: SystemConstraints,
    /// Requested pixel scale. Used by the PNG path; the SVG path ignores it
    /// (vector output is resolution-independent).
    scale: f32,
}

/// HTTP error to surface to the client: `(status code, reason, body)`.
type HttpError = (u16, &'static str, String);

/// Run a synchronous render closure, turning a panic into a recoverable
/// `Err(message)` instead of letting it tear down the whole server.
///
/// The render pipeline (system generation, sysmap/worldmap rasterizing)
/// is a large body of index-and-unwrap code reachable from untrusted
/// query params; a single pathological world must not be able to take
/// the service down for every other connected client. This catches any
/// `panic!` from one request at the boundary and lets the handler answer
/// `500` instead. It only works when the binary is built with
/// `panic = "unwind"` — the `release-server` profile (and the default
/// dev/test profile). The size-optimized `[profile.release]` keeps
/// `panic = "abort"` for the WASM bundle, so this is paired with input
/// validation in the generators themselves rather than relied on alone.
fn catch_render<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|payload| {
        if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        }
    })
}

/// Log a caught render panic and answer with a generic `500` (which still
/// carries CORS headers via `write_simple`, so the browser sees a clean
/// error instead of a CORS failure from a dropped upstream connection).
/// The panic message is logged server-side, not leaked to the client.
async fn render_panic_500(
    stream: &mut TcpStream,
    route: &str,
    panic_msg: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log::error!("panic while rendering {route}: {panic_msg}");
    write_simple(stream, 500, "Internal Server Error", "internal render error").await
}

/// Parse + validate the shared `/api/system*` query params. On success
/// returns the deterministic seed, the constraint set, and the requested
/// scale. On failure returns the status/reason/body the caller should write.
///
/// Error mapping:
/// - Missing or malformed required params → `400`
/// - `build_constraints` returning `Err` (invalid / partial / contradictory
///   UWP) → `422`
fn parse_system_request(query: &str) -> Result<SystemRequest, HttpError> {
    let params = parse_query(query);

    let missing = |p: &str| (400, "Bad Request", format!("missing required param: {p}"));
    let sector = params
        .get("sector")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| missing("sector"))?;
    let hex = params
        .get("hex")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| missing("hex"))?;
    let name = params
        .get("name")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| missing("name"))?;
    let uwp = params
        .get("uwp")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| missing("uwp"))?;

    let pbg = params.get("pbg").cloned().unwrap_or_default();
    let stellar = params.get("stellar").map(|s| s.as_str()).unwrap_or("");
    let worlds = params.get("worlds").and_then(|s| s.trim().parse::<i32>().ok());

    let scale = params
        .get("scale")
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(2.0);

    // One shared path, in the library: the override validator calls exactly
    // this, so what it checks is what this endpoint will generate rather
    // than a parallel implementation that happens to agree today.
    let (seed, constraints) = crate::api::system_from_upstream(&crate::api::UpstreamSystem {
        sector,
        hex,
        name,
        uwp,
        pbg: &pbg,
        stellar,
        worlds,
    })
    .map_err(|e| match e {
        crate::api::UpstreamError::BadHex(_) => (400, "Bad Request", e.to_string()),
        crate::api::UpstreamError::Constraints(_) => {
            (422, "Unprocessable Entity", e.to_string())
        }
        // Our data, not the caller's request.
        crate::api::UpstreamError::Override(_) => {
            (500, "Internal Server Error", e.to_string())
        }
    })?;

    Ok(SystemRequest {
        seed,
        constraints,
        scale,
    })
}

/// Handler for `GET /api/system`. Renders the system map as a PNG at the
/// requested scale. `same (sector, hex, name, uwp, pbg, stellar, worlds,
/// scale)` always yields byte-identical output — `scale` does not feed any
/// RNG. Render failure (scale < 1.0, NaN, tiny-skia OOM) → `500 text/plain`.
async fn handle_system(
    stream: &mut TcpStream,
    query: &str,
    head_only: bool,
    if_none_match: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Answered before parsing or rendering: the ETag comes from the inputs,
    // so a client that already holds this response costs nothing to serve.
    let etag = system_etag(query);
    if etag_matches(if_none_match, &etag) {
        return write_not_modified(stream, &etag).await;
    }
    let req = match parse_system_request(query) {
        Ok(r) => r,
        Err((code, reason, body)) => return write_simple(stream, code, reason, &body).await,
    };

    let png = match catch_render(|| generate_system_png_scaled(req.seed, req.constraints, req.scale))
    {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => {
            return write_simple(stream, 500, "Internal Server Error", &format!("{e}")).await;
        }
        Err(panic_msg) => return render_panic_500(stream, "/api/system", &panic_msg).await,
    };

    write_system_png(stream, &png, head_only, &etag).await
}

/// Handler for `GET /api/system_svg`. The vector parallel to
/// `/api/system`: identical query params, but the response is an
/// `image/svg+xml` document whose bodies are wrapped in
/// `<g class="sysmap-body" data-…>` groups so a consuming web app can make
/// individual bodies clickable. SVG is resolution-independent, so the
/// `scale` param is accepted but ignored.
async fn handle_system_svg(
    stream: &mut TcpStream,
    query: &str,
    head_only: bool,
    if_none_match: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let etag = system_etag(query);
    if etag_matches(if_none_match, &etag) {
        return write_not_modified(stream, &etag).await;
    }
    let req = match parse_system_request(query) {
        Ok(r) => r,
        Err((code, reason, body)) => return write_simple(stream, code, reason, &body).await,
    };

    let svg = match catch_render(|| generate_system_svg(req.seed, req.constraints)) {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            return write_simple(stream, 500, "Internal Server Error", &format!("{e}")).await;
        }
        Err(panic_msg) => return render_panic_500(stream, "/api/system_svg", &panic_msg).await,
    };

    write_svg(stream, svg.as_bytes(), head_only, &etag).await
}

/// Handler for `GET /api/world`. Renders a planet surface PNG, caching the
/// canonical-scale render in GCS. Subsequent requests for the same
/// `(sector, hex, name, uwp, orbit)` are served from the cache and
/// downsampled to the requested scale instead of paying the 20–30 s
/// generation cost again.
///
/// Deterministic seed chain (identical to `/api/system`'s, just continuing
/// through `planet_seed`):
/// ```text
/// (sector, hex_x, hex_y)  →  seed::system_seed       →  sys_seed
/// (sys_seed, orbit, name) →  seed::planet_seed       →  seed
/// generate_planet_png_scaled_decorated(seed, uwp, Some(name), CANONICAL_SCALE, deco)
///   └─ worldmap::generate_decorated(uwp, seed, name, deco)
///       └─ ChaCha8Rng::seed_from_u64(seed)
/// ```
///
/// `deco` is the world's decorations (see [`resolve_decorations`]): the
/// `deco` query param when given, otherwise whatever `data/overrides.json`
/// states for the named world at `(sector, hex)`. It isn't part of the
/// seed — adding a tidal lock changes the climate, not the terrain seed —
/// but it is part of the cache key and path ([`planet_cache_object`]).
///
/// `scale` is **not** part of the seed or the cache key — the bucket
/// only ever stores the canonical-scale PNG, and the response is
/// downsampled on-the-fly. `scale > CANONICAL_SCALE` is clamped (we
/// don't upsample).
///
/// Error mapping mirrors `/api/system`: 400 missing param or bad `deco`,
/// 422 invalid UWP (from `worldmap::generate` → `MapError`), 500 render
/// failure.
async fn handle_world(
    stream: &mut TcpStream,
    query: &str,
    head_only: bool,
    gcs: Arc<GcsClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let params = parse_query(query);

    let sector = match params.get("sector") {
        Some(s) if !s.is_empty() => s.as_str(),
        _ => {
            return write_simple(stream, 400, "Bad Request", "missing required param: sector")
                .await;
        }
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

    let orbit = params
        .get("orbit")
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(3);

    let deco = match resolve_decorations(&params, sector, hex, name) {
        Ok(d) => d,
        Err(e) => {
            return write_simple(stream, 400, "Bad Request", &format!("bad deco: {e}")).await;
        }
    };

    // Projection: `flat` (default — the equirectangular map every existing
    // consumer already gets) or `globe` (orthographic spinning planet). The
    // globe path has its own cache namespace and output (static PNG or
    // animated APNG), so it branches off before the flat-only `scale`
    // handling below.
    if params
        .get("projection")
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("globe"))
    {
        let sys_seed = system_seed(sector, hex_x, hex_y);
        let seed = planet_seed(sys_seed, orbit, name);
        return handle_world_globe(stream, &params, seed, uwp, name, &deco, head_only, gcs)
            .await;
    }

    // Requested scale: defaults to 1.0 to match `generate_planet_png`'s
    // legacy native resolution. Values > CANONICAL_SCALE are clamped
    // (we don't upsample — the cache holds canonical-scale bytes and
    // upsampling would just give a blurry larger image).
    let requested_scale = params
        .get("scale")
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(1.0);
    if !requested_scale.is_finite() || requested_scale < 1.0 {
        return write_simple(
            stream,
            400,
            "Bad Request",
            "scale must be finite and >= 1.0",
        )
        .await;
    }
    let output_scale = requested_scale.min(PLANET_CANONICAL_SCALE);

    let sys_seed = system_seed(sector, hex_x, hex_y);
    let seed = planet_seed(sys_seed, orbit, name);
    let cache_key = planet_cache_key(seed, uwp, name, &deco);
    let cache_object = planet_cache_object(None, cache_key, &deco);

    // Try cache first. Disabled-mode GCS returns Ok(None) here so the
    // cache_status will be "DISABLED" rather than "HIT".
    let (canonical_bytes, cache_status) = match gcs.get(&cache_object).await {
        Ok(Some(bytes)) => (bytes, "HIT"),
        Ok(None) if gcs.is_disabled() => {
            let bytes =
                match catch_render(|| {
                    generate_planet_png_scaled_decorated(
                        seed,
                        uwp,
                        Some(name),
                        PLANET_CANONICAL_SCALE,
                        &deco,
                    )
                }) {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => return classify_render_error(stream, e).await,
                    Err(panic_msg) => {
                        return render_panic_500(stream, "/api/world", &panic_msg).await;
                    }
                };
            (bytes, "DISABLED")
        }
        Ok(None) => {
            let bytes =
                match catch_render(|| {
                    generate_planet_png_scaled_decorated(
                        seed,
                        uwp,
                        Some(name),
                        PLANET_CANONICAL_SCALE,
                        &deco,
                    )
                }) {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => return classify_render_error(stream, e).await,
                    Err(panic_msg) => {
                        return render_panic_500(stream, "/api/world", &panic_msg).await;
                    }
                };
            // Awaited, not detached — see CACHE_PUT_TIMEOUT.
            cache_put(&gcs, &cache_object, bytes.clone()).await;
            (bytes, "MISS")
        }
        Err(e) => {
            log::warn!("GCS get failed for {cache_object}: {e}; regenerating");
            let bytes =
                match catch_render(|| {
                    generate_planet_png_scaled_decorated(
                        seed,
                        uwp,
                        Some(name),
                        PLANET_CANONICAL_SCALE,
                        &deco,
                    )
                }) {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => return classify_render_error(stream, e).await,
                    Err(panic_msg) => {
                        return render_panic_500(stream, "/api/world", &panic_msg).await;
                    }
                };
            (bytes, "BYPASS")
        }
    };

    // Downsample if the request asked for less than canonical. At
    // canonical we serve the cached bytes directly — bit-equivalence
    // is the contract.
    let response_bytes = if (output_scale - PLANET_CANONICAL_SCALE).abs() < f32::EPSILON {
        canonical_bytes
    } else {
        let factor = output_scale / PLANET_CANONICAL_SCALE;
        match downsample_png(&canonical_bytes, factor) {
            Ok(b) => b,
            Err(e) => {
                return write_simple(
                    stream,
                    500,
                    "Internal Server Error",
                    &format!("downsample failed: {e}"),
                )
                .await;
            }
        }
    };

    write_png(stream, &response_bytes, head_only, Some(cache_status)).await
}

/// Globe sub-handler for `GET /api/world?projection=globe`.
///
/// Renders the planet as an orthographic globe — either a static PNG
/// (`format=png`/`static`) or a spinning animated PNG (default, or
/// `format=apng`). Both are cached in GCS under a projection-specific path
/// (`world/v2/globe[-anim]/…`) so they never collide with the flat map's
/// cache. The render size and frame count are fixed server-side
/// ([`GLOBE_PNG_SIZE`] etc.) so the cache key stays `(seed, uwp, name, deco)`
/// per variant.
#[allow(clippy::too_many_arguments)]
async fn handle_world_globe(
    stream: &mut TcpStream,
    params: &HashMap<String, String>,
    seed: u64,
    uwp: &str,
    name: &str,
    deco: &WorldDecorations,
    head_only: bool,
    gcs: Arc<GcsClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let format = params
        .get("format")
        .map(|s| s.trim().to_ascii_lowercase());

    // `format=texture` serves the raw equirectangular surface texture for
    // client-side (WebGL) globe rendering, with the starport coords in a
    // header. It has its own response shape, so branch before the image path.
    if format.as_deref() == Some("texture") {
        // `clouds=0`/`false`/`no`/`off` opts out of the baked cloud deck.
        // Default on: the deck is derived from the UWP's atmosphere and
        // hydrographics, so it carries information about the world. But it is
        // composited into the surface RGB — there is nowhere to put a separate
        // layer in a single texture — so a consumer that wants the bare
        // surface, or composites its own weather, needs a way to say so.
        let clouds = !matches!(
            params.get("clouds").map(|s| s.trim().to_ascii_lowercase()).as_deref(),
            Some("0") | Some("false") | Some("no") | Some("off")
        );
        return handle_world_globe_texture(stream, gcs, seed, uwp, name, deco, head_only, clouds)
            .await;
    }

    // Animated by default; `format=png`/`static` asks for a single frame.
    let animated = !matches!(format.as_deref(), Some("png") | Some("static"));
    let variant = if animated { "globe-anim" } else { "globe" };
    let cache_key = planet_cache_key(seed, uwp, name, deco);
    let cache_object = planet_cache_object(Some(variant), cache_key, deco);

    let uwp_owned = uwp.to_string();
    let name_owned = name.to_string();
    let deco = deco.clone();
    let render = move || {
        if animated {
            generate_globe_apng(
                seed,
                &uwp_owned,
                Some(&name_owned),
                GLOBE_APNG_SIZE,
                ApngTiming {
                    frames: GLOBE_FRAMES,
                    delay_num: GLOBE_DELAY_NUM,
                    delay_den: GLOBE_DELAY_DEN,
                },
                TexSize::HIGH,
                &deco,
            )
        } else {
            generate_globe_png(
                seed,
                &uwp_owned,
                Some(&name_owned),
                GLOBE_PNG_SIZE,
                0.0,
                TexSize::HIGH,
                &deco,
            )
        }
    };

    serve_planet_cached(stream, &gcs, &cache_object, head_only, render).await
}

/// Cache-or-render-then-serve for planet PNG/APNG bytes. Mirrors the flat
/// `/api/world` cache logic (HIT / DISABLED / MISS+upload / BYPASS) but
/// without the flat path's scale-downsample tail, so the globe variants reuse
/// it directly. The render closure is run through [`catch_render`] so a panic
/// in one bad request becomes a clean 500 instead of aborting the process.
/// Globe texture sub-handler for `…&projection=globe&format=texture`. Serves
/// the equirectangular surface texture (RGB surface + alpha emissive) for
/// client-side rendering, cached under `world/v2/globe-tex/`, with the
/// starport's `(lon, lat)` echoed back in an `X-Starport` header (read from the
/// PNG's `Starport` tEXt chunk so it survives a cache hit).
#[allow(clippy::too_many_arguments)]
async fn handle_world_globe_texture(
    stream: &mut TcpStream,
    gcs: Arc<GcsClient>,
    seed: u64,
    uwp: &str,
    name: &str,
    deco: &WorldDecorations,
    head_only: bool,
    clouds: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cache_key = planet_cache_key(seed, uwp, name, deco);
    // Separate namespaces: the two variants are different images for the same
    // world, so they must not share a cache slot.
    let variant = if clouds { "globe-tex" } else { "globe-tex-clear" };
    let cache_object = planet_cache_object(Some(variant), cache_key, deco);

    let uwp_owned = uwp.to_string();
    let name_owned = name.to_string();
    let deco = deco.clone();
    let render = move || {
        generate_globe_texture(
            seed,
            &uwp_owned,
            Some(&name_owned),
            TexSize::HIGH,
            clouds,
            &deco,
        )
    };

    match cache_or_render_bytes(stream, &gcs, &cache_object, render).await? {
        Some((bytes, status)) => {
            let starport = read_starport_chunk(&bytes);
            write_texture(stream, &bytes, head_only, Some(status), starport.as_deref()).await
        }
        None => Ok(()), // an error response was already written
    }
}

/// Cache-or-render a planet image as raw bytes. Tries the GCS cache, and on
/// miss/disabled/error runs `render` (panic-safe via [`catch_render`]); a true
/// miss also fire-and-forget uploads. Returns `Some((bytes, x_cache_status))`
/// ready to serve, or `None` when an error response was already written to the
/// stream (caller should just return). The serialization of the various
/// `/api/world` image responses (PNG, APNG, texture) is left to the caller.
async fn cache_or_render_bytes(
    stream: &mut TcpStream,
    gcs: &Arc<GcsClient>,
    cache_object: &str,
    render: impl Fn() -> Result<Vec<u8>, crate::api::WorldgenError>,
) -> Result<Option<(Vec<u8>, &'static str)>, Box<dyn std::error::Error + Send + Sync>> {
    let (bytes, status, do_upload) = match gcs.get(cache_object).await {
        Ok(Some(b)) => (b, "HIT", false),
        Ok(None) if gcs.is_disabled() => match catch_render(&render) {
            Ok(Ok(b)) => (b, "DISABLED", false),
            Ok(Err(e)) => {
                classify_render_error(stream, e).await?;
                return Ok(None);
            }
            Err(panic_msg) => {
                render_panic_500(stream, "/api/world", &panic_msg).await?;
                return Ok(None);
            }
        },
        Ok(None) => match catch_render(&render) {
            Ok(Ok(b)) => (b, "MISS", true),
            Ok(Err(e)) => {
                classify_render_error(stream, e).await?;
                return Ok(None);
            }
            Err(panic_msg) => {
                render_panic_500(stream, "/api/world", &panic_msg).await?;
                return Ok(None);
            }
        },
        Err(e) => {
            log::warn!("GCS get failed for {cache_object}: {e}; regenerating");
            match catch_render(&render) {
                Ok(Ok(b)) => (b, "BYPASS", false),
                Ok(Err(e)) => {
                    classify_render_error(stream, e).await?;
                    return Ok(None);
                }
                Err(panic_msg) => {
                    render_panic_500(stream, "/api/world", &panic_msg).await?;
                    return Ok(None);
                }
            }
        }
    };

    if do_upload {
        cache_put(gcs, cache_object, bytes.clone()).await;
    }

    Ok(Some((bytes, status)))
}

async fn serve_planet_cached(
    stream: &mut TcpStream,
    gcs: &Arc<GcsClient>,
    cache_object: &str,
    head_only: bool,
    render: impl Fn() -> Result<Vec<u8>, crate::api::WorldgenError>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match cache_or_render_bytes(stream, gcs, cache_object, render).await? {
        Some((bytes, status)) => write_png(stream, &bytes, head_only, Some(status)).await,
        None => Ok(()),
    }
}

/// Extract the `Starport` tEXt chunk's "lon,lat" payload from a globe-texture
/// PNG, if present. Only decodes the PNG header/metadata, not the pixels.
fn read_starport_chunk(png_bytes: &[u8]) -> Option<String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let reader = decoder.read_info().ok()?;
    reader
        .info()
        .uncompressed_latin1_text
        .iter()
        .find(|c| c.keyword == "Starport")
        .map(|c| c.text.clone())
}

/// Map a `WorldgenError` from the planet generator into the right HTTP
/// status. The library has three error variants but only two of them
/// are reachable from this code path — we don't pass constraints, so
/// `Constraints` is impossible; `Map(MapError)` is the bad-UWP case
/// (→ 422); `Render(_)` is everything else (→ 500).
async fn classify_render_error(
    stream: &mut TcpStream,
    e: crate::api::WorldgenError,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::api::WorldgenError::*;
    match e {
        Map(m) => write_simple(stream, 422, "Unprocessable Entity", &format!("{m:?}")).await,
        Constraints(_) | Render(_) => {
            write_simple(stream, 500, "Internal Server Error", &format!("{e}")).await
        }
    }
}

/// Compute the SipHash-2-4 cache key for a planet render. The key is
/// derived purely from the inputs that determine the canonical-scale
/// PNG bytes — not from `scale` (the bucket only ever stores the
/// canonical render).
///
/// Empty decorations hash exactly as they did before decorations existed
/// — nothing is written for them — so every undecorated world keeps the
/// cache slot it already has. A non-empty value appends its canonical
/// query encoding, which is one spelling per value, so `tl` and
/// `tl:0:10` (different substellar points, different maps) never share a
/// slot and no two spellings of one value can split it.
fn planet_cache_key(seed: u64, uwp: &str, name: &str, deco: &WorldDecorations) -> u64 {
    let mut h = SipHasher24::new_with_keys(CACHE_SIP_KEY_0, CACHE_SIP_KEY_1);
    h.write(b"world_v1\0");
    h.write_u64(seed);
    h.write(uwp.trim().to_ascii_uppercase().as_bytes());
    h.write_u8(0);
    h.write(name.trim().to_lowercase().as_bytes());
    if !deco.is_empty() {
        h.write(b"\0deco\0");
        h.write(deco.to_query().as_bytes());
    }
    h.finish()
}

/// GCS object path for a cached planet render: `variant` is `None` for the
/// flat map, else the globe variant's segment (`globe`, `globe-anim`,
/// `globe-tex`, `globe-tex-clear`). Decorated worlds get the
/// [`DECO_CACHE_VERSION`] segment after the variant; undecorated ones keep
/// exactly the path they had before decorations existed.
fn planet_cache_object(variant: Option<&str>, key: u64, deco: &WorldDecorations) -> String {
    let variant = variant.map_or(String::new(), |v| format!("/{v}"));
    let deco_segment = if deco.is_empty() {
        String::new()
    } else {
        format!("/{DECO_CACHE_VERSION}")
    };
    format!("{PLANET_CACHE_PREFIX}{variant}{deco_segment}/{key:016x}.png")
}

/// The decorations a `/api/world` request renders with.
///
/// An explicit `deco` param wins — including `deco=none`, which forces an
/// undecorated render even for a world the overrides lock. A missing or
/// blank param falls back to what `data/overrides.json` states for the
/// world `name` at `(sector, hex)`, and to no decorations when it states
/// nothing.
///
/// That fallback is the whole answer: nothing in worldgen infers a tidal
/// lock from the star and orbit (see `systems::astro::auto_tide_locked` for
/// why), so a lock the overrides don't state only happens if the client
/// passes `deco=tl` itself.
///
/// A malformed value is an error, never ignored — a typo that quietly
/// rendered the undecorated map would also cache it.
fn resolve_decorations(
    params: &HashMap<String, String>,
    sector: &str,
    hex: &str,
    name: &str,
) -> Result<WorldDecorations, DecorationError> {
    decorations_or_else(params, || overrides::decorations_for(sector, hex, name))
}

/// [`resolve_decorations`] with the override lookup passed in, so tests can
/// exercise the fallback without a locked world in `data/overrides.json`.
fn decorations_or_else(
    params: &HashMap<String, String>,
    stated: impl FnOnce() -> Option<WorldDecorations>,
) -> Result<WorldDecorations, DecorationError> {
    match params.get(WorldDecorations::QUERY_PARAM) {
        Some(q) if !q.trim().is_empty() => WorldDecorations::parse_query(q),
        _ => Ok(stated().unwrap_or_default()),
    }
}

/// Decode a PNG, draw it into a pixmap scaled by `factor`, re-encode.
/// `factor` must be in (0.0, 1.0]; we don't upsample here.
///
/// Uses `tiny_skia` primitives only — no new image-processing crate.
/// Bilinear filtering is good enough for our 2.0 → 1.0 downsample;
/// upgrade to a higher-quality filter later if it matters.
fn downsample_png(bytes: &[u8], factor: f32) -> Result<Vec<u8>, String> {
    let src = tiny_skia::Pixmap::decode_png(bytes).map_err(|e| format!("decode: {e}"))?;
    let target_w = ((src.width() as f32) * factor).round().max(1.0) as u32;
    let target_h = ((src.height() as f32) * factor).round().max(1.0) as u32;
    let mut dst = tiny_skia::Pixmap::new(target_w, target_h)
        .ok_or_else(|| format!("Pixmap::new failed for {target_w}x{target_h}"))?;
    let paint = tiny_skia::PixmapPaint {
        quality: tiny_skia::FilterQuality::Bilinear,
        ..Default::default()
    };
    dst.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &paint,
        tiny_skia::Transform::from_scale(factor, factor),
        None,
    );
    dst.encode_png().map_err(|e| format!("encode: {e}"))
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

// ---------------------------------------------------------------------------
// Response writers
// ---------------------------------------------------------------------------

const CORS_HEADERS: &str = "Access-Control-Allow-Origin: *\r\n\
     Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
     Access-Control-Allow-Headers: *\r\n\
     Access-Control-Expose-Headers: X-Cache, X-Starport\r\n";

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
    x_cache: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let x_cache_header = match x_cache {
        Some(v) => format!("X-Cache: {v}\r\n"),
        None => String::new(),
    };
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: image/png\r\n\
         Content-Length: {len}\r\n\
         Cache-Control: public, max-age=31536000, immutable\r\n\
         Connection: close\r\n\
         {x_cache_header}\
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

/// Write the globe-texture PNG response: same as [`write_png`] plus an
/// optional `X-Starport: lon,lat` header (exposed to cross-origin JS via the
/// `Access-Control-Expose-Headers` in [`CORS_HEADERS`]).
async fn write_texture(
    stream: &mut TcpStream,
    bytes: &[u8],
    head_only: bool,
    x_cache: Option<&str>,
    x_starport: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let x_cache_header = match x_cache {
        Some(v) => format!("X-Cache: {v}\r\n"),
        None => String::new(),
    };
    let starport_header = match x_starport {
        Some(v) => format!("X-Starport: {v}\r\n"),
        None => String::new(),
    };
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: image/png\r\n\
         Content-Length: {len}\r\n\
         Cache-Control: public, max-age=31536000, immutable\r\n\
         Connection: close\r\n\
         {x_cache_header}\
         {starport_header}\
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

/// Write an `image/png` 200 for a *system* render: validated by ETag rather
/// than declared immutable, since curated data can change what it shows.
async fn write_system_png(
    stream: &mut TcpStream,
    bytes: &[u8],
    head_only: bool,
    etag: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: image/png\r\n\
         Content-Length: {len}\r\n\
         ETag: {etag}\r\n\
         {cache}\
         Connection: close\r\n\
         {cors}\
         \r\n",
        len = bytes.len(),
        cache = SYSTEM_CACHE_CONTROL,
        cors = CORS_HEADERS,
    );
    stream.write_all(headers.as_bytes()).await?;
    if !head_only {
        stream.write_all(bytes).await?;
    }
    stream.shutdown().await.ok();
    Ok(())
}

/// Fingerprint of everything a system render depends on besides the query
/// string.
///
/// The system endpoints used to advertise `immutable` on the grounds that
/// their output was a pure function of the query parameters. Overrides ended
/// that: the same URL legitimately renders differently once curated data for
/// that system ships, and `immutable` tells every browser never to revalidate
/// — not even on a hard reload, which is precisely what it is for. The result
/// was a year-long window in which a change could not reach anyone who had
/// already looked.
///
/// Mixing the override file and a generator version into the ETag makes the
/// response identify what produced it. Deploying new curated data changes
/// every affected ETag, so clients revalidate once and pick it up.
///
/// Bump `SYSTEM_RENDER_VERSION` when a change alters what a system looks like
/// without changing the override file — a placement rule, the renderer.
const SYSTEM_RENDER_VERSION: u32 = 5;

fn system_etag(query: &str) -> String {
    let mut h = SipHasher24::new_with_keys(CACHE_SIP_KEY_0, CACHE_SIP_KEY_1);
    h.write(b"system_etag\0");
    h.write_u32(SYSTEM_RENDER_VERSION);
    h.write(query.as_bytes());
    h.write_u8(0);
    // The curated data itself: its content decides the output just as much as
    // the query does.
    h.write(crate::systems::overrides::fingerprint().as_bytes());
    format!("\"{:016x}\"", h.finish())
}

/// True when the client already holds this exact response.
///
/// `If-None-Match` is a comma-separated list and may be `*`.
fn etag_matches(if_none_match: Option<&str>, etag: &str) -> bool {
    let Some(header) = if_none_match else {
        return false;
    };
    header.split(',').any(|candidate| {
        let c = candidate.trim();
        c == "*" || c == etag || c.strip_prefix("W/").is_some_and(|w| w == etag)
    })
}

/// Write a `304 Not Modified`. No body, by definition.
async fn write_not_modified(
    stream: &mut TcpStream,
    etag: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let headers = format!(
        "HTTP/1.1 304 Not Modified\r\n\
         ETag: {etag}\r\n\
         {cache}\
         Connection: close\r\n\
         {cors}\
         \r\n",
        cache = SYSTEM_CACHE_CONTROL,
        cors = CORS_HEADERS,
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.shutdown().await.ok();
    Ok(())
}

/// Cache policy for the system endpoints.
///
/// Deliberately not `immutable`: see [`system_etag`]. An hour of freshness
/// keeps repeat views off the network entirely, and after that a conditional
/// request costs a 304 — which, because the ETag comes from the inputs, is
/// answered without rendering the system at all. Cheaper than today's cache
/// hit was, not more expensive.
const SYSTEM_CACHE_CONTROL: &str = "Cache-Control: public, max-age=3600, must-revalidate\r\n";

/// Write an `image/svg+xml` 200 response. Mirrors [`write_png`] but with the
/// SVG content type; `charset=utf-8` since the body is text. Same long
/// immutable cache headers — the output is a deterministic function of the
/// query string.
async fn write_svg(
    stream: &mut TcpStream,
    bytes: &[u8],
    head_only: bool,
    etag: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: image/svg+xml; charset=utf-8\r\n\
         Content-Length: {len}\r\n\
         ETag: {etag}\r\n\
         {cache}\
         Connection: close\r\n\
         {cors}\
         \r\n",
        len = bytes.len(),
        cache = SYSTEM_CACHE_CONTROL,
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
    fn catch_render_returns_value_on_success() {
        assert_eq!(catch_render(|| 1 + 2).unwrap(), 3);
    }

    #[test]
    fn catch_render_traps_panic_into_err() {
        // A render that panics (e.g. an out-of-bounds table lookup) must
        // come back as Err with the message instead of unwinding past the
        // boundary. Requires the test/dev profile's panic = "unwind".
        let got = catch_render(|| -> i32 { panic!("boom: index out of bounds") });
        let msg = got.expect_err("panic must be trapped");
        assert!(msg.contains("boom"), "panic message preserved, got: {msg}");
        // The out-of-bounds index panic from a real array also traps.
        let arr = [0_i32; 3];
        let idx = 9usize;
        let oob = catch_render(|| arr[idx]);
        assert!(oob.is_err(), "array OOB panic must be trapped");
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
        assert_eq!(crate::api::digit_at("804", 0), Some(8));
        assert_eq!(crate::api::digit_at("804", 1), Some(0));
        assert_eq!(crate::api::digit_at("804", 2), Some(4));
        assert_eq!(crate::api::digit_at("804", 3), None);
        // Non-digit char yields None.
        assert_eq!(crate::api::digit_at("8X4", 1), None);
    }

    /// A tide-locked value for the cache-key tests.
    fn locked(substellar: Option<(f64, f64)>) -> WorldDecorations {
        WorldDecorations::tide_locked(crate::decorations::TideLock {
            substellar: substellar
                .map(|(lat, lon)| crate::decorations::LatLon::from_degrees(lat, lon).unwrap()),
        })
    }

    /// The Noricum request the smoke tests use, through the real seed chain.
    fn noricum_seed() -> u64 {
        planet_seed(system_seed("Trojan Reach", 20, 18), 3, "Noricum")
    }

    #[test]
    fn undecorated_cache_key_and_paths_are_unchanged() {
        // Pinned from `planet_cache_key(seed, uwp, name)` as it was before
        // decorations existed. If this moves, every world already in the
        // bucket is orphaned and re-renders on its next view.
        let key = planet_cache_key(noricum_seed(), "D8867BB-1", "Noricum", &Default::default());
        assert_eq!(key, 0x44c4_61aa_baca_9b93);

        let none = WorldDecorations::default();
        assert_eq!(planet_cache_object(None, key, &none), "world/v2/44c461aabaca9b93.png");
        for variant in ["globe", "globe-anim", "globe-tex", "globe-tex-clear"] {
            assert_eq!(
                planet_cache_object(Some(variant), key, &none),
                format!("world/v2/{variant}/44c461aabaca9b93.png"),
            );
        }
    }

    #[test]
    fn decorated_cache_keys_are_distinct() {
        let seed = noricum_seed();
        let key = |d: &WorldDecorations| planet_cache_key(seed, "D8867BB-1", "Noricum", d);
        let plain = key(&WorldDecorations::default());
        let tl = key(&locked(None));
        let tl_0_10 = key(&locked(Some((0.0, 10.0))));
        assert_ne!(plain, tl);
        assert_ne!(plain, tl_0_10);
        assert_ne!(tl, tl_0_10, "different substellar points are different maps");
        // One value, one slot, however it was spelled on the way in.
        assert_eq!(tl_0_10, key(&WorldDecorations::parse_query("tl:0.00:370").unwrap()));
    }

    #[test]
    fn decorated_paths_sit_under_the_deco_namespace() {
        let d = locked(None);
        assert_eq!(planet_cache_object(None, 0xab, &d), "world/v2/deco-v1/00000000000000ab.png");
        assert_eq!(
            planet_cache_object(Some("globe-tex"), 0xab, &d),
            "world/v2/globe-tex/deco-v1/00000000000000ab.png",
        );
    }

    #[test]
    fn deco_param_parses_and_rejects_unknown_tokens() {
        let never = || -> Option<WorldDecorations> { panic!("override consulted") };
        let q = |s: &str| parse_query(s);
        assert_eq!(decorations_or_else(&q("deco=tl"), never).unwrap(), locked(None));
        assert_eq!(
            decorations_or_else(&q("deco=tl:12.5:270"), never).unwrap(),
            locked(Some((12.5, 270.0))),
        );
        assert!(decorations_or_else(&q("deco=bogus"), never).is_err());
        assert!(decorations_or_else(&q("deco=tl,tl"), never).is_err());
    }

    #[test]
    fn missing_deco_falls_back_to_the_override() {
        let stated = || Some(locked(None));
        // Absent or blank → whatever the override states.
        assert_eq!(decorations_or_else(&parse_query("uwp=x"), stated).unwrap(), locked(None));
        assert_eq!(decorations_or_else(&parse_query("deco="), stated).unwrap(), locked(None));
        // `none` is an explicit answer and beats the override.
        assert!(decorations_or_else(&parse_query("deco=none"), stated).unwrap().is_empty());
        // No override statement → undecorated.
        assert!(decorations_or_else(&parse_query(""), || None).unwrap().is_empty());
        // And the real lookup: a world with no override says nothing.
        assert!(
            resolve_decorations(&parse_query(""), "Nowhere", "0101", "Anything")
                .unwrap()
                .is_empty()
        );
    }
}
