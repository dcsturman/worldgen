//! Cloud layer for the globe.
//!
//! Globe-only, like [`super::orbital`]: the flat map is a reference document
//! and weather would sit on top of exactly the terrain a referee is trying to
//! read. On a globe the opposite holds — an atmosphere with no weather in it
//! is one of the strongest tells that a planet is a rendering rather than a
//! photograph.
//!
//! Two design choices are worth stating up front.
//!
//! **Coverage is derived from the UWP, not chosen for looks.** Atmosphere code
//! and hydrographics set how much cloud a world gets, so the layer *carries
//! information*: a referee can look at a globe and see that it's a thin-atmo
//! world, or that a dense atmosphere is socked in. Decoration that happens to
//! also be data. Worlds that physically cannot have weather — atmosphere 0 or
//! 1, near-vacuum — get no layer at all rather than a faint one.
//!
//! **Clouds are baked into the surface RGB.** The globe's main consumer warps
//! our texture with its own lighting, so there is nowhere to put a separate
//! cloud layer and no way to give it parallax or a true cast shadow. What we
//! can do honestly is view-independent: thick cloud slightly darkens the
//! ground beneath it (ambient occlusion, plausible under any sun) and dims the
//! city lights it covers. A directional drop-shadow would look right only
//! under the one sun angle we guessed at, and wrong under every other.

use ::noise::{Fbm, MultiFractal, NoiseFn, Simplex};

use super::Uwp;

/// Fraction of the sky a world's cloud deck covers before latitude banding,
/// by atmosphere code. Hydrographics scales this further — there has to be
/// something to evaporate.
///
/// Atmosphere 0–1 is absent entirely (see [`CloudField::from_uwp`]); the
/// exotic and corrosive codes run heavy because whatever is suspended in them
/// is doing the same optical job as water vapour.
fn coverage_for_atmosphere(atmo: u8) -> f64 {
    match atmo {
        2..=3 => 0.10,   // very thin / thin — wisps
        4..=5 => 0.20,   // thin, breathable
        6..=7 => 0.30,   // standard
        8..=9 => 0.38,   // dense
        10..=11 => 0.72, // exotic / corrosive — heavy, permanent deck
        _ => 0.85,       // insidious and beyond — effectively overcast
    }
}

/// Hydrographics multiplier: a desert world has little to put in the air even
/// under a thick atmosphere, an ocean world plenty. Never quite zero, since
/// dust and volcanic haze cloud a dry sky too.
fn coverage_for_hydrographics(hyd: u8) -> f64 {
    match hyd {
        0 => 0.22,
        1..=2 => 0.45,
        3..=4 => 0.70,
        5..=7 => 0.90,
        _ => 1.0,
    }
}

/// Spread applied to the raw fBm before thresholding.
///
/// fBm output clusters near zero rather than filling [-1, 1] — its tails are
/// rare — so treating the raw value as uniform makes the coverage threshold
/// badly non-linear: a target of 0.07 lands at a cut the noise essentially
/// never exceeds, and a thin-atmosphere world comes out with a perfectly
/// clear sky instead of wisps. Spreading first makes `target` behave roughly
/// like the fraction of sky it claims to be.
const NOISE_GAIN: f64 = 2.2;

/// Softness of the cloud edge, in noise units either side of the threshold.
/// Small values give hard-edged blobs; too large and the deck turns to haze.
const EDGE: f64 = 0.13;

/// Peak opacity of fully-covered sky. Deliberately under 1.0 — even heavy
/// cloud lets some ground colour through at this scale, and leaving a little
/// keeps the terrain we spent so long on from vanishing entirely.
const MAX_OPACITY: f64 = 0.86;

/// Domain-warp amplitude for the cloud field, in sphere units. This is what
/// turns fBm blobs into something with the sheared, banded look of weather.
const WARP: f64 = 0.18;

/// A world's cloud deck, sampled on the unit sphere.
pub struct CloudField {
    shape: Fbm<Simplex>,
    warp: Fbm<Simplex>,
    /// Base coverage from the UWP, before latitude banding.
    coverage: f64,
}

impl CloudField {
    /// Build a cloud field for `uwp`, or `None` for a world that can't have
    /// weather (atmosphere 0 or 1 — vacuum or trace).
    pub fn from_uwp(uwp: &Uwp, seed: u64) -> Option<Self> {
        let atmo = uwp.atmosphere();
        if atmo <= 1 {
            return None;
        }
        let coverage = coverage_for_atmosphere(atmo) * coverage_for_hydrographics(uwp.hydrographics());

        let base = seed ^ 0xC10D_5EED_A1B2_C3D4;
        let shape_seed = (base ^ (base >> 32)) as u32;
        // Frequency sets the size of a weather system. At 2.4 the coarsest
        // octave spanned ~136 texels of a 2048-wide texture and the deck read
        // as lumps of cotton; 5.5 puts the largest features nearer the
        // thousand-kilometre scale real storm systems occupy, with the finest
        // octave landing around 3 texels.
        let shape = Fbm::<Simplex>::new(shape_seed)
            .set_octaves(5)
            .set_frequency(5.5)
            .set_lacunarity(2.1)
            .set_persistence(0.55);
        // Low octave count: the warp only has to shear the deck, and it costs
        // three evaluations per sample (one per axis).
        let warp = Fbm::<Simplex>::new(shape_seed.wrapping_add(0x2545_F491))
            .set_octaves(2)
            .set_frequency(1.6)
            .set_lacunarity(2.0)
            .set_persistence(0.5);

        Some(Self {
            shape,
            warp,
            coverage,
        })
    }

    /// Cloud opacity at a unit-sphere position, in `[0, MAX_OPACITY]`.
    pub fn opacity_at(&self, p: &[f64; 3]) -> f64 {
        let target = (self.coverage * band(p[2].clamp(-1.0, 1.0).asin())).clamp(0.0, 1.0);
        if target <= 0.0 {
            return 0.0;
        }

        // Shear the sample point before reading the shape band.
        let wx = self.warp.get([p[0], p[1], p[2]]);
        let wy = self.warp.get([p[0] + 8.4, p[1] - 3.7, p[2] + 12.1]);
        let wz = self.warp.get([p[0] - 19.2, p[1] + 6.3, p[2] - 5.8]);
        let q = [p[0] + WARP * wx, p[1] + WARP * wy, p[2] + WARP * wz];

        let n = (0.5 + 0.5 * NOISE_GAIN * self.shape.get(q)).clamp(0.0, 1.0);
        // Threshold placed so roughly `target` of the sky ends up covered:
        // higher coverage pushes the cut down into the noise distribution.
        let cut = 1.0 - target;
        MAX_OPACITY * smoothstep(cut - EDGE, cut + EDGE, n)
    }
}

/// Latitude weighting of cloud cover, `lat` in radians.
///
/// Earth from orbit is unmistakably *striped*, and an unbanded cloud field
/// reads as fog rather than weather — this is the part doing most of the work.
/// Three features, all visible in any full-disc photograph: the equatorial
/// convergence zone as a bright unbroken band, the subtropical highs at
/// roughly 25–30° where the descending air is famously cloudless (every major
/// desert on Earth sits in one), and the mid-latitude storm tracks near 55°.
fn band(lat: f64) -> f64 {
    let d = lat.abs().to_degrees();
    let gauss = |mu: f64, sigma: f64| (-((d - mu) / sigma).powi(2) * 0.5).exp();
    // Baseline, plus the two wet bands. The subtropical dry zone emerges from
    // the gap between them rather than being subtracted explicitly.
    (0.42 + 0.85 * gauss(0.0, 11.0) + 0.70 * gauss(55.0, 19.0)).min(1.35)
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(uwp: &str) -> Option<CloudField> {
        CloudField::from_uwp(&Uwp::parse(uwp).unwrap(), 7)
    }

    /// Mean opacity over a near-uniform sample of the sphere, which is what
    /// "how cloudy does this world look" actually means.
    fn mean_opacity(f: &CloudField) -> f64 {
        const N: usize = 4000;
        let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
        let mut total = 0.0;
        for i in 0..N {
            // Fibonacci sphere — uniform by area, so no polar oversampling.
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / N as f64;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let theta = golden * i as f64;
            total += f.opacity_at(&[r * theta.cos(), r * theta.sin(), z]);
        }
        total / N as f64
    }

    /// Vacuum and trace atmospheres get no cloud layer at all — not a faint
    /// one. A world with no air cannot have weather, and the absence is
    /// information the viewer should be able to trust.
    #[test]
    fn airless_worlds_have_no_clouds() {
        for uwp in ["A700899-A", "A710899-A", "X000000-0"] {
            assert!(
                field(uwp).is_none(),
                "{uwp} has atmosphere <= 1 and must have no cloud field"
            );
        }
    }

    /// Cloud cover has to track the UWP, or it's decoration rather than
    /// information: thicker atmospheres cloudier than thin ones, wet worlds
    /// cloudier than dry ones under the same atmosphere.
    #[test]
    fn coverage_tracks_atmosphere_and_hydrographics() {
        let thin = mean_opacity(&field("A846899-A").unwrap());
        let standard = mean_opacity(&field("A866899-A").unwrap());
        let dense = mean_opacity(&field("A896899-A").unwrap());
        assert!(
            thin < standard && standard < dense,
            "atmosphere should drive coverage: thin={thin:.3} standard={standard:.3} dense={dense:.3}"
        );

        let desert = mean_opacity(&field("A860899-A").unwrap());
        let ocean = mean_opacity(&field("A86A899-A").unwrap());
        assert!(
            desert < ocean,
            "hydrographics should drive coverage: desert={desert:.3} ocean={ocean:.3}"
        );
    }

    /// An Earth-like world should land in a believable range rather than
    /// merely being "more than a desert" — real global cloud cover is ~0.40
    /// hemispheric-mean, and the deck is partly transparent here, so anything
    /// wildly outside this band means the thresholding drifted.
    #[test]
    fn earthlike_coverage_is_plausible() {
        let m = mean_opacity(&field("C886977-8").unwrap());
        assert!(
            (0.10..=0.55).contains(&m),
            "earth-like mean cloud opacity {m:.3} outside a plausible range"
        );
    }

    /// The banding is the thing that makes a cloud field read as weather
    /// rather than fog, so assert the structure directly: the equatorial
    /// convergence zone is cloudier than the subtropical highs at ~27°.
    #[test]
    fn latitude_bands_are_present() {
        let equator = band(0.0);
        let subtropics = band(27.0_f64.to_radians());
        let storm_track = band(55.0_f64.to_radians());
        assert!(
            equator > subtropics,
            "ITCZ ({equator:.3}) should out-cloud the subtropical highs ({subtropics:.3})"
        );
        assert!(
            storm_track > subtropics,
            "storm track ({storm_track:.3}) should out-cloud the subtropical highs ({subtropics:.3})"
        );
    }

    #[test]
    fn opacity_is_bounded_and_deterministic() {
        let f = field("C886977-8").unwrap();
        let g = field("C886977-8").unwrap();
        for i in 0..500 {
            let t = i as f64 / 500.0 * std::f64::consts::TAU;
            let p = [t.cos() * 0.6, t.sin() * 0.6, (t * 0.37).sin() * 0.8];
            let n = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            let p = [p[0] / n, p[1] / n, p[2] / n];
            let a = f.opacity_at(&p);
            assert!((0.0..=MAX_OPACITY).contains(&a), "opacity {a} out of range");
            assert_eq!(a, g.opacity_at(&p), "same seed must give the same sky");
        }
    }
}
