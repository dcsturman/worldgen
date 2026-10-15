//! Callisto's review file: every world whose published data strains the
//! physics, one sector at a time (IMPLEMENTATION.md §7).
//!
//! ```text
//! cargo run --bin callisto-review --features backend -- "Trojan Reach"
//! cargo run --bin callisto-review --features backend -- "Trojan Reach" "Spinward Marches"
//! ```
//!
//! Fetches each sector from TravellerMap (`TRAVELLERMAP_URL`), generates every
//! system with Callisto exactly as `/api/system` does, and writes
//! `docs/callisto/review/<sector>.md`, replacing what was there: the file is
//! the current state of the published data, not a log. It is deterministic,
//! so a rerun after a rules change shows up as a diff.

use std::fmt::Write as _;
use std::process::ExitCode;

use worldgen::api::{Generator, UpstreamSystem, system_from_upstream};
use worldgen::callisto::body::Fit;
use worldgen::systems::system::{OrbitContent, System};
use worldgen::systems::world::World;

/// One strained or odd world.
struct Entry {
    hex: String,
    system: String,
    world: String,
    uwp: String,
    stars: String,
    story: String,
}

/// A row of TravellerMap's tab-delimited sector file.
struct Row {
    hex: String,
    name: String,
    uwp: String,
    pbg: String,
    stars: String,
    worlds: Option<i32>,
}

fn parse_sector(text: &str) -> Result<Vec<Row>, String> {
    let mut lines = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let header: Vec<&str> = lines.next().ok_or("empty sector file")?.split('\t').collect();
    let col = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .ok_or_else(|| format!("sector file has no {name} column"))
    };
    let (hex, name, uwp, pbg, stars, w) =
        (col("Hex")?, col("Name")?, col("UWP")?, col("PBG")?, col("Stars")?, col("W")?);
    Ok(lines
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let get = |i: usize| f.get(i).map_or("", |s| s.trim()).to_string();
            Row {
                hex: get(hex),
                name: get(name),
                uwp: get(uwp),
                pbg: get(pbg),
                stars: get(stars),
                worlds: get(w).parse().ok(),
            }
        })
        .collect())
}

/// Every world in `system`: orbit slots, their moons, and companions' too.
fn worlds(system: &System) -> Vec<&World> {
    let mut out = Vec::new();
    for slot in system.orbit_slots.iter().flatten() {
        match slot {
            OrbitContent::World(w) => {
                out.push(w);
                out.extend(w.satellites.sats.iter());
            }
            OrbitContent::GasGiant(g) => out.extend(g.satellites().iter()),
            _ => {}
        }
    }
    for c in [system.secondary.as_deref(), system.tertiary.as_deref()]
        .into_iter()
        .flatten()
    {
        out.extend(worlds(c));
    }
    out
}

/// What a sector's review found.
#[derive(Default)]
struct Review {
    strained: Vec<Entry>,
    odd: Vec<Entry>,
    /// Systems where Callisto dropped a published fact that Book 6 kept
    /// (IMPLEMENTATION.md §10): "system (hex): what".
    dropped: Vec<String>,
    failures: Vec<String>,
}

fn review(sector: &str, rows: &[Row]) -> Review {
    let mut out = Review::default();
    let (entries, odd, failures) = (&mut out.strained, &mut out.odd, &mut out.failures);
    for r in rows {
        let upstream = UpstreamSystem {
            sector,
            hex: &r.hex,
            name: &r.name,
            uwp: &r.uwp,
            pbg: &r.pbg,
            stellar: &r.stars,
            worlds: r.worlds,
        };
        let generated = system_from_upstream(&upstream).map_err(|e| e.to_string()).and_then(|(seed, cs)| {
            let book6 = Generator::Book6.generate(seed, cs.clone()).ok();
            Generator::Callisto
                .generate(seed, cs)
                .map(|s| (s, book6))
                .map_err(|e| e.to_string())
        });
        let system = generated.map(|(s, book6)| {
            let book6_dropped = book6.map_or(0, |b| b.dropped_constraints().len());
            let dropped = s.dropped_constraints();
            if !dropped.is_empty() && book6_dropped == 0 {
                let what: Vec<String> = dropped.iter().map(|d| d.to_string()).collect();
                out.dropped.push(format!("{} ({}): {}", r.name, r.hex, what.join("; ")));
            }
            s
        });
        let system = match system {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{} ({}): {e}", r.name, r.hex));
                continue;
            }
        };
        for w in worlds(&system) {
            let Some(p) = w.callisto.as_deref() else { continue };
            let entry = |story: String| Entry {
                hex: r.hex.clone(),
                system: r.name.clone(),
                world: w.name.clone(),
                uwp: w.to_uwp(),
                stars: r.stars.clone(),
                story,
            };
            if let Fit::Strained { story } = &p.fit {
                entries.push(entry(story.clone()));
            }
            if !p.oddities.is_empty() {
                odd.push(entry(p.oddities.join("; ")));
            }
        }
    }
    out
}

fn render(sector: &str, systems: usize, review: &Review) -> String {
    let (entries, odd, failures) = (&review.strained, &review.odd, &review.failures);
    let mut out = String::new();
    let _ = writeln!(out, "# Callisto review: {sector}\n");
    let _ = writeln!(
        out,
        "Worlds whose published data strains the physics, from `cargo run --bin \
         callisto-review --features backend -- \"{sector}\"`. Replaced on every run. \
         {systems} systems, {} strained world(s), {} odd.\n",
        entries.len(),
        odd.len()
    );
    let _ = writeln!(
        out,
        "**Strained** worlds are ones where a published fact forces something the physics \
         can't support: the data to check first. **Oddities** have an easy story (rulebook \
         Section 14.2), or are what the rules themselves leave odd.\n"
    );
    if !entries.is_empty() {
        let _ = writeln!(out, "# Strained\n");
    }
    for e in entries {
        let who = if e.world == e.system {
            e.world.clone()
        } else {
            format!("{} (in the {} system)", e.world, e.system)
        };
        let _ = writeln!(out, "## {} {who}\n", e.hex);
        let _ = writeln!(out, "- **Published:** {} around {}", e.uwp, e.stars);
        let _ = writeln!(out, "- **Physics and story:** {}\n", e.story);
    }
    if !odd.is_empty() {
        let _ = writeln!(out, "# Oddities\n");
    }
    for e in odd {
        let who = if e.world == e.system {
            e.world.clone()
        } else {
            format!("{} (in the {} system)", e.world, e.system)
        };
        let _ = writeln!(out, "## {} {who}\n", e.hex);
        let _ = writeln!(out, "- **Published:** {} around {}", e.uwp, e.stars);
        let _ = writeln!(out, "- **Story:** {}\n", e.story);
    }
    // IMPLEMENTATION.md §10: a system Book 6 generates with every published
    // fact must not lose one under Callisto.
    let _ = writeln!(out, "# Published facts dropped\n");
    if review.dropped.is_empty() {
        let _ = writeln!(out, "None: every published fact Book 6 keeps, Callisto keeps.\n");
    }
    for d in &review.dropped {
        let _ = writeln!(out, "- {d}");
    }
    if !review.dropped.is_empty() {
        let _ = writeln!(out);
    }
    if !failures.is_empty() {
        let _ = writeln!(out, "# Systems that did not generate\n");
        for f in failures {
            let _ = writeln!(out, "- {f}");
        }
    }
    out
}

fn slug(sector: &str) -> String {
    sector
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[tokio::main]
async fn main() -> ExitCode {
    let sectors: Vec<String> = std::env::args().skip(1).collect();
    if sectors.is_empty() {
        eprintln!("usage: callisto-review <sector> [<sector>…]");
        return ExitCode::FAILURE;
    }
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("worldgen-callisto-review/1.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("could not build HTTP client: {e}");
            return ExitCode::FAILURE;
        }
    };
    let dir = std::path::Path::new("docs/callisto/review");
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("could not create {}: {e}", dir.display());
        return ExitCode::FAILURE;
    }
    let base = worldgen::util::travellermap_base_url();
    for sector in &sectors {
        let url = format!("{base}/api/sec");
        let text = match client
            .get(&url)
            .query(&[("sector", sector.as_str()), ("type", "TabDelimited")])
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
            Ok(r) => {
                eprintln!("{sector}: {url} returned {}", r.status());
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("{sector}: fetching {url}: {e}");
                return ExitCode::FAILURE;
            }
        };
        let rows = match parse_sector(&text) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{sector}: {e}");
                return ExitCode::FAILURE;
            }
        };
        let review = review(sector, &rows);
        let path = dir.join(format!("{}.md", slug(sector)));
        if let Err(e) = std::fs::write(&path, render(sector, rows.len(), &review)) {
            eprintln!("could not write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!(
            "{sector}: {} systems, {} strained, {} odd, {} dropped, {} failed -> {}",
            rows.len(),
            review.strained.len(),
            review.odd.len(),
            review.dropped.len(),
            review.failures.len(),
            path.display()
        );
    }
    ExitCode::SUCCESS
}
