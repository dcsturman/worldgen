//! Byte-identity pins for undecorated (rotating) worlds.
//!
//! The tide-lock climate model threads a [`ClimateModel`] through every
//! temperature, water, wind, cloud and settlement decision. The rotating arm
//! of each branch must stay *exactly* what it was: every existing world is
//! cached in GCS under a key with nothing about the generator in it, so a
//! single pixel of drift makes a cached world disagree with a fresh render of
//! itself. These hashes were computed on the tree before the climate branch
//! existed; if one moves, a rotating world changed.
//!
//! [`ClimateModel`]: super::climate::ClimateModel

use super::*;

/// FNV-1a: fixed forever, unlike `DefaultHasher`, so a pinned value can't
/// move under a toolchain upgrade.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Hashes of `[svg, png, globe png, globe texture, globe texture w/o clouds]`.
fn hashes(uwp: &str, seed: u64) -> [u64; 5] {
    let map = generate(uwp, seed, Some("Golden")).unwrap();
    [
        fnv(render_svg(&map).as_bytes()),
        fnv(&render_png(&map).unwrap()),
        fnv(&render_globe_png(&map, 160, 0.7, TexSize { w: 256, h: 128 }).unwrap()),
        fnv(&render_globe_texture(&map, TexSize { w: 256, h: 128 }, true).unwrap()),
        fnv(&render_globe_texture(&map, TexSize { w: 256, h: 128 }, false).unwrap()),
    ]
}

/// A spread of climates: garden, Earth, waterworld, desert, vacuum ice ball,
/// dense-atmosphere urban, a thin-atmosphere populated desert (the tide-lock
/// acceptance UWP, here unlocked), and a trace-atmosphere rock.
const CASES: &[(&str, u64)] = &[
    ("A788899-A", 1),
    ("C886977-8", 0xDEAD_BEEF),
    ("A78A899-A", 1),
    ("A780899-A", 0xDEAD_BEEF),
    ("A300077-A", 1),
    ("A8888AA-A", 0xDEAD_BEEF),
    ("C530677-8", 1),
    ("E5306A8-8", 0xDEAD_BEEF),
];

/// Pinned from the pre-tide-lock tree (base commit 560525d plus the
/// decoration plumbing, which renders identically — see
/// `empty_decorations_generate_the_undecorated_map`).
const PINS: &[[u64; 5]] = &[
    [
        0xcb360e9741ffb934,
        0xb6e37cbd85d8555e,
        0x410c2056997c5b4d,
        0xf49ad51743a719d4,
        0x3bae82ae0eec06cc,
    ],
    [
        0x599f942d912ffdcd,
        0x823ce7c63d227b8d,
        0xc3b21a83b998699f,
        0xed80a0dad7f78a55,
        0xaa9631796afe59b1,
    ],
    [
        0x780dbede093a9425,
        0x875856cdd0875e93,
        0x4587a8c1969bd062,
        0x62f5e032ca5a3662,
        0x1254ed5890cfac43,
    ],
    [
        0x1008e54a23b55d8b,
        0x55d9d7ddfda3364b,
        0xc09ae015443bc56c,
        0x3156dad431e2cfda,
        0x96ba5207b24342e5,
    ],
    [
        0xeedec4b52698a7f1,
        0xdd1e571b26698766,
        0x607a5903df87ad33,
        0x4df3f367d3d89bd7,
        0x4df3f367d3d89bd7,
    ],
    [
        0xc9ef0111058bdd31,
        0x2cd47de8c21a6d80,
        0xf2071feaa656e512,
        0x29d26fd9d4bc2a5a,
        0xf509c1acb5a234e0,
    ],
    [
        0x8ba1f4fa633148e3,
        0xaa39079ccf01dd69,
        0xa14d66877c4cec83,
        0xc71cb9860d75f58b,
        0xcf22965fa2f0d826,
    ],
    [
        0x6a9162d65a5895d9,
        0x3c940ac4fd3ab9f4,
        0xa1030f2ec2b1aedf,
        0x394b8c75a9b34abe,
        0xb475009d0ab42d08,
    ],
];

/// Prints the current hashes in `PINS` form. Run to re-pin only when a
/// rotating-world change is intended (and bump the `/api/world` cache version
/// to match): `cargo test --lib worldmap::golden::print_pins -- --ignored --nocapture`.
#[test]
#[ignore]
fn print_pins() {
    for (uwp, seed) in CASES {
        let h = hashes(uwp, *seed);
        println!(
            "    [0x{:016x}, 0x{:016x}, 0x{:016x}, 0x{:016x}, 0x{:016x}], // {uwp} {seed}",
            h[0], h[1], h[2], h[3], h[4]
        );
    }
}

#[test]
fn rotating_worlds_render_byte_identically() {
    assert_eq!(PINS.len(), CASES.len());
    // One thread per case: each is a full flat + globe render, and serially
    // they'd make this the slowest test in the suite by a wide margin.
    std::thread::scope(|s| {
        for ((uwp, seed), pin) in CASES.iter().zip(PINS) {
            s.spawn(move || assert_eq!(&hashes(uwp, *seed), pin, "{uwp} seed {seed} changed"));
        }
    });
}
