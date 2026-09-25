# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Worldgen is a set of tools for the Traveller RPG, built in Rust. It produces a WASM web frontend (Leptos) and a native WebSocket backend (tokio + tungstenite + Firestore) from a single crate. The same `comms::TradeState` type is shared between the WASM client and native server, which is why the crate is split with feature flags rather than into a workspace.

## Build / Run

The crate has two distinct compile targets that share most code:

- **Frontend (WASM)** — built with Trunk; default features.
- **Backend (native)** — built with Cargo; gated by `--features backend` (pulls in tokio, firestore, rustls, etc.). Backend code lives under `src/backend/` behind `#[cfg(feature = "backend")]` in `lib.rs`.

**Local dev — preferred path is the two scripts:**

```bash
./scripts/run-backend.sh    # WebSocket server on :8081 (Firestore=debug by default)
./scripts/run-frontend.sh   # Trunk dev server on :8080 with --features local-dev
```

Run them in separate terminals. The frontend's `local-dev` feature points its WebSocket clients at `localhost:8081` directly, bypassing nginx. Override env vars by prefixing: `RUST_LOG=trace ./scripts/run-backend.sh`. Set `SENTRY_DSN=...` to enable error reporting.

**Other targets and one-offs:**

```bash
# Frontend production build (outputs to dist/)
trunk build --release

# Run the simulator's live smoke test against TravellerMap (network-bound, ~20s)
cargo test --features backend --lib -- --ignored simulator_smoke_regina --nocapture

# Smoke-test the deployed /system HTTP endpoint at tools.callistoflight.com.
# External consumers (Traveller Map client) depend on this URL; run this
# before pushing any change that could affect the public surface
# (worldgen library API, sysmap renderer, backend http_server, or nginx.conf).
# Override the target with `WORLDGEN_BASE_URL=https://staging…` for staging.
cargo test --features backend --test production_smoke -- --ignored --nocapture

# Tests (CI runs `cargo build` + `cargo test`)
cargo test
cargo test <test_name>           # single test
cargo test --lib simulator::     # all simulator unit tests

# Lint / format
cargo clippy
cargo fmt   # uses leptosfmt via .vscode/settings.json — prefer `leptosfmt --rustfmt` if formatting view! macros
```

### Test conventions: why some test commands need extra flags

Routine `cargo test` is fast and hermetic; the awkward-looking flags on the
two smoke commands above are how the repo keeps it that way.

- **`--features backend`** — the `backend` feature pulls in tokio, tonic,
  firestore, reqwest, gcloud-sdk, hyper-rustls, sentry, etc. (~40 s cold
  compile, many MB of dep graph, some of which doesn't build on wasm).
  Default features are `["frontend"]` so the dev loop and `trunk build`
  stay quick and WASM-compatible. Any test that needs backend code
  (the smoke tests, anything `#[cfg(feature = "backend")]`) must opt in.

- **`#[ignore]` + `-- --ignored`** — Rust's idiomatic marker for tests
  that have side effects, hit external networks, or take too long for
  routine runs. Used in this repo for:
  - `simulator_smoke_regina` — hits travellermap.com (network-bound, ~20 s)
  - `worldmap::tests::*` PNG-dump tests — write files to `/tmp/`
  - `tests/production_smoke.rs` — hits live `tools.callistoflight.com`

  The harness still **compiles** ignored tests (so they can't bit-rot
  silently), but skips execution unless you pass `-- --ignored`. Anything
  after `--` is forwarded to the test binary itself, not Cargo. You can
  also filter further with `-- --ignored <name_substring>` to run just
  one ignored test.

- **`--test <name>`** — runs only one integration-test binary
  (`tests/<name>.rs`). Without it, `cargo test --features backend --
  --ignored` would also run every other ignored test in the repo
  (simulator regina, worldmap dumps, etc.). Useful for cycle time and
  for not polluting `/tmp/` when you're only after the production check.

So the production-smoke invocation answers four independent questions:

```
cargo test --features backend --test production_smoke -- --ignored --nocapture
            ^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^^   ^^^^^^^^^  ^^^^^^^^^^
            compile backend?   which test binary?        run ignored? print stdout?
```

Drop `--features backend` → file doesn't compile (cfg-gated).
Drop `--test production_smoke` → also runs every other ignored test.
Drop `-- --ignored` → tests are listed as ignored but don't execute.
Drop `--nocapture` → still works, but you lose live progress output.

### Why debug builds optimize dependencies

`Cargo.toml` sets `[profile.dev.package."*"] opt-level = 3`. The test suite
spends nearly all its time inside `noise` evaluating simplex fBm — a planet
render is millions of texels at tens of noise evaluations each, and the
tectonic domain warp multiplies that again. Unoptimized, one
`planet_png_scaled_*` test took over a minute and CI ran for thirteen.

Optimizing *only* dependencies keeps our own code compiled the way `cargo
test` normally compiles it — full debug info, real line numbers in panics,
assertions intact — while the hot numeric library underneath runs at release
speed. That plus a cargo cache in CI took the pipeline from 13m06s to 2m28s
without dropping a single test. If it ever needs to get faster again, split
the render-heavy `tests/library_api.rs` into its own parallel job before
reaching for `#[ignore]`; coverage is the expensive thing to buy back.

The wasm target requires `rustup target add wasm32-unknown-unknown` and `cargo install trunk`. `Trunk.toml` sets `getrandom_backend="wasm_js"` via rustflags — needed because `getrandom` 0.3 requires explicit backend selection on wasm.

## Binaries

Defined in `Cargo.toml`:

- `main` (`src/bin/main.rs`) — the deployed WASM entry point. Path-based routing in one binary: `/world` → system generator, `/trade` → trade computer, `/` → selector.
- `world`, `trade` (`src/bin/{world,trade}.rs`) — standalone WASM entry points for each tool. Not used by the deployed app but kept for separable deployment.
- `server` (`src/bin/server.rs`) — native WebSocket server. Requires `--features backend`.

Trunk only builds `main` (see `<link data-trunk rel="rust" data-bin="main" />` in `index.html`).

## Architecture

### Frontend ↔ Backend split

The trade computer is a multi-client synchronized app. The server is **authoritative** for trade-table generation and pricing — clients send a partial `TradeState` (world names, UWPs, coords, zones, broker/steward skills), the server fills in the generated `World` objects and `AvailableGoodsTable`, and broadcasts the full state back to all connected clients. This is why `comms::TradeState` lives outside `backend/` — both sides serialize it.

Wire format is `comms::ServerMessage`, an `untagged` serde enum of either a `TradeState` (state update) or a `ServerCommand` (e.g., `Regenerate`).

The WebSocket URL is constructed at runtime from `window.location`:
- Production / Docker: same host, path `/ws/trade` (nginx proxies to `127.0.0.1:8081`).
- `local-dev` feature: hardcoded to `ws://localhost:8081/ws/trade` when the host starts with `localhost`.

### Crate layout

- `src/systems/` — Traveller system generation: stars, worlds, gas giants, satellites, name tables, lookup tables. Entry point is `systems::system::System`. `world::World` carries the UWP and is the unit shared across modules.
- `src/worldmap/` — Per-planet surface maps from a UWP + seed. Terrain is sampled on a 3D unit sphere (icosahedral grid + simplex-fBm elevation, climate, biome — see `grid::xy_to_sphere`) and rendered two ways: the **flat** equirectangular SVG/PNG (`render`, `raster::RasterJob`) and a **globe** (`globe`) that orthographically warps a once-built equirectangular texture onto a spinning sphere. The globe ships as a static PNG or a looping animated PNG (APNG) via `render_globe_png` / `render_globe_apng`; the frontend animates live by re-warping the texture on a `<canvas>` each frame. Both projections are deterministic from `(uwp, seed)`.
  Every climate decision (temperature, water, humidity, rain-shadow wind, cloud bands, settlement placement) goes through `climate::ClimateModel`, stored on `WorldMap`, so per-hex biomes and per-pixel flat/globe colour share one rule. `Rotating` is the latitude model and must stay **byte-identical** — `worldmap::golden` pins flat/globe hashes computed before the model existed; if a pin moves, a rotating world changed and every cached one is stale. `TideLocked` replaces latitude with θ, the angle from the substellar point: temperature is SPEC's `cos(θ)^¼` insolation curve blended with an atmospheric-transport term and softened over a twilight band, with `t_sub`/`t_night` set so the world's mean matches the rotating model's (spread by atmosphere code); water fills an antistellar ice sheet first (up to 15% of the surface) and only the excess is liquid, pooled at the inner terminator edge via a θ-biased *water potential* — on a locked world `sea_level` thresholds that potential, not raw elevation; settlements are confined to θ 60–120°, weighted to 75–105°. Resolving a lock draws nothing from the map RNG, so the terrain is the unlocked world's. Known limitation: the globe's fixed-direction lighting spins with the planet, so a locked world's lit hemisphere isn't its substellar one in the APNG/static globe (WebGL clients light the texture themselves).
- `src/trade/` — Trade rules: `TradeClass`, `PortCode`, `ZoneClassification`, UWP→trade-class derivation (`upp_to_trade_classes`), `available_goods`, `available_passengers`, `ship_manifest`, and the master `table` of trade goods.
- `src/components/` — Leptos components. `selector` is the landing page; `system_generator` (`World` component) and `trade_computer` (`Trade` component) are the two tool screens; `system_view`, `world_list`, `traveller_map` are sub-views.
- `src/comms/` — WebSocket client (`Client`) and the shared `TradeState`. Compiles for both wasm and native.
- `src/backend/` — Native-only: `server` (tokio TcpListener + per-client mpsc), `firestore` (persistence; `FIRESTORE_DATABASE_ID=debug` runs without Firestore). Gated by `#[cfg(feature = "backend")]`.
- `src/logging.rs` — Reads `?log=<level>&module=<prefix>` from the URL to configure `wasm_logger` at startup. Useful for debugging deployed builds without recompiling.
- `src/util.rs` — Shared helpers (e.g., `calculate_hex_distance` for galactic-hex coords used by both client and server).

### State management (frontend)

Components use Leptos signals and `reactive_stores` for nested state. The trade computer's authoritative state lives on the server; the client renders whatever `TradeState` the WebSocket pushes.

## Deployment

Single Docker image runs both nginx (serving `dist/`) and the trade server, supervised by supervisord (`supervisord.conf`). nginx proxies `/ws/trade` → `127.0.0.1:8081`. The build is multi-stage (a WASM-frontend stage and a musl-static-server stage), each compiling with BuildKit `--mount=type=cache` over its cargo `target/` so a source-only change recompiles just the changed crate, not the whole dep tree. A `.dockerignore` keeps the build context to the manifests/source/assets (without it, the multi-GB `target/` ships to the daemon on every build and pressures the cache mounts into eviction → full recompile). `push_image.sh` builds for `linux/amd64` and deploys to Cloud Run.

The cache **mounts** persist only in the local BuildKit daemon — they are not exported by `--cache-to=type=registry`, so a fresh CI runner (or a machine after a build-cache GC) recompiles every dependency. If cold-build speed (or registry-portable dep caching) ever matters, reintroduce `cargo-chef`: a `chef cook --recipe recipe.json` layer compiles only the dependencies and *is* an exportable layer, keyed on `Cargo.toml`/`Cargo.lock` rather than source. It was present historically and removed in `76ff721`.

Server env vars (see `src/bin/server.rs`):
- `GOOGLE_APPLICATION_CREDENTIALS`, `GCP_PROJECT`, `FIRESTORE_DATABASE_ID` (`"debug"` to skip Firestore)
- `GCS_BUCKET` (cache for `/api/world` planet renders; `"debug"` or unset → no cache, every request regenerates). The endpoint takes `projection=flat` (default — the equirectangular PNG every existing consumer already gets) or `projection=globe` (orthographic). For `projection=globe`, `format` selects the output:
  - `apng` (default) — a spinning APNG. Smooth but heavy; **prefer `texture` for web clients** (the APNG's discrete frames look jerky vs. a live warp).
  - `png` — a single static globe frame.
  - `texture` — the raw equirectangular surface texture as a 2048×1024 RGBA PNG (RGB = day surface, **alpha = night-side city-light emissive**), for client-side (e.g. WebGL) globe rendering. Size is `worldmap::TexSize::HIGH`; the in-browser path uses `TexSize::STANDARD` (1024×512) instead, since it builds the texture in WASM while the user waits and its canvas can't resolve more. Raising it costs payload — roughly 420 KB → 1.4–1.8 MB per world on a cache miss — so `TexSize` is explicit at each call site rather than a single global constant. A cloud deck is composited into the surface RGB by default, with coverage derived from the UWP's atmosphere and hydrographics (atmosphere 0–1 gets none at all) — `clouds=0` opts out and caches under its own `globe-tex-clear/` namespace. It has to be baked in rather than shipped as a separate layer, since the consumer gets one texture, which is why the opt-out exists. The starport's `(lon, lat)` in radians is returned in an **`X-Starport`** response header (and embedded as a `Starport` tEXt chunk so it survives a cache hit); absent for class X/Y / unpopulated worlds. CORS exposes `X-Cache, X-Starport` so cross-origin JS can read them. The reference warp is `worldmap::globe::GlobeTexture::warp_into`.
  Each variant caches under its own path (`world/v2/`, `world/v2/globe/`, `world/v2/globe-anim/`, `world/v2/globe-tex/`) so they never collide. The version segment is bumped whenever a worldgen change alters what a world looks like — the cache key is `(seed, uwp, name)` with nothing about the generator in it, so without a bump previously-viewed worlds keep serving their old terrain forever while unviewed ones render with the new.
  **Decorations** (`src/decorations.rs`, e.g. a tidal lock) ride on an optional `deco` param: `deco=tl`, `deco=tl:LAT:LON`, or `deco=none`. An unknown token is a **400**, never ignored — a typo would otherwise render and cache the unlocked world. With no `deco` param the server falls back to what `data/overrides.json` states for `name` at `(sector, hex)`; `deco=none` overrides that. Nothing infers a lock from the star and orbit — only the override file locks a world, because TravellerMap's UWPs were never generated with tidal locking in mind (see `systems::astro::auto_tide_locked`) — so a client that wants a lock the file doesn't state passes `deco=tl` itself. Empty decorations leave the cache key and path **exactly** as they were (pinned by `undecorated_cache_key_and_paths_are_unchanged`); a decorated render adds its canonical `deco` string to the key and a `deco-v1/` segment after the variant (`world/v2/deco-v1/…`, `world/v2/globe-tex/deco-v1/…`). Bump `DECO_CACHE_VERSION` when the decorated climate model changes output — it invalidates only decorated worlds, where bumping `v2` would re-render all of them.
- `WS_PORT` (default 8081), `WS_HOST` (default `0.0.0.0`)
- `RUST_LOG`
- `WORLDGEN_RENDER_THREADS` (default: one per available core) — workers the
  globe texture build splits across. Native only; the in-browser path is
  single-threaded by construction (`render_threads` is hardcoded to 1 on
  wasm) and renders a quarter of the texels. Set it to `1` to reproduce
  single-vCPU behaviour locally. Output is byte-identical at any worker
  count, asserted by `texture_is_independent_of_worker_count` — which is the
  whole safety argument for the threading, since a lost race here would show
  up not as a crash but as a planet that looks different depending on which
  machine rendered it, then cached in GCS for whichever version won.

### Cloud Run sizing is a renderer parameter, not just capacity

`push_image.sh` deploys with `--cpu 4 --memory 2Gi --max-instances 50`, and
all three are load-bearing:

- **`--cpu 4`** — the texture build parallelizes across cores, so vCPU count
  sets render latency. A cold 2048×1024 globe is ~24 s at 1 vCPU and ~11 s at
  4. The TravellerMap client tells the user a first render takes "up to ~15
  seconds" while it spins, so this is the difference between that copy being
  true and being a lie. Costs roughly 1.5× per cold render for a 2.7×
  speedup, and only on a cache miss.
- **`--memory 2Gi`** — Cloud Run requires it at 4 vCPU. Not a measured need.
- **`--max-instances 50`** — not a preference. us-central1 allows this
  project 200 total vCPU, and 4 vCPU across the old cap of 100 instances asks
  for 400, so the deploy is *rejected* without it. Raising CPU again means
  lowering this to match, or raising the quota.

These live in the script rather than being set by hand, because a hand-set
value is exactly what a later scripted deploy silently reverts.

`push_image.sh` finishes by comparing the running revision's image digest
before and after, and exits non-zero if it didn't change. `set -e` already
covers a *failed* build — it aborts before `gcloud run deploy` runs at all —
so this covers the other case: a deploy that succeeds while serving the same
bits, which is what a dead buildx builder produced on 2026-07-21.

**Check the exit status directly, not through a pipe.** `./scripts/push_image.sh
| tail -20` reports `tail`'s status, not the script's, so a failed deploy looks
like a successful one. This has caused three false "deployed" reports; the
script itself was correct every time.

### The startup probe must hit `/api/health`, not port 80

The image runs nginx and the render server under supervisord. Cloud Run's
default startup probe is a TCP check on port 80 — which nginx satisfies about
two seconds before the render server binds 8081. In that window the instance
is "ready" and taking traffic, and every request gets nginx's 502 from a
refused upstream connect. This fires on every scale-out, and it is what once
made a smoke test compare a 2.4 MB PNG against a 157-byte error page and
report it as broken determinism.

`/api/health` is dependency-free (no render, no GCS, no Firestore) so it can
only answer once both processes are up:

```bash
gcloud run services update worldgen --region=us-central1 \
  --startup-probe=httpGet.path=/api/health,httpGet.port=80,periodSeconds=3,failureThreshold=20,timeoutSeconds=2
```

`timeoutSeconds` must be strictly less than `periodSeconds` or the update is
rejected. Apply this only *after* an image containing `/api/health` is live,
or the probe fails every instance.

### Cache writes are awaited, deliberately

`/api/world` awaits its GCS upload (10 s timeout) rather than detaching it.
Shipping the response first looks like the obvious optimization and is
backwards on Cloud Run: CPU is throttled to near zero the moment a response
completes, so a detached upload is scheduled exactly when the instance loses
the ability to perform it. Nothing retries, so a failed write leaves that
world uncached indefinitely and every future viewer pays a full render.

### `TRAVELLERMAP_URL` — build-time, baked into both binaries

`TRAVELLERMAP_URL` configures the base URL of the TravellerMap-compatible
service the frontend and the simulator hit (for sector/world lookups,
search, tile rendering). Default is `https://travellermap.com`; override
with e.g. `https://tmap.internal` to point at a self-hosted instance.

It's resolved at **compile time** via `option_env!` in
`src/util.rs::travellermap_base_url` so a single env var set during the
build propagates to both the WASM bundle and the native server binary.
Same value goes everywhere — no per-call URL params, no two-place setup.

- **Local dev (cargo / trunk):** `TRAVELLERMAP_URL=… cargo build` or
  `TRAVELLERMAP_URL=… trunk build`. `./scripts/run-backend.sh` and
  `./scripts/run-frontend.sh` inherit the shell env, so a single
  `export` works for both terminals.
- **Production (Docker / Cloud Run):** `./scripts/push_image.sh`
  forwards the value as a `--build-arg`; the Dockerfile re-exports it
  as `ENV` in both build stages so cargo/trunk see it.

  Resolution order is **environment → `scripts/deploy.env` → the
  script's default (`https://travellermap.com`)**. The checked-in
  default stays public so a fork deploys against the public service;
  a private instance goes in `scripts/deploy.env`, which is gitignored
  and per-machine. Copy `scripts/deploy.env.example` to create it —
  the same pattern the travellermap repo uses. **This deployment's
  `deploy.env` sets `https://travellermap.callistoflight.com`.**

  Getting this wrong is silent. The value is compile-time with no
  runtime override, so an image built against the wrong host behaves
  normally and simply talks to the wrong server — production ran that
  way undetected for some time, and the way to check a deployed build
  is to grep the wasm:

  ```bash
  W=$(curl -s https://tools.callistoflight.com/ | grep -oE 'main-[a-f0-9]+_bg\.wasm' | head -1)
  curl -s "https://tools.callistoflight.com/$W" | strings | grep -oE 'https://[a-zA-Z0-9.-]*travellermap[a-zA-Z0-9.-]*' | sort -u
  ```
- **Re-builds:** `build.rs` declares
  `cargo:rerun-if-env-changed=TRAVELLERMAP_URL` so changing the value
  between builds correctly invalidates the cargo cache. Without this,
  cargo would silently reuse the binary it built with the old URL.

## Conventions

- UWP indices have named constants in `src/trade/mod.rs` (`UPP_SIZE`, `UPP_ATMOSPHERE`, …). Use them rather than magic indices when parsing UWPs.
- `World` objects are *generated by the server* in the trade flow — clients never construct them for round-trip. Only `name`, `uwp`, `coords`, `zone` go up; the populated `World` comes back down.
- New backend-only deps must be added as `optional = true` in `Cargo.toml` and listed in the `backend` feature, otherwise wasm builds will break.
