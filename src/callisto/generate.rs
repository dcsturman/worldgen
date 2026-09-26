//! The Callisto generator: rulebook Section 2's procedure, from a set of
//! constraints to a [`System`].
//!
//! Stage 1 covers the stars and the orbits: Table 1 steps 1 to 8. Each orbit
//! is placed, zoned and measured, the main world is set in its orbit, and the
//! companions are placed at their separations. Nothing else fills the orbits
//! yet; giants, belts and other worlds arrive with the later stages, so the
//! PBG and world counts only size the orbit list for now.

use crate::callisto::dice::{Rng, Roller};
use crate::callisto::layout::{Layout, OrbitInfo, Separation, book6_orbit_mkm};
use crate::callisto::orbits::{
    Gap, MAX_POSITION_HD, OrbitPlan, Zone, lay_out, main_world_position, number_of_orbits,
};
use crate::callisto::stars::{
    CStar, Move, SeparationRow, ThirdStar, companion_from_mass, keep_habitable_zone_clear,
    number_of_stars, primary_luminosity, primary_spectral, roll_separation, subtype, third_star,
    white_dwarf_age,
};
use crate::systems::constraint::{Constraint, ConstraintError, SystemConstraints};
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
    let pdata = primary.data();
    let mut companions: Vec<Companion> = (1..star_count)
        .map(|i| {
            let spec = listed.get(i).cloned().unwrap_or_default();
            resolve_companion(&spec, &primary, habitable, roller)
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
            let plan = lay_out(
                number_of_orbits(if hosted { known_bodies } else { 0 }, roller),
                hosted.then_some(mw_position),
                data.innermost_hd,
                &[],
                roller,
            );
            clip(plan, limit_hd)
        })
        .collect();

    // Build the systems, innermost structure first.
    let system_name = constraints
        .system_name
        .clone()
        .unwrap_or_else(|| main_world.name.trim().to_string());
    main_world.gen_trade_classes();
    let mut mw = Some(main_world);

    let mut built: Vec<System> = companions
        .iter()
        .zip(&companion_plans)
        .enumerate()
        .map(|(i, (c, plan))| {
            let notes = match c.moved {
                Some(m) => vec![format!(
                    "Moved {} to keep the habitable zone stable",
                    match m {
                        Move::Inward => "inward",
                        Move::Outward => "outward",
                    }
                )],
                None => Vec::new(),
            };
            let mut s = build_system(
                &c.star,
                c.name.clone(),
                plan,
                if host == Some(i) { mw.take() } else { None },
                notes,
            );
            if c.pinned {
                s.callisto.as_mut().expect("just built").notes.push(
                    "Separation stated by the source".to_string(),
                );
            }
            s
        })
        .collect();

    let mut system = build_system(
        &primary,
        Some(system_name),
        &primary_plan,
        mw.take(),
        primary_notes,
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

/// One star's `System`: its orbits as slots, the main world in its own.
fn build_system(
    cstar: &CStar,
    name: Option<String>,
    plan: &OrbitPlan,
    main_world: Option<World>,
    notes: Vec<String>,
) -> System {
    let data = cstar.data();
    let star = cstar.star;
    let mut system = System::new(
        star.star_type,
        star.subtype,
        star.size,
        StarOrbit::Primary,
        plan.positions.len(),
    );
    if let Some(n) = name {
        system.name = n;
    }
    let orbits = plan
        .positions
        .iter()
        .map(|&p| OrbitInfo {
            position_hd: p,
            distance_mkm: p * data.hd_mkm,
            zone: Zone::at(p),
        })
        .collect();
    if let (Some(mut w), Some(slot)) = (main_world, plan.main_world) {
        w.orbit = slot;
        w.position_in_system = slot;
        w.orbit_distance_mkm = Some(plan.positions[slot] * data.hd_mkm);
        w.compute_astro_data(&star);
        system.orbit_slots[slot] = Some(OrbitContent::World(w));
    }
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
    }));
    system
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
        if let Some(OrbitContent::World(w)) = s {
            w.orbit = i;
            w.position_in_system = i;
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
    use crate::callisto::dice::{Kind::*, Scripted};

    fn noricum() -> SystemConstraints {
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

    /// Rulebook 12.3, stars and orbits: the rolls it lists, in the order the
    /// generator makes them.
    #[test]
    fn noricum_stars_and_orbits() {
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
        ]);
        let system = generate(noricum(), &mut r).unwrap();
        let layout = system.callisto.as_deref().unwrap();
        let positions: Vec<f32> = layout.orbits.iter().map(|o| o.position_hd).collect();
        // The M6 at 6 HD takes an orbit slot of its own among the planets';
        // the M9 at 200 HD is beyond them.
        //
        // The example keeps 76 and 100, but the M9 at 200 HD has a gap of its
        // own, 67 to 600, which the example overlooks: by the rules both are
        // crossed out, and the 100 HD cap leaves two of the seven orbits
        // unplaced.
        assert_eq!(positions, [0.36, 0.74, 1.4, 2.0, 6.0, 34.0]);
        assert_eq!(layout.crossed_out, [4.5, 7.9, 18.0, 76.0, 100.0]);
        let mw = system.orbit_slots[2].as_ref().unwrap();
        assert!(matches!(mw, OrbitContent::World(w) if w.name == "Noricum"));
        assert!(matches!(system.orbit_slots[4], Some(OrbitContent::Tertiary)));
        assert_eq!(system.secondary.as_ref().unwrap().orbit, StarOrbit::Far);
        assert_eq!(system.tertiary.as_ref().unwrap().orbit, StarOrbit::System(4));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_stated_distance_places_the_main_world() {
        let mut cs = noricum();
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

    #[test]
    fn free_systems_are_deterministic_and_well_formed() {
        for seed in 0..300 {
            let cs = SystemConstraints::from_main_world("Test", "A867977-C").unwrap();
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

