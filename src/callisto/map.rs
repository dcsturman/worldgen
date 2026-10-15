//! What a Callisto world's physics tells its surface map (IMPLEMENTATION.md
//! §5, "map-side decorations").
//!
//! The map generator knows only a UWP, a seed and [`WorldDecorations`]; this
//! turns a body's [`Physics`] into decorations: its mean temperature, its
//! tilt, boiled oceans, fluid (non-water) oceans, and a tidal lock. A two-season world (Section 10.1)
//! gets a map for each end of its year as well as its annual mean.
//!
//! [`Physics`]: crate::callisto::body::Physics

use crate::callisto::rotation::Rotation;
use crate::callisto::temperature::temperature;
use crate::decorations::{TideLock, WorldDecorations};
use crate::systems::system::{OrbitContent, System};
use crate::systems::world::World;

/// Which part of a world's year a map shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Season {
    /// The year's mean.
    #[default]
    Mean,
    /// The near end of an eccentric orbit: summer everywhere at once.
    Near,
    /// The far end: winter.
    Far,
}

impl Season {
    /// `near`, `far`, or anything else (the mean).
    pub fn parse(s: &str) -> Season {
        match s.trim().to_ascii_lowercase().as_str() {
            "near" => Season::Near,
            "far" => Season::Far,
            _ => Season::Mean,
        }
    }
}

/// The map decorations a Callisto world's physics implies, on top of the
/// decorations it already carries (a stated lock keeps its stated substellar
/// point). `None` for a world without Callisto physics.
///
/// A body other than the main world that physics locks to its star gets a
/// locked map; the main world is locked only when a source says so (Section
/// 10.2), which its own decorations already carry. Moons turn with their
/// planet, so a moon's map is never star-locked.
pub fn map_decorations(w: &World, season: Season) -> Option<WorldDecorations> {
    let p = w.callisto.as_deref()?;
    let mut d = w.decorations().clone();
    let spin = p.spin.as_ref();
    let locked = spin.is_some_and(|s| s.rotation == Rotation::StarLocked);
    if locked && d.tide_locked.is_none() && !w.is_mainworld() && p.moon.is_none() {
        d.tide_locked = Some(TideLock::default());
    }
    // The season moves the world along its orbit: near or far end.
    let factor = match (season, spin) {
        (Season::Near, Some(s)) => 1.0 - s.eccentricity,
        (Season::Far, Some(s)) => 1.0 + s.eccentricity,
        _ => 1.0,
    };
    let t = if factor == 1.0 {
        p.temperature?
    } else {
        temperature(p.position_hd * factor, w.atmosphere, w.hydro, p.hydro_is_ice)
    };
    d.mean_temp_c = Some(t.celsius.round().clamp(-273.0, 3000.0) as i16);
    // Oceans boil for good or not at all: whether they have is the year's
    // mean (Section 8.5), never one end of it. A summer map is hot with its
    // seas, not dry.
    d.steam = p.temperature.is_some_and(|m| m.steam);
    // Seas under an atmosphere of A or more aren't water (the Fl trade
    // code; they never boil either, see `temperature`): drawn as
    // ammonia-water against its own freezing point, on barren land.
    d.fluid_oceans = w.atmosphere >= 10 && w.hydro >= 1;
    // A locked world has no meaningful tilt (Section 10.3).
    if d.tide_locked.is_none() {
        d.tilt_deg = spin.map(|s| s.tilt_deg.round().clamp(0.0, 90.0) as u8);
    }
    Some(d)
}

/// Find a body by name anywhere in a system: its orbits, their moons, and
/// its companions'. Names are compared without regard to case. `orbit`, the
/// slot a map request names, breaks a tie between two bodies with one name
/// (two "Planetoid Belt"s).
pub fn find_world<'a>(system: &'a System, name: &str, orbit: Option<usize>) -> Option<&'a World> {
    let mut found: Vec<(usize, &World)> = Vec::new();
    collect(system, name, &mut found);
    found
        .iter()
        .find(|(slot, _)| Some(*slot) == orbit)
        .or(found.first())
        .map(|(_, w)| *w)
}

fn collect<'a>(system: &'a System, name: &str, out: &mut Vec<(usize, &'a World)>) {
    let matches = |w: &World| w.name.eq_ignore_ascii_case(name.trim());
    for (i, slot) in system.orbit_slots.iter().enumerate() {
        let moons: &[World] = match slot {
            Some(OrbitContent::World(w)) => {
                if matches(w) {
                    out.push((i, w));
                }
                &w.satellites.sats
            }
            Some(OrbitContent::GasGiant(g)) => g.satellites(),
            _ => continue,
        };
        out.extend(moons.iter().filter(|m| matches(m)).map(|m| (i, m)));
    }
    for c in [system.secondary.as_deref(), system.tertiary.as_deref()]
        .into_iter()
        .flatten()
    {
        collect(c, name, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{UpstreamSystem, system_from_upstream};

    fn noricum() -> System {
        let (seed, cs) = system_from_upstream(&UpstreamSystem {
            sector: "Trojan Reach",
            hex: "2018",
            name: "Noricum",
            uwp: "D8867BB-1",
            pbg: "804",
            stellar: "G2 V M9 V M6 V",
            worlds: Some(14),
        })
        .unwrap();
        crate::callisto::generate::generate_from_constraints_seeded(seed, cs).unwrap()
    }

    #[test]
    fn the_main_world_gets_its_temperature_and_tilt() {
        let s = noricum();
        let w = find_world(&s, "noricum", None).unwrap();
        assert!(w.is_mainworld());
        let d = map_decorations(w, Season::Mean).unwrap();
        let t = w.callisto.as_deref().unwrap().temperature.unwrap();
        assert_eq!(d.mean_temp_c, Some(t.celsius.round() as i16));
        assert!(d.tilt_deg.is_some());
        assert!(d.tide_locked.is_none(), "a main world is only locked by a source");
    }

    #[test]
    fn a_locked_planet_gets_a_locked_map() {
        let s = noricum();
        let locked = s.orbit_slots.iter().flatten().find_map(|c| match c {
            OrbitContent::World(w)
                if !w.is_mainworld()
                    && w.callisto.as_deref().and_then(|p| p.spin.as_ref()).is_some_and(|sp| sp.rotation == Rotation::StarLocked) =>
            {
                Some(w)
            }
            _ => None,
        });
        let w = locked.expect("Noricum has a planet inside its lock limit");
        let d = map_decorations(w, Season::Mean).unwrap();
        assert!(d.tide_locked.is_some());
        assert_eq!(d.tilt_deg, None);
    }

    /// Pourne's seas aren't water: under atmosphere B they're fluid oceans,
    /// and the main world's map says so. Noricum's are water.
    #[test]
    fn fluid_oceans_under_an_exotic_sky() {
        let (seed, cs) = system_from_upstream(&UpstreamSystem {
            sector: "Trojan Reach",
            hex: "2324",
            name: "Pourne",
            uwp: "A9B2887-A",
            pbg: "902",
            stellar: "F3 V",
            worlds: Some(11),
        })
        .unwrap();
        let s = crate::callisto::generate::generate_from_constraints_seeded(seed, cs).unwrap();
        let d = map_decorations(find_world(&s, "Pourne", None).unwrap(), Season::Mean).unwrap();
        assert!(d.fluid_oceans);
        assert!(!d.steam, "fluid oceans never boil");
        let n = noricum();
        assert!(!map_decorations(find_world(&n, "Noricum", None).unwrap(), Season::Mean).unwrap().fluid_oceans);
    }

    #[test]
    fn a_world_without_physics_has_none() {
        let w = World::from_uwp("Regina", "A788899-C", false, true).unwrap();
        assert_eq!(map_decorations(&w, Season::Mean), None);
    }
}

/// Scan saved TravellerMap sector files for worlds worth looking at.
/// `CALLISTO_SCAN="<sector>=<file.tsv>;…" cargo test --lib -- --ignored callisto_scan --nocapture`
#[cfg(test)]
mod scan {
    use super::*;
    use crate::api::{UpstreamSystem, system_from_upstream};
    use crate::callisto::body::BodyClass;

    #[test]
    #[ignore]
    fn callisto_scan() {
        let spec = std::env::var("CALLISTO_SCAN").unwrap_or_default();
        for pair in spec.split(';').filter(|p| !p.is_empty()) {
            let (sector, path) = pair.split_once('=').unwrap();
            let text = std::fs::read_to_string(path).unwrap();
            let mut lines = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
            let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
            let col = |n: &str| header.iter().position(|h| *h == n).unwrap();
            let (hx, nm, up, pb, st, w) = (col("Hex"), col("Name"), col("UWP"), col("PBG"), col("Stars"), col("W"));
            for l in lines {
                let f: Vec<&str> = l.split('\t').collect();
                let Ok((seed, cs)) = system_from_upstream(&UpstreamSystem {
                    sector, hex: f[hx], name: f[nm], uwp: f[up], pbg: f[pb], stellar: f[st],
                    worlds: f[w].trim().parse().ok(),
                }) else { continue };
                let Ok(sys) = crate::callisto::generate::generate_from_constraints_seeded(seed, cs) else { continue };
                let mut all = Vec::new();
                gather(&sys, &mut all);
                for w in all {
                    let Some(p) = w.callisto.as_deref() else { continue };
                    let Some(t) = p.temperature else { continue };
                    let spin = p.spin.as_ref();
                    let mut tags = Vec::new();
                    if w.is_mainworld() {
                        tags.push("MAIN".to_string());
                        if spin.is_some_and(|s| s.physics_rotation != s.rotation) { tags.push("physics-locked".into()); }
                    } else if spin.is_some_and(|s| s.rotation == Rotation::StarLocked) && w.atmosphere >= 4 && w.atmosphere <= 9 {
                        tags.push("locked-with-air".into());
                    }
                    if let Some((a, b)) = spin.and_then(|s| s.two_season) { tags.push(format!("two-season {}/{}", a.name(), b.name())); }
                    if t.steam { tags.push("steam".into()); }
                    if p.moon.as_ref().is_some_and(|m| m.volcanic) { tags.push("volcanic moon".into()); }
                    if p.class == BodyClass::World && (4..=9).contains(&w.atmosphere) && w.hydro >= 1 && !w.is_mainworld() && t.band == crate::callisto::temperature::TempBand::Temperate { tags.push("second garden world".into()); }
                    let interesting = tags.iter().any(|t| t != "MAIN") || f[nm] == "Hilfer";
                    if interesting {
                        println!("{sector} {} {:<16} {:<14} {} {:>5.0}°C {:?}", f[hx], f[nm], w.name, w.to_uwp(), t.celsius, tags);
                    }
                }
            }
        }
    }

    fn gather<'a>(s: &'a System, out: &mut Vec<&'a World>) {
        for slot in s.orbit_slots.iter().flatten() {
            match slot {
                OrbitContent::World(w) => { out.push(w); out.extend(w.satellites.sats.iter()); }
                OrbitContent::GasGiant(g) => out.extend(g.satellites().iter()),
                _ => {}
            }
        }
        for c in [s.secondary.as_deref(), s.tertiary.as_deref()].into_iter().flatten() { gather(c, out); }
    }
}
