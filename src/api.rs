//! Top-level library API for external Rust consumers.
//!
//! This module is the canonical entry point for code outside this repo
//! that depends on worldgen as a Cargo dependency (typically with
//! `default-features = false`). It composes the existing seeded
//! `System::generate_from_constraints_seeded` + `sysmap::render_png` and
//! `worldmap::generate` + `worldmap::render_png` pipelines into two
//! "give me a PNG" calls and surfaces a single unified error type.
//!
//! The Leptos UI in this crate still uses the lower-level pieces
//! directly; this module exists only for the library shape.

use crate::systems::constraint::{Constraint, ConstraintError, SystemConstraints};
use crate::systems::system::{StarOrbit, StarSize, StarType, System};
use crate::worldmap::{MapError, WorldMap};

/// Unified error type for the public library API.
///
/// Composes the three failure modes the underlying pipelines can produce:
/// invalid constraints, invalid UWP/worldmap input, and rasterization
/// failure (which is just a `String` in the existing renderer surface).
#[derive(Debug, thiserror::Error)]
pub enum WorldgenError {
    /// One or more `SystemConstraints` were invalid or contradictory.
    /// The inner vec preserves every individual error so the caller can
    /// surface them all at once.
    #[error("system constraints invalid: {0:?}")]
    Constraints(Vec<ConstraintError>),

    /// The UWP string passed to [`generate_planet_png`] could not be
    /// parsed into a valid Traveller world profile.
    #[error("worldmap input invalid: {0:?}")]
    Map(MapError),

    /// PNG rasterization failed. The string is whatever the renderer
    /// produced — typically a tiny-skia error mapped to text.
    #[error("png render failed: {0}")]
    Render(String),
}

impl From<Vec<ConstraintError>> for WorldgenError {
    fn from(v: Vec<ConstraintError>) -> Self {
        WorldgenError::Constraints(v)
    }
}

impl From<MapError> for WorldgenError {
    fn from(e: MapError) -> Self {
        WorldgenError::Map(e)
    }
}

/// Generate a Traveller solar system from `constraints`, render it to a
/// system-map PNG, and return the bytes.
///
/// **Determinism contract:** for a fixed `(seed, constraints)` pair, the
/// returned PNG bytes are byte-identical across runs, machines, and OS
/// versions. This holds as long as the worldgen dep version is pinned
/// (bumping the dep may change generation rules or rendering pixels).
///
/// The intermediate `System` is also accessible via
/// [`System::generate_from_constraints_seeded`] if a consumer needs the
/// structured data, not just the rendered image.
pub fn generate_system_png(
    seed: u64,
    constraints: SystemConstraints,
) -> Result<Vec<u8>, WorldgenError> {
    generate_system_png_scaled(seed, constraints, 1.0)
}

/// Generate a Traveller solar system PNG at the requested pixel scale.
///
/// `scale = 1.0` matches [`generate_system_png`] byte-for-byte and
/// produces a 1600×900 image; `scale = 2.0` produces 3200×1800; higher
/// values scale proportionally. **Composition is preserved** — the
/// layout, orbit positions, body radii, font sizes, stroke widths, and
/// the legend all scale by the same factor. Only the pixel count
/// changes; the relative look is identical.
///
/// Determinism is preserved across scales: the `scale` parameter does
/// not feed any RNG, so `(seed, constraints)` continues to drive system
/// generation, and `(seed, constraints, scale)` deterministically maps
/// to the output PNG bytes.
///
/// Returns [`WorldgenError::Render`] if `scale < 1.0` or not finite.
pub fn generate_system_png_scaled(
    seed: u64,
    constraints: SystemConstraints,
    scale: f32,
) -> Result<Vec<u8>, WorldgenError> {
    let system = System::generate_from_constraints_seeded(seed, constraints)?;
    crate::sysmap::render_png_scaled(&system, scale).map_err(WorldgenError::Render)
}

/// Generate a planet surface map for the given UWP, render it to PNG,
/// and return the bytes.
///
/// The `seed` parameter is the planet-specific seed — typically derived
/// from [`crate::seed::planet_seed`] using the system seed plus the
/// planet's identity within the system. Same `(seed, uwp, name)` triple
/// always produces byte-identical output.
///
/// The intermediate [`WorldMap`] is also accessible via
/// [`crate::worldmap::generate`] if a consumer needs the structured
/// terrain/climate/biome data.
pub fn generate_planet_png(
    seed: u64,
    uwp: &str,
    name: Option<&str>,
) -> Result<Vec<u8>, WorldgenError> {
    generate_planet_png_scaled(seed, uwp, name, 1.0)
}

/// Generate a planet PNG at `scale × native resolution`.
///
/// `scale = 1.0` is byte-identical to [`generate_planet_png`]; higher
/// values scale every pixel dimension and font/stroke size by the same
/// factor while keeping the same composition. The native dimensions are
/// ~975×630 plus a legend band (`SHEET_WIDTH × (SHEET_HEIGHT + LEGEND_HEIGHT)`).
///
/// Determinism contract: same `(seed, uwp, name, scale)` always yields
/// the same bytes. `scale` doesn't feed any RNG — it only scales the
/// pixel grid. Returns [`WorldgenError::Render`] if `scale < 1.0` or
/// not finite.
pub fn generate_planet_png_scaled(
    seed: u64,
    uwp: &str,
    name: Option<&str>,
    scale: f32,
) -> Result<Vec<u8>, WorldgenError> {
    let map: WorldMap = crate::worldmap::generate(uwp, seed, name)?;
    crate::worldmap::render_png_scaled(&map, scale).map_err(WorldgenError::Render)
}

/// One star's classification, as the convenience builder expects it.
///
/// Mirrors a single `Constraint::Star` row but with the fields the
/// common library use case actually has: a spectral letter (G, M, etc.),
/// an optional subtype digit (the `2` in `G2`), and a size class. The
/// first `StarSpec` in [`build_constraints`]'s `stars` slice becomes the
/// system's primary; subsequent specs become companions (orbit rolled).
#[derive(Debug, Clone, Copy)]
pub struct StarSpec {
    pub spectral: StarType,
    /// Subtype digit 0–9 (e.g. the `2` in `G2`). `None` lets the
    /// generator roll it.
    pub subtype: Option<u8>,
    pub size: StarSize,
}

impl StarSpec {
    /// Construct a `StarSpec` with all three pieces specified — the
    /// common case (e.g. `StarSpec::new(StarType::G, 2, StarSize::V)` for
    /// a G2 V star).
    pub fn new(spectral: StarType, subtype: u8, size: StarSize) -> Self {
        Self { spectral, subtype: Some(subtype), size }
    }

    /// Construct a `StarSpec` with the subtype left for the generator
    /// to roll.
    pub fn with_rolled_subtype(spectral: StarType, size: StarSize) -> Self {
        Self { spectral, subtype: None, size }
    }
}

/// Build a [`SystemConstraints`] for the headline library use case:
/// pin the main world (with a fully-specified UWP) and the system's
/// star roster, ask for *N* gas giants / planetoid belts / additional
/// rocky planets, and let the generator roll everything else.
///
/// - `main_world_name` / `main_world_uwp` — required. The UWP must be
///   fully specified (9 chars, no `'X'` wildcards), e.g. `"A788899-A"`.
/// - `stars` — one entry per star in the system. The first entry is
///   placed at `StarOrbit::Primary`; subsequent entries are companions
///   whose orbit the generator picks. Pass `&[]` to let the generator
///   roll the entire star roster from the main world's UWP.
/// - `num_gas_giants`, `num_planetoid_belts`, `num_planets` — counts of
///   each "anonymous" body to drop into the system. Each lands at the
///   next available orbit, with size / UWP / satellites all rolled. A
///   body whose orbit can't be placed (no free slots, or blocked by
///   star zones) is skipped with a `warn!` log line — generation
///   succeeds even if not every requested body fits.
///
/// `num_planets` counts *additional* rocky planets beyond the main
/// world. The main world itself is always placed.
///
/// Returns an error only if the main-world UWP fails to parse. All
/// other validity checks happen inside
/// [`generate_system_png`] / [`crate::systems::system::System::generate_from_constraints_seeded`].
///
/// # Example
///
/// ```ignore
/// use worldgen::{
///     build_constraints, generate_system_png, seed::system_seed,
///     StarSpec, StarSize, StarType,
/// };
///
/// // Noricum (Trojan Reach 3128), G2 V + M9 V + M6 V, 2 gas giants, 1 belt, 3 planets
/// let cs = build_constraints(
///     "Noricum",
///     "D8867BB-1",
///     &[
///         StarSpec::new(StarType::G, 2, StarSize::V),
///         StarSpec::new(StarType::M, 9, StarSize::V),
///         StarSpec::new(StarType::M, 6, StarSize::V),
///     ],
///     2, 1, 3,
/// )?;
/// let png = generate_system_png(system_seed("Trojan Reach", 31, 28), cs)?;
/// # Ok::<(), worldgen::WorldgenError>(())
/// ```
pub fn build_constraints(
    main_world_name: &str,
    main_world_uwp: &str,
    stars: &[StarSpec],
    num_gas_giants: usize,
    num_planetoid_belts: usize,
    num_planets: usize,
) -> Result<SystemConstraints, WorldgenError> {
    let mut cs = SystemConstraints::from_main_world(main_world_name, main_world_uwp).map_err(
        |e| WorldgenError::Constraints(vec![ConstraintError::ContradictoryUwp(e)]),
    )?;

    for (i, spec) in stars.iter().enumerate() {
        cs.bodies.push(Constraint::Star {
            orbit: if i == 0 { Some(StarOrbit::Primary) } else { None },
            spectral: Some(spec.spectral),
            subtype: spec.subtype,
            size: Some(spec.size),
        });
    }

    for _ in 0..num_gas_giants {
        cs.bodies.push(Constraint::GasGiant {
            name: None,
            orbit: None,
            size: None,
            num_satellites: None,
        });
    }

    for _ in 0..num_planetoid_belts {
        cs.bodies.push(Constraint::Belt {
            name: None,
            orbit: None,
            uwp: None,
            num_satellites: None,
        });
    }

    for _ in 0..num_planets {
        cs.bodies.push(Constraint::Planet {
            name: None,
            orbit: None,
            uwp: None,
            num_satellites: None,
            is_mainworld: false,
        });
    }

    Ok(cs)
}

/// Parse a Traveller-Map style "Stellar" string into a list of
/// [`StarSpec`]s suitable for passing to [`build_constraints`].
///
/// Accepts strings like:
/// - `"G2 V"` — single star, subtype as inline digit, size as own token
/// - `"G2 V M9 V M6 V"` — three stars (e.g. Noricum)
/// - `"G2V"` — single token, subtype and size mashed together
/// - `"G V"` — no subtype digit; produces a `StarSpec` whose `subtype`
///   is `None` so the generator rolls it
/// - `"BD G2 V"` — `BD` (brown dwarf) tokens are silently skipped; the
///   library doesn't render them today
///
/// Tolerant of garbage: any token that doesn't start with a known
/// spectral letter (O/B/A/F/G/K/M, case-sensitive) is skipped, and
/// unknown size suffixes drop the entire star. An empty string returns
/// an empty `Vec`.
pub fn parse_stellar(s: &str) -> Vec<StarSpec> {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i];
        i += 1;
        if tok.eq_ignore_ascii_case("BD") {
            continue;
        }
        let bytes = tok.as_bytes();
        if bytes.is_empty() {
            continue;
        }
        let spectral = match bytes[0] {
            b'O' => StarType::O,
            b'B' => StarType::B,
            b'A' => StarType::A,
            b'F' => StarType::F,
            b'G' => StarType::G,
            b'K' => StarType::K,
            b'M' => StarType::M,
            _ => continue,
        };
        let mut subtype: Option<u8> = None;
        let mut j = 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            subtype = Some(subtype.unwrap_or(0) * 10 + (bytes[j] - b'0'));
            j += 1;
        }
        let size_str: String = if j < bytes.len() {
            std::str::from_utf8(&bytes[j..]).unwrap_or("").to_string()
        } else if i < tokens.len() && is_size_token(tokens[i]) {
            let s = tokens[i].to_string();
            i += 1;
            s
        } else {
            continue;
        };
        let size = match size_str.as_str() {
            "Ia" => StarSize::Ia,
            "Ib" => StarSize::Ib,
            "II" => StarSize::II,
            "III" => StarSize::III,
            "IV" => StarSize::IV,
            "V" => StarSize::V,
            "VI" => StarSize::VI,
            "D" => StarSize::D,
            _ => continue,
        };
        out.push(match subtype {
            Some(sub) => StarSpec::new(spectral, sub, size),
            None => StarSpec::with_rolled_subtype(spectral, size),
        });
    }
    out
}

fn is_size_token(s: &str) -> bool {
    matches!(s, "Ia" | "Ib" | "II" | "III" | "IV" | "V" | "VI" | "D")
}

#[cfg(test)]
mod parse_stellar_tests {
    use super::*;

    #[test]
    fn empty_string_yields_no_stars() {
        assert!(parse_stellar("").is_empty());
        assert!(parse_stellar("   ").is_empty());
    }

    #[test]
    fn single_split_token() {
        let s = parse_stellar("G2 V");
        assert_eq!(s.len(), 1);
        assert!(matches!(s[0].spectral, StarType::G));
        assert_eq!(s[0].subtype, Some(2));
        assert!(matches!(s[0].size, StarSize::V));
    }

    #[test]
    fn single_inline_token() {
        let s = parse_stellar("G2V");
        assert_eq!(s.len(), 1);
        assert!(matches!(s[0].spectral, StarType::G));
        assert_eq!(s[0].subtype, Some(2));
        assert!(matches!(s[0].size, StarSize::V));
    }

    #[test]
    fn three_stars_noricum() {
        let s = parse_stellar("G2 V M9 V M6 V");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].subtype, Some(2));
        assert_eq!(s[1].subtype, Some(9));
        assert_eq!(s[2].subtype, Some(6));
    }

    #[test]
    fn skips_brown_dwarf() {
        let s = parse_stellar("BD G2 V");
        assert_eq!(s.len(), 1);
        assert!(matches!(s[0].spectral, StarType::G));
    }

    #[test]
    fn no_subtype_uses_with_rolled_subtype() {
        let s = parse_stellar("G V");
        assert_eq!(s.len(), 1);
        assert!(matches!(s[0].spectral, StarType::G));
        assert_eq!(s[0].subtype, None);
        assert!(matches!(s[0].size, StarSize::V));
    }

    #[test]
    fn unknown_token_skipped() {
        // "Z" isn't a spectral letter; ignored. "G2 V" still parses.
        let s = parse_stellar("Z9 V G2 V");
        assert_eq!(s.len(), 1);
        assert!(matches!(s[0].spectral, StarType::G));
    }
}
