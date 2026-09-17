//! Constraint-driven system generation inputs.
//!
//! Today the system generator takes a single fully-specified main world
//! and rolls everything else around it. The constraint API generalises
//! that: callers describe whatever bodies they want to fix (a star type,
//! a planet at a particular orbit, a gas giant, a moon) and the
//! generator fills in the gaps. The classic "main world + UWP" call is
//! just the special case of a single `Planet { is_mainworld: true, .. }`
//! constraint with a fully-specified UWP.
//!
//! ## Wildcards in partial UWPs
//!
//! In a `PartialUwp` parsed via [`PartialUwp::parse`], the character
//! `'X'` (or `'x'`) in any column **except the starport** means "wild" —
//! the generator rolls this digit using the same per-orbit modifier table
//! it uses for full generation, and any sibling digits the user *did*
//! specify act as inputs to that roll.
//!
//! The **starport column** is the exception: `'X'` there is the literal
//! [`PortCode::X`] ("no starport"), matching the canonical Traveller
//! meaning and [`crate::systems::world::World::from_uwp`]. This is what
//! Traveller Map sends for frontier worlds, and a full UWP like
//! `X788899-A` must parse as a complete, generatable main world — not be
//! rejected as "wild port, hence partial". To leave the port for the
//! generator to roll, set `PartialUwp::port` to `None` directly (e.g. via
//! the UI dropdown) rather than typing `X`.

use crate::systems::gas_giant::GasGiantSize;
use crate::systems::system::{StarOrbit, StarSize, StarType};
use crate::trade::PortCode;

/// One column of a UWP, either user-specified or left for the generator.
///
/// Represented as `Option<T>` rather than a named enum so it composes
/// cleanly with the rest of the API (`unwrap_or`, `is_some`, etc.).
/// `None` = "wild": roll this digit during generation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartialUwp {
    pub port: Option<PortCode>,
    /// Size, as an `i8` rather than `u8` so the satellite codes fit: a
    /// rendered satellite UWP writes `S` for a body under ~1,000 km
    /// (internally -1) and `R` for a ring (0). Without the sign those
    /// round-trip as ehex 26 and 27, which `to_uwp` then renders as the
    /// literal "26" — a silently malformed nine-column UWP rather than an
    /// error.
    pub size: Option<i8>,
    pub atmosphere: Option<u8>,
    pub hydro: Option<u8>,
    pub population: Option<u8>,
    pub government: Option<u8>,
    pub law: Option<u8>,
    pub tech: Option<u8>,
}

impl PartialUwp {
    /// Parse a 9-character UWP string with `'X'` allowed for any column.
    ///
    /// Whitespace is stripped. The hyphen between law and tech is
    /// required. Returns the column-by-column breakdown, with `None`
    /// for any column the user left as `'X'`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        if cleaned.len() != 9 {
            return Err(format!(
                "UWP must be 9 chars including hyphen (e.g. \"A788899-A\"); got \"{cleaned}\" ({} chars)",
                cleaned.len(),
            ));
        }
        let chars: Vec<char> = cleaned.chars().collect();
        if chars[7] != '-' {
            return Err(format!(
                "UWP must have '-' at position 7 (between law and tech); got '{}' in \"{cleaned}\"",
                chars[7]
            ));
        }
        Ok(PartialUwp {
            port: parse_port(chars[0])?,
            size: parse_size(chars[1])?,
            atmosphere: parse_digit(chars[2])?,
            hydro: parse_digit(chars[3])?,
            population: parse_digit(chars[4])?,
            government: parse_digit(chars[5])?,
            law: parse_digit(chars[6])?,
            tech: parse_digit(chars[8])?,
        })
    }

    /// True if every column is specified — i.e. equivalent to a
    /// classic full UWP and safe to feed directly to
    /// [`crate::systems::world::World::from_uwp`].
    pub fn is_complete(&self) -> bool {
        self.port.is_some()
            && self.size.is_some()
            && self.atmosphere.is_some()
            && self.hydro.is_some()
            && self.population.is_some()
            && self.government.is_some()
            && self.law.is_some()
            && self.tech.is_some()
    }

    /// Format as a 9-char UWP string, using `'X'` for any wild column.
    pub fn to_string_with_wildcards(&self) -> String {
        // Size goes back out the way it came in, satellite codes included.
        fn size_d(v: Option<i8>) -> char {
            match v {
                Some(-1) => 'S',
                Some(n) if n >= 0 => crate::util::value_to_ehex(n as u32),
                Some(_) => 'X',
                None => 'X',
            }
        }
        fn d(v: Option<u8>) -> char {
            // Specified columns render in Traveller ehex (so 16 → `G`, not a
            // truncated hex digit); wild columns render as `X`.
            match v {
                Some(n) => crate::util::value_to_ehex(n as u32),
                None => 'X',
            }
        }
        // A wild port renders as `?`, not `X` — `X` is the literal "no
        // starport" code, so emitting it here would turn "unknown" into a
        // definite statement on the way back out.
        let port = self
            .port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "?".to_string());
        format!(
            "{}{}{}{}{}{}{}-{}",
            port,
            size_d(self.size),
            d(self.atmosphere),
            d(self.hydro),
            d(self.population),
            d(self.government),
            d(self.law),
            d(self.tech),
        )
    }
}

/// Size column parser. Accepts the two satellite codes `to_uwp` emits —
/// `S` (small, internally -1) and `R` (ring, 0) — so a UWP copied off a
/// rendered system map parses back to the body it describes. Everything
/// else is an ordinary ehex digit.
fn parse_size(c: char) -> Result<Option<i8>, String> {
    match c.to_ascii_uppercase() {
        'X' => Ok(None),
        'S' => Ok(Some(-1)),
        'R' => Ok(Some(0)),
        other => crate::util::ehex_to_value(other)
            .filter(|v| *v <= i8::MAX as u32)
            .map(|v| Some(v as i8))
            .ok_or_else(|| format!("invalid size digit '{c}'")),
    }
}

fn parse_digit(c: char) -> Result<Option<u8>, String> {
    if c == 'X' || c == 'x' {
        return Ok(None);
    }
    // Traveller ehex, so columns past `F` (15) — e.g. Tech Level `G` (16) —
    // parse instead of erroring out.
    crate::util::ehex_to_value(c)
        .map(|d| Some(d as u8))
        .ok_or_else(|| format!("invalid ehex digit '{c}'"))
}

fn parse_port(c: char) -> Result<Option<PortCode>, String> {
    // `X` in the port column is the literal "no starport" code
    // ([`PortCode::X`]), matching the canonical Traveller meaning and what
    // Traveller Map sends — NOT a wildcard. (The other columns still treat
    // `X` as wild; see the module-level docs.) Without this, every X-port
    // world parsed to a wild port, leaving the main-world UWP "incomplete"
    // and rejected at generation.
    //
    // Which leaves no way to write "this world has a starport and I don't
    // know its class" — a distinction the type has always supported
    // (`Option<PortCode>`) and the string format didn't. `?` fills that gap.
    // It matters for curated data: the Drinaxian Companion says Traefar has
    // "a small commercial spaceport" without giving a class, and spelling
    // that `X` would assert the opposite of what the source says.
    if c == '?' {
        return Ok(None);
    }
    Ok(Some(match c {
        'A' => PortCode::A,
        'B' => PortCode::B,
        'C' => PortCode::C,
        'D' => PortCode::D,
        'E' => PortCode::E,
        'X' | 'x' => PortCode::X,
        'Y' => PortCode::Y,
        'H' => PortCode::H,
        'G' => PortCode::G,
        'F' => PortCode::F,
        _ => return Err(format!("invalid port code '{c}'")),
    }))
}

/// One body the user has fixed in the system.
///
/// `Planet` covers both ordinary worlds and the main world (via the
/// `is_mainworld` flag) — there's no separate `MainWorld` variant.
/// Validation enforces ≤1 main world per system.
#[derive(Debug, Clone)]
pub enum Constraint {
    Star {
        /// `None` means "let the generator roll the orbit" — used by
        /// Traveller-Map autopopulation when the source data only tells
        /// us which stars exist, not where they sit. UI sets this to
        /// `Some(Primary)` or `Some(System(n))`.
        orbit: Option<StarOrbit>,
        spectral: Option<StarType>,
        /// Subtype digit 0-9 (e.g. the `4` in `F4 II`). `None` rolls.
        subtype: Option<u8>,
        size: Option<StarSize>,
        /// The star's own name.
        ///
        /// On a **companion** it also names that companion's sub-system, so
        /// bodies orbiting it derive their names from it. On the **primary**
        /// it is the star's name alone — the *system* is named separately
        /// (after its main world by default), because the two are usually
        /// but not always the same string. Sol and Terra; Fijari and Oghma.
        name: Option<String>,
    },
    Planet {
        name: Option<String>,
        orbit: Option<i32>,
        uwp: Option<PartialUwp>,
        num_satellites: Option<i32>,
        is_mainworld: bool,
    },
    GasGiant {
        name: Option<String>,
        orbit: Option<i32>,
        /// `None` = "either size, roll it" — the autopop case where
        /// PBG only tells us a giant exists, not its size class.
        size: Option<GasGiantSize>,
        num_satellites: Option<i32>,
    },
    Moon {
        name: Option<String>,
        parent_orbit: i32,
        uwp: Option<PartialUwp>,
    },
    /// A planetoid belt — always size 0. Same fields as a Planet
    /// otherwise; if the user supplies a `uwp` with a non-zero `size`
    /// column, the placement code overrides it back to 0.
    Belt {
        name: Option<String>,
        orbit: Option<i32>,
        uwp: Option<PartialUwp>,
        num_satellites: Option<i32>,
    },
    /// An explicitly empty (blocked) orbit. Orbit is required —
    /// validation rejects an Empty constraint without one.
    Empty { orbit: i32 },
}

/// All user-specified constraints for a single system generation.
#[derive(Debug, Clone, Default)]
pub struct SystemConstraints {
    pub bodies: Vec<Constraint>,
    /// Names the system, and through it most of its bodies: anything
    /// without a name of its own becomes "<this> <roman numeral>".
    ///
    /// A system-level field rather than a property of the primary star,
    /// because that's what it is — the primary's "name" and the system's
    /// name are the same string, and pretending otherwise would give two
    /// places to set one value. Companion stars are different: they carry
    /// their own names on [`Constraint::Star`], since each companion is its
    /// own sub-system whose bodies derive from it.
    ///
    /// `None` rolls from the name tables, as every system did before this
    /// field existed.
    pub system_name: Option<String>,
    /// Facts applied to the finished system rather than steering its
    /// generation — a body identified by relative position, a moon of a
    /// body whose orbit nothing pinned. See `systems::overrides::PostSpec`.
    ///
    /// They ride along here so every existing caller
    /// (`generate_system_png`, the SVG path, the validator) picks them up
    /// without changing its signature, and so the single
    /// `generate_from_constraints` entry point stays the only place a
    /// system is built.
    pub post: Vec<crate::systems::overrides::PostSpec>,
    /// Bodies belonging to the secondary star's own sub-system, with orbit
    /// numbers in *its* orbits rather than the primary's.
    ///
    /// Companions have always generated their own worlds and gas giants —
    /// `System` is recursive and `fill_system` runs on them — they just
    /// rolled everything randomly because no constraints reached them.
    pub secondary_bodies: Vec<Constraint>,
    /// The same for the tertiary.
    pub tertiary_bodies: Vec<Constraint>,
    /// Where the main world sits, when a source states it. `None` uses the
    /// habitable-zone default.
    pub main_world_orbit: Option<i32>,
    /// The main world's total satellite count. `None` rolls it.
    ///
    /// Counted as a *total*: a moon named explicitly in an override is
    /// deducted from this before it reaches the generator, so "one moon,
    /// called Baen" cannot come out as a rolled moon plus Baen.
    pub main_world_num_satellites: Option<i32>,
}

impl SystemConstraints {
    /// Build a constraints set equivalent to today's "main world name +
    /// UWP" call: a single `Planet` constraint flagged main-world.
    pub fn from_main_world(name: &str, uwp: &str) -> Result<Self, String> {
        Ok(SystemConstraints {
            system_name: None,
            post: Vec::new(),
            secondary_bodies: Vec::new(),
            tertiary_bodies: Vec::new(),
            main_world_orbit: None,
            main_world_num_satellites: None,
            bodies: vec![Constraint::Planet {
                name: Some(name.to_string()),
                orbit: None,
                uwp: Some(PartialUwp::parse(uwp)?),
                num_satellites: None,
                is_mainworld: true,
            }],
        })
    }

    /// Run static (star-independent) checks on the constraint set.
    ///
    /// Returns every error found, not just the first — callers want to
    /// surface all problems at once so the user can fix them in one
    /// pass. Star-dependent checks (illegal orbit for the chosen star)
    /// happen during generation.
    pub fn validate(&self) -> Vec<ConstraintError> {
        let mut errors = Vec::new();

        let main_world_count = self
            .bodies
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Constraint::Planet {
                        is_mainworld: true,
                        ..
                    }
                )
            })
            .count();
        if main_world_count > 1 {
            errors.push(ConstraintError::MultipleMainWorlds(main_world_count));
        }

        let mut seen_orbits = std::collections::BTreeSet::new();
        for c in &self.bodies {
            let orbit = match c {
                Constraint::Planet { orbit: Some(o), .. } => Some(*o),
                Constraint::GasGiant { orbit: Some(o), .. } => Some(*o),
                Constraint::Belt { orbit: Some(o), .. } => Some(*o),
                Constraint::Empty { orbit } => Some(*orbit),
                _ => None,
            };
            if let Some(o) = orbit
                && !seen_orbits.insert(o)
            {
                errors.push(ConstraintError::DuplicateOrbit(o));
            }
        }

        for c in &self.bodies {
            let uwp = match c {
                Constraint::Planet { uwp: Some(p), .. } => Some(p),
                Constraint::Moon { uwp: Some(p), .. } => Some(p),
                Constraint::Belt { uwp: Some(p), .. } => Some(p),
                _ => None,
            };
            if let Some(p) = uwp
                && let Some(err) = check_uwp_consistency(p)
            {
                errors.push(err);
            }
        }

        errors
    }

    pub fn main_world(&self) -> Option<&Constraint> {
        self.bodies.iter().find(|c| {
            matches!(
                c,
                Constraint::Planet {
                    is_mainworld: true,
                    ..
                }
            )
        })
    }
}

/// Reject combinations that can never be coherent regardless of how
/// the unspecified columns are rolled. Only fires when the offending
/// pair is fully specified — partials are filled in around their
/// siblings, so they're consistent by construction.
fn check_uwp_consistency(p: &PartialUwp) -> Option<ConstraintError> {
    if let (Some(size), Some(hydro)) = (p.size, p.hydro)
        && size == 0
        && hydro > 0
    {
        return Some(ConstraintError::ContradictoryUwp(format!(
            "size 0 cannot have hydro > 0 (got hydro={hydro})"
        )));
    }
    if let (Some(atm), Some(hydro)) = (p.atmosphere, p.hydro)
        && (atm <= 1 || atm >= 10)
        && hydro >= 10
    {
        return Some(ConstraintError::ContradictoryUwp(format!(
            "atmosphere {atm} is incompatible with hydro {hydro}"
        )));
    }
    None
}

#[derive(Debug, Clone)]
pub enum ConstraintError {
    MultipleMainWorlds(usize),
    DuplicateOrbit(i32),
    ContradictoryUwp(String),
    IllegalOrbit { orbit: i32, reason: String },
    MoonMissingParent(i32),
    UnsupportedYet(String),
}

impl std::fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintError::MultipleMainWorlds(n) => {
                write!(f, "{n} main worlds specified; at most 1 allowed")
            }
            ConstraintError::DuplicateOrbit(o) => {
                write!(f, "orbit {o} specified more than once")
            }
            ConstraintError::ContradictoryUwp(s) => write!(f, "contradictory UWP: {s}"),
            ConstraintError::IllegalOrbit { orbit, reason } => {
                write!(f, "orbit {orbit} is illegal: {reason}")
            }
            ConstraintError::MoonMissingParent(o) => {
                write!(
                    f,
                    "moon constraint references non-existent parent orbit {o}"
                )
            }
            ConstraintError::UnsupportedYet(s) => write!(f, "not yet supported: {s}"),
        }
    }
}

impl std::error::Error for ConstraintError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The satellite size codes must survive a parse/render round-trip.
    ///
    /// `to_uwp` writes `S` for a satellite under ~1,000 km and `R` for a
    /// ring. Before `parse_size` existed both went through `ehex_to_value`,
    /// where `S` is 26 and `R` is 27 — so `GS00169-A` parsed to size 26 and
    /// rendered back as the literal `"26"`, a malformed nine-column UWP,
    /// with no error anywhere. Baen is exactly that UWP.
    #[test]
    fn satellite_size_codes_round_trip() {
        let baen = PartialUwp::parse("GS00169-A").expect("parses");
        assert_eq!(baen.size, Some(-1), "S is the small-satellite code");
        assert_eq!(baen.to_string_with_wildcards(), "GS00169-A");

        let ring = PartialUwp::parse("?RXXXXX-X").expect("parses");
        assert_eq!(ring.size, Some(0), "R is a ring");

        // An ordinary digit is untouched, and a wild size still renders X.
        assert_eq!(PartialUwp::parse("A9B2887-A").unwrap().size, Some(9));
        assert_eq!(
            PartialUwp::parse("?XXXXXX-X").unwrap().to_string_with_wildcards(),
            "?XXXXXX-X"
        );
    }

    #[test]
    fn parse_full_uwp() {
        let p = PartialUwp::parse("A788899-A").unwrap();
        assert!(p.is_complete());
        assert_eq!(p.port, Some(PortCode::A));
        assert_eq!(p.size, Some(7));
        assert_eq!(p.atmosphere, Some(8));
        assert_eq!(p.tech, Some(0xA));
    }

    #[test]
    fn parse_partial_uwp() {
        let p = PartialUwp::parse("HXXXXXX-X").unwrap();
        assert!(!p.is_complete());
        assert_eq!(p.port, Some(PortCode::H));
        assert_eq!(p.size, None);
        assert_eq!(p.atmosphere, None);
        assert_eq!(p.tech, None);
    }

    #[test]
    fn parse_x_port_is_literal_no_starport() {
        // `X` in the starport column is the literal "no starport" code, not
        // a wildcard — so an otherwise-full UWP is complete and generatable.
        let p = PartialUwp::parse("X788899-A").unwrap();
        assert_eq!(p.port, Some(PortCode::X));
        assert_eq!(p.size, Some(7));
        assert!(p.is_complete(), "X-port full UWP must be complete");

        let lower = PartialUwp::parse("x788899-A").unwrap();
        assert_eq!(lower.port, Some(PortCode::X));
    }

    #[test]
    fn parse_wild_non_port_columns() {
        // `X` still means wild in the non-port columns.
        let p = PartialUwp::parse("AXXXXXX-X").unwrap();
        assert_eq!(p.port, Some(PortCode::A));
        assert_eq!(p.size, None);
        assert_eq!(p.tech, None);
        assert!(!p.is_complete());
    }

    #[test]
    fn parse_ehex_tech_level_g() {
        // Tech Level G = 16 must parse (ehex), not error as an invalid hex
        // digit, and round-trip back to `G`.
        let p = PartialUwp::parse("A788899-G").unwrap();
        assert_eq!(p.tech, Some(16));
        assert!(p.is_complete());
        assert_eq!(p.to_string_with_wildcards(), "A788899-G");
    }

    #[test]
    fn parse_rejects_wrong_length() {
        assert!(PartialUwp::parse("A78-A").is_err());
        assert!(PartialUwp::parse("A788899A").is_err());
    }

    #[test]
    fn parse_rejects_missing_hyphen() {
        assert!(PartialUwp::parse("A7888990A").is_err());
    }

    #[test]
    fn validate_catches_multiple_mainworlds() {
        let cs = SystemConstraints {
            bodies: vec![
                Constraint::Planet {
                    name: Some("a".into()),
                    orbit: None,
                    uwp: None,
                    num_satellites: None,
                    is_mainworld: true,
                },
                Constraint::Planet {
                    name: Some("b".into()),
                    orbit: None,
                    uwp: None,
                    num_satellites: None,
                    is_mainworld: true,
                },
            ],
            system_name: None,
            post: Vec::new(),
            secondary_bodies: Vec::new(),
            tertiary_bodies: Vec::new(),
            main_world_orbit: None,
            main_world_num_satellites: None,
        };
        let errs = cs.validate();
        assert!(
            errs.iter()
                .any(|e| matches!(e, ConstraintError::MultipleMainWorlds(_)))
        );
    }

    #[test]
    fn validate_catches_duplicate_orbits() {
        let cs = SystemConstraints {
            bodies: vec![
                Constraint::Planet {
                    name: None,
                    orbit: Some(3),
                    uwp: None,
                    num_satellites: None,
                    is_mainworld: false,
                },
                Constraint::GasGiant {
                    name: None,
                    orbit: Some(3),
                    size: Some(GasGiantSize::Large),
                    num_satellites: None,
                },
            ],
            system_name: None,
            post: Vec::new(),
            secondary_bodies: Vec::new(),
            tertiary_bodies: Vec::new(),
            main_world_orbit: None,
            main_world_num_satellites: None,
        };
        let errs = cs.validate();
        assert!(
            errs.iter()
                .any(|e| matches!(e, ConstraintError::DuplicateOrbit(3)))
        );
    }

    #[test]
    fn validate_catches_contradictory_uwp() {
        let cs = SystemConstraints {
            bodies: vec![Constraint::Planet {
                name: None,
                orbit: None,
                // size=0 (asteroid) but hydro=A is impossible
                uwp: Some(PartialUwp::parse("A0AA000-0").unwrap()),
                num_satellites: None,
                is_mainworld: true,
            }],
            system_name: None,
            post: Vec::new(),
            secondary_bodies: Vec::new(),
            tertiary_bodies: Vec::new(),
            main_world_orbit: None,
            main_world_num_satellites: None,
        };
        let errs = cs.validate();
        assert!(
            errs.iter()
                .any(|e| matches!(e, ConstraintError::ContradictoryUwp(_)))
        );
    }

    #[test]
    fn from_main_world_builds_expected_shape() {
        let cs = SystemConstraints::from_main_world("Regina", "A788899-A").unwrap();
        assert_eq!(cs.bodies.len(), 1);
        match &cs.bodies[0] {
            Constraint::Planet {
                is_mainworld: true,
                uwp: Some(p),
                name: Some(n),
                ..
            } => {
                assert_eq!(n, "Regina");
                assert!(p.is_complete());
            }
            _ => panic!("expected single mainworld Planet constraint"),
        }
    }
}
