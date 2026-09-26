//! Star data: rulebook Section 3.4, Tables 5 and 6.
//!
//! Table 5 (luminosity class V, B0 to M9) is read from the rulebook's data as
//! printed; every later step uses one or two of its columns. Table 6 changes a
//! class V row for the other luminosity classes; its cells are prose ("half",
//! "200 ×", "about 5,500 Mkm"), so its figures are constants here, and a test
//! checks each one still appears in the printed cell.
//!
//! Where Table 6 gives no figure for a column, the column comes from the
//! formula behind Table 5 (IMPLEMENTATION.md §2), so a giant's lock limit and
//! moon limit are worked out rather than invented.

use std::sync::LazyLock;

use crate::callisto::tables;
use crate::systems::system::{Star, StarSize, StarType};

/// One star's figures, as the rulebook's later steps use them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarData {
    /// Surface temperature in kelvin; sets the star's colour.
    pub temp_k: f32,
    /// Brightness, Sun = 1.
    pub luminosity: f32,
    /// Mass, Sun = 1.
    pub mass: f32,
    /// One habitable distance in Mkm: position × this = real distance.
    pub hd_mkm: f32,
    /// Days at thrust 1 to an orbit at position 1.0.
    pub days_to_hd: f32,
    /// The star's 100-diameter limit, Mkm.
    pub jump_shadow_mkm: f32,
    /// Closest position (HD) an orbit can have.
    pub innermost_hd: f32,
    /// Position (HD) inside which a world is tidally locked. Infinite where
    /// the table says "all": every orbit in the system is locked.
    pub lock_limit_hd: f32,
    /// The figure Table 30 turns into a widest moon orbit.
    pub moon_limit: f32,
}

/// A white dwarf's age, rolled once (Table 6: 1D, 1 to 2 young, 3 to 6 old).
/// It sets how bright the dwarf still is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteDwarfAge {
    Young,
    Old,
}

impl WhiteDwarfAge {
    pub fn from_roll(d1: i32) -> Self {
        if d1 <= 2 {
            WhiteDwarfAge::Young
        } else {
            WhiteDwarfAge::Old
        }
    }
}

/// Lock limits at or beyond this many HD print as "all".
const LOCK_ALL_HD: f32 = 3.0;

/// Days at thrust 1 for a distance in Mkm: accelerate to the midpoint, then
/// decelerate (rulebook Section 11.1).
pub fn days_at_thrust_1(mkm: f32) -> f32 {
    0.234 * mkm.sqrt()
}

/// The lock limit's formula: 0.4 × M^⅓ ÷ √L, in HD.
pub fn lock_limit_formula(mass: f32, luminosity: f32) -> f32 {
    0.4 * mass.cbrt() / luminosity.sqrt()
}

/// The moon limit's formula: 117 × √L ÷ M^⅓.
pub fn moon_limit_formula(mass: f32, luminosity: f32) -> f32 {
    117.0 * luminosity.sqrt() / mass.cbrt()
}

/// The innermost-orbit formula: a twentieth of the jump shadow, in HD, and
/// never inside 0.05.
pub fn innermost_formula(jump_shadow_mkm: f32, hd_mkm: f32) -> f32 {
    (jump_shadow_mkm / 20.0 / hd_mkm).max(0.05)
}

/// Table 5, keyed by spectral class and subtype.
static CLASS_V: LazyLock<Vec<(StarType, u8, StarData)>> = LazyLock::new(|| {
    tables::table(5)
        .rows
        .iter()
        .map(|row| {
            let (t, sub) = parse_star_name(&row[0])
                .unwrap_or_else(|| panic!("Table 5: can't read star {:?}", row[0]));
            let n = |i: usize| number(&row[i]);
            let lock = if row[8] == "all" { f32::INFINITY } else { n(8) };
            let data = StarData {
                temp_k: n(1),
                luminosity: n(2),
                mass: n(3),
                hd_mkm: n(4),
                days_to_hd: n(5),
                jump_shadow_mkm: n(6),
                innermost_hd: n(7),
                lock_limit_hd: lock,
                moon_limit: n(9),
            };
            (t, sub, data)
        })
        .collect()
});

/// A printed number: "21,624" → 21624.
fn number(cell: &str) -> f32 {
    cell.replace(',', "")
        .parse()
        .unwrap_or_else(|_| panic!("Callisto table: not a number: {cell:?}"))
}

/// "G2 V" → (G, 2).
fn parse_star_name(s: &str) -> Option<(StarType, u8)> {
    let mut chars = s.chars();
    let t = match chars.next()? {
        'O' => StarType::O,
        'B' => StarType::B,
        'A' => StarType::A,
        'F' => StarType::F,
        'G' => StarType::G,
        'K' => StarType::K,
        'M' => StarType::M,
        _ => return None,
    };
    let sub = chars.next()?.to_digit(10)? as u8;
    Some((t, sub))
}

/// Table 5's row for a class V star. O stars read B0, as the rulebook says
/// ("treat O as B0"); Table 5 begins there.
pub fn class_v(star_type: StarType, subtype: u8) -> StarData {
    if star_type == StarType::O {
        return class_v(StarType::B, 0);
    }
    CLASS_V
        .iter()
        .find(|(t, s, _)| *t == star_type && *s == subtype.min(9))
        .map(|(_, _, d)| *d)
        .unwrap_or_else(|| panic!("Table 5 has no {star_type:?}{subtype} V"))
}

/// The class V row whose mass is nearest `mass` (Section 3.5: a companion's
/// spectral class and subtype come from its mass).
pub fn nearest_by_mass(mass: f32) -> (StarType, u8) {
    CLASS_V
        .iter()
        .min_by(|a, b| (a.2.mass - mass).abs().total_cmp(&(b.2.mass - mass).abs()))
        .map(|(t, s, _)| (*t, *s))
        .expect("Table 5 is not empty")
}

/// Star data for any star, class V or not (Tables 5 and 6).
///
/// `wd_age` matters only for a white dwarf.
pub fn star_data(star: &Star, wd_age: WhiteDwarfAge) -> StarData {
    let v = class_v(star.star_type, star.subtype);
    match star.size {
        // IV (subgiant) is not in the rulebook. It is the short step between
        // the main sequence and a giant, and Travellermap lists a few; it is
        // read as class V until the rulebook says otherwise.
        StarSize::V | StarSize::IV => v,
        StarSize::VI => StarData {
            luminosity: v.luminosity * 0.5,
            mass: v.mass * 0.9,
            hd_mkm: v.hd_mkm * 0.7,
            days_to_hd: days_at_thrust_1(v.hd_mkm * 0.7),
            jump_shadow_mkm: v.jump_shadow_mkm * 0.8,
            // "innermost orbit and lock limit unchanged"
            ..v
        },
        StarSize::III => match star.star_type {
            StarType::K | StarType::M => {
                // Anything inside 400 Mkm has been engulfed.
                let mut d = scaled(v, 200.0, 1.5, 14.0, 40.0);
                d.innermost_hd = (400.0 / d.hd_mkm).max(d.innermost_hd);
                d
            }
            // Table 6 gives giants of classes K/M and G/F; hotter giants are
            // read with the G/F row, the nearer of the two.
            _ => scaled(v, 60.0, 2.0, 8.0, 15.0),
        },
        StarSize::II => scaled(v, 1_000.0, 5.0, 30.0, 60.0),
        StarSize::Ia | StarSize::Ib => scaled(v, 30_000.0, 15.0, 170.0, 300.0),
        StarSize::D => {
            let (luminosity, hd_mkm) = match wd_age {
                WhiteDwarfAge::Young => (0.05, 30.0),
                WhiteDwarfAge::Old => (0.001, 5.0),
            };
            let mass = 0.6;
            // Everything inside 300 Mkm is locked, and nothing survived there.
            let limit_hd = 300.0 / hd_mkm;
            StarData {
                // A white dwarf is white-hot whatever it once was.
                temp_k: 10_000.0,
                luminosity,
                mass,
                hd_mkm,
                days_to_hd: days_at_thrust_1(hd_mkm),
                jump_shadow_mkm: 1.8,
                innermost_hd: limit_hd.max(0.05),
                lock_limit_hd: limit_hd,
                moon_limit: moon_limit_formula(mass, luminosity),
            }
        }
        StarSize::BD => {
            let (luminosity, mass, hd_mkm, shadow) = (0.000_01, 0.05, 0.5, 14.0);
            StarData {
                temp_k: 1_000.0,
                luminosity,
                mass,
                hd_mkm,
                days_to_hd: days_at_thrust_1(hd_mkm),
                jump_shadow_mkm: shadow,
                innermost_hd: innermost_formula(shadow, hd_mkm),
                lock_limit_hd: lock_or_all(lock_limit_formula(mass, luminosity)),
                moon_limit: moon_limit_formula(mass, luminosity),
            }
        }
    }
}

/// A Table 6 giant row: light and distance scale the class V row, mass is
/// the stated figure, and the columns Table 6 doesn't give come from the
/// formulas.
fn scaled(v: StarData, light: f32, mass: f32, hd: f32, shadow: f32) -> StarData {
    let luminosity = v.luminosity * light;
    let hd_mkm = v.hd_mkm * hd;
    let jump_shadow_mkm = v.jump_shadow_mkm * shadow;
    StarData {
        temp_k: v.temp_k,
        luminosity,
        mass,
        hd_mkm,
        days_to_hd: days_at_thrust_1(hd_mkm),
        jump_shadow_mkm,
        innermost_hd: innermost_formula(jump_shadow_mkm, hd_mkm),
        lock_limit_hd: lock_or_all(lock_limit_formula(mass, luminosity)),
        moon_limit: moon_limit_formula(mass, luminosity),
    }
}

fn lock_or_all(hd: f32) -> f32 {
    if hd >= LOCK_ALL_HD { f32::INFINITY } else { hd }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn star(t: StarType, sub: u8, size: StarSize) -> Star {
        Star {
            star_type: t,
            subtype: sub,
            size,
        }
    }

    /// The range a printed figure stands for: "0.0072" is 0.00715 to
    /// 0.00725; "150" is 149.5 to 150.5; "21,624" is 21,623.5 to 21,624.5.
    fn printed_range(cell: &str) -> (f64, f64) {
        let clean = cell.replace(',', "");
        let v: f64 = clean.parse().unwrap();
        let decimals = clean.split_once('.').map_or(0, |(_, f)| f.len());
        let half = 0.5 * 10f64.powi(-(decimals as i32));
        (v - half, v + half)
    }

    /// The range `f` of the printed inputs covers as each input varies over
    /// its printed range. `f` must be monotonic in each input, so the
    /// corners of the input box bound it.
    fn corner_range(inputs: &[&str], f: impl Fn(&[f64]) -> f64) -> (f64, f64) {
        let ranges: Vec<(f64, f64)> = inputs.iter().map(|c| printed_range(c)).collect();
        (0..1u32 << ranges.len())
            .map(|mask| {
                let xs: Vec<f64> = ranges
                    .iter()
                    .enumerate()
                    .map(|(i, r)| if mask & (1 << i) == 0 { r.0 } else { r.1 })
                    .collect();
                f(&xs)
            })
            .fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
    }

    /// Does `f` of the printed inputs reach the printed output's range?
    fn consistent(inputs: &[&str], output: &str, f: impl Fn(&[f64]) -> f64) -> bool {
        let (lo, hi) = corner_range(inputs, f);
        let (olo, ohi) = printed_range(output);
        lo <= ohi && hi >= olo
    }

    /// IMPLEMENTATION.md §8: every computed column of Table 5 is its formula
    /// applied to the printed inputs, within the rounding of both.
    #[test]
    fn table_5_matches_its_formulas() {
        let mut bad = Vec::new();
        for row in &tables::table(5).rows {
            let (l, m, hd, days, shadow, inner, lock, moon) = (
                row[2].as_str(),
                row[3].as_str(),
                row[4].as_str(),
                row[5].as_str(),
                row[6].as_str(),
                row[7].as_str(),
                row[8].as_str(),
                row[9].as_str(),
            );
            let mut check = |col: &str, ok: bool| {
                if !ok {
                    bad.push(format!("{} {col}", row[0]));
                }
            };
            check("1 HD", consistent(&[l], hd, |x| 149.6 * x[0].sqrt()));
            check("days", consistent(&[hd], days, |x| 0.234 * x[0].sqrt()));
            check(
                "inner orbit",
                consistent(&[shadow, hd], inner, |x| (x[0] / 20.0 / x[1]).max(0.05)),
            );
            if lock == "all" {
                let (_, hi) = corner_range(&[m, l], |x| 0.4 * x[0].cbrt() / x[1].sqrt());
                check("lock (all)", hi >= f64::from(LOCK_ALL_HD));
            } else {
                check(
                    "lock",
                    consistent(&[m, l], lock, |x| 0.4 * x[0].cbrt() / x[1].sqrt()),
                );
            }
            check(
                "moon",
                consistent(&[m, l], moon, |x| 117.0 * x[1].sqrt() / x[0].cbrt()),
            );
        }
        assert!(bad.is_empty(), "Table 5 cells that don't match their formula: {bad:?}");
    }

    #[test]
    fn the_sun_reads_as_the_sun() {
        let g2 = class_v(StarType::G, 2);
        assert_eq!(g2.hd_mkm, 150.0);
        assert_eq!(g2.lock_limit_hd, 0.40);
        assert_eq!(g2.moon_limit, 117.0);
        assert_eq!(class_v(StarType::M, 5).lock_limit_hd, f32::INFINITY);
    }

    #[test]
    fn o_stars_read_b0() {
        assert_eq!(class_v(StarType::O, 5), class_v(StarType::B, 0));
    }

    #[test]
    fn nearest_by_mass_finds_the_row() {
        assert_eq!(nearest_by_mass(1.0), (StarType::G, 2));
        // Table 7 on a G2 primary, roll 7: half the mass.
        assert_eq!(nearest_by_mass(0.5), (StarType::M, 1));
    }

    /// Table 6's figures are code; each must still be printed in its row.
    #[test]
    fn table_6_constants_are_still_printed() {
        let t6 = tables::table(6);
        let row = |class: &str| {
            t6.rows
                .iter()
                .find(|r| r[0].starts_with(class))
                .unwrap_or_else(|| panic!("Table 6 has no {class} row"))
                .join(" | ")
        };
        let expect = [
            ("VI subdwarf", &["half", "0.9 ×", "0.7 ×", "0.8 ×"][..]),
            ("III giant, spectral class K or M", &["200 ×", "1.5", "14 ×", "40 ×", "400 Mkm"]),
            ("III giant, spectral class G or F", &["60 ×", "2", "8 ×", "15 ×"]),
            ("II bright giant", &["1,000 ×", "5", "30 ×", "60 ×"]),
            ("I supergiant", &["30,000 ×", "15", "170 ×", "300 ×"]),
            ("D white dwarf", &["0.001", "0.05", "0.6", "5 Mkm", "30 Mkm", "1.8 Mkm", "300 Mkm", "1 to 2 young"]),
            ("BD brown dwarf", &["0.00001", "0.05", "0.5 Mkm", "14 Mkm"]),
        ];
        for (class, figures) in expect {
            let printed = row(class);
            for f in figures {
                assert!(printed.contains(f), "Table 6 {class}: {f:?} not in {printed:?}");
            }
        }
    }

    #[test]
    fn a_subdwarf_is_dimmer_but_keeps_its_limits() {
        let v = class_v(StarType::K, 5);
        let vi = star_data(&star(StarType::K, 5, StarSize::VI), WhiteDwarfAge::Old);
        assert_eq!(vi.luminosity, v.luminosity * 0.5);
        assert_eq!(vi.hd_mkm, v.hd_mkm * 0.7);
        assert_eq!(vi.innermost_hd, v.innermost_hd);
        assert_eq!(vi.lock_limit_hd, v.lock_limit_hd);
    }

    #[test]
    fn a_red_giant_engulfs_its_inner_system() {
        let g = star_data(&star(StarType::K, 0, StarSize::III), WhiteDwarfAge::Old);
        assert_eq!(g.hd_mkm, class_v(StarType::K, 0).hd_mkm * 14.0);
        assert!((g.innermost_hd * g.hd_mkm - 400.0).abs() < 0.01);
    }

    #[test]
    fn a_white_dwarf_depends_on_its_age() {
        let wd = star(StarType::G, 2, StarSize::D);
        let young = star_data(&wd, WhiteDwarfAge::Young);
        let old = star_data(&wd, WhiteDwarfAge::Old);
        assert_eq!((young.hd_mkm, old.hd_mkm), (30.0, 5.0));
        // Nothing orbits inside 300 Mkm.
        assert_eq!(old.innermost_hd * old.hd_mkm, 300.0);
        assert_eq!(WhiteDwarfAge::from_roll(2), WhiteDwarfAge::Young);
        assert_eq!(WhiteDwarfAge::from_roll(3), WhiteDwarfAge::Old);
    }
}
