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
#[derive(Clone)]
pub struct Candidate {
    /// The candidate world (already populated, with trade classes). Its
    /// `coordinates` are the **sector-local** hex, matching the wire-format
    /// `WorldRef` the frontend renders.
    pub world: World,
    /// Canonical TravellerMap sector name this world belongs to — its *own*
    /// sector, which may differ from the current location's when the jump
    /// neighbourhood spills across a sector boundary.
    pub sector: String,
    /// Absolute hex coordinates: `(sx*32 + hex_x, sy*40 + hex_y)` where
    /// `(sx, sy)` are the sector's TravellerMap offsets. Globally unique and
    /// parity-preserving, so `calculate_hex_distance` works across sector
    /// boundaries. All planner distance math uses these.
    pub abs: (i32, i32),
    /// Distance in parsecs from the current location to this candidate.
    pub distance: i32,
    /// TravellerMap allegiance code (e.g. `"Im"`, `"ImAp"`, `"AsT4"`,
    /// `"Zh"`). `None` when the data wasn't available. Used by the route
    /// planner to apply a heavy penalty for foreign-empire space.
    pub allegiance: Option<String>,
    /// Number of gas giants in the system (TravellerMap `PBG` third digit).
    /// Used by the pirate planner's wilderness-refuel filter — a pirate
    /// avoids systems where it can't skim fuel. The merchant planner ignores
    /// this. Defaults to `0` for worlds fetched before this was plumbed.
    pub gas_giants: u8,
}

/// Read-only context for scoring.
///
/// `history` is interpreted as **most-recent first** — index `0` is the
/// last world we visited, index `1` is the one before that, etc.
pub struct RouteContext<'a> {
    /// Absolute hex of the world the trip makes for to *finish*: the
    /// destination when one is set, else the home world (a round trip).
    /// Drives the terminal bias, the forced-terminal override, the
    /// head-for-finish spiral, and the first-half exclusion (so the trip
    /// doesn't end early by arriving at the finish world). Once the ship has
    /// left, routing only ever cares about where it's headed — this hex —
    /// not where it started. Absolute (see [`Candidate::abs`]) so a terminal
    /// in another sector still exerts a correct pull.
    pub terminal_abs: (i32, i32),
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
    /// Absolute hex of the ship's current position. On a direct run this
    /// anchors the strict-progress filter: only candidates strictly closer
    /// to the terminal than we are now stay in the pool.
    pub current_abs: (i32, i32),
    /// Recently visited worlds, **most recent first**.
    pub history: &'a [WorldRef],
    /// When `true`, the terminal is a distinct destination (a one-way run),
    /// not the home world of a round trip. The planner then steers toward the
    /// terminal from the very first hop: no first-half exclusion (arriving
    /// early is fine), a **strict-progress filter** (only candidates strictly
    /// closer to the terminal than the current position stay in the pool, so
    /// the ship can never move backward — distance monotonically decreases
    /// and arrival is guaranteed; falls back to the full pool only when boxed
    /// in), and a directional pull (`ROUTE_W_TERMINAL_DIRECT`) that favours
    /// faster progress among the forward options. `false` → the classic
    /// round trip: explore first, then lean home after the halfway point via
    /// `ROUTE_W_HOME_BIAS`.
    pub direct_run: bool,
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

/// Penalty for picking a candidate in foreign-empire space. Sized to
/// be a strong preference against — comparable in magnitude to a single
/// high-value cargo lot's trade-value contribution (~30M for a 30-ton
/// 400 kCr/ton good with a +3 sale DM) — but overcomable when the trade
/// looks genuinely outsized.
pub const ROUTE_W_FOREIGN_EMPIRE: f64 = 10_000_000.0;

/// Strength of the "head home" pressure in the second half of the trip.
/// Per parsec from home, scaled linearly by trip progress beyond 50%.
/// In practice the trade-value score dwarfs this; it's a gentle bias for
/// the 50–75% window. Past `HEAD_HOME_THRESHOLD` the planner switches to
/// a hard "minimize distance to home" mode (see `pick_next`).
pub const ROUTE_W_HOME_BIAS: f64 = 50_000.0;

/// Directional pull toward the terminal on a **direct run** (a one-way trip
/// to a distinct destination — see `RouteContext::direct_run`). Applied per
/// hex of remaining distance to the terminal, from the very first hop.
/// "Never move backward" is enforced structurally by the strict-progress
/// filter in `pick_next`, not by this weight — the pull's job is only to
/// favour *faster* progress among the forward options (a few hexes ≈ a
/// port-A bonus), while a genuinely outsized on-the-way trade can still
/// justify the slower forward stop. Round trips use `ROUTE_W_HOME_BIAS`.
pub const ROUTE_W_TERMINAL_DIRECT: f64 = 200_000.0;

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
            let sale_dm = find_max_dm(&entry.sale_dm, candidate_classes) as f64;
            let purchase_dm = find_max_dm(&entry.purchase_dm, candidate_classes) as f64;
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

    // 5) History penalty. Match by (sector, hex_x, hex_y), not name — the
    //    candidate's own sector against the history entry's, so the same
    //    local hex in a *different* sector doesn't false-match.
    //    Decays linearly with recency: most recent → full penalty,
    //    second most recent → half, third → third, etc.
    if let Some((hx, hy)) = candidate.world.coordinates
        && let Some(idx) = ctx
            .history
            .iter()
            .position(|w| w.sector == candidate.sector && w.hex_x == hx && w.hex_y == hy)
    {
        let recency = (idx as f64) + 1.0;
        score -= ROUTE_W_HISTORY / recency;
    }

    // 6) Terminal bias — pull toward the finish world (the destination when
    //    set, else home). On a direct run to a distinct destination we steer
    //    from the very first hop with a steady, stronger pull so every stop
    //    makes net progress; on a round trip we explore first and lean home
    //    only after the halfway point. Absolute coords, so the pull is
    //    correct even when the terminal lies in another sector.
    {
        let dist_term = calculate_hex_distance(
            candidate.abs.0,
            candidate.abs.1,
            ctx.terminal_abs.0,
            ctx.terminal_abs.1,
        ) as f64;
        if ctx.direct_run {
            score -= dist_term * ROUTE_W_TERMINAL_DIRECT;
        } else {
            let total = ctx.start_date.days_until(ctx.target_date) as f64;
            let elapsed = ctx.start_date.days_until(ctx.current_date) as f64;
            if total > 0.0 {
                let progress = elapsed / total;
                if progress > 0.5 {
                    // Linear ramp: 0 at progress=0.5, full at progress>=1.0.
                    let ramp = ((progress - 0.5) / 0.5).clamp(0.0, 1.0);
                    score -= dist_term * ROUTE_W_HOME_BIAS * ramp;
                }
            }
        }
    }

    // 7) Foreign-empire penalty. Imperial / Non-aligned / Client-state
    //    space is fine; everything else (Aslan Hierate clans, Zhodani
    //    Consulate, Solomani, Hivers, K'kree, etc.) gets a near-hard
    //    block. Worlds with no allegiance data are treated as friendly.
    if !is_allegiance_friendly(candidate.allegiance.as_deref()) {
        score -= ROUTE_W_FOREIGN_EMPIRE;
    }

    score
}

/// Whether a TravellerMap allegiance code represents space the simulator
/// is willing to route through.
///
/// Friendly prefixes:
/// - `Im*` — Third Imperium and all its sub-codes (`ImAp`, `ImDc`, etc.).
/// - `Na*` — Non-aligned (humans or other).
/// - `Cs*` — Client states (e.g. `CsIm`, `CsZh`).
/// - missing data — assumed friendly to avoid over-blocking.
///
/// Anything else (`As`, `Zh`, `So`, `Hv`, `Kk`, `Va*`, etc.) is foreign.
pub fn is_allegiance_friendly(allegiance: Option<&str>) -> bool {
    match allegiance {
        None => true,
        Some(code) => {
            let code = code.trim();
            if code.is_empty() {
                return true;
            }
            code.starts_with("Im") || code.starts_with("Na") || code.starts_with("Cs")
        }
    }
}

/// Pick the best destination from `candidates`. Returns `None` only if
/// the list is empty.
///
/// Forced-terminal override: if we're at or past the target date and any
/// candidate is the terminal world (matched by absolute hex), that
/// candidate wins immediately regardless of score.
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

    // On a round trip, exclude the terminal (home) from the candidate pool
    // while we're still in the first half — otherwise the planner heads home
    // immediately on the first or second hop (home is typically high-pop
    // A-port and scores very well), ending the trip early. On a direct run to
    // a distinct destination we skip this: arriving early is fine, and the
    // directional pull in `score_candidate` steers us there steadily.
    // Falls back to all candidates if the terminal is somehow the only option.
    let is_terminal = |c: &&Candidate| -> bool { c.abs == ctx.terminal_abs };
    let dist_to_terminal = |p: (i32, i32)| -> i32 {
        calculate_hex_distance(p.0, p.1, ctx.terminal_abs.0, ctx.terminal_abs.1)
    };
    let candidates_for_search: Vec<&Candidate> = if ctx.direct_run {
        // Strict-progress filter: on a direct run, only candidates strictly
        // closer to the terminal than the current position are eligible —
        // the ship never moves backward, no matter how rich a market behind
        // it looks, so remaining distance decreases every hop and arrival is
        // guaranteed. Falls back to the full pool only when boxed in (no
        // forward world within jump range), where a sidestep is the only way
        // out.
        let cur_d = dist_to_terminal(ctx.current_abs);
        let forward: Vec<&Candidate> = candidates
            .iter()
            .filter(|c| dist_to_terminal(c.abs) < cur_d)
            .collect();
        if forward.is_empty() {
            candidates.iter().collect()
        } else {
            forward
        }
    } else if progress < 0.5 {
        let filtered: Vec<&Candidate> = candidates.iter().filter(|c| !is_terminal(c)).collect();
        if filtered.is_empty() {
            candidates.iter().collect()
        } else {
            filtered
        }
    } else {
        candidates.iter().collect()
    };

    // Forced-terminal override. At or past target, if the finish world
    // (destination, or home when none is set) is reachable, take it
    // regardless of score. Matched on absolute hex, so it works across
    // sector boundaries.
    if progress >= 1.0
        && let Some(term) = candidates.iter().find(|c| c.abs == ctx.terminal_abs)
    {
        return Some(term);
    }

    // Head-for-finish mode. Past `HEAD_HOME_THRESHOLD` of trip progress, the
    // trade-value score (which can be in the tens of millions) drowns out
    // the terminal-bias penalty, so we override it entirely: pick the
    // candidate with the smallest hex distance to the terminal, breaking ties
    // by score.
    if progress >= HEAD_HOME_THRESHOLD {
        return candidates
            .iter()
            .map(|c| {
                let dh = calculate_hex_distance(
                    c.abs.0,
                    c.abs.1,
                    ctx.terminal_abs.0,
                    ctx.terminal_abs.1,
                );
                (c, dh)
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

    // Normal mode: pick highest score among candidates excluding the terminal
    // (in the first half of the trip) or all candidates (in the second half).
    candidates_for_search.into_iter().max_by(|a, b| {
        let sa = score_candidate(a, market, ctx);
        let sb = score_candidate(b, market, ctx);
        sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
    })
}

// ---- helpers --------------------------------------------------------------

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
    /// derive trade classes from the UWP. We use `from_uwp` for the
    /// stat fields and then walk the UWP through `uwp_to_trade_classes`
    /// to populate the trade classes the planner reads.
    fn mk_world(name: &str, uwp: &str, x: i32, y: i32) -> World {
        let mut w = World::from_uwp(name, uwp, false, true).expect("from_uwp");
        w.coordinates = Some((x, y));
        // Derive trade classes from the UWP's 8-character body
        // ("A788899-A" → "A788899A").
        let body: String = uwp.chars().filter(|c| *c != '-').collect();
        let chars: Vec<char> = body.chars().collect();
        let classes = crate::trade::uwp_to_trade_classes(&chars);
        // Push them onto the world via gen_trade_classes' equivalent —
        // but gen_trade_classes generates from population/atmosphere.
        // We can't directly set trade_classes (private), so for tests
        // that need them populated we rely on `gen_trade_classes`
        // matching what `uwp_to_trade_classes` would produce.
        // The two routines use the same rules, so this works.
        let _ = classes; // explicit: gen_trade_classes derives the same
        w.gen_trade_classes();
        w
    }

    /// Test candidate: single unnamed sector, so `abs` == the local hex.
    fn cand(name: &str, uwp: &str, x: i32, y: i32, distance: i32) -> Candidate {
        Candidate {
            world: mk_world(name, uwp, x, y),
            sector: String::new(),
            abs: (x, y),
            distance,
            allegiance: None,
            gas_giants: 0,
        }
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

    fn ctx(terminal_abs: (i32, i32), history: &[WorldRef]) -> RouteContext<'_> {
        RouteContext {
            terminal_abs,
            current_date: Date::new(0, 1105),
            start_date: Date::new(0, 1105),
            target_date: Date::new(100, 1105),
            jump: 2,
            fuel_cost_per_parsec: 10_000,
            // Far from every test terminal by default, so the direct-run
            // strict-progress filter keeps all candidates unless a test
            // positions the ship deliberately.
            current_abs: (100, 100),
            history,
            direct_run: false,
        }
    }

    #[test]
    fn empty_candidates_returns_none() {
        let market = AvailableGoodsTable::default();
        let c = ctx((0, 0), &[]);
        assert!(pick_next(&[], &market, &c).is_none());
    }

    #[test]
    fn forced_home_when_past_target() {
        // Two candidates: a wonderful non-home world and home itself.
        // Past target date, home should win regardless of score.
        let great = cand("Great", "A999999-F", 1, 1, 1);
        let home = cand("Home", "A788899-A", 5, 5, 4);

        let market = AvailableGoodsTable::default();
        let mut c = ctx((5, 5), &[]);
        c.current_date = Date::new(150, 1105); // past target (100)

        let cands = [great, home];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Home");
    }

    #[test]
    fn closer_preferred_all_else_equal() {
        // Two identical worlds at different distances → closer wins.
        let near = cand("Near", "C555555-7", 1, 0, 1);
        let far = cand("Far", "C555555-7", 3, 0, 3);
        let market = AvailableGoodsTable::default();
        let c = ctx((0, 0), &[]);

        let cands = [near, far];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Near");
    }

    #[test]
    fn higher_port_wins_all_else_equal() {
        // Same UWP-body except port: A vs E, identical distance.
        let porta = cand("PortA", "A555555-7", 1, 0, 1);
        let porte = cand("PortE", "E555555-7", 0, 1, 1);
        let market = AvailableGoodsTable::default();
        let c = ctx((0, 0), &[]);

        let cands = [porta, porte];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "PortA");
    }

    #[test]
    fn history_penalty_applies() {
        let visited_ref = mk_world_ref("Visited", "C555555-7", 1, 0);

        // Same shape candidates; only difference is whether history has it.
        let visited_cand = cand("Visited", "C555555-7", 1, 0, 1);
        let fresh_cand = cand("Fresh", "C555555-7", 0, 1, 1);

        let market = AvailableGoodsTable::default();
        let history = vec![visited_ref];
        let c = ctx((0, 0), &history);

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
    fn history_requires_matching_sector() {
        // Same local hex, DIFFERENT sector → not a revisit. Cross-sector
        // routes must not false-match history entries by hex alone.
        let mut visited_ref = mk_world_ref("Visited", "C555555-7", 1, 0);
        visited_ref.sector = "Spinward Marches".to_string();

        // The candidate is at local hex (1,0) too, but in another sector.
        let mut lookalike = cand("Lookalike", "C555555-7", 1, 0, 1);
        lookalike.sector = "Trojan Reach".to_string();

        let market = AvailableGoodsTable::default();
        let history = vec![visited_ref];
        let c = ctx((0, 0), &history);

        // A genuinely fresh world in a third position for comparison.
        let fresh = cand("Fresh", "C555555-7", 0, 1, 1);
        let lookalike_score = score_candidate(&lookalike, &market, &c);
        let fresh_score = score_candidate(&fresh, &market, &c);
        // Neither carries a history penalty; the tiny difference left is
        // the terminal-distance bias, which is zero this early in the trip.
        assert!(
            (lookalike_score - fresh_score).abs() < 1.0,
            "sector-mismatched candidate must not be history-penalized; got {lookalike_score} vs {fresh_score}"
        );
    }

    #[test]
    fn home_bias_kicks_in_after_halfway() {
        // Home at (0,0). One candidate near home, one far. Far has a
        // small port advantage that's enough to win when the bias is
        // off, but should lose once we're past 50% of the trip.
        let market = AvailableGoodsTable::default();

        // Early in trip: FarA's port-A bonus should beat Near.
        let mut c_early = ctx((0, 0), &[]);
        c_early.current_date = Date::new(0, 1105); // progress = 0
        let cands_early = [
            cand("Near", "C555555-7", 1, 0, 1),
            cand("FarA", "A555555-7", 8, 0, 1),
        ];
        let early = pick_next(&cands_early, &market, &c_early).unwrap();
        assert_eq!(
            early.world.name, "FarA",
            "Early in trip, port-A world should win"
        );

        // Late in trip: home bias on FarA (far from home) should
        // outweigh its port-A bonus.
        let mut c_late = ctx((0, 0), &[]);
        c_late.current_date = Date::new(95, 1105); // progress ~ 0.95
        let cands_late = [
            cand("Near", "C555555-7", 1, 0, 1),
            cand("FarA", "A555555-7", 8, 0, 1),
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
        let non_ag = cand("NonAg", "A302666-7", 1, 0, 1);
        assert!(
            non_ag
                .world
                .get_trade_classes()
                .contains(&TradeClass::NonAgricultural),
            "fixture should be Non-Agricultural; got {:?}",
            non_ag.world.get_trade_classes()
        );

        // Neutral counterpart: same population, same port, but not Non-Ag.
        // A666666-7 has atm 6, hydro 6, pop 6 → Agricultural+Rich, not Non-Ag.
        let neutral = cand("Neutral", "A666666-7", 0, 1, 1);
        assert!(
            !neutral
                .world
                .get_trade_classes()
                .contains(&TradeClass::NonAgricultural),
            "neutral fixture shouldn't be Non-Agricultural; got {:?}",
            neutral.world.get_trade_classes()
        );

        let c = ctx((0, 0), &[]);

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
        let home = cand("Home", "A999999-F", 5, 5, 1);
        let other = cand("Other", "C555555-7", 6, 5, 1);
        let market = AvailableGoodsTable::default();
        let mut c = ctx((5, 5), &[]);
        c.current_date = Date::new(10, 1105); // ~10% progress

        let cands = [home, other];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Other");
    }

    #[test]
    fn home_allowed_after_halfway() {
        // Same setup as above but past 50% progress — home becomes eligible
        // and (because A-port pop 9) should win on score.
        let home = cand("Home", "A999999-F", 5, 5, 1);
        let other = cand("Other", "C555555-7", 6, 5, 1);
        let market = AvailableGoodsTable::default();
        let mut c = ctx((5, 5), &[]);
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
        let great_far = cand("Great", "A999999-F", 5, 0, 2);
        let mediocre_near = cand("Near", "E555555-5", 1, 0, 2);
        let market = AvailableGoodsTable::default();
        let mut c = ctx((0, 0), &[]);
        c.current_date = Date::new(80, 1105); // 80% progress, past 0.75

        let cands = [great_far, mediocre_near];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Near");
    }

    #[test]
    fn head_for_finish_spirals_to_destination_not_home() {
        // With a destination set (terminal at (5,0), distinct from the (0,0)
        // start), past the head-home threshold the planner must spiral toward
        // the *destination*, not back toward the origin. A great trade world
        // sitting at the origin must lose to a mediocre one at the destination.
        let great_at_origin = cand("Great", "A999999-F", 0, 0, 2);
        let mediocre_at_dest = cand("AtDest", "E555555-5", 5, 0, 2);
        let market = AvailableGoodsTable::default();
        let mut c = ctx((5, 0), &[]); // terminal = destination (5,0)
        c.current_date = Date::new(80, 1105); // 80% progress, past 0.75

        let cands = [great_at_origin, mediocre_at_dest];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "AtDest");
    }

    #[test]
    fn cross_sector_terminal_pull_uses_absolute_coords() {
        // The terminal lies in the next sector coreward: local hexes would
        // put it "far away" (or at a phantom in-sector position), but the
        // absolute coords place it just past the boundary. Of two forward
        // options, the one closer in ABSOLUTE terms must win the
        // head-for-finish spiral, even though its *local* hex is not closer
        // to the terminal's local hex.
        //
        // Layout (absolute columns 10, rows around the sector seam at 0):
        //   terminal   abs (10, -2)  — two rows into the coreward sector
        //   nearer     abs (10,  1)  — local hex (10, 1), 3 rows from terminal
        //   farther    abs (10,  6)  — local hex (10, 6), 8 rows from terminal
        //
        // In *local-hex* space the terminal would sit at (10, 38) of its own
        // sector — nearer to local (10,6) than to (10,1) — so a local-hex
        // planner picks the wrong world. Absolute coords pick correctly.
        let mut nearer = cand("Nearer", "E555555-5", 10, 1, 2);
        nearer.abs = (10, 1);
        let mut farther = cand("Farther", "A999999-F", 10, 6, 2);
        farther.abs = (10, 6);

        let market = AvailableGoodsTable::default();
        let mut c = ctx((10, -2), &[]); // terminal beyond the sector seam
        c.current_date = Date::new(80, 1105); // head-for-finish mode

        let cands = [farther, nearer];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Nearer");
    }

    #[test]
    fn terminal_excluded_in_first_half_on_round_trip() {
        // On a round trip (no destination), the planner must not end early by
        // arriving home in the first half — even if home scores best. Home is
        // excluded from the first-half candidate pool.
        let home = cand("Home", "A999999-F", 5, 5, 1);
        let other = cand("Other", "C555555-7", 6, 5, 1);
        let market = AvailableGoodsTable::default();
        let mut c = ctx((5, 5), &[]); // direct_run = false (round trip)
        c.current_date = Date::new(10, 1105); // ~10% progress

        let cands = [home, other];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Other");
    }

    #[test]
    fn direct_run_does_not_exclude_terminal_early() {
        // On a direct run to a distinct destination, arriving early is fine:
        // the destination is NOT excluded in the first half, and when it's the
        // best-scoring option it wins (contrast the round-trip test above).
        let dest = cand("Dest", "A999999-F", 5, 5, 1);
        let other = cand("Other", "C555555-7", 6, 5, 1);
        let market = AvailableGoodsTable::default();
        let mut c = ctx((5, 5), &[]);
        c.current_date = Date::new(10, 1105); // ~10% progress
        c.direct_run = true;

        let cands = [dest, other];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Dest");
    }

    #[test]
    fn direct_run_steers_toward_the_destination_from_the_start() {
        // Early in the trip, a direct run steers toward the destination: of two
        // equal-jump forward options (neither the destination itself), the one
        // closer to the destination wins on the directional pull — even against
        // a port-A advantage on the farther one. A round trip at the same early
        // point has no such pull, so the port-A world wins there instead.
        let mk_cands = || {
            [
                // Farther from the destination (4 hexes) but a nicer port.
                cand("Away", "A555555-5", 2, 0, 1),
                // Closer to the destination (1 hex), plainer port.
                cand("Toward", "E555555-5", 5, 0, 1),
            ]
        };
        let market = AvailableGoodsTable::default();

        // Direct run, 10% in: the directional pull picks the closer world.
        let mut c_direct = ctx((6, 0), &[]);
        c_direct.current_date = Date::new(10, 1105);
        c_direct.direct_run = true;
        let direct_cands = mk_cands();
        let direct = pick_next(&direct_cands, &market, &c_direct).unwrap();
        assert_eq!(direct.world.name, "Toward");

        // Round trip, same early point: no directional pull yet → port-A wins.
        let mut c_round = ctx((6, 0), &[]);
        c_round.current_date = Date::new(10, 1105);
        let round_cands = mk_cands();
        let round = pick_next(&round_cands, &market, &c_round).unwrap();
        assert_eq!(round.world.name, "Away");
    }

    #[test]
    fn direct_run_never_moves_backward() {
        // The strict-progress filter: a fabulously rich market BEHIND the
        // ship must lose to a modest world ahead — no trade score can buy a
        // backward hop on a direct run.
        let rich_behind = cand("RichBehind", "A999999-F", 3, 0, 2);
        let modest_ahead = cand("ModestAhead", "E555555-5", 7, 0, 2);
        let market = AvailableGoodsTable::default();
        let mut c = ctx((10, 0), &[]); // terminal well ahead
        c.current_abs = (5, 0); // ship between the two candidates
        c.current_date = Date::new(10, 1105); // early in the trip
        c.direct_run = true;

        let cands = [rich_behind, modest_ahead];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "ModestAhead");
    }

    #[test]
    fn direct_run_boxed_in_falls_back_to_full_pool() {
        // If nothing within jump range makes progress (a dead end), the
        // planner must still return something — a sidestep beats a stall.
        let backward = cand("OnlyOption", "C555555-7", 3, 0, 2);
        let market = AvailableGoodsTable::default();
        let mut c = ctx((10, 0), &[]);
        c.current_abs = (5, 0); // the only candidate is behind us
        c.direct_run = true;

        let cands = [backward];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "OnlyOption");
    }

    #[test]
    fn allegiance_friendliness() {
        // Friendly: Imperial variants, Non-aligned, Client states, missing/empty.
        assert!(is_allegiance_friendly(None));
        assert!(is_allegiance_friendly(Some("")));
        assert!(is_allegiance_friendly(Some("   ")));
        assert!(is_allegiance_friendly(Some("Im")));
        assert!(is_allegiance_friendly(Some("ImAp")));
        assert!(is_allegiance_friendly(Some("ImDc")));
        assert!(is_allegiance_friendly(Some("Na")));
        assert!(is_allegiance_friendly(Some("NaHu")));
        assert!(is_allegiance_friendly(Some("NaXX")));
        assert!(is_allegiance_friendly(Some("CsIm")));
        assert!(is_allegiance_friendly(Some("CsZh")));

        // Foreign: Aslan clans, Zhodani, Solomani, Hivers, K'kree, Vargr.
        assert!(!is_allegiance_friendly(Some("As")));
        assert!(!is_allegiance_friendly(Some("AsT0")));
        assert!(!is_allegiance_friendly(Some("AsT4")));
        assert!(!is_allegiance_friendly(Some("AsXX")));
        assert!(!is_allegiance_friendly(Some("Zh")));
        assert!(!is_allegiance_friendly(Some("ZhCo")));
        assert!(!is_allegiance_friendly(Some("So")));
        assert!(!is_allegiance_friendly(Some("Hv")));
        assert!(!is_allegiance_friendly(Some("Kk")));
        assert!(!is_allegiance_friendly(Some("Va")));
    }

    #[test]
    fn foreign_empire_loses_to_friendly() {
        // A great-on-paper foreign world (A-port, high pop) should still
        // lose to a mediocre Imperial world thanks to the heavy penalty.
        let foreign_great = Candidate {
            allegiance: Some("AsT4".to_string()),
            ..cand("AslanA", "A999999-F", 1, 0, 1)
        };
        let imperial_meh = Candidate {
            allegiance: Some("Im".to_string()),
            ..cand("ImpC", "C555555-7", 2, 0, 2)
        };
        let market = AvailableGoodsTable::default();
        let c = ctx((0, 0), &[]);

        let cands = [foreign_great, imperial_meh];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(
            chosen.world.name, "ImpC",
            "Imperial world should beat foreign world even when stats favor foreign"
        );
    }

    #[test]
    fn foreign_empire_picked_as_last_resort() {
        // If foreign space is the *only* option, the planner still
        // returns it rather than `None` — the penalty is heavy but not
        // a hard block.
        let only_foreign = Candidate {
            allegiance: Some("Zh".to_string()),
            ..cand("Zhodane", "C555555-7", 1, 0, 1)
        };
        let market = AvailableGoodsTable::default();
        let c = ctx((0, 0), &[]);

        let cands = [only_foreign];
        let chosen = pick_next(&cands, &market, &c).unwrap();
        assert_eq!(chosen.world.name, "Zhodane");
    }
}
