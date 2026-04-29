//! Incident and marooning helpers for the ship simulator.
//!
//! This module hosts the pure logic for the incident system that runs at
//! every port: rolling for avoidance, rolling on the incident table, and
//! computing port/zone/law modifiers. It also owns the cargo-loss
//! algorithm used by the Piracy variant and the rescue-ETA helper used by
//! marooning.
//!
//! Random helpers (`roll_2d6`, `roll_1d6`, `roll_1d3`) live here so the
//! executor and tests share one source of truth.

use rand::Rng;

use crate::simulator::economy::{DAYS_PER_WEEK, RESCUE_PARSECS_PER_WEEK};
use crate::trade::PortCode;
use crate::trade::ZoneClassification;
use crate::trade::available_goods::AvailableGoodsTable;

/// Roll 2d6.
pub fn roll_2d6() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(1..=6) + rng.random_range(1..=6)
}

/// Roll 1d6.
pub fn roll_1d6() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(1..=6)
}

/// Roll 1d3 (uniform `1..=3`).
pub fn roll_1d3() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(1..=3)
}

/// Port-quality modifier component.
fn port_mod(port: PortCode) -> i32 {
    match port {
        PortCode::A => 4,
        PortCode::B => 2,
        PortCode::C => 0,
        PortCode::D => 0,
        PortCode::E => -2,
        PortCode::X => -4,
        _ => 0,
    }
}

/// Travel-zone modifier component.
fn zone_mod(zone: ZoneClassification) -> i32 {
    match zone {
        ZoneClassification::Green => 0,
        ZoneClassification::Amber => -2,
        ZoneClassification::Red => -4,
    }
}

/// Law-level modifier for the *avoidance* roll. Both anarchic worlds
/// (law 0) and police-state worlds (law 10+) make incidents harder to
/// avoid, just for different reasons.
fn avoidance_law_mod(law: i32) -> i32 {
    match law {
        ..=-1 => -4,
        0 => -4,
        1..=4 => -2,
        5..=9 => 0,
        _ => -2, // 10..
    }
}

/// Law-level modifier for the *incident-table* roll. Higher law means
/// the kinds of incidents that do happen tend toward government
/// complications rather than piracy.
fn table_law_mod(law: i32) -> i32 {
    match law {
        ..=7 => 0,
        8..=9 => 2,
        _ => 4, // 10..
    }
}

/// Avoidance-roll penalty applied while the ship is in foreign-empire
/// space (Aslan Hierate, Zhodani Consulate, Solomani, etc.). Subtracted
/// from the avoidance roll, making incidents more likely.
pub const FOREIGN_EMPIRE_AVOIDANCE_PENALTY: i32 = -4;

/// Incident-table penalty applied while in foreign-empire space. Added
/// to the table roll, biasing outcomes toward Accident / Government
/// Complication (the high end of the table).
pub const FOREIGN_EMPIRE_TABLE_PENALTY: i32 = 4;

/// Sum the three avoidance components, applying the X+Red collapse rule:
/// when both port and zone would each contribute -4, only count one of
/// them. If `is_foreign` is true (the ship is in non-Imperial space), an
/// extra penalty is applied on top.
pub fn avoidance_modifier(
    port: PortCode,
    zone: ZoneClassification,
    law: i32,
    is_foreign: bool,
) -> i32 {
    let mut total = port_mod(port) + zone_mod(zone) + avoidance_law_mod(law);
    if matches!(port, PortCode::X) && matches!(zone, ZoneClassification::Red) {
        // Both contributed -4; cancel one.
        total += 4;
    }
    if is_foreign {
        total += FOREIGN_EMPIRE_AVOIDANCE_PENALTY;
    }
    total
}

/// Sum the three incident-table components, applying the X+Red collapse
/// rule (same as `avoidance_modifier`). If `is_foreign` is true, the
/// foreign-empire penalty is added on top, pushing outcomes toward the
/// worse end of the table.
pub fn incident_table_modifier(
    port: PortCode,
    zone: ZoneClassification,
    law: i32,
    is_foreign: bool,
) -> i32 {
    let mut total = port_mod(port) + zone_mod(zone) + table_law_mod(law);
    if matches!(port, PortCode::X) && matches!(zone, ZoneClassification::Red) {
        total += 4;
    }
    if is_foreign {
        total += FOREIGN_EMPIRE_TABLE_PENALTY;
    }
    total
}

/// Remove `target_tons` tons of cargo from the manifest by repeatedly
/// picking a good uniformly at random among those still in stock and
/// depleting it before moving on. Returns the per-good loss list and the
/// total `buy_cost * tons_removed` sunk.
///
/// If the manifest contains fewer than `target_tons` tons total, removes
/// everything available.
pub fn pirate_cargo(
    goods: &mut AvailableGoodsTable,
    target_tons: i32,
    rng: &mut impl Rng,
) -> (Vec<(String, i32)>, i64) {
    if target_tons <= 0 {
        return (Vec::new(), 0);
    }

    let mut breakdown: Vec<(String, i32)> = Vec::new();
    let mut buy_cost_sunk: i64 = 0;
    let mut remaining = target_tons;

    while remaining > 0 {
        // Collect indices of goods that still have stock.
        let candidates: Vec<usize> = goods
            .goods
            .iter()
            .enumerate()
            .filter_map(|(i, g)| if g.quantity > 0 { Some(i) } else { None })
            .collect();
        if candidates.is_empty() {
            break;
        }
        let pick = candidates[rng.random_range(0..candidates.len())];
        let g = &mut goods.goods[pick];
        let take = g.quantity.min(remaining);
        g.quantity -= take;
        remaining -= take;
        buy_cost_sunk += g.buy_cost as i64 * take as i64;

        // Append to breakdown — coalesce if we already have an entry for
        // this good (would only happen if a later pick lands on the same
        // index after we depleted then re-picked, which can't happen
        // because depleted goods are filtered out — but be defensive).
        if let Some(slot) = breakdown.iter_mut().find(|(name, _)| name == &g.name) {
            slot.1 += take;
        } else {
            breakdown.push((g.name.clone(), take));
        }
    }

    (breakdown, buy_cost_sunk)
}

/// Days until a help message reaches the home port, given the parsecs
/// the ship has already jumped from home: `ceil(parsecs / 4) * 7`.
pub fn rescue_eta_days(total_parsecs_jumped: u32) -> u32 {
    if total_parsecs_jumped == 0 {
        return 0;
    }
    let weeks = total_parsecs_jumped.div_ceil(RESCUE_PARSECS_PER_WEEK);
    weeks * DAYS_PER_WEEK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::available_goods::Good;
    use rand::SeedableRng;

    // ----- avoidance_modifier --------------------------------------------

    #[test]
    fn avoidance_a_green_law5() {
        // port +4, zone 0, law 0 = +4
        assert_eq!(
            avoidance_modifier(PortCode::A, ZoneClassification::Green, 5, false),
            4
        );
    }

    #[test]
    fn avoidance_x_red_law5_collapses() {
        // port -4 + zone -4 + law 0, collapse cancels one -4 → -4 (NOT -8).
        assert_eq!(
            avoidance_modifier(PortCode::X, ZoneClassification::Red, 5, false),
            -4
        );
    }

    #[test]
    fn avoidance_x_amber_law5() {
        // port -4 + zone -2 + law 0 = -6
        assert_eq!(
            avoidance_modifier(PortCode::X, ZoneClassification::Amber, 5, false),
            -6
        );
    }

    #[test]
    fn avoidance_e_red_law5() {
        // port -2 + zone -4 + law 0 = -6
        assert_eq!(
            avoidance_modifier(PortCode::E, ZoneClassification::Red, 5, false),
            -6
        );
    }

    #[test]
    fn avoidance_a_red_law5() {
        // port +4 + zone -4 + law 0 = 0
        assert_eq!(
            avoidance_modifier(PortCode::A, ZoneClassification::Red, 5, false),
            0
        );
    }

    #[test]
    fn avoidance_b_green_law0() {
        // port +2 + zone 0 + law -4 = -2
        assert_eq!(
            avoidance_modifier(PortCode::B, ZoneClassification::Green, 0, false),
            -2
        );
    }

    #[test]
    fn avoidance_x_red_law0_collapsed_plus_law() {
        // collapsed -4 + law -4 = -8
        assert_eq!(
            avoidance_modifier(PortCode::X, ZoneClassification::Red, 0, false),
            -8
        );
    }

    #[test]
    fn avoidance_x_red_law10() {
        // collapsed -4 + law -2 = -6
        assert_eq!(
            avoidance_modifier(PortCode::X, ZoneClassification::Red, 10, false),
            -6
        );
    }

    // ----- incident_table_modifier ---------------------------------------

    #[test]
    fn table_a_green_law5() {
        // port +4 + zone 0 + law 0 = +4
        assert_eq!(
            incident_table_modifier(PortCode::A, ZoneClassification::Green, 5, false),
            4
        );
    }

    #[test]
    fn table_x_red_law8() {
        // collapsed -4 + law +2 = -2
        assert_eq!(
            incident_table_modifier(PortCode::X, ZoneClassification::Red, 8, false),
            -2
        );
    }

    #[test]
    fn table_a_green_law10() {
        // port +4 + zone 0 + law +4 = +8
        assert_eq!(
            incident_table_modifier(PortCode::A, ZoneClassification::Green, 10, false),
            8
        );
    }

    // ----- pirate_cargo --------------------------------------------------

    fn mk_good(name: &str, qty: i32, buy_cost: i32) -> Good {
        Good {
            name: name.to_string(),
            quantity: qty,
            transacted: 0,
            base_cost: buy_cost,
            buy_cost,
            buy_cost_comment: String::new(),
            sell_price: None,
            sell_price_comment: String::new(),
            source_index: 0,
            quantity_roll: 0,
            buy_price_roll: None,
            sell_price_roll: None,
        }
    }

    #[test]
    fn pirate_cargo_total_preserved_when_under_available() {
        let mut t = AvailableGoodsTable {
            goods: vec![
                mk_good("A", 30, 100),
                mk_good("B", 20, 200),
                mk_good("C", 50, 300),
            ],
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let (breakdown, sunk) = pirate_cargo(&mut t, 40, &mut rng);
        let stolen: i32 = breakdown.iter().map(|(_, q)| q).sum();
        assert_eq!(stolen, 40);
        let remaining: i32 = t.goods.iter().map(|g| g.quantity).sum();
        assert_eq!(remaining, 30 + 20 + 50 - 40);
        // Sunk cost matches the breakdown.
        let expected_sunk: i64 = breakdown
            .iter()
            .map(|(name, q)| {
                let g = t.goods.iter().find(|g| &g.name == name).unwrap();
                g.buy_cost as i64 * *q as i64
            })
            .sum();
        assert_eq!(sunk, expected_sunk);
    }

    #[test]
    fn pirate_cargo_runs_out_one_good_before_next() {
        let mut t = AvailableGoodsTable {
            goods: vec![mk_good("A", 5, 100), mk_good("B", 100, 200)],
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        // Ask for 60 — first good must be fully drained, then the
        // remaining 55 comes from the second good.
        let (_breakdown, _sunk) = pirate_cargo(&mut t, 60, &mut rng);
        let remaining: i32 = t.goods.iter().map(|g| g.quantity).sum();
        assert_eq!(remaining, 5 + 100 - 60);
        // No good has gone negative.
        assert!(t.goods.iter().all(|g| g.quantity >= 0));
    }

    #[test]
    fn pirate_cargo_caps_at_total() {
        let mut t = AvailableGoodsTable {
            goods: vec![mk_good("A", 10, 100), mk_good("B", 5, 200)],
        };
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let (breakdown, sunk) = pirate_cargo(&mut t, 1_000, &mut rng);
        let stolen: i32 = breakdown.iter().map(|(_, q)| q).sum();
        assert_eq!(stolen, 15);
        assert!(t.goods.iter().all(|g| g.quantity == 0));
        assert_eq!(sunk, 10 * 100 + 5 * 200);
    }

    // ----- rescue_eta_days -----------------------------------------------

    #[test]
    fn rescue_eta_zero_parsecs_zero_days() {
        assert_eq!(rescue_eta_days(0), 0);
    }

    #[test]
    fn rescue_eta_one_parsec_one_week() {
        assert_eq!(rescue_eta_days(1), 7);
    }

    #[test]
    fn rescue_eta_four_parsecs_one_week() {
        assert_eq!(rescue_eta_days(4), 7);
    }

    #[test]
    fn rescue_eta_five_parsecs_two_weeks() {
        assert_eq!(rescue_eta_days(5), 14);
    }

    #[test]
    fn rescue_eta_eight_parsecs_two_weeks() {
        assert_eq!(rescue_eta_days(8), 14);
    }

    #[test]
    fn rescue_eta_nine_parsecs_three_weeks() {
        assert_eq!(rescue_eta_days(9), 21);
    }

    // ----- foreign empire penalty ----------------------------------------

    #[test]
    fn avoidance_foreign_penalty_subtracts_4() {
        // A-port Green law 5 = +4 friendly. With is_foreign=true, +4 - 4 = 0.
        let friendly = avoidance_modifier(PortCode::A, ZoneClassification::Green, 5, false);
        let foreign = avoidance_modifier(PortCode::A, ZoneClassification::Green, 5, true);
        assert_eq!(friendly, 4);
        assert_eq!(foreign, 0);
        assert_eq!(foreign - friendly, FOREIGN_EMPIRE_AVOIDANCE_PENALTY);
    }

    #[test]
    fn table_foreign_penalty_adds_4() {
        // C-port Green law 5 = 0 friendly. With is_foreign=true, 0 + 4 = 4.
        let friendly = incident_table_modifier(PortCode::C, ZoneClassification::Green, 5, false);
        let foreign = incident_table_modifier(PortCode::C, ZoneClassification::Green, 5, true);
        assert_eq!(friendly, 0);
        assert_eq!(foreign, 4);
        assert_eq!(foreign - friendly, FOREIGN_EMPIRE_TABLE_PENALTY);
    }

    #[test]
    fn foreign_penalty_stacks_with_xred_collapse() {
        // X-port Red law 0: collapsed -4 + law -4 = -8 friendly. Foreign: -12.
        let friendly = avoidance_modifier(PortCode::X, ZoneClassification::Red, 0, false);
        let foreign = avoidance_modifier(PortCode::X, ZoneClassification::Red, 0, true);
        assert_eq!(friendly, -8);
        assert_eq!(foreign, -12);
    }
}
