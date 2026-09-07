//! Globe-only ("from orbit") colour path.
//!
//! The flat map is a *reference document*: [`super::colormap`] paints it in
//! flat legend swatches, threshold-replaced rather than blended, so a referee
//! can read a region's biome straight off the key and the palette audit can
//! prove the key never drifts. That is exactly the right call there — and
//! exactly the wrong one on a globe.
//!
//! Nobody measures anything on a spinning sphere. A globe is a *picture*, and
//! the tell that ours was a picture of a map was the paint-by-numbers look:
//! continent-sized regions of one flat tan meeting one flat grey along a hard
//! edge. So this module trades legend fidelity for continuity while keeping
//! the world recognisably the same world:
//!
//! 1. **Blended, not stepped.** The same [`super::colormap`] palette
//!    constants, laid out as control points on a `(temperature, humidity)`
//!    grid and interpolated, so a biome boundary is a gradient tens of
//!    kilometres wide instead of a one-texel cliff.
//! 2. **Mottled.** A coarse noise band perturbs the humidity/temperature a
//!    texel classifies at, breaking flat regions into patches; a fine band
//!    jitters brightness for surface grain. See [`super::noise::DetailField`].
//! 3. **A real ocean ramp.** Depth drives a continuous shelf→abyss gradient
//!    instead of the flat/deep two-colour threshold, which is what produced
//!    the staircased cyan fringe around every island.
//! 4. **A photographic tone curve.** Desaturate, pull the exposure down, and
//!    run a mild S-curve — orbital photography is far lower-chroma than a
//!    cartographic palette.
//!
//! Nothing here feeds the legend, so unlike `colormap` this module is free to
//! name colours the key doesn't ([`C_SHELF`], [`C_ABYSS`]) and to emit
//! arbitrary intermediates.

use super::colormap::{
    C_DESERT_RED, C_DESERT_SAND, C_GRASSLAND, C_ICE_CAP, C_JUNGLE, C_ROCKY_HIGHLAND,
    C_SANDY_HIGHLAND, C_SAVANNA, C_SEA_ICE, C_SNOW, C_STEPPE, C_STONE, C_TAIGA, C_TEMPERATE_FOREST,
    C_TEMPERATE_RAINFOREST, C_TROP_SEASONAL_FOREST, C_TUNDRA, ICE_TEMP,
};

/// Working colour type: unclamped f64 RGB, 0–255 per channel.
type Rgb = (f64, f64, f64);

// ---- Globe-only palette additions ----------------------------------------

/// Continental-shelf water: what the ocean fades *to* at the coast. Much
/// less saturated than the flat map's `C_SHALLOW_OCEAN` — from orbit a
/// shelf reads as pale grey-teal, not swimming-pool cyan.
pub const C_SHELF: (u8, u8, u8) = (58, 104, 128);
/// Open-ocean abyss. Deep water absorbs almost everything; the flat map's
/// `C_DEEP_OCEAN` is a legible cartographic blue, this is a photographic one.
///
/// "Photographic" is not the same as "dark", which is the trap this constant
/// fell into once already. Most of a water world's ocean sits at full abyss
/// (measured: 67–83% of ocean texels on typical worlds), so this colour *is*
/// the ocean — it isn't a rare extreme reached in the middle of a basin. Set
/// it too low and the planet reads as a lit continent floating on black.
/// Deep water photographs as a saturated navy, not as absence of light: the
/// tone curve below takes this to roughly (28, 54, 86) on screen, which is
/// where open ocean actually lands in orbital photography.
pub const C_ABYSS: (u8, u8, u8) = (26, 62, 104);

/// Depth (in above-sea-level units, so negative elevation) at which the
/// shelf gradient has fully saturated to [`C_ABYSS`]. Wide enough that a
/// basin keeps a visible gradient across it rather than clipping to a flat
/// sheet a few texels off the coast.
///
/// Sized against the actual depth distribution rather than guessed: ocean
/// depth runs to about 1.0 with a median near 0.5, so at 0.20 roughly
/// two-thirds of the water clipped to a single flat colour and all the
/// bathymetry was crammed into a narrow coastal band. This spans enough of
/// the distribution to keep a visible shelf without turning the open ocean
/// pale — the `powf` in [`ocean_color`] still biases the ramp toward deep.
const SHELF_DEPTH: f64 = 0.32;

// ---- Biome control grid --------------------------------------------------

/// One humidity control point within a temperature row.
type Stop = (f64, (u8, u8, u8));

/// Biome palette as control points on a `(temperature, humidity)` grid.
/// Rows are ordered by their temperature anchor; within a row, stops are
/// ordered by humidity. Colours are interpolated along humidity inside a
/// row and between rows across temperature, so every biome the flat map
/// steps between is reachable here as a continuum.
///
/// The anchors sit at the *centre* of each band `colormap::biome_color`
/// switches on, so a texel deep inside a biome still paints that biome's
/// legend swatch — only the boundaries change.
const ROWS: &[(f64, &[Stop])] = &[
    // Polar. One stop: ice doesn't care about humidity.
    (0.08, &[(0.0, C_ICE_CAP)]),
    // Cold: dry tundra → boreal taiga.
    (0.24, &[(0.12, C_TUNDRA), (0.55, C_TAIGA)]),
    // Temperate: steppe → grassland → forest → rainforest.
    (
        0.46,
        &[
            (0.18, C_STEPPE),
            (0.50, C_GRASSLAND),
            (0.69, C_TEMPERATE_FOREST),
            (0.90, C_TEMPERATE_RAINFOREST),
        ],
    ),
    // Hot: dryland red → desert sand → savanna → seasonal forest → jungle.
    //
    // The extra driest stop is the one place this grid adds a biome the flat
    // map effectively doesn't have. `colormap` reaches `C_DESERT_RED` only via
    // a near-zero lerp at the extreme, so a hyd-0 world paints as one flat
    // beige ball — and a single hue over an entire disc is the most artificial
    // thing a planet can do. Real drylands are iron-red about as often as they
    // are sand.
    (
        0.78,
        &[
            (0.0, C_DESERT_RED),
            (0.16, C_DESERT_SAND),
            (0.37, C_SAVANNA),
            (0.60, C_TROP_SEASONAL_FOREST),
            (0.85, C_JUNGLE),
        ],
    ),
];

// ---- Detail-band gains ---------------------------------------------------

/// How far the coarse band may swing the humidity a texel classifies at.
/// This is the knob that turns a flat region into a patchwork; too high and
/// biomes stop tracking the climate model at all.
const MOTTLE_HUMIDITY: f64 = 0.11;
/// Same for temperature. Deliberately smaller — temperature is strongly
/// latitudinal and mottling it hard would produce tropical islands in the
/// arctic.
const MOTTLE_TEMP: f64 = 0.035;
/// Brightness swing from the fine band, on land. Applied per channel with a
/// slight warm bias so grain reads as sunlit/shadowed ground rather than as
/// a grey wash.
const GRAIN_LAND: (f64, f64, f64) = (0.15, 0.125, 0.09);
/// Ditto over water, where there is nothing to catch the light.
const GRAIN_SEA: f64 = 0.045;

// ---- Tone curve ----------------------------------------------------------

/// Chroma retained by the final tone curve. Orbital photography is far
/// less saturated than a cartographic palette.
const SATURATION: f64 = 0.78;
/// Overall exposure multiplier — the reference look is a stop or so down.
const EXPOSURE: f64 = 0.95;
/// Strength of the filmic S-curve blended over the linear response.
const CONTRAST: f64 = 0.16;

// ---- Entry point ---------------------------------------------------------

/// Map one texel to its surface colour for the globe.
///
/// `elev_above_sea` is signed (negative is ocean depth), `temp` and
/// `humidity` are in [0, 1], and `mottle`/`grain` are the two detail bands
/// from [`super::noise::DetailField::sample`] (roughly [-1, 1]).
///
/// This is the globe-side counterpart to [`super::colormap::elevation_color`]
/// and takes the same climate inputs, so both projections agree about where
/// the deserts are — they just disagree about whether a desert has an edge.
pub fn surface_color(
    elev_above_sea: f64,
    temp: f64,
    humidity: f64,
    mottle: f64,
    grain: f64,
) -> (u8, u8, u8) {
    // The coarse band perturbs *what the texel is*, before classification.
    let temp = (temp + mottle * MOTTLE_TEMP).clamp(0.0, 1.0);
    let humidity = (humidity + mottle * MOTTLE_HUMIDITY).clamp(0.0, 1.0);

    let (base, is_sea) = if elev_above_sea < 0.0 {
        (ocean_color(-elev_above_sea, temp, mottle), true)
    } else {
        // Nudge the elevation the overlays see so snow lines and scree
        // edges wander instead of following a perfect contour.
        let elev = elev_above_sea + mottle * 0.03;
        let base = biome_blend(temp, humidity);
        let base = apply_rock(base, elev, temp, humidity);
        (apply_snow(base, elev, temp, humidity), false)
    };

    // The fine band modulates brightness only.
    let lit = if is_sea {
        let m = 1.0 + grain * GRAIN_SEA;
        (base.0 * m, base.1 * m, base.2 * m)
    } else {
        (
            base.0 * (1.0 + grain * GRAIN_LAND.0),
            base.1 * (1.0 + grain * GRAIN_LAND.1),
            base.2 * (1.0 + grain * GRAIN_LAND.2),
        )
    };

    clamp_rgb(tone(lit))
}

// ---- Biome blending ------------------------------------------------------

/// Interpolate the control grid at `(temp, humidity)`.
fn biome_blend(temp: f64, humidity: f64) -> Rgb {
    let first = ROWS[0];
    let last = ROWS[ROWS.len() - 1];
    if temp <= first.0 {
        return row_color(first.1, humidity);
    }
    if temp >= last.0 {
        return row_color(last.1, humidity);
    }
    // Find the bracketing rows. `smoothstep` rather than a raw lerp so each
    // band keeps a wide core of its own colour and transitions quickly in
    // between — a straight lerp leaves most of the planet mid-blend.
    let hi = ROWS.iter().position(|(t, _)| *t > temp).unwrap();
    let (t0, s0) = ROWS[hi - 1];
    let (t1, s1) = ROWS[hi];
    let w = smoothstep(t0, t1, temp);
    lerp3(row_color(s0, humidity), row_color(s1, humidity), w)
}

/// Piecewise-linear interpolation along one row's humidity stops, clamped
/// at both ends.
fn row_color(stops: &[Stop], humidity: f64) -> Rgb {
    if stops.len() == 1 || humidity <= stops[0].0 {
        return to_f64(stops[0].1);
    }
    let last = stops[stops.len() - 1];
    if humidity >= last.0 {
        return to_f64(last.1);
    }
    let hi = stops.iter().position(|(h, _)| *h > humidity).unwrap();
    let (h0, c0) = stops[hi - 1];
    let (h1, c1) = stops[hi];
    lerp3(to_f64(c0), to_f64(c1), (humidity - h0) / (h1 - h0))
}

// ---- Elevation overlays --------------------------------------------------

/// Exposed rock on high ground, blended in over a band rather than
/// threshold-replaced. Hot, dry highlands weather to sand; everything else
/// to grey, and the highest ground fades toward bare stone.
///
/// The hot-and-dry case also pulls toward `C_DESERT_RED`, which the flat map
/// reaches only at its extreme: real drylands are iron-red far more often
/// than they are uniform sand, and the hue break is most of what stops a
/// desert world reading as a single beige ball.
fn apply_rock(base: Rgb, elev: f64, temp: f64, humidity: f64) -> Rgb {
    let arid = smoothstep(0.45, 0.20, humidity) * smoothstep(0.45, 0.62, temp);
    let rock = lerp3(
        to_f64(C_ROCKY_HIGHLAND),
        lerp3(to_f64(C_SANDY_HIGHLAND), to_f64(C_DESERT_RED), 0.45),
        arid,
    );
    let out = lerp3(base, rock, smoothstep(0.30, 0.46, elev));
    // Bare stone on the true summits, at partial strength so the biome tint
    // still shows through.
    lerp3(out, to_f64(C_STONE), smoothstep(0.55, 0.85, elev) * 0.6)
}

/// Snow on cold, high, *wet* ground. Same three-way gate as the flat map's
/// `apply_snow_overlay` — dry mountains stay rocky — but each condition is a
/// soft ramp, so the snow line is a fade instead of a contour line.
fn apply_snow(base: Rgb, elev: f64, temp: f64, humidity: f64) -> Rgb {
    let cold = smoothstep(0.58, 0.34, temp);
    let high = smoothstep(0.42, 0.62, elev);
    let wet = smoothstep(0.30, 0.48, humidity);
    lerp3(base, to_f64(C_SNOW), cold * high * wet)
}

// ---- Ocean ---------------------------------------------------------------

/// Continuous shelf→abyss ramp, plus a sea-ice blend at freezing latitudes.
///
/// The exponent biases the gradient toward deep water: shelves are narrow in
/// reality, so a linear ramp over the same depth range leaves far too much of
/// the ocean pale. `mottle` also jitters the freezing point, which gives the
/// ice edge a ragged margin instead of a latitude circle.
fn ocean_color(depth: f64, temp: f64, mottle: f64) -> Rgb {
    let d = (depth / SHELF_DEPTH).clamp(0.0, 1.0).powf(0.8);
    let water = lerp3(to_f64(C_SHELF), to_f64(C_ABYSS), d);
    let icy = smoothstep(ICE_TEMP + 0.05, ICE_TEMP - 0.03, temp + mottle * 0.04);
    lerp3(water, to_f64(C_SEA_ICE), icy)
}

// ---- Tone curve ----------------------------------------------------------

/// Desaturate, expose down, and apply a mild filmic S-curve.
///
/// Applied last, to land and sea alike, so the whole disc shares one
/// response. Cheap enough to be free next to the noise sampling, and on its
/// own it accounts for a good share of the "photo, not diagram" difference.
fn tone(c: Rgb) -> Rgb {
    let luma = 0.2126 * c.0 + 0.7152 * c.1 + 0.0722 * c.2;
    let desat = (
        luma + (c.0 - luma) * SATURATION,
        luma + (c.1 - luma) * SATURATION,
        luma + (c.2 - luma) * SATURATION,
    );
    (
        curve(desat.0 * EXPOSURE),
        curve(desat.1 * EXPOSURE),
        curve(desat.2 * EXPOSURE),
    )
}

/// Per-channel S-curve: blend the linear response toward a smoothstep,
/// which lifts contrast in the midtones while rolling off both ends.
fn curve(v: f64) -> f64 {
    let x = (v / 255.0).clamp(0.0, 1.0);
    let s = x * x * (3.0 - 2.0 * x);
    (x + (s - x) * CONTRAST) * 255.0
}

// ---- Helpers -------------------------------------------------------------

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp3(a: Rgb, b: Rgb, t: f64) -> Rgb {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

fn to_f64(c: (u8, u8, u8)) -> Rgb {
    (c.0 as f64, c.1 as f64, c.2 as f64)
}

fn clamp_rgb(c: Rgb) -> (u8, u8, u8) {
    (
        c.0.clamp(0.0, 255.0).round() as u8,
        c.1.clamp(0.0, 255.0).round() as u8,
        c.2.clamp(0.0, 255.0).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max_channel_delta(a: (u8, u8, u8), b: (u8, u8, u8)) -> i32 {
        let d = |x: u8, y: u8| (x as i32 - y as i32).abs();
        d(a.0, b.0).max(d(a.1, b.1)).max(d(a.2, b.2))
    }

    /// The whole point of this module: no cliffs.
    ///
    /// Sweep `(temp, humidity)` in small steps with the detail bands held at
    /// zero and measure the largest single-step colour change. The bound is
    /// calibrated, not arbitrary: neighbouring legend swatches are 40–120 LSB
    /// apart, so a threshold-replace shows up as a step in that range, while
    /// the steepest *ramp* here (ice cap fading to tundra over a 0.16-wide
    /// temperature band) moves ~8 LSB per step. Anything under ~20 is a
    /// gradient; anything over 40 is a cliff.
    ///
    /// The same sweep is run against `colormap::elevation_color` and asserted
    /// to *fail* the bound, which is what gives this test teeth — it proves
    /// the sweep is fine-grained enough to catch a step if one existed, and
    /// documents the deliberate difference between the two paths.
    #[test]
    fn biome_transitions_are_continuous() {
        const STEP: f64 = 0.005;
        const GRADIENT: i32 = 20;
        const CLIFF: i32 = 40;

        let sweep = |f: &dyn Fn(f64, f64) -> (u8, u8, u8)| -> i32 {
            let mut worst = 0;
            for i in 0..=200 {
                let v = i as f64 * STEP;
                for j in 0..200 {
                    let u = j as f64 * STEP;
                    worst = worst
                        .max(max_channel_delta(f(v, u), f(v, u + STEP)))
                        .max(max_channel_delta(f(u, v), f(u + STEP, v)));
                }
            }
            worst
        };

        let orbital = sweep(&|t, h| surface_color(0.10, t, h, 0.0, 0.0));
        assert!(
            orbital <= GRADIENT,
            "globe colormap has a {orbital}-LSB step — that reads as a hard \
             biome boundary, which is the paint-by-numbers look this module \
             exists to remove"
        );

        let flat = sweep(&|t, h| super::super::colormap::elevation_color(0.10, t, h));
        assert!(
            flat >= CLIFF,
            "the flat map's threshold-replace colormap stepped only {flat} LSB \
             across this sweep — either it stopped quantizing (in which case \
             this test no longer proves anything) or the sweep is too coarse"
        );
    }

    /// Ocean darkens monotonically with depth, with no step at the coast —
    /// the staircased cyan fringe was the most artificial thing in the old
    /// globe.
    #[test]
    fn ocean_darkens_smoothly_with_depth() {
        let mut prev = surface_color(-0.0001, 0.5, 0.5, 0.0, 0.0);
        for i in 1..=200 {
            let depth = i as f64 * 0.002;
            let c = surface_color(-depth, 0.5, 0.5, 0.0, 0.0);
            let luma = |p: (u8, u8, u8)| p.0 as i32 + p.1 as i32 + p.2 as i32;
            assert!(
                luma(c) <= luma(prev),
                "ocean brightened going deeper at {depth:.3}: {prev:?} -> {c:?}"
            );
            assert!(
                max_channel_delta(c, prev) <= 6,
                "depth cliff at {depth:.3}: {prev:?} -> {c:?}"
            );
            prev = c;
        }
    }

    /// Open ocean must read as deep water, not as a hole in the planet.
    ///
    /// Most of a water world's surface is at full abyss, so this colour sets
    /// the mood of the whole disc — and the globe then multiplies it by limb
    /// darkening and the terminator sweep, so anything already near-black in
    /// the albedo is black on screen across most of the visible hemisphere.
    /// The first cut of this module shipped an abyss that did exactly that.
    #[test]
    fn open_ocean_is_deep_blue_not_black() {
        // Well past SHELF_DEPTH, so this is the flat abyss colour.
        for depth in [0.35, 0.6, 1.0] {
            let c = surface_color(-depth, 0.5, 0.5, 0.0, 0.0);
            let (r, g, b) = (c.0 as i32, c.1 as i32, c.2 as i32);
            assert!(
                b >= 70,
                "abyss at depth {depth} is {c:?} — too dark to read as water                  once limb darkening is applied on top"
            );
            assert!(
                b > g && g > r,
                "abyss at depth {depth} is {c:?} — deep water must stay                  blue-dominant, not grey"
            );
            // ...and not so bright it stops being deep water.
            assert!(b <= 130, "abyss at depth {depth} is {c:?} — too pale");
        }
    }

    /// Deep inside a biome the globe should still paint recognisably that
    /// biome — blending softens the boundaries, it shouldn't repaint the
    /// world. Compared against the flat map's swatch after the same tone
    /// curve, which is the only transform applied unconditionally.
    #[test]
    fn biome_cores_track_the_flat_map() {
        /// `(temperature, humidity, expected swatch, name)`.
        type Case = (f64, f64, (u8, u8, u8), &'static str);
        let cases: &[Case] = &[
            (0.25, 0.12, C_TUNDRA, "tundra"),
            (0.25, 0.60, C_TAIGA, "taiga"),
            (0.46, 0.18, C_STEPPE, "steppe"),
            (0.46, 0.50, C_GRASSLAND, "grassland"),
            (0.78, 0.12, C_DESERT_SAND, "desert"),
            (0.78, 0.85, C_JUNGLE, "jungle"),
        ];
        for (temp, hum, swatch, name) in cases {
            let got = surface_color(0.05, *temp, *hum, 0.0, 0.0);
            let want = clamp_rgb(tone(to_f64(*swatch)));
            assert!(
                max_channel_delta(got, want) <= 12,
                "{name}: globe painted {got:?}, toned legend swatch is {want:?}"
            );
        }
    }

    /// The detail bands must perturb, not dominate.
    ///
    /// Bounding only the worst case turns out to be the wrong test: the
    /// biggest legitimate swing is a texel on the ice margin, where a full
    /// `MOTTLE_TEMP` deflection carries it from ice cap to tundra — and since
    /// `C_ICE_CAP` is by far the brightest swatch, that alone is ~90 LSB. It
    /// is also exactly the effect we want (a ragged snow line rather than a
    /// latitude circle), so the worst case is bounded loosely, at roughly half
    /// the full palette span (`C_ICE_CAP` to `C_JUNGLE`, ~195 LSB) — detail
    /// may push a texel into a neighbouring biome, never across the palette.
    ///
    /// The mean carries the real assertion, and is measured at `TYPICAL`
    /// rather than at full deflection: `±1` is the extreme tail of an fBm, so
    /// averaging the corners would describe a planet that doesn't exist. What
    /// has to stay small is the deflection of an ordinary texel, which is what
    /// distinguishes "textured" from "noise wash".
    #[test]
    fn detail_bands_stay_a_perturbation() {
        /// Amplitude a 4-octave fBm sample actually spends most of its time
        /// inside, as a fraction of the nominal [-1, 1] range.
        const TYPICAL: f64 = 0.35;

        let mut worst = 0;
        let mut total = 0i64;
        let mut n = 0i64;
        for ti in 0..=20 {
            for hi in 0..=20 {
                let (t, h) = (ti as f64 / 20.0, hi as f64 / 20.0);
                let plain = surface_color(0.10, t, h, 0.0, 0.0);
                for (m, g) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                    worst = worst.max(max_channel_delta(plain, surface_color(0.10, t, h, m, g)));
                    let typical = surface_color(0.10, t, h, m * TYPICAL, g * TYPICAL);
                    total += max_channel_delta(plain, typical) as i64;
                    n += 1;
                }
            }
        }
        let mean = total as f64 / n as f64;
        assert!(
            worst <= 100,
            "detail swamped the base colour somewhere: {worst} LSB at full \
             deflection is over half the palette span"
        );
        assert!(
            mean <= 15.0,
            "detail is a wash, not a texture: mean deflection {mean:.1} LSB"
        );
    }
}
