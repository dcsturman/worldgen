//! World decorations: facts about a world beyond its UWP that change how its
//! surface map is generated.
//!
//! A UWP and a seed fully determine a map today. Some worlds need more than
//! that — a world tidally locked to its star has no latitude-banded climate
//! at all, and nothing in the UWP can say so. A decoration is how that fact
//! travels from the system generator (or an override, or a URL) to the map
//! generator.
//!
//! The type sits at the crate root rather than under `systems` or `worldmap`
//! because both sides import it, and so does the backend's cache key and the
//! frontend's URL. Neither module should depend on the other to name it.
//!
//! ## The contract every consumer relies on
//!
//! [`WorldDecorations::default()`] is *empty*, and an empty value must take
//! exactly the code path that existed before decorations did. Existing maps
//! are cached in GCS under keys with no decoration in them; if an empty value
//! rendered even one pixel differently, every cached world would silently
//! disagree with a fresh render of itself.
//!
//! ## Wire format
//!
//! One query parameter, `deco`, holding a comma-separated list of tokens:
//!
//! ```text
//! deco=tl                 tide-locked, substellar point derived from the seed
//! deco=tl:12.5:270        tide-locked, substellar point at lat 12.5°, lon 270°
//! deco=none               explicitly no decorations (same value as absent)
//! ```
//!
//! [`WorldDecorations::to_query`] emits one canonical spelling per value, so
//! it doubles as the cache-key input: two requests that mean the same world
//! can't land in two cache slots. Characters are limited to `[a-z0-9.:,-]`,
//! none of which need percent-encoding in a query string. `+` is avoided on
//! purpose — the backend's query parser decodes it to a space.

use serde::{Deserialize, Serialize};

/// Everything beyond the UWP that shapes a world's map.
///
/// A struct of optional fields rather than a list of enum variants: each
/// decoration can appear at most once, a field has a fixed position so the
/// canonical query order falls out of the declaration order, and a new
/// decoration is a new `Option` field that defaults to `None` — so adding one
/// can't change the meaning of any value that already exists.
///
/// Not `Copy`, deliberately. A future decoration may well carry a `String`,
/// and callers that got used to copying would all break at once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldDecorations {
    /// The world keeps one face to its star. `None` is the ordinary rotating
    /// world every map was before this existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tide_locked: Option<TideLock>,
}

/// A tidal lock to the world's primary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TideLock {
    /// Where the star stands overhead. `None` derives it from the map seed
    /// (see [`TideLock::resolve`]), so a locked world without a stated
    /// substellar point still doesn't always put its hot spot in the same
    /// place on the projection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substellar: Option<LatLon>,
}

/// A point on the planet, stored in hundredths of a degree.
///
/// Integers, not floats, so the type can be `Eq` and `Hash` honestly and the
/// query encoding round-trips exactly — a cache key built from a float that
/// printed as `12.500000000000002` on one path and `12.5` on another would
/// split one world across two cache slots. A hundredth of a degree is about
/// a kilometre on an Earth-sized world, far below one map hex.
///
/// Serialized to JSON as `[lat_degrees, lon_degrees]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "[f64; 2]", try_from = "[f64; 2]")]
pub struct LatLon {
    /// Latitude in centidegrees, `-9000..=9000`.
    lat_cdeg: i32,
    /// Longitude in centidegrees, normalized into `0..36000`.
    lon_cdeg: i32,
}

/// Why a decoration string or value was rejected.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DecorationError {
    #[error("unknown decoration {0:?}; expected \"tl\" or \"none\"")]
    Unknown(String),
    #[error("decoration {0:?} given more than once")]
    Duplicate(String),
    #[error("tide lock takes no arguments or exactly two (tl:LAT:LON); got {0:?}")]
    TideLockArity(String),
    #[error("{0:?} is not a finite number of degrees")]
    BadNumber(String),
    #[error("latitude {0} is outside -90..=90")]
    LatitudeOutOfRange(f64),
}

/// Seed salt for the derived substellar longitude. Any fixed value works; it
/// exists so the longitude isn't a simple function of the same bits every
/// other seeded stream starts from.
const SUBSTELLAR_SALT: u64 = 0x5B57_E11A_0DDB_A115;

impl WorldDecorations {
    /// The query-string key the value travels under, in `/api/world` and on
    /// the `/worldmap` page.
    pub const QUERY_PARAM: &'static str = "deco";

    /// True when no decoration is set — the value that must reproduce the
    /// undecorated map byte for byte.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// A value with only the tide lock set.
    pub fn tide_locked(lock: TideLock) -> Self {
        Self {
            tide_locked: Some(lock),
        }
    }

    /// Canonical query encoding. Empty for an empty value, so callers can
    /// append `&deco=…` only when this is non-empty and leave every existing
    /// URL exactly as it was.
    pub fn to_query(&self) -> String {
        let mut tokens: Vec<String> = Vec::new();
        if let Some(lock) = &self.tide_locked {
            tokens.push(match lock.substellar {
                None => "tl".to_string(),
                Some(p) => format!(
                    "tl:{}:{}",
                    fmt_centidegrees(p.lat_cdeg),
                    fmt_centidegrees(p.lon_cdeg)
                ),
            });
        }
        tokens.join(",")
    }

    /// Parse a `deco` value. Empty, whitespace or `none` is the empty value.
    ///
    /// Unknown tokens are an error rather than ignored: a mistyped `deco=tI`
    /// that quietly rendered — and cached — the unlocked map would be the
    /// hardest possible version of that bug to notice.
    pub fn parse_query(s: &str) -> Result<Self, DecorationError> {
        let s = s.trim();
        if s.is_empty() || s.eq_ignore_ascii_case("none") {
            return Ok(Self::default());
        }
        let mut out = Self::default();
        for token in s.split(',').map(str::trim) {
            let mut parts = token.split(':');
            let key = parts.next().unwrap_or_default().to_ascii_lowercase();
            let args: Vec<&str> = parts.collect();
            match key.as_str() {
                "tl" => {
                    if out.tide_locked.is_some() {
                        return Err(DecorationError::Duplicate(token.to_string()));
                    }
                    let substellar = match args.as_slice() {
                        [] => None,
                        [lat, lon] => Some(LatLon::from_degrees(
                            parse_degrees(lat)?,
                            parse_degrees(lon)?,
                        )?),
                        _ => return Err(DecorationError::TideLockArity(token.to_string())),
                    };
                    out.tide_locked = Some(TideLock { substellar });
                }
                _ => return Err(DecorationError::Unknown(token.to_string())),
            }
        }
        Ok(out)
    }
}

impl std::fmt::Display for WorldDecorations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_query())
    }
}

impl std::str::FromStr for WorldDecorations {
    type Err = DecorationError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_query(s)
    }
}

impl TideLock {
    /// The substellar point this lock uses on a map generated from `seed`:
    /// the stated one if there is one, otherwise one derived from the seed.
    ///
    /// A derived point always sits on the equator. A locked world's spin axis
    /// is perpendicular to its orbit, so the star stands over the equator and
    /// both geographic poles lie on the terminator — which is what the
    /// climate model expects. Only the longitude varies with the seed.
    ///
    /// Integer arithmetic on purpose: the result is identical on native and
    /// wasm, and lands on the same centidegree grid an explicit point does, so
    /// `tl` and `tl:0:<derived lon>` render the same map.
    ///
    /// This takes the map seed alone and draws nothing from the generator's
    /// RNG, so resolving a lock can't shift any other seeded stream.
    pub fn resolve(&self, seed: u64) -> LatLon {
        self.substellar.unwrap_or_else(|| LatLon {
            lat_cdeg: 0,
            lon_cdeg: (splitmix64(seed ^ SUBSTELLAR_SALT) % 36_000) as i32,
        })
    }
}

impl LatLon {
    /// Build from degrees, rounding to the nearest hundredth. Longitude wraps
    /// into `[0, 360)`; latitude must already be within `[-90, 90]`.
    pub fn from_degrees(lat: f64, lon: f64) -> Result<Self, DecorationError> {
        if !lat.is_finite() {
            return Err(DecorationError::BadNumber(lat.to_string()));
        }
        if !lon.is_finite() {
            return Err(DecorationError::BadNumber(lon.to_string()));
        }
        if !(-90.0..=90.0).contains(&lat) {
            return Err(DecorationError::LatitudeOutOfRange(lat));
        }
        let lat_cdeg = (lat * 100.0).round() as i32;
        let lon_cdeg = ((lon * 100.0).round() as i64).rem_euclid(36_000) as i32;
        Ok(Self { lat_cdeg, lon_cdeg })
    }

    pub fn lat_degrees(&self) -> f64 {
        self.lat_cdeg as f64 / 100.0
    }

    pub fn lon_degrees(&self) -> f64 {
        self.lon_cdeg as f64 / 100.0
    }

    /// Unit-sphere position in the map's convention — the same one
    /// `worldmap::grid::xy_to_sphere` uses: `z = sin(lat)`, longitude measured
    /// from +x toward +y.
    pub fn to_unit_vector(&self) -> [f64; 3] {
        let lat = self.lat_degrees().to_radians();
        let lon = self.lon_degrees().to_radians();
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    }
}

impl From<LatLon> for [f64; 2] {
    fn from(p: LatLon) -> Self {
        [p.lat_degrees(), p.lon_degrees()]
    }
}

impl TryFrom<[f64; 2]> for LatLon {
    type Error = DecorationError;
    fn try_from([lat, lon]: [f64; 2]) -> Result<Self, Self::Error> {
        Self::from_degrees(lat, lon)
    }
}

fn parse_degrees(s: &str) -> Result<f64, DecorationError> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| DecorationError::BadNumber(s.to_string()))
}

/// Degrees with at most two decimals and no trailing zeros: `1250` → `12.5`,
/// `-3` → `-0.03`, `0` → `0`. Built from the integer so there's no float
/// formatting in the canonical form at all.
fn fmt_centidegrees(cdeg: i32) -> String {
    let sign = if cdeg < 0 { "-" } else { "" };
    let abs = cdeg.unsigned_abs();
    let (whole, frac) = (abs / 100, abs % 100);
    match frac {
        0 => format!("{sign}{whole}"),
        f if f % 10 == 0 => format!("{sign}{whole}.{}", f / 10),
        f => format!("{sign}{whole}.{f:02}"),
    }
}

/// SplitMix64 finalizer — a fixed, dependency-free bit mixer, so the derived
/// longitude can't change under us with a `rand` upgrade.
fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locked_at(lat: f64, lon: f64) -> WorldDecorations {
        WorldDecorations::tide_locked(TideLock {
            substellar: Some(LatLon::from_degrees(lat, lon).unwrap()),
        })
    }

    #[test]
    fn default_is_empty_and_encodes_to_nothing() {
        let d = WorldDecorations::default();
        assert!(d.is_empty());
        assert_eq!(d.to_query(), "");
    }

    #[test]
    fn empty_spellings_parse_to_default() {
        for s in ["", "  ", "none", "NONE"] {
            assert_eq!(
                WorldDecorations::parse_query(s).unwrap(),
                WorldDecorations::default()
            );
        }
    }

    #[test]
    fn bare_tide_lock_round_trips() {
        let d = WorldDecorations::parse_query("tl").unwrap();
        assert_eq!(d, WorldDecorations::tide_locked(TideLock::default()));
        assert!(!d.is_empty());
        assert_eq!(d.to_query(), "tl");
    }

    #[test]
    fn explicit_substellar_round_trips() {
        let d = WorldDecorations::parse_query("tl:12.5:-90").unwrap();
        assert_eq!(d, locked_at(12.5, 270.0));
        // Longitude comes back normalized, so both spellings share one key.
        assert_eq!(d.to_query(), "tl:12.5:270");
        assert_eq!(WorldDecorations::parse_query(&d.to_query()).unwrap(), d);
    }

    #[test]
    fn canonical_form_is_stable_across_spellings() {
        // Every spelling of one value must produce one cache key.
        let a = WorldDecorations::parse_query(" TL:0.10:360.00 ").unwrap();
        let b = WorldDecorations::parse_query("tl:0.1:0").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.to_query(), "tl:0.1:0");
    }

    #[test]
    fn centidegree_formatting() {
        assert_eq!(fmt_centidegrees(0), "0");
        assert_eq!(fmt_centidegrees(1250), "12.5");
        assert_eq!(fmt_centidegrees(-3), "-0.03");
        assert_eq!(fmt_centidegrees(-9000), "-90");
        assert_eq!(fmt_centidegrees(35_999), "359.99");
    }

    #[test]
    fn rejects_malformed_values() {
        use DecorationError::*;
        let err = |s: &str| WorldDecorations::parse_query(s).unwrap_err();
        assert!(matches!(err("tI"), Unknown(_)));
        assert!(matches!(err("tl,tl"), Duplicate(_)));
        assert!(matches!(err("tl:10"), TideLockArity(_)));
        assert!(matches!(err("tl:1:2:3"), TideLockArity(_)));
        assert!(matches!(err("tl:north:0"), BadNumber(_)));
        assert!(matches!(err("tl:NaN:0"), BadNumber(_)));
        assert!(matches!(err("tl:91:0"), LatitudeOutOfRange(_)));
    }

    #[test]
    fn derived_substellar_is_deterministic_equatorial_and_seed_dependent() {
        let lock = TideLock::default();
        let a = lock.resolve(42);
        assert_eq!(a, lock.resolve(42));
        assert_eq!(a.lat_degrees(), 0.0);
        assert!((0.0..360.0).contains(&a.lon_degrees()));
        // Different seeds should scatter the hot spot, not pin it.
        let lons: std::collections::HashSet<i32> =
            (0..32u64).map(|s| lock.resolve(s).lon_cdeg).collect();
        assert!(
            lons.len() > 28,
            "derived longitudes collide too often: {lons:?}"
        );
    }

    #[test]
    fn stated_substellar_ignores_seed() {
        let p = LatLon::from_degrees(-20.0, 45.0).unwrap();
        let lock = TideLock {
            substellar: Some(p),
        };
        assert_eq!(lock.resolve(1), p);
        assert_eq!(lock.resolve(2), p);
    }

    #[test]
    fn derived_point_is_expressible_explicitly() {
        // `tl` and `tl:0:<derived>` must mean the same map.
        let derived = TideLock::default().resolve(7);
        let explicit =
            WorldDecorations::parse_query(&format!("tl:0:{}", fmt_centidegrees(derived.lon_cdeg)))
                .unwrap();
        assert_eq!(explicit.tide_locked.unwrap().resolve(999), derived);
    }

    #[test]
    fn unit_vector_matches_map_convention() {
        let v = LatLon::from_degrees(0.0, 90.0).unwrap().to_unit_vector();
        assert!(v[0].abs() < 1e-12 && (v[1] - 1.0).abs() < 1e-12 && v[2].abs() < 1e-12);
        let n = LatLon::from_degrees(90.0, 0.0).unwrap().to_unit_vector();
        assert!((n[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn json_is_compact_and_round_trips() {
        assert_eq!(
            serde_json::to_string(&WorldDecorations::default()).unwrap(),
            "{}"
        );
        let d = locked_at(-12.25, 300.0);
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, r#"{"tide_locked":{"substellar":[-12.25,300.0]}}"#);
        assert_eq!(serde_json::from_str::<WorldDecorations>(&json).unwrap(), d);
        // A missing field is the empty value, so old serialized worlds load.
        assert_eq!(
            serde_json::from_str::<WorldDecorations>("{}").unwrap(),
            WorldDecorations::default()
        );
        assert!(serde_json::from_str::<WorldDecorations>(r#"{"tide_lock":{}}"#).is_err());
    }
}
