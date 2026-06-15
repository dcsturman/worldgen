# Worldgen library integration

You're depending on the `worldgen` crate (the system + planet generator built
for the Traveller RPG) to render deterministic system and planet maps from
stable identifiers. This doc is the minimum you need to know.

## Cargo.toml

Assuming your project lives next to `worldgen/` (same parent directory):

```toml
[dependencies]
worldgen = { path = "../worldgen", default-features = false }
```

`default-features = false` is **required**. The default profile turns on
the `frontend` feature, which pulls in Leptos, wasm-bindgen, web-sys, and
the full WASM toolchain. None of that is needed to call the library API;
leaving it on roughly triples your build time and dep count for no gain.

Do **not** enable `frontend` or `backend` — both are for the apps in
this repo, not for library consumers.

## Public API

```rust
worldgen::generate_system_png(seed: u64, constraints: SystemConstraints)
    -> Result<Vec<u8>, WorldgenError>
worldgen::generate_system_png_scaled(seed: u64, constraints: SystemConstraints, scale: f32)
    -> Result<Vec<u8>, WorldgenError>
worldgen::generate_planet_png(seed: u64, uwp: &str, name: Option<&str>)
    -> Result<Vec<u8>, WorldgenError>
worldgen::generate_planet_png_scaled(seed: u64, uwp: &str, name: Option<&str>, scale: f32)
    -> Result<Vec<u8>, WorldgenError>
```

`generate_system_png_scaled` produces a higher-resolution render — pass
`2.0` for a 3200×1800 image, `4.0` for 6400×3600, etc. `scale` must be
finite and `>= 1.0`. **Composition is unchanged** at any scale: orbit
positions, body radii, font sizes, stroke widths, and the legend all
scale by the same factor — only pixel count changes. `scale = 1.0` is
byte-for-byte identical to `generate_system_png` (the unscaled call is
implemented as `generate_system_png_scaled(seed, constraints, 1.0)`).
Output is deterministic per `(seed, constraints, scale)`; `scale` does
not feed any RNG. Use higher scales when exporting for VTT use.

`generate_planet_png_scaled` works the same way for the per-planet
renderer. Native resolution at `scale = 1.0` is ~1000×655 (sheet +
legend); pass `2.0` for ~2000×1310. Same byte-identity contract: the
unscaled `generate_planet_png` is `..._scaled(..., 1.0)` under the
hood. Same determinism contract: `scale` doesn't feed the RNG.

Stable seed derivation from TravellerMap-style identity:

```rust
worldgen::seed::system_seed(sector: &str, hex_x: u8, hex_y: u8) -> u64
worldgen::seed::planet_seed(system_seed: u64, planet_orbit: i32, planet_name: &str) -> u64
```

Constraint types (re-exported at crate root):

- `SystemConstraints` — top-level container.
- `Constraint` — sum type: `Star`, `Planet`, `GasGiant`, `Moon`, `Belt`, `Empty`.
- `PartialUwp` — partial UWP (`'X'` = wild). Build via `PartialUwp::parse("A788899-A")`.
- Enums: `StarOrbit`, `StarType`, `StarSize`, `GasGiantSize`.
- Error: `WorldgenError` (`Constraints(Vec<ConstraintError>)`, `Map(MapError)`, `Render(String)`).

If you need the intermediate data (not just PNG bytes):

- `worldgen::systems::system::System` — full system structure from
  `System::generate_from_constraints_seeded(seed, constraints)`.
- `worldgen::worldmap::WorldMap` — full planet structure from
  `worldgen::worldmap::generate(uwp, seed, name)`.

## Minimum-viable usage

```rust
use worldgen::seed::{planet_seed, system_seed};
use worldgen::{
    Constraint, PartialUwp, SystemConstraints, generate_planet_png, generate_system_png,
};

fn render_world_from_travellermap(
    sector: &str, hex_x: u8, hex_y: u8,
    world_name: &str, world_uwp: &str, main_world_orbit: i32,
) -> (Vec<u8>, Vec<u8>) {
    let sys_seed = system_seed(sector, hex_x, hex_y);
    let constraints = SystemConstraints {
        bodies: vec![Constraint::Planet {
            name: Some(world_name.into()),
            orbit: None,
            uwp: Some(PartialUwp::parse(world_uwp).unwrap()),
            num_satellites: None,
            is_mainworld: true,
        }],
    };
    let system_png = generate_system_png(sys_seed, constraints).unwrap();
    let planet_png = generate_planet_png(
        planet_seed(sys_seed, main_world_orbit, world_name),
        world_uwp, Some(world_name),
    ).unwrap();
    (system_png, planet_png)
}
```

A runnable copy of this lives at `examples/external_consumer.rs` in the
worldgen repo (`cargo run --example external_consumer --no-default-features`).

## Building constraints

`SystemConstraints` describes what bodies to pin or count when generating
a system. For the common "I have a main world + counts" case there's a
convenience builder; for richer needs you can compose `Constraint` rows
by hand.

### `build_constraints` — the simple path

```rust
worldgen::build_constraints(
    main_world_name: &str,
    main_world_uwp: &str,         // fully-specified, 9 chars, e.g. "A788899-A"
    stars: &[StarSpec],           // [] means "roll the whole star roster"
    num_gas_giants: usize,
    num_planetoid_belts: usize,
    num_planets: usize,           // additional rocky planets beyond the main world
) -> Result<SystemConstraints, WorldgenError>
```

`StarSpec` is `{ spectral: StarType, subtype: Option<u8>, size: StarSize }`.
Build one with `StarSpec::new(StarType::G, 2, StarSize::V)` (G2 V) or
`StarSpec::with_rolled_subtype(StarType::M, StarSize::V)` (M-class main
sequence, subtype rolled). The first `StarSpec` becomes the primary;
subsequent ones become companions. Spectral types are `O | B | A | F | G
| K | M`; sizes are `Ia | Ib | II | III | IV | V | VI | D`.

Example — Noricum (Trojan Reach 3128) with its real three-star roster,
2 gas giants, 1 belt, 3 extra planets:

```rust
use worldgen::{
    build_constraints, generate_system_png, seed::system_seed,
    StarSpec, StarSize, StarType,
};

let cs = build_constraints(
    "Noricum",
    "D8867BB-1",
    &[
        StarSpec::new(StarType::G, 2, StarSize::V),
        StarSpec::new(StarType::M, 9, StarSize::V),
        StarSpec::new(StarType::M, 6, StarSize::V),
    ],
    2,  // gas giants
    1,  // planetoid belts
    3,  // additional planets
)?;
let png = generate_system_png(system_seed("Trojan Reach", 31, 28), cs)?;
```

What the builder does under the hood: pushes one `Constraint::Star` per
`StarSpec` (first one pinned to `Primary`, the rest with `orbit: None`),
then N anonymous `GasGiant` / `Belt` / `Planet` rows with every field
`None` — telling the generator "place one, pick a free orbit, roll the
details."

### Composing `Constraint` rows by hand

If you need more than counts (e.g. pin a specific planet to orbit 8 with
a partial UWP, or place a moon under a particular parent), `cs.bodies`
is just a `Vec<Constraint>` you can push directly into. Variants:

- `Constraint::Star { orbit, spectral, subtype, size }` — any field `None`
  rolls.
- `Constraint::Planet { name, orbit, uwp, num_satellites, is_mainworld }`
  — set `is_mainworld: true` on exactly one row (the builder already
  does this); `uwp: Some(PartialUwp::parse("A8XXXXX-X")?)` pins specific
  digits (uppercase `X` = wild).
- `Constraint::GasGiant { name, orbit, size, num_satellites }` —
  `size: Some(GasGiantSize::Large)` forces large.
- `Constraint::Belt { name, orbit, uwp, num_satellites }`.
- `Constraint::Moon { name, parent_orbit, uwp }` — `parent_orbit` is the
  parent body's orbit number.
- `Constraint::Empty { orbit }` — block an orbit.

### What can go wrong

- **Returned errors** (`WorldgenError::Constraints(Vec<ConstraintError>)`):
  malformed main-world UWP, multiple `is_mainworld: true` rows, duplicate
  pinned orbits, contradictory UWP columns (size 0 with non-zero hydro,
  etc.), or a main world with a partial UWP. All hard rejections.
- **Silently dropped** (logged at `warn!`, generation still succeeds):
  a body whose pinned orbit is occupied or out of range, or a counted
  body that ran out of free orbit slots. Treat the requested counts as
  "up to N", not a hard guarantee.

## Determinism contract

- The system generator uses `rand_chacha::ChaCha8Rng`, whose algorithm is
  contractually frozen across `rand_chacha` versions. Same seed → same
  generation, forever.
- `system_seed` and `planet_seed` use SipHash-2-4 with hardcoded keys
  defined in `src/seed.rs`. The recipe is pinned; the snapshot tests in
  `src/seed.rs` will fail loudly if the hash ever changes.
- **Bumping the `worldgen` dep version can change image content** — if a
  generation rule, name table, or rasterizer change lands upstream, the
  output for a given seed shifts. This is a deliberate compat boundary,
  not a bug. **Pin your `worldgen` dependency by commit SHA** (or `tag =`
  if tags exist) if you need stable images across worldgen updates.

## HTTP endpoint (for non-Rust callers)

If you can't link worldgen as a Rust crate — e.g. a browser client like
Traveller Map's web frontend — the backend server exposes the same
flow over HTTP. One endpoint, no auth, permissive CORS.

```
GET <base>/system
  ?sector=<string>     required  sector name (used only for the seed)
  &hex=<CCRR>          required  4-digit string; "2018" → hex_x=20, hex_y=18
  &name=<string>       required  main-world name
  &uwp=<9-char>        required  full UWP, e.g. "D8867BB-1"
  &pbg=<3-char>        optional  PBG digits; char[1]=belts, char[2]=giants
  &stellar=<string>    optional  e.g. "G2 V M9 V M6 V"; empty → roll
  &worlds=<int>        optional  system W digit; planet count = max(W - 1 - belts - giants, 0)
  &scale=<float>       optional  pixel scale, default 2.0, must be finite and >= 1.0
```

Response:
- `200 image/png` — the system map (default 3200×1800 at `scale=2.0`).
- `400 text/plain` — missing or malformed required parameter.
- `422 text/plain` — `build_constraints` rejected the inputs (invalid /
  partial / contradictory UWP). Body is the constraint error reason.
- `500 text/plain` — render failure (scale out of range, tiny-skia OOM).

CORS: `Access-Control-Allow-Origin: *`, `Access-Control-Allow-Methods:
GET, HEAD, OPTIONS`, `Access-Control-Allow-Headers: *`. OPTIONS preflight
returns `204 No Content` with the same headers.

Determinism contract is the same as the library: same
`(sector, hex, name, uwp, pbg, stellar, worlds, scale)` always yields
byte-identical PNG bytes. `scale` never feeds any RNG.

Example:
```
http://<host>:<port>/system?sector=Trojan+Reach&hex=2018&name=Noricum&uwp=D8867BB-1&pbg=804&stellar=G2+V+M9+V+M6+V&worlds=14
```

Where to point the client:
- **Local dev (this repo's `./scripts/run-backend.sh`):**
  `http://127.0.0.1:8081/system`
- **Deployed (Cloud Run):** same path on the deployed hostname, served
  on the same port the WebSocket endpoints use. Behind nginx in the
  Docker image you may need to add a `/system` proxy rule alongside the
  existing `/ws/trade` rules.

The HTTP and WebSocket endpoints share one TCP port. The dispatcher
peeks the first inbound bytes and routes anything with `Upgrade:
websocket` to the WS handlers; everything else goes to the HTTP
handler. The trade-tool / simulator / captain's-log WebSocket flows
are unchanged.

### `GET /world` — planet surface map, with GCS-backed cache

Same shape as `/system`, scoped to a single planet. The generator
takes 20–30 s per cold call, so the backend caches the canonical-scale
render in Google Cloud Storage and serves subsequent calls in
~200 ms.

```
GET <base>/world
  ?sector=<string>     required  sector name (seed input only)
  &hex=<CCRR>          required  4-digit string; "2018" → hex_x=20, hex_y=18
  &name=<string>       required  planet name; the main world today
  &uwp=<9-char>        required  full UWP, e.g. "D8867BB-1"
  &orbit=<int>         optional  planet's system orbit (default 3 — typical main world)
  &scale=<float>       optional  pixel scale, default 1.0, must be finite and >= 1.0
```

Response:
- `200 image/png` — the planet surface map. Native dimensions at
  `scale = 1.0` are ~1000×655; the cache always stores the
  **canonical** `scale = 2.0` render (~2000×1310), and smaller requests
  are downsampled on-the-fly.
- `X-Cache: HIT | MISS | DISABLED | BYPASS` — `HIT` and `MISS` are
  the obvious cache states. `DISABLED` means the backend is running
  with `GCS_BUCKET=debug` (local dev) and didn't try to cache.
  `BYPASS` means the GCS GET failed and the backend regenerated to
  avoid blocking the request.
- `Cache-Control: public, max-age=31536000, immutable` — browsers can
  cache aggressively; the underlying bytes never change for a given
  query.
- Same `400 / 422 / 500` semantics as `/system`. CORS headers
  identical.

**Determinism contract (same as `/system`, just continuing the chain):**

```
(sector, hex_x, hex_y)       → seed::system_seed                → sys_seed
(sys_seed, orbit, name)      → seed::planet_seed                → seed
generate_planet_png_scaled(seed, uwp, Some(name), CANONICAL_SCALE)
  └─ worldmap::generate(uwp, seed, name)
      └─ ChaCha8Rng::seed_from_u64(seed)
```

Same `(sector, hex, name, uwp, orbit)` → same `seed` → same canonical
PNG bytes, forever. `scale` is **not** part of the seed or the cache
key — it only affects the post-cache downsample step.

**Scale clamping**: `scale > 2.0` is silently clamped to the canonical
scale. We don't upsample (poor quality) and won't regenerate fresh at
a higher scale (defeats the cache). If a consumer truly needs a
higher resolution, file an issue and we'll bump the canonical or add
a separate code path.

**Cache mechanics**:
- Cache key is `SipHash-2-4(seed, normalized_uwp, normalized_name)`
  — no scale, no sector/hex (those already determine `seed`).
- Object path: `world/v1/<u64_hex>.png` in the bucket named by
  `GCS_BUCKET`. Bump the `v1` segment to invalidate every cached
  render at once (e.g. on a worldgen version bump that changes pixel
  output).
- Set `GCS_BUCKET=debug` to disable caching (local dev). The endpoint
  still serves valid PNGs, just regenerates every time.

Example:
```
http://<host>:<port>/world?sector=Trojan+Reach&hex=2018&name=Noricum&uwp=D8867BB-1
```

Deployed URL once the nginx rule and the next image push are in:
```
https://tools.callistoflight.com/world?...
```

## What this library is NOT

- No HTTP server. (The `backend` feature exists for the trade computer's
  WebSocket server — irrelevant here.)
- No UI. The Leptos components ride behind the `frontend` feature.
- No trade computer / passenger / freight calculations exposed at the
  top level. The `trade` module is compiled because the `systems`
  module needs its data types (`PortCode`, `TradeClass`, etc.), but
  the user-facing trade tools aren't reachable.
- No async runtime needed; the library is fully synchronous.
