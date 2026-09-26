//! Orbits and zones: rulebook Section 4, Tables 10 to 14.
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

/// The widest position orbits are generated to (Section 4.4: "stop when a
/// position passes 100").
pub const MAX_POSITION_HD: f32 = 100.0;

/// A main world's position band from its UWP (Table 10), rolling any sub-roll
/// the row calls for, then spreading the band over 1D (1 the inner end, 6 the
/// outer). Returns the position in HD, rounded to two figures.
pub fn main_world_position(atmosphere: i32, hydro: i32, roller: &mut impl Roller) -> f32 {
    let band = match atmosphere {
        4 | 5 if hydro >= 1 => Band::Range(0.85, 1.00),
        6 | 7 if hydro >= 1 => Band::Range(0.90, 1.15),
        8 | 9 if hydro >= 1 => Band::Range(1.10, 1.40),
        4..=9 => {
            if roller.d1() <= 4 {
                Band::Range(0.60, 0.85) // a hot desert
            } else {
                Band::Range(1.25, 1.60) // a cold desert
            }
        }
        2 | 3 => Band::Range(0.75, 0.95),
        0 | 1 => Band::Exact(airless_position(roller.d2())),
        10 | 15 => Band::Range(1.00, 1.30),
        11 | 12 => {
            if roller.d1() <= 3 {
                Band::Range(0.30, 0.90) // Venus-like
            } else {
                Band::Range(2.5, 3.7) // cold, kept mild by a thick sky
            }
        }
        13 => Band::Range(1.60, 2.10),
        14 => Band::Range(0.80, 1.00),
        _ => Band::Range(0.90, 1.15),
    };
    match band {
        Band::Exact(p) => p,
        Band::Range(lo, hi) => {
            let step = (roller.d1().clamp(1, 6) - 1) as f32 / 5.0;
            sig2(lo + (hi - lo) * step)
        }
    }
}

enum Band {
    Range(f32, f32),
    Exact(f32),
}

/// Table 10's airless row, read from its printed cell: "Roll 2D: 2 0.10,
/// 3 0.16, …, 12 10". Airless worlds can be anywhere.
fn airless_position(roll: i32) -> f32 {
    let cell = &tables::table(10)
        .rows
        .iter()
        .find(|r| r[0].starts_with("Atmosphere 0 or 1"))
        .expect("Table 10 has an airless row")[1];
    let list = cell.split_once(':').map_or(cell.as_str(), |(_, l)| l);
    let roll = roll.clamp(2, 12);
    list.split(',')
        .filter_map(|pair| pair.trim().split_once(' '))
        .find(|(r, _)| r.parse() == Ok(roll))
        .map(|(_, p)| parse_f32(p))
        .unwrap_or_else(|| panic!("Table 10's airless row has no {roll}"))
}

/// Number of orbits (Section 4.2): 2D − 2, minimum 1, and at least as many
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
}

/// Lay out a star's orbits (Sections 4.3 and 4.4).
///
/// - `count`: orbits wanted, from [`number_of_orbits`].
/// - `main_world`: the main world's position, or `None` to build outward from
///   Table 11.
/// - `innermost`: the star's innermost orbit (Table 5).
/// - `gaps`: companion gaps to cross positions out of.
pub fn lay_out(
    count: usize,
    main_world: Option<f32>,
    innermost: f32,
    gaps: &[Gap],
    roller: &mut impl Roller,
) -> OrbitPlan {
    let in_gap = |p: f32| gaps.iter().any(|g| g.contains(p));
    let mut plan = OrbitPlan::default();

    let Some(mw) = main_world else {
        // Orbit 1 from Table 11, moved out to the innermost orbit if closer,
        // then outward by ratios.
        let first = first_orbit(roller).max(innermost);
        let mut outward = outward_from(first, count, true, &in_gap, roller, &mut plan.crossed_out);
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
    let inward_n = match roller.d1() {
        1 | 2 => others / 3,
        3 | 4 => others / 2,
        _ => others * 2 / 3,
    };
    let outward_n = others - inward_n;

    // Inward: divide, stopping at the innermost orbit. An inward orbit that
    // would fall below it is not generated.
    let mut inward = Vec::new();
    let mut p = mw;
    let mut made = 0;
    while made < inward_n {
        p = sig2(p / spacing_ratio(roller));
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
    let outward = outward_from(mw, outward_n + inward_short + 1, false, &in_gap, roller, &mut plan.crossed_out);

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
) -> Vec<f32> {
    let mut out = vec![start];
    let mut stable = if start_may_be_in_gap && in_gap(start) { 0 } else { 1 };
    let mut p = start;
    while stable < count && p < MAX_POSITION_HD {
        p = sig2(p * spacing_ratio(roller)).min(MAX_POSITION_HD);
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
    fn table_10_constants_are_still_printed() {
        let printed = tables::table(10)
            .rows
            .iter()
            .map(|r| r[1].as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        for band in [
            "0.85 to 1.00", "0.90 to 1.15", "1.10 to 1.40", "0.60 to 0.85", "1.25 to 1.60",
            "0.75 to 0.95", "1.00 to 1.30", "0.30 to 0.90", "2.5 to 3.7", "1.60 to 2.10",
            "0.80 to 1.00",
        ] {
            assert!(printed.contains(band), "Table 10 no longer prints {band}");
        }
    }

    #[test]
    fn the_band_spreads_over_the_die() {
        // Example 12.1: atmosphere 6, hydrographics 7, 1D = 3 → 1.0.
        assert_eq!(main_world_position(6, 7, &mut Scripted::new(&[(D1, 3)])), 1.0);
        // Noricum: atmosphere 8, 1D = 6 → 1.4.
        assert_eq!(main_world_position(8, 6, &mut Scripted::new(&[(D1, 6)])), 1.4);
        assert_eq!(main_world_position(8, 6, &mut Scripted::new(&[(D1, 1)])), 1.1);
        // Airless: 2D straight to a position.
        assert_eq!(main_world_position(0, 0, &mut Scripted::new(&[(D2, 7)])), 1.0);
    }

    /// Example 12.1's orbits: four orbits around a main world at 1.0.
    #[test]
    fn example_12_1_orbits() {
        let mut r = Scripted::new(&[(D2, 6), (D1, 5), (D2, 6), (D2, 3), (D2, 5)]);
        let n = number_of_orbits(0, &mut r);
        let plan = lay_out(n, Some(1.0), 0.05, &[], &mut r);
        assert_eq!(plan.positions, [0.45, 0.61, 1.0, 1.6]);
        assert_eq!(plan.main_world, Some(2));
        assert_eq!(r.remaining(), 0);
    }

    /// Example 12.2's orbits: no main world around an M4 V, whose innermost
    /// orbit is 0.14.
    #[test]
    fn example_12_2_orbits() {
        let mut r = Scripted::new(&[(D2, 5), (D2, 6), (D2, 8), (D2, 2)]);
        let n = number_of_orbits(0, &mut r);
        let plan = lay_out(n, None, 0.14, &[], &mut r);
        // 0.125 moves out to 0.14; ×1.90 = 0.27; ×1.25 = 0.34.
        assert_eq!(plan.positions, [0.14, 0.27, 0.34]);
        assert_eq!(r.remaining(), 0);
    }

    /// Noricum's orbits: seven known bodies, and the M6 companion's gap from
    /// 2 to 18 HD crossing out three positions.
    #[test]
    fn noricum_orbits() {
        let mut r = Scripted::new(&[
            (D2, 4), // 2D − 2 = 2, but 7 bodies are known
            (D1, 1), // a third of the other six inward
            (D2, 8), (D2, 9), // inward: ÷1.90, ÷2.05
            (D2, 4), (D2, 10), (D2, 7), (D2, 10), // outward: 2.0, then 4.5, 7.9, 18 crossed out
            (D2, 8), (D2, 10), (D2, 6), // 34, 76, 100
        ]);
        let n = number_of_orbits(7, &mut r);
        assert_eq!(n, 7);
        let gap = Gap { lo: 2.0, hi: 18.0 };
        let plan = lay_out(n, Some(1.4), 0.05, &[gap], &mut r);
        assert_eq!(plan.positions, [0.36, 0.74, 1.4, 2.0, 34.0, 76.0, 100.0]);
        assert_eq!(plan.crossed_out, [4.5, 7.9, 18.0]);
        assert_eq!(plan.shortfall, 0);
        assert_eq!(r.remaining(), 0);
    }
}
