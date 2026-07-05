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

use crate::components::captains_log_piracy_instructions::{LogTone, instructions};
use crate::simulator::types::{Action, SimulationParams, SimulationResult, SimulationStep};
use crate::trade::ZoneClassification;

/// Build the full pirate-log prompt string from a completed simulation.
///
/// `ship_name` and `captain_name` are taken from the simulator's
/// `Ship::name` / `Ship::captain_name` fields. When either is blank the data
/// line tells the model to invent one in keeping with Traveller corsair
/// naming. `tone` selects the tonal register of the instruction header.
pub fn build_piracy_prompt(
    ship_name: &str,
    captain_name: &str,
    params: &SimulationParams,
    steps: &[SimulationStep],
    result: &SimulationResult,
    tone: LogTone,
) -> String {
    let header = instructions(tone);
    // Instruction header is ~7 KB; each cruise line is short. Budget for a
    // long cruise.
    let mut out = String::with_capacity(header.len() + 600 + steps.len() * 120);

    out.push_str(&header);
    out.push_str("\n== CRUISE DATA ==\n\n");
    write_cruise_header(&mut out, ship_name, captain_name, params, result);

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
    captain_name: &str,
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

    let captain = captain_name.trim();
    if captain.is_empty() {
        out.push_str(
            "Captain: (unnamed — invent a name per the distribution in the instructions and use it consistently throughout)\n",
        );
    } else {
        let _ = writeln!(out, "Captain: {captain}");
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
    if !result.prizes.is_empty() {
        let _ = writeln!(out, "Prizes brought home: {}", result.prizes.len());
        for p in &result.prizes {
            let _ = writeln!(
                out,
                "  - {} (~{}t), {}% hull",
                p.class_name,
                p.hull_tons,
                (p.condition * 100.0).round() as i32
            );
        }
    }

    if let Some(dest) = &params.destination {
        let reached = result.returned_home && !result.marooned;
        let _ = writeln!(
            out,
            "Cruise destination (one-way run — made for here to finish, NOT a round trip to the hideout): {} ({}, hex {:02}{:02}), UWP {}, {} zone — {}",
            dest.name,
            dest.sector,
            dest.hex_x,
            dest.hex_y,
            dest.uwp,
            zone_label(dest.zone),
            if reached { "REACHED" } else { "did not reach" },
        );
    }

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
        Action::PreyPassed {
            class_name,
            hull_tons,
            reason,
            ..
        } => {
            use crate::simulator::types::PreyPassedReason as PP;
            let tail = match reason {
                PP::HoldFull => "the hold was full; let her go and made for a fence",
                PP::LyingLow => "the heat was too high; let her go and ran for the hideout to lie low",
            };
            let _ = writeln!(
                out,
                "{date} @ {here} — Sighted a {class_name} (~{hull_tons}t) but {tail}."
            );
        }
        Action::EncounterResolved {
            encounter,
            class_name,
            target_hull_tons,
            mor_total,
            resistance,
            outcome,
            act_tier,
            loot_value,
            pirate_damage_credits,
            weeks_lost,
            ..
        } => {
            use crate::simulator::types::{EncounterOutcome as EO, EncounterType};
            let ship = if *encounter == EncounterType::RichFreighter {
                format!("richly-laden {class_name}")
            } else {
                class_name.clone()
            };
            let dmg = if *pirate_damage_credits > 0 {
                format!(" We took {pirate_damage_credits} Cr of damage to repair.")
            } else {
                String::new()
            };
            let time = if *weeks_lost > 0 {
                format!(" The fight cost us {weeks_lost} weeks.")
            } else {
                String::new()
            };
            match outcome {
                EO::PreyJumpedClear => {
                    let _ = writeln!(
                        out,
                        "{date} @ {here} — Spotted prey (a {ship}, ~{target_hull_tons}t) but she had the legs on us and jumped clear.",
                    );
                }
                EO::BrokeOffOutgunned => {
                    let _ = writeln!(
                        out,
                        "{date} @ {here} — Sized up a {ship} (~{target_hull_tons}t, morale {mor_total}) and judged it not worth the fight; broke off.",
                    );
                }
                EO::DrivenOffMauled => {
                    let _ = writeln!(
                        out,
                        "{date} @ {here} — Pressed a {ship} (~{target_hull_tons}t, morale {mor_total}, resistance {resistance}) but they fought back hard and drove us off with nothing.{dmg}{time}",
                    );
                }
                EO::Surrendered | EO::FoughtAndWon => {
                    let how = if *outcome == EO::FoughtAndWon {
                        format!("Took a {ship} (~{target_hull_tons}t) after a fight (resistance {resistance})")
                    } else {
                        format!("A {ship} (~{target_hull_tons}t, morale {mor_total}) yielded without a fight")
                    };
                    let deed = act_tier.map(|t| t.label()).unwrap_or("plundered her");
                    let _ = writeln!(
                        out,
                        "{date} @ {here} — {how}: {deed} — +{loot_value} Cr of loot.{dmg}{time}",
                    );
                }
            }
        }
        Action::ThreatEncounter {
            class_name,
            q_ship,
            recognized,
            escape_margin,
            outcome,
            damage_credits,
            weeks_lost,
            ..
        } => {
            let who = if *q_ship {
                format!("{class_name} (a disguised q-ship)")
            } else {
                class_name.clone()
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
        Action::PrizeTaken {
            class_name,
            hull_tons,
            condition_pct,
            ..
        } => {
            let _ = writeln!(
                out,
                "{date} @ {here} — Seized the {class_name} (~{hull_tons}t) whole as a prize, {condition_pct}% hull, to fly home.",
            );
        }
        Action::PrizeDeclined {
            class_name,
            hull_tons,
            ..
        } => {
            let _ = writeln!(
                out,
                "{date} @ {here} — Had the {class_name} (~{hull_tons}t) dead in space and worth taking, but every prize crew was already committed; left her behind.",
            );
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
            reason,
            budget,
            total_parsecs_jumped,
            rescue_eta_days,
            rescue_arrives_on,
        } => {
            let _ = writeln!(
                out,
                "{date} @ {here} — MAROONED ({reason}) — budget {budget} Cr, {total_parsecs_jumped} pc travelled; mayday reaches home on {} ({rescue_eta_days} days).",
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
