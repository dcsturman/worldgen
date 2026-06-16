//! Integration tests for the public library API.
//!
//! This file sits at the crate root (outside `src/`), so it links against
//! the same public surface an external Cargo consumer would. Any test
//! that fails to compile here means the API isn't actually reachable —
//! a `pub` was missed somewhere, or a needed type isn't re-exported.

use worldgen::seed::{planet_seed, system_seed};
use worldgen::{
    Constraint, PartialUwp, StarSize, StarSpec, StarType, SystemConstraints, WorldgenError,
    build_constraints, generate_planet_png, generate_planet_png_scaled, generate_system_png,
    generate_system_png_scaled,
};

/// PNG magic header — all our renders must start with this 8-byte
/// signature for a downstream image library to accept them.
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

fn noricum_constraints() -> SystemConstraints {
    SystemConstraints::from_main_world("Noricum", "D8867BB-1").expect("Noricum UWP is valid")
}

#[test]
fn system_png_is_valid_png() {
    let bytes = generate_system_png(42, noricum_constraints())
        .expect("a fully-specified main-world constraint always generates");
    assert!(
        bytes.len() > 1000,
        "PNG suspiciously small: {} bytes",
        bytes.len()
    );
    assert_eq!(&bytes[..8], PNG_MAGIC);
}

#[test]
fn system_png_is_deterministic() {
    // Same seed + same constraints must produce byte-identical PNG.
    // This is the headline determinism guarantee for the library API.
    let a = generate_system_png(42, noricum_constraints()).unwrap();
    let b = generate_system_png(42, noricum_constraints()).unwrap();
    assert_eq!(
        a, b,
        "same (seed, constraints) produced different PNG bytes"
    );
}

#[test]
fn different_seeds_produce_different_systems() {
    let a = generate_system_png(42, noricum_constraints()).unwrap();
    let b = generate_system_png(43, noricum_constraints()).unwrap();
    // Not formally required, but ChaCha8Rng with two single-bit-different
    // seeds should produce wildly different streams. If this ever fires
    // it's almost certainly a regression where the seed isn't reaching
    // the generation path.
    assert_ne!(
        a, b,
        "different seeds produced identical PNG — seed not plumbed?"
    );
}

#[test]
fn planet_png_is_valid_png() {
    let bytes = generate_planet_png(42, "A788899-A", Some("Regina")).unwrap();
    assert!(bytes.len() > 1000);
    assert_eq!(&bytes[..8], PNG_MAGIC);
}

#[test]
fn planet_png_is_deterministic() {
    let a = generate_planet_png(42, "A788899-A", Some("Regina")).unwrap();
    let b = generate_planet_png(42, "A788899-A", Some("Regina")).unwrap();
    assert_eq!(a, b, "same (seed, uwp, name) produced different PNG bytes");
}

#[test]
fn invalid_uwp_returns_map_error() {
    let result = generate_planet_png(42, "not-a-real-uwp", None);
    assert!(matches!(result, Err(WorldgenError::Map(_))));
}

#[test]
fn planet_png_scaled_at_1_0_matches_unscaled_byte_for_byte() {
    // Legacy contract: existing `generate_planet_png` keeps producing
    // today's exact bytes. The new `_scaled` API at scale=1.0 must
    // not perturb a single pixel.
    let a = generate_planet_png(42, "A788899-A", Some("Regina")).unwrap();
    let b = generate_planet_png_scaled(42, "A788899-A", Some("Regina"), 1.0).unwrap();
    assert_eq!(a, b, "scale=1.0 must be byte-identical to unscaled render");
}

#[test]
fn planet_png_scaled_at_2_0_doubles_dimensions() {
    let bytes = generate_planet_png_scaled(42, "A788899-A", Some("Regina"), 2.0).unwrap();
    assert_eq!(&bytes[..8], PNG_MAGIC);
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    // At scale=1.0 the planet renders to ~1000x655. Scale=2.0 must
    // double both axes, preserving composition.
    let unscaled = generate_planet_png(42, "A788899-A", Some("Regina")).unwrap();
    let uw = u32::from_be_bytes([unscaled[16], unscaled[17], unscaled[18], unscaled[19]]);
    let uh = u32::from_be_bytes([unscaled[20], unscaled[21], unscaled[22], unscaled[23]]);
    assert_eq!(w, uw * 2, "scale=2.0 width should be 2x native");
    assert_eq!(h, uh * 2, "scale=2.0 height should be 2x native");
}

#[test]
fn planet_png_scaled_is_deterministic() {
    let a = generate_planet_png_scaled(42, "A788899-A", Some("Regina"), 2.0).unwrap();
    let b = generate_planet_png_scaled(42, "A788899-A", Some("Regina"), 2.0).unwrap();
    assert_eq!(
        a, b,
        "same (seed, uwp, name, scale) must produce identical bytes"
    );
}

#[test]
fn planet_png_scaled_rejects_below_1_0() {
    let r = generate_planet_png_scaled(42, "A788899-A", Some("Regina"), 0.5);
    assert!(matches!(r, Err(WorldgenError::Render(_))));
}

#[test]
fn planet_png_scaled_rejects_non_finite() {
    let r = generate_planet_png_scaled(42, "A788899-A", Some("Regina"), f32::NAN);
    assert!(matches!(r, Err(WorldgenError::Render(_))));
    let r = generate_planet_png_scaled(42, "A788899-A", Some("Regina"), f32::INFINITY);
    assert!(matches!(r, Err(WorldgenError::Render(_))));
}

#[test]
fn empty_constraints_returns_constraints_error() {
    // No main world → constraint validation should fail.
    let result = generate_system_png(42, SystemConstraints::default());
    assert!(matches!(result, Err(WorldgenError::Constraints(_))));
}

/// End-to-end replication of the documented "TravellerMap identity →
/// PNG" flow, exercising the same code path the external consumer will
/// run. Going through `system_seed` / `planet_seed` is what makes the
/// flow stable across machines and time.
fn render_world_from_travellermap(
    sector: &str,
    hex_x: u8,
    hex_y: u8,
    world_name: &str,
    world_uwp: &str,
    main_world_orbit: i32,
) -> (Vec<u8>, Vec<u8>) {
    let sys_seed = system_seed(sector, hex_x, hex_y);
    let mut constraints = SystemConstraints::default();
    constraints.bodies.push(Constraint::Planet {
        name: Some(world_name.into()),
        orbit: None,
        uwp: Some(PartialUwp::parse(world_uwp).unwrap()),
        num_satellites: None,
        is_mainworld: true,
    });
    let system_png = generate_system_png(sys_seed, constraints).unwrap();
    let planet_png = generate_planet_png(
        planet_seed(sys_seed, main_world_orbit, world_name),
        world_uwp,
        Some(world_name),
    )
    .unwrap();
    (system_png, planet_png)
}

#[test]
fn end_to_end_travellermap_flow_produces_valid_pngs() {
    let (sys, planet) =
        render_world_from_travellermap("Trojan Reach", 31, 28, "Noricum", "D8867BB-1", 3);
    assert!(sys.len() > 1000);
    assert!(planet.len() > 1000);
    assert_eq!(&sys[..8], PNG_MAGIC);
    assert_eq!(&planet[..8], PNG_MAGIC);
}

#[test]
fn same_travellermap_identity_produces_byte_identical_pngs() {
    // The user's actual headline use case: clicking the same world on
    // TravellerMap twice always produces the same images. If this test
    // ever fires, something in the determinism chain broke — either the
    // seed derivation, the ChaCha RNG plumbing, or the rasterizer.
    let (sys1, planet1) =
        render_world_from_travellermap("Trojan Reach", 31, 28, "Noricum", "D8867BB-1", 3);
    let (sys2, planet2) =
        render_world_from_travellermap("Trojan Reach", 31, 28, "Noricum", "D8867BB-1", 3);
    assert_eq!(sys1, sys2, "system PNG drifted across runs");
    assert_eq!(planet1, planet2, "planet PNG drifted across runs");
}

#[test]
fn build_constraints_assembles_full_system_recipe() {
    // The headline first-application use case: main world UWP + N stars
    // + counts of gas giants / belts / planets. The builder must hand
    // the generator a constraint set that produces a valid system PNG.
    let cs = build_constraints(
        "Noricum",
        "D8867BB-1",
        &[
            StarSpec::new(StarType::G, 2, StarSize::V),
            StarSpec::new(StarType::M, 9, StarSize::V),
            StarSpec::new(StarType::M, 6, StarSize::V),
        ],
        2, // gas giants
        1, // planetoid belts
        3, // additional planets
    )
    .expect("valid main-world UWP");
    let png = generate_system_png(42, cs).expect("builder output should always generate");
    assert!(png.len() > 1000);
    assert_eq!(&png[..8], PNG_MAGIC);
}

#[test]
fn build_constraints_is_deterministic_under_same_seed() {
    let cs1 = build_constraints(
        "Noricum",
        "D8867BB-1",
        &[StarSpec::new(StarType::G, 2, StarSize::V)],
        2,
        1,
        3,
    )
    .unwrap();
    let cs2 = build_constraints(
        "Noricum",
        "D8867BB-1",
        &[StarSpec::new(StarType::G, 2, StarSize::V)],
        2,
        1,
        3,
    )
    .unwrap();
    let a = generate_system_png(99, cs1).unwrap();
    let b = generate_system_png(99, cs2).unwrap();
    assert_eq!(a, b);
}

#[test]
fn build_constraints_rejects_malformed_uwp() {
    let result = build_constraints("Noricum", "not-a-uwp", &[], 0, 0, 0);
    assert!(matches!(result, Err(WorldgenError::Constraints(_))));
}

#[test]
fn build_constraints_with_zero_stars_lets_generator_roll() {
    // Empty stars slice: generator rolls the entire star roster from
    // the main world's UWP using the existing star generation pipeline.
    let cs = build_constraints("Regina", "A788899-A", &[], 1, 0, 2).unwrap();
    let png = generate_system_png(7, cs).expect("no-star spec should still generate");
    assert!(png.len() > 1000);
}

#[test]
fn scaled_render_at_1_0_matches_unscaled_byte_for_byte() {
    // Legacy contract: existing callers using `generate_system_png` get
    // exactly today's output. The scaled API at scale=1.0 must not
    // perturb a single pixel.
    let a = generate_system_png(42, noricum_constraints()).unwrap();
    let b = generate_system_png_scaled(42, noricum_constraints(), 1.0).unwrap();
    assert_eq!(a, b, "scale=1.0 must be byte-identical to unscaled render");
}

#[test]
fn scaled_render_at_2_0_is_3200x1800() {
    let bytes = generate_system_png_scaled(42, noricum_constraints(), 2.0).unwrap();
    assert_eq!(&bytes[..8], PNG_MAGIC);
    // PNG IHDR at byte offset 16 holds width (4) then height (4),
    // big-endian. Read them and confirm dimensions.
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    assert_eq!((w, h), (3200, 1800), "scale=2.0 should yield 3200x1800");
}

#[test]
fn scaled_render_is_deterministic() {
    // scale must not perturb the RNG; same (seed, constraints, scale)
    // must produce byte-identical output.
    let a = generate_system_png_scaled(42, noricum_constraints(), 2.0).unwrap();
    let b = generate_system_png_scaled(42, noricum_constraints(), 2.0).unwrap();
    assert_eq!(a, b);
}

#[test]
fn scaled_render_rejects_below_1_0() {
    let r = generate_system_png_scaled(42, noricum_constraints(), 0.5);
    assert!(matches!(r, Err(WorldgenError::Render(_))));
}

#[test]
fn scaled_render_rejects_non_finite() {
    let r = generate_system_png_scaled(42, noricum_constraints(), f32::NAN);
    assert!(matches!(r, Err(WorldgenError::Render(_))));
    let r = generate_system_png_scaled(42, noricum_constraints(), f32::INFINITY);
    assert!(matches!(r, Err(WorldgenError::Render(_))));
}

#[test]
fn different_travellermap_identities_produce_different_systems() {
    let (sys1, _) =
        render_world_from_travellermap("Trojan Reach", 31, 28, "Noricum", "D8867BB-1", 3);
    let (sys2, _) =
        render_world_from_travellermap("Trojan Reach", 31, 29, "Noricum", "D8867BB-1", 3);
    let (sys3, _) =
        render_world_from_travellermap("Spinward Marches", 31, 28, "Noricum", "D8867BB-1", 3);
    assert_ne!(sys1, sys2);
    assert_ne!(sys1, sys3);
    assert_ne!(sys2, sys3);
}
