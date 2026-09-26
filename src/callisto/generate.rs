//! The Callisto generator: rulebook Section 2's procedure, from a set of
//! constraints to a [`System`].
//!
//! Stage 1 covers the stars and the orbits: Table 1 steps 1 to 8. Each orbit
//! is placed, zoned and measured, the main world is set in its orbit, and the
//! companions are placed at their separations. Nothing else fills the orbits
//! yet; giants, belts and other worlds arrive with the later stages, so the
//! PBG and world counts only size the orbit list for now.

use crate::callisto::body::{BodyClass, GiantKind, IceAvailability, Physics};
use crate::callisto::dice::{Rng, Roller};
use crate::callisto::fill::{
    composition, first_giant, giant_kind, giants_present, gravity, ice, number_of_giants,
};
use crate::callisto::populate::{
    Body, Fill, Pin, Slot, fill_open, fuel_line, is_ice_source, match_counts, place_giants,
    place_ice_belt, place_pins, roll_codes,
};
use crate::callisto::layout::{Layout, OrbitInfo, Separation, book6_orbit_mkm};
use crate::callisto::orbits::{
    Gap, MAX_POSITION_HD, OrbitPlan, Zone, lay_out, main_world_position, number_of_orbits,
};
use crate::callisto::stars::{
    CStar, Move, SeparationRow, ThirdStar, companion_from_mass, keep_habitable_zone_clear,
    number_of_stars, primary_luminosity, primary_spectral, roll_separation, subtype, third_star,
    white_dwarf_age,
};
use crate::systems::constraint::{Constraint, ConstraintError, PartialUwp, SystemConstraints};
use crate::systems::gas_giant::{GasGiant, GasGiantSize};
use crate::trade::PortCode;
use crate::systems::overrides::Target;
use crate::systems::system::{OrbitContent, Star, StarOrbit, StarSize, StarType, System};
use crate::systems::world::World;

/// Generate a Callisto system from `constraints` with the worldgen RNG seeded
/// by `seed`, so the same seed always gives the same system.
pub fn generate_from_constraints_seeded(
    seed: u64,
    constraints: SystemConstraints,
) -> Result<System, Vec<ConstraintError>> {
    let _guard = crate::util::RngScope::new(seed);
    generate(constraints, &mut Rng)
}

/// A listed star, as a constraint gives it: any field may still be rolled.
#[derive(Debug, Clone, Default)]
struct StarSpec {
    spectral: Option<StarType>,
    subtype: Option<u8>,
    size: Option<StarSize>,
    name: Option<String>,
    orbit: Option<StarOrbit>,
}

/// A companion once its star and separation are known.
#[derive(Debug, Clone)]
struct Companion {
    star: CStar,
    name: Option<String>,
    /// Separation from the primary, in the primary's HD.
    separation_hd: f32,
    /// What the primary keeps clear for it.
    gap: Gap,
    contact: bool,
    moved: Option<Move>,
    /// Set when the separation came from a constraint rather than a roll.
    pinned: bool,
    /// Bodies a source lists around this companion, orbit numbers its own.
    bodies: Vec<Constraint>,
}

/// Generate a Callisto system, rolling on `roller`.
pub fn generate(
    constraints: SystemConstraints,
    roller: &mut impl Roller,
) -> Result<System, Vec<ConstraintError>> {
    let errors = constraints.validate();
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut main_world = main_world(&constraints)?;
    let habitable = (4..=9).contains(&main_world.atmosphere) && main_world.hydro >= 1;

    // Steps 1 and 2: the stars, as listed or rolled.
    let listed: Vec<StarSpec> = constraints
        .bodies
        .iter()
        .filter_map(|b| match b {
            Constraint::Star {
                orbit,
                spectral,
                subtype,
                size,
                name,
            } => Some(StarSpec {
                spectral: *spectral,
                subtype: *subtype,
                size: *size,
                name: name.clone(),
                orbit: *orbit,
            }),
            _ => None,
        })
        .collect();
    let primary_spec = listed.first().cloned().unwrap_or_default();
    let primary = resolve_primary(&primary_spec, habitable, roller);
    let star_count = if listed.is_empty() {
        number_of_stars(primary.star.star_type, roller)
    } else {
        listed.len().min(3)
    };

    // Steps 3 and 4: star data, then each companion's star and separation.
    // A companion carries the bodies a source lists for it (Book 6's
    // secondary and tertiary sub-systems, in listing order).
    let pdata = primary.data();
    let mut companions: Vec<Companion> = (1..star_count)
        .map(|i| {
            let spec = listed.get(i).cloned().unwrap_or_default();
            let mut c = resolve_companion(&spec, &primary, habitable, roller);
            c.bodies = match i {
                1 => constraints.secondary_bodies.clone(),
                2 => constraints.tertiary_bodies.clone(),
                _ => Vec::new(),
            };
            c
        })
        .collect();

    // Table 9: a third star close to the second makes a pair, which orbits
    // the primary at the larger separation.
    let close_pair = companions.len() == 2
        && third_star(companions[0].separation_hd, companions[1].separation_hd)
            == ThirdStar::ClosePair;
    if close_pair {
        companions.sort_by(|a, b| b.separation_hd.total_cmp(&a.separation_hd));
    }
    let primary_gaps: Vec<Gap> = if close_pair {
        vec![companions[0].gap]
    } else {
        companions.iter().filter(|c| !c.contact).map(|c| c.gap).collect()
    };

    // Which star hosts the main world: the primary, unless it can't and a
    // main-sequence companion can.
    let host = if primary.can_host_main_world() {
        None
    } else {
        companions
            .iter()
            .position(|c| c.star.star.size == StarSize::V && !c.contact)
    };
    let host_data = host.map_or(pdata, |i| companions[i].star.data());

    // Step 5: the main world's position, stated or from Table 10.
    let stated_mkm = stated_main_world_mkm(&constraints, &main_world.name);
    let mw_position = match stated_mkm {
        Some(mkm) => mkm / host_data.hd_mkm,
        None => main_world_position(main_world.atmosphere, main_world.hydro, roller),
    };

    // Steps 6 and 7: orbits. The host gets them around the main world; the
    // other stars get theirs from Table 11.
    let known_bodies = 1 + constraints
        .bodies
        .iter()
        .filter(|b| {
            matches!(
                b,
                Constraint::GasGiant { .. }
                    | Constraint::Belt { .. }
                    | Constraint::Planet {
                        is_mainworld: false,
                        ..
                    }
            )
        })
        .count();

    let mut primary_notes = Vec::new();
    if !primary.can_host_main_world() {
        match host {
            Some(i) => primary_notes.push(format!(
                "The {} cannot host a habitable world; the main world orbits the {} companion",
                primary.star, companions[i].star.star
            )),
            None => primary_notes.push(format!(
                "Strained: the listed {} cannot host the main world and no main-sequence \
                 companion can; it orbits the {} anyway",
                primary.star, primary.star
            )),
        }
    }

    let primary_plan = if host.is_none() {
        let plan = lay_out(
            number_of_orbits(known_bodies, roller),
            Some(mw_position),
            pdata.innermost_hd,
            &primary_gaps,
            roller,
        );
        if primary_gaps.iter().any(|g| g.contains(mw_position)) {
            primary_notes.push(format!(
                "Strained: the main world at {mw_position} HD sits where a companion's orbit \
                 leaves nothing stable"
            ));
        }
        plan
    } else {
        lay_out(number_of_orbits(0, roller), None, pdata.innermost_hd, &primary_gaps, roller)
    };

    // Each companion's own orbits reach a third of the way to its partner.
    let companion_plans: Vec<OrbitPlan> = companions
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if c.contact {
                return OrbitPlan::default();
            }
            let partner_hd = if close_pair {
                companions[0].separation_hd.min(companions[1].separation_hd) / 10.0
            } else {
                c.separation_hd
            };
            let data = c.star.data();
            let limit_hd = partner_hd * pdata.hd_mkm / 3.0 / data.hd_mkm;
            let hosted = host == Some(i);
            let known = if hosted { known_bodies } else { c.bodies.len() };
            let plan = lay_out(
                number_of_orbits(known, roller),
                hosted.then_some(mw_position),
                data.innermost_hd,
                &[],
                roller,
            );
            clip(plan, limit_hd)
        })
        .collect();

    // Steps 9 to 13: fill the orbits. The host star takes every body the
    // constraints name; the other stars' orbits are only filled when the
    // counts are free, since a published count describes the charted system.
    let mut primary_slots = slots_from(&primary_plan);
    let mut companion_slots: Vec<Vec<Slot>> = companion_plans.iter().map(slots_from).collect();
    let host_gaps = if host.is_none() { primary_gaps.clone() } else { Vec::new() };
    let host_notes_start = primary_notes.len();
    let host_fuel = {
        let (host_slots, host_star) = match host {
            None => (&mut primary_slots, pdata),
            Some(i) => (&mut companion_slots[i], companions[i].star.data()),
        };
        populate_host(host_slots, &host_star, &host_gaps, &constraints, &mut primary_notes, roller)
    };
    let small_star = |d: &crate::callisto::star::StarData| d.mass < 0.3;
    // A companion a source lists bodies for gets exactly those.
    for (i, (c, slots)) in companions.iter().zip(companion_slots.iter_mut()).enumerate() {
        if host != Some(i) && !c.bodies.is_empty() {
            let own = SystemConstraints {
                bodies: c.bodies.clone(),
                ..Default::default()
            };
            let mut ignored = Vec::new();
            populate_host(slots, &c.star.data(), &[], &own, &mut ignored, roller);
        }
    }
    if constraints.free_counts {
        if host.is_some() {
            fill_open(&mut primary_slots, small_star(&pdata), roller);
            roll_codes(&mut primary_slots, small_star(&pdata), roller);
        }
        for (i, (c, slots)) in companions.iter().zip(companion_slots.iter_mut()).enumerate() {
            if host != Some(i) && c.bodies.is_empty() {
                let d = c.star.data();
                fill_open(slots, small_star(&d), roller);
                roll_codes(slots, small_star(&d), roller);
            }
        }
    }
    // The main world's own rolls: its composition (its UWP is fixed).
    let mw_zone = Zone::at(mw_position);
    let mw_composition = if main_world.size >= 1 {
        Some(composition(mw_zone, 0, roller))
    } else {
        None
    };
    main_world.callisto = Some(Box::new(Physics {
        class: if main_world.size == 0 { BodyClass::Belt } else { BodyClass::World },
        zone: mw_zone,
        position_hd: mw_position,
        composition: mw_composition,
        gravity: mw_composition.map(|c| gravity(main_world.size, c)),
        hydro_is_ice: matches!(mw_zone, Zone::Cold | Zone::Outer) && main_world.hydro >= 1,
        ice_source: false,
    }));

    // Build the systems, innermost structure first.
    let system_name = constraints
        .system_name
        .clone()
        .unwrap_or_else(|| main_world.name.trim().to_string());
    main_world.gen_trade_classes();
    let mut mw = Some(main_world);
    // The host's notes about filling it travel with the host.
    let host_notes: Vec<String> = primary_notes.drain(host_notes_start..).collect();
    let (mut primary_extra, mut hosted_extra) = (Vec::new(), Vec::new());
    if host.is_none() {
        primary_extra = host_notes;
    } else {
        hosted_extra = host_notes;
    }
    primary_notes.extend(primary_extra);

    let mut host_fuel = Some(host_fuel);
    let mut built: Vec<System> = companions
        .iter()
        .zip(companion_slots)
        .enumerate()
        .map(|(i, (c, slots))| {
            let mut notes = match c.moved {
                Some(m) => vec![format!(
                    "Moved {} to keep the habitable zone stable",
                    match m {
                        Move::Inward => "inward",
                        Move::Outward => "outward",
                    }
                )],
                None => Vec::new(),
            };
            if c.pinned {
                notes.push("Separation stated by the source".to_string());
            }
            let hosted = host == Some(i);
            if hosted {
                notes.append(&mut hosted_extra);
            }
            let name = c.name.clone().unwrap_or_else(|| {
                crate::systems::name_tables::gen_star_system_name()
            });
            build_system(
                &c.star,
                name,
                slots,
                if hosted { mw.take() } else { None },
                notes,
                &companion_plans[i],
                if hosted { host_fuel.take() } else { None },
            )
        })
        .collect();

    let mut system = build_system(
        &primary,
        system_name,
        primary_slots,
        mw.take(),
        primary_notes,
        &primary_plan,
        host_fuel.take(),
    );
    if let Some(layout) = system.callisto.as_mut() {
        layout.gaps = primary_gaps;
    }

    // Place the companions around the primary.
    if close_pair {
        let mut outer = built.remove(0);
        let inner = built.remove(0);
        let inner_sep = companions[1].separation_hd.min(companions[0].separation_hd) / 10.0;
        attach(&mut outer, inner, inner_sep * pdata.hd_mkm, false, true);
        let sep = companions[0].separation_hd;
        attach(&mut system, outer, sep * pdata.hd_mkm, false, false);
    } else {
        for (i, (c, s)) in companions.iter().zip(built).enumerate() {
            attach(&mut system, s, c.separation_hd * pdata.hd_mkm, c.contact, i == 1);
        }
    }

    // Override facts aimed at a Book 6 orbit number now aim at wherever that
    // pinned body landed.
    let constraints = retarget_pins(constraints, &system);
    if !constraints.post.is_empty() {
        let unresolved = crate::systems::overrides::apply_post(&mut system, &constraints.post);
        system.dropped.extend(unresolved);
    }
    Ok(system)
}

/// The main world, which must be fully specified (as for Book 6).
fn main_world(constraints: &SystemConstraints) -> Result<World, Vec<ConstraintError>> {
    let Some(Constraint::Planet {
        name,
        uwp: Some(uwp),
        is_mainworld: true,
        ..
    }) = constraints.main_world()
    else {
        return Err(vec![ConstraintError::UnsupportedYet(
            "a fully-specified main-world Planet constraint is required".to_string(),
        )]);
    };
    if !uwp.is_complete() {
        return Err(vec![ConstraintError::UnsupportedYet(
            "partial UWPs on the main world aren't supported yet — supply every column"
                .to_string(),
        )]);
    }
    let uwp = uwp.to_string_with_wildcards();
    let name = name.clone().unwrap_or_else(|| "Main World".to_string());
    World::from_uwp(&name, &uwp, false, true).map_err(|e| {
        vec![ConstraintError::ContradictoryUwp(format!(
            "from_uwp({uwp:?}) failed: {e}"
        ))]
    })
}

/// A distance a source states for the main world, in Mkm: an override's
/// distance, else a pinned Book 6 orbit number converted.
fn stated_main_world_mkm(constraints: &SystemConstraints, name: &str) -> Option<f32> {
    constraints
        .post
        .iter()
        .filter(|p| match &p.target {
            Target::MainWorld => true,
            Target::Named(n) => n == name,
            _ => false,
        })
        .find_map(|p| p.distance_mkm)
        .or_else(|| constraints.main_world_orbit.map(book6_orbit_mkm))
}

/// The primary, from its listing where it has one, else rolled.
fn resolve_primary(spec: &StarSpec, habitable: bool, roller: &mut impl Roller) -> CStar {
    let (star_type, rare_subtype) = match spec.spectral {
        Some(t) => (t, None),
        None => primary_spectral(habitable, roller),
    };
    let sub = spec.subtype.or(rare_subtype).unwrap_or_else(|| subtype(roller));
    let size = spec
        .size
        .unwrap_or_else(|| primary_luminosity(star_type, habitable, roller));
    // A rolled white dwarf rolls again for what it once was.
    let (star_type, sub) = if spec.spectral.is_none() && size == StarSize::D {
        let (t, rare) = primary_spectral(false, roller);
        (t, rare.unwrap_or_else(|| subtype(roller)))
    } else {
        (star_type, sub)
    };
    let star = Star {
        star_type,
        subtype: sub,
        size,
    };
    CStar {
        star,
        wd_age: white_dwarf_age(size, roller),
    }
}

/// A companion: its star as listed or from Table 7, then its separation as
/// stated or from Table 8, moved clear of a habitable main world's orbit.
fn resolve_companion(
    spec: &StarSpec,
    primary: &CStar,
    habitable: bool,
    roller: &mut impl Roller,
) -> Companion {
    let pdata = primary.data();
    let star = match spec.spectral {
        Some(star_type) => Star {
            star_type,
            subtype: spec.subtype.unwrap_or_else(|| subtype(roller)),
            size: spec.size.unwrap_or(StarSize::V),
        },
        None => companion_from_mass(pdata.mass, roller),
    };
    let star = CStar {
        star,
        wd_age: white_dwarf_age(star.size, roller),
    };

    let pinned_hd = match spec.orbit {
        Some(StarOrbit::System(n)) => Some(book6_orbit_mkm(n as i32) / pdata.hd_mkm),
        _ => None,
    };
    let (separation_hd, gap, contact, moved) = match (spec.orbit, pinned_hd) {
        (_, Some(hd)) => (hd, Gap { lo: hd / 3.0, hi: hd * 3.0 }, false, None),
        (Some(StarOrbit::Primary), _) => {
            let row = crate::callisto::stars::separation_rows()[0];
            (row.separation_hd, row.gap(), true, None)
        }
        _ => {
            let row: SeparationRow = roll_separation(primary.star.star_type, roller);
            let (row, moved) = if habitable {
                keep_habitable_zone_clear(row, roller)
            } else {
                (row, None)
            };
            (row.separation_hd, row.gap(), row.is_contact(), moved)
        }
    };
    Companion {
        star,
        name: spec.name.clone(),
        separation_hd,
        gap,
        contact,
        moved,
        pinned: pinned_hd.is_some(),
        bodies: Vec::new(),
    }
}

/// Keep only the orbits inside `limit_hd`, and always the main world's.
fn clip(plan: OrbitPlan, limit_hd: f32) -> OrbitPlan {
    let mw = plan.main_world.map(|i| plan.positions[i]);
    let positions: Vec<f32> = plan
        .positions
        .iter()
        .copied()
        .filter(|p| *p <= limit_hd || Some(*p) == mw)
        .collect();
    OrbitPlan {
        main_world: mw.and_then(|m| positions.iter().position(|p| *p == m)),
        positions,
        ..plan
    }
}

/// Slots for a laid-out plan, the main world's marked.
fn slots_from(plan: &OrbitPlan) -> Vec<Slot> {
    plan.positions
        .iter()
        .enumerate()
        .map(|(i, &position)| Slot {
            position,
            fill: if plan.main_world == Some(i) { Fill::MainWorld } else { Fill::Open },
            pinned_orbit: None,
        })
        .collect()
}

/// Fill the host star's orbits (steps 9 to 13) from the constraints, and
/// return its refuelling line's inputs.
fn populate_host(
    slots: &mut Vec<Slot>,
    star: &crate::callisto::star::StarData,
    gaps: &[Gap],
    constraints: &SystemConstraints,
    notes: &mut Vec<String>,
    roller: &mut impl Roller,
) -> (IceAvailability, bool) {
    let small_star = star.mass < 0.3;
    let free = constraints.free_counts;

    // Bodies a source pins to a Book 6 orbit go in first.
    let mut pins = Vec::new();
    let (mut giants, mut belts_total, mut others_total) = (Vec::new(), 0usize, 0usize);
    let (mut pinned_belts, mut pinned_others) = (0usize, 0usize);
    let mut unpinned: Vec<(BodyClass, Option<String>, Option<PartialUwp>)> = Vec::new();
    for b in &constraints.bodies {
        match b {
            Constraint::GasGiant { name, orbit, size, .. } => {
                let kind = match size {
                    Some(GasGiantSize::Large) => GiantKind::JupiterClass,
                    // A source's "small" giant is an ice giant or a
                    // Saturn-class one: roll which, as Table 17 would.
                    Some(GasGiantSize::Small) => {
                        if roller.d2() <= 7 { GiantKind::IceGiant } else { GiantKind::SaturnClass }
                    }
                    None => giant_kind(roller),
                };
                match orbit {
                    Some(n) => pins.push(Pin {
                        book6_orbit: *n,
                        position_hd: book6_orbit_mkm(*n) / star.hd_mkm,
                        fill: Fill::Giant { kind, name: name.clone() },
                    }),
                    None => giants.push((kind, name.clone())),
                }
            }
            Constraint::Belt { name, orbit, uwp, .. } => {
                belts_total += 1;
                match orbit {
                    Some(n) => {
                        pinned_belts += 1;
                        let mut body = Body::new(BodyClass::Belt);
                        body.name = name.clone();
                        body.uwp = uwp.clone();
                        pins.push(Pin {
                            book6_orbit: *n,
                            position_hd: book6_orbit_mkm(*n) / star.hd_mkm,
                            fill: Fill::Body(body),
                        });
                    }
                    None => unpinned.push((BodyClass::Belt, name.clone(), uwp.clone())),
                }
            }
            Constraint::Planet { is_mainworld: false, name, orbit, uwp, .. } => {
                others_total += 1;
                let class = match uwp.as_ref().and_then(|u| u.size) {
                    Some(s) if s > 10 => BodyClass::SubNeptune,
                    _ => BodyClass::World,
                };
                match orbit {
                    Some(n) => {
                        pinned_others += 1;
                        let mut body = Body::new(class);
                        body.name = name.clone();
                        body.uwp = uwp.clone();
                        pins.push(Pin {
                            book6_orbit: *n,
                            position_hd: book6_orbit_mkm(*n) / star.hd_mkm,
                            fill: Fill::Body(body),
                        });
                    }
                    None => unpinned.push((class, name.clone(), uwp.clone())),
                }
            }
            Constraint::Empty { orbit } => pins.push(Pin {
                book6_orbit: *orbit,
                position_hd: book6_orbit_mkm(*orbit) / star.hd_mkm,
                fill: Fill::Empty,
            }),
            _ => {}
        }
    }
    place_pins(slots, pins, notes);

    // Step 9: giants. A published count (even zero) is used as it stands.
    if free && giants_present(roller) {
        let n = number_of_giants(roller);
        giants.extend((0..n).map(|_| (giant_kind(roller), None)));
    }
    let any_pinned_giant = slots.iter().any(|s| matches!(s.fill, Fill::Giant { .. }));
    let first = (!giants.is_empty() && !any_pinned_giant).then(|| first_giant(roller));
    place_giants(slots, giants, first, !free, gaps, notes);

    // Step 10: ice. Under a published belt count the charted ice belt is one
    // of the published belts, so it needs one to spare.
    let ice = ice(roller);
    let belts_left = belts_total - pinned_belts;
    let belt_allowed = ice != IceAvailability::Sparse && (free || belts_left >= 1);
    let ice_belt = place_ice_belt(slots, belt_allowed);
    let ice_in_moons = ice != IceAvailability::Sparse && !ice_belt;

    // Steps 11 and 12: fill the rest, then meet the published counts.
    fill_open(slots, small_star, roller);
    if !free {
        match_counts(
            slots,
            Some(belts_left - usize::from(ice_belt)),
            Some(others_total - pinned_others),
            gaps,
        );
    }
    // Named or partly-specified bodies without an orbit take the first
    // generated body of their kind.
    for (class, name, uwp) in unpinned {
        if name.is_none() && uwp.is_none() {
            continue;
        }
        let target = slots.iter_mut().find_map(|s| match &mut s.fill {
            Fill::Body(b) if b.rolled && b.name.is_none() && b.uwp.is_none() && (b.class == class || (class == BodyClass::World && b.class != BodyClass::Belt)) => Some(b),
            _ => None,
        });
        if let Some(b) = target {
            b.name = name;
            b.uwp = uwp;
        }
    }

    // Step 13: each body's codes.
    roll_codes(slots, small_star, roller);
    (ice, ice_in_moons)
}

/// One star's `System`: its slots materialized as orbit contents.
fn build_system(
    cstar: &CStar,
    name: String,
    slots: Vec<Slot>,
    main_world: Option<World>,
    notes: Vec<String>,
    plan: &OrbitPlan,
    fuel: Option<(IceAvailability, bool)>,
) -> System {
    let data = cstar.data();
    let star = cstar.star;
    let mut system = System::new(star.star_type, star.subtype, star.size, StarOrbit::Primary, 0);
    system.name = name;
    let ice = fuel.map_or(IceAvailability::Sparse, |f| f.0);
    let mut main_world = main_world;
    let mut names = Vec::with_capacity(slots.len());
    let mut mw_slot = None;
    for (i, slot) in slots.iter().enumerate() {
        let distance = slot.position * data.hd_mkm;
        let content = match &slot.fill {
            Fill::Open => None,
            Fill::Empty => Some(OrbitContent::Blocked),
            Fill::MainWorld => main_world.take().map(|mut w| {
                w.orbit = i;
                w.position_in_system = i;
                w.orbit_distance_mkm = Some(distance);
                w.compute_astro_data(&star);
                mw_slot = Some(i);
                OrbitContent::World(w)
            }),
            Fill::Giant { kind, name } => {
                let size = match kind {
                    GiantKind::JupiterClass => GasGiantSize::Large,
                    _ => GasGiantSize::Small,
                };
                let mut g = GasGiant::new(size, i);
                g.radius_km = crate::util::rng_random_range(kind.diameter_km()) / 2;
                g.callisto = Some(*kind);
                match name {
                    Some(n) => g.name = n.clone(),
                    None => g.gen_name(&system.name, i),
                }
                Some(OrbitContent::GasGiant(g))
            }
            Fill::Body(b) => {
                let c = b.codes.unwrap_or(crate::callisto::fill::Codes {
                    size: 0,
                    atmosphere: 0,
                    hydro: 0,
                    composition: None,
                    gravity: None,
                    hydro_is_ice: false,
                });
                let mut w = World::new(String::new(), i, i, c.size, c.atmosphere, c.hydro, 0, false, false);
                let u = b.uwp.clone().unwrap_or_default();
                w.set_subordinate_stats(
                    u.port.unwrap_or(PortCode::Y),
                    u.government.map_or(0, i32::from),
                    u.law.map_or(0, i32::from),
                    u.tech.map_or(0, i32::from),
                    Vec::new(),
                );
                w.set_population(u.population.map_or(0, i32::from));
                match &b.name {
                    Some(n) => w.name = n.clone(),
                    None => w.gen_name(&system.name, i),
                }
                w.orbit_distance_mkm = Some(distance);
                w.callisto = Some(Box::new(Physics {
                    class: b.class,
                    zone: slot.zone(),
                    position_hd: slot.position,
                    composition: c.composition,
                    gravity: c.gravity,
                    hydro_is_ice: c.hydro_is_ice,
                    ice_source: is_ice_source(slot, ice),
                }));
                Some(OrbitContent::World(w))
            }
        };
        names.push(match &content {
            Some(OrbitContent::World(w)) => w.name.clone(),
            Some(OrbitContent::GasGiant(g)) => g.name.clone(),
            _ => String::new(),
        });
        system.orbit_slots.push(content);
    }
    let fuel = fuel.map(|(ice, in_moons)| fuel_line(&slots, &names, ice, in_moons, &data, mw_slot));
    let orbits = slots
        .iter()
        .map(|s| OrbitInfo {
            position_hd: s.position,
            distance_mkm: s.position * data.hd_mkm,
            zone: s.zone(),
        })
        .collect();
    let mut notes = notes;
    if plan.shortfall > 0 {
        notes.push(format!(
            "{} orbit(s) could not be placed: no room inside {MAX_POSITION_HD} HD",
            plan.shortfall
        ));
    }
    system.callisto = Some(Box::new(Layout {
        star: data,
        orbits,
        crossed_out: plan.crossed_out.clone(),
        gaps: Vec::new(),
        separation: None,
        notes,
        fuel,
    }));
    system
}

/// Rewrite override facts aimed at a Book 6 orbit number (`AtOrbit`) to the
/// slot the body pinned there landed in. Positions identify the slot, since
/// companions inserted after filling shift the indices.
fn retarget_pins(mut constraints: SystemConstraints, system: &System) -> SystemConstraints {
    let Some(layout) = system.callisto.as_deref() else {
        return constraints;
    };
    let hd = layout.star.hd_mkm;
    for p in constraints.post.iter_mut() {
        if let Target::AtOrbit(n) = p.target {
            let want = book6_orbit_mkm(n) / hd;
            if let Some(slot) = layout
                .orbits
                .iter()
                .enumerate()
                .filter(|(i, _)| matches!(system.orbit_slots.get(*i), Some(Some(_))))
                .min_by(|a, b| {
                    (a.1.position_hd / want).ln().abs().total_cmp(&(b.1.position_hd / want).ln().abs())
                })
                .map(|(i, _)| i)
            {
                p.target = Target::AtOrbit(slot as i32);
            }
        }
    }
    constraints
}

/// Hang `companion` off `parent` at `separation_mkm`: as a contact pair, as an
/// orbit slot at its separation, or as a far companion beyond the orbits.
fn attach(
    parent: &mut System,
    mut companion: System,
    separation_mkm: f32,
    contact: bool,
    tertiary: bool,
) {
    let (pdata_hd, orbits) = parent
        .callisto
        .as_ref()
        .map(|l| (l.star.hd_mkm, l.orbits.clone()))
        .expect("Callisto systems carry a layout");
    let separation_hd = separation_mkm / pdata_hd;
    if let Some(l) = companion.callisto.as_mut() {
        l.separation = Some(Separation {
            hd: separation_hd,
            mkm: separation_mkm,
        });
    }
    companion.orbit = if contact {
        StarOrbit::Primary
    } else if separation_hd > MAX_POSITION_HD {
        StarOrbit::Far
    } else {
        // An orbit slot of its own, in order among the parent's orbits.
        let slot = orbits
            .iter()
            .position(|o| o.position_hd > separation_hd)
            .unwrap_or(orbits.len());
        insert_slot(
            parent,
            slot,
            OrbitInfo {
                position_hd: separation_hd,
                distance_mkm: separation_mkm,
                zone: Zone::at(separation_hd),
            },
            if tertiary {
                OrbitContent::Tertiary
            } else {
                OrbitContent::Secondary
            },
        );
        StarOrbit::System(slot)
    };
    let boxed = Some(Box::new(companion));
    if tertiary {
        parent.tertiary = boxed;
    } else {
        parent.secondary = boxed;
    }
}

/// Insert an orbit at `slot`, shifting later slots (and the orbit numbers of
/// whatever sits in them) out by one.
fn insert_slot(system: &mut System, slot: usize, info: OrbitInfo, content: OrbitContent) {
    system.orbit_slots.insert(slot, Some(content));
    if let Some(l) = system.callisto.as_mut() {
        l.orbits.insert(slot, info);
    }
    for (i, s) in system.orbit_slots.iter_mut().enumerate().skip(slot + 1) {
        match s {
            Some(OrbitContent::World(w)) => {
                w.orbit = i;
                w.position_in_system = i;
            }
            Some(OrbitContent::GasGiant(g)) => g.orbit = i,
            _ => {}
        }
    }
    for companion in [system.secondary.as_deref_mut(), system.tertiary.as_deref_mut()]
        .into_iter()
        .flatten()
    {
        if let StarOrbit::System(n) = companion.orbit
            && n >= slot
        {
            companion.orbit = StarOrbit::System(n + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{StarSpec as ApiStar, build_constraints};
    use crate::callisto::body::{Composition, TransitFuel};
    use crate::callisto::dice::{Kind::*, Scripted};
    use crate::callisto::star::days_at_thrust_1;

    fn noricum_constraints() -> SystemConstraints {
        build_constraints(
            "Noricum",
            "D8867BB-1",
            &[
                ApiStar::new(StarType::G, 2, StarSize::V),
                ApiStar::new(StarType::M, 9, StarSize::V),
                ApiStar::new(StarType::M, 6, StarSize::V),
            ],
            2,
            1,
            3,
        )
        .unwrap()
    }

    /// Rulebook 12.3, Noricum, as far as stage 2 goes: the rolls it lists, in
    /// the order the generator makes them. Where the example and the rules
    /// part (see below) the rules win.
    #[test]
    fn noricum() {
        let mut r = Scripted::new(&[
            (D2, 9), // M9 separation: 200 HD
            (D2, 6), // M6 separation: 6 HD
            (D1, 6), // main world position: 1.4
            (D2, 4), // orbits: 2, but 7 bodies
            (D1, 1),
            (D2, 8), (D2, 9),
            (D2, 4), (D2, 10), (D2, 7), (D2, 10),
            (D2, 8), (D2, 10), (D2, 6),
            // The companions' own orbits (the example leaves them unrolled).
            (D2, 2), (D2, 2),
            (D2, 2), (D2, 2),
            (D2, 4), (D2, 3), // giants: ice, ice
            (D1, 1), // first giant: Outer
            (D2, 5), // ice: charted
            (D2, 6), (D2, 8), // fill 0.36 and 0.74: worlds
            (D2, 10), (D1, 6), (D2, 4), // 0.36: size, composition, atmosphere
            (D2, 8), (D1, 4), (D2, 4), (D2, 6), // 0.74
            (D2, 5), (D1, 3), (D2, 8), (D2, 3), // the icy outer world
            (D1, 3), // main world composition
        ]);
        let system = generate(noricum_constraints(), &mut r).unwrap();
        assert_eq!(r.remaining(), 0);
        let layout = system.callisto.as_deref().unwrap();
        let positions: Vec<f32> = layout.orbits.iter().map(|o| o.position_hd).collect();
        // The example keeps 76 and 100, but the M9 at 200 HD has a gap of its
        // own, 67 to 600, which the example overlooks: both are crossed out.
        // The seven published bodies still need orbits, so the second giant
        // takes the free Cold orbit at 2.0 (where the example put the belt),
        // and the belt and the icy world go beyond the M9's gap.
        assert_eq!(layout.crossed_out, [4.5, 7.9, 18.0, 76.0, 100.0]);
        assert_eq!(positions[..6], [0.36, 0.74, 1.4, 2.0, 6.0, 34.0]);
        let kinds: Vec<String> = system
            .orbit_slots
            .iter()
            .map(|s| match s {
                Some(OrbitContent::World(w)) if w.is_mainworld() => "main".to_string(),
                Some(OrbitContent::World(w)) => {
                    let p = w.callisto.as_deref().unwrap();
                    format!("{} {}", p.class.name(), w.to_uwp())
                }
                Some(OrbitContent::GasGiant(g)) => g.callisto.unwrap().name().to_string(),
                Some(OrbitContent::Tertiary) => "M6".to_string(),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "World Y610000-0",
                "World Y633000-0",
                "main",
                "Ice giant",
                "M6",
                "Ice giant",
                "Belt Y000000-0",
                "World Y320000-0",
            ]
        );
        // The main world: rocky, 1 g.
        let mw = system.orbit_slots[2].as_ref().unwrap();
        let OrbitContent::World(mw) = mw else { panic!() };
        let p = mw.callisto.as_deref().unwrap();
        assert_eq!((p.composition, p.gravity), (Some(Composition::Rocky), Some(1.0)));
        // Fuel: a giant for transit; the nearer giant, at 2.0 HD, is 4 days out.
        let fuel = layout.fuel.as_ref().unwrap();
        assert_eq!(fuel.transit, TransitFuel::GiantPlanet);
        assert!((fuel.local_days.unwrap() - days_at_thrust_1(2.0 * 150.0)).abs() < 1e-4);
    }

    #[test]
    fn a_stated_distance_places_the_main_world() {
        let mut cs = noricum_constraints();
        cs.main_world_orbit = Some(3); // Book 6 orbit 3 = 149.6 Mkm ≈ 1.0 HD
        let system = generate_from_constraints_seeded(7, cs).unwrap();
        let layout = system.callisto.as_deref().unwrap();
        let mw_slot = system
            .orbit_slots
            .iter()
            .position(|s| matches!(s, Some(OrbitContent::World(w)) if w.is_mainworld()))
            .unwrap();
        assert!((layout.orbits[mw_slot].position_hd - 149.6 / 150.0).abs() < 1e-4);
    }

    /// IMPLEMENTATION.md §1's fuel invariant over 5,000 free systems: giant
    /// planets in 83% ± 2% of them (Traveller's rate), and a refuelling line
    /// in every one.
    #[test]
    fn giants_in_83_percent_of_free_systems() {
        let n = 5_000;
        let mut with_giants = 0;
        for seed in 0..n {
            let mut cs = SystemConstraints::from_main_world("Test", "A867977-C").unwrap();
            cs.free_counts = true;
            let s = generate_from_constraints_seeded(seed, cs).unwrap();
            let host = std::iter::once(&s)
                .chain(s.secondary.as_deref())
                .chain(s.tertiary.as_deref())
                .find(|x| x.callisto.as_ref().is_some_and(|l| l.fuel.is_some()))
                .expect("some star carries the fuel line");
            if host.orbit_slots.iter().any(|o| matches!(o, Some(OrbitContent::GasGiant(_)))) {
                with_giants += 1;
            }
        }
        let rate = f64::from(with_giants) / n as f64;
        assert!((0.81..=0.85).contains(&rate), "giants in {:.1}% of systems", rate * 100.0);
    }

    #[test]
    fn free_systems_are_deterministic_and_well_formed() {
        for seed in 0..300 {
            let mut cs = SystemConstraints::from_main_world("Test", "A867977-C").unwrap();
            cs.free_counts = true;
            let a = generate_from_constraints_seeded(seed, cs.clone()).unwrap();
            let b = generate_from_constraints_seeded(seed, cs).unwrap();
            let la = a.callisto.as_deref().unwrap();
            assert_eq!(la, b.callisto.as_deref().unwrap(), "seed {seed}");
            assert_eq!(la.orbits.len(), a.orbit_slots.len(), "seed {seed}");
            assert!(
                la.orbits.windows(2).all(|w| w[0].position_hd <= w[1].position_hd),
                "seed {seed}: orbits out of order"
            );
            let mw = a
                .orbit_slots
                .iter()
                .flatten()
                .filter(|s| matches!(s, OrbitContent::World(w) if w.is_mainworld()))
                .count()
                + [a.secondary.as_deref(), a.tertiary.as_deref()]
                    .into_iter()
                    .flatten()
                    .flat_map(|c| c.orbit_slots.iter().flatten())
                    .filter(|s| matches!(s, OrbitContent::World(w) if w.is_mainworld()))
                    .count();
            assert_eq!(mw, 1, "seed {seed}: main world placed {mw} times");
        }
    }
}

/// Render a handful of real systems under both generators, for eyeballing.
/// `CALLISTO_DUMP_DIR=<dir> cargo test --lib -- --ignored callisto_dump`.
#[cfg(test)]
mod dump {
    use crate::api::{Generator, UpstreamSystem, generate_system_png_with, system_from_upstream};

    #[test]
    #[ignore]
    fn callisto_dump() {
        let dir = std::env::var("CALLISTO_DUMP_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let systems = [
            ("Trojan Reach", "3128", "Noricum", "D8867BB-1", "213", "G2 V M9 V M6 V", Some(7)),
            ("Spinward Marches", "1910", "Regina", "A788899-C", "703", "F7 V BD M3 V", Some(8)),
            ("Trojan Reach", "2424", "Hilfer", "BA5077A-6", "400", "M6 V", Some(3)),
            ("Spinward Marches", "1717", "Mora", "AA99AC7-F", "503", "F0 V", Some(9)),
        ];
        for (sector, hex, name, uwp, pbg, stellar, worlds) in systems {
            let (seed, cs) = system_from_upstream(&UpstreamSystem {
                sector,
                hex,
                name,
                uwp,
                pbg,
                stellar,
                worlds,
            })
            .unwrap();
            for g in [Generator::Book6, Generator::Callisto] {
                let png = generate_system_png_with(g, seed, cs.clone(), 1.0).unwrap();
                std::fs::write(format!("{dir}/{name}-{}.png", g.as_str()), png).unwrap();
            }
        }
    }
}

