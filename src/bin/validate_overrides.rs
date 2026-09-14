//! Validator for `data/overrides.json`.
//!
//! ```text
//! cargo run --bin validate-overrides --features backend
//! cargo run --bin validate-overrides --features backend -- --verbose
//! ```
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

use worldgen::api::{UpstreamSystem, system_from_upstream};
use worldgen::systems::overrides::{self, SystemOverride};
use worldgen::systems::system::{DroppedConstraint, System};

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

fn check(o: &SystemOverride, up: &Entry, verbose: bool) -> Vec<Failure> {
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

    let system = match System::generate_from_constraints_seeded(seed, cs) {
        Ok(s) => s,
        Err(errs) => {
            for e in errs {
                out.push(Failure::Author(format!("{e}")));
            }
            return out;
        }
    };

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

    if verbose && out.is_empty() {
        println!("  {} {} — {} ok", o.sector, o.hex, up.name);
        for (i, slot) in system.orbit_slots.iter().enumerate() {
            if let Some(c) = slot {
                println!("    orbit {i:>2}: {c:?}");
            }
        }
    }
    out
}

#[tokio::main]
async fn main() -> ExitCode {
    let verbose = std::env::args().any(|a| a == "--verbose" || a == "-v");

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
            Ok(up) => check(o, &up, verbose),
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
