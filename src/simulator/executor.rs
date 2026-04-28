//! Server-side simulation executor.
//!
//! Runs the per-jump loop that ties the route planner, market
//! generation, `process_trades`, and the periodic accounting together,
//! streaming `SimulationStep`s out as they happen.
//!
//! This file is async because [`WorldCache`] does network I/O. The
//! route planner itself (`route::pick_next`) is sync.

use crate::simulator::economy::{self, ABORT_OVERFLOW_DAYS, PERIOD_DAYS, TURN_DAYS};
use crate::simulator::route::{self, RouteContext};
use crate::simulator::types::{
    Action, SimulationParams, SimulationResult, SimulationStep, WorldRef,
};
use crate::simulator::world_fetch::{FetchError, WorldCache};
use crate::systems::world::World;
use crate::trade::available_goods::{AvailableGoodsTable, Good};
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;
use crate::trade::table::TradeTable;

/// Errors the executor can return before producing a [`SimulationResult`].
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    /// The home world's UWP failed to parse.
    #[error("invalid home world UWP: {0}")]
    InvalidHomeUwp(String),
    /// Network or parsing failure fetching candidate worlds.
    #[error("world fetch failed: {0}")]
    Fetch(#[from] FetchError),
    /// Internal logic error — should never fire in practice.
    #[error("simulation invariant violated: {0}")]
    Invariant(String),
}

/// Run a full simulation. Calls `on_step` for each step as it happens;
/// returns the final result. The `cache` is reused across the whole run
/// so candidate world fetches are de-duplicated.
pub async fn run_simulation(
    params: SimulationParams,
    cache: &mut WorldCache,
    mut on_step: impl FnMut(SimulationStep) + Send,
) -> Result<SimulationResult, ExecutorError> {
    // === setup ===========================================================
    let mut budget: i64 = params.starting_budget;
    let mut manifest = ShipManifest::default();
    let mut current_date = params.start_date;
    let mut days_since_payment: u32 = 0;
    let mut periods_paid: u32 = 0;
    let mut went_negative = false;
    let mut returned_home = false;
    let mut completed_normally = true;
    let mut history: Vec<WorldRef> = Vec::new();
    let mut jumps_taken: u32 = 0;

    let mut current_world =
        World::from_upp(&params.home_world.name, &params.home_world.uwp, false, true).map_err(
            |e| ExecutorError::InvalidHomeUwp(format!("{}: {}", params.home_world.uwp, e)),
        )?;
    current_world.gen_trade_classes();
    current_world.coordinates = Some((params.home_world.hex_x, params.home_world.hex_y));
    current_world.travel_zone = params.home_world.zone;
    let mut current_ref = params.home_world.clone();

    // Initial Arrive at the home world (distance 0).
    emit(
        &mut on_step,
        current_date,
        &current_ref,
        budget,
        Action::Arrive {
            from: current_ref.clone(),
            distance: 0,
            fuel_cost: 0,
        },
    );

    // === main loop =======================================================
    loop {
        // (1) Periodic costs.
        while days_since_payment >= PERIOD_DAYS {
            let maintenance = params.maintenance_per_period;
            let salary = params.crew_salary_per_period;
            budget -= maintenance + salary;
            emit(
                &mut on_step,
                current_date,
                &current_ref,
                budget,
                Action::PayPeriodic {
                    maintenance,
                    salary,
                    period_index: periods_paid,
                },
            );
            periods_paid += 1;
            if budget < 0 && !went_negative {
                went_negative = true;
                emit(
                    &mut on_step,
                    current_date,
                    &current_ref,
                    budget,
                    Action::BudgetWarning {
                        note: format!(
                            "Budget went negative ({}) after period {}.",
                            budget, periods_paid
                        ),
                    },
                );
            }
            days_since_payment -= PERIOD_DAYS;
        }

        // (2) Abort check: too far past target.
        let overflow_days = current_date.days_until(params.target_completion_date);
        if overflow_days < -ABORT_OVERFLOW_DAYS {
            completed_normally = false;
            emit(
                &mut on_step,
                current_date,
                &current_ref,
                budget,
                Action::AbortOverflow {
                    days_past_target: -overflow_days,
                },
            );
            break;
        }

        // (3) End-of-trip detection. We're home and have actually travelled.
        // Price and sell whatever's still in the hold (anything we bought on
        // the last leg expecting to sell at home), then break out of the
        // loop. Without this, profitable cargo bought for the home leg sits
        // unrealized and the trip P&L is skewed by hundreds of kCr.
        let at_home = jumps_taken > 0 && worldref_same_hex(&current_ref, &params.home_world);

        // (4) Generate this port's market and price to buy locally.
        let trade_table = TradeTable::global();
        let pop = current_world.get_population();
        let mut market = AvailableGoodsTable::for_world(
            trade_table,
            &current_world.get_trade_classes(),
            pop,
            params.illegal_goods,
        )
        .map_err(ExecutorError::Invariant)?;
        market.price_goods_to_buy(
            &current_world.get_trade_classes(),
            params.buyer_broker_skill,
            params.seller_broker_skill,
        );

        // (5) SELL phase: price what's already in the manifest at this
        // world; sell anything that beats its buy_cost, hold the rest.
        manifest.trade_goods.price_goods_to_sell(
            Some(current_world.get_trade_classes()),
            params.seller_broker_skill,
            params.buyer_broker_skill,
        );
        for good in manifest.trade_goods.goods.iter_mut() {
            if good.quantity <= 0 {
                continue;
            }
            let sell_price = good.sell_price.unwrap_or(0);
            if sell_price >= good.buy_cost {
                let qty = good.quantity;
                good.transacted = qty;
                // Apply the proceeds to the running budget *now* so each
                // Sell step reflects the cash inflow it represents. The
                // matching deduction was paid when these goods were bought
                // on a previous turn (and is recorded in `good.buy_cost`).
                let sell_proceeds = sell_price as i64 * qty as i64;
                budget += sell_proceeds;
                let profit = sell_proceeds - good.buy_cost as i64 * qty as i64;
                emit(
                    &mut on_step,
                    current_date,
                    &current_ref,
                    budget,
                    Action::SellGood {
                        good: good.name.clone(),
                        qty,
                        sell_price,
                        paid: good.buy_cost,
                        profit,
                    },
                );
            } else {
                good.transacted = 0;
                emit(
                    &mut on_step,
                    current_date,
                    &current_ref,
                    budget,
                    Action::HoldGood {
                        good: good.name.clone(),
                        qty: good.quantity,
                        would_sell_at: sell_price,
                        paid: good.buy_cost,
                        reason: if good.sell_price.is_none() {
                            "no sell price".to_string()
                        } else {
                            "below cost".to_string()
                        },
                    },
                );
            }
        }

        // (5b) End-of-trip settlement. If we're back home, realize the sale
        // revenue from the cargo we just priced and break. We pass empty
        // `buy_goods` and `None` passengers because the next-jump prep
        // (steps 7-10) is skipped when at home.
        if at_home {
            // Sale proceeds were already applied to `budget` per-good in
            // step (5); we just need to flush the manifest mutations
            // (clear sold cargo) without double-counting revenue.
            manifest.process_trades(0, &[], &None);
            returned_home = true;
            break;
        }

        // (6) ROUTE phase: gather candidates, pick the next destination.
        let candidates = cache
            .candidates_within(
                &current_ref.sector,
                (current_ref.hex_x, current_ref.hex_y),
                params.jump,
            )
            .await?;
        if candidates.is_empty() {
            emit(
                &mut on_step,
                current_date,
                &current_ref,
                budget,
                Action::NoCandidate {
                    note: "No reachable worlds within jump range.".to_string(),
                },
            );
            completed_normally = false;
            break;
        }

        let ctx = RouteContext {
            home: &params.home_world,
            current_date,
            start_date: params.start_date,
            target_date: params.target_completion_date,
            jump: params.jump,
            fuel_cost_per_parsec: params.fuel_cost_per_parsec,
            history: &history,
        };
        let next = match route::pick_next(&candidates, &market, &ctx) {
            Some(n) => n,
            None => {
                emit(
                    &mut on_step,
                    current_date,
                    &current_ref,
                    budget,
                    Action::NoCandidate {
                        note: "Route planner returned no destination.".to_string(),
                    },
                );
                completed_normally = false;
                break;
            }
        };
        let next_ref = world_ref_for(&next.world, &current_ref.sector);

        // (7) BUY phase: re-price the market for the chosen destination
        // and pick the most-profit-per-ton lots that fit in budget+hold.
        let next_classes = next.world.get_trade_classes();
        market.price_goods_to_sell(
            Some(next_classes.clone()),
            params.seller_broker_skill,
            params.buyer_broker_skill,
        );
        let fuel_for_jump = (next.distance as i64) * params.fuel_cost_per_parsec;
        // Keep some headroom for upcoming life-support costs. The exact
        // pax mix is unknown until step (8); reserve a generous slack
        // (worst-case staterooms + LS for a fully-booked ship).
        let pax_reserve = pax_reserve_estimate(&params);
        let buy_budget = (budget - fuel_for_jump - pax_reserve).max(0);
        let buy_goods = pick_to_buy(&market, params.cargo_capacity, buy_budget);
        for g in &buy_goods {
            // Apply purchase cost to budget *now* so each Buy step reflects
            // the cash outflow.
            let total_cost = g.transacted as i64 * g.buy_cost as i64;
            budget -= total_cost;
            emit(
                &mut on_step,
                current_date,
                &current_ref,
                budget,
                Action::BuyGood {
                    good: g.name.clone(),
                    qty: g.transacted,
                    unit_cost: g.buy_cost,
                    total_cost,
                },
            );
        }

        // (8) FREIGHT + PAX.
        let mut available_pax = AvailablePassengers::default();
        available_pax.generate(
            current_world.get_population(),
            current_world.port,
            current_world.travel_zone,
            current_world.tech_level,
            next.world.get_population(),
            next.world.port,
            next.world.travel_zone,
            next.world.tech_level,
            next.distance,
            params.steward_skill as i32,
            params.buyer_broker_skill as i32,
        );

        let total_buy_tons: i32 = buy_goods.iter().map(|g| g.transacted).sum();
        let cargo_remaining = (params.cargo_capacity - total_buy_tons).max(0);
        let (chosen_lots, freight_tons) =
            pick_freight(&available_pax.freight_lots, cargo_remaining);
        manifest.freight_lot_indices = chosen_lots;
        let (h, m, b, l) = pick_passengers(params.staterooms, params.low_berths, &available_pax);
        manifest.high_passengers = h;
        manifest.medium_passengers = m;
        manifest.basic_passengers = b;
        manifest.low_passengers = l;
        let pax_revenue_pending = manifest.passenger_revenue(next.distance) as i64;
        let freight_revenue_pending =
            manifest.freight_revenue(next.distance, &available_pax) as i64;
        if !manifest.freight_lot_indices.is_empty() {
            emit(
                &mut on_step,
                current_date,
                &current_ref,
                budget,
                Action::LoadFreight {
                    tons: freight_tons,
                    lots: manifest.freight_lot_indices.len() as u32,
                    revenue_pending: freight_revenue_pending,
                },
            );
        }
        if h + m + b + l > 0 {
            emit(
                &mut on_step,
                current_date,
                &current_ref,
                budget,
                Action::BoardPax {
                    high: h,
                    medium: m,
                    basic: b,
                    low: l,
                    revenue_pending: pax_revenue_pending,
                },
            );
        }

        // (9) Pay life support.
        let (sr_cost, ls_cost, low_cost) = economy::passenger_costs(h, m, b, l);
        budget -= sr_cost + ls_cost + low_cost;
        emit(
            &mut on_step,
            current_date,
            &current_ref,
            budget,
            Action::PayLifeSupport {
                stateroom_cost: sr_cost,
                ls_cost,
                low_cost,
            },
        );
        if budget < 0 && !went_negative {
            went_negative = true;
            emit(
                &mut on_step,
                current_date,
                &current_ref,
                budget,
                Action::BudgetWarning {
                    note: format!("Budget went negative ({}) paying life support.", budget),
                },
            );
        }

        // (10) Pay fuel.
        budget -= fuel_for_jump;
        let from_ref = current_ref.clone();
        emit(
            &mut on_step,
            current_date,
            &current_ref,
            budget,
            Action::Jump {
                to: next_ref.clone(),
                distance: next.distance,
                fuel_cost: fuel_for_jump,
            },
        );

        // (11) process_trades — mutates the manifest (sells transacted
        // goods, adds buy_goods, clears pax/freight) and accumulates a
        // settlement delta on `manifest.profit`. We *don't* use that delta
        // for the budget; the goods part is already applied via the per-
        // action sell/buy updates above. What's still pending is the
        // pax + freight revenue, which only realizes when the ship arrives
        // at the destination.
        manifest.process_trades(next.distance, &buy_goods, &Some(available_pax));
        let pending_revenue = pax_revenue_pending + freight_revenue_pending;
        budget += pending_revenue;
        if budget < 0 && !went_negative {
            went_negative = true;
            emit(
                &mut on_step,
                current_date,
                &current_ref,
                budget,
                Action::BudgetWarning {
                    note: format!("Budget went negative ({}) after settling trades.", budget),
                },
            );
        }

        // (12) Advance state to the new world; bump time.
        history.insert(0, current_ref.clone());
        if history.len() > 8 {
            history.truncate(8);
        }
        current_world = next.world.clone();
        current_ref = next_ref.clone();
        current_date = current_date.add_days(TURN_DAYS);
        days_since_payment += TURN_DAYS;
        jumps_taken += 1;

        // (13) Emit the Arrive at the new world.
        emit(
            &mut on_step,
            current_date,
            &current_ref,
            budget,
            Action::Arrive {
                from: from_ref,
                distance: next.distance,
                fuel_cost: fuel_for_jump,
            },
        );
    }

    // === final tally =====================================================
    let gross = budget - params.starting_budget;
    let crew = if gross > 0 {
        (gross as f64 * params.crew_profit_share as f64).round() as i64
    } else {
        0
    };
    let owner = gross - crew;

    Ok(SimulationResult {
        final_budget: budget,
        gross_profit: gross,
        crew_share: crew,
        owner_profit: owner,
        end_date: current_date,
        jumps: jumps_taken,
        completed_normally,
        returned_home,
        went_negative,
    })
}

// ===== helpers ==========================================================

/// Build a [`SimulationStep`] from the current loop state and call the
/// callback. Inlined to avoid juggling closure types across awaits.
fn emit(
    on_step: &mut impl FnMut(SimulationStep),
    date: crate::simulator::types::Date,
    location: &WorldRef,
    budget_after: i64,
    action: Action,
) {
    on_step(SimulationStep {
        date,
        location: location.clone(),
        budget_after,
        action,
    });
}

/// Build a `WorldRef` for one of the executor's neighbouring worlds.
/// In v1 the simulator stays in-sector so we propagate the current
/// sector unchanged.
fn world_ref_for(world: &World, sector: &str) -> WorldRef {
    let (hex_x, hex_y) = world.coordinates.unwrap_or((0, 0));
    WorldRef {
        name: world.name.clone(),
        uwp: world.to_uwp(),
        sector: sector.to_string(),
        hex_x,
        hex_y,
        zone: world.travel_zone,
    }
}

/// Two `WorldRef`s point at the same world iff they share sector and
/// hex. We don't compare names or UWPs because either could differ
/// between the user's input and what TravellerMap returned.
fn worldref_same_hex(a: &WorldRef, b: &WorldRef) -> bool {
    a.sector == b.sector && a.hex_x == b.hex_x && a.hex_y == b.hex_y
}

/// Quick upper-bound estimate of life-support + stateroom costs we
/// might owe this turn, used to reserve budget headroom while picking
/// goods to buy. Conservative: assumes a fully-booked ship.
fn pax_reserve_estimate(params: &SimulationParams) -> i64 {
    // Worst case: every stateroom is high passage, every low berth is full.
    let high = params.staterooms.max(0);
    let low = params.low_berths.max(0);
    let (sr, ls, low_cost) = economy::passenger_costs(high, 0, 0, low);
    sr + ls + low_cost
}

/// Greedy buy planner. Eligible goods are those with a positive
/// `sell_price` and a profit per ton (`sell_price - buy_cost`). We sort
/// by profit-per-ton descending and consume the lots in order while we
/// have hold capacity and credits to spend.
///
/// The returned vector contains *clones* of the chosen market entries
/// with `transacted` set to the tons we want to buy and ready to be
/// passed to `ShipManifest::process_trades` as the buy list.
fn pick_to_buy(market: &AvailableGoodsTable, cargo_capacity: i32, buy_budget: i64) -> Vec<Good> {
    if cargo_capacity <= 0 || buy_budget <= 0 {
        return Vec::new();
    }
    // Score each good: profit per ton, only if both prices are sane.
    let mut scored: Vec<(f64, &Good)> = market
        .goods
        .iter()
        .filter_map(|g| {
            let sell = g.sell_price?;
            if sell <= g.buy_cost || g.quantity <= 0 || g.buy_cost <= 0 {
                return None;
            }
            let profit_per_ton = (sell - g.buy_cost) as f64;
            Some((profit_per_ton, g))
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut chosen: Vec<Good> = Vec::new();
    let mut remaining_cargo = cargo_capacity;
    let mut remaining_budget = buy_budget;
    for (_, g) in scored {
        if remaining_cargo <= 0 || remaining_budget <= 0 {
            break;
        }
        let by_cargo = remaining_cargo;
        let by_budget = (remaining_budget / g.buy_cost as i64) as i32;
        let take = g.quantity.min(by_cargo).min(by_budget.max(0));
        if take <= 0 {
            continue;
        }
        let mut clone = g.clone();
        clone.transacted = take;
        chosen.push(clone);
        remaining_cargo -= take;
        remaining_budget -= take as i64 * g.buy_cost as i64;
    }
    chosen
}

/// Pick freight lots largest-first into the available cargo. Returns
/// `(indices_into_freight_lots, total_tons)`.
fn pick_freight(
    freight_lots: &[crate::trade::available_passengers::FreightLot],
    cargo_remaining: i32,
) -> (Vec<usize>, i32) {
    if cargo_remaining <= 0 {
        return (Vec::new(), 0);
    }
    // Sort indices by lot size descending.
    let mut order: Vec<usize> = (0..freight_lots.len()).collect();
    order.sort_by(|&a, &b| freight_lots[b].size.cmp(&freight_lots[a].size));

    let mut chosen: Vec<usize> = Vec::new();
    let mut tons = 0;
    let mut left = cargo_remaining;
    for idx in order {
        let lot = &freight_lots[idx];
        if lot.size <= left {
            chosen.push(idx);
            tons += lot.size;
            left -= lot.size;
            if left <= 0 {
                break;
            }
        }
    }
    (chosen, tons)
}

/// Greedy passenger filler: high → medium → basic until staterooms run
/// out, then low until low berths run out.
fn pick_passengers(
    staterooms: i32,
    low_berths: i32,
    available: &AvailablePassengers,
) -> (i32, i32, i32, i32) {
    let mut rooms = staterooms.max(0);
    let high = available.high.max(0).min(rooms);
    rooms -= high;
    let medium = available.medium.max(0).min(rooms);
    rooms -= medium;
    // Basics share two-to-a-room: each remaining stateroom can take 2.
    let basic_capacity = rooms.saturating_mul(2);
    let basic = available.basic.max(0).min(basic_capacity);
    let low = available.low.max(0).min(low_berths.max(0));
    (high, medium, basic, low)
}

// ===== test smoke =======================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::ZoneClassification;

    fn dummy_home() -> WorldRef {
        WorldRef {
            name: "Home".to_string(),
            uwp: "A788899-A".to_string(),
            sector: "TestSector".to_string(),
            hex_x: 10,
            hex_y: 10,
            zone: ZoneClassification::Green,
        }
    }

    #[test]
    fn worldref_same_hex_compares_sector_and_hex() {
        let a = dummy_home();
        let mut b = a.clone();
        b.name = "Different".to_string();
        b.uwp = "X000000-0".to_string();
        assert!(worldref_same_hex(&a, &b));
        b.hex_x = 11;
        assert!(!worldref_same_hex(&a, &b));
    }

    #[test]
    fn pax_reserve_estimate_is_finite() {
        let params = SimulationParams {
            buyer_broker_skill: 0,
            seller_broker_skill: 0,
            steward_skill: 0,
            cargo_capacity: 100,
            staterooms: 6,
            low_berths: 4,
            jump: 2,
            maintenance_per_period: 0,
            crew_salary_per_period: 0,
            fuel_cost_per_parsec: 0,
            crew_profit_share: 0.0,
            starting_budget: 0,
            home_world: dummy_home(),
            start_date: crate::simulator::types::Date::new(0, 1105),
            target_completion_date: crate::simulator::types::Date::new(100, 1105),
            illegal_goods: false,
        };
        assert!(pax_reserve_estimate(&params) > 0);
    }

    #[test]
    fn pick_passengers_high_first_then_basic_pairs() {
        let avail = AvailablePassengers {
            high: 2,
            medium: 3,
            basic: 5,
            low: 4,
            ..Default::default()
        };
        // 4 staterooms, 2 low berths.
        let (h, m, b, l) = pick_passengers(4, 2, &avail);
        // 2 high uses 2 rooms; 2 medium uses 2 more rooms; basics get 0
        // because no rooms left.
        assert_eq!(h, 2);
        assert_eq!(m, 2);
        assert_eq!(b, 0);
        assert_eq!(l, 2);
    }

    #[test]
    fn pick_freight_largest_first() {
        use crate::trade::available_passengers::FreightLot;
        let lots = vec![
            FreightLot {
                size: 20,
                size_roll: 2,
            },
            FreightLot {
                size: 50,
                size_roll: 5,
            },
            FreightLot {
                size: 10,
                size_roll: 1,
            },
        ];
        let (chosen, tons) = pick_freight(&lots, 60);
        // Largest 50 then 10 → 60 tons; 20 doesn't fit.
        assert_eq!(tons, 60);
        assert_eq!(chosen.len(), 2);
        assert!(chosen.contains(&1));
        assert!(chosen.contains(&2));
    }

    /// End-to-end smoke test against the live TravellerMap API. Ignored
    /// by default because it requires network — run with
    /// `cargo test --features backend -- --ignored simulator_smoke`.
    #[tokio::test]
    #[ignore]
    async fn simulator_smoke_regina() {
        let _ = env_logger::Builder::from_default_env()
            .is_test(true)
            .try_init();
        let params = SimulationParams {
            buyer_broker_skill: 2,
            seller_broker_skill: 1,
            steward_skill: 1,
            cargo_capacity: 80,
            staterooms: 6,
            low_berths: 4,
            jump: 2,
            maintenance_per_period: 30_000,
            crew_salary_per_period: 12_000,
            fuel_cost_per_parsec: 5_000,
            crew_profit_share: 0.1,
            starting_budget: 1_000_000,
            home_world: WorldRef {
                name: "Regina".to_string(),
                uwp: "A788899-A".to_string(),
                sector: "Spinward Marches".to_string(),
                hex_x: 19,
                hex_y: 10,
                zone: ZoneClassification::Green,
            },
            start_date: crate::simulator::types::Date::new(1, 1105),
            target_completion_date: crate::simulator::types::Date::new(180, 1105),
            illegal_goods: false,
        };
        let mut cache = WorldCache::new();
        let mut step_count = 0;
        let result = run_simulation(params, &mut cache, |s| {
            step_count += 1;
            eprintln!(
                "[{}] {} budget={} {:?}",
                s.date.format(),
                s.location.name,
                s.budget_after,
                s.action
            );
        })
        .await
        .expect("simulation should complete");
        eprintln!("Steps: {}", step_count);
        eprintln!("Result: {:#?}", result);
        assert!(result.jumps > 0, "should have made at least one jump");
    }
}
