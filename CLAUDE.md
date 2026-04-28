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

# Tests (CI runs `cargo build` + `cargo test`)
cargo test
cargo test <test_name>           # single test
cargo test --lib simulator::     # all simulator unit tests

# Lint / format
cargo clippy
cargo fmt   # uses leptosfmt via .vscode/settings.json — prefer `leptosfmt --rustfmt` if formatting view! macros
```

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
- `src/trade/` — Trade rules: `TradeClass`, `PortCode`, `ZoneClassification`, UWP→trade-class derivation (`upp_to_trade_classes`), `available_goods`, `available_passengers`, `ship_manifest`, and the master `table` of trade goods.
- `src/components/` — Leptos components. `selector` is the landing page; `system_generator` (`World` component) and `trade_computer` (`Trade` component) are the two tool screens; `system_view`, `world_list`, `traveller_map` are sub-views.
- `src/comms/` — WebSocket client (`Client`) and the shared `TradeState`. Compiles for both wasm and native.
- `src/backend/` — Native-only: `server` (tokio TcpListener + per-client mpsc), `firestore` (persistence; `FIRESTORE_DATABASE_ID=debug` runs without Firestore). Gated by `#[cfg(feature = "backend")]`.
- `src/logging.rs` — Reads `?log=<level>&module=<prefix>` from the URL to configure `wasm_logger` at startup. Useful for debugging deployed builds without recompiling.
- `src/util.rs` — Shared helpers (e.g., `calculate_hex_distance` for galactic-hex coords used by both client and server).

### State management (frontend)

Components use Leptos signals and `reactive_stores` for nested state. The trade computer's authoritative state lives on the server; the client renders whatever `TradeState` the WebSocket pushes.

## Deployment

Single Docker image runs both nginx (serving `dist/`) and the trade server, supervised by supervisord (`supervisord.conf`). nginx proxies `/ws/trade` → `127.0.0.1:8081`. Build is multi-stage with `cargo-chef` for caching and produces a musl-static server binary so it runs on `nginx:1.27-alpine`. `push_image.sh` builds for `linux/amd64` and deploys to Cloud Run.

Server env vars (see `src/bin/server.rs`):
- `GOOGLE_APPLICATION_CREDENTIALS`, `GCP_PROJECT`, `FIRESTORE_DATABASE_ID` (`"debug"` to skip Firestore)
- `WS_PORT` (default 8081), `WS_HOST` (default `0.0.0.0`)
- `RUST_LOG`

## Conventions

- UWP indices have named constants in `src/trade/mod.rs` (`UPP_SIZE`, `UPP_ATMOSPHERE`, …). Use them rather than magic indices when parsing UWPs.
- `World` objects are *generated by the server* in the trade flow — clients never construct them for round-trip. Only `name`, `uwp`, `coords`, `zone` go up; the populated `World` comes back down.
- New backend-only deps must be added as `optional = true` in `Cargo.toml` and listed in the `backend` feature, otherwise wasm builds will break.
