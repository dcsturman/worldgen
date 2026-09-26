//! Validator for `data/overrides.json`.
//!
//! ```text
//! cargo run --bin validate-overrides --features backend
//! cargo run --bin validate-overrides --features backend -- --verbose
//! cargo run --bin validate-overrides --features backend -- --callisto
//! ```
//!
//! `--callisto` generates with Callisto rather than Book 6.
//!
//! Silent on success, non-zero exit on failure. Run it after editing an
//! override, and in CI whenever the file changes.
//!
//! ## What it checks, and why over the network
//!
//! Every override is keyed by `(sector, hex)`, and a mistyped hex is the
//! likeliest authoring error — you're copying coordinates off a page. It is
//! also the most invisible: the override simply applies to nothing, or to
//! somebody else's system, and generation carries on looking perfectly
//! healthy. No offline check can catch that, so this one asks TravellerMap
//! what actually sits at those coordinates and compares it to the `world`
//! field the override carries for exactly this purpose.
//!
//! Having fetched the real stellar and PBG data, it then generates the system
//! through [`worldgen::api::system_from_upstream`] — the same function
//! `/api/system` calls — and asserts nothing was dropped. Generation is
//! seeded from the coordinates, so this isn't sampling one possible outcome:
//! it is *the* outcome, the one production will render, deterministically.
//!
//! ## Whose fault a failure is
//!
//! Reported failures say which, because the fix differs:
//!
//! * **your override** — a wrong hex, a collision with a companion star, a
//!   moon hung off an orbit with nothing in it.
//! * **the generator** — a constraint that was perfectly satisfiable and got
//!   dropped anyway. `OrbitOutOfRange` is the signature: the system should
//!   have grown to reach the orbit and didn't.
//!
//! Conflating them sends you to edit a file that was fine.

use std::process::ExitCode;

use serde::Deserialize;

use worldgen::api::{
    Generator, UpstreamSystem, build_constraints, digit_at, parse_stellar, system_from_upstream,
};
use worldgen::systems::overrides::{self, SystemOverride};
use worldgen::systems::system::{DroppedConstraint, OrbitContent, System};

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(rename = "Worlds")]
    worlds: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "UWP")]
    uwp: String,
    #[serde(rename = "PBG", default)]
    pbg: String,
    #[serde(rename = "Stellar", default)]
    stellar: String,
    #[serde(rename = "Worlds", default)]
    worlds: Option<i32>,
}

/// A problem with one override, and whose it is to fix.
enum Failure {
    /// The override is wrong.
    Author(String),
    /// The generator is wrong.
    Generator(String),
}

async fn fetch(client: &reqwest::Client, o: &SystemOverride) -> Result<Entry, String> {
    let url = format!(
        "{}/api/jumpworlds",
        worldgen::util::travellermap_base_url()
    );
    // `.query()` rather than formatting the string: sector names contain
    // spaces, and reqwest encodes them correctly without another dependency.
    let resp = client
        .get(&url)
        .query(&[
            ("sector", o.sector.as_str()),
            ("hex", o.hex.as_str()),
            ("jump", "0"),
        ])
        .send()
        .await
        .map_err(|e| format!("fetching {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("{url} returned {}", resp.status()));
    }
    let env: Envelope = resp
        .json()
        .await
        .map_err(|e| format!("parsing {url}: {e}"))?;
    env.worlds
        .into_iter()
        .next()
        .ok_or_else(|| format!("no world at {} {}", o.sector, o.hex))
}

fn check(o: &SystemOverride, up: &Entry, verbose: bool, generator: Generator) -> Vec<Failure> {
    let mut out = Vec::new();

    // The coordinate check. This is the one that needs the network, and the
    // one most likely to fire.
    if !up.name.eq_ignore_ascii_case(o.world.trim()) {
        out.push(Failure::Author(format!(
            "hex {} in {} is \"{}\", not \"{}\" — check the coordinates",
            o.hex, o.sector, up.name, o.world
        )));
        // Everything below would be checked against the wrong system.
        return out;
    }

    let upstream = UpstreamSystem {
        sector: &o.sector,
        hex: &o.hex,
        name: &up.name,
        uwp: &up.uwp,
        pbg: &up.pbg,
        stellar: &up.stellar,
        worlds: up.worlds,
    };
    let (seed, cs) = match system_from_upstream(&upstream) {
        Ok(v) => v,
        Err(e) => {
            out.push(Failure::Author(format!("{e}")));
            return out;
        }
    };

    let errors = cs.validate();
    if !errors.is_empty() {
        for e in errors {
            out.push(Failure::Author(format!("{e}")));
        }
        return out;
    }

    let system: System = match generator.generate(seed, cs) {
        Ok(s) => s,
        Err(e) => {
            out.push(Failure::Author(format!("{e}")));
            return out;
        }
    };

    // Did a pinned orbit stretch the system to reach it?
    //
    // `ensure_orbits_for_constraints` grows the orbit list until every pinned
    // body fits, which is what makes a high pin work at all — but it also
    // means a typo'd orbit silently invents orbits that the star would never
    // have had. Pinning to 14 in a system with 10 is not an error the
    // generator can refuse; it's a fact about the source that needs a human
    // to look at it. So compare against what the system would have been
    // without the override.
    let baseline = {
        let stars = parse_stellar(&up.stellar);
        let belts = digit_at(&up.pbg, 1).unwrap_or(0) as usize;
        let giants = digit_at(&up.pbg, 2).unwrap_or(0) as usize;
        let planets = match up.worlds.map(|w| w.min(64)) {
            Some(w) => (w - 1 - belts as i32 - giants as i32).max(0) as usize,
            None => 0,
        };
        build_constraints(&up.name, &up.uwp, &stars, giants, belts, planets)
            .ok()
            .and_then(|cs| generator.generate(seed, cs).ok())
            .map(|s| s.orbit_slots.len())
    };
    // Book 6 only: Callisto adds an orbit for a pinned body that lands where
    // it has none, by design (see `callisto::populate::place_pins`).
    if let Some(natural) = baseline
        && generator == Generator::Book6
        && system.orbit_slots.len() > natural
    {
        out.push(Failure::Author(format!(
            "this override stretches the system from {natural} orbits to {} — check the \
             pinned orbits are real and not a typo",
            system.orbit_slots.len()
        )));
    }

    for d in system.dropped_constraints() {
        // The diagnostic variant already carries the distinction, so there's
        // no need for a separate solvability pass: a constraint that was
        // satisfiable and got dropped anyway shows up as OrbitOutOfRange,
        // which means the system failed to grow to reach it.
        match d {
            DroppedConstraint::OrbitOutOfRange { .. } | DroppedConstraint::NoFreeOrbit { .. } => {
                out.push(Failure::Generator(format!("{d}")));
            }
            _ => out.push(Failure::Author(format!("{d}"))),
        }
    }

    // Printed whether or not it passed. A failure is precisely when you want
    // to see the system that produced it — the first version only printed on
    // success, which is backwards.
    if verbose {
        println!(
            "  {} {} — {} ok  [{}]",
            o.sector, o.hex, up.name, up.stellar
        );
        for (i, slot) in system.orbit_slots.iter().enumerate() {
            // One line per body. The full Debug of a World runs to a
            // paragraph, which at a dozen bodies a system buries the thing
            // you opened the output to look at.
            let line = match slot {
                Some(OrbitContent::World(w)) => {
                    let main = if w.is_mainworld() { " [main]" } else { "" };
                    let f = w.facilities_string();
                    let f = if f.trim().is_empty() {
                        String::new()
                    } else {
                        format!("  [{}]", f.trim())
                    };
                    format!("{:<22} {}{}{}", w.name, w.to_uwp(), main, f)
                }
                Some(OrbitContent::GasGiant(g)) => format!("{:<22} gas giant", g.name),
                Some(OrbitContent::Secondary) => "companion star".to_string(),
                Some(OrbitContent::Tertiary) => "companion star (tertiary)".to_string(),
                Some(OrbitContent::Blocked) => continue,
                None => continue,
            };
            println!("    orbit {i:>2}: {line}");
            // Moons too: an override can attach one, and without showing it
            // there is no way to see that the thing you just wrote landed.
            let sats: &[_] = match slot {
                Some(OrbitContent::World(w)) => &w.satellites.sats,
                Some(OrbitContent::GasGiant(g)) => g.satellites(),
                _ => &[],
            };
            for m in sats {
                println!("             moon: {:<15} {} (sat orbit {})", m.name, m.to_uwp(), m.orbit);
            }
        }
        // Companions are systems in their own right; their bodies are
        // invisible from the primary's slots.
        for (label, child) in [
            ("secondary", system.secondary.as_deref()),
            ("tertiary", system.tertiary.as_deref()),
        ] {
            let Some(child) = child else { continue };
            println!("    -- {label}: {} --", child.star_name());
            for (i, slot) in child.orbit_slots.iter().enumerate() {
                let line = match slot {
                    Some(OrbitContent::World(w)) => format!("{:<22} {}", w.name, w.to_uwp()),
                    Some(OrbitContent::GasGiant(g)) => format!("{:<22} gas giant", g.name),
                    _ => continue,
                };
                println!("       orbit {i:>2}: {line}");
                let sats: &[_] = match slot {
                    Some(OrbitContent::World(w)) => &w.satellites.sats,
                    Some(OrbitContent::GasGiant(g)) => g.satellites(),
                    _ => &[],
                };
                for m in sats {
                    println!("                moon: {:<15} {} (sat orbit {})", m.name, m.to_uwp(), m.orbit);
                }
            }
        }
    }
    out
}

#[tokio::main]
async fn main() -> ExitCode {
    let verbose = std::env::args().any(|a| a == "--verbose" || a == "-v");
    let generator = if std::env::args().any(|a| a == "--callisto") {
        Generator::Callisto
    } else {
        Generator::Book6
    };

    let all = overrides::all();
    if all.is_empty() {
        if verbose {
            println!("no overrides to validate");
        }
        return ExitCode::SUCCESS;
    }

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("worldgen-override-validator/1.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("could not build HTTP client: {e}");
            return ExitCode::FAILURE;
        }
    };

    if verbose {
        println!(
            "validating {} override(s) against {}",
            all.len(),
            worldgen::util::travellermap_base_url()
        );
    }

    // Concurrent: the network round-trip dominates, and generation itself is
    // milliseconds once the data is in hand.
    let fetches = all.iter().map(|o| {
        let client = &client;
        async move { (*o, fetch(client, o).await) }
    });
    let results = futures_util::future::join_all(fetches).await;

    let mut failed = 0usize;
    for (o, fetched) in results {
        let failures = match fetched {
            Ok(up) => check(o, &up, verbose, generator),
            Err(e) => vec![Failure::Author(e)],
        };
        if failures.is_empty() {
            continue;
        }
        failed += 1;
        eprintln!("{} {} ({}):", o.sector, o.hex, o.world);
        for f in failures {
            match f {
                Failure::Author(m) => eprintln!("  override: {m}"),
                Failure::Generator(m) => eprintln!("  GENERATOR BUG: {m}"),
            }
        }
    }

    if failed > 0 {
        eprintln!("\n{failed} of {} override(s) failed", all.len());
        return ExitCode::FAILURE;
    }
    if verbose {
        println!("all {} override(s) ok", all.len());
    }
    ExitCode::SUCCESS
}
