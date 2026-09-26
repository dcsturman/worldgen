//! Stars: rulebook Sections 3.1 to 3.3 and 3.5, Tables 2, 3, 4, 7, 8 and 9.
//!
//! Rolling the stars a system doesn't list, and placing its companions.

use crate::callisto::dice::Roller;
use crate::callisto::orbits::Gap;
use crate::callisto::star::{WhiteDwarfAge, nearest_by_mass, star_data};
use crate::callisto::tables;
use crate::systems::system::{Star, StarSize, StarType};

/// A star as Callisto needs it: its classification plus a white dwarf's age.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CStar {
    pub star: Star,
    pub wd_age: WhiteDwarfAge,
}

impl CStar {
    pub fn data(&self) -> crate::callisto::star::StarData {
        star_data(&self.star, self.wd_age)
    }

    /// Can a main world orbit it? Not a white dwarf, a giant or a brown dwarf
    /// (Section 3.5, "Which star hosts the main world").
    pub fn can_host_main_world(&self) -> bool {
        matches!(self.star.size, StarSize::V | StarSize::VI | StarSize::IV)
    }
}

/// Number of stars (Table 2; DM −1 if the primary is spectral class M).
pub fn number_of_stars(primary: StarType, roller: &mut impl Roller) -> usize {
    let dm = if primary == StarType::M { -1 } else { 0 };
    let t = tables::table(2).dice().expect("Table 2 is a dice table");
    match t.lookup(roller.d2() + dm)[0].split(':').next().unwrap_or("") {
        "Two" => 2,
        "Three" => 3,
        _ => 1,
    }
}

/// The primary's spectral class (Table 3): the "habitable main world" column
/// for a system built around one.
pub fn primary_spectral(habitable: bool, roller: &mut impl Roller) -> (StarType, Option<u8>) {
    let t = tables::table(3).dice().expect("Table 3 is a dice table");
    let cell = &t.lookup(roller.d2())[if habitable { 0 } else { 1 }];
    match spectral_letter(cell) {
        Some(st) => (st, None),
        // "Rare: roll 1D. 1 to 4 A, 5 B, 6 O (treat O as B0)"
        None => match roller.d1() {
            1..=4 => (StarType::A, None),
            5 => (StarType::B, None),
            _ => (StarType::B, Some(0)),
        },
    }
}

fn spectral_letter(cell: &str) -> Option<StarType> {
    Some(match cell {
        "O" => StarType::O,
        "B" => StarType::B,
        "A" => StarType::A,
        "F" => StarType::F,
        "G" => StarType::G,
        "K" => StarType::K,
        "M" => StarType::M,
        _ => return None,
    })
}

/// Subtype: 2D − 2, a 10 reading 9.
pub fn subtype(roller: &mut impl Roller) -> u8 {
    (roller.d2() - 2).clamp(0, 9) as u8
}

/// The primary's luminosity class (Table 4). A habitable main world makes it
/// class V without a roll; O and B stars rolling 2 to 4 are supergiants.
pub fn primary_luminosity(
    spectral: StarType,
    habitable: bool,
    roller: &mut impl Roller,
) -> StarSize {
    if habitable {
        return StarSize::V;
    }
    let roll = roller.d2();
    if matches!(spectral, StarType::O | StarType::B) && roll <= 4 {
        return StarSize::Ia;
    }
    let t = tables::table(4).dice().expect("Table 4 is a dice table");
    match t.lookup(roll)[0].as_str() {
        "D" => StarSize::D,
        "III" => StarSize::III,
        "VI" => StarSize::VI,
        _ => StarSize::V,
    }
}

/// A white dwarf's age, rolled for every white dwarf (Table 6).
pub fn white_dwarf_age(size: StarSize, roller: &mut impl Roller) -> WhiteDwarfAge {
    if size == StarSize::D {
        WhiteDwarfAge::from_roll(roller.d1())
    } else {
        WhiteDwarfAge::Old
    }
}

/// A companion rolled from its mass (Table 7), as a fraction of the primary's.
pub fn companion_from_mass(primary_mass: f32, roller: &mut impl Roller) -> Star {
    let t = tables::table(7).dice().expect("Table 7 is a dice table");
    let cell = &t.lookup(roller.d2())[0];
    let dwarf = |size| Star {
        star_type: StarType::M,
        subtype: 9,
        size,
    };
    let Some(fraction) = leading_number(cell) else {
        // "Rare: roll 1D", 1 to 4 brown dwarf; 5 to 6 white dwarf (the
        // companion was once the bigger star).
        return if roller.d1() <= 4 {
            dwarf(StarSize::BD)
        } else {
            Star {
                star_type: StarType::G,
                subtype: 2,
                size: StarSize::D,
            }
        };
    };
    let mass = primary_mass * fraction;
    if mass < 0.08 {
        return dwarf(StarSize::BD);
    }
    let (star_type, subtype) = nearest_by_mass(mass);
    Star {
        star_type,
        subtype,
        size: StarSize::V,
    }
}

/// "0.017 (a contact pair…)" → 0.017; "2,000" → 2000; prose → None.
fn leading_number(cell: &str) -> Option<f32> {
    let digits: String = cell
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .filter(|c| *c != ',')
        .collect();
    digits.parse().ok()
}

/// One row of Table 8.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparationRow {
    /// 0 for the "2" row through 10 for the "12" row.
    pub index: usize,
    pub separation_hd: f32,
    /// Worlds orbit the primary alone out to this position.
    pub alone_hd: f32,
    /// Worlds orbit both stars beyond this position; infinite where nothing
    /// orbits both.
    pub both_hd: f32,
}

impl SeparationRow {
    pub fn gap(&self) -> Gap {
        Gap {
            lo: self.alone_hd,
            hi: self.both_hd,
        }
    }

    /// The contact pair of the "2" row: treated as one star for planets.
    pub fn is_contact(&self) -> bool {
        self.index == 0
    }
}

/// Table 8's rows, in order.
pub fn separation_rows() -> Vec<SeparationRow> {
    tables::table(8)
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| SeparationRow {
            index,
            separation_hd: leading_number(&row[1]).expect("Table 8 separation"),
            alone_hd: leading_number(&row[3]).expect("Table 8 alone-out-to"),
            both_hd: leading_number(&row[4]).unwrap_or(f32::INFINITY),
        })
        .collect()
}

/// Roll a companion's separation row (Table 8; DM +1 if the primary is
/// spectral class M).
pub fn roll_separation(primary: StarType, roller: &mut impl Roller) -> SeparationRow {
    let dm = if primary == StarType::M { 1 } else { 0 };
    let index = (roller.d2() + dm).clamp(2, 12) as usize - 2;
    separation_rows()[index]
}

/// How a companion was moved to keep a habitable main world's orbit stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Inward,
    Outward,
}

/// Section 3.5's habitable-zone check. If position 1.0 falls in the
/// companion's gap, roll 1D: 1 to 3 move it inward a row at a time until 1.0
/// is beyond "orbit both"; 4 to 6 outward until 1.0 is inside "primary
/// alone".
pub fn keep_habitable_zone_clear(
    row: SeparationRow,
    roller: &mut impl Roller,
) -> (SeparationRow, Option<Move>) {
    if !row.gap().contains(1.0) {
        return (row, None);
    }
    let rows = separation_rows();
    if roller.d1() <= 3 {
        let moved = rows[..row.index]
            .iter()
            .rev()
            .find(|r| r.both_hd < 1.0)
            .copied()
            .unwrap_or(rows[0]);
        (moved, Some(Move::Inward))
    } else {
        let moved = rows[row.index..]
            .iter()
            .find(|r| r.alone_hd > 1.0)
            .copied()
            .unwrap_or(rows[rows.len() - 1]);
        (moved, Some(Move::Outward))
    }
}

/// How a third star sits relative to the second (Table 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThirdStar {
    /// Both companions orbit the primary on their own.
    Independent,
    /// The two companions are a close pair orbiting each other at a tenth of
    /// the smaller separation; the pair orbits the primary at the larger.
    ClosePair,
}

pub fn third_star(second_hd: f32, third_hd: f32) -> ThirdStar {
    if third_hd < second_hd / 3.0 || third_hd > second_hd * 3.0 {
        ThirdStar::Independent
    } else {
        ThirdStar::ClosePair
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callisto::dice::{Kind::*, Scripted};

    #[test]
    fn table_8_reads_as_numbers() {
        let rows = separation_rows();
        assert_eq!(rows.len(), 11);
        assert_eq!((rows[0].separation_hd, rows[0].alone_hd), (0.05, 0.017));
        assert_eq!(rows[8].both_hd, 1_800.0);
        assert_eq!(rows[10].both_hd, f32::INFINITY);
    }

    #[test]
    fn a_gap_over_the_habitable_zone_moves_the_companion() {
        let rows = separation_rows();
        // Row "5" (2 HD): the gap 0.67 to 6 covers 1.0.
        let (moved, how) = keep_habitable_zone_clear(rows[3], &mut Scripted::new(&[(D1, 2)]));
        assert_eq!(how, Some(Move::Inward));
        assert_eq!(moved.separation_hd, 0.2); // both beyond 0.6
        let (moved, how) = keep_habitable_zone_clear(rows[3], &mut Scripted::new(&[(D1, 5)]));
        assert_eq!(how, Some(Move::Outward));
        assert_eq!(moved.separation_hd, 6.0); // alone out to 2
        // Row "6" (6 HD) is already clear.
        assert_eq!(keep_habitable_zone_clear(rows[4], &mut Scripted::new(&[])).1, None);
    }

    #[test]
    fn noricum_companions_are_independent() {
        // M9 at 200 HD, M6 at 6 HD: the M9 is beyond three times the M6.
        assert_eq!(third_star(6.0, 200.0), ThirdStar::Independent);
        assert_eq!(third_star(6.0, 10.0), ThirdStar::ClosePair);
    }

    #[test]
    fn companion_mass_picks_the_nearest_row() {
        // G2 V primary, 2D = 7: half its mass.
        let s = companion_from_mass(1.0, &mut Scripted::new(&[(D2, 7)]));
        assert_eq!((s.star_type, s.subtype, s.size), (StarType::M, 1, StarSize::V));
        // An M5 V (0.16) at a tenth is below 0.08: a brown dwarf.
        let bd = companion_from_mass(0.16, &mut Scripted::new(&[(D2, 11)]));
        assert_eq!(bd.size, StarSize::BD);
    }

    #[test]
    fn a_habitable_main_world_gets_a_main_sequence_primary() {
        let mut r = Scripted::new(&[(D2, 10)]);
        assert_eq!(primary_spectral(true, &mut r), (StarType::F, None));
        assert_eq!(primary_luminosity(StarType::F, true, &mut r), StarSize::V);
        assert_eq!(r.remaining(), 0);
    }
}
