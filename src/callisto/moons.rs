//! Satellites and rings: rulebook Section 9, Tables 28 to 31.
//!
//! Moons orbit in planetary radii. Every world, sub-Neptune, icy dwarf and
//! giant rolls for them; the star only enters through the stability limit
//! (Table 30). Rolls live here; [`crate::callisto::populate`] runs them for
//! each body, and the generator turns them into satellite `World`s.

use serde::{Deserialize, Serialize};

use crate::callisto::body::{Composition, GiantKind};
use crate::callisto::dice::Roller;
use crate::callisto::fill;
use crate::callisto::orbits::Zone;
use crate::callisto::tables;

/// Table 29's bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoonBand {
    Close,
    Far,
    Extreme,
}

/// What a moon orbits, as Tables 28 and 30 and Section 9.2 sort them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Parent {
    /// A terrestrial world or an icy dwarf, by size, and its surface
    /// gravity for the moon's period.
    World { size: i32, gravity: f32 },
    SubNeptune { size: i32 },
    Giant(GiantKind),
}

impl Parent {
    fn is_giant(self) -> bool {
        matches!(self, Parent::Giant(_))
    }

    /// Section 9.4's multiplier on Table 31's period: √(size ÷ 8 ÷ gravity),
    /// about 2 for a gas giant, 1.6 for an ice giant, and a sub-Neptune's
    /// surface is taken at 1.2 g.
    pub fn period_factor(self) -> f32 {
        match self {
            Parent::World { size, gravity } => (size.max(1) as f32 / 8.0 / gravity.max(0.05)).sqrt(),
            Parent::SubNeptune { size } => (size as f32 / 8.0 / 1.2).sqrt(),
            Parent::Giant(GiantKind::IceGiant) => 1.6,
            Parent::Giant(_) => 2.0,
        }
    }
}

/// One moon or ring as rolled.
#[derive(Debug, Clone, PartialEq)]
pub struct Moon {
    /// Size code: −1 for S (under 1,600 km), 0 for a ring (R).
    pub size: i32,
    /// Distance from the planet's centre in its radii; a ring's outer edge.
    pub radii: f32,
    /// `None` for a ring.
    pub band: Option<MoonBand>,
    /// A ring's inner and outer edge, radii.
    pub ring: Option<(f32, f32)>,
    /// The "6" on Section 9.2's size roll: a quarter to a third of a world.
    pub large: bool,
    /// World codes for a moon of size 1 or more (Section 8, composition DM
    /// +1); `None` for S and rings, which are airless.
    pub codes: Option<fill::Codes>,
}

impl Moon {
    pub fn is_ring(&self) -> bool {
        self.ring.is_some()
    }
}

/// How many moons, and whether there is a ring (Table 28).
pub fn count_and_ring(parent: Parent, roller: &mut impl Roller) -> (usize, Option<(f32, f32)>) {
    let (moons, ring) = match parent {
        Parent::World { size, .. } if size >= 3 => {
            ((roller.d1() - 3).max(0), roller.d2() == 12)
        }
        Parent::World { .. } => ((roller.d1() - 4).max(0), false),
        Parent::SubNeptune { .. } => ((roller.d1() - 2).max(0), roller.d1() == 6),
        Parent::Giant(GiantKind::IceGiant) => (roller.d1() + 1, roller.d1() <= 3),
        Parent::Giant(_) => (roller.d2(), roller.d1() <= 4),
    };
    // A ring spans 1.5 to 2 radii on 1D 1 to 3, else 2 to 3.
    let ring = ring.then(|| if roller.d1() <= 3 { (1.5, 2.0) } else { (2.0, 3.0) });
    (moons as usize, ring)
}

/// A moon's size (Section 9.2), and whether it is a large moon. `first_of_
/// temperate_giant` is the giant-in-the-habitable-zone case: 2D − 3, at
/// least 1.
pub fn moon_size(parent: Parent, first_of_temperate_giant: bool, roller: &mut impl Roller) -> (i32, bool) {
    match parent {
        Parent::Giant(_) if first_of_temperate_giant => ((roller.d2() - 3).max(1), false),
        Parent::Giant(_) => {
            let s = roller.d2() - 6;
            (if s >= 1 { s } else { -1 }, false)
        }
        Parent::World { size, .. } | Parent::SubNeptune { size } => match roller.d1() {
            1..=3 => (-1, false),
            4 | 5 => (if size <= 3 { -1 } else { 1 }, false),
            _ => {
                let div = if matches!(parent, Parent::SubNeptune { .. }) { 5 } else { 3 };
                ((size / div).max(1), true)
            }
        },
    }
}

/// A moon's band and distance in radii (Table 29).
pub fn moon_orbit(roller: &mut impl Roller) -> (MoonBand, f32) {
    match roller.d1() {
        1..=3 => (MoonBand::Close, (roller.d2() + 1) as f32),
        4 | 5 => (MoonBand::Far, ((roller.d2() - 2) * 5 + 15) as f32),
        _ => (MoonBand::Extreme, ((roller.d2() - 2) * 25 + 75) as f32),
    }
}

/// Table 30's widest moon orbit for a planet at `position_hd` around a star
/// whose moon limit is `moon_limit`: `Some(Some(radii))`, `Some(None)` for
/// "any", or `None` for "none" (the planet keeps no moons). Read from the
/// table: the nearest row and column, a giant one row up.
pub fn widest_orbit(moon_limit: f32, position_hd: f32, giant: bool) -> Option<Option<f32>> {
    let t = tables::table(30);
    let nearest = |values: &[f32], x: f32| {
        values
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 / x).ln().abs().total_cmp(&(b.1 / x).ln().abs()))
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    let row_limits: Vec<f32> = t.rows.iter().map(|r| leading(&r[0])).collect();
    let cols: Vec<f32> = t.header[1..].iter().map(|h| leading(h)).collect();
    let mut row = nearest(&row_limits, moon_limit.max(0.1));
    if giant {
        row = row.saturating_sub(1);
    }
    let col = nearest(&cols, position_hd.max(0.01));
    match t.rows[row][col + 1].as_str() {
        "none" => None,
        "any" => Some(None),
        n => Some(Some(n.parse().expect("Table 30 cells are numbers"))),
    }
}

/// "200 or more" → 200; "16+" → 16.
fn leading(s: &str) -> f32 {
    s.chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .parse()
        .expect("Table 30 headings are numbers")
}

/// A moon's orbital period in hours (Table 31's formula): 1.41 × r^1.5 for
/// a size 8, 1 g parent, times the parent's factor.
pub fn period_hours(radii: f32, parent: Parent) -> f32 {
    1.41 * radii.powf(1.5) * parent.period_factor()
}

/// Roll every moon and ring of one body (Section 9.1 to 9.3). `count` is a
/// published number of moons (named ones already taken off), used instead of
/// Table 28's roll; the ring is rolled either way.
pub fn roll_moons(
    parent: Parent,
    zone: Zone,
    position_hd: f32,
    moon_limit: f32,
    count: Option<usize>,
    roller: &mut impl Roller,
) -> Vec<Moon> {
    let (rolled, ring) = count_and_ring(parent, roller);
    let n = count.unwrap_or(rolled);
    let temperate_giant = parent.is_giant() && zone == Zone::Temperate;
    let mut moons: Vec<Moon> = Vec::new();
    for i in 0..n {
        let (size, large) = moon_size(parent, temperate_giant && i == 0, roller);
        let (band, mut radii) = moon_orbit(roller);
        // A Close moon inside the ring's outer edge moves just outside it.
        if let Some((_, outer)) = ring
            && band == MoonBand::Close
            && radii < outer
        {
            radii = outer + 1.0;
        }
        while moons.iter().any(|m| m.radii == radii) {
            radii += roller.d1() as f32;
        }
        let codes = (size >= 1).then(|| moon_codes(size, zone, roller));
        moons.push(Moon {
            size,
            radii,
            band: Some(band),
            ring: None,
            large,
            codes,
        });
    }
    // The stability limit: nothing beyond it, and "none" keeps no moons.
    match widest_orbit(moon_limit, position_hd, parent.is_giant()) {
        None => moons.clear(),
        Some(Some(cap)) => {
            for m in moons.iter_mut().filter(|m| m.radii > cap) {
                m.radii = cap;
            }
            // Two moons can't share a distance: step later ones inward.
            let mut taken: Vec<f32> = Vec::new();
            moons.retain_mut(|m| {
                while taken.contains(&m.radii) && m.radii > 3.0 {
                    m.radii -= 1.0;
                }
                if taken.contains(&m.radii) {
                    return false;
                }
                taken.push(m.radii);
                true
            });
        }
        Some(None) => {}
    }
    if let Some((inner, outer)) = ring {
        moons.push(Moon {
            size: 0,
            radii: outer,
            band: None,
            ring: Some((inner, outer)),
            large: false,
            codes: None,
        });
    }
    moons.sort_by(|a, b| a.radii.total_cmp(&b.radii));
    moons
}

/// A moon of size 1 or more as a world (Section 8) at its planet's
/// position: composition with DM +1 (moons are icy), then atmosphere and
/// hydrographics.
fn moon_codes(size: i32, zone: Zone, roller: &mut impl Roller) -> fill::Codes {
    let composition: Composition = fill::composition(zone, 1, roller);
    let atmosphere = fill::atmosphere(size, composition, zone, roller);
    let hydro = fill::hydrographics(size, atmosphere, zone, roller);
    fill::Codes {
        size,
        atmosphere,
        hydro,
        composition: Some(composition),
        gravity: Some(fill::gravity(size, composition)),
        hydro_is_ice: matches!(zone, Zone::Cold | Zone::Outer) && hydro >= 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callisto::dice::{Kind::*, Scripted};

    /// Table 30 is the star's moon limit times the position, "none" below 3
    /// radii and "any" beyond the widest band (325).
    #[test]
    fn table_30_is_its_formula() {
        let t = tables::table(30);
        for row in &t.rows {
            let limit = leading(&row[0]);
            for (i, h) in t.header[1..].iter().enumerate() {
                let x = limit * leading(h);
                let cell = row[i + 1].as_str();
                match cell {
                    "none" => assert!(x < 3.0, "{limit} × {h}: none but {x}"),
                    "any" => assert!(x > 325.0, "{limit} × {h}: any but {x}"),
                    n => {
                        let n: f32 = n.parse().unwrap();
                        assert!((n - x).abs() <= 1.0, "{limit} × {h}: printed {n}, formula {x}");
                    }
                }
            }
        }
    }

    /// Table 31 is 1.41 × r^1.5 hours for a size 8, 1 g parent.
    #[test]
    fn table_31_is_its_formula() {
        let earth = Parent::World { size: 8, gravity: 1.0 };
        let hours = |cell: &str| -> f32 {
            let n: f32 = cell.split_whitespace().next().unwrap().parse().unwrap();
            if cell.contains("day") && !cell.contains(" h") { n * 24.0 } else { n }
        };
        for row in &tables::table(31).rows {
            for pair in row.chunks(2) {
                if pair[0].is_empty() {
                    continue;
                }
                let r: f32 = pair[0].parse().unwrap();
                let printed = hours(&pair[1]);
                let formula = period_hours(r, earth);
                assert!(
                    (printed - formula).abs() / formula < 0.05,
                    "{r} radii: printed {printed} h, formula {formula:.1} h"
                );
            }
        }
    }

    #[test]
    fn table_28_formulas_are_still_printed() {
        let rows: Vec<String> = tables::table(28).rows.iter().map(|r| r.join(" | ")).collect();
        let expect = [
            "World, size 3 or more | 1D − 3 (minimum 0) | 2D roll of 12",
            "World, size 1 or 2 | 1D − 4 (minimum 0) | none",
            "Sub-Neptune | 1D − 2 (minimum 0) | 1D roll of 6",
            "Ice giant | 1D + 1 | 1D roll of 1 to 3",
            "Gas giant | 2D | 1D roll of 1 to 4",
        ];
        assert_eq!(rows, expect);
    }

    /// Noricum (13.3): 1D = 4, one moon; no ring; size 1D = 6, a large moon
    /// of size 2; Close, 2D = 3, 4 radii; the limit at 1.4 HD (117 × 1.4)
    /// keeps it. As a size 2 world it then rolls composition, atmosphere and
    /// hydrographics: airless and dry.
    #[test]
    fn noricums_moon() {
        let mut r = Scripted::new(&[
            (D1, 4), (D2, 5), // one moon, no ring
            (D1, 6), (D1, 2), (D2, 3), // large, size 2; Close, 4 radii
            (D1, 3), (D2, 7), (D2, 7), // composition, atmosphere, hydrographics
        ]);
        let parent = Parent::World { size: 8, gravity: 1.0 };
        let moons = roll_moons(parent, Zone::Temperate, 1.4, 117.0, None, &mut r);
        assert_eq!(r.remaining(), 0);
        assert_eq!(moons.len(), 1);
        let m = &moons[0];
        assert_eq!((m.size, m.large, m.band, m.radii), (2, true, Some(MoonBand::Close), 4.0));
        let c = m.codes.unwrap();
        assert_eq!((c.atmosphere, c.hydro), (0, 0));
    }

    #[test]
    fn a_red_dwarf_habitable_zone_keeps_its_moons_close() {
        // An M4 V's moon limit is 16; at 1 HD the widest orbit is 13 radii
        // (row 13), a giant's 8.
        assert_eq!(widest_orbit(16.0, 1.0, false), Some(Some(13.0)));
        assert_eq!(widest_orbit(16.0, 1.0, true), Some(Some(8.0)));
        assert_eq!(widest_orbit(16.0, 0.1, false), None);
        assert_eq!(widest_orbit(117.0, 16.0, false), Some(None));
    }

    #[test]
    fn periods() {
        // Section 9.4: a large moon of a giant at 8 radii has about a
        // 64-hour day; at 20 radii about ten days.
        let gg = Parent::Giant(GiantKind::JupiterClass);
        assert!((period_hours(8.0, gg) - 64.0).abs() < 1.0);
        assert!((period_hours(20.0, gg) / 24.0 - 10.5).abs() < 0.5);
    }
}
