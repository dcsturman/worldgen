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

## What this library is NOT

- No HTTP server. (The `backend` feature exists for the trade computer's
  WebSocket server — irrelevant here.)
- No UI. The Leptos components ride behind the `frontend` feature.
- No trade computer / passenger / freight calculations exposed at the
  top level. The `trade` module is compiled because the `systems`
  module needs its data types (`PortCode`, `TradeClass`, etc.), but
  the user-facing trade tools aren't reachable.
- No async runtime needed; the library is fully synchronous.
