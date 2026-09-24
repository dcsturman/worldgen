//! Travel-time arithmetic for the system map's legend.
//!
//! The legend is a table-side quick reference: "how long to get from the
//! primary out to that world at my ship's thrust?" Traveller answers that
//! with the brachistochrone — accelerate at constant thrust to the midpoint,
//! flip, decelerate to a stop — which from rest to rest over distance `d` at
//! acceleration `a` takes `t = 2 * sqrt(d / a)`. It ignores orbital motion
//! and the star's gravity, exactly as the rulebook table does, so the legend
//! agrees with the number a referee would compute by hand.

/// Standard gravity in m/s². One "G" of Traveller thrust.
pub const G_MS2: f64 = 9.81;

/// Thrust ratings the legend tabulates. 1G and 2G are the common civilian
/// drives; 6G is the ceiling for warships and the fastest thing a player
/// can plausibly be aboard. 3G and 4G would be useful too, but the panel
/// only has ~340 px beside the orbit diagram, and each 40 px column takes
/// about six characters from the name column — ordinary names with a
/// `(Gas Giant)` suffix would start truncating, which costs more at the
/// table than the in-between thrusts buy (they can be read off by eye: time
/// scales as 1/sqrt(G)).
pub const THRUSTS_G: [u32; 3] = [1, 2, 6];

/// Seconds to cover `distance_mkm` (millions of km) from rest to rest at a
/// constant `thrust_g`, flipping at the midpoint.
///
/// Computed in `f64`: the distance spans 1e9 to 1e13 m, and at the far end
/// `f32`'s 24-bit mantissa would be visible in a 2-significant-figure
/// output only by luck — cheaper to not rely on luck.
pub fn brachistochrone_secs(distance_mkm: f64, thrust_g: f64) -> f64 {
    let d_m = distance_mkm * 1.0e9;
    let a = thrust_g * G_MS2;
    2.0 * (d_m / a).sqrt()
}

/// Format a duration for a narrow right-aligned legend column.
///
/// One scheme across every row, always at most four characters, always a
/// number followed by a single-letter unit, so the column's right edge lines
/// up and the eye reads the unit without a header:
///
/// | range        | form     | example |
/// |--------------|----------|---------|
/// | < 1 h        | `NNm`    | `45m`   |
/// | 1 h – 10 h   | `N.Nh`   | `3.2h`  |
/// | 10 h – 1 d   | `NNh`    | `17h`   |
/// | 1 d – 10 d   | `N.Nd`   | `1.4d`  |
/// | 10 d – 4 w   | `NNd`    | `12d`   |
/// | ≥ 4 w        | `N.Nw` / `NNw` | `4.3w` |
///
/// One decimal below ten and none above keeps two significant figures —
/// the formula ignores orbital positions, so a third figure would claim a
/// precision the number doesn't have. Days run to 28 rather than switching
/// at 7 because "12d" is how people actually plan a trip; weeks only take
/// over past a month, which in practice is only the slowest hauls to the
/// outermost orbits.
///
/// Thresholds are checked against the *rounded* value so e.g. 9.97 h prints
/// as `10h`, never as the five-character `10.0h`.
pub fn format_duration(secs: f64) -> String {
    const MIN: f64 = 60.0;
    const HOUR: f64 = 3600.0;
    const DAY: f64 = 86_400.0;
    const WEEK: f64 = 7.0 * DAY;

    let one_dp = |v: f64| (v * 10.0).round() / 10.0;

    let minutes = (secs / MIN).round();
    if minutes < 60.0 {
        return format!("{}m", minutes.max(1.0) as u32);
    }
    let hours = secs / HOUR;
    if one_dp(hours) < 10.0 {
        return format!("{:.1}h", hours);
    }
    if hours.round() < 24.0 {
        return format!("{:.0}h", hours);
    }
    let days = secs / DAY;
    if one_dp(days) < 10.0 {
        return format!("{:.1}d", days);
    }
    if days.round() < 28.0 {
        return format!("{:.0}d", days);
    }
    let weeks = secs / WEEK;
    if one_dp(weeks) < 10.0 {
        format!("{:.1}w", weeks)
    } else {
        format!("{:.0}w", weeks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: f64 = 86_400.0;

    #[test]
    fn one_au_at_one_g_is_about_2_9_days() {
        // 1 AU = 149.6 Mkm. 2·sqrt(1.496e11 / 9.81) = 246,983 s = 2.86 d.
        let t = brachistochrone_secs(149.6, 1.0);
        assert!((t - 246_983.0).abs() < 50.0, "got {t}");
        assert!((t / DAY - 2.86).abs() < 0.01);
        assert_eq!(format_duration(t), "2.9d");
    }

    #[test]
    fn time_scales_as_inverse_sqrt_of_thrust_and_sqrt_of_distance() {
        let base = brachistochrone_secs(149.6, 1.0);
        // 4× the thrust halves the time; 4× the distance doubles it.
        assert!((brachistochrone_secs(149.6, 4.0) - base / 2.0).abs() < 1e-6);
        assert!((brachistochrone_secs(4.0 * 149.6, 1.0) - base * 2.0).abs() < 1e-6);
    }

    #[test]
    fn spot_values() {
        // Orbit 0 (29.9 Mkm) at 6G: 2·sqrt(2.99e10 / 58.86) = 45,077 s.
        let t = brachistochrone_secs(29.9, 6.0);
        assert!((t - 45_077.0).abs() < 5.0, "got {t}");
        assert_eq!(format_duration(t), "13h");
        // Orbit 19 (5,882,488 Mkm) at 1G: 2·sqrt(5.88e15 / 9.81) = 48.97e6 s
        // ≈ 81 weeks — the slowest thing the legend will ever print.
        let t = brachistochrone_secs(5_882_488.0, 1.0);
        assert!((t - 48.97e6).abs() < 0.01e6, "got {t}");
        assert_eq!(format_duration(t), "81w");
        // A G2 V's 100D limit (139.2 Mkm) at 2G: 1.96 d.
        let t = brachistochrone_secs(139.2, 2.0);
        assert!((t / DAY - 1.95).abs() < 0.01, "got {}", t / DAY);
    }

    #[test]
    fn formatter_scheme() {
        const H: f64 = 3600.0;
        assert_eq!(format_duration(10.0), "1m");
        assert_eq!(format_duration(45.0 * 60.0), "45m");
        assert_eq!(format_duration(59.6 * 60.0), "1.0h");
        assert_eq!(format_duration(3.24 * H), "3.2h");
        assert_eq!(format_duration(9.97 * H), "10h");
        assert_eq!(format_duration(17.2 * H), "17h");
        assert_eq!(format_duration(23.6 * H), "1.0d");
        assert_eq!(format_duration(1.44 * DAY), "1.4d");
        assert_eq!(format_duration(9.96 * DAY), "10d");
        assert_eq!(format_duration(12.3 * DAY), "12d");
        assert_eq!(format_duration(27.6 * DAY), "3.9w");
        assert_eq!(format_duration(30.0 * DAY), "4.3w");
        assert_eq!(format_duration(69.9 * DAY), "10w");
        assert_eq!(format_duration(200.0 * DAY), "29w");
    }

    #[test]
    fn formatter_never_exceeds_four_characters() {
        // The column pitch is sized for four glyphs; sweep 1 s to ~3 years.
        let mut s = 1.0;
        while s < 1.0e8 {
            let f = format_duration(s);
            assert!(f.len() <= 4, "{s} s formatted as {f:?}");
            s *= 1.07;
        }
    }
}
