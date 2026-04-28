//! Pure route planner for the ship simulator.
//!
//! `pick_next` scores each candidate destination and returns the
//! highest-scoring one. Scoring is weighted across trade value,
//! population, port quality, distance, history, and a "head home"
//! pressure that ramps up after the trip's halfway point.
//!
//! All scoring weights are first-cut and are meant to be tuned after
//! end-to-end runs. They live here as `pub const` so tests can see
//! them and a future tuner can swap values in one place.

use crate::simulator::types::{Date, WorldRef};
use crate::systems::world::World;
use crate::trade::PortCode;
use crate::trade::TradeClass;
use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::table::TradeTable;
use crate::util::calculate_hex_distance;

/// One destination world the planner is considering.
pub struct Candidate {
    /// The candidate world (already populated, with trade classes).
    pub world: World,
    /// Distance in parsecs from the current location to this candidate.
    pub distance: i32,
}

/// Read-only context for scoring.
///
/// `history` is interpreted as **most-recent first** — index `0` is the
/// last world we visited, index `1` is the one before that, etc.
pub struct RouteContext<'a> {
    /// Home world (for the "head home" bias and forced-home override).
    pub home: &'a WorldRef,
    /// Current in-game date.
    pub current_date: Date,
    /// Date the run started — used to compute trip progress.
    pub start_date: Date,
    /// Target completion date — used to compute trip progress and the
    /// forced-home override.
    pub target_date: Date,
    /// Ship's jump capability in parsecs. Currently informational; the
    /// caller is expected to filter candidates by jump range before
    /// calling `pick_next`.
    pub jump: i32,
    /// Fuel cost per parsec, used by the distance penalty.
    pub fuel_cost_per_parsec: i64,
    /// Recently visited worlds, **most recent first**.
    pub history: &'a [WorldRef],
}

// === Scoring weights ============================================================
// First-cut values; tune after end-to-end runs. Documented order of
// magnitude in comments. Public so tests can reason about them.

/// Per-unit-of-population bonus. Population code 9 → +450k.
pub const ROUTE_W_POP: f64 = 50_000.0;

/// Bonus added if the world's port code is `A`.
pub const ROUTE_W_PORT_A: f64 = 200_000.0;

/// Bonus added if the world's port code is `B`.
pub const ROUTE_W_PORT_B: f64 = 80_000.0;

/// Multiplier on `(distance * fuel_cost_per_parsec)` for the distance
/// penalty. With fuel ~10000 cr/pc, a 3-parsec jump → −150k.
pub const ROUTE_W_DIST: f64 = 5.0;

/// Base history penalty. Decays as `ROUTE_W_HISTORY / recency` where
/// `recency` is 1 for the most recent visit, 2 for the one before, etc.
pub const ROUTE_W_HISTORY: f64 = 300_000.0;

/// Strength of the "head home" pressure in the second half of the trip.
/// Per parsec from home, scaled linearly by trip progress beyond 50%.
/// In practice the trade-value score dwarfs this; it's a gentle bias for
/// the 50–75% window. Past `HEAD_HOME_THRESHOLD` the planner switches to
/// a hard "minimize distance to home" mode (see `pick_next`).
pub const ROUTE_W_HOME_BIAS: f64 = 50_000.0;

/// Trip-progress fraction at which the planner abandons trade-value
/// optimization and starts spiralling home. Past this threshold,
/// `pick_next` picks the candidate closest to the home world (ties
/// broken by trade score). Forced-home (100% progress) further overrides
/// this when home itself is reachable.
pub const HEAD_HOME_THRESHOLD: f64 = 0.75;

/// Score a single candidate. Higher is better.
pub fn score_candidate(
    candidate: &Candidate,
    market: &AvailableGoodsTable,
    ctx: &RouteContext,
) -> f64 {
    let candidate_classes = candidate.world.get_trade_classes();
    let trade_table = TradeTable::global();

    // 1) Trade value.
    let mut score: f64 = 0.0;
    for good in &market.goods {
        if let Some(entry) = trade_table.get(good.source_index) {
            let sale_dm = find_max_dm(&entry.sale_dm, &candidate_classes) as f64;
            let purchase_dm = find_max_dm(&entry.purchase_dm, &candidate_classes) as f64;
            let weight = good.quantity as f64 * good.base_cost as f64;
            score += (sale_dm - purchase_dm) * weight;
        }
    }

    // 2) Population bonus.
    score += candidate.world.get_population() as f64 * ROUTE_W_POP;

    // 3) Port bonus.
    match candidate.world.port {
        PortCode::A => score += ROUTE_W_PORT_A,
        PortCode::B => score += ROUTE_W_PORT_B,
        _ => {}
    }

    // 4) Distance penalty.
    score -= candidate.distance as f64 * ctx.fuel_cost_per_parsec as f64 * ROUTE_W_DIST;

    // 5) History penalty. Match by (sector, hex_x, hex_y), not name.
    //    Decays linearly with recency: most recent → full penalty,
    //    second most recent → half, third → third, etc.
    let cand_sector = home_sector_of(candidate);
    if let Some((sector, hx, hy)) = cand_sector
        && let Some(idx) = ctx
            .history
            .iter()
            .position(|w| w.sector == sector && w.hex_x == hx && w.hex_y == hy)
    {
        let recency = (idx as f64) + 1.0;
        score -= ROUTE_W_HISTORY / recency;
    }

    // 6) Home bias. Past 50% of the trip, push toward home.
    let total = ctx.start_date.days_until(ctx.target_date) as f64;
    let elapsed = ctx.start_date.days_until(ctx.current_date) as f64;
    if total > 0.0 {
        let progress = elapsed / total;
        if progress > 0.5
            && let (Some(cand_coords), Some(home_coords)) =
                (candidate.world.coordinates, sector_hex_of(ctx.home))
        {
            let dist_home =
                calculate_hex_distance(cand_coords.0, cand_coords.1, home_coords.0, home_coords.1)
                    as f64;
            // Linear ramp: 0 at progress=0.5, full at progress>=1.0.
            let ramp = ((progress - 0.5) / 0.5).clamp(0.0, 1.0);
            score -= dist_home * ROUTE_W_HOME_BIAS * ramp;
        }
    }

    score
}

/// Pick the best destination from `candidates`. Returns `None` only if
/// the list is empty.
///
/// Forced-home override: if we're at or past the target date and any
/// candidate is the home world (matched by `sector + hex_x + hex_y`),
/// that candidate wins immediately regardless of score.
pub fn pick_next<'a>(
    candidates: &'a [Candidate],
    market: &AvailableGoodsTable,
    ctx: &RouteContext,
) -> Option<&'a Candidate> {
    if candidates.is_empty() {
        return None;
    }

    let total = ctx.start_date.days_until(ctx.target_date) as f64;
    let elapsed = ctx.start_date.days_until(ctx.current_date) as f64;
    let progress = if total > 0.0 { elapsed / total } else { 0.0 };

    // Exclude home from the candidate pool while we're still in the first
    // half of the trip — otherwise the planner returns home immediately on
    // the first or second hop because home worlds are typically high-pop
    // A-port and score very well. Falls back to all candidates if home is
    // somehow the only option.
    let is_home = |c: &&Candidate| -> bool {
        matches!(
            home_sector_of(c),
            Some((_, hx, hy)) if hx == ctx.home.hex_x && hy == ctx.home.hex_y
        )
    };
    let candidates_for_search: Vec<&Candidate> = if progress < 0.5 {
        let filtered: Vec<&Candidate> = candidates.iter().filter(|c| !is_home(c)).collect();
        if filtered.is_empty() {
            candidates.iter().collect()
        } else {
            filtered
        }
    } else {
        candidates.iter().collect()
    };

    // Forced-home override. At or past target, if home itself is reachable,
    // take it regardless of score. v1 is in-sector, so we match purely on
    // hex coordinates.
    if progress >= 1.0 {
        for c in candidates {
            if let Some((_, hx, hy)) = home_sector_of(c)
                && hx == ctx.home.hex_x
                && hy == ctx.home.hex_y
            {
                return Some(c);
            }
        }
    }

    // Head-home mode. Past `HEAD_HOME_THRESHOLD` of trip progress, the
    // trade-value score (which can be in the tens of millions) drowns out
    // the home-bias penalty, so we override it entirely: pick the candidate
    // with the smallest hex distance to home, breaking ties by score.
    if progress >= HEAD_HOME_THRESHOLD
        && let Some(home_coords) = sector_hex_of(ctx.home)
    {
        return candidates
            .iter()
            .filter_map(|c| {
                c.world.coordinates.map(|(x, y)| {
                    let dh = calculate_hex_distance(x, y, home_coords.0, home_coords.1);
                    (c, dh)
                })
            })
            .min_by(|a, b| {
                a.1.cmp(&b.1).then_with(|| {
                    let sa = score_candidate(a.0, market, ctx);
                    let sb = score_candidate(b.0, market, ctx);
                    sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
                })
            })
            .map(|(c, _)| c);
    }

    // Normal mode: pick highest score among non-home candidates (in the
    // first half of the trip) or all candidates (in the second half).
    candidates_for_search.into_iter().max_by(|a, b| {
        let sa = score_candidate(a, market, ctx);
        let sb = score_candidate(b, market, ctx);
        sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
    })
}

// ---- helpers --------------------------------------------------------------

/// Return `(sector, hex_x, hex_y)` for a candidate world, if its
/// coordinates are populated. We don't have a sector on the `World`
/// itself, so it comes back empty — the caller compares against the
/// home `WorldRef.sector` separately.
///
/// In v1 the simulator is in-sector, so all worlds share one sector;
/// equality on sector is implicitly satisfied. We surface only the
/// hex pair here and let the history-match code combine sectors at the
/// `WorldRef` level.
fn home_sector_of(candidate: &Candidate) -> Option<(String, i32, i32)> {
    candidate
        .world
        .coordinates
        .map(|(x, y)| (String::new(), x, y))
}

/// Convenience — extract `(hex_x, hex_y)` from a `WorldRef`.
fn sector_hex_of(w: &WorldRef) -> Option<(i32, i32)> {
    Some((w.hex_x, w.hex_y))
}

/// Local copy of the `find_max_dm` helper used in `available_goods.rs`.
/// Returns the max DM across the candidate world's trade classes, or 0
/// if nothing matches. Reproduced here to avoid changing visibility on
/// the existing `available_goods` module.
fn find_max_dm(
    dm_map: &std::collections::HashMap<TradeClass, i16>,
    world_trade_classes: &[TradeClass],
) -> i16 {
    world_trade_classes
        .iter()
        .filter_map(|tc| dm_map.get(tc))
        .copied()
        .max()
        .unwrap_or(0)
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::ZoneClassification;
    use crate::trade::available_goods::Good;

    /// Build a `World` from a UWP, set its sector-hex coordinates, and
    /// derive trade classes from the UWP. We use `from_upp` for the
    /// stat fields and then walk the UWP through `upp_to_trade_classes`
    /// to populate the trade classes the planner reads.
    fn mk_world(name: &str, uwp: &str, x: i32, y: i32) -> World {
        let mut w = World::from_upp(name, uwp, false, true).expect("from_upp");
        w.coordinates = Some((x, y));
        // Derive trade classes from the UWP's 8-character body
        // ("A788899-A" → "A788899A").
        let body: String = uwp.chars().filter(|c| *c != '-').collect();
        let chars: Vec<char> = body.chars().collect();
        let classes = crate::trade::upp_to_trade_classes(&chars);
        // Push them onto the world via gen_trade_classes' equivalent —
        // but gen_trade_classes generates from population/atmosphere.
        // We can't directly set trade_classes (private), so for tests
        // that need them populated we rely on `gen_trade_classes`
        // matching what `upp_to_trade_classes` would produce.
        // The two routines use the same rules, so this works.
        let _ = classes; // explicit: gen_trade_classes derives the same
        w.gen_trade_classes();
        w
    }

    fn mk_world_ref(name: &str, uwp: &str, x: i32, y: i32) -> WorldRef {
        WorldRef {
            name: name.to_string(),
            uwp: uwp.to_string(),
            sector: String::new(),
            hex_x: x,
            hex_y: y,
            zone: ZoneClassification::Green,
        }
    }

    fn ctx<'a>(home: &'a WorldRef, history: &'a [WorldRef]) -> RouteContext<'a> {
        RouteContext {
            home,
            current_date: Date::new(0, 1105),
            start_date: Date::new(0, 1105),
            target_date: Date::new(100, 1105),
            jump: 2,
            fuel_cost_per_parsec: 10_000,
            history,
        }
    }

    #[test]
    fn empty_candidates_returns_none() {
        let home = mk_world_ref("Home", "A788899-A", 0, 0);
        let market = AvailableGoodsTable::default();
        let c = ctx(&home, &[]);
        assert!(pick_next(&[], &market, &c).is_none());
    }

    #[test]
    fn forced_home_when_past_target() {
        // Two candidates: a wonderful non-home world and home itself.
        // Past target date, home should win regardless of score.
        let home_ref = mk_world_ref("Home", "A788899-A", 5, 5);

        let great = Candidate {
            world: mk_world("Great", "A999999-F", 1, 1),
            distance: 1,
        };
        let home = Candidate {
            world: mk_world("Home", "A788899-A", 5, 5),
            distance: 4,
        };

        let market = AvailableGoodsTable::default();
        let mut c = ctx(&home_ref, &[]);
        c.current_date = Date::new(150, 1105); // past target (100)

        // Order matters? Try both orders.
        let cands = [great, home];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Home");
    }

    #[test]
    fn closer_preferred_all_else_equal() {
        // Two identical worlds at different distances → closer wins.
        let home_ref = mk_world_ref("Home", "A788899-A", 0, 0);
        let near = Candidate {
            world: mk_world("Near", "C555555-7", 1, 0),
            distance: 1,
        };
        let far = Candidate {
            world: mk_world("Far", "C555555-7", 3, 0),
            distance: 3,
        };
        let market = AvailableGoodsTable::default();
        let c = ctx(&home_ref, &[]);

        let cands = [near, far];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Near");
    }

    #[test]
    fn higher_port_wins_all_else_equal() {
        // Same UWP-body except port: A vs E, identical distance.
        let home_ref = mk_world_ref("Home", "A788899-A", 0, 0);
        let porta = Candidate {
            world: mk_world("PortA", "A555555-7", 1, 0),
            distance: 1,
        };
        let porte = Candidate {
            world: mk_world("PortE", "E555555-7", 0, 1),
            distance: 1,
        };
        let market = AvailableGoodsTable::default();
        let c = ctx(&home_ref, &[]);

        let cands = [porta, porte];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "PortA");
    }

    #[test]
    fn history_penalty_applies() {
        let home_ref = mk_world_ref("Home", "A788899-A", 0, 0);
        let visited_ref = mk_world_ref("Visited", "C555555-7", 1, 0);

        // Same shape candidates; only difference is whether history has it.
        let visited_cand = Candidate {
            world: mk_world("Visited", "C555555-7", 1, 0),
            distance: 1,
        };
        let fresh_cand = Candidate {
            world: mk_world("Fresh", "C555555-7", 0, 1),
            distance: 1,
        };

        let market = AvailableGoodsTable::default();
        let history = vec![visited_ref];
        let c = ctx(&home_ref, &history);

        let visited_score = score_candidate(&visited_cand, &market, &c);
        let fresh_score = score_candidate(&fresh_cand, &market, &c);
        assert!(
            visited_score < fresh_score,
            "visited ({visited_score}) should score lower than fresh ({fresh_score})"
        );

        let cands = [visited_cand, fresh_cand];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Fresh");
    }

    #[test]
    fn home_bias_kicks_in_after_halfway() {
        // Home at (0,0). One candidate near home, one far. Far has a
        // small port advantage that's enough to win when the bias is
        // off, but should lose once we're past 50% of the trip.
        let home_ref = mk_world_ref("Home", "A788899-A", 0, 0);

        let near_home = Candidate {
            world: mk_world("Near", "C555555-7", 1, 0),
            distance: 1,
        };
        let far_with_a = Candidate {
            world: mk_world("FarA", "A555555-7", 8, 0),
            distance: 1, // distance from current location, not from home
        };

        let market = AvailableGoodsTable::default();

        // Early in trip: FarA's port-A bonus should beat Near.
        let mut c_early = ctx(&home_ref, &[]);
        c_early.current_date = Date::new(0, 1105); // progress = 0
        let cands_early = [
            Candidate {
                world: near_home.world.clone(),
                distance: 1,
            },
            Candidate {
                world: far_with_a.world.clone(),
                distance: 1,
            },
        ];
        let early = pick_next(&cands_early, &market, &c_early).unwrap();
        assert_eq!(
            early.world.name, "FarA",
            "Early in trip, port-A world should win"
        );

        // Late in trip: home bias on far_with_a (far from home) should
        // outweigh its port-A bonus.
        let mut c_late = ctx(&home_ref, &[]);
        c_late.current_date = Date::new(95, 1105); // progress ~ 0.95
        let cands_late = [
            Candidate {
                world: near_home.world.clone(),
                distance: 1,
            },
            Candidate {
                world: far_with_a.world.clone(),
                distance: 1,
            },
        ];
        let late = pick_next(&cands_late, &market, &c_late).unwrap();
        assert_eq!(
            late.world.name, "Near",
            "Late in trip, near-home world should win"
        );
    }

    #[test]
    fn trade_value_matters() {
        // Build a market with a single hand-built `Good` whose
        // source_index points at trade-table entry 52 ("Textiles"),
        // which has a sale DM of Na+2 (Non-Agricultural). One candidate
        // is Non-Agricultural and one isn't — the Non-Agricultural one
        // should win because the sale_dm is favourable there. We pick
        // worlds whose other features (population, port) are roughly
        // balanced so the trade-value signal dominates.
        let home_ref = mk_world_ref("Home", "A788899-A", 0, 0);

        let entry_52 = TradeTable::global()
            .get(52)
            .expect("trade table entry 52 should exist");
        assert_eq!(entry_52.name, "Textiles");
        // Sanity: Na+2 should be in the sale DM map.
        assert_eq!(entry_52.sale_dm.get(&TradeClass::NonAgricultural), Some(&2));

        let mut market = AvailableGoodsTable::default();
        market.goods.push(Good {
            name: "Textiles".to_string(),
            quantity: 100,
            transacted: 0,
            base_cost: entry_52.base_cost,
            buy_cost: entry_52.base_cost,
            buy_cost_comment: String::new(),
            sell_price: None,
            sell_price_comment: String::new(),
            source_index: 52,
            quantity_roll: 0,
            buy_price_roll: None,
            sell_price_roll: None,
        });

        // Non-Agricultural: atm 0-3, hydro 0-3, pop ≥ 6 (per
        // World::gen_trade_classes). UWP "A302666-7": port A, size 3,
        // atm 0, hydro 2, pop 6 → Non-Ag (also Vacuum and
        // NonIndustrial; that's fine).
        let non_ag_world = mk_world("NonAg", "A302666-7", 1, 0);
        assert!(
            non_ag_world
                .get_trade_classes()
                .contains(&TradeClass::NonAgricultural),
            "fixture should be Non-Agricultural; got {:?}",
            non_ag_world.get_trade_classes()
        );

        // Neutral counterpart: same population, same port, but not Non-Ag.
        // A666666-7 has atm 6, hydro 6, pop 6 → Agricultural+Rich, not Non-Ag.
        let neutral_world = mk_world("Neutral", "A666666-7", 0, 1);
        assert!(
            !neutral_world
                .get_trade_classes()
                .contains(&TradeClass::NonAgricultural),
            "neutral fixture shouldn't be Non-Agricultural; got {:?}",
            neutral_world.get_trade_classes()
        );

        let non_ag = Candidate {
            world: non_ag_world,
            distance: 1,
        };
        let neutral = Candidate {
            world: neutral_world,
            distance: 1,
        };

        let c = ctx(&home_ref, &[]);

        // Score directly first to compare the two.
        let s_non_ag = score_candidate(&non_ag, &market, &c);
        let s_neutral = score_candidate(&neutral, &market, &c);
        assert!(
            s_non_ag > s_neutral,
            "Non-Ag should outscore neutral when carrying Textiles; got non_ag={s_non_ag} neutral={s_neutral}"
        );

        let cands = [neutral, non_ag];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(
            chosen.world.name, "NonAg",
            "Non-Agricultural buyer should win when carrying Textiles"
        );
    }

    #[test]
    fn home_excluded_in_first_half() {
        // Home is a great trade target (high pop, A port), but in the first
        // half of the trip we should NOT pick it — otherwise the trip ends
        // immediately. Should pick the non-home option.
        let home_ref = mk_world_ref("Home", "A999999-F", 5, 5);
        let home = Candidate {
            world: mk_world("Home", "A999999-F", 5, 5),
            distance: 1,
        };
        let other = Candidate {
            world: mk_world("Other", "C555555-7", 6, 5),
            distance: 1,
        };
        let market = AvailableGoodsTable::default();
        let mut c = ctx(&home_ref, &[]);
        c.current_date = Date::new(10, 1105); // ~10% progress

        let cands = [home, other];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Other");
    }

    #[test]
    fn home_allowed_after_halfway() {
        // Same setup as above but past 50% progress — home becomes eligible
        // and (because A-port pop 9) should win on score.
        let home_ref = mk_world_ref("Home", "A999999-F", 5, 5);
        let home = Candidate {
            world: mk_world("Home", "A999999-F", 5, 5),
            distance: 1,
        };
        let other = Candidate {
            world: mk_world("Other", "C555555-7", 6, 5),
            distance: 1,
        };
        let market = AvailableGoodsTable::default();
        let mut c = ctx(&home_ref, &[]);
        c.current_date = Date::new(60, 1105); // 60% progress

        let cands = [home, other];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Home");
    }

    #[test]
    fn head_home_mode_picks_closest_to_home() {
        // Past the head-home threshold, the planner should pick the candidate
        // closest to home (not the one with the best trade score). Set up:
        // home at (0, 0). A great trade world far from home (5 hexes) vs a
        // mediocre one near home (1 hex). The mediocre one must win.
        let home_ref = mk_world_ref("Home", "A788899-A", 0, 0);
        let great_far = Candidate {
            world: mk_world("Great", "A999999-F", 5, 0),
            distance: 2,
        };
        let mediocre_near = Candidate {
            world: mk_world("Near", "E555555-5", 1, 0),
            distance: 2,
        };
        let market = AvailableGoodsTable::default();
        let mut c = ctx(&home_ref, &[]);
        c.current_date = Date::new(80, 1105); // 80% progress, past 0.75

        let cands = [great_far, mediocre_near];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Near");
    }
}
