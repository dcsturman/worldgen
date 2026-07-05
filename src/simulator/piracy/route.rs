//! Pirate route selection: HUNT vs FENCE-RUN scoring over a refuelable
//! candidate pool.
//!
//! Both modes first **hard-filter** to wilderness-refuelable systems (a gas
//! giant in-system, or a mainworld with `hydro >= 1`) — a pirate avoids
//! systems where it can't skim fuel. HUNT mode then maximizes the expected
//! value of the system's Prey Encounter (more traffic and richer economies
//! are better; defenders, scaled up by the pirate's own reputation, are
//! worse) minus the fuel/distance cost. FENCE-RUN mode instead seeks the
//! nearest reachable **low-law** world to liquidate the hold.

use super::encounter::encounter_dms;
use super::fencing::law_bonus;
use super::strength::economy_multiplier;
use crate::simulator::route::{Candidate, HEAD_HOME_THRESHOLD, RouteContext};
use crate::util::calculate_hex_distance;

/// Which objective the planner optimizes this jump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteMode {
    /// Look for the best place to raid.
    Hunt,
    /// Divert to a low-law world to fence the hold.
    FenceRun,
}

/// Pirate-specific routing context, composed over the merchant
/// [`RouteContext`] (reused for home/dates/fuel/history).
pub struct PirateRouteContext<'a> {
    pub base: &'a RouteContext<'a>,
    pub reputation: f64,
    pub plundered_tons: i32,
    pub cargo_capacity: i32,
    pub budget: i64,
    pub upcoming_upkeep: i64,
    pub ship_weapons: i16,
    /// True when every prize crew is committed — head home to drop the prizes
    /// off (banking them and freeing the crews) before hunting on.
    pub prize_run: bool,
    /// True when reputation is too hot — break off raiding and run for the
    /// hideout to lie low until it cools.
    pub lying_low: bool,
    /// Hex of the world the cruise makes for to *finish* — the destination
    /// when one is set, else the hideout. Distinct from `base.home`, which
    /// stays the hideout for the lying-low / prize drop-off diversions.
    pub terminal_hex: (i32, i32),
}

/// Hold-fullness fraction at/above which the pirate breaks to fence.
pub const FENCE_HOLD_FRAC: f64 = 0.6;
/// Cash floor (as a multiple of upcoming upkeep) below which the pirate
/// breaks to fence to make payroll.
pub const FENCE_CASH_FLOOR_MULT: i64 = 2;
/// Hold-fullness at/above which the pirate won't start a new raid — only a
/// small amount of room left, not worth a fight, so it makes for a fence.
pub const RAID_HOLD_CEILING: f64 = 0.75;

/// Whether the hold is too full to bother attacking (see [`RAID_HOLD_CEILING`]).
pub fn hold_too_full_to_raid(plundered_tons: i32, cargo_capacity: i32) -> bool {
    cargo_capacity > 0 && plundered_tons as f64 >= cargo_capacity as f64 * RAID_HOLD_CEILING
}

/// Distance-penalty multiplier for pirate routing.
pub const ROUTE_P_DIST: f64 = 3.0;
/// Baseline expected prey value at a neutral system.
pub const HUNT_PREY_BASE: f64 = 400_000.0;
/// Baseline expected defender risk at a neutral system.
pub const HUNT_THREAT_BASE: f64 = 300_000.0;
/// How sharply each point of security DM (naval bases, patrolled empires)
/// multiplies the defender risk. High enough that a `+2` empire world repels
/// the pirate even when its economy makes the prey rich.
pub const HUNT_THREAT_PER_SECURITY: f64 = 0.7;

/// Whether the pirate can wilderness-refuel here.
pub fn is_refuelable(c: &Candidate) -> bool {
    c.gas_giants > 0 || c.world.hydro >= 1
}

/// Decide HUNT vs FENCE-RUN from hold fullness and cash pressure.
pub fn route_mode(ctx: &PirateRouteContext) -> RouteMode {
    let hold_frac = if ctx.cargo_capacity > 0 {
        ctx.plundered_tons as f64 / ctx.cargo_capacity as f64
    } else {
        0.0
    };
    let cash_low = ctx.budget < ctx.upcoming_upkeep.saturating_mul(FENCE_CASH_FLOOR_MULT);
    if hold_frac >= FENCE_HOLD_FRAC || cash_low {
        RouteMode::FenceRun
    } else {
        RouteMode::Hunt
    }
}

/// Expected-value score for HUNT mode. Higher is better.
pub fn score_hunt(c: &Candidate, ctx: &PirateRouteContext) -> f64 {
    let dms = encounter_dms(&c.world, c.allegiance.as_deref());
    let econ = economy_multiplier(&c.world);

    let prey = HUNT_PREY_BASE * (1.0 + 0.3 * dms.traffic as f64).max(0.1) * econ;
    let recognition = 1.0 + 0.04 * ctx.reputation;
    let threat = HUNT_THREAT_BASE
        * (1.0 + HUNT_THREAT_PER_SECURITY * dms.security.max(0) as f64)
        * recognition
        / (1.0 + 0.1 * ctx.ship_weapons.max(0) as f64);
    let dist = c.distance as f64 * ctx.base.fuel_cost_per_parsec as f64 * ROUTE_P_DIST;

    prey - threat - dist
}

/// Score for FENCE-RUN mode: prefer fenceable (low-law), nearby worlds.
pub fn score_fence(c: &Candidate, ctx: &PirateRouteContext) -> f64 {
    let law = c.world.get_law_level();
    let fenceability = match law_bonus(law) {
        Some(b) => 100_000.0 + b as f64 * 50_000.0,
        None => -1.0e12,
    };
    let dist = c.distance as f64 * ctx.base.fuel_cost_per_parsec as f64 * ROUTE_P_DIST;
    fenceability - dist
}

/// Is this candidate at hex `target` (matched by coordinates)?
fn is_at_hex(c: &Candidate, target: (i32, i32)) -> bool {
    matches!(c.world.coordinates, Some((x, y)) if x == target.0 && y == target.1)
}

/// Hex of the hideout / home base.
fn home_hex(ctx: &PirateRouteContext) -> (i32, i32) {
    (ctx.base.home.hex_x, ctx.base.home.hex_y)
}

/// From `pool`, the candidate closest (in hexes) to `target`, ties broken by
/// hunt score. `None` only if no candidate has coordinates.
fn closest_to<'a>(
    pool: &[&'a Candidate],
    target: (i32, i32),
    ctx: &PirateRouteContext,
) -> Option<&'a Candidate> {
    pool.iter()
        .filter_map(|c| {
            c.world
                .coordinates
                .map(|(x, y)| (*c, calculate_hex_distance(x, y, target.0, target.1)))
        })
        .min_by(|a, b| {
            a.1.cmp(&b.1).then_with(|| {
                score_hunt(b.0, ctx)
                    .partial_cmp(&score_hunt(a.0, ctx))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .map(|(c, _)| c)
}

/// Trip progress in `0.0..` (may exceed 1.0 past the target date).
fn progress(ctx: &PirateRouteContext) -> f64 {
    let total = ctx.base.start_date.days_until(ctx.base.target_date) as f64;
    let elapsed = ctx.base.start_date.days_until(ctx.base.current_date) as f64;
    if total > 0.0 { elapsed / total } else { 0.0 }
}

/// Pick the next system. Applies the refuel hard-filter, the home-exclusion
/// in the first half of the voyage, the head-home spiral near the end, and
/// the per-mode scoring.
pub fn pick_next_pirate<'a>(
    candidates: &'a [Candidate],
    mode: RouteMode,
    ctx: &PirateRouteContext,
) -> Option<&'a Candidate> {
    if candidates.is_empty() {
        return None;
    }
    let prog = progress(ctx);

    // Forced-terminal override once we're at/past the target date: if the
    // finish world (destination, or hideout when none is set) is reachable,
    // take it regardless of score.
    if prog >= 1.0
        && let Some(term) = candidates.iter().find(|c| is_at_hex(c, ctx.terminal_hex))
    {
        return Some(term);
    }

    // Hard filter: only refuelable systems. Fall back to the full list so a
    // fuel desert can't dead-end the voyage.
    let refuelable: Vec<&Candidate> = candidates.iter().filter(|c| is_refuelable(c)).collect();
    let pool: Vec<&Candidate> = if refuelable.is_empty() {
        candidates.iter().collect()
    } else {
        refuelable
    };

    // Hideout diversion: on a prize drop-off run (all prize crews committed)
    // or lying low (reputation too hot), make for the *hideout* — not the
    // destination — to bank prizes / let the heat cool.
    if ctx.prize_run || ctx.lying_low {
        return closest_to(&pool, home_hex(ctx), ctx);
    }

    // Head for the finish: past the head-home threshold, spiral toward the
    // terminal (the destination when set, else the hideout).
    if prog >= HEAD_HOME_THRESHOLD {
        return closest_to(&pool, ctx.terminal_hex, ctx);
    }

    // In the first half, exclude the terminal so the cruise doesn't end early
    // by arriving at the finish world.
    let search: Vec<&Candidate> = if prog < 0.5 {
        let filtered: Vec<&Candidate> = pool
            .iter()
            .copied()
            .filter(|c| !is_at_hex(c, ctx.terminal_hex))
            .collect();
        if filtered.is_empty() { pool } else { filtered }
    } else {
        pool
    };

    let score = |c: &Candidate| match mode {
        RouteMode::Hunt => score_hunt(c, ctx),
        RouteMode::FenceRun => score_fence(c, ctx),
    };

    search.into_iter().max_by(|a, b| {
        score(a)
            .partial_cmp(&score(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulator::types::{Date, WorldRef};
    use crate::systems::world::World;
    use crate::trade::ZoneClassification;

    fn world(uwp: &str, x: i32, y: i32) -> World {
        let mut w = World::from_uwp("W", uwp, false, true).unwrap();
        w.coordinates = Some((x, y));
        w.gen_trade_classes();
        w
    }

    fn cand(uwp: &str, x: i32, y: i32, dist: i32, gas: u8) -> Candidate {
        Candidate {
            world: world(uwp, x, y),
            distance: dist,
            allegiance: None,
            gas_giants: gas,
        }
    }

    fn home_ref() -> WorldRef {
        WorldRef {
            name: "Hideout".into(),
            uwp: "X000000-0".into(),
            sector: String::new(),
            hex_x: 0,
            hex_y: 0,
            zone: ZoneClassification::Green,
        }
    }

    fn base<'a>(home: &'a WorldRef, history: &'a [WorldRef]) -> RouteContext<'a> {
        RouteContext {
            home,
            current_date: Date::new(10, 1105),
            start_date: Date::new(0, 1105),
            target_date: Date::new(100, 1105),
            jump: 2,
            fuel_cost_per_parsec: 10_000,
            history,
        }
    }

    fn pctx<'a>(base: &'a RouteContext<'a>) -> PirateRouteContext<'a> {
        PirateRouteContext {
            base,
            reputation: 0.0,
            plundered_tons: 0,
            cargo_capacity: 100,
            budget: 1_000_000,
            upcoming_upkeep: 100_000,
            ship_weapons: 6,
            prize_run: false,
            lying_low: false,
            terminal_hex: (0, 0), // == home hex by default (round-trip)
        }
    }

    #[test]
    fn refuelable_by_gas_giant_or_water() {
        // Vacuum world, no gas giant → not refuelable.
        assert!(!is_refuelable(&cand("X000000-0", 1, 0, 1, 0)));
        // Same, but a gas giant in-system → refuelable.
        assert!(is_refuelable(&cand("X000000-0", 1, 0, 1, 1)));
        // Water world, no gas giant → refuelable.
        assert!(is_refuelable(&cand("A788899-A", 1, 0, 1, 0)));
    }

    #[test]
    fn mode_switches_to_fence_when_hold_full() {
        let h = home_ref();
        let b = base(&h, &[]);
        let mut ctx = pctx(&b);
        assert_eq!(route_mode(&ctx), RouteMode::Hunt);
        ctx.plundered_tons = 70; // 70/100 = 0.7 > 0.6
        assert_eq!(route_mode(&ctx), RouteMode::FenceRun);
    }

    #[test]
    fn mode_switches_to_fence_when_cash_low() {
        let h = home_ref();
        let b = base(&h, &[]);
        let mut ctx = pctx(&b);
        ctx.budget = 150_000; // < 100_000 * 2
        assert_eq!(route_mode(&ctx), RouteMode::FenceRun);
    }

    #[test]
    fn hunt_excludes_unrefuelable_systems() {
        let h = home_ref();
        let b = base(&h, &[]);
        let ctx = pctx(&b);
        // A dry vacuum world (no gas giant) vs a watery one.
        let dry = cand("A000000-0", 1, 0, 1, 0);
        let wet = cand("C788510-7", 2, 0, 1, 0); // hydro 8
        let cands = [dry, wet];
        let chosen = pick_next_pirate(&cands, RouteMode::Hunt, &ctx).unwrap();
        assert!(is_refuelable(chosen));
    }

    #[test]
    fn lying_low_routes_home_mid_cruise() {
        let h = home_ref();
        let b = base(&h, &[]); // prog 0.1 — well before the head-home threshold
        let mut ctx = pctx(&b);
        ctx.lying_low = true;
        // The hideout (hex 0,0) and a richer world farther out, both refuelable.
        let home = cand("A788899-A", 0, 0, 1, 0);
        let away = cand("A788899-A", 5, 0, 1, 0);
        let cands = [away, home];
        let chosen = pick_next_pirate(&cands, RouteMode::Hunt, &ctx).unwrap();
        assert_eq!(chosen.world.coordinates, Some((0, 0)));
    }

    #[test]
    fn head_home_spirals_to_destination_not_hideout() {
        // Past the head-home threshold with a destination set: the planner
        // must spiral toward the destination hex, not the hideout.
        let h = home_ref();
        let mut b = base(&h, &[]);
        b.current_date = Date::new(80, 1105); // prog 0.8, past HEAD_HOME_THRESHOLD
        let mut ctx = pctx(&b);
        ctx.terminal_hex = (5, 0); // destination, distinct from home (0,0)
        // Hideout (0,0) and the destination (5,0), both refuelable.
        let hideout = cand("A788899-A", 0, 0, 1, 0);
        let dest = cand("A788899-A", 5, 0, 1, 0);
        let cands = [hideout, dest];
        let chosen = pick_next_pirate(&cands, RouteMode::Hunt, &ctx).unwrap();
        assert_eq!(chosen.world.coordinates, Some((5, 0)));
    }

    #[test]
    fn lying_low_still_routes_to_hideout_with_destination_set() {
        // Even on a one-way run, lying low makes for the hideout (0,0), not
        // the destination.
        let h = home_ref();
        let b = base(&h, &[]); // prog 0.1
        let mut ctx = pctx(&b);
        ctx.lying_low = true;
        ctx.terminal_hex = (5, 0); // destination elsewhere
        let hideout = cand("A788899-A", 0, 0, 1, 0);
        let dest = cand("A788899-A", 5, 0, 1, 0);
        let cands = [dest, hideout];
        let chosen = pick_next_pirate(&cands, RouteMode::Hunt, &ctx).unwrap();
        assert_eq!(chosen.world.coordinates, Some((0, 0)));
    }

    #[test]
    fn fence_run_prefers_low_law() {
        let h = home_ref();
        let b = base(&h, &[]);
        let ctx = pctx(&b);
        // Both refuelable (water); one low-law, one high-law.
        let low_law = cand("C788810-7", 1, 0, 1, 0); // law 0
        let high_law = cand("C788818-7", 2, 0, 1, 0); // law 8 → unfenceable
        let cands = [high_law, low_law];
        let chosen = pick_next_pirate(&cands, RouteMode::FenceRun, &ctx).unwrap();
        assert!(law_bonus(chosen.world.get_law_level()).is_some());
    }
}
