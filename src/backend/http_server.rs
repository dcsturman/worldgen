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

use siphasher::sip::SipHasher24;
use std::hash::Hasher;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::api::{
    build_constraints, generate_globe_apng, generate_globe_png, generate_globe_texture,
    generate_planet_png_scaled, generate_system_png_scaled, generate_system_svg, parse_stellar,
};
use crate::backend::gcs::GcsClient;
use crate::worldmap::{ApngTiming, TexSize};
use crate::seed::{planet_seed, system_seed};
use crate::systems::constraint::SystemConstraints;

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
/// The cache key is `(seed, uwp, name)` — pure world *identity*, with nothing
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

/// Globe (orthographic projection) render parameters for `?projection=globe`.
/// Fixed server-side so the cache key stays `(seed, uwp, name)` per variant
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
        "/api/system" => handle_system(reader.get_mut(), query, head_only).await,
        "/api/system_svg" => handle_system_svg(reader.get_mut(), query, head_only).await,
        "/api/world" => handle_world(reader.get_mut(), query, head_only, gcs).await,
        _ => write_simple(reader.get_mut(), 404, "Not Found", "Unknown endpoint").await,
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

    // `hex` is a 4-character "CCRR" sub-sector hex location. We treat each
    // pair as a u8 — Traveller hexes run up to 32×40 per sector, within u8.
    let (hex_x, hex_y) = parse_hex_quad(hex).ok_or((
        400,
        "Bad Request",
        "hex must be a 4-digit string like \"2018\"".to_string(),
    ))?;

    let pbg = params.get("pbg").cloned().unwrap_or_default();
    let belts = digit_at(&pbg, 1).unwrap_or(0) as usize;
    let giants = digit_at(&pbg, 2).unwrap_or(0) as usize;

    let stellar = params.get("stellar").map(|s| s.as_str()).unwrap_or("");
    let stars = parse_stellar(stellar);

    // `worlds` is the system's `W` digit (total body count) from Traveller
    // Map. We back out the main world, the belts and the gas giants to leave
    // just the extra rocky planets the caller wants placed. The generator
    // grows its orbit list to fit every requested body, so clamp this
    // untrusted query param to a generous ceiling — real Traveller systems
    // top out around two dozen bodies — to keep a bogus `worlds=99999` from
    // allocating a giant orbit vector and generating that many worlds.
    const MAX_WORLDS: i32 = 64;
    let worlds = params
        .get("worlds")
        .and_then(|s| s.trim().parse::<i32>().ok())
        .map(|w| w.min(MAX_WORLDS));
    let planets = match worlds {
        Some(w) => (w - 1 - belts as i32 - giants as i32).max(0) as usize,
        None => 0,
    };

    let scale = params
        .get("scale")
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(2.0);

    let seed = system_seed(sector, hex_x, hex_y);
    let constraints = build_constraints(name, uwp, &stars, giants, belts, planets)
        .map_err(|e| (422, "Unprocessable Entity", format!("{e}")))?;

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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    write_png(stream, &png, head_only, None).await
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    write_svg(stream, svg.as_bytes(), head_only).await
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
/// generate_planet_png_scaled(seed, uwp, Some(name), CANONICAL_SCALE)
///   └─ worldmap::generate(uwp, seed, name)
///       └─ ChaCha8Rng::seed_from_u64(seed)
/// ```
///
/// `scale` is **not** part of the seed or the cache key — the bucket
/// only ever stores the canonical-scale PNG, and the response is
/// downsampled on-the-fly. `scale > CANONICAL_SCALE` is clamped (we
/// don't upsample).
///
/// Error mapping mirrors `/api/system`: 400 missing param, 422 invalid
/// UWP (from `worldmap::generate` → `MapError`), 500 render failure.
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
        return handle_world_globe(stream, &params, seed, uwp, name, head_only, gcs).await;
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
    let cache_key = planet_cache_key(seed, uwp, name);
    let cache_object = format!("{PLANET_CACHE_PREFIX}/{cache_key:016x}.png");

    // Try cache first. Disabled-mode GCS returns Ok(None) here so the
    // cache_status will be "DISABLED" rather than "HIT".
    let (canonical_bytes, cache_status) = match gcs.get(&cache_object).await {
        Ok(Some(bytes)) => (bytes, "HIT"),
        Ok(None) if gcs.is_disabled() => {
            let bytes =
                match catch_render(|| {
                    generate_planet_png_scaled(seed, uwp, Some(name), PLANET_CANONICAL_SCALE)
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
                    generate_planet_png_scaled(seed, uwp, Some(name), PLANET_CANONICAL_SCALE)
                }) {
                    Ok(Ok(b)) => b,
                    Ok(Err(e)) => return classify_render_error(stream, e).await,
                    Err(panic_msg) => {
                        return render_panic_500(stream, "/api/world", &panic_msg).await;
                    }
                };
            // Fire-and-forget upload so the response ships immediately.
            // A failed upload just means the next request is another
            // cache miss — correctness is preserved, only the next
            // user's latency is affected.
            let gcs2 = gcs.clone();
            let key2 = cache_object.clone();
            let bytes2 = bytes.clone();
            tokio::spawn(async move {
                if let Err(e) = gcs2.put(&key2, bytes2, "image/png").await {
                    log::warn!("GCS put failed for {key2}: {e}");
                }
            });
            (bytes, "MISS")
        }
        Err(e) => {
            log::warn!("GCS get failed for {cache_object}: {e}; regenerating");
            let bytes =
                match catch_render(|| {
                    generate_planet_png_scaled(seed, uwp, Some(name), PLANET_CANONICAL_SCALE)
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
/// ([`GLOBE_PNG_SIZE`] etc.) so the cache key stays `(seed, uwp, name)` per
/// variant.
async fn handle_world_globe(
    stream: &mut TcpStream,
    params: &HashMap<String, String>,
    seed: u64,
    uwp: &str,
    name: &str,
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
        return handle_world_globe_texture(stream, gcs, seed, uwp, name, head_only, clouds).await;
    }

    // Animated by default; `format=png`/`static` asks for a single frame.
    let animated = !matches!(format.as_deref(), Some("png") | Some("static"));
    let variant = if animated { "globe-anim" } else { "globe" };
    let cache_key = planet_cache_key(seed, uwp, name);
    let cache_object = format!("{PLANET_CACHE_PREFIX}/{variant}/{cache_key:016x}.png");

    let uwp_owned = uwp.to_string();
    let name_owned = name.to_string();
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
            )
        } else {
            generate_globe_png(
                seed,
                &uwp_owned,
                Some(&name_owned),
                GLOBE_PNG_SIZE,
                0.0,
                TexSize::HIGH,
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
async fn handle_world_globe_texture(
    stream: &mut TcpStream,
    gcs: Arc<GcsClient>,
    seed: u64,
    uwp: &str,
    name: &str,
    head_only: bool,
    clouds: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cache_key = planet_cache_key(seed, uwp, name);
    // Separate namespaces: the two variants are different images for the same
    // world, so they must not share a cache slot.
    let variant = if clouds { "globe-tex" } else { "globe-tex-clear" };
    let cache_object = format!("{PLANET_CACHE_PREFIX}/{variant}/{cache_key:016x}.png");

    let uwp_owned = uwp.to_string();
    let name_owned = name.to_string();
    let render =
        move || generate_globe_texture(seed, &uwp_owned, Some(&name_owned), TexSize::HIGH, clouds);

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
        let gcs2 = gcs.clone();
        let key2 = cache_object.to_string();
        let bytes2 = bytes.clone();
        tokio::spawn(async move {
            if let Err(e) = gcs2.put(&key2, bytes2, "image/png").await {
                log::warn!("GCS put failed for {key2}: {e}");
            }
        });
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
fn planet_cache_key(seed: u64, uwp: &str, name: &str) -> u64 {
    let mut h = SipHasher24::new_with_keys(CACHE_SIP_KEY_0, CACHE_SIP_KEY_1);
    h.write(b"world_v1\0");
    h.write_u64(seed);
    h.write(uwp.trim().to_ascii_uppercase().as_bytes());
    h.write_u8(0);
    h.write(name.trim().to_lowercase().as_bytes());
    h.finish()
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

/// Write an `image/svg+xml` 200 response. Mirrors [`write_png`] but with the
/// SVG content type; `charset=utf-8` since the body is text. Same long
/// immutable cache headers — the output is a deterministic function of the
/// query string.
async fn write_svg(
    stream: &mut TcpStream,
    bytes: &[u8],
    head_only: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let headers = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: image/svg+xml; charset=utf-8\r\n\
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
        assert_eq!(digit_at("804", 0), Some(8));
        assert_eq!(digit_at("804", 1), Some(0));
        assert_eq!(digit_at("804", 2), Some(4));
        assert_eq!(digit_at("804", 3), None);
        // Non-digit char yields None.
        assert_eq!(digit_at("8X4", 1), None);
    }
}
