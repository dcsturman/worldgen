//! # Utility Functions Module
//!
//! This module provides common utility functions used throughout the worldgen application,
//! including random number generation for dice rolls and number base conversion utilities.

pub use rand::Rng;
use std::fmt::Display;
/// Converts Arabic numerals to Roman numerals for numbers 0-20
///
/// Used primarily for displaying orbital positions and other small numbers
/// in a classical format appropriate for the Traveller universe aesthetic.
///
/// # Arguments
///
/// * `num` - Integer between 0 and 20 to convert
///
/// # Returns
///
/// String containing the Roman numeral representation
///
/// # Panics
///
/// Panics if the input number is greater than 20
///
/// # Examples
///
/// ```
/// use worldgen::util::arabic_to_roman;
///
/// assert_eq!(arabic_to_roman(1), "I");
/// assert_eq!(arabic_to_roman(4), "IV");
/// assert_eq!(arabic_to_roman(9), "IX");
/// assert_eq!(arabic_to_roman(0), "N");
/// ```
pub fn arabic_to_roman(num: usize) -> String {
    if num > 20 {
        panic!("Input ({num}) must be an integer between 0 and 20");
    }
    let roman_numerals: [(usize, &str); 21] = [
        (20, "XX"),
        (19, "XIX"),
        (18, "XVIII"),
        (17, "XVII"),
        (16, "XVI"),
        (15, "XV"),
        (14, "XIV"),
        (13, "XIII"),
        (12, "XII"),
        (11, "XI"),
        (10, "X"),
        (9, "IX"),
        (8, "VIII"),
        (7, "VII"),
        (6, "VI"),
        (5, "V"),
        (4, "IV"),
        (3, "III"),
        (2, "II"),
        (1, "I"),
        (0, "N"),
    ];
    for (value, symbol) in roman_numerals {
        if num >= value {
            return symbol.to_string();
        }
    }
    "".to_string()
}

/// Utility type to easily format and convert things from credits into MCr
///
/// Supports conversion from i64, i32, i16, and f64
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mcr(i64);

impl Display for Mcr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}", self.0 as f64 / 1_000_000.0)
    }
}
impl From<i64> for Mcr {
    fn from(credits: i64) -> Self {
        Mcr(credits)
    }
}

impl From<i32> for Mcr {
    fn from(credits: i32) -> Self {
        Mcr(credits as i64)
    }
}

impl From<i16> for Mcr {
    fn from(credits: i16) -> Self {
        Mcr(credits as i64)
    }
}

impl From<f64> for Mcr {
    fn from(credits: f64) -> Self {
        Mcr((credits * 1_000_000.0) as i64)
    }
}

/// Convert a i16 for Credits into MCr
pub fn mcr(credits: i64) -> f64 {
    credits as f64 / 1_000_000.0
}

/// Simulates rolling two six-sided dice (2d6)
///
/// This is the most common dice roll in Traveller, used for everything from
/// character generation to trade good availability. Returns a value between
/// 2 and 12 with a bell curve distribution.
///
/// # Returns
///
/// Sum of two dice rolls, ranging from 2 to 12
///
/// # Examples
///
/// ```
/// use worldgen::util::roll_2d6;
///
/// let result = roll_2d6();
/// assert!(result >= 2 && result <= 12);
/// ```
pub fn roll_2d6() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(1..=6) + rng.random_range(1..=6)
}

/// Simulates rolling one six-sided die (1d6)
///
/// Used for various random determinations throughout the system generation
/// and trade calculations. Returns a uniform distribution between 1 and 6.
///
/// # Returns
///
/// Single die roll result, ranging from 1 to 6
///
/// # Examples
///
/// ```
/// use worldgen::util::roll_1d6;
///
/// let result = roll_1d6();
/// assert!(result >= 1 && result <= 6);
/// ```
pub fn roll_1d6() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(1..=6)
}

/// Generates a random digit from 0 to 9
///
/// Used for generating hexadecimal digits in Universal World Profiles (UWPs)
/// and other base-16 representations. Returns a uniform distribution.
///
/// # Returns
///
/// Random digit from 0 to 9 inclusive
///
/// # Examples
///
/// ```
/// use worldgen::util::roll_10;
///
/// let result = roll_10();
/// assert!(result >= 0 && result <= 9);
/// ```
pub fn roll_10() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(0..=9)
}
