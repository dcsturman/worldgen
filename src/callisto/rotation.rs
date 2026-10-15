//! Rotation, tilt and eccentricity: rulebook Section 10, Tables 32 to 35.

use serde::{Deserialize, Serialize};

use crate::callisto::dice::Roller;
use crate::callisto::tables;
use crate::callisto::temperature::TempBand;

/// How a body turns (Section 10.2, and 9.4 for moons).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Rotation {
    /// A free rotator: its day in hours, and whether it turns backwards.
    Free { hours: f32, retrograde: bool },
    /// One face always toward its star; its day is its year.
    StarLocked,
    /// Three rotations to two orbits, like Mercury: a day of two years and
    /// no permanent hot side.
    Resonance32,
    /// A moon: one face toward its planet, its day its month.
    PlanetLocked { hours: f32 },
}

impl Rotation {
    pub fn name(self) -> &'static str {
        match self {
            Rotation::Free { .. } => "free",
            Rotation::StarLocked => "star-locked",
            Rotation::Resonance32 => "3:2 resonance",
            Rotation::PlanetLocked { .. } => "planet-locked",
        }
    }

    /// Its day in hours, given its year in days.
    pub fn day_hours(self, year_days: f32) -> f32 {
        match self {
            Rotation::Free { hours, .. } | Rotation::PlanetLocked { hours } => hours,
            Rotation::StarLocked => year_days * 24.0,
            Rotation::Resonance32 => year_days * 2.0 * 24.0,
        }
    }
}

/// Eccentricity (Table 32): 2D, DM −2 at or below the lock limit (tides
/// circularise orbits), +1 with a companion inside 20 HD.
pub fn eccentricity(inside_lock_limit: bool, close_companion: bool, roller: &mut impl Roller) -> f32 {
    let dm = if inside_lock_limit { -2 } else { 0 } + i32::from(close_companion);
    let t = tables::table(32).dice().expect("Table 32 is a dice table");
    t.lookup(roller.d2() + dm)[0]
        .parse()
        .expect("Table 32 eccentricities are numbers")
}

/// Rotation (Section 10.2, Table 33): locked inside the lock limit, or in a
/// 3:2 resonance there if the eccentricity is 0.15 or more; otherwise free,
/// its day from Table 33 (DM +2 with a large moon).
pub fn rotation(
    inside_lock_limit: bool,
    eccentricity: f32,
    large_moon: bool,
    roller: &mut impl Roller,
) -> Rotation {
    if inside_lock_limit {
        return if eccentricity >= 0.15 {
            Rotation::Resonance32
        } else {
            Rotation::StarLocked
        };
    }
    free_day(large_moon, roller)
}

/// A free rotator's day (Table 33). The rare 12: 1D × 100 hours, retrograde
/// on 1D 1 to 2, like Venus.
pub fn free_day(large_moon: bool, roller: &mut impl Roller) -> Rotation {
    let t = tables::table(33).dice().expect("Table 33 is a dice table");
    let cell = &t.lookup(roller.d2() + if large_moon { 2 } else { 0 })[0];
    match cell.parse::<f32>() {
        Ok(hours) => Rotation::Free {
            hours,
            retrograde: false,
        },
        Err(_) => {
            let hours = roller.d1() as f32 * 100.0;
            Rotation::Free {
                hours,
                retrograde: roller.d1() <= 2,
            }
        }
    }
}

/// Axial tilt in degrees (Table 34): 2D, DM −2 with a large moon and −2 at
/// or below the lock limit.
pub fn tilt(large_moon: bool, inside_lock_limit: bool, roller: &mut impl Roller) -> f32 {
    let dm = if large_moon { -2 } else { 0 } + if inside_lock_limit { -2 } else { 0 };
    let t = tables::table(34).dice().expect("Table 34 is a dice table");
    let cell = &t.lookup(roller.d2() + dm)[0];
    cell.chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .expect("Table 34 tilts are degrees")
}

/// The year in Earth days (Section 10.4): 0.2 × d^1.5 ÷ √M, d in Mkm.
pub fn year_days(distance_mkm: f32, star_mass: f32) -> f32 {
    0.2 * distance_mkm.powf(1.5) / star_mass.sqrt()
}

/// A world's two seasons, if its eccentricity makes the near and far ends of
/// its year different bands (Section 10.1): read at position × (1 − e) and
/// × (1 + e), for eccentricity 0.2 or more.
pub fn two_season(
    eccentricity: f32,
    band_at: impl Fn(f32) -> TempBand,
    position_hd: f32,
) -> Option<(TempBand, TempBand)> {
    if eccentricity < 0.2 {
        return None;
    }
    let near = band_at(position_hd * (1.0 - eccentricity));
    let far = band_at(position_hd * (1.0 + eccentricity));
    (near != far).then_some((near, far))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callisto::dice::{Kind::*, Scripted};

    /// IMPLEMENTATION.md §8: Table 35 is its formula.
    #[test]
    fn table_35_is_its_formula() {
        let days = |cell: &str| -> f32 {
            let n: f32 = cell.split_whitespace().next().unwrap().parse().unwrap();
            if cell.contains("year") { n * 365.25 } else { n }
        };
        for row in &tables::table(35).rows {
            let d: f32 = row[0].replace(',', "").parse().unwrap();
            for (col, mass) in [(1, 1.0), (2, 0.3)] {
                let printed = days(&row[col]);
                let formula = year_days(d, mass);
                assert!(
                    (printed - formula).abs() / formula < 0.05,
                    "{d} Mkm, mass {mass}: printed {printed:.1} d, formula {formula:.1} d"
                );
            }
        }
    }

    /// Noricum (13.3): day 2D = 4 + 2 for its large moon, 14 hours; tilt
    /// 2D = 6 − 2, 6°; eccentricity 2D = 8, 0.10; year 604 days at 209 Mkm.
    #[test]
    fn noricum_in_full() {
        let mut r = Scripted::new(&[(D2, 8), (D2, 4), (D2, 6)]);
        let e = eccentricity(false, false, &mut r);
        assert_eq!(e, 0.10);
        assert_eq!(
            rotation(false, e, true, &mut r),
            Rotation::Free {
                hours: 14.0,
                retrograde: false
            }
        );
        assert_eq!(tilt(true, false, &mut r), 6.0);
        assert!((year_days(209.0, 1.0) - 604.0).abs() < 1.0);
    }

    #[test]
    fn inside_the_lock_limit() {
        let mut r = Scripted::new(&[]);
        assert_eq!(rotation(true, 0.05, false, &mut r), Rotation::StarLocked);
        assert_eq!(rotation(true, 0.15, false, &mut r), Rotation::Resonance32);
        assert_eq!(Rotation::Resonance32.day_hours(10.0), 480.0);
    }

    #[test]
    fn the_rare_long_day() {
        let mut r = Scripted::new(&[(D2, 12), (D1, 3), (D1, 1)]);
        assert_eq!(
            free_day(false, &mut r),
            Rotation::Free {
                hours: 300.0,
                retrograde: true
            }
        );
    }

    #[test]
    fn two_seasons_only_when_the_bands_differ() {
        use crate::callisto::temperature::temperature;
        let band = |p: f32| temperature(p, 6, 7, false).band;
        assert_eq!(two_season(0.1, band, 1.0), None);
        let two = two_season(0.3, band, 1.0).unwrap();
        assert!(two.0 > two.1, "near end warmer: {two:?}");
    }
}
