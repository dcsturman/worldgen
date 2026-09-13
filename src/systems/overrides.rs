//! Curated per-system facts that generation must honour.
//!
//! TravellerMap gives us a main world's UWP, its PBG digits and its stellar
//! data — and nothing else. Published adventures give far more: the name of a
//! system, a secondary world's UWP, which orbit the gas giant sits in. That
//! material has nowhere to live in the upstream data, so it lives here, keyed
//! by `(sector, hex)` and merged into the constraints at generation time.
//!
//! ## Why a separate type from [`Constraint`]
//!
//! These are deliberately *not* serde derives on `Constraint`. Two reasons.
//!
//! The file is hand-written, so it gets hand-friendly spellings: a star is
//! `"class": "M5 V"` rather than three enum fields, a UWP is the string you'd
//! read off a page. And the file format then stops tracking the internal
//! representation, so refactoring `Constraint` doesn't invalidate data.
//!
//! The second reason matters more: there is **no way to spell a main-world
//! override**. The main world's UWP is upstream data, and an override that
//! contradicted it would produce a system disagreeing with the map everyone
//! else sees. Omitting `is_mainworld` from the schema makes that unsayable
//! rather than merely discouraged.

use serde::{Deserialize, Serialize};

use crate::systems::constraint::{Constraint, PartialUwp};
use crate::systems::gas_giant::GasGiantSize;
use crate::systems::system::StarOrbit;

/// The whole override file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverrideFile {
    pub overrides: Vec<SystemOverride>,
}

/// Everything we know about one system beyond what TravellerMap carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemOverride {
    /// Sector name as TravellerMap spells it, e.g. "Trojan Reach".
    pub sector: String,
    /// Four-digit hex, e.g. "2324".
    pub hex: String,
    /// The main world's name at that hex.
    ///
    /// Redundant with upstream data, and that is the point: the validator
    /// asserts TravellerMap agrees. A mistyped hex is the likeliest authoring
    /// error — you're copying coordinates off a page — and it is otherwise
    /// invisible, since the override simply applies to nothing, or to
    /// somebody else's system.
    pub world: String,
    /// Where this came from. Six months on, "which adventure said that?" has
    /// no other answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Names the system, and through it every body without a name of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bodies: Vec<BodySpec>,
}

/// A star's orbit, spelled the way a person would: `"primary"`, `"far"`, or
/// an orbit number.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StarOrbitSpec {
    Named(String),
    Index(usize),
}

/// One body in an override.
///
/// `deny_unknown_fields` throughout: a mistyped key must fail the parse
/// rather than be ignored. Silently discarding `"orbits": 5` because the
/// field is called `orbit` would reproduce exactly the failure this whole
/// effort exists to remove.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BodySpec {
    /// A companion star. The primary is described by the system's stellar
    /// data; naming it means naming the system (`system_name`).
    Star {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        orbit: Option<StarOrbitSpec>,
        /// Spectral class as written on the page, e.g. `"M5 V"`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        class: Option<String>,
    },
    Planet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        orbit: Option<i32>,
        /// UWP with `X` for any column you don't know.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uwp: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moons: Option<i32>,
    },
    Belt {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        orbit: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uwp: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moons: Option<i32>,
    },
    GasGiant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        orbit: Option<i32>,
        /// `"small"` or `"large"`; omit to let the generator roll it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moons: Option<i32>,
    },
    Moon {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        parent_orbit: i32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uwp: Option<String>,
    },
    /// An orbit known to be empty.
    Empty { orbit: i32 },
}

impl BodySpec {
    /// Lower one body to the generator's own constraint type.
    pub fn to_constraint(&self) -> Result<Constraint, String> {
        let uwp = |u: &Option<String>| -> Result<Option<PartialUwp>, String> {
            u.as_deref().map(PartialUwp::parse).transpose()
        };
        Ok(match self {
            BodySpec::Star { name, orbit, class } => {
                let orbit = match orbit {
                    None => None,
                    Some(StarOrbitSpec::Index(n)) => Some(StarOrbit::System(*n)),
                    Some(StarOrbitSpec::Named(s)) if s.eq_ignore_ascii_case("primary") => {
                        Some(StarOrbit::Primary)
                    }
                    Some(StarOrbitSpec::Named(s)) if s.eq_ignore_ascii_case("far") => {
                        Some(StarOrbit::Far)
                    }
                    Some(StarOrbitSpec::Named(s)) => {
                        return Err(format!(
                            "star orbit must be \"primary\", \"far\" or a number; got \"{s}\""
                        ));
                    }
                };
                // Reuse the same parser the stellar column goes through, so a
                // class written in an override means exactly what it means
                // coming from TravellerMap.
                let spec = class
                    .as_deref()
                    .map(|c| {
                        crate::api::parse_stellar(c).into_iter().next().ok_or_else(|| {
                            format!("could not parse star class \"{c}\"")
                        })
                    })
                    .transpose()?;
                Constraint::Star {
                    orbit,
                    spectral: spec.as_ref().map(|s| s.spectral),
                    subtype: spec.as_ref().and_then(|s| s.subtype),
                    size: spec.as_ref().map(|s| s.size),
                    name: name.clone(),
                }
            }
            BodySpec::Planet {
                name,
                orbit,
                uwp: u,
                moons,
            } => Constraint::Planet {
                name: name.clone(),
                orbit: *orbit,
                uwp: uwp(u)?,
                num_satellites: *moons,
                // Never settable from an override; see the module docs.
                is_mainworld: false,
            },
            BodySpec::Belt {
                name,
                orbit,
                uwp: u,
                moons,
            } => Constraint::Belt {
                name: name.clone(),
                orbit: *orbit,
                uwp: uwp(u)?,
                num_satellites: *moons,
            },
            BodySpec::GasGiant {
                name,
                orbit,
                size,
                moons,
            } => {
                let size = match size.as_deref() {
                    None => None,
                    Some(s) if s.eq_ignore_ascii_case("small") => Some(GasGiantSize::Small),
                    Some(s) if s.eq_ignore_ascii_case("large") => Some(GasGiantSize::Large),
                    Some(s) => {
                        return Err(format!(
                            "gas giant size must be \"small\" or \"large\"; got \"{s}\""
                        ));
                    }
                };
                Constraint::GasGiant {
                    name: name.clone(),
                    orbit: *orbit,
                    size,
                    num_satellites: *moons,
                }
            }
            BodySpec::Moon {
                name,
                parent_orbit,
                uwp: u,
            } => Constraint::Moon {
                name: name.clone(),
                parent_orbit: *parent_orbit,
                uwp: uwp(u)?,
            },
            BodySpec::Empty { orbit } => Constraint::Empty { orbit: *orbit },
        })
    }
}

impl SystemOverride {
    /// Fold this override into constraints already built from TravellerMap
    /// data (main world, stellar column, PBG digits).
    ///
    /// The rule is: **the override wins, and PBG counts mean "at least"**.
    ///
    /// For gas giants, belts and ordinary planets, the upstream data
    /// contributes only a *count* — `build_constraints` pushes that many
    /// anonymous, orbit-less bodies. An override body of the same kind is a
    /// better-specified instance of one of those, so it replaces one rather
    /// than adding to the total: PBG 2 gas giants plus one pinned to orbit 5
    /// gives two giants, one of them at orbit 5. Supply more than PBG claims
    /// and you get all of them — you know something the digit doesn't.
    ///
    /// Stars work differently, because stellar data already describes each
    /// companion; an override naming one means *that* star, not an extra one.
    /// So override stars patch the existing companions in order, field by
    /// field, and only spill over into new companions if you list more than
    /// the stellar column does.
    pub fn merge_into(
        &self,
        mut cs: crate::systems::constraint::SystemConstraints,
    ) -> Result<crate::systems::constraint::SystemConstraints, String> {
        if let Some(n) = &self.system_name {
            cs.system_name = Some(n.clone());
        }

        let mine: Vec<Constraint> = self
            .bodies
            .iter()
            .map(|b| b.to_constraint())
            .collect::<Result<_, _>>()?;

        // --- stars: patch companions positionally ---
        let (my_stars, my_bodies): (Vec<_>, Vec<_>) = mine
            .into_iter()
            .partition(|c| matches!(c, Constraint::Star { .. }));
        let mut companion_idx = 0usize;
        let mut patched = 0usize;
        for c in cs.bodies.iter_mut() {
            let Constraint::Star {
                orbit,
                spectral,
                subtype,
                size,
                name,
            } = c
            else {
                continue;
            };
            // The primary is not a companion and is never patched here; its
            // name is the system's name.
            if matches!(orbit, Some(StarOrbit::Primary)) {
                continue;
            }
            let Some(Constraint::Star {
                orbit: o2,
                spectral: sp2,
                subtype: st2,
                size: sz2,
                name: n2,
            }) = my_stars.get(companion_idx)
            else {
                break;
            };
            if o2.is_some() {
                *orbit = *o2;
            }
            if sp2.is_some() {
                *spectral = *sp2;
            }
            if st2.is_some() {
                *subtype = *st2;
            }
            if sz2.is_some() {
                *size = *sz2;
            }
            if n2.is_some() {
                *name = n2.clone();
            }
            companion_idx += 1;
            patched += 1;
        }
        // More override stars than the stellar column described: the extras
        // are genuinely new companions.
        for extra in my_stars.into_iter().skip(patched) {
            cs.bodies.push(extra);
        }

        // --- everything else: replace an anonymous PBG body of the same kind ---
        for c in my_bodies {
            let kind = discriminant_of(&c);
            if let Some(pos) = cs.bodies.iter().position(|e| {
                discriminant_of(e) == kind && is_anonymous_pbg_filler(e)
            }) {
                cs.bodies[pos] = c;
            } else {
                cs.bodies.push(c);
            }
        }

        Ok(cs)
    }
}

/// Which kind of body a constraint is, for match-up during the merge.
fn discriminant_of(c: &Constraint) -> u8 {
    match c {
        Constraint::Star { .. } => 0,
        Constraint::Planet { .. } => 1,
        Constraint::Belt { .. } => 2,
        Constraint::GasGiant { .. } => 3,
        Constraint::Moon { .. } => 4,
        Constraint::Empty { .. } => 5,
    }
}

/// True for the placeholder bodies `build_constraints` pushes to represent a
/// PBG digit: no name, no orbit, no UWP, nothing but "one of these exists".
///
/// Only those are eligible to be replaced by an override. Anything carrying
/// real information — including the main world — is left alone, so a merge
/// can't quietly overwrite a fact that came from somewhere else.
fn is_anonymous_pbg_filler(c: &Constraint) -> bool {
    match c {
        Constraint::Planet {
            name,
            orbit,
            uwp,
            num_satellites,
            is_mainworld,
        } => {
            !*is_mainworld
                && name.is_none()
                && orbit.is_none()
                && uwp.is_none()
                && num_satellites.is_none()
        }
        Constraint::Belt {
            name,
            orbit,
            uwp,
            num_satellites,
        } => name.is_none() && orbit.is_none() && uwp.is_none() && num_satellites.is_none(),
        Constraint::GasGiant {
            name,
            orbit,
            size,
            num_satellites,
        } => name.is_none() && orbit.is_none() && size.is_none() && num_satellites.is_none(),
        _ => false,
    }
}

/// Canonical lookup key for a system.
///
/// Sector names arrive spelled however the caller spells them, and
/// `seed::system_seed` already lowercases and trims before hashing — so the
/// override key has to agree with that or an override would apply to a
/// differently-seeded system. Whitespace is collapsed as well, since
/// `"Trojan  Reach"` and `"Trojan Reach"` are obviously the same sector to a
/// person and obviously different to a hasher.
pub fn canonical_key(sector: &str, hex: &str) -> String {
    let sector: String = sector.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}/{}", sector.to_lowercase(), hex.trim().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "overrides": [
        { "sector": "Trojan Reach", "hex": "2324", "world": "Pourne",
          "note": "Adventure: Flatlined, p.42",
          "system_name": "Merak Mists",
          "bodies": [
            { "type": "star", "name": "Kepler's Lantern", "orbit": "far", "class": "M9 V" },
            { "type": "gas_giant", "orbit": 5, "size": "large" },
            { "type": "belt", "orbit": 7 },
            { "type": "planet", "orbit": 2, "name": "Novastron", "uwp": "X4A0000-0" },
            { "type": "moon", "parent_orbit": 5, "name": "Sulatra" },
            { "type": "empty", "orbit": 1 }
          ] }
      ] }"#;

    #[test]
    fn sample_file_parses_and_lowers() {
        let f: OverrideFile = serde_json::from_str(SAMPLE).expect("parses");
        assert_eq!(f.overrides.len(), 1);
        let o = &f.overrides[0];
        assert_eq!(o.world, "Pourne");
        assert_eq!(o.system_name.as_deref(), Some("Merak Mists"));
        assert_eq!(o.bodies.len(), 6);
        for b in &o.bodies {
            b.to_constraint().expect("lowers to a constraint");
        }
    }

    /// A mistyped key must fail the parse. Ignoring it would put us straight
    /// back to overrides that silently don't apply.
    #[test]
    fn unknown_fields_are_rejected() {
        let bad = r#"{ "overrides": [ { "sector": "Trojan Reach", "hex": "2324",
            "world": "Pourne", "bodies": [ { "type": "gas_giant", "orbits": 5 } ] } ] }"#;
        let err = serde_json::from_str::<OverrideFile>(bad).unwrap_err().to_string();
        assert!(err.contains("orbits"), "error should name the bad key: {err}");
    }

    /// An override cannot claim to be the main world — the schema has no way
    /// to say it, and the lowering hard-codes the flag off.
    #[test]
    fn overrides_can_never_set_the_main_world() {
        let f: OverrideFile = serde_json::from_str(SAMPLE).unwrap();
        for b in &f.overrides[0].bodies {
            if let Constraint::Planet { is_mainworld, .. } = b.to_constraint().unwrap() {
                assert!(!is_mainworld);
            }
        }
    }

    #[test]
    fn star_class_parses_like_the_stellar_column() {
        let b = BodySpec::Star {
            name: None,
            orbit: Some(StarOrbitSpec::Index(4)),
            class: Some("M5 V".into()),
        };
        match b.to_constraint().unwrap() {
            Constraint::Star {
                orbit,
                spectral,
                subtype,
                size,
                ..
            } => {
                assert_eq!(orbit, Some(StarOrbit::System(4)));
                assert!(spectral.is_some());
                assert_eq!(subtype, Some(5));
                assert!(size.is_some());
            }
            other => panic!("expected a Star, got {other:?}"),
        }
    }

    #[test]
    fn bad_values_are_reported_not_swallowed() {
        let bad_size = BodySpec::GasGiant {
            name: None,
            orbit: None,
            size: Some("enormous".into()),
            moons: None,
        };
        assert!(bad_size.to_constraint().unwrap_err().contains("enormous"));

        let bad_orbit = BodySpec::Star {
            name: None,
            orbit: Some(StarOrbitSpec::Named("middle".into())),
            class: None,
        };
        assert!(bad_orbit.to_constraint().unwrap_err().contains("middle"));
    }

    use crate::api::{StarSpec, build_constraints, parse_stellar};

    fn pourne_constraints(ggs: usize, belts: usize, planets: usize) -> crate::systems::constraint::SystemConstraints {
        let stars: Vec<StarSpec> = parse_stellar("F3 V M9 V");
        build_constraints("Pourne", "A9B2887-A", &stars, ggs, belts, planets).unwrap()
    }

    fn count(cs: &crate::systems::constraint::SystemConstraints, kind: u8) -> usize {
        cs.bodies.iter().filter(|c| discriminant_of(c) == kind).count()
    }

    /// PBG says how many; an override says which. A pinned gas giant is one
    /// of the two the digit promised, not a third.
    #[test]
    fn an_override_body_replaces_a_pbg_placeholder_rather_than_adding() {
        let cs = pourne_constraints(2, 0, 0);
        assert_eq!(count(&cs, 3), 2, "PBG contributed two giants");

        let ov = SystemOverride {
            sector: "Trojan Reach".into(),
            hex: "2324".into(),
            world: "Pourne".into(),
            note: None,
            system_name: None,
            bodies: vec![BodySpec::GasGiant {
                name: None,
                orbit: Some(5),
                size: Some("large".into()),
                moons: None,
            }],
        };
        let merged = ov.merge_into(cs).unwrap();
        assert_eq!(count(&merged, 3), 2, "still two giants, not three");
        assert!(
            merged.bodies.iter().any(|c| matches!(
                c,
                Constraint::GasGiant { orbit: Some(5), .. }
            )),
            "one of them is pinned to orbit 5"
        );
    }

    /// Knowing more than the digit does is allowed — the override wins.
    #[test]
    fn more_override_bodies_than_pbg_claims_are_all_kept() {
        let cs = pourne_constraints(1, 0, 0);
        let ov = SystemOverride {
            sector: "Trojan Reach".into(),
            hex: "2324".into(),
            world: "Pourne".into(),
            note: None,
            system_name: None,
            bodies: vec![
                BodySpec::GasGiant { name: None, orbit: Some(5), size: None, moons: None },
                BodySpec::GasGiant { name: None, orbit: Some(9), size: None, moons: None },
            ],
        };
        let merged = ov.merge_into(cs).unwrap();
        assert_eq!(count(&merged, 3), 2, "both pinned giants survive");
    }

    /// Naming a companion means *that* companion, not a new star. Stellar
    /// data already said how many there are.
    #[test]
    fn an_override_star_patches_the_companion_instead_of_adding_one() {
        let cs = pourne_constraints(0, 0, 0);
        assert_eq!(count(&cs, 0), 2, "stellar column gave a primary and one companion");

        let ov = SystemOverride {
            sector: "Trojan Reach".into(),
            hex: "2324".into(),
            world: "Pourne".into(),
            note: None,
            system_name: Some("Merak Mists".into()),
            bodies: vec![BodySpec::Star {
                name: Some("Kepler's Lantern".into()),
                orbit: Some(StarOrbitSpec::Named("far".into())),
                class: None,
            }],
        };
        let merged = ov.merge_into(cs).unwrap();
        assert_eq!(count(&merged, 0), 2, "still two stars, not three");
        assert_eq!(merged.system_name.as_deref(), Some("Merak Mists"));

        let named = merged.bodies.iter().find_map(|c| match c {
            Constraint::Star { name: Some(n), orbit, spectral, .. } => Some((n.clone(), *orbit, *spectral)),
            _ => None,
        });
        let (name, orbit, spectral) = named.expect("a companion carries the name");
        assert_eq!(name, "Kepler's Lantern");
        assert_eq!(orbit, Some(StarOrbit::Far), "override orbit wins");
        assert!(
            spectral.is_some(),
            "class came from the stellar column, which the override didn't contradict"
        );
    }

    /// The main world is upstream data. A merge must not be able to touch it,
    /// even when the override supplies a planet with no other home.
    #[test]
    fn the_main_world_is_never_replaced_by_a_merge() {
        let cs = pourne_constraints(0, 0, 0); // no anonymous planets at all
        let ov = SystemOverride {
            sector: "Trojan Reach".into(),
            hex: "2324".into(),
            world: "Pourne".into(),
            note: None,
            system_name: None,
            bodies: vec![BodySpec::Planet {
                name: Some("Novastron".into()),
                orbit: Some(2),
                uwp: Some("X4A0000-0".into()),
                moons: None,
            }],
        };
        let merged = ov.merge_into(cs).unwrap();
        let mains: Vec<_> = merged
            .bodies
            .iter()
            .filter(|c| matches!(c, Constraint::Planet { is_mainworld: true, .. }))
            .collect();
        assert_eq!(mains.len(), 1, "exactly one main world, still Pourne");
        assert!(matches!(
            mains[0],
            Constraint::Planet { name: Some(n), .. } if n == "Pourne"
        ));
        assert_eq!(count(&merged, 1), 2, "Novastron was added alongside it");
    }

    #[test]
    fn canonical_key_ignores_spelling_noise() {
        assert_eq!(
            canonical_key("Trojan  Reach ", "2324"),
            canonical_key("trojan reach", "2324")
        );
        assert_ne!(
            canonical_key("Trojan Reach", "2324"),
            canonical_key("Trojan Reach", "2325")
        );
    }
}
