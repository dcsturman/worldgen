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

/// Noise units above the coverage threshold at which cloud reaches
/// [`MAX_OPACITY`].
///
/// This is what stops the deck being a binary mask. Thresholding alone gives
/// every covered texel the same opacity, so the sky comes out as flat white
/// paper cut-outs with a soft edge — the interior of a cloud is exactly as
/// opaque as its rim, which no photograph of a planet has ever shown. Ramping
/// opacity with how far the noise clears the cut makes thickness continuous:
/// the fringe of a system is a veil you can see ground through, the core is
/// solid, and everything between is between.
const THICK_SPAN: f64 = 0.17;

/// Opacity of the thinnest cloud that still counts as cloud, as a fraction of
/// [`MAX_OPACITY`]. Above zero so a wisp is visible rather than a hole.
const THIN_FLOOR: f64 = 0.34;

/// Amplitude of the fine band folded into the shape noise before
/// thresholding.
///
/// Perturbing the field *before* the cut rather than modulating opacity after
/// it does two jobs with one term: it frays the edges of a system into
/// filaments, and it punches thin patches through the middle of a thick deck.
/// Modulating afterwards would only have done the second, and would have left
/// the outline as smooth as the coarse noise that drew it.
const DETAIL_AMP: f64 = 0.20;

/// Domain-warp amplitude for the cloud field, in sphere units. This is what
/// turns fBm blobs into something with the sheared, banded look of weather.
const WARP: f64 = 0.18;

/// A world's cloud deck, sampled on the unit sphere.
pub struct CloudField {
    shape: Fbm<Simplex>,
    /// Fine band that breaks up both the outline and the interior; see
    /// [`DETAIL_AMP`].
    detail: Fbm<Simplex>,
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
        // Picks up where `shape` runs out (5.5 → ~53 across five octaves at
        // lacunarity 2.1), carrying the deck down to the scale of individual
        // cloud streets. Three octaves: this is one extra fBm evaluation on
        // every texel of a 2048x1024 texture, so it buys detail with the
        // fewest octaves that still read as fractal.
        let detail = Fbm::<Simplex>::new(shape_seed.wrapping_add(0x85EB_CA6B))
            .set_octaves(3)
            .set_frequency(13.0)
            .set_lacunarity(2.1)
            .set_persistence(0.5);
        // Low octave count: the warp only has to shear the deck, and it costs
        // three evaluations per sample (one per axis).
        let warp = Fbm::<Simplex>::new(shape_seed.wrapping_add(0x2545_F491))
            .set_octaves(2)
            .set_frequency(1.6)
            .set_lacunarity(2.0)
            .set_persistence(0.5);

        Some(Self {
            shape,
            detail,
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

        let raw = self.shape.get(q) + DETAIL_AMP * self.detail.get(q);
        let n = (0.5 + 0.5 * NOISE_GAIN * raw).clamp(0.0, 1.0);
        // Threshold placed so roughly `target` of the sky ends up covered:
        // higher coverage pushes the cut down into the noise distribution.
        let cut = 1.0 - target;
        // Two independent factors: `presence` decides whether there is cloud
        // here at all (and softens the boundary), `thickness` decides how much
        // light it stops. Multiplying them means a cloud fades in at its rim
        // *and* thins toward it, which is what gives the deck depth.
        let presence = smoothstep(cut - EDGE, cut + EDGE, n);
        let thickness = ((n - cut) / THICK_SPAN).clamp(0.0, 1.0);
        MAX_OPACITY * presence * (THIN_FLOOR + (1.0 - THIN_FLOOR) * thickness)
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

    /// Share of the sphere carrying cloud thick enough to see, which is what
    /// "cloud cover" means everywhere outside this file.
    fn fraction_covered(f: &CloudField) -> f64 {
        const N: usize = 4000;
        let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
        let mut covered = 0usize;
        for i in 0..N {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / N as f64;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let theta = golden * i as f64;
            if f.opacity_at(&[r * theta.cos(), r * theta.sin(), z]) > 0.05 * MAX_OPACITY {
                covered += 1;
            }
        }
        covered as f64 / N as f64
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
    /// merely being "more than a desert" — anything wildly outside this band
    /// means the thresholding drifted.
    ///
    /// Measured as *coverage* — the share of sky with cloud in it — not as
    /// mean opacity. The two were interchangeable while every covered texel
    /// sat at MAX_OPACITY, and stopped being so the moment thickness became
    /// continuous: mean opacity now falls when clouds get thinner even though
    /// exactly as much sky is cloudy, so it can no longer answer the question
    /// this test is asking. Coverage is also the quantity the real-world
    /// figure refers to.
    #[test]
    fn earthlike_coverage_is_plausible() {
        let f = field("C886977-8").unwrap();
        let covered = fraction_covered(&f);
        assert!(
            (0.15..=0.60).contains(&covered),
            "earth-like cloud coverage {covered:.3} outside a plausible range"
        );
        // Separately, the deck must still stop a meaningful amount of light —
        // coverage by veil so thin it's invisible would pass the check above.
        let m = mean_opacity(&f);
        assert!(
            m > 0.05,
            "earth-like mean cloud opacity {m:.3} — the deck is invisible"
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

    /// The deck must have a range of thicknesses, not two states.
    ///
    /// With a plain threshold every covered texel came out at MAX_OPACITY, so
    /// the sky read as flat white cut-outs laid on the planet — visually the
    /// same failure as the paint-by-numbers terrain this whole effort exists
    /// to remove. Assert that a good share of the cloud is genuinely partial:
    /// see-through enough to show ground, opaque enough to be cloud.
    #[test]
    fn cloud_thickness_varies_rather_than_being_a_binary_mask() {
        const N: usize = 20_000;
        let f = field("C886977-8").unwrap();
        let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
        let (mut cloudy, mut partial, mut solid) = (0usize, 0usize, 0usize);
        for i in 0..N {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / N as f64;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let theta = golden * i as f64;
            let a = f.opacity_at(&[r * theta.cos(), r * theta.sin(), z]);
            // Below 5% of peak is clear sky, not thin cloud.
            if a > 0.05 * MAX_OPACITY {
                cloudy += 1;
                if a < 0.85 * MAX_OPACITY {
                    partial += 1;
                } else {
                    solid += 1;
                }
            }
        }
        assert!(cloudy > 0, "world should have some cloud at all");
        let share = partial as f64 / cloudy as f64;
        assert!(
            share > 0.5,
            "only {:.0}% of cloud is partially transparent ({partial} partial, \
             {solid} solid) — the deck is behaving like a binary mask",
            share * 100.0
        );
        // ...and it must still reach real opacity somewhere, or the clouds
        // are haze rather than weather.
        assert!(
            solid > 0,
            "no cloud anywhere reaches near-peak opacity — the deck is all veil"
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
