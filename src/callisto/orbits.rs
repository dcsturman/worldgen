//! Orbits and zones: rulebook Section 5, Tables 10 to 14.
//!
//! An orbit is placed by its **position** in habitable distances (HD), and
//! everything here works in HD; [`crate::callisto::star::StarData::hd_mkm`]
//! turns a position into Mkm.

use crate::callisto::dice::Roller;
use crate::callisto::tables;

/// Round to two significant figures, as the rulebook rounds every position.
///
/// Ties go to the even digit. The worked examples were computed that way:
/// Noricum's 34 × 2.25 = 76.5 is printed as 76, not 77.
pub fn sig2(x: f32) -> f32 {
    if x <= 0.0 || !x.is_finite() {
        return x;
    }
    let x = f64::from(x);
    let scale = 10f64.powi(1 - x.log10().floor() as i32);
    // Go through the decimal string so 0.61 × 100 = 60.999… still rounds as
    // the 61 it is printed as.
    let scaled: f64 = format!("{:.6}", x * scale).parse().unwrap_or(x * scale);
    (scaled.round_ties_even() / scale) as f32
}

/// The zones of Table 13. Their order is Table 21's column order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Zone {
    Inner,
    Hot,
    Temperate,
    Cold,
    Outer,
}

/// Table 13's boundaries: Hot starts at 0.5, Temperate at 0.95, Cold at 1.7,
/// Outer at 2.7.
const ZONE_EDGES: [(f32, Zone); 4] = [
    (0.5, Zone::Hot),
    (0.95, Zone::Temperate),
    (1.7, Zone::Cold),
    (2.7, Zone::Outer),
];

impl Zone {
    /// The zone at a position, read straight off Table 13.
    pub fn at(position_hd: f32) -> Zone {
        ZONE_EDGES
            .iter()
            .rev()
            .find(|(edge, _)| position_hd >= *edge)
            .map_or(Zone::Inner, |(_, z)| *z)
    }

    pub fn name(self) -> &'static str {
        match self {
            Zone::Inner => "Inner",
            Zone::Hot => "Hot",
            Zone::Temperate => "Temperate",
            Zone::Cold => "Cold",
            Zone::Outer => "Outer",
        }
    }
}

/// The widest position orbits are generated to (Section 5.4: "stop when a
/// position passes 100").
pub const MAX_POSITION_HD: f32 = 100.0;

/// A main world's position from its UWP (Table 10), read from the table:
/// the first row whose atmosphere (and, where the row names one,
/// hydrographics) matches, any sub-roll the band cell calls for, then 1D
/// spreading the band evenly (1 the inner end, 6 the outer). Rounded to two
/// figures.
pub fn main_world_position(atmosphere: i32, hydro: i32, roller: &mut impl Roller) -> f32 {
    let t = tables::table(10);
    let row = t
        .rows
        .iter()
        .find(|r| row_matches(&r[0], atmosphere, hydro))
        .unwrap_or_else(|| panic!("Table 10 has no row for atmosphere {atmosphere}"));
    let cell = row[1].as_str();
    let (lo, hi) = if let Some(list) = cell.strip_prefix("Roll 2D:") {
        // The airless row: straight to a position, 2D, or 1D + 6 for a world
        // with water, which only lasts as ice away from the star.
        let roll = if hydro >= 1 { roller.d1() + 6 } else { roller.d2() };
        return airless_position(list, roll);
    } else if let Some(options) = cell.strip_prefix("Roll 1D:") {
        // "1 to 4, 0.60 to 0.85 (a hot desert); 5 to 6, 1.25 to 1.60 (…)"
        let roll = roller.d1();
        options
            .split(';')
            .find_map(|opt| {
                let (rolls, band) = opt.trim().split_once(", ")?;
                let range = crate::callisto::tables::parse_roll(rolls)?;
                range.contains(&roll).then(|| parse_band(band))?
            })
            .unwrap_or_else(|| panic!("Table 10: no band for 1D = {roll} in {cell:?}"))
    } else {
        parse_band(cell).unwrap_or_else(|| panic!("Table 10: can't read band {cell:?}"))
    };
    let step = (roller.d1().clamp(1, 6) - 1) as f32 / 5.0;
    sig2(lo + (hi - lo) * step)
}

/// "0.88 to 1.04", with anything after it (a parenthesised note) ignored.
fn parse_band(text: &str) -> Option<(f32, f32)> {
    let (lo, rest) = text.trim().split_once(" to ")?;
    let hi: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    Some((lo.trim().parse().ok()?, hi.parse().ok()?))
}

/// Does a Table 10 row label ("Atmosphere 4 or 5, hydrographics 9 or A",
/// "Atmosphere A (exotic) or F (unusual)") cover this world?
fn row_matches(label: &str, atmosphere: i32, hydro: i32) -> bool {
    let Some(rest) = label.strip_prefix("Atmosphere ") else {
        return false;
    };
    let (atm, hyd) = match rest.split_once(", hydrographics ") {
        Some((a, h)) => (a, Some(h)),
        None => (rest, None),
    };
    codes_match(atm, atmosphere) && hyd.is_none_or(|h| codes_match(h, hydro))
}

/// "4 or 5", "4 to 9", "9 or A", "0", "A (exotic) or F (unusual)": does the
/// list of ehex codes or range cover `value`?
fn codes_match(spec: &str, value: i32) -> bool {
    // Drop parenthesised notes.
    let mut clean = String::new();
    let mut depth = 0;
    for c in spec.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 => clean.push(c),
            _ => {}
        }
    }
    let code = |t: &str| {
        t.trim()
            .chars()
            .next()
            .and_then(crate::util::ehex_to_value)
            .map(|v| v as i32)
    };
    clean.split(" or ").any(|part| match part.split_once(" to ") {
        Some((a, b)) => code(a).zip(code(b)).is_some_and(|(a, b)| (a..=b).contains(&value)),
        None => code(part) == Some(value),
    })
}

/// Table 10's airless list: "2 0.10, 3 0.16, …, 12 10. If hydrographics …".
fn airless_position(list: &str, roll: i32) -> f32 {
    let list = list.split('.').collect::<Vec<_>>();
    // The list's own decimals are split by '.', so rejoin up to the sentence
    // that starts with "If".
    let joined = list
        .iter()
        .take_while(|s| !s.trim_start().starts_with("If"))
        .copied()
        .collect::<Vec<_>>()
        .join(".");
    let roll = roll.clamp(2, 12);
    joined
        .split(',')
        .filter_map(|pair| pair.trim().split_once(' '))
        .find(|(r, _)| r.parse() == Ok(roll))
        .map(|(_, p)| parse_f32(p.trim()))
        .unwrap_or_else(|| panic!("Table 10's airless row has no {roll}"))
}

/// Number of orbits (Section 5.2): 2D − 2, minimum 1, and at least as many
/// as the bodies already known to be there.
pub fn number_of_orbits(known_bodies: usize, roller: &mut impl Roller) -> usize {
    ((roller.d2() - 2).max(1) as usize).max(known_bodies)
}

/// Position of orbit 1 when there is no main world (Table 11).
pub fn first_orbit(roller: &mut impl Roller) -> f32 {
    let t = tables::table(11).dice().expect("Table 11 is a dice table");
    parse_f32(&t.lookup(roller.d2())[0])
}

/// A spacing ratio (Table 12).
pub fn spacing_ratio(roller: &mut impl Roller) -> f32 {
    let t = tables::table(12).dice().expect("Table 12 is a dice table");
    parse_f32(&t.lookup(roller.d2())[0])
}

fn parse_f32(cell: &str) -> f32 {
    cell.parse()
        .unwrap_or_else(|_| panic!("Callisto table: not a number: {cell:?}"))
}

/// Range of positions (HD) a companion star keeps clear: from "worlds orbit
/// the primary alone out to" up to "worlds orbit both stars beyond". `hi` is
/// infinite where nothing orbits both.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gap {
    pub lo: f32,
    pub hi: f32,
}

impl Gap {
    /// Worlds orbit the primary alone *out to* `lo`, and both stars only
    /// *beyond* `hi`, so `lo` itself is stable and `hi` is not.
    pub fn contains(&self, position_hd: f32) -> bool {
        position_hd > self.lo && position_hd <= self.hi
    }
}

/// The result of laying out one star's orbits.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OrbitPlan {
    /// Stable positions, innermost first.
    pub positions: Vec<f32>,
    /// Index into `positions` of the main world, if there is one.
    pub main_world: Option<usize>,
    /// Positions rolled inside a companion's gap and crossed out.
    pub crossed_out: Vec<f32>,
    /// How many of the requested orbits could not be placed (the 100 HD cap
    /// or the innermost limit ran out of room).
    pub shortfall: usize,
    /// The last Table 12 ratio rolled.
    pub last_ratio: Option<f32>,
    /// Orbits a known count forced beyond 100 HD because no gap could be
    /// split (Section 5.4); each deserves a note on the record.
    pub beyond_100: usize,
}

/// Split the widest gap between neighbouring positions (Section 5.4, "More
/// orbits for a known count"): the geometric mean of the two, if they are at
/// least 1.56 apart so both halves keep Table 12's minimum of 1.25. The edges
/// of a companion's gap count as positions, but a new orbit never goes inside
/// a gap. `None` when nothing can be split.
pub fn split_widest(positions: &[f32], gaps: &[Gap]) -> Option<f32> {
    let in_gap = |p: f32| gaps.iter().any(|g| g.contains(p));
    let (lo, hi) = (
        positions.iter().copied().fold(f32::MAX, f32::min),
        positions.iter().copied().fold(f32::MIN, f32::max),
    );
    let mut points: Vec<f32> = positions
        .iter()
        .copied()
        .chain(gaps.iter().flat_map(|g| [g.lo, g.hi]).filter(|&e| e > lo && e < hi))
        .collect();
    points.sort_by(f32::total_cmp);
    points.dedup();
    let mut pairs: Vec<(f32, f32)> = points
        .windows(2)
        .map(|w| (w[0], w[1]))
        // A stretch across a gap's interior is the gap, not a gap to split.
        .filter(|&(a, b)| !in_gap((a * b).sqrt()) && b / a >= SPLIT_RATIO)
        .collect();
    pairs.sort_by(|x, y| (y.1 / y.0).total_cmp(&(x.1 / x.0)));
    pairs
        .into_iter()
        .map(|(a, b)| sig2((a * b).sqrt()))
        .find(|&mid| !in_gap(mid) && !positions.contains(&mid))
}

/// The closest two neighbours may be and still be split: √1.56 ≈ 1.25, Table
/// 12's smallest ratio.
pub const SPLIT_RATIO: f32 = 1.56;

/// Multiply outward past 100 HD, skipping gaps, for a known count with no gap
/// left to split. Returns the new position.
pub fn beyond(outermost: f32, gaps: &[Gap], roller: &mut impl Roller, crossed: &mut Vec<f32>) -> f32 {
    let mut p = sig2(outermost * spacing_ratio(roller));
    while gaps.iter().any(|g| g.contains(p)) {
        crossed.push(p);
        p = sig2(p * spacing_ratio(roller));
    }
    p
}

/// Lay out a star's orbits (Sections 5.3 and 5.4).
///
/// - `count`: orbits wanted, from [`number_of_orbits`].
/// - `main_world`: the main world's position, or `None` to build outward from
///   Table 11.
/// - `innermost`: the star's innermost orbit (Table 5).
/// - `gaps`: companion gaps to cross positions out of.
///
/// `known` says the count is a known number of bodies rather than a 2D roll:
/// if the outward run passes 100 HD short of it, the widest gaps are split
/// until it is met (Section 5.4).
///
/// `inward` fixes how many orbits lie inside the main world, for a source
/// that gives its place in the order ("the fourth planet"); otherwise 1D
/// splits them.
pub fn lay_out(
    count: usize,
    main_world: Option<f32>,
    innermost: f32,
    gaps: &[Gap],
    known: bool,
    inward: Option<usize>,
    roller: &mut impl Roller,
) -> OrbitPlan {
    let count = inward.map_or(count, |n| count.max(n + 1));
    let mut plan = lay_out_run(count, main_world, innermost, gaps, inward, roller);
    // A main world whose place in the order is given needs exactly that many
    // orbits inside it; where the inward run came up short (the innermost
    // orbit, a companion's gap), split the widest gaps inside it.
    if let (Some(want), Some(mw)) = (inward, main_world) {
        loop {
            let inside: Vec<f32> = plan.positions.iter().copied().filter(|&p| p < mw).collect();
            if inside.len() >= want {
                break;
            }
            let mut bounded = vec![innermost];
            bounded.extend(inside.iter().copied().filter(|&p| p > innermost));
            bounded.push(mw);
            let Some(p) = split_widest(&bounded, gaps).filter(|&p| p < mw && p >= innermost) else {
                break;
            };
            let at = plan.positions.partition_point(|&x| x < p);
            plan.positions.insert(at, p);
        }
        plan.main_world = plan.positions.iter().position(|&p| p == mw);
    }
    if known {
        while plan.positions.len() < count {
            let p = match split_widest(&plan.positions, gaps) {
                Some(p) => p,
                None => {
                    plan.beyond_100 += 1;
                    let outermost = *plan.positions.last().unwrap_or(&innermost);
                    beyond(outermost, gaps, roller, &mut plan.crossed_out)
                }
            };
            let at = plan.positions.partition_point(|&x| x < p);
            plan.positions.insert(at, p);
        }
        plan.main_world = main_world.and_then(|m| plan.positions.iter().position(|&p| p == m));
        plan.shortfall = 0;
    }
    plan
}

/// Section 5.3's run: inward and outward from the main world by Table 12
/// ratios, or outward from Table 11's first orbit.
fn lay_out_run(
    count: usize,
    main_world: Option<f32>,
    innermost: f32,
    gaps: &[Gap],
    inward: Option<usize>,
    roller: &mut impl Roller,
) -> OrbitPlan {
    let in_gap = |p: f32| gaps.iter().any(|g| g.contains(p));
    let mut plan = OrbitPlan::default();

    let Some(mw) = main_world else {
        // Orbit 1 from Table 11, moved out to the innermost orbit if closer,
        // then outward by ratios.
        let first = first_orbit(roller).max(innermost);
        let mut outward = outward_from(first, count, true, &in_gap, roller, &mut plan.crossed_out, &mut plan.last_ratio);
        if in_gap(first) {
            plan.crossed_out.push(first);
            outward.remove(0);
        }
        plan.shortfall = count.saturating_sub(outward.len());
        plan.positions = outward;
        return plan;
    };

    // Split the other orbits by 1D: 1–2 a third inward, 3–4 half, 5–6 two
    // thirds, rounding down.
    let others = count.saturating_sub(1);
    let inward_n = inward.unwrap_or_else(|| match roller.d1() {
        1 | 2 => others / 3,
        3 | 4 => others / 2,
        _ => others * 2 / 3,
    });
    let outward_n = others - inward_n;

    // Inward: divide, stopping at the innermost orbit. An inward orbit that
    // would fall below it is not generated.
    let mut inward = Vec::new();
    let mut p = mw;
    let mut made = 0;
    while made < inward_n {
        let ratio = spacing_ratio(roller);
        plan.last_ratio = Some(ratio);
        p = sig2(p / ratio);
        if p < innermost {
            break;
        }
        if in_gap(p) {
            plan.crossed_out.push(p);
            continue;
        }
        inward.push(p);
        made += 1;
    }
    let inward_short = inward_n - inward.len();

    // Outward: the orbits the inward side couldn't fit go outward instead,
    // so a known body count still gets its orbits.
    let outward = outward_from(
        mw,
        outward_n + inward_short + 1,
        false,
        &in_gap,
        roller,
        &mut plan.crossed_out,
        &mut plan.last_ratio,
    );

    inward.reverse();
    plan.main_world = Some(inward.len());
    plan.positions = inward;
    plan.positions.extend(outward);
    plan.shortfall = count.saturating_sub(plan.positions.len());
    plan
}

/// Multiply outward from `start` until `count` stable positions (counting
/// `start`) exist or the 100 HD cap is reached. Positions in a gap are
/// crossed out, don't count, and the multiplying carries on from them.
///
/// `start_may_be_in_gap` says whether `start` itself was rolled (Table 11)
/// rather than fixed (a main world); the caller handles crossing it out.
fn outward_from(
    start: f32,
    count: usize,
    start_may_be_in_gap: bool,
    in_gap: &impl Fn(f32) -> bool,
    roller: &mut impl Roller,
    crossed_out: &mut Vec<f32>,
    last_ratio: &mut Option<f32>,
) -> Vec<f32> {
    let mut out = vec![start];
    let mut stable = if start_may_be_in_gap && in_gap(start) { 0 } else { 1 };
    let mut p = start;
    while stable < count && p < MAX_POSITION_HD {
        let ratio = spacing_ratio(roller);
        *last_ratio = Some(ratio);
        p = sig2(p * ratio).min(MAX_POSITION_HD);
        if in_gap(p) {
            crossed_out.push(p);
            continue;
        }
        out.push(p);
        stable += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callisto::dice::{Kind::*, Scripted};

    #[test]
    fn two_figure_rounding_matches_the_examples() {
        assert_eq!(sig2(1.0 / 1.65), 0.61);
        assert_eq!(sig2(0.61 / 1.35), 0.45);
        assert_eq!(sig2(4.5 * 1.75), 7.9);
        assert_eq!(sig2(34.0 * 2.25), 76.0);
        assert_eq!(sig2(1.4 * 1.45), 2.0);
        assert_eq!(sig2(0.125), 0.12);
        assert_eq!(sig2(130.0), 130.0);
    }

    #[test]
    fn zones_follow_table_13() {
        assert_eq!(Zone::at(0.45), Zone::Inner);
        assert_eq!(Zone::at(0.5), Zone::Hot);
        assert_eq!(Zone::at(0.95), Zone::Temperate);
        assert_eq!(Zone::at(1.6), Zone::Temperate);
        assert_eq!(Zone::at(1.7), Zone::Cold);
        assert_eq!(Zone::at(2.7), Zone::Outer);
        // The constants must still be what Table 13 prints.
        let printed: Vec<String> = tables::table(13).rows.iter().map(|r| r[1].clone()).collect();
        assert_eq!(
            printed,
            ["below 0.5", "0.5 to 0.95", "0.95 to 1.7", "1.7 to 2.7", "2.7 and beyond"]
        );
    }

    #[test]
    fn table_10_rows_are_read() {
        // Every atmosphere and hydrographics a main world can have finds a row.
        for atm in 0..=15 {
            for hyd in 0..=10 {
                assert!(
                    tables::table(10).rows.iter().any(|r| row_matches(&r[0], atm, hyd)),
                    "no Table 10 row for atmosphere {atm}, hydrographics {hyd}"
                );
            }
        }
        assert!(row_matches("Atmosphere 4 or 5, hydrographics 9 or A", 5, 10));
        assert!(!row_matches("Atmosphere 4 or 5, hydrographics 9 or A", 5, 8));
        assert!(row_matches("Atmosphere A (exotic) or F (unusual)", 15, 3));
        assert!(!row_matches("Atmosphere A (exotic) or F (unusual)", 11, 3));
        assert!(row_matches("Atmosphere 4 to 9, hydrographics 0", 7, 0));
    }

    /// Rulebook 8.5 via Table 10: every habitable main world, at every 1D
    /// result, comes out Temperate.
    #[test]
    fn habitable_main_worlds_placed_by_table_10_are_temperate() {
        use crate::callisto::dice::Scripted;
        use crate::callisto::temperature::{TempBand, temperature};
        for atm in 4..=9 {
            for hyd in 1..=10 {
                for d in 1..=6 {
                    let p = main_world_position(atm, hyd, &mut Scripted::new(&[(D1, d)]));
                    let t = temperature(p, atm, hyd, false);
                    assert_eq!(
                        t.band,
                        TempBand::Temperate,
                        "atmosphere {atm}, hydrographics {hyd}, 1D = {d}: {p} HD, {:.1} °C",
                        t.celsius
                    );
                }
            }
        }
    }

    #[test]
    fn the_band_spreads_over_the_die() {
        // Example 13.1: atmosphere 6, hydrographics 7, band 0.91 to 1.11,
        // 1D = 4 → 1.03 → 1.0.
        assert_eq!(main_world_position(6, 7, &mut Scripted::new(&[(D1, 4)])), 1.0);
        // Noricum: atmosphere 8, band 1.10 to 1.38, 1D = 6 → 1.38 → 1.4.
        assert_eq!(main_world_position(8, 6, &mut Scripted::new(&[(D1, 6)])), 1.4);
        assert_eq!(main_world_position(8, 6, &mut Scripted::new(&[(D1, 1)])), 1.1);
        // Airless: 2D straight to a position; with water, 1D + 6.
        assert_eq!(main_world_position(0, 0, &mut Scripted::new(&[(D2, 7)])), 1.0);
        assert_eq!(main_world_position(1, 3, &mut Scripted::new(&[(D1, 1)])), 1.0);
        assert_eq!(main_world_position(1, 3, &mut Scripted::new(&[(D1, 6)])), 10.0);
        // A cold desert: sub-roll 5, then 1D = 6 → 1.60 → 1.6.
        assert_eq!(main_world_position(6, 0, &mut Scripted::new(&[(D1, 5), (D1, 6)])), 1.6);
    }

    /// Example 13.1's orbits: four orbits around a main world at 1.0.
    #[test]
    fn example_12_1_orbits() {
        let mut r = Scripted::new(&[(D2, 6), (D1, 5), (D2, 6), (D2, 3), (D2, 5)]);
        let n = number_of_orbits(0, &mut r);
        let plan = lay_out(n, Some(1.0), 0.05, &[], false, None, &mut r);
        assert_eq!(plan.positions, [0.45, 0.61, 1.0, 1.6]);
        assert_eq!(plan.main_world, Some(2));
        assert_eq!(r.remaining(), 0);
    }

    /// Example 13.2's orbits: no main world around an M4 V, whose innermost
    /// orbit is 0.14.
    #[test]
    fn example_12_2_orbits() {
        let mut r = Scripted::new(&[(D2, 5), (D2, 6), (D2, 8), (D2, 2)]);
        let n = number_of_orbits(0, &mut r);
        let plan = lay_out(n, None, 0.14, &[], false, None, &mut r);
        // 0.125 moves out to 0.14; ×1.90 = 0.27; ×1.25 = 0.34.
        assert_eq!(plan.positions, [0.14, 0.27, 0.34]);
        assert_eq!(r.remaining(), 0);
    }

    /// Noricum's orbits (rulebook 13.3): fourteen known bodies, the M6
    /// companion's gap from 2 to 18 HD, and the outward run stopping at 100
    /// nine orbits short of fourteen, so the five widest gaps are split.
    #[test]
    fn noricum_orbits() {
        let mut r = Scripted::new(&[
            (D2, 4), // 2D − 2 = 2, but 14 bodies are known
            (D1, 1), // a third of the other thirteen inward: 4
            (D2, 8), (D2, 9), (D2, 7), (D2, 10), // inward
            (D2, 4), (D2, 10), (D2, 7), (D2, 10), // 2.0, then 4.5, 7.9, 18 crossed out
            (D2, 8), (D2, 10), (D2, 6), // 34, 76, 100
        ]);
        let n = number_of_orbits(14, &mut r);
        assert_eq!(n, 14);
        let gap = Gap { lo: 2.0, hi: 18.0 };
        let plan = lay_out(n, Some(1.4), 0.05, &[gap], true, None, &mut r);
        // The example prints the innermost as 0.091, dividing the unrounded
        // 0.2057 by 2.25; the rule rounds every position, so 0.21 ÷ 2.25 is
        // 0.093 and its split with 0.21 is 0.14 either way.
        assert_eq!(
            plan.positions,
            [0.093, 0.14, 0.21, 0.36, 0.52, 0.74, 1.0, 1.4, 2.0, 25.0, 34.0, 51.0, 76.0, 100.0]
        );
        assert_eq!(plan.main_world, Some(7));
        assert_eq!(plan.crossed_out, [4.5, 7.9, 18.0]);
        assert_eq!(plan.beyond_100, 0);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn splitting_prefers_the_widest_gap_and_respects_companion_gaps() {
        let gap = Gap { lo: 2.0, hi: 18.0 };
        // 18 (the gap's outer edge) to 34 is split; 2.0 to 18 is the gap.
        assert_eq!(split_widest(&[1.4, 2.0, 34.0], &[gap]), Some(25.0));
        // Nothing 1.56 apart: no split.
        assert_eq!(split_widest(&[1.0, 1.5, 2.2], &[]), None);
    }
}
