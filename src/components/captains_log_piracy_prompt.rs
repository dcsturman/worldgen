//! Build the *pirate* captain's-log prompt sent to the captain's-log model.
//!
//! Companion to [`crate::components::captains_log_prompt`], but for the
//! piracy simulation. It walks the [`SimulationStep`] stream and renders a
//! compact chronological brief of the raiding cruise — raids, threat
//! evasions, fence deals, reputation swings — prefixed with the pirate
//! [`INSTRUCTIONS`] header. The string is sent as-is to the backend, which
//! forwards it as the user message.
//!
//! No I/O, no async, no native deps — wasm-friendly.

use std::fmt::Write as _;

use crate::components::captains_log_piracy_instructions::INSTRUCTIONS;
use crate::simulator::types::{Action, SimulationParams, SimulationResult, SimulationStep};
use crate::trade::ZoneClassification;

/// Build the full pirate-log prompt string from a completed simulation.
///
/// `ship_name` is taken from the simulator's `Ship::name` field. If blank,
/// the data line tells the model to invent one in keeping with Traveller
/// corsair naming.
pub fn build_piracy_prompt(
    ship_name: &str,
    params: &SimulationParams,
    steps: &[SimulationStep],
    result: &SimulationResult,
) -> String {
    // Instruction header is ~6 KB; each cruise line is short. Budget for a
    // long cruise.
    let mut out = String::with_capacity(6_500 + steps.len() * 120);

    out.push_str(INSTRUCTIONS);
    out.push_str("\n== CRUISE DATA ==\n\n");
    write_cruise_header(&mut out, ship_name, params, result);

    out.push_str("\n== CRUISE EVENTS (chronological) ==\n");
    for step in steps {
        write_event(&mut out, step);
    }

    out.push_str("\n== END CRUISE DATA ==\n\nNow write the corsair's log.\n");
    out
}

fn write_cruise_header(
    out: &mut String,
    ship_name: &str,
    params: &SimulationParams,
    result: &SimulationResult,
) {
    let trimmed = ship_name.trim();
    if trimmed.is_empty() {
        out.push_str(
            "Ship: (unregistered — invent a name in keeping with Traveller corsair naming and use it consistently throughout)\n",
        );
    } else {
        let _ = writeln!(out, "Ship: {trimmed}");
    }

    let home = &params.home_world;
    let _ = writeln!(
        out,
        "Hideout / home base: {} ({}, hex {:02}{:02}), UWP {}, {} zone",
        home.name,
        home.sector,
        home.hex_x,
        home.hex_y,
        home.uwp,
        zone_label(home.zone),
    );

    let _ = writeln!(out, "Doctrine (attitude): {}", params.attitude.label());
    let _ = writeln!(out, "Cruise start: {}", params.start_date.format());
    let _ = writeln!(out, "Cruise end:   {}", result.end_date.format());
    let days = params.start_date.days_until(result.end_date).max(0);
    let _ = writeln!(out, "Days on cruise: {days}");
    let _ = writeln!(out, "Total jumps: {}", result.jumps);

    let s = &params.ship;
    let _ = writeln!(
        out,
        "Ship config: J-{}, thrust {}, {} crew, weapons rating {}, leadership {}.",
        s.jump_rating, s.thrust, s.crew_size, s.weapons, s.leadership_skill,
    );

    let _ = writeln!(out, "Starting budget: {} Cr", params.starting_budget);
    let _ = writeln!(out, "Final budget:    {} Cr", result.final_budget);
    let _ = writeln!(
        out,
        "Final reputation: {:.1}",
        result.final_reputation
    );
    let _ = writeln!(
        out,
        "Total loot fenced: {} Cr",
        result.total_loot_fenced
    );
    let _ = writeln!(out, "Raids that took a haul: {}", result.raids);
    let _ = writeln!(out, "Ships destroyed: {}", result.ships_destroyed);

    if result.marooned
        && let (Some(loc), Some(on)) = (result.marooned_at.as_ref(), result.marooned_on)
    {
        let signal_arrives = result
            .rescue_arrives_on
            .map(|d| d.format())
            .unwrap_or_else(|| "unknown".to_string());
        let _ = writeln!(
            out,
            "MAROONED at {} on {}. Distress signal will reach home ({}) on {}. Actual rescue would take additional time after that.",
            loc.name,
            on.format(),
            params.home_world.name,
            signal_arrives,
        );
    }
}

/// Emit one concise line per cruise-relevant step. Merchant-only actions
/// (Sell/Buy/Hold/Freight/Pax/life-support) never appear in a pirate run,
/// so they're silently skipped.
fn write_event(out: &mut String, step: &SimulationStep) {
    let date = step.date.format();
    let here = &step.location.name;
    match &step.action {
        Action::Arrive {
            from, distance, ..
        } => {
            let _ = writeln!(
                out,
                "{date} — Jumped into {here} from {} ({} pc).",
                from.name, distance
            );
        }
        Action::EncounterResolved {
            encounter,
            target_hull_tons,
            mor_total,
            menace,
            surrender_margin,
            act_tier,
            loot_value,
            went_loud,
            pirate_damage_credits,
            target_escaped,
            ..
        } => {
            if *target_escaped {
                let _ = writeln!(
                    out,
                    "{date} @ {here} — Spotted prey ({}, ~{}t) but she had the legs on us and jumped clear.",
                    encounter.label(),
                    target_hull_tons
                );
            } else if let Some(tier) = act_tier {
                let loud = if *went_loud { "; it went loud" } else { "" };
                let dmg = if *pirate_damage_credits > 0 {
                    format!("; we took {pirate_damage_credits} Cr of damage")
                } else {
                    String::new()
                };
                let _ = writeln!(
                    out,
                    "{date} @ {here} — Raided a {} (~{}t): morale {}, menace {}, surrender margin {}. We {} — +{} Cr of loot{}{}.",
                    encounter.label(),
                    target_hull_tons,
                    mor_total,
                    menace,
                    surrender_margin,
                    tier.label(),
                    loot_value,
                    loud,
                    dmg
                );
            } else {
                let _ = writeln!(
                    out,
                    "{date} @ {here} — Closed on a {} (~{}t) but broke off without a take (surrender margin {}).",
                    encounter.label(),
                    target_hull_tons,
                    surrender_margin
                );
            }
        }
        Action::ThreatEncounter {
            threat,
            q_ship,
            recognized,
            escape_margin,
            outcome,
            damage_credits,
            weeks_lost,
            ..
        } => {
            let who = if *q_ship {
                format!("{} (a disguised q-ship)", threat.label())
            } else {
                threat.label().to_string()
            };
            let made = if *recognized {
                "we were recognized"
            } else {
                "we kept our cool"
            };
            let dmg = if *damage_credits > 0 {
                format!(", {damage_credits} Cr damage")
            } else {
                String::new()
            };
            let weeks = if *weeks_lost > 0 {
                format!(", {weeks_lost} weeks lost")
            } else {
                String::new()
            };
            let _ = writeln!(
                out,
                "{date} @ {here} — Defender: {who}; {made} (escape margin {escape_margin}) — {outcome}{dmg}{weeks}.",
            );
        }
        Action::FenceAttempt {
            law_level,
            seized,
            payout_pct,
            cargo_value,
            payout,
            tons_disposed,
            ..
        } => {
            if *seized {
                let _ = writeln!(
                    out,
                    "{date} @ {here} (law {law_level}) — Fence sting: {tons_disposed}t (worth {cargo_value} Cr) seized, nothing realized.",
                );
            } else {
                let _ = writeln!(
                    out,
                    "{date} @ {here} (law {law_level}) — Fenced {tons_disposed}t (worth {cargo_value} Cr) at {payout_pct}% → +{payout} Cr.",
                );
            }
        }
        Action::ReputationChange {
            delta,
            new_value,
            reason,
        } => {
            let _ = writeln!(
                out,
                "{date} @ {here} — Reputation {}{:.1} → {:.1} ({reason}).",
                if *delta >= 0.0 { "+" } else { "" },
                delta,
                new_value
            );
        }
        Action::PayPeriodic {
            maintenance,
            salary,
            mortgage,
            period_index,
        } => {
            let total = maintenance + salary + mortgage;
            let _ = writeln!(
                out,
                "{date} @ {here} — Month {}: paid {total} Cr upkeep (maintenance {maintenance} + crew {salary} + mortgage {mortgage}).",
                period_index + 1
            );
        }
        Action::Marooned {
            budget,
            total_parsecs_jumped,
            rescue_eta_days,
            rescue_arrives_on,
        } => {
            let _ = writeln!(
                out,
                "{date} @ {here} — MAROONED — budget {budget} Cr, {total_parsecs_jumped} pc travelled; mayday reaches home on {} ({rescue_eta_days} days).",
                rescue_arrives_on.format()
            );
        }
        // Everything else (merchant trade actions, jumps, quiet scouting,
        // analytics-only incident variants) is not narrated for a pirate
        // cruise.
        _ => {}
    }
}

fn zone_label(z: ZoneClassification) -> &'static str {
    match z {
        ZoneClassification::Green => "Green",
        ZoneClassification::Amber => "Amber",
        ZoneClassification::Red => "Red",
    }
}
