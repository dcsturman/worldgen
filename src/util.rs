//! # Utility Functions Module
//!
//! This module provides common utility functions used throughout the worldgen application,
//! including random number generation for dice rolls and number base conversion utilities.

pub use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::cell::RefCell;
use std::fmt::Display;

thread_local! {
    /// Thread-local seeded RNG consulted by `roll_2d6` / `roll_1d6` /
    /// `roll_10` (and the few direct rng helpers in `src/systems/system.rs`)
    /// when set. The library's seeded entry points install one of these
    /// via [`RngScope`]; outside that scope, the helpers fall back to the
    /// system RNG so the existing non-seeded UI path is unchanged.
    static WORLDGEN_RNG: RefCell<Option<ChaCha8Rng>> = const { RefCell::new(None) };
}

/// RAII guard installing a seeded `ChaCha8Rng` as the worldgen thread-local
/// for the lifetime of the guard. On drop (including via panic unwind) the
/// previous thread-local state is restored, so nested seeded calls compose
/// correctly and a panic mid-generation doesn't leak seeded entropy into
/// unrelated calls on the same thread.
pub struct RngScope {
    prev: Option<ChaCha8Rng>,
}

impl RngScope {
    pub fn new(seed: u64) -> Self {
        let prev =
            WORLDGEN_RNG.with(|cell| cell.borrow_mut().replace(ChaCha8Rng::seed_from_u64(seed)));
        RngScope { prev }
    }
}

impl Drop for RngScope {
    fn drop(&mut self) {
        WORLDGEN_RNG.with(|cell| *cell.borrow_mut() = self.prev.take());
    }
}

/// Generate a `random_range` value using the worldgen thread-local seeded
/// RNG if one is installed, otherwise the system RNG. Use this for the
/// handful of direct rng calls in `src/systems/system.rs` that the dice
/// helpers don't already cover.
pub fn rng_random_range<T, R>(range: R) -> T
where
    T: rand::distr::uniform::SampleUniform,
    R: rand::distr::uniform::SampleRange<T>,
{
    WORLDGEN_RNG.with(|cell| {
        if let Some(rng) = cell.borrow_mut().as_mut() {
            rng.random_range(range)
        } else {
            rand::rng().random_range(range)
        }
    })
}

/// Pick a random element from a slice using the worldgen thread-local
/// seeded RNG if one is installed, otherwise the system RNG.
pub fn rng_choose<T>(slice: &[T]) -> Option<&T> {
    use rand::seq::IndexedRandom;
    WORLDGEN_RNG.with(|cell| {
        if let Some(rng) = cell.borrow_mut().as_mut() {
            slice.choose(rng)
        } else {
            slice.choose(&mut rand::rng())
        }
    })
}
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
pub struct Credits(i64);

impl Credits {
    pub fn as_string(&self) -> String {
        String::from(self)
    }
}
impl Display for Credits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from(self))
    }
}

impl From<&Credits> for String {
    fn from(credits: &Credits) -> String {
        if credits.0.abs() < 1000 {
            format!("{} Cr", credits.0)
        } else if credits.0.abs() < 1000000 {
            format!("{:.2} KCr", credits.0 as f64 / 1_000.0)
        } else {
            format!("{:.2} MCr", credits.0 as f64 / 1_000_000.0)
        }
    }
}

impl From<Credits> for String {
    fn from(credits: Credits) -> String {
        String::from(&credits)
    }
}

impl From<i64> for Credits {
    fn from(credits: i64) -> Self {
        Credits(credits)
    }
}

impl From<i32> for Credits {
    fn from(credits: i32) -> Self {
        Credits(credits as i64)
    }
}

impl From<i16> for Credits {
    fn from(credits: i16) -> Self {
        Credits(credits as i64)
    }
}

impl From<f64> for Credits {
    fn from(credits: f64) -> Self {
        Credits((credits * 1_000_000.0) as i64)
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
    WORLDGEN_RNG.with(|cell| {
        if let Some(rng) = cell.borrow_mut().as_mut() {
            rng.random_range(1..=6) + rng.random_range(1..=6)
        } else {
            let mut r = rand::rng();
            r.random_range(1..=6) + r.random_range(1..=6)
        }
    })
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
    WORLDGEN_RNG.with(|cell| {
        if let Some(rng) = cell.borrow_mut().as_mut() {
            rng.random_range(1..=6)
        } else {
            rand::rng().random_range(1..=6)
        }
    })
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
    WORLDGEN_RNG.with(|cell| {
        if let Some(rng) = cell.borrow_mut().as_mut() {
            rng.random_range(0..=9)
        } else {
            rand::rng().random_range(0..=9)
        }
    })
}

/// Calculate the distance in parsecs between two hex coordinates
///
/// Uses cube coordinate conversion for accurate hex grid distance calculation.
/// This is the standard method for calculating jump distances in Traveller.
///
/// # Arguments
///
/// * `hex_x1` - Column coordinate of first hex
/// * `hex_y1` - Row coordinate of first hex
/// * `hex_x2` - Column coordinate of second hex
/// * `hex_y2` - Row coordinate of second hex
///
/// # Returns
///
/// Distance in parsecs (hex units) between the two coordinates
///
/// # Examples
///
/// ```
/// use worldgen::util::calculate_hex_distance;
///
/// let distance = calculate_hex_distance(10, 15, 11, 15); // Adjacent hexes = 1
/// assert_eq!(distance, 1);
///
/// let distance = calculate_hex_distance(10, 10, 10, 10); // Same hex = 0
/// assert_eq!(distance, 0);
/// ```
pub fn calculate_hex_distance(hex_x1: i32, hex_y1: i32, hex_x2: i32, hex_y2: i32) -> i32 {
    // Convert offset coordinates to cube coordinates
    let (x1, y1, z1) = offset_to_cube(hex_x1, hex_y1);
    let (x2, y2, z2) = offset_to_cube(hex_x2, hex_y2);

    // Calculate distance using cube coordinates
    ((x1 - x2).abs() + (y1 - y2).abs() + (z1 - z2).abs()) / 2
}

/// Convert offset hex coordinates to cube coordinates
///
/// Transforms Traveller's standard offset coordinate system into cube coordinates
/// for efficient distance calculations. Uses the odd-q offset system which is
/// standard for Traveller maps.
///
/// # Arguments
///
/// * `col` - Column coordinate (X in offset system)
/// * `row` - Row coordinate (Y in offset system)
///
/// # Returns
///
/// Tuple of (x, y, z) cube coordinates where x + y + z = 0
pub fn offset_to_cube(col: i32, row: i32) -> (i32, i32, i32) {
    let x = col;
    let z = row - (col + (col & 1)) / 2;
    let y = -x - z;
    (x, y, z)
}

/// Build a `/worldmap?…` URL that opens a deterministic per-world map.
///
/// The seed is derived from `name + uwp` so the same world always opens
/// to the same surface map across sessions. Used by the system view's
/// per-world "Map" link. The hash is truncated to u32 so the displayed
/// seed string stays short (≤10 decimal digits) — full u64 hashes
/// overflow the on-map seed badge.
pub fn worldmap_url(name: &str, uwp: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    name.hash(&mut h);
    uwp.hash(&mut h);
    let seed = h.finish() as u32 as u64;
    let n = if name.is_empty() { "World" } else { name };
    format!(
        "/worldmap?uwp={}&seed={}&name={}",
        urlencode_minimal(uwp),
        seed,
        urlencode_minimal(n)
    )
}

/// Tiny URL encoder — covers the few characters our names/UWPs realistically
/// use (space, &, ?, #). Full RFC 3986 is overkill given our alphabet.
fn urlencode_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
            out.push(c);
        } else {
            for b in c.to_string().bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// Default base URL when `TRAVELLERMAP_URL` isn't set at build time.
const DEFAULT_TRAVELLERMAP_URL: &str = "https://travellermap.com";

/// The base URL of the TravellerMap-compatible service worldgen talks
/// to for sector/world lookups and tile rendering.
///
/// Resolved at **compile time** via `option_env!` from the
/// `TRAVELLERMAP_URL` environment variable, so the same value is baked
/// into both the WASM frontend bundle and the native backend binary
/// from a single build-time setting. Defaults to
/// `https://travellermap.com` when the env var is unset.
///
/// Any trailing slash is stripped so callers can write
/// `format!("{}/data/...", travellermap_base_url())` without worrying
/// about double-slash URLs.
///
/// Set the env var when building:
///
/// ```text
/// TRAVELLERMAP_URL=https://tmap.internal cargo build --features backend --bin server
/// TRAVELLERMAP_URL=https://tmap.internal trunk build --release
/// ```
///
/// `build.rs` declares `cargo:rerun-if-env-changed=TRAVELLERMAP_URL`
/// so flipping the value between builds invalidates the cache
/// correctly instead of silently reusing a binary with the old URL
/// baked in.
pub fn travellermap_base_url() -> &'static str {
    let raw = option_env!("TRAVELLERMAP_URL").unwrap_or(DEFAULT_TRAVELLERMAP_URL);
    raw.trim_end_matches('/')
}

/// Decode a single Traveller "extended hex" (ehex) digit to its value.
///
/// Traveller uses pseudo-hex that runs past `F`: `0`-`9` then the letters
/// `A`-`Z` **skipping `I` and `O`** (so they're not confused with `1` and
/// `0`). So `A`=10 … `H`=17, `J`=18 … `N`=22, `P`=23 … `Z`=33. This is why
/// plain `i32::from_str_radix(_, 16)` is wrong for UWP columns — it caps at
/// `F` (15) and rejects `G`+ (e.g. Tech Level 16 = `G`). Case-insensitive;
/// returns `None` for any other character.
pub fn ehex_to_value(c: char) -> Option<u32> {
    let c = c.to_ascii_uppercase();
    match c {
        '0'..='9' => Some(c as u32 - '0' as u32),
        'A'..='H' => Some(10 + (c as u32 - 'A' as u32)),
        'J'..='N' => Some(18 + (c as u32 - 'J' as u32)),
        'P'..='Z' => Some(23 + (c as u32 - 'P' as u32)),
        _ => None,
    }
}

/// Encode a value as a Traveller ehex digit — the inverse of
/// [`ehex_to_value`]. `0`-`9` then `A`-`H`, `J`-`N`, `P`-`Z` (skipping `I`
/// and `O`). Values outside `0..=33` return `'?'`.
pub fn value_to_ehex(v: u32) -> char {
    match v {
        0..=9 => (b'0' + v as u8) as char,
        10..=17 => (b'A' + (v - 10) as u8) as char,
        18..=22 => (b'J' + (v - 18) as u8) as char,
        23..=33 => (b'P' + (v - 23) as u8) as char,
        _ => '?',
    }
}

/// Escape the five XML predefined entities so a string is safe to embed
/// in either SVG text content or a double-quoted attribute value. Shared
/// by the `worldmap` and `sysmap` SVG renderers so the escaping rules
/// can't drift between the two.
pub fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod ehex_tests {
    use super::{ehex_to_value, value_to_ehex};

    #[test]
    fn decodes_digits_and_letters() {
        assert_eq!(ehex_to_value('0'), Some(0));
        assert_eq!(ehex_to_value('9'), Some(9));
        assert_eq!(ehex_to_value('A'), Some(10));
        assert_eq!(ehex_to_value('F'), Some(15));
        assert_eq!(ehex_to_value('G'), Some(16)); // the Tech-Level-16 case
        assert_eq!(ehex_to_value('H'), Some(17));
        assert_eq!(ehex_to_value('J'), Some(18)); // skips I
        assert_eq!(ehex_to_value('N'), Some(22));
        assert_eq!(ehex_to_value('P'), Some(23)); // skips O
        assert_eq!(ehex_to_value('Z'), Some(33));
        assert_eq!(ehex_to_value('a'), Some(10)); // case-insensitive
    }

    #[test]
    fn rejects_skipped_and_invalid_letters() {
        // I and O are not ehex digits (they'd be confused with 1 and 0).
        assert_eq!(ehex_to_value('I'), None);
        assert_eq!(ehex_to_value('O'), None);
        assert_eq!(ehex_to_value('-'), None);
        assert_eq!(ehex_to_value(' '), None);
    }

    #[test]
    fn encodes_inverse_of_decode() {
        assert_eq!(value_to_ehex(16), 'G');
        assert_eq!(value_to_ehex(18), 'J');
        assert_eq!(value_to_ehex(23), 'P');
        assert_eq!(value_to_ehex(99), '?');
        for v in 0..=33u32 {
            assert_eq!(ehex_to_value(value_to_ehex(v)), Some(v), "round-trip {v}");
        }
    }
}

#[cfg(test)]
mod travellermap_url_tests {
    use super::*;

    #[test]
    fn default_when_unset_at_build_time() {
        // The dev/test build doesn't set TRAVELLERMAP_URL, so the
        // helper must fall through to the default. If someone ever
        // sets it during local test runs, this test will catch the
        // surprise so they remember to unset it.
        let url = travellermap_base_url();
        assert!(
            url == "https://travellermap.com" || option_env!("TRAVELLERMAP_URL").is_some(),
            "expected default URL, got {url:?}"
        );
    }

    #[test]
    fn no_trailing_slash() {
        assert!(
            !travellermap_base_url().ends_with('/'),
            "base URL must not end with '/' so callers can concat \"/path\""
        );
    }
}
