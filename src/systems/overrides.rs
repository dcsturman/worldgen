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
    /// Free-text header so the file explains itself to whoever opens it.
    /// Named with a leading underscore to read as "not data".
    #[serde(default, rename = "_comment", skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
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

fn is_primary_star(s: &StarRef) -> bool {
    matches!(s, StarRef::Primary)
}

/// Which star a body orbits.
///
/// Orbit numbers are *that star's* orbits. Makergod's Oghma table reads
/// "Secondary Star Doruc — orbit 1 Khazha", meaning Doruc's orbit 1, not the
/// primary's: a companion is its own system with its own orbit numbering,
/// which is exactly how the generator already models it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarRef {
    #[default]
    Primary,
    Secondary,
    Tertiary,
}

/// Where a body sits, said the way a source says it.
///
/// Books describe position relatively — "the next world out from Torpol",
/// "the system's outer gas giant" — and an absolute orbit number is a
/// *derived* fact that has to be re-derived whenever generation shifts.
/// Recording the relationship instead keeps the file saying what the page
/// says.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PositionSpec {
    /// The outermost body of this kind in the system.
    Outermost,
    /// The innermost body of this kind.
    Innermost,
    /// The first body of this kind orbiting beyond the named one.
    After(String),
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
        /// Which star this body orbits; defaults to the primary.
        #[serde(default, skip_serializing_if = "is_primary_star")]
        star: StarRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        orbit: Option<i32>,
        /// Relative position, for when the source says "the next world out"
        /// rather than a number. Mutually exclusive with `orbit`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position: Option<PositionSpec>,
        /// UWP with `X` for any column you don't know.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uwp: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moons: Option<i32>,
        /// Bases and installations: naval, scout, farming, mining, colony,
        /// lab, military. Sources name these constantly ("Sternmetal
        /// Horizons has a mining base in the inner belt") and a UWP has
        /// nowhere to put them.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        facilities: Vec<String>,
        /// Travel zone for this body: "green", "amber" or "red". Per body,
        /// because an interdicted moon in an otherwise green system is a
        /// thing sources describe.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        zone: Option<String>,
    },
    Belt {
        /// Which star this body orbits; defaults to the primary.
        #[serde(default, skip_serializing_if = "is_primary_star")]
        star: StarRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        orbit: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position: Option<PositionSpec>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uwp: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moons: Option<i32>,
        /// Bases and installations: naval, scout, farming, mining, colony,
        /// lab, military. Sources name these constantly ("Sternmetal
        /// Horizons has a mining base in the inner belt") and a UWP has
        /// nowhere to put them.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        facilities: Vec<String>,
        /// Travel zone for this body: "green", "amber" or "red". Per body,
        /// because an interdicted moon in an otherwise green system is a
        /// thing sources describe.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        zone: Option<String>,
    },
    GasGiant {
        /// Which star this body orbits; defaults to the primary.
        #[serde(default, skip_serializing_if = "is_primary_star")]
        star: StarRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        orbit: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position: Option<PositionSpec>,
        /// `"small"` or `"large"`; omit to let the generator roll it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        size: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moons: Option<i32>,
    },
    Moon {
        /// Which star this body orbits; defaults to the primary.
        #[serde(default, skip_serializing_if = "is_primary_star")]
        star: StarRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// The parent's orbit number. Use `parent` instead when the source
        /// names the body rather than numbering it — "a moon of Bulhai".
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_orbit: Option<i32>,
        /// The parent body's name. Resolved after generation, so it works
        /// for bodies whose orbit nothing pinned.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
        /// This moon's own orbit around its parent, in the satellite
        /// numbering the generator already uses (close orbits are small
        /// numbers, far orbits multiples of five, extreme ones multiples of
        /// twenty-five). Source tables give these — Ra-La-Lantra's moons sit
        /// at 0, 8, 11, 27 and 34 — and without it the roll picks.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        satellite_orbit: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        uwp: Option<String>,
        /// Bases and installations: naval, scout, farming, mining, colony,
        /// lab, military. Sources name these constantly ("Sternmetal
        /// Horizons has a mining base in the inner belt") and a UWP has
        /// nowhere to put them.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        facilities: Vec<String>,
        /// Travel zone for this body: "green", "amber" or "red". Per body,
        /// because an interdicted moon in an otherwise green system is a
        /// thing sources describe.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        zone: Option<String>,
    },
    /// An orbit known to be empty.
    Empty { orbit: i32 },
}

/// What a body in an override turns into.
///
/// Two kinds, split by *when the fact can be known*. An absolute orbit or a
/// UWP steers generation and has to be in hand before it runs. A relative
/// position — "the outermost gas giant" — cannot be resolved until the
/// system exists, and neither can "a moon of Bulhai" when nothing pinned
/// Bulhai's orbit. Those are applied to the finished system instead.
#[derive(Debug)]
pub enum Lowered {
    /// Feeds the generator.
    Constraint(Constraint),
    /// Applied to the generated system afterwards.
    Post(PostSpec),
}

/// Which kind of body a [`PostSpec`] is looking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostKind {
    Planet,
    Belt,
    GasGiant,
}

/// How a [`PostSpec`] finds the body it describes.
#[derive(Debug, Clone)]
pub enum Target {
    /// The outermost body of this kind.
    Outermost(PostKind),
    /// The innermost body of this kind.
    Innermost(PostKind),
    /// The first body of this kind beyond the named one.
    After { kind: PostKind, body: String },
    /// A satellite of the named body.
    MoonOf(String),
    /// Whatever sits at this orbit. Used for attributes on a body that
    /// pinned its own orbit — the placement was a constraint, but the
    /// facilities and travel zone still get attached afterwards.
    AtOrbit(i32),
}

/// A fact applied to the finished system rather than steering generation.
#[derive(Debug, Clone)]
pub struct PostSpec {
    pub target: Target,
    /// For a moon, its orbit around its parent.
    pub satellite_orbit: Option<usize>,
    pub name: Option<String>,
    pub uwp: Option<PartialUwp>,
    pub facilities: Vec<crate::systems::world::Facility>,
    pub zone: Option<crate::trade::ZoneClassification>,
}

impl BodySpec {
    /// Lower one body into everything it says.
    ///
    /// A single line can carry both a placement fact and attribute facts:
    /// "a belt at orbit 7 with a mining base" is a constraint that steers
    /// generation plus a facility that can only be attached once the belt
    /// exists. So this returns a list rather than one or the other.
    pub fn lower_all(&self) -> Result<Vec<Lowered>, String> {
        let mut out = vec![self.lower()?];
        if let Some(attrs) = self.attributes()? {
            out.push(Lowered::Post(attrs));
        }
        Ok(out)
    }

    /// The facilities and travel zone this body declares, if any, as a
    /// post-generation fact aimed at wherever the body ends up.
    pub fn attributes(&self) -> Result<Option<PostSpec>, String> {
        let (facs, z, orbit, position) = match self {
            BodySpec::Planet {
                facilities,
                zone,
                orbit,
                position,
                ..
            }
            | BodySpec::Belt {
                facilities,
                zone,
                orbit,
                position,
                ..
            } => (facilities, zone, *orbit, position.clone()),
            BodySpec::Moon {
                facilities,
                zone,
                parent,
                parent_orbit,
                ..
            } => {
                if facilities.is_empty() && zone.is_none() {
                    return Ok(None);
                }
                // A moon's attributes ride on the moon spec itself, which the
                // post-pass creates; nothing extra to target.
                let _ = (parent, parent_orbit);
                return Ok(None);
            }
            _ => return Ok(None),
        };
        if facs.is_empty() && z.is_none() {
            return Ok(None);
        }
        let target = match (orbit, position) {
            (Some(o), None) => Target::AtOrbit(o),
            (None, Some(p)) => match p {
                PositionSpec::Outermost => Target::Outermost(self.post_kind()),
                PositionSpec::Innermost => Target::Innermost(self.post_kind()),
                PositionSpec::After(b) => Target::After {
                    kind: self.post_kind(),
                    body: b,
                },
            },
            _ => {
                return Err(
                    "facilities or a zone need the body to be locatable: give it an orbit or a \
                     position"
                        .into(),
                );
            }
        };
        Ok(Some(PostSpec {
            target,
            satellite_orbit: None,
            name: None,
            uwp: None,
            facilities: self.parse_facilities()?,
            zone: self.parse_zone()?,
        }))
    }

    /// Which star this body orbits.
    pub fn star(&self) -> StarRef {
        match self {
            BodySpec::Planet { star, .. }
            | BodySpec::Belt { star, .. }
            | BodySpec::GasGiant { star, .. }
            | BodySpec::Moon { star, .. } => *star,
            // Stars and empty orbits are always described relative to the
            // system they are part of.
            _ => StarRef::Primary,
        }
    }

    fn post_kind(&self) -> PostKind {
        match self {
            BodySpec::Belt { .. } => PostKind::Belt,
            BodySpec::GasGiant { .. } => PostKind::GasGiant,
            _ => PostKind::Planet,
        }
    }

    fn parse_facilities(&self) -> Result<Vec<crate::systems::world::Facility>, String> {
        use crate::systems::world::Facility;
        let names: &[String] = match self {
            BodySpec::Planet { facilities, .. }
            | BodySpec::Belt { facilities, .. }
            | BodySpec::Moon { facilities, .. } => facilities,
            _ => &[],
        };
        names
            .iter()
            .map(|n| match n.to_ascii_lowercase().as_str() {
                "naval" => Ok(Facility::Naval),
                "scout" => Ok(Facility::Scout),
                "farming" => Ok(Facility::Farming),
                "mining" => Ok(Facility::Mining),
                "colony" => Ok(Facility::Colony),
                "lab" | "research" => Ok(Facility::Lab),
                "military" => Ok(Facility::Military),
                other => Err(format!(
                    "unknown facility \"{other}\"; expected naval, scout, farming, mining, \
                     colony, lab or military"
                )),
            })
            .collect()
    }

    fn parse_zone(&self) -> Result<Option<crate::trade::ZoneClassification>, String> {
        use crate::trade::ZoneClassification;
        let z: &Option<String> = match self {
            BodySpec::Planet { zone, .. }
            | BodySpec::Belt { zone, .. }
            | BodySpec::Moon { zone, .. } => zone,
            _ => &None,
        };
        match z.as_deref() {
            None => Ok(None),
            Some(v) if v.eq_ignore_ascii_case("green") => Ok(Some(ZoneClassification::Green)),
            Some(v) if v.eq_ignore_ascii_case("amber") => Ok(Some(ZoneClassification::Amber)),
            Some(v) if v.eq_ignore_ascii_case("red") => Ok(Some(ZoneClassification::Red)),
            Some(v) => Err(format!(
                "zone must be \"green\", \"amber\" or \"red\"; got \"{v}\""
            )),
        }
    }

    /// Lower one body, routing it to generation or to the post-pass.
    pub fn lower(&self) -> Result<Lowered, String> {
        let uwp = |u: &Option<String>| -> Result<Option<PartialUwp>, String> {
            u.as_deref().map(PartialUwp::parse).transpose()
        };
        let target = |pos: &PositionSpec, kind: PostKind| match pos {
            PositionSpec::Outermost => Target::Outermost(kind),
            PositionSpec::Innermost => Target::Innermost(kind),
            PositionSpec::After(b) => Target::After {
                kind,
                body: b.clone(),
            },
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
                        crate::api::parse_stellar(c)
                            .into_iter()
                            .next()
                            .ok_or_else(|| format!("could not parse star class \"{c}\""))
                    })
                    .transpose()?;
                Lowered::Constraint(Constraint::Star {
                    orbit,
                    spectral: spec.as_ref().map(|s| s.spectral),
                    subtype: spec.as_ref().and_then(|s| s.subtype),
                    size: spec.as_ref().map(|s| s.size),
                    name: name.clone(),
                })
            }
            BodySpec::Planet {
                name,
                orbit,
                position,
                uwp: u,
                moons,
                ..
            } => {
                if orbit.is_some() && position.is_some() {
                    return Err("a body has both an orbit and a position; pick one".into());
                }
                match position {
                    Some(p) => Lowered::Post(PostSpec {
                        target: target(p, PostKind::Planet),
                        satellite_orbit: None,
                        name: name.clone(),
                        uwp: uwp(u)?,
                    facilities: Vec::new(),
                    zone: None,
                    }),
                    None => Lowered::Constraint(Constraint::Planet {
                        name: name.clone(),
                        orbit: *orbit,
                        uwp: uwp(u)?,
                        num_satellites: *moons,
                        // Never settable from an override; see the module docs.
                        is_mainworld: false,
                    }),
                }
            }
            BodySpec::Belt {
                name,
                orbit,
                position,
                uwp: u,
                moons,
                ..
            } => {
                if orbit.is_some() && position.is_some() {
                    return Err("a body has both an orbit and a position; pick one".into());
                }
                match position {
                    Some(p) => Lowered::Post(PostSpec {
                        target: target(p, PostKind::Belt),
                        satellite_orbit: None,
                        name: name.clone(),
                        uwp: uwp(u)?,
                    facilities: Vec::new(),
                    zone: None,
                    }),
                    None => Lowered::Constraint(Constraint::Belt {
                        name: name.clone(),
                        orbit: *orbit,
                        uwp: uwp(u)?,
                        num_satellites: *moons,
                    }),
                }
            }
            BodySpec::GasGiant {
                name,
                orbit,
                position,
                size,
                moons,
                ..
            } => {
                if orbit.is_some() && position.is_some() {
                    return Err("a body has both an orbit and a position; pick one".into());
                }
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
                match position {
                    Some(p) => Lowered::Post(PostSpec {
                        target: target(p, PostKind::GasGiant),
                        satellite_orbit: None,
                        name: name.clone(),
                        uwp: None,
                    facilities: Vec::new(),
                    zone: None,
                    }),
                    None => Lowered::Constraint(Constraint::GasGiant {
                        name: name.clone(),
                        orbit: *orbit,
                        size,
                        num_satellites: *moons,
                    }),
                }
            }
            BodySpec::Moon {
                name,
                parent_orbit,
                parent,
                satellite_orbit,
                uwp: u,
                ..
            } => match (parent_orbit, parent) {
                (Some(_), Some(_)) => {
                    return Err("a moon has both parent_orbit and parent; pick one".into());
                }
                (None, None) => {
                    return Err("a moon needs either parent_orbit or parent".into());
                }
                (Some(o), None) => Lowered::Constraint(Constraint::Moon {
                    name: name.clone(),
                    parent_orbit: *o,
                    uwp: uwp(u)?,
                }),
                (None, Some(p)) => Lowered::Post(PostSpec {
                    target: Target::MoonOf(p.clone()),
                    satellite_orbit: *satellite_orbit,
                    name: name.clone(),
                    uwp: uwp(u)?,
                facilities: Vec::new(),
                zone: None,
                }),
            },
            BodySpec::Empty { orbit } => Lowered::Constraint(Constraint::Empty { orbit: *orbit }),
        })
    }

    /// Lower to a plain constraint, erroring if this body is a post-pass
    /// fact. Kept for callers that only deal in generation input.
    pub fn to_constraint(&self) -> Result<Constraint, String> {
        match self.lower()? {
            Lowered::Constraint(c) => Ok(c),
            Lowered::Post(_) => {
                Err("this body is positioned relatively and is applied after generation".into())
            }
        }
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

        let mut mine: Vec<Constraint> = Vec::new();
        for b in &self.bodies {
            let star = b.star();
            for l in b.lower_all()? {
                match (l, star) {
                    // A companion's bodies go to that companion's own
                    // sub-system, where the orbit numbers mean its orbits.
                    (Lowered::Constraint(c), StarRef::Secondary) => cs.secondary_bodies.push(c),
                    (Lowered::Constraint(c), StarRef::Tertiary) => cs.tertiary_bodies.push(c),
                    (Lowered::Constraint(c), StarRef::Primary) => mine.push(c),
                    // Post facts resolve against the whole system, companions
                    // included, so they need no routing.
                    (Lowered::Post(p), _) => cs.post.push(p),
                }
            }
        }

        // --- stars ---
        let (my_stars, my_bodies): (Vec<_>, Vec<_>) = mine
            .into_iter()
            .partition(|c| matches!(c, Constraint::Star { .. }));
        // A star override aimed at the primary patches the primary; everything
        // else queues up against the companions in order.
        //
        // Splitting these matters: without it an override correcting the
        // primary's spectral class was matched against the first companion
        // instead, rewriting the wrong star. Real case — the Drinaxian
        // Companion gives Palindrome's primary as K7 V where TravellerMap has
        // K9 V, and the system also has an M1 V companion that would have
        // silently become K7 V.
        let (primary_patch, companion_patches): (Vec<_>, Vec<_>) = my_stars
            .into_iter()
            .partition(|c| matches!(c, Constraint::Star { orbit: Some(StarOrbit::Primary), .. }));

        if let Some(Constraint::Star {
            spectral: sp2,
            subtype: st2,
            size: sz2,
            ..
        }) = primary_patch.first()
        {
            for c in cs.bodies.iter_mut() {
                let Constraint::Star {
                    orbit,
                    spectral,
                    subtype,
                    size,
                    ..
                } = c
                else {
                    continue;
                };
                if !matches!(orbit, Some(StarOrbit::Primary)) {
                    continue;
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
                // Not the name: the primary's name is the system's, and
                // validation rejects a name here.
                break;
            }
        }

        let my_stars = companion_patches;
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
            // The primary was handled above.
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

/// The override file, compiled into the binary.
///
/// Compiled in rather than read at runtime because deploy *is* the update
/// mechanism: there's no filesystem in the container worth depending on, no
/// per-request I/O, and a malformed file becomes a build failure instead of
/// a production surprise. `include_str!` also works on wasm, so the frontend
/// sees the same data as the server.
const OVERRIDES_JSON: &str = include_str!("../../data/overrides.json");

/// Parsed overrides, keyed by [`canonical_key`].
///
/// Panics on a malformed file. That is deliberate: the file ships inside the
/// binary, so a parse failure means the build was broken before it left the
/// machine, and there is no sensible way to serve half of a curated data set.
/// The validator catches it long before this does.
fn table() -> &'static std::collections::HashMap<String, SystemOverride> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<String, SystemOverride>> =
        std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let file: OverrideFile = serde_json::from_str(OVERRIDES_JSON)
            .expect("data/overrides.json is malformed — run the override validator");
        file.overrides
            .into_iter()
            .map(|o| (canonical_key(&o.sector, &o.hex), o))
            .collect()
    })
}

/// Every override, for validation and tooling.
pub fn all() -> Vec<&'static SystemOverride> {
    table().values().collect()
}

/// The override for one system, if there is one.
pub fn lookup(sector: &str, hex: &str) -> Option<&'static SystemOverride> {
    table().get(&canonical_key(sector, hex))
}

/// Merge the override for `(sector, hex)` into `cs`, if one exists.
///
/// The single entry point both the server and the validator go through, so
/// what the validator checks is exactly what production will generate.
pub fn apply(
    sector: &str,
    hex: &str,
    cs: crate::systems::constraint::SystemConstraints,
) -> Result<crate::systems::constraint::SystemConstraints, String> {
    match lookup(sector, hex) {
        Some(o) => o.merge_into(cs),
        None => Ok(cs),
    }
}

/// Apply post-generation facts to a finished system.
///
/// Returns the ones that couldn't be resolved, as
/// [`DroppedConstraint`]s, so an override naming "the outermost gas giant"
/// in a system with no gas giants fails loudly rather than doing nothing.
pub fn apply_post(
    system: &mut crate::systems::system::System,
    specs: &[PostSpec],
) -> Vec<crate::systems::system::DroppedConstraint> {
    // Try this level, then hand whatever didn't resolve to the companions.
    //
    // A companion is a `System` in its own right with its own orbit slots,
    // so a moon of Khazha — which orbits the secondary — is invisible from
    // the primary's slots. Recursing is what lets a source table describe
    // the whole system rather than only the part orbiting the primary star.
    let unresolved = apply_post_level(system, specs);
    if unresolved.is_empty() {
        return Vec::new();
    }
    let mut still = unresolved;
    for child in [system.secondary.as_deref_mut(), system.tertiary.as_deref_mut()]
        .into_iter()
        .flatten()
    {
        if still.is_empty() {
            break;
        }
        let retry: Vec<PostSpec> = still.into_iter().map(|(s, _)| s).collect();
        still = apply_post_level(child, &retry);
    }
    still.into_iter().map(|(_, d)| d).collect()
}

/// Apply what this one system can, returning the specs it couldn't resolve
/// along with the diagnostic each would produce if nothing else can either.
fn apply_post_level(
    system: &mut crate::systems::system::System,
    specs: &[PostSpec],
) -> Vec<(PostSpec, crate::systems::system::DroppedConstraint)> {
    use crate::systems::system::{BodyKind, DroppedConstraint, OrbitContent};

    let mut dropped = Vec::new();

    for spec in specs {
        // Resolve the target to an orbit index, given what actually got
        // generated.
        // Belts and planets are both `OrbitContent::World`, so "the outermost
        // planet" needs to tell them apart. Size alone doesn't: `World::to_uwp`
        // renders a size-0 body as "S" (a rockball) unless it is *named* as a
        // planetoid, in which case it renders "0". Plenty of ordinary outer
        // worlds are size 0 — treating those as belts found no planets at all
        // in Torpol beyond the main world.
        //
        // So follow the same rule `to_uwp` uses, name and all. It is a
        // heuristic, but it is *the* heuristic this codebase already relies on
        // to draw the map, and a second, disagreeing one would be worse.
        fn is_belt(w: &crate::systems::world::World) -> bool {
            w.size <= 0 && w.name.to_lowercase().contains("planetoid")
        }
        let matches_kind = |c: &OrbitContent, kind: PostKind| match (c, kind) {
            (OrbitContent::World(w), PostKind::Planet) => !is_belt(w),
            (OrbitContent::World(w), PostKind::Belt) => is_belt(w),
            (OrbitContent::GasGiant(_), PostKind::GasGiant) => true,
            _ => false,
        };
        let of_kind = |system: &crate::systems::system::System, kind: PostKind| -> Vec<usize> {
            system
                .orbit_slots
                .iter()
                .enumerate()
                .filter_map(|(i, slot)| {
                    slot.as_ref()
                        .filter(|c| matches_kind(c, kind))
                        .map(|_| i)
                })
                .collect()
        };
        let orbit_of_named = |system: &crate::systems::system::System, name: &str| -> Option<usize> {
            system.orbit_slots.iter().position(|slot| match slot {
                Some(OrbitContent::World(w)) => w.name.eq_ignore_ascii_case(name),
                Some(OrbitContent::GasGiant(g)) => g.name.eq_ignore_ascii_case(name),
                _ => false,
            })
        };

        let body_kind = |k: PostKind| match k {
            PostKind::Planet => BodyKind::Planet,
            PostKind::Belt => BodyKind::Belt,
            PostKind::GasGiant => BodyKind::GasGiant,
        };

        let target_orbit: Option<usize> = match &spec.target {
            Target::AtOrbit(o) => {
                let o = *o;
                if o >= 0 && (o as usize) < system.orbit_slots.len() {
                    Some(o as usize)
                } else {
                    None
                }
            }
            Target::Outermost(kind) => of_kind(system, *kind).last().copied(),
            Target::Innermost(kind) => of_kind(system, *kind).first().copied(),
            Target::After { kind, body } => match orbit_of_named(system, body) {
                Some(after) => of_kind(system, *kind).into_iter().find(|o| *o > after),
                None => {
                    dropped.push((
                        spec.clone(),
                        DroppedConstraint::RelativeBodyNotFound {
                            body: body_kind(*kind),
                            reference: body.clone(),
                        },
                    ));
                    continue;
                }
            },
            Target::MoonOf(parent) => {
                match orbit_of_named(system, parent) {
                    Some(o) => {
                        // Build the moon and hang it off its parent.
                        let star = system.star;
                        let mainworld = system
                            .orbit_slots
                            .iter()
                            .flatten()
                            .find_map(|c| match c {
                                OrbitContent::World(w) if w.is_mainworld() => Some(w.clone()),
                                _ => None,
                            })
                            .unwrap_or_default();
                        let mut moon = crate::systems::world::World::generate_with_partial(
                            &star,
                            o,
                            &mainworld,
                            spec.uwp.as_ref(),
                            spec.name.as_deref(),
                            true,
                            false,
                        );
                        if !spec.facilities.is_empty() {
                            moon.set_facilities(spec.facilities.clone());
                        }
                        if let Some(z) = spec.zone {
                            moon.travel_zone = z;
                        }
                        // The satellite's own orbit around its parent. The
                        // generator already rolls one from the standard table;
                        // this pins it when a source gives the number.
                        if let Some(so) = spec.satellite_orbit {
                            moon.orbit = so;
                        }
                        match system.orbit_slots.get_mut(o).and_then(|s| s.as_mut()) {
                            Some(OrbitContent::GasGiant(g)) => {
                                use crate::systems::has_satellites::HasSatellites;
                                g.push_satellite(moon);
                            }
                            Some(OrbitContent::World(w)) => w.satellites.sats.push(moon),
                            _ => dropped.push((
                                spec.clone(),
                                DroppedConstraint::MoonParentCannotHoldMoons {
                                    parent_orbit: o as i32,
                                },
                            )),
                        }
                        continue;
                    }
                    None => {
                        dropped.push((
                            spec.clone(),
                            DroppedConstraint::RelativeBodyNotFound {
                                body: BodyKind::Moon,
                                reference: parent.clone(),
                            },
                        ));
                        continue;
                    }
                }
            }
        };

        let Some(orbit) = target_orbit else {
            let kind = match &spec.target {
                Target::Outermost(k) | Target::Innermost(k) => body_kind(*k),
                Target::After { kind, .. } => body_kind(*kind),
                Target::MoonOf(_) => BodyKind::Moon,
                Target::AtOrbit(_) => BodyKind::Planet,
            };
            dropped.push((
                spec.clone(),
                DroppedConstraint::NoBodyAtRelativePosition { body: kind },
            ));
            continue;
        };

        // Apply the fact. A UWP means rebuilding the world at that orbit
        // from the pinned columns — it has already been generated, and
        // editing its digits in place would leave the derived fields (trade
        // classes, astro data) describing the world it used to be.
        let star = system.star;
        let mainworld = system
            .orbit_slots
            .iter()
            .flatten()
            .find_map(|c| match c {
                OrbitContent::World(w) if w.is_mainworld() => Some(w.clone()),
                _ => None,
            })
            .unwrap_or_default();

        match system.orbit_slots.get_mut(orbit).and_then(|s| s.as_mut()) {
            Some(OrbitContent::World(w)) => {
                if !spec.facilities.is_empty() {
                    let mut f = spec.facilities.clone();
                    f.dedup();
                    w.set_facilities(f);
                }
                if let Some(z) = spec.zone {
                    w.travel_zone = z;
                }
                if let Some(partial) = &spec.uwp {
                    let mut rebuilt = crate::systems::world::World::generate_with_partial(
                        &star,
                        orbit,
                        &mainworld,
                        Some(partial),
                        spec.name.as_deref().or(Some(w.name.as_str())),
                        false,
                        false,
                    );
                    rebuilt.orbit = orbit;
                    rebuilt.compute_astro_data(&star);
                    *w = rebuilt;
                } else if let Some(n) = &spec.name {
                    w.name = n.clone();
                }
            }
            Some(OrbitContent::GasGiant(g)) => {
                if let Some(n) = &spec.name {
                    g.name = n.clone();
                }
            }
            _ => {}
        }
    }

    dropped
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
            b.lower().expect("lowers");
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
            if let Lowered::Constraint(Constraint::Planet { is_mainworld, .. }) = b.lower().unwrap()
            {
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
            star: StarRef::Primary,
            position: None,
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
                star: StarRef::Primary,
                position: None,
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
                BodySpec::GasGiant { star: StarRef::Primary, name: None, orbit: Some(5), position: None, size: None, moons: None },
                BodySpec::GasGiant { star: StarRef::Primary, name: None, orbit: Some(9), position: None, size: None, moons: None },
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

    /// An override correcting the *primary's* class must patch the primary,
    /// not the first companion.
    ///
    /// Real case: the Drinaxian Companion gives Palindrome's primary as K7 V
    /// where TravellerMap has K9 V, and Palindrome also has an M1 V
    /// companion. Matching star overrides against companions in blind order
    /// rewrote the companion as K7 V and left the primary wrong — two errors
    /// from one fix, neither of them visible.
    #[test]
    fn a_primary_class_override_patches_the_primary_not_the_companion() {
        // K9 V primary with an M1 V companion, as Palindrome comes in.
        let stars = parse_stellar("K9 V M1 V");
        let cs = build_constraints("Palindrome", "B433334-B", &stars, 2, 0, 3).unwrap();

        let ov = SystemOverride {
            sector: "Trojan Reach".into(),
            hex: "2216".into(),
            world: "Palindrome".into(),
            note: None,
            system_name: None,
            bodies: vec![BodySpec::Star {
                name: None,
                orbit: Some(StarOrbitSpec::Named("primary".into())),
                class: Some("K7 V".into()),
            }],
        };
        let merged = ov.merge_into(cs).unwrap();
        assert_eq!(count(&merged, 0), 2, "still two stars");

        let mut primary = None;
        let mut companion = None;
        for c in &merged.bodies {
            if let Constraint::Star {
                orbit,
                spectral,
                subtype,
                ..
            } = c
            {
                if matches!(orbit, Some(StarOrbit::Primary)) {
                    primary = Some((*spectral, *subtype));
                } else {
                    companion = Some((*spectral, *subtype));
                }
            }
        }
        let (p_spec, p_sub) = primary.expect("a primary");
        assert_eq!(p_sub, Some(7), "primary corrected to K7");
        assert!(p_spec.is_some());

        let (c_spec, c_sub) = companion.expect("a companion");
        assert_eq!(c_sub, Some(1), "companion left as M1, untouched");
        assert!(c_spec.is_some());
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
                star: StarRef::Primary,
                position: None,
                facilities: Vec::new(),
                zone: None,
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

    /// The shipped file must parse. It's compiled in, so a broken one is a
    /// broken build — but `table()` only parses on first use, which could be
    /// deep inside a request. This makes it a test failure instead.
    #[test]
    fn the_shipped_override_file_parses() {
        let file: OverrideFile =
            serde_json::from_str(OVERRIDES_JSON).expect("data/overrides.json parses");
        // Every body must lower cleanly too — a parseable file can still
        // contain an unparseable star class or UWP.
        for o in &file.overrides {
            for b in &o.bodies {
                // `lower`, not `to_constraint`: a relatively-positioned body
                // is a valid override that simply isn't a constraint.
                b.lower()
                    .unwrap_or_else(|e| panic!("{} {}: {e}", o.sector, o.hex));
            }
        }
    }

    /// Keys must be unique: two entries for one system would mean one of them
    /// silently never applies, depending on map insertion order.
    #[test]
    fn no_two_overrides_claim_the_same_system() {
        let file: OverrideFile = serde_json::from_str(OVERRIDES_JSON).unwrap();
        let mut seen = std::collections::HashSet::new();
        for o in &file.overrides {
            let k = canonical_key(&o.sector, &o.hex);
            assert!(seen.insert(k.clone()), "two overrides for {k}");
        }
    }

    /// The overwhelmingly common path: no override, constraints untouched.
    #[test]
    fn apply_is_a_no_op_for_a_system_with_no_override() {
        let before = pourne_constraints(2, 1, 3);
        let n_before = before.bodies.len();
        let after = apply("Nowhere In Particular", "0000", before).unwrap();
        assert_eq!(after.bodies.len(), n_before);
        assert_eq!(after.system_name, None);
    }

    fn ov(bodies: Vec<BodySpec>) -> SystemOverride {
        SystemOverride {
            sector: "Trojan Reach".into(),
            hex: "2221".into(),
            world: "Torpol".into(),
            note: None,
            system_name: None,
            bodies,
        }
    }

    fn generate(bodies: Vec<BodySpec>) -> crate::systems::system::System {
        let stars = parse_stellar("F4 V");
        let cs = build_constraints("Torpol", "B55A77A-8", &stars, 4, 2, 9).unwrap();
        let merged = ov(bodies).merge_into(cs).unwrap();
        crate::systems::system::System::generate_from_constraints_seeded(77, merged).unwrap()
    }

    /// "The outermost gas giant is Bulhai" — the name lands on whichever
    /// giant actually ended up outermost, not on an orbit guessed in advance.
    #[test]
    fn outermost_resolves_against_the_generated_system() {
        let sys = generate(vec![BodySpec::GasGiant {
            star: StarRef::Primary,
            name: Some("Bulhai".into()),
            orbit: None,
            position: Some(PositionSpec::Outermost),
            size: None,
            moons: None,
        }]);
        assert_eq!(sys.dropped_constraints(), Vec::new());

        let giants: Vec<(usize, String)> = sys
            .orbit_slots
            .iter()
            .enumerate()
            .filter_map(|(i, c)| match c {
                Some(crate::systems::system::OrbitContent::GasGiant(g)) => {
                    Some((i, g.name.clone()))
                }
                _ => None,
            })
            .collect();
        assert!(giants.len() > 1, "need several giants for this to mean anything");
        assert_eq!(
            giants.last().unwrap().1,
            "Bulhai",
            "the outermost giant should be the named one; giants were {giants:?}"
        );
        assert!(
            giants[..giants.len() - 1].iter().all(|(_, n)| n != "Bulhai"),
            "only the outermost should be renamed"
        );
    }

    /// "The next world out from Torpol" — resolved against the main world's
    /// actual orbit, with the UWP applied to whichever world that is.
    #[test]
    fn after_resolves_to_the_next_body_beyond_the_named_one() {
        let sys = generate(vec![BodySpec::Planet {
            star: StarRef::Primary,
            facilities: Vec::new(),
            zone: None,
            name: Some("Traefar".into()),
            orbit: None,
            position: Some(PositionSpec::After("Torpol".into())),
            uwp: Some("?X106XX-X".into()),
            moons: None,
        }]);
        assert_eq!(sys.dropped_constraints(), Vec::new());

        let main_orbit = sys
            .orbit_slots
            .iter()
            .position(|c| matches!(c, Some(crate::systems::system::OrbitContent::World(w)) if w.is_mainworld()))
            .expect("a main world");
        let traefar = sys
            .orbit_slots
            .iter()
            .enumerate()
            .find_map(|(i, c)| match c {
                Some(crate::systems::system::OrbitContent::World(w)) if w.name == "Traefar" => {
                    Some((i, w.clone()))
                }
                _ => None,
            })
            .expect("Traefar was placed");
        assert!(
            traefar.0 > main_orbit,
            "Traefar at {} should be beyond Torpol at {main_orbit}",
            traefar.0
        );
        // The pinned columns survived the rebuild.
        assert_eq!(traefar.1.atmosphere, 1);
        assert_eq!(traefar.1.hydro, 0);
        assert_eq!(traefar.1.get_population(), 6);
    }

    /// A moon can name its parent instead of numbering it, which is what a
    /// source does — and works even when nothing pinned the parent's orbit.
    #[test]
    fn a_moon_can_find_its_parent_by_name() {
        let sys = generate(vec![
            BodySpec::GasGiant {
                star: StarRef::Primary,
                name: Some("Bulhai".into()),
                orbit: None,
                position: Some(PositionSpec::Outermost),
                size: None,
                moons: None,
            },
            BodySpec::Moon {
                star: StarRef::Primary,
                satellite_orbit: None,
                facilities: Vec::new(),
                zone: None,
                name: Some("Bulhai Freeport".into()),
                parent_orbit: None,
                parent: Some("Bulhai".into()),
                uwp: Some("FXXX4XX-X".into()),
            },
        ]);
        assert_eq!(sys.dropped_constraints(), Vec::new());

        let found = sys.orbit_slots.iter().flatten().any(|c| match c {
            crate::systems::system::OrbitContent::GasGiant(g) => {
                g.name == "Bulhai" && g.satellites().iter().any(|m| m.name == "Bulhai Freeport")
            }
            _ => false,
        });
        assert!(found, "the freeport should hang off Bulhai");
    }

    /// A body addressed to a companion lands in that companion's
    /// sub-system, at *its* orbit number.
    ///
    /// Makergod's Oghma table reads "Secondary Star Doruc - orbit 1 Khazha",
    /// and orbit 1 of Doruc is not orbit 1 of Fijari.
    #[test]
    fn a_body_can_belong_to_a_companion_sub_system() {
        let stars = parse_stellar("K5 V M5 V");
        let cs = build_constraints("Oghma", "B534754-9", &stars, 4, 0, 6).unwrap();
        let ov = SystemOverride {
            sector: "Trojan Reach".into(),
            hex: "2020".into(),
            world: "Oghma".into(),
            note: None,
            system_name: None,
            bodies: vec![BodySpec::GasGiant {
                star: StarRef::Secondary,
                name: Some("Khazha".into()),
                orbit: Some(1),
                position: None,
                size: Some("small".into()),
                moons: None,
            }],
        };
        let merged = ov.merge_into(cs).unwrap();
        assert_eq!(
            merged.secondary_bodies.len(),
            1,
            "the body should be routed to the secondary, not the primary"
        );

        let sys =
            crate::systems::system::System::generate_from_constraints_seeded(9, merged).unwrap();
        assert_eq!(sys.dropped_constraints(), Vec::new());

        let secondary = sys.secondary.as_ref().expect("a companion was generated");
        let khazha = secondary
            .orbit_slots
            .iter()
            .enumerate()
            .find_map(|(i, c)| match c {
                Some(crate::systems::system::OrbitContent::GasGiant(g)) if g.name == "Khazha" => {
                    Some(i)
                }
                _ => None,
            })
            .expect("Khazha should be in the secondary's sub-system");
        assert_eq!(khazha, 1, "at the secondary's orbit 1");

        // And not in the primary's.
        assert!(
            !sys.orbit_slots.iter().flatten().any(|c| matches!(
                c,
                crate::systems::system::OrbitContent::GasGiant(g) if g.name == "Khazha"
            )),
            "Khazha must not appear in the primary's orbits"
        );
    }

    /// A system that never mentions its companions generates exactly as it
    /// did before companions could be addressed at all.
    #[test]
    fn unaddressed_companions_are_unchanged() {
        let build = || {
            let stars = parse_stellar("K5 V M5 V");
            let cs = build_constraints("Oghma", "B534754-9", &stars, 4, 0, 6).unwrap();
            crate::systems::system::System::generate_from_constraints_seeded(9, cs).unwrap()
        };
        let a = build();
        let b = build();
        let shape = |s: &crate::systems::system::System| {
            s.secondary
                .as_ref()
                .map(|c| c.orbit_slots.iter().map(|o| o.is_some()).collect::<Vec<_>>())
        };
        assert_eq!(shape(&a), shape(&b));
        assert!(shape(&a).is_some_and(|v| v.iter().any(|x| *x)),
            "test is vacuous if the companion has no bodies");
    }

    /// A source table gives satellite orbits — Ra-La-Lantra's moons sit at
    /// 0, 8, 11, 27 and 34 — and pinning one must stick rather than being
    /// re-rolled from the standard satellite table.
    #[test]
    fn a_moon_can_pin_its_orbit_around_its_parent() {
        let sys = generate(vec![
            BodySpec::GasGiant {
                star: StarRef::Primary,
                name: Some("Ra-La-Lantra".into()),
                orbit: None,
                position: Some(PositionSpec::Outermost),
                size: Some("large".into()),
                moons: None,
            },
            BodySpec::Moon {
                star: StarRef::Primary,
                satellite_orbit: Some(27),
                facilities: Vec::new(),
                zone: None,
                name: Some("Daliant".into()),
                parent_orbit: None,
                parent: Some("Ra-La-Lantra".into()),
                uwp: Some("X100000-0".into()),
            },
        ]);
        assert_eq!(sys.dropped_constraints(), Vec::new());

        let moon = sys
            .orbit_slots
            .iter()
            .flatten()
            .find_map(|c| match c {
                crate::systems::system::OrbitContent::GasGiant(g) => {
                    g.satellites().iter().find(|m| m.name == "Daliant").cloned()
                }
                _ => None,
            })
            .expect("Daliant should orbit Ra-La-Lantra");
        assert_eq!(moon.orbit, 27, "the pinned satellite orbit should stick");
    }

    /// A relative position that matches nothing must fail loudly. Doing
    /// nothing quietly is the failure this whole effort exists to remove.
    #[test]
    fn an_unresolvable_position_is_reported() {
        let sys = generate(vec![BodySpec::Planet {
            star: StarRef::Primary,
            facilities: Vec::new(),
            zone: None,
            name: Some("Ghost".into()),
            orbit: None,
            position: Some(PositionSpec::After("Nowhere".into())),
            uwp: None,
            moons: None,
        }]);
        let dropped = sys.dropped_constraints();
        assert!(
            dropped.iter().any(|d| matches!(
                d,
                crate::systems::system::DroppedConstraint::RelativeBodyNotFound { .. }
            )),
            "expected RelativeBodyNotFound, got {dropped:?}"
        );
    }

    /// Facilities and travel zones attach to the finished body — the real
    /// case being "the Imperial megacorporation Sternmetal Horizons also has
    /// a mining base in the inner belt", which a UWP has nowhere to put.
    #[test]
    fn facilities_and_zones_attach_to_the_body() {
        use crate::systems::world::Facility;
        use crate::trade::ZoneClassification;

        // The system is packed — every orbit the star supports has something
        // in it — so this attaches to a belt the PBG digit already put there
        // rather than adding one. That is the real use: "Sternmetal Horizons
        // has a mining base in the inner belt", about a belt that exists.
        let baseline = generate(vec![]);
        let belt_orbit = baseline
            .orbit_slots
            .iter()
            .position(|s| matches!(
                s,
                Some(crate::systems::system::OrbitContent::World(w))
                    if w.name.to_lowercase().contains("planetoid")
            ))
            .expect("the PBG digit put a belt somewhere") as i32;

        let sys = generate(vec![BodySpec::Belt {
            star: StarRef::Primary,
            facilities: vec!["mining".into()],
            zone: Some("amber".into()),
            name: None,
            orbit: Some(belt_orbit),
            position: None,
            uwp: None,
            moons: None,
        }]);
        assert_eq!(sys.dropped_constraints(), Vec::new());

        let body = match sys.orbit_slots.get(belt_orbit as usize) {
            Some(Some(crate::systems::system::OrbitContent::World(w))) => w,
            other => panic!("orbit {belt_orbit} holds {other:?}"),
        };
        assert!(
            body.facilities_string().to_lowercase().contains("mining"),
            "facilities were {}",
            body.facilities_string()
        );
        assert_eq!(body.travel_zone, ZoneClassification::Amber);
        let _ = Facility::Mining;
    }

    /// An unknown facility or zone must fail the parse rather than be
    /// dropped — same reasoning as `deny_unknown_fields`.
    #[test]
    fn bad_facilities_and_zones_are_rejected() {
        let bad_fac = BodySpec::Planet {
            star: StarRef::Primary,
            facilities: vec!["shipyard".into()],
            zone: None,
            name: None,
            orbit: Some(1),
            position: None,
            uwp: None,
            moons: None,
        };
        assert!(bad_fac.lower_all().unwrap_err().contains("shipyard"));

        let bad_zone = BodySpec::Planet {
            star: StarRef::Primary,
            facilities: Vec::new(),
            zone: Some("chartreuse".into()),
            name: None,
            orbit: Some(1),
            position: None,
            uwp: None,
            moons: None,
        };
        assert!(bad_zone.lower_all().unwrap_err().contains("chartreuse"));
    }

    /// A facility on a body with no orbit and no position has nothing to
    /// attach to, and says so.
    #[test]
    fn attributes_need_a_locatable_body() {
        let floating = BodySpec::Planet {
            star: StarRef::Primary,
            facilities: vec!["naval".into()],
            zone: None,
            name: Some("Somewhere".into()),
            orbit: None,
            position: None,
            uwp: None,
            moons: None,
        };
        assert!(floating.lower_all().unwrap_err().contains("locatable"));
    }

    /// An orbit and a position say two different things about one body.
    #[test]
    fn orbit_and_position_together_are_rejected() {
        let b = BodySpec::GasGiant {
            star: StarRef::Primary,
            name: None,
            orbit: Some(5),
            position: Some(PositionSpec::Outermost),
            size: None,
            moons: None,
        };
        assert!(b.lower().unwrap_err().contains("pick one"));
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
