//! # System Generation Tables Module
//!
//! This module provides lookup tables and utility functions for stellar zone boundaries,
//! luminosity data, and environmental calculations used in Traveller solar system generation.
//! It implements the astronomical rules for determining habitable zones, temperature zones,
//! and stellar characteristics based on star type, size, and subtype.
//!
//! ## Key Features
//!
//! - **Zone Calculations**: Determines orbital zone boundaries for different star types
//! - **Stellar Data**: Luminosity and mass lookup tables for all star classifications
//! - **Environmental Tables**: Atmospheric and temperature calculation constants
//! - **Orbital Mechanics**: Distance calculations for planetary orbits
//!
//! ## Zone System
//!
//! The module defines five orbital zones around stars:
//! - **Inside Zone**: Extremely close orbits, typically uninhabitable
//! - **Hot Zone**: High temperature region, limited habitability  
//! - **Inner Zone**: Warm region, some habitability potential
//! - **Habitable Zone**: Optimal temperature range for life
//! - **Outer Zone**: Cold region, limited habitability
//!
//! ## Usage
//!
//! ```rust,ignore
//! use worldgen::systems::system_tables::{get_zone, get_luminosity};
//!
//! let zones = get_zone(&star);
//! let luminosity = get_luminosity(&star);
//! ```
//!
//! Example of using `get_zone` with a `System`:
//!
//! ```rust,ignore
//! use worldgen::systems::system_tables::get_zone;
//! use worldgen::systems::system::System;
//!
//! let system = System::default();
//! let zone_table = get_zone(&system.star);
//! println!("Habitable zone: {}", zone_table.habitable);
//! ```

use crate::systems::system::{Star, StarSize, StarSubType, StarType};
use crate::util::roll_2d6;
use lazy_static::lazy_static;
use log::debug;
use std::collections::HashMap;

/// Retrieves orbital zone boundaries for a given star
///
/// Looks up pre-calculated zone boundaries based on the star's classification.
/// The zones determine where different types of worlds can exist and their
/// environmental characteristics.
///
/// # Arguments
///
/// * `star` - Star to calculate zones for
///
/// # Returns
///
/// `ZoneTable` containing zone boundaries in orbital positions
///
/// # Examples
///
/// ```rust
/// # use worldgen::systems::system_tables::get_zone;
/// # use worldgen::systems::system::{Star, StarType, StarSize, StarSubType};
/// # let orbit = 3;
/// let star = Star {
///     star_type: StarType::G,
///     size: StarSize::V,
///     subtype: 2,
/// };
/// let zones = get_zone(&star);
/// if orbit <= zones.habitable {
///     // World is in habitable zone
/// }
/// ```
///
/// Example of using `get_zone` with a `Star`:
///
/// ```rust,ignore
/// use worldgen::systems::system_tables::get_zone;
/// use worldgen::systems::system::{System, Star, StarType, StarSize};
///
/// let star = Star {
///     star_type: StarType::G,
///     size: StarSize::V,
///     subtype: 2,
/// };
/// let zones = get_zone(&star);
/// let orbit = 3;
/// if orbit <= zones.habitable {
///     println!("Orbit {} is in the habitable zone", orbit);
/// }
/// ```
pub fn get_zone(star: &Star) -> ZoneTable {
    debug!(
        "get_zone: {:?} as {:?}",
        star,
        ZONE_TABLE.get(&(star.size, star.star_type, round_subtype(star.subtype)))
    );
    *ZONE_TABLE
        .get(&(star.size, star.star_type, round_subtype(star.subtype)))
        .unwrap()
}

/// Determines the habitable zone boundary for a star
///
/// Returns the orbital position of the habitable zone, or -1 if the star
/// has no habitable zone (habitable zone overlaps with inner zone).
///
/// # Arguments
///
/// * `star` - Star to check for habitable zone
///
/// # Returns
///
/// Orbital position of habitable zone, or -1 if none exists
pub fn get_habitable(star: &Star) -> i32 {
    let habitable = get_zone(star).habitable;
    if habitable > get_zone(star).inner {
        habitable
    } else {
        -1
    }
}

/// Rounds star subtype to nearest table entry
///
/// Stellar subtypes 0-9 are grouped into early (0-4) and late (5-9) variants
/// for table lookup purposes. This simplifies the lookup tables while
/// maintaining sufficient granularity.
///
/// # Arguments
///
/// * `subtype` - Raw subtype value (0-9)
///
/// # Returns
///
/// Rounded subtype (0 or 5)
///
/// # Panics
///
/// Panics if subtype is outside valid range (0-9)
pub fn round_subtype(subtype: StarSubType) -> u8 {
    match subtype {
        0..=4 => 0,
        5..=9 => 5,
        _ => panic!("Invalid subtype"),
    }
}

/// Retrieves stellar luminosity for a given star
///
/// Looks up luminosity from comprehensive tables based on star classification.
/// Luminosity affects zone boundaries and world temperature calculations.
///
/// # Arguments
///
/// * `star` - Star to look up luminosity for
///
/// # Returns
///
/// Stellar luminosity as multiple of Sol's luminosity
pub(crate) fn get_luminosity(star: &Star) -> f32 {
    interpolate_subtype(&LUMINOSITY_TABLE, star, m9_anchor(star.size).map(|(l, _)| l))
}

/// Retrieves stellar mass for a given star
///
/// Looks up mass from tables based on star classification.
/// Mass affects orbital mechanics and companion star generation.
///
/// # Arguments
///
/// * `star` - Star to look up mass for
///
/// # Returns
///
/// Stellar mass as multiple of Sol's mass
pub(crate) fn get_solar_mass(star: &Star) -> f32 {
    interpolate_subtype(&MASS_TABLE, star, m9_anchor(star.size).map(|(_, m)| m))
}

/// Book 6's M9 figures — the far end of the main sequence the tables below
/// stop short of.
///
/// Classic Traveller Book 6 (*Scouts*) tabulates stellar luminosity and mass
/// for B0 through M9 (pp. 44–45); the tables here were transcribed from it
/// (their M0 and M5 rows match it exactly) but stop at M5. Without M9 every
/// M6–M9 star rounded to M5, which made a late red dwarf about seven times
/// too bright by Book 6's own numbers — Hilfer (M6 V) came out scorching at
/// the 5 Mkm the setting places it. `None` where Book 6 gives none (size IV
/// has no K5–M9) or for white dwarfs, whose M row these tables treat
/// separately. Returns `(luminosity, mass)`.
fn m9_anchor(size: StarSize) -> Option<(f32, f32)> {
    Some(match size {
        StarSize::Ia => (141_000.0, 30.0),
        StarSize::Ib => (117_000.0, 25.0),
        StarSize::II => (16_200.0, 18.0),
        StarSize::III => (2_690.0, 9.2),
        StarSize::V => (0.001, 0.215),
        StarSize::VI => (0.000_06, 0.058),
        StarSize::IV | StarSize::D => return None,
    })
}

/// The next cooler spectral class, whose subtype 0 continues this class's 9.
fn next_cooler(t: StarType) -> Option<StarType> {
    match t {
        StarType::O => Some(StarType::B),
        StarType::B => Some(StarType::A),
        StarType::A => Some(StarType::F),
        StarType::F => Some(StarType::G),
        StarType::G => Some(StarType::K),
        StarType::K => Some(StarType::M),
        StarType::M => None,
    }
}

/// Look a star up in a mass or luminosity table, interpolating between rows.
///
/// The tables hold subtypes 0 and 5 only, and every other subtype used to
/// round to one of them — so a G2 got G0's figures and an M8 got M5's. That
/// only fed description text (temperature and year), never generation, but
/// it made those figures step in jumps of five subtypes.
///
/// Now subtypes 1–4 sit between the class's 0 and 5 rows, and 6–9 between
/// its 5 row and the next cooler class's 0 row (G9 lies between G5 and K0).
/// Interpolation is in log space, because mass and luminosity fall off
/// geometrically along the sequence, not linearly. Subtypes 0 and 5 read
/// their rows exactly, so a star on a row is unchanged.
///
/// M6–M8 interpolate from M5 toward Book 6's M9 row (see [`m9_anchor`]),
/// four subtypes on rather than five. Where there's no row to interpolate
/// toward it falls back to rounding, as before: next to O-class rows, which
/// are 0.0 placeholders rather than data, and for sizes Book 6 gives no M9
/// for (IV) or that these tables don't cover that way (D).
fn interpolate_subtype(
    table: &HashMap<(StarType, u8, StarSize), f32>,
    star: &Star,
    m9: Option<f32>,
) -> f32 {
    let at = |t: StarType, sub: u8| table.get(&(t, sub, star.size)).copied();
    let rounded = || at(star.star_type, round_subtype(star.subtype)).unwrap();
    let s = star.subtype;
    if s == 0 || s == 5 {
        return rounded();
    }
    let (lo, hi, frac) = if s < 5 {
        (at(star.star_type, 0), at(star.star_type, 5), f32::from(s) / 5.0)
    } else if star.star_type == StarType::M {
        // M ends the sequence: there's no "N0" row, so M6–M8 run from M5 to
        // Book 6's M9, which is four subtypes on, not five.
        (at(StarType::M, 5), m9, f32::from(s - 5) / 4.0)
    } else {
        (
            at(star.star_type, 5),
            next_cooler(star.star_type).and_then(|n| at(n, 0)),
            f32::from(s - 5) / 5.0,
        )
    };
    match (lo, hi) {
        (Some(a), Some(b)) if a > 0.0 && b > 0.0 => (a.ln() + (b.ln() - a.ln()) * frac).exp(),
        _ => rounded(),
    }
}

/// Converts orbital position to distance in millions of kilometers
///
/// Translates abstract orbital positions to actual distances for
/// astronomical calculations and display. Slots 0..=19 are tabulated;
/// orbits beyond the table are extrapolated (see below) so a system with
/// more bodies than the classic 20-slot table can still place and render
/// them rather than panicking on an out-of-range index.
///
/// # Arguments
///
/// * `orbit` - Orbital position (0 and up; negatives clamp to 0)
///
/// # Returns
///
/// Distance from star in millions of kilometers
pub fn get_orbital_distance(orbit: i32) -> f32 {
    let idx = orbit.max(0) as usize;
    if idx < ORBITAL_DISTANCE.len() {
        return ORBITAL_DISTANCE[idx];
    }
    // Past the tabulated slots, Traveller's orbit spacing follows the
    // doubling recurrence d(n) = 2·d(n-1) − 0.4 AU — the same step baked
    // into ORBITAL_DISTANCE[1] (0.4 AU ≈ 59.84 Mkm). Walk it forward from
    // the last tabulated slot so any orbit index gets a sane distance.
    const STEP_MKM: f32 = 0.4 * 149.6; // 0.4 AU in millions of km
    let mut dist = ORBITAL_DISTANCE[ORBITAL_DISTANCE.len() - 1];
    for _ in ORBITAL_DISTANCE.len()..=idx {
        dist = 2.0 * dist - STEP_MKM;
    }
    dist
}

/// Retrieves cloud coverage percentage for atmosphere type
///
/// Different atmosphere types have characteristic cloud coverage patterns
/// that affect albedo and temperature calculations.
///
/// # Arguments
///
/// * `atmosphere` - Atmosphere code (0-10)
///
/// # Returns
///
/// Cloud coverage as percentage (0-70)
pub(crate) fn get_cloudiness(atmosphere: i32) -> i32 {
    // Clamp rather than index raw: extended-hex UWPs can carry atmosphere
    // codes above 15, and an out-of-range index here aborts the process.
    let idx = (atmosphere.max(0) as usize).min(CLOUDINESS.len() - 1);
    CLOUDINESS[idx]
}

/// Retrieves greenhouse effect multiplier for atmosphere type
///
/// Different atmospheres trap heat to varying degrees, affecting
/// world temperature calculations beyond simple stellar heating.
///
/// # Arguments
///
/// * `atmosphere` - Atmosphere code (0-15)
///
/// # Returns
///
/// Greenhouse effect multiplier (0.0-0.5)
pub(crate) fn get_greenhouse(atmosphere: i32) -> f32 {
    // GREENHOUSE covers 0-15, but extended-hex UWPs can exceed that;
    // clamp so an exotic atmosphere code can't abort the process.
    let idx = (atmosphere.max(0) as usize).min(GREENHOUSE.len() - 1);
    GREENHOUSE[idx]
}

/// Generates random world temperature with modifier
///
/// Rolls 2d6 plus modifier and looks up result on temperature table.
/// Used for determining base world temperature before environmental
/// adjustments.
///
/// # Arguments
///
/// * `modifier` - Temperature modifier based on stellar and orbital factors
///
/// # Returns
///
/// Base world temperature in degrees Celsius
pub(crate) fn get_world_temp(modifier: i32) -> f32 {
    let roll = (roll_2d6() + modifier).clamp(0, AVG_WORLD_TEMP.len() as i32 - 1) as usize;
    AVG_WORLD_TEMP[roll]
}

/// Orbital distances in millions of kilometers
///
/// Maps orbital positions 0-19 to actual distances from the star.
/// Based on a geometric progression that provides realistic
/// spacing for planetary orbits.
///
/// Position 0 ≈ 30 million km (very close, like Mercury)
/// Position 19 ≈ 5.9 billion km (very far, like Pluto)
const ORBITAL_DISTANCE: [f32; 20] = [
    29.9, 59.8, 104.7, 149.6, 239.3, 418.9, 777.9, 1495.9, 2932.0, 5804.0, 11548.0, 23038.0,
    46016.0, 91972.0, 183885.0, 367711.0, 735363.0, 1470666.0, 2941274.0, 5882488.0,
];

/// Cloud coverage percentages by atmosphere type
///
/// Maps atmosphere codes (0-15) to typical cloud coverage.
/// Used in albedo and temperature calculations.
///
/// - 0-1: No atmosphere, no clouds (0%)
/// - 2-3: Thin atmosphere, minimal clouds (10%)
/// - 4-9: Various thick atmospheres (20-70%)
/// - 10: Dense atmosphere, maximum clouds (70%)
/// - 11-13: Exotic / corrosive / insidious — dense, heavily clouded (70%)
/// - 14-15: Unusual high-code atmospheres — tapering cover (60/40%)
///
/// Earlier this table stopped at index 10, so any world with an exotic
/// atmosphere (B-F = 11-15, common in Traveller Map data) panicked the
/// astro calculation and — under panic=abort — took down the whole
/// server process. It now spans the full 0-15 atmosphere range, and
/// `get_cloudiness` clamps out-of-range indices for safety.
const CLOUDINESS: [i32; 16] = [
    0, 0, 10, 10, 20, 30, 40, 50, 60, 70, 70, 70, 70, 70, 60, 40,
];

/// Greenhouse effect multipliers by atmosphere type
///
/// Maps atmosphere codes (0-15) to greenhouse heating effects.
/// Higher values indicate stronger heat retention.
///
/// - 0-3: No greenhouse effect (0.0)
/// - 4-9: Moderate greenhouse effect (0.05-0.15)
/// - 10-12: Strong greenhouse effect (0.5)
/// - 13-15: Reduced effect for exotic atmospheres
const GREENHOUSE: [f32; 16] = [
    0.0, 0.0, 0.0, 0.0, 0.05, 0.05, 0.1, 0.1, 0.15, 0.15, 0.5, 0.5, 0.5, 0.15, 0.10, 0.0,
];

/// Average world temperatures by 2d6+modifier roll
///
/// Base temperature table for world generation. Roll 2d6 plus
/// modifiers for stellar type, orbital position, etc.
///
/// Results range from -2.5°C (frozen) to 35°C (very hot).
/// Index 7 (roll of 7) gives 15°C, roughly Earth-like.
const AVG_WORLD_TEMP: [f32; 16] = [
    -2.5, 0.0, 2.5, 5.0, 7.5, 10.0, 12.5, 15.0, 17.5, 20.0, 22.5, 25.0, 27.5, 30.0, 32.5, 35.0,
];

/// Orbital zone boundaries for a star system
///
/// Defines the boundaries between different temperature and habitability
/// zones around a star. All values are orbital positions (0-19), not
/// actual distances.
///
/// ## Zone Definitions
///
/// - **Inside**: Closest stable orbits, extreme heat
/// - **Hot**: High temperature zone, minimal habitability
/// - **Inner**: Warm zone, some habitability potential  
/// - **Habitable**: Optimal temperature range for life
/// - **Outer**: Cold zone, limited habitability potential
#[derive(Debug, Clone, Copy)]
pub struct ZoneTable {
    /// Boundary of inside zone (orbital position)
    pub inside: i32,
    /// Boundary of hot zone (orbital position)
    pub hot: i32,
    /// Boundary of inner zone (orbital position)
    pub inner: i32,
    /// Boundary of habitable zone (orbital position)
    pub habitable: i32,
    /// Boundary of outer zone (orbital position)
    ///
    /// Currently unused in calculations but included for completeness
    #[allow(dead_code)]
    pub outer: i32,
}

lazy_static! {
    /// Comprehensive zone boundary lookup table
    ///
    /// Contains pre-calculated zone boundaries for all valid combinations
    /// of star size, type, and subtype in the Traveller universe.
    ///
    /// ## Table Structure
    ///
    /// Key: `(StarSize, StarType, rounded_subtype)`
    /// Value: `ZoneTable` with zone boundaries
    ///
    /// ## Coverage
    ///
    /// - **Star Sizes**: Ia, Ib, II, III, IV, V, VI, D
    /// - **Star Types**: O, B, A, F, G, K, M
    /// - **Subtypes**: 0 (early) and 5 (late) variants
    ///
    /// Total entries: 112 stellar classifications
    static ref ZONE_TABLE: HashMap<(StarSize, StarType, u8), ZoneTable> = HashMap::from_iter(vec![
        (
            (StarSize::Ia, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::Ia, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::Ia, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 7,
                inner: 12,
                habitable: 13,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 7,
                inner: 12,
                habitable: 13,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::A, 0),
            ZoneTable {
                inside: 1,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::A, 5),
            ZoneTable {
                inside: 1,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::F, 0),
            ZoneTable {
                inside: 2,
                hot: 5,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::F, 5),
            ZoneTable {
                inside: 2,
                hot: 5,
                inner: 10,
                habitable: 11,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::G, 0),
            ZoneTable {
                inside: 3,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::G, 5),
            ZoneTable {
                inside: 4,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::K, 0),
            ZoneTable {
                inside: 5,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::K, 5),
            ZoneTable {
                inside: 5,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::M, 0),
            ZoneTable {
                inside: 6,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ia, StarType::M, 5),
            ZoneTable {
                inside: 0,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::Ib, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::Ib, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 7,
                inner: 12,
                habitable: 13,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 5,
                inner: 10,
                habitable: 11,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 10,
                habitable: 11,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::A, 5),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::F, 0),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::F, 5),
            ZoneTable {
                inside: 0,
                hot: 3,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::G, 0),
            ZoneTable {
                inside: 0,
                hot: 3,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::G, 5),
            ZoneTable {
                inside: 1,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::K, 0),
            ZoneTable {
                inside: 2,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::K, 5),
            ZoneTable {
                inside: 2,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::M, 0),
            ZoneTable {
                inside: 3,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::Ib, StarType::M, 5),
            ZoneTable {
                inside: 3,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 14
            }
        ),
        (
            (StarSize::II, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::II, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::II, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 10,
                habitable: 11,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 2,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::A, 5),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::F, 0),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::F, 5),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::G, 0),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::G, 5),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::K, 0),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::K, 5),
            ZoneTable {
                inside: 1,
                hot: 2,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::M, 0),
            ZoneTable {
                inside: 3,
                hot: 3,
                inner: 9,
                habitable: 10,
                outer: 13
            }
        ),
        (
            (StarSize::II, StarType::M, 5),
            ZoneTable {
                inside: 5,
                hot: 5,
                inner: 10,
                habitable: 11,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::III, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::III, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 4,
                inner: 9,
                habitable: 10,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::A, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 6,
                habitable: 7,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::F, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 5,
                habitable: 6,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::F, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 5,
                habitable: 6,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::G, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 5,
                habitable: 6,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::G, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 6,
                habitable: 7,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::K, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 6,
                habitable: 7,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::K, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::M, 0),
            ZoneTable {
                inside: 0,
                hot: 1,
                inner: 7,
                habitable: 8,
                outer: 13
            }
        ),
        (
            (StarSize::III, StarType::M, 5),
            ZoneTable {
                inside: 3,
                hot: 3,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::IV, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::IV, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 6,
                inner: 11,
                habitable: 12,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 2,
                inner: 8,
                habitable: 9,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 6,
                habitable: 7,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::A, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 5,
                habitable: 6,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::F, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::F, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::G, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::G, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::K, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 3,
                habitable: 4,
                outer: 13
            }
        ),
        (
            (StarSize::IV, StarType::K, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::IV, StarType::M, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::IV, StarType::M, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::V, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::V, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::V, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 5,
                inner: 11,
                habitable: 12,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 2,
                inner: 8,
                habitable: 9,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::A, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 6,
                habitable: 7,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::A, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 5,
                habitable: 6,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::F, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 4,
                habitable: 5,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::F, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 3,
                habitable: 4,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::G, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 2,
                habitable: 3,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::G, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 2,
                habitable: 3,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::K, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 1,
                habitable: 2,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::K, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: 0,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::M, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: 0,
                outer: 14
            }
        ),
        (
            (StarSize::V, StarType::M, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 14
            }
        ),
        (
            (StarSize::VI, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::B, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::B, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::A, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::A, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::F, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::VI, StarType::F, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 2,
                habitable: 3,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::G, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 1,
                habitable: 2,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::G, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 1,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::K, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::K, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::M, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::VI, StarType::M, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: 0,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::O, 0),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::D, StarType::O, 5),
            ZoneTable {
                inside: 0,
                hot: 0,
                inner: 0,
                habitable: 0,
                outer: 0
            }
        ),
        (
            (StarSize::D, StarType::B, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::B, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: 0,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::A, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::A, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::F, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::F, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::G, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::G, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::K, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::K, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::M, 0),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
        (
            (StarSize::D, StarType::M, 5),
            ZoneTable {
                inside: -1,
                hot: -1,
                inner: -1,
                habitable: -1,
                outer: 4
            }
        ),
    ]);
}

lazy_static! {
    static ref LUMINOSITY_TABLE: HashMap<(StarType, u8, StarSize), f32> = HashMap::from_iter(vec![
        ((StarType::O, 0, StarSize::Ia), 0.0),
        ((StarType::O, 0, StarSize::Ib), 0.0),
        ((StarType::O, 0, StarSize::II), 0.0),
        ((StarType::O, 0, StarSize::III), 0.0),
        ((StarType::O, 0, StarSize::IV), 0.0),
        ((StarType::O, 0, StarSize::V), 0.0),
        ((StarType::O, 0, StarSize::VI), 0.0),
        ((StarType::O, 0, StarSize::D), 0.0),
        ((StarType::O, 5, StarSize::Ia), 0.0),
        ((StarType::O, 5, StarSize::Ib), 0.0),
        ((StarType::O, 5, StarSize::II), 0.0),
        ((StarType::O, 5, StarSize::III), 0.0),
        ((StarType::O, 5, StarSize::IV), 0.0),
        ((StarType::O, 5, StarSize::V), 0.0),
        ((StarType::O, 5, StarSize::VI), 0.0),
        ((StarType::O, 5, StarSize::D), 0.0),
        ((StarType::B, 0, StarSize::Ia), 560_000.0),
        ((StarType::B, 0, StarSize::Ib), 270_000.0),
        ((StarType::B, 0, StarSize::II), 170_000.0),
        ((StarType::B, 0, StarSize::III), 107_000.0),
        ((StarType::B, 0, StarSize::IV), 81_000.0),
        ((StarType::B, 0, StarSize::V), 56_000.0),
        ((StarType::B, 0, StarSize::VI), 0.0),
        ((StarType::B, 0, StarSize::D), 0.46),
        ((StarType::B, 5, StarSize::Ia), 204_000.0),
        ((StarType::B, 5, StarSize::Ib), 46_700.0),
        ((StarType::B, 5, StarSize::II), 18_600.0),
        ((StarType::B, 5, StarSize::III), 6_700.0),
        ((StarType::B, 5, StarSize::IV), 2_000.0),
        ((StarType::B, 5, StarSize::V), 1_400.0),
        ((StarType::B, 5, StarSize::VI), 0.0),
        ((StarType::B, 5, StarSize::D), 0.46),
        ((StarType::A, 0, StarSize::Ia), 107_000.0),
        ((StarType::A, 0, StarSize::Ib), 15_000.0),
        ((StarType::A, 0, StarSize::II), 2_200.0),
        ((StarType::A, 0, StarSize::III), 280.0),
        ((StarType::A, 0, StarSize::IV), 156.0),
        ((StarType::A, 0, StarSize::V), 90.0),
        ((StarType::A, 0, StarSize::VI), 0.0),
        ((StarType::A, 0, StarSize::D), 0.005),
        ((StarType::A, 5, StarSize::Ia), 81_000.0),
        ((StarType::A, 5, StarSize::Ib), 11_700.0),
        ((StarType::A, 5, StarSize::II), 850.0),
        ((StarType::A, 5, StarSize::III), 90.0),
        ((StarType::A, 5, StarSize::IV), 37.0),
        ((StarType::A, 5, StarSize::V), 16.0),
        ((StarType::A, 5, StarSize::VI), 0.0),
        ((StarType::A, 5, StarSize::D), 0.005),
        ((StarType::F, 0, StarSize::Ia), 61_000.0),
        ((StarType::F, 0, StarSize::Ib), 7_400.0),
        ((StarType::F, 0, StarSize::II), 600.0),
        ((StarType::F, 0, StarSize::III), 53.0),
        ((StarType::F, 0, StarSize::IV), 19.0),
        ((StarType::F, 0, StarSize::V), 8.1),
        ((StarType::F, 0, StarSize::VI), 0.0),
        ((StarType::F, 0, StarSize::D), 0.0003),
        ((StarType::F, 5, StarSize::Ia), 51_000.0),
        ((StarType::F, 5, StarSize::Ib), 5_100.0),
        ((StarType::F, 5, StarSize::II), 510.0),
        ((StarType::F, 5, StarSize::III), 43.0),
        ((StarType::F, 5, StarSize::IV), 12.0),
        ((StarType::F, 5, StarSize::V), 3.5),
        ((StarType::F, 5, StarSize::VI), 0.977),
        ((StarType::F, 5, StarSize::D), 0.0003),
        ((StarType::G, 0, StarSize::Ia), 67_000.0),
        ((StarType::G, 0, StarSize::Ib), 6_100.0),
        ((StarType::G, 0, StarSize::II), 560.0),
        ((StarType::G, 0, StarSize::III), 50.0),
        ((StarType::G, 0, StarSize::IV), 6.5),
        ((StarType::G, 0, StarSize::V), 1.21),
        ((StarType::G, 0, StarSize::VI), 0.322),
        ((StarType::G, 0, StarSize::D), 0.00006),
        ((StarType::G, 5, StarSize::Ia), 89_000.0),
        ((StarType::G, 5, StarSize::Ib), 8_100.0),
        ((StarType::G, 5, StarSize::II), 740.0),
        ((StarType::G, 5, StarSize::III), 75.0),
        ((StarType::G, 5, StarSize::IV), 4.9),
        ((StarType::G, 5, StarSize::V), 0.67),
        ((StarType::G, 5, StarSize::VI), 0.186),
        ((StarType::G, 5, StarSize::D), 0.00006),
        ((StarType::K, 0, StarSize::Ia), 100_000.0),
        ((StarType::K, 0, StarSize::Ib), 11_700.0),
        ((StarType::K, 0, StarSize::II), 890.0),
        ((StarType::K, 0, StarSize::III), 95.0),
        ((StarType::K, 0, StarSize::IV), 4.67),
        ((StarType::K, 0, StarSize::V), 0.42),
        ((StarType::K, 0, StarSize::VI), 0.117),
        ((StarType::K, 0, StarSize::D), 0.00004),
        ((StarType::K, 5, StarSize::Ia), 107_000.0),
        ((StarType::K, 5, StarSize::Ib), 20_400.0),
        ((StarType::K, 5, StarSize::II), 2_450.0),
        ((StarType::K, 5, StarSize::III), 320.0),
        ((StarType::K, 5, StarSize::IV), 0.0),
        ((StarType::K, 5, StarSize::V), 0.08),
        ((StarType::K, 5, StarSize::VI), 0.025),
        ((StarType::K, 5, StarSize::D), 0.00004),
        ((StarType::M, 0, StarSize::Ia), 117_000.0),
        ((StarType::M, 0, StarSize::Ib), 46_000.0),
        ((StarType::M, 0, StarSize::II), 4_600.0),
        ((StarType::M, 0, StarSize::III), 470.0),
        ((StarType::M, 0, StarSize::IV), 0.0),
        ((StarType::M, 0, StarSize::V), 0.04),
        ((StarType::M, 0, StarSize::VI), 0.011),
        ((StarType::M, 0, StarSize::D), 0.00003),
        ((StarType::M, 5, StarSize::Ia), 129_000.0),
        ((StarType::M, 5, StarSize::Ib), 89_000.0),
        ((StarType::M, 5, StarSize::II), 14_900.0),
        ((StarType::M, 5, StarSize::III), 2_280.0),
        ((StarType::M, 5, StarSize::IV), 0.0),
        ((StarType::M, 5, StarSize::V), 0.007),
        ((StarType::M, 5, StarSize::VI), 0.002),
        ((StarType::M, 5, StarSize::D), 0.00003),
    ]);
}

lazy_static! {
    /// Stellar mass lookup table
    ///
    /// Mass values are in units of solar masses (M☉)
    /// 0 means that case just shouldn't occur.
    static ref MASS_TABLE: HashMap<(StarType, u8, StarSize), f32> = HashMap::from_iter(vec![
        ((StarType::O, 0, StarSize::Ia), 0.0),
        ((StarType::O, 0, StarSize::Ib), 0.0),
        ((StarType::O, 0, StarSize::II), 0.0),
        ((StarType::O, 0, StarSize::III), 0.0),
        ((StarType::O, 0, StarSize::IV), 0.0),
        ((StarType::O, 0, StarSize::V), 0.0),
        ((StarType::O, 0, StarSize::VI), 0.0),
        ((StarType::O, 0, StarSize::D), 0.0),
        ((StarType::O, 5, StarSize::Ia), 0.0),
        ((StarType::O, 5, StarSize::Ib), 0.0),
        ((StarType::O, 5, StarSize::II), 0.0),
        ((StarType::O, 5, StarSize::III), 0.0),
        ((StarType::O, 5, StarSize::IV), 0.0),
        ((StarType::O, 5, StarSize::V), 0.0),
        ((StarType::O, 5, StarSize::VI), 0.0),
        ((StarType::O, 5, StarSize::D), 0.0),
        ((StarType::B, 0, StarSize::Ia), 60.0),
        ((StarType::B, 0, StarSize::Ib), 50.0),
        ((StarType::B, 0, StarSize::II), 30.0),
        ((StarType::B, 0, StarSize::III), 25.0),
        ((StarType::B, 0, StarSize::IV), 20.0),
        ((StarType::B, 0, StarSize::V), 18.0),
        ((StarType::B, 0, StarSize::VI), 0.0),
        ((StarType::B, 0, StarSize::D), 0.26),
        ((StarType::B, 5, StarSize::Ia), 30.0),
        ((StarType::B, 5, StarSize::Ib), 25.0),
        ((StarType::B, 5, StarSize::II), 20.0),
        ((StarType::B, 5, StarSize::III), 15.0),
        ((StarType::B, 5, StarSize::IV), 10.0),
        ((StarType::B, 5, StarSize::V), 6.5),
        ((StarType::B, 5, StarSize::VI), 0.0),
        ((StarType::B, 5, StarSize::D), 0.26),
        ((StarType::A, 0, StarSize::Ia), 18.0),
        ((StarType::A, 0, StarSize::Ib), 16.0),
        ((StarType::A, 0, StarSize::II), 14.0),
        ((StarType::A, 0, StarSize::III), 12.0),
        ((StarType::A, 0, StarSize::IV), 6.0),
        ((StarType::A, 0, StarSize::V), 3.2),
        ((StarType::A, 0, StarSize::VI), 0.0),
        ((StarType::A, 0, StarSize::D), 0.36),
        ((StarType::A, 5, StarSize::Ia), 15.0),
        ((StarType::A, 5, StarSize::Ib), 13.0),
        ((StarType::A, 5, StarSize::II), 11.0),
        ((StarType::A, 5, StarSize::III), 9.0),
        ((StarType::A, 5, StarSize::IV), 4.0),
        ((StarType::A, 5, StarSize::V), 2.1),
        ((StarType::A, 5, StarSize::VI), 0.0),
        ((StarType::A, 5, StarSize::D), 0.36),
        ((StarType::F, 0, StarSize::Ia), 13.0),
        ((StarType::F, 0, StarSize::Ib), 12.0),
        ((StarType::F, 0, StarSize::II), 10.0),
        ((StarType::F, 0, StarSize::III), 8.0),
        ((StarType::F, 0, StarSize::IV), 2.5),
        ((StarType::F, 0, StarSize::V), 1.7),
        ((StarType::F, 0, StarSize::VI), 0.0),
        ((StarType::F, 0, StarSize::D), 0.42),
        ((StarType::F, 5, StarSize::Ia), 12.0),
        ((StarType::F, 5, StarSize::Ib), 10.0),
        ((StarType::F, 5, StarSize::II), 8.1),
        ((StarType::F, 5, StarSize::III), 5.0),
        ((StarType::F, 5, StarSize::IV), 2.0),
        ((StarType::F, 5, StarSize::V), 1.3),
        ((StarType::F, 5, StarSize::VI), 0.8),
        ((StarType::F, 5, StarSize::D), 0.42),
        ((StarType::G, 0, StarSize::Ia), 12.0),
        ((StarType::G, 0, StarSize::Ib), 10.0),
        ((StarType::G, 0, StarSize::II), 8.1),
        ((StarType::G, 0, StarSize::III), 2.5),
        ((StarType::G, 0, StarSize::IV), 1.75),
        ((StarType::G, 0, StarSize::V), 1.04),
        ((StarType::G, 0, StarSize::VI), 0.6),
        ((StarType::G, 0, StarSize::D), 0.63),
        ((StarType::G, 5, StarSize::Ia), 13.0),
        ((StarType::G, 5, StarSize::Ib), 12.0),
        ((StarType::G, 5, StarSize::II), 10.0),
        ((StarType::G, 5, StarSize::III), 3.2),
        ((StarType::G, 5, StarSize::IV), 2.0),
        ((StarType::G, 5, StarSize::V), 0.94),
        ((StarType::G, 5, StarSize::VI), 0.528),
        ((StarType::G, 5, StarSize::D), 0.63),
        ((StarType::K, 0, StarSize::Ia), 14.0),
        ((StarType::K, 0, StarSize::Ib), 13.0),
        ((StarType::K, 0, StarSize::II), 11.0),
        ((StarType::K, 0, StarSize::III), 4.0),
        ((StarType::K, 0, StarSize::IV), 2.3),
        ((StarType::K, 0, StarSize::V), 0.825),
        ((StarType::K, 0, StarSize::VI), 0.43),
        ((StarType::K, 0, StarSize::D), 0.83),
        ((StarType::K, 5, StarSize::Ia), 18.0),
        ((StarType::K, 5, StarSize::Ib), 16.0),
        ((StarType::K, 5, StarSize::II), 14.0),
        ((StarType::K, 5, StarSize::III), 5.0),
        ((StarType::K, 5, StarSize::IV), 0.0),
        ((StarType::K, 5, StarSize::V), 0.57),
        ((StarType::K, 5, StarSize::VI), 0.33),
        ((StarType::K, 5, StarSize::D), 0.83),
        ((StarType::M, 0, StarSize::Ia), 20.0),
        ((StarType::M, 0, StarSize::Ib), 16.0),
        ((StarType::M, 0, StarSize::II), 14.0),
        ((StarType::M, 0, StarSize::III), 6.3),
        ((StarType::M, 0, StarSize::IV), 0.0),
        ((StarType::M, 0, StarSize::V), 0.489),
        ((StarType::M, 0, StarSize::VI), 0.154),
        ((StarType::M, 0, StarSize::D), 1.11),
        ((StarType::M, 5, StarSize::Ia), 25.0),
        ((StarType::M, 5, StarSize::Ib), 20.0),
        ((StarType::M, 5, StarSize::II), 16.0),
        ((StarType::M, 5, StarSize::III), 7.4),
        ((StarType::M, 5, StarSize::IV), 0.0),
        ((StarType::M, 5, StarSize::V), 0.331),
        ((StarType::M, 5, StarSize::VI), 0.104),
        ((StarType::M, 5, StarSize::D), 1.11),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::system::{Star, System};

    fn v(t: crate::systems::system::StarType, sub: u8) -> Star {
        Star { star_type: t, subtype: sub, size: StarSize::V }
    }

    /// Subtypes 0 and 5 read their table rows exactly; everything between
    /// interpolates instead of rounding. Before, an F4 V took F0's 1.7 M☉.
    #[test]
    fn mass_and_luminosity_interpolate_between_subtype_rows() {
        use crate::systems::system::StarType::{F, G, K, M};
        assert_eq!(get_solar_mass(&v(G, 0)), 1.04);
        assert_eq!(get_solar_mass(&v(G, 5)), 0.94);
        let f4 = get_solar_mass(&v(F, 4));
        assert!(f4 < 1.7 && f4 > 1.3, "F4 V {f4} should lie between F0 and F5");
        // G9 lies between G5 and the next class's K0.
        let g9 = get_solar_mass(&v(G, 9));
        assert!(g9 < 0.94 && g9 > 0.825, "G9 V {g9}");
        // Monotonic along the sequence, G0 through K0.
        let seq: Vec<f32> = (0..=9).map(|s| get_solar_mass(&v(G, s))).collect();
        assert!(seq.windows(2).all(|w| w[1] < w[0]), "{seq:?}");
        assert!(get_luminosity(&v(G, 2)) < get_luminosity(&v(G, 0)));
        // A G2 V is the Sun: about one solar mass.
        assert!((get_solar_mass(&v(G, 2)) - 1.0).abs() < 0.01);
        // M6–M9 run from M5 to Book 6's M9 row (0.001 L☉, 0.215 M☉), which
        // these tables once stopped short of — every late red dwarf used to
        // read as an M5.
        let m9m = get_solar_mass(&v(M, 9));
        let m9l = get_luminosity(&v(M, 9));
        assert!((m9m - 0.215).abs() < 1e-4, "M9 V mass {m9m}");
        assert!((m9l - 0.001).abs() < 1e-6, "M9 V luminosity {m9l}");
        let m6 = get_luminosity(&v(M, 6));
        assert!(m6 < 0.007 && m6 > 0.001, "M6 V luminosity {m6}");
        assert_eq!(get_luminosity(&v(K, 5)), 0.08, "on-row stars read their row exactly");
    }

    /// O-class rows are 0.0 placeholders; interpolating toward one would
    /// produce nonsense, so those fall back to rounding.
    #[test]
    fn placeholder_rows_are_not_interpolated_toward() {
        use crate::systems::system::StarType::{B, O};
        assert_eq!(get_solar_mass(&v(O, 2)), 0.0);
        let b9 = get_solar_mass(&v(B, 9));
        assert!(b9 > 0.0, "B9 V interpolates toward A0, not a placeholder");
    }

    #[test]
    fn test_get_orbital_distance_extrapolates_past_table() {
        // Tabulated slots return the table verbatim.
        assert_eq!(get_orbital_distance(0), 29.9);
        assert_eq!(get_orbital_distance(19), 5882488.0);
        // Negative orbits clamp to slot 0 rather than panicking.
        assert_eq!(get_orbital_distance(-3), 29.9);
        // Past the table, orbits extrapolate via d(n) = 2·d(n-1) − 0.4 AU
        // instead of panicking on an out-of-range index.
        let step = 0.4 * 149.6;
        let d20 = 2.0 * 5882488.0 - step;
        assert!((get_orbital_distance(20) - d20).abs() < 0.1);
        assert!((get_orbital_distance(21) - (2.0 * d20 - step)).abs() < 1.0);
        // Strictly increasing well past the old 20-slot ceiling.
        assert!(get_orbital_distance(30) > get_orbital_distance(25));
    }

    #[test]
    fn test_atmosphere_getters_cover_exotic_codes() {
        // Regression: a world with an exotic atmosphere (B-F = 11-15,
        // e.g. Pourne A9B2887-A, atmosphere B=11) used to index past the
        // 11-entry CLOUDINESS table and abort the whole server process.
        // Every standard atmosphere code must return a value.
        for atmo in 0..=15 {
            let c = get_cloudiness(atmo);
            assert!((0..=100).contains(&c), "cloudiness {c} out of range at {atmo}");
            let _ = get_greenhouse(atmo);
        }
        // Out-of-range (extended-hex) and negative atmospheres clamp
        // instead of panicking.
        assert_eq!(get_cloudiness(33), get_cloudiness(15));
        assert_eq!(get_cloudiness(-1), get_cloudiness(0));
        assert_eq!(get_greenhouse(33), get_greenhouse(15));
    }

    #[test_log::test]
    fn test_get_zone() {
        // Test case 1: Star Type B, Size Ia, Subtype 2
        let system1 = System {
            star: Star {
                star_type: StarType::B,
                size: StarSize::Ia,
                subtype: 2,
            },
            // Other fields can be left as default values for this test
            ..Default::default()
        };

        let zone1 = get_zone(&system1.star);
        assert_eq!(zone1.inside, 0);
        assert_eq!(zone1.hot, 7);
        assert_eq!(zone1.inner, 12);
        assert_eq!(zone1.habitable, 13);
        assert_eq!(zone1.outer, 14);

        // Test case 2: Star Type G, Size II, Subtype 8
        let system2 = System {
            star: Star {
                star_type: StarType::G,
                size: StarSize::II,
                subtype: 8,
            },
            ..Default::default()
        };

        let zone2 = get_zone(&system2.star);
        assert_eq!(zone2.inside, 0);
        assert_eq!(zone2.hot, 1);
        assert_eq!(zone2.inner, 7);
        assert_eq!(zone2.habitable, 8);
        assert_eq!(zone2.outer, 13);

        // Add more test cases as needed
    }
}
