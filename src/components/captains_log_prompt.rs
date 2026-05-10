//! Build the captain's-log prompt sent to `gemini-3-flash-preview`.
//!
//! This module is purely client-side text assembly: it walks the
//! [`SimulationStep`] stream, coalesces consecutive steps at the same
//! world into per-port visit blocks, and renders the whole thing into a
//! single prompt string. The string is then sent (as-is) to the backend,
//! which forwards it as the user message of `streamGenerateContent`.
//!
//! The static instruction header (tone, name distribution, canon rules,
//! marooned hook, etc.) lives in
//! [`crate::components::captains_log_instructions`] so it can be edited
//! without touching this file's serialization logic.
//!
//! No I/O, no async, no native deps — wasm-friendly.

use std::fmt::Write as _;

use crate::components::captains_log_instructions::INSTRUCTIONS;
use crate::simulator::types::{
    Action, Date, SimulationParams, SimulationResult, SimulationStep, WorldRef,
};
use crate::trade::{TradeClass, ZoneClassification, uwp_to_trade_classes};

/// Build the full prompt string from a completed simulation.
///
/// `ship_name` is taken from the simulator's `Ship::name` field. If
/// blank, the data line tells the model to invent one in keeping with
/// Traveller free-trader naming.
pub fn build_prompt(
    ship_name: &str,
    params: &SimulationParams,
    steps: &[SimulationStep],
    result: &SimulationResult,
) -> String {
    // Pre-size: instruction header is ~5 KB, each visit averages ~600
    // bytes; budget conservatively for 25 visits.
    let mut out = String::with_capacity(5_500 + 25 * 800);

    out.push_str(INSTRUCTIONS);
    out.push_str("\n== VOYAGE DATA ==\n\n");
    write_voyage_header(&mut out, ship_name, params, result);
    write_ship_config(&mut out, params);

    let visits = coalesce_visits(steps);
    out.push_str("\n== PORT VISITS (chronological) ==\n");
    for (idx, v) in visits.iter().enumerate() {
        let label = if idx == visits.len() - 1 && idx > 0 && world_eq(&v.world, &params.home_world)
        {
            format!("--- Visit {} (return home) ---", idx + 1)
        } else {
            format!("--- Visit {} ---", idx + 1)
        };
        write_visit(&mut out, &label, v);
    }

    out.push_str("\n== END VOYAGE DATA ==\n\nNow write the captain's log.\n");
    out
}

// ---------------------------------------------------------------------
// Voyage header / ship config
// ---------------------------------------------------------------------

fn write_voyage_header(
    out: &mut String,
    ship_name: &str,
    params: &SimulationParams,
    result: &SimulationResult,
) {
    let trimmed = ship_name.trim();
    if trimmed.is_empty() {
        out.push_str(
            "Ship: (unregistered — invent a name in keeping with Traveller free-trader naming and use it consistently throughout)\n",
        );
    } else {
        let _ = writeln!(out, "Ship: {trimmed}");
    }

    let home = &params.home_world;
    let _ = writeln!(
        out,
        "Home world: {} ({}, hex {:02}{:02}), UWP {}, {} zone",
        home.name,
        home.sector,
        home.hex_x,
        home.hex_y,
        home.uwp,
        zone_label(home.zone),
    );
    let tcs = trade_classes_for(&home.uwp);
    if !tcs.is_empty() {
        let _ = writeln!(out, "  Trade classes: {}", tcs.join(", "));
    }

    let _ = writeln!(out, "Voyage start: {}", params.start_date.format());
    let _ = writeln!(out, "Voyage end:   {}", result.end_date.format());
    let days = params.start_date.days_until(result.end_date).max(0);
    let _ = writeln!(out, "Days on voyage: {days}");
    let _ = writeln!(out, "Total jumps: {}", result.jumps);
    let _ = writeln!(out, "Starting budget: {} Cr", params.starting_budget);
    let _ = writeln!(out, "Final budget:    {} Cr", result.final_budget);
    let _ = writeln!(out, "Gross profit:    {} Cr", result.gross_profit);
    let _ = writeln!(out, "Owner profit:    {} Cr", result.owner_profit);
    let _ = writeln!(
        out,
        "Crew share:       {} Cr ({:.0}%)",
        result.crew_share,
        params.crew_profit_share * 100.0
    );
    let _ = writeln!(
        out,
        "Returned home: {}",
        if result.returned_home { "yes" } else { "no" }
    );
    let _ = writeln!(
        out,
        "Went negative during voyage: {}",
        if result.went_negative { "yes" } else { "no" }
    );

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

fn write_ship_config(out: &mut String, params: &SimulationParams) {
    let s = &params.ship;
    let _ = writeln!(
        out,
        "\nShip config: {}t cargo, J-{}, {} crew, {} passenger staterooms, {} low berths,\n  weapons rating {}, broker {}, steward {}, leadership {}.",
        s.cargo_capacity,
        s.jump_rating,
        s.crew_size,
        s.passenger_staterooms,
        s.low_berths,
        s.weapons,
        s.broker_skill,
        s.steward_skill,
        s.leadership_skill,
    );
}

// ---------------------------------------------------------------------
// Per-visit coalescing
// ---------------------------------------------------------------------

/// One coalesced port visit: a contiguous run of steps at the same world.
///
/// "Same world" is identity by `(sector, hex_x, hex_y)` so revisits show
/// up as separate visits even if the name string matches.
struct Visit<'a> {
    world: WorldRef,
    arrived: Date,
    departed: Date,

    sells: Vec<SoldGood>,
    holds: Vec<HeldGood>,
    buys: Vec<BoughtGood>,
    freight_tons: i32,
    freight_lots: u32,
    freight_revenue: i64,
    pax_high: i32,
    pax_medium: i32,
    pax_basic: i32,
    pax_low: i32,
    pax_revenue: i64,
    periodic_ticks: u32,
    periodic_maintenance: i64,
    periodic_salary: i64,
    periodic_mortgage: i64,
    life_support_paid: u32,
    life_support_total: i64,
    incidents: Vec<IncidentSummary>,
    inbound_arrival: Option<InboundArrival<'a>>,
    budget_after_last: i64,
    closing_warning: Option<String>,
    aborted: bool,
    marooned_here: bool,
}

struct SoldGood {
    good: String,
    qty: i32,
    sell_price: i32,
    paid: i32,
    profit: i64,
}
struct HeldGood {
    good: String,
    qty: i32,
    would_sell_at: i32,
    paid: i32,
    reason: String,
}
struct BoughtGood {
    good: String,
    qty: i32,
    unit_cost: i32,
    total_cost: i64,
}

struct InboundArrival<'a> {
    from: &'a WorldRef,
    distance: i32,
    fuel_cost: i64,
}

enum IncidentSummary {
    Piracy {
        cargo_lost_tons: i32,
        cargo_breakdown: Vec<(String, i32)>,
        buy_cost_sunk: i64,
        credits_lost: i64,
        weeks_lost: u32,
        weapons: i16,
        avoidance_total: i32,
        table_total: i32,
    },
    TradeScam {
        credits_lost: i64,
        weeks_lost: u32,
    },
    CrewLoss {
        weeks_lost: u32,
    },
    Accident {
        repair_cost: i64,
    },
    Government {
        fine_credits: i64,
        weeks_lost: u32,
    },
}

fn world_eq(a: &WorldRef, b: &WorldRef) -> bool {
    a.sector == b.sector && a.hex_x == b.hex_x && a.hex_y == b.hex_y
}

/// Walk steps and coalesce consecutive same-world steps into [`Visit`]s.
///
/// Identity is full hex (sector + coords) — name alone is not enough,
/// since the same name can appear in different sectors and we want
/// revisits to be treated as separate visits anyway.
fn coalesce_visits(steps: &[SimulationStep]) -> Vec<Visit<'_>> {
    let mut visits: Vec<Visit<'_>> = Vec::new();
    let mut cur: Option<Visit<'_>> = None;

    for step in steps {
        // A `Jump` step closes the current visit. Its location is the
        // departure world (the executor records the jump from the
        // origin's perspective).
        let is_jump = matches!(step.action, Action::Jump { .. });

        let need_new = match (&cur, is_jump) {
            (None, _) => true,
            (Some(v), false) => !world_eq(&v.world, &step.location),
            (Some(_), true) => false,
        };
        if need_new {
            if let Some(v) = cur.take() {
                visits.push(v);
            }
            cur = Some(Visit {
                world: step.location.clone(),
                arrived: step.date,
                departed: step.date,
                sells: Vec::new(),
                holds: Vec::new(),
                buys: Vec::new(),
                freight_tons: 0,
                freight_lots: 0,
                freight_revenue: 0,
                pax_high: 0,
                pax_medium: 0,
                pax_basic: 0,
                pax_low: 0,
                pax_revenue: 0,
                periodic_ticks: 0,
                periodic_maintenance: 0,
                periodic_salary: 0,
                periodic_mortgage: 0,
                life_support_paid: 0,
                life_support_total: 0,
                incidents: Vec::new(),
                inbound_arrival: None,
                budget_after_last: step.budget_after,
                closing_warning: None,
                aborted: false,
                marooned_here: false,
            });
        }

        let v = cur.as_mut().expect("cur is Some");
        v.departed = step.date;
        v.budget_after_last = step.budget_after;

        match &step.action {
            Action::Arrive {
                from,
                distance,
                fuel_cost,
            } => {
                v.inbound_arrival = Some(InboundArrival {
                    from,
                    distance: *distance,
                    fuel_cost: *fuel_cost,
                });
            }
            Action::SellGood {
                good,
                qty,
                sell_price,
                paid,
                profit,
            } => v.sells.push(SoldGood {
                good: good.clone(),
                qty: *qty,
                sell_price: *sell_price,
                paid: *paid,
                profit: *profit,
            }),
            Action::HoldGood {
                good,
                qty,
                would_sell_at,
                paid,
                reason,
            } => v.holds.push(HeldGood {
                good: good.clone(),
                qty: *qty,
                would_sell_at: *would_sell_at,
                paid: *paid,
                reason: reason.clone(),
            }),
            Action::BuyGood {
                good,
                qty,
                unit_cost,
                total_cost,
            } => v.buys.push(BoughtGood {
                good: good.clone(),
                qty: *qty,
                unit_cost: *unit_cost,
                total_cost: *total_cost,
            }),
            Action::LoadFreight {
                tons,
                lots,
                revenue_pending,
            } => {
                v.freight_tons += tons;
                v.freight_lots += lots;
                v.freight_revenue += revenue_pending;
            }
            Action::BoardPax {
                high,
                medium,
                basic,
                low,
                revenue_pending,
            } => {
                v.pax_high += high;
                v.pax_medium += medium;
                v.pax_basic += basic;
                v.pax_low += low;
                v.pax_revenue += revenue_pending;
            }
            Action::PayLifeSupport {
                stateroom_cost,
                ls_cost,
                low_cost,
                crew_cost,
            } => {
                v.life_support_paid += 1;
                v.life_support_total += stateroom_cost + ls_cost + low_cost + crew_cost;
            }
            Action::PayPeriodic {
                maintenance,
                salary,
                mortgage,
                ..
            } => {
                v.periodic_ticks += 1;
                v.periodic_maintenance += maintenance;
                v.periodic_salary += salary;
                v.periodic_mortgage += mortgage;
            }
            Action::Jump { .. } => {
                let done = cur.take().expect("cur was Some");
                visits.push(done);
                continue;
            }
            Action::IncidentAvoided { .. } => {
                // Skip — pure analytics.
            }
            Action::IncidentPiracy {
                cargo_lost_tons,
                cargo_lost_breakdown,
                buy_cost_sunk,
                credits_lost,
                weeks_lost,
                weapons,
                avoidance_total,
                table_total,
                ..
            } => v.incidents.push(IncidentSummary::Piracy {
                cargo_lost_tons: *cargo_lost_tons,
                cargo_breakdown: cargo_lost_breakdown.clone(),
                buy_cost_sunk: *buy_cost_sunk,
                credits_lost: *credits_lost,
                weeks_lost: *weeks_lost,
                weapons: *weapons,
                avoidance_total: *avoidance_total,
                table_total: *table_total,
            }),
            Action::IncidentTradeScam {
                credits_lost,
                weeks_lost,
                ..
            } => v.incidents.push(IncidentSummary::TradeScam {
                credits_lost: *credits_lost,
                weeks_lost: *weeks_lost,
            }),
            Action::IncidentCrewLoss { weeks_lost, .. } => {
                v.incidents.push(IncidentSummary::CrewLoss {
                    weeks_lost: *weeks_lost,
                });
            }
            Action::IncidentAccident { repair_cost, .. } => {
                v.incidents.push(IncidentSummary::Accident {
                    repair_cost: *repair_cost,
                });
            }
            Action::IncidentGovernment {
                fine_credits,
                weeks_lost,
                ..
            } => v.incidents.push(IncidentSummary::Government {
                fine_credits: *fine_credits,
                weeks_lost: *weeks_lost,
            }),
            Action::BudgetWarning { note } => {
                v.closing_warning = Some(note.clone());
            }
            Action::NoCandidate { note } => {
                v.closing_warning = Some(format!("No reachable destination: {note}"));
                v.aborted = true;
            }
            Action::AbortOverflow { days_past_target } => {
                v.closing_warning = Some(format!("Aborted — {days_past_target} days past target."));
                v.aborted = true;
            }
            Action::Marooned { .. } => {
                v.marooned_here = true;
            }
        }
    }

    if let Some(v) = cur.take() {
        visits.push(v);
    }
    visits
}

// ---------------------------------------------------------------------
// Per-visit rendering
// ---------------------------------------------------------------------

fn write_visit(out: &mut String, label: &str, v: &Visit<'_>) {
    let _ = writeln!(out, "\n{label}");

    let _ = writeln!(
        out,
        "World: {} ({}, hex {:02}{:02}), UWP {}, {} zone",
        v.world.name,
        v.world.sector,
        v.world.hex_x,
        v.world.hex_y,
        v.world.uwp,
        zone_label(v.world.zone),
    );
    let tcs = trade_classes_for(&v.world.uwp);
    if !tcs.is_empty() {
        let _ = writeln!(out, "Trade classes: {}", tcs.join(", "));
    }
    let _ = writeln!(out, "Arrived: {}", v.arrived.format());
    let _ = writeln!(out, "Departed: {}", v.departed.format());
    let _ = writeln!(
        out,
        "Days at port: {}",
        v.arrived.days_until(v.departed).max(0)
    );

    if let Some(arr) = &v.inbound_arrival {
        let _ = writeln!(
            out,
            "Inbound jump: from {} ({} pc, fuel {} Cr)",
            arr.from.name, arr.distance, arr.fuel_cost
        );
    }

    // Sells: cap at top 6 by absolute profit so the model can pick a
    // winner / loser to highlight; rollup line keeps the books honest.
    if !v.sells.is_empty() {
        out.push_str("Sold (from previous leg):\n");
        let mut sorted: Vec<&SoldGood> = v.sells.iter().collect();
        sorted.sort_by_key(|s| -s.profit.abs());
        for s in sorted.iter().take(6) {
            let _ = writeln!(
                out,
                "  - {}t {} @ {}/t (paid {}/t) = {}{} Cr",
                s.qty,
                s.good,
                s.sell_price,
                s.paid,
                if s.profit >= 0 { "+" } else { "" },
                s.profit
            );
        }
        if v.sells.len() > 6 {
            let total: i64 = v.sells.iter().map(|s| s.profit).sum();
            let _ = writeln!(
                out,
                "  - (... and {} more lots; total profit on this leg = {}{} Cr)",
                v.sells.len() - 6,
                if total >= 0 { "+" } else { "" },
                total
            );
        }
    }

    if !v.holds.is_empty() {
        out.push_str("Held (could not sell profitably):\n");
        for h in v.holds.iter().take(6) {
            let _ = writeln!(
                out,
                "  - {}t {} (would sell at {}/t, paid {}/t — {})",
                h.qty, h.good, h.would_sell_at, h.paid, h.reason
            );
        }
    }

    if !v.buys.is_empty() {
        out.push_str("Bought:\n");
        let mut sorted: Vec<&BoughtGood> = v.buys.iter().collect();
        sorted.sort_by_key(|b| -b.total_cost);
        for b in sorted.iter().take(6) {
            let _ = writeln!(
                out,
                "  - {}t {} @ {}/t = {} Cr",
                b.qty, b.good, b.unit_cost, b.total_cost
            );
        }
        if v.buys.len() > 6 {
            let _ = writeln!(out, "  - (... and {} more lots.)", v.buys.len() - 6);
        }
    }

    let _ = writeln!(
        out,
        "Loaded freight: {}t in {} lots, pending revenue {} Cr",
        v.freight_tons, v.freight_lots, v.freight_revenue
    );
    let _ = writeln!(
        out,
        "Boarded passengers: {}H/{}M/{}B/{}L, pending revenue {} Cr",
        v.pax_high, v.pax_medium, v.pax_basic, v.pax_low, v.pax_revenue
    );

    if v.periodic_ticks > 0 {
        let total = v.periodic_maintenance + v.periodic_salary + v.periodic_mortgage;
        let _ = writeln!(
            out,
            "Periodic costs paid: {} tick(s), total {} Cr (maintenance {} + salary {} + mortgage {})",
            v.periodic_ticks, total, v.periodic_maintenance, v.periodic_salary, v.periodic_mortgage
        );
    }
    if v.life_support_paid > 0 {
        let _ = writeln!(
            out,
            "Life support paid: {} jump(s), {} Cr total",
            v.life_support_paid, v.life_support_total
        );
    }

    for inc in &v.incidents {
        write_incident(out, inc, v.inbound_arrival.is_some());
    }

    if let Some(note) = &v.closing_warning {
        let _ = writeln!(out, "Note: {note}");
    }
    if v.aborted {
        out.push_str("Voyage aborted at this port.\n");
    }
    if v.marooned_here {
        out.push_str("MAROONED at this port — voyage ends here.\n");
    }

    let _ = writeln!(out, "Budget when leaving port: {} Cr", v.budget_after_last);
}

fn write_incident(out: &mut String, inc: &IncidentSummary, has_inbound_jump: bool) {
    match inc {
        IncidentSummary::Piracy {
            cargo_lost_tons,
            cargo_breakdown,
            buy_cost_sunk,
            credits_lost,
            weeks_lost,
            weapons,
            avoidance_total,
            table_total,
        } => {
            let when = if has_inbound_jump {
                "on inbound leg"
            } else {
                "in this system"
            };
            let _ = writeln!(out, "INCIDENT — Piracy {when}:");
            let breakdown = cargo_breakdown
                .iter()
                .map(|(g, t)| format!("{t}t {g}"))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                out,
                "  Cargo lost: {}t total{}",
                cargo_lost_tons,
                if breakdown.is_empty() {
                    String::new()
                } else {
                    format!(" ({breakdown})")
                }
            );
            let _ = writeln!(
                out,
                "  Sunk buy-cost: {buy_cost_sunk} Cr (cargo we paid for but lost)"
            );
            let _ = writeln!(out, "  Repair credits: {credits_lost} Cr");
            let _ = writeln!(out, "  Delay: {weeks_lost} weeks");
            let _ = writeln!(
                out,
                "  Our weapons rating: {weapons}; avoidance roll total {avoidance_total}, table result {table_total}"
            );
        }
        IncidentSummary::TradeScam {
            credits_lost,
            weeks_lost,
        } => {
            let _ = writeln!(
                out,
                "INCIDENT — Trade scam: lost {credits_lost} Cr, delayed {weeks_lost} weeks."
            );
        }
        IncidentSummary::CrewLoss { weeks_lost } => {
            let _ = writeln!(
                out,
                "INCIDENT — Crew loss: hiring/paperwork delay of {weeks_lost} weeks (no credit penalty)."
            );
        }
        IncidentSummary::Accident { repair_cost } => {
            let _ = writeln!(
                out,
                "INCIDENT — Mechanical accident: {repair_cost} Cr in repairs."
            );
        }
        IncidentSummary::Government {
            fine_credits,
            weeks_lost,
        } => {
            let _ = writeln!(
                out,
                "INCIDENT — Government complication: {fine_credits} Cr fine, delayed {weeks_lost} weeks."
            );
        }
    }
}

// ---------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------

fn zone_label(z: ZoneClassification) -> &'static str {
    match z {
        ZoneClassification::Green => "Green",
        ZoneClassification::Amber => "Amber",
        ZoneClassification::Red => "Red",
    }
}

/// Pretty trade-class names for the prompt. Skips the zone classes
/// (Amber/Red are already on the visit line above).
fn trade_classes_for(uwp: &str) -> Vec<&'static str> {
    let chars: Vec<char> = uwp.chars().collect();
    if chars.len() < 9 || chars[7] != '-' {
        return Vec::new();
    }
    // uwp_to_trade_classes wants 8 chars: starport + 6 stat digits +
    // tech-level. Skip the dash at index 7.
    let uwp: [char; 8] = [
        chars[0], chars[1], chars[2], chars[3], chars[4], chars[5], chars[6], chars[8],
    ];
    uwp_to_trade_classes(&uwp)
        .into_iter()
        .map(|tc| match tc {
            TradeClass::Agricultural => "Agricultural",
            TradeClass::Asteroid => "Asteroid",
            TradeClass::Barren => "Barren",
            TradeClass::Desert => "Desert",
            TradeClass::FluidOceans => "FluidOceans",
            TradeClass::Garden => "Garden",
            TradeClass::HighPopulation => "HighPopulation",
            TradeClass::HighTech => "HighTech",
            TradeClass::IceCapped => "IceCapped",
            TradeClass::Industrial => "Industrial",
            TradeClass::LowPopulation => "LowPopulation",
            TradeClass::LowTech => "LowTech",
            TradeClass::NonAgricultural => "NonAgricultural",
            TradeClass::NonIndustrial => "NonIndustrial",
            TradeClass::Poor => "Poor",
            TradeClass::Rich => "Rich",
            TradeClass::Vacuum => "Vacuum",
            TradeClass::WaterWorld => "WaterWorld",
            TradeClass::AmberZone => "",
            TradeClass::RedZone => "",
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::Ship;

    fn wr(name: &str, sector: &str, x: i32, y: i32, uwp: &str) -> WorldRef {
        WorldRef {
            name: name.to_string(),
            sector: sector.to_string(),
            hex_x: x,
            hex_y: y,
            uwp: uwp.to_string(),
            zone: ZoneClassification::Green,
        }
    }

    #[test]
    fn coalesces_same_world_runs_into_one_visit() {
        let regina = wr("Regina", "Spinward Marches", 19, 10, "A788899-A");
        let efate = wr("Efate", "Spinward Marches", 17, 5, "A646930-D");
        let steps = vec![
            SimulationStep {
                date: Date::new(91, 1108),
                location: regina.clone(),
                budget_after: 500_000,
                action: Action::BuyGood {
                    good: "Computers".to_string(),
                    qty: 40,
                    unit_cost: 9_000,
                    total_cost: 360_000,
                },
            },
            SimulationStep {
                date: Date::new(98, 1108),
                location: regina.clone(),
                budget_after: 113_000,
                action: Action::Jump {
                    to: efate.clone(),
                    distance: 2,
                    fuel_cost: 1_000,
                },
            },
            SimulationStep {
                date: Date::new(105, 1108),
                location: efate.clone(),
                budget_after: 113_000,
                action: Action::Arrive {
                    from: regina.clone(),
                    distance: 2,
                    fuel_cost: 1_000,
                },
            },
        ];
        let visits = coalesce_visits(&steps);
        assert_eq!(visits.len(), 2);
        assert!(world_eq(&visits[0].world, &regina));
        assert!(world_eq(&visits[1].world, &efate));
    }

    #[test]
    fn build_prompt_includes_named_ship_and_dates() {
        let regina = wr("Regina", "Spinward Marches", 19, 10, "A788899-A");
        let params = SimulationParams {
            ship: Ship {
                cargo_capacity: 80,
                crew_size: 4,
                jump_rating: 2,
                ..Default::default()
            },
            fuel_cost_per_parsec: 500,
            crew_profit_share: 0.10,
            starting_budget: 500_000,
            home_world: regina.clone(),
            start_date: Date::new(91, 1108),
            target_completion_date: Date::new(180, 1108),
            illegal_goods: false,
        };
        let result = SimulationResult {
            final_budget: 612_400,
            gross_profit: 112_400,
            crew_share: 11_240,
            owner_profit: 101_160,
            end_date: Date::new(267, 1108),
            jumps: 3,
            completed_normally: true,
            returned_home: true,
            went_negative: false,
            marooned: false,
            marooned_at: None,
            marooned_on: None,
            rescue_arrives_on: None,
        };
        let s = build_prompt("Free Trader Beowulf", &params, &[], &result);
        assert!(s.contains("Free Trader Beowulf"));
        assert!(s.contains("091-1108"));
        assert!(s.contains("267-1108"));
        assert!(s.contains("Now write the captain's log."));
    }

    #[test]
    fn build_prompt_falls_back_when_ship_name_blank() {
        let regina = wr("Regina", "Spinward Marches", 19, 10, "A788899-A");
        let params = SimulationParams {
            ship: Ship::default(),
            fuel_cost_per_parsec: 500,
            crew_profit_share: 0.10,
            starting_budget: 500_000,
            home_world: regina.clone(),
            start_date: Date::new(91, 1108),
            target_completion_date: Date::new(180, 1108),
            illegal_goods: false,
        };
        let result = SimulationResult {
            final_budget: 0,
            gross_profit: 0,
            crew_share: 0,
            owner_profit: 0,
            end_date: Date::new(91, 1108),
            jumps: 0,
            completed_normally: true,
            returned_home: true,
            went_negative: false,
            marooned: false,
            marooned_at: None,
            marooned_on: None,
            rescue_arrives_on: None,
        };
        let s = build_prompt("", &params, &[], &result);
        assert!(s.contains("(unregistered"));
    }
}
