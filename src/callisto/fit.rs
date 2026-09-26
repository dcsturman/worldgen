//! How well a world's published facts and the physics agree
//! (IMPLEMENTATION.md §7; rulebook Section 13.2).
//!
//! Published data always wins: nothing here changes a world. It says what
//! the physics would have said, so the oddity is on the record rather than
//! buried, and [`crate::callisto::body::Fit::Strained`] worlds are what the
//! review file lists.

use crate::callisto::body::Fit;
use crate::callisto::orbits::Zone;
use crate::callisto::temperature::{TempBand, Temperature};

/// What a published UWP asks that the world-building rolls (Section 7)
/// could not have produced, each with the story that makes it work. Section
/// 13.2 calls these oddities with easy stories: on the record, not strained.
pub fn uwp_oddities(size: i32, atmosphere: i32, hydro: i32, zone: Zone) -> Vec<String> {
    let mut out = Vec::new();
    if size <= 1 && atmosphere > 0 {
        out.push(format!(
            "Size {size} with atmosphere {atmosphere}: a world this small cannot hold air, so \
             it is a thin, slowly escaping envelope renewed from the interior"
        ));
    } else if atmosphere <= 9 && atmosphere > size + 6 {
        // Section 7.3 tops out at 2D − 7 + size, +1 for iron: size + 6.
        out.push(format!(
            "Size {size} with atmosphere {atmosphere}: a small world with a thick atmosphere, \
             cold and dense with heavy gases"
        ));
    }
    if atmosphere <= 1 && hydro >= 1 && matches!(zone, Zone::Inner | Zone::Hot) {
        out.push(format!(
            "Hydrographics {hydro} on an airless world in the {} zone: the water is ice in \
             permanently shadowed craters",
            zone.name()
        ));
    }
    out
}

/// A habitable main world (atmosphere 4–9, hydrographics 1+) that its
/// position leaves Frozen or Roasting.
pub fn habitability_strain(atmosphere: i32, hydro: i32, t: &Temperature) -> Option<String> {
    let habitable = (4..=9).contains(&atmosphere) && hydro >= 1;
    (habitable && matches!(t.band, TempBand::Frozen | TempBand::Roasting)).then(|| {
        format!(
            "Breathable air and water, but its orbit makes it {} at {:.0} °C: people live where \
             local conditions allow, in {}",
            t.band.name(),
            t.celsius,
            if t.band == TempBand::Frozen {
                "geothermal valleys and under domes"
            } else {
                "the polar regions"
            }
        )
    })
}

/// The fit from what was strained and what was adjusted.
pub fn assess(strains: Vec<String>, adjustments: Vec<(String, String)>) -> Fit {
    if !strains.is_empty() {
        return Fit::Strained {
            story: strains.join("; "),
        };
    }
    match adjustments.into_iter().next() {
        Some((what, why)) => Fit::Adjusted { what, why },
        None => Fit::Tuned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callisto::temperature::temperature;

    #[test]
    fn ordinary_worlds_are_not_odd() {
        assert!(uwp_oddities(8, 6, 7, Zone::Temperate).is_empty());
        // Size 3, atmosphere 9: 12 − 7 + 3 + 1 for iron = 9, possible.
        assert!(uwp_oddities(3, 9, 0, Zone::Temperate).is_empty());
        // Airless with ice, out in the Cold zone: that is just ice.
        assert!(uwp_oddities(4, 0, 3, Zone::Cold).is_empty());
    }

    #[test]
    fn impossible_codes_are_odd() {
        assert_eq!(uwp_oddities(1, 4, 0, Zone::Temperate).len(), 1);
        assert_eq!(uwp_oddities(2, 9, 0, Zone::Temperate).len(), 1);
        assert_eq!(uwp_oddities(5, 0, 4, Zone::Inner).len(), 1);
    }

    #[test]
    fn a_habitable_world_left_frozen_is_strained() {
        let frozen = temperature(3.0, 6, 7, false);
        assert!(habitability_strain(6, 7, &frozen).is_some());
        let fine = temperature(1.0, 6, 7, false);
        assert!(habitability_strain(6, 7, &fine).is_none());
    }
}
