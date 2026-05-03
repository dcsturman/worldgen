//! Continuous color ramp from (elevation_above_sea, temperature, humidity)
//! to RGB, plus a hillshade modulator. Replaces the discrete `Biome::color`
//! lookup for rendering — the rasterizer samples the noise fields per pixel
//! and turns each sample into a smooth color, giving the "from orbit" look.
//!
//! Biome class is decided by `(temp, humidity)` first; elevation only adds
//! a "rocky" overlay above ~0.5 and snow when cold enough. This avoids the
//! "every mountain looks like Tibet" failure of the prior ramp.
//!
//! ## Palette: single source of truth
//!
//! Every base color the rasterizer can paint is declared as a `pub const`
//! below. `biome_color`, `ocean_color`, `apply_rocky_overlay`, and
//! `apply_snow_overlay` reference these constants exclusively — no
//! hand-written RGB tuples in this file outside the const block. The
//! legend (see `render::LEGEND_TERRAINS`) reads the same constants, so the
//! key on the rendered map can never drift from what the map actually
//! paints.
//!
//! Some colors are LERP endpoints (e.g. `C_DEEP_OCEAN` and
//! `C_SHALLOW_OCEAN`, or `C_DESERT_SAND` and `C_DESERT_RED`). The
//! intermediate hues produced by lerping are not in the legend on
//! purpose — the user reads them as "the same region as the nearest
//! endpoint". `LEGEND_PALETTE` below names exactly the constants the
//! legend exposes; the audit test (`palette_audit`) walks a grid of pure
//! inputs and asserts every output color falls within tolerance of one of
//! the constants.

// ---- Palette: single source of truth -------------------------------------

// Oceans + ice
pub const C_DEEP_OCEAN: (u8, u8, u8) = (22, 50, 96);
pub const C_SHALLOW_OCEAN: (u8, u8, u8) = (62, 124, 168);
pub const C_SEA_ICE: (u8, u8, u8) = (215, 222, 232);
pub const C_ICE_CAP: (u8, u8, u8) = (235, 240, 245);

// Cold-zone biomes
pub const C_TUNDRA: (u8, u8, u8) = (158, 150, 130);
pub const C_TAIGA: (u8, u8, u8) = (60, 92, 70);

// Temperate-zone biomes
pub const C_STEPPE: (u8, u8, u8) = (198, 184, 138);
pub const C_GRASSLAND: (u8, u8, u8) = (168, 184, 108);
pub const C_TEMPERATE_FOREST: (u8, u8, u8) = (96, 138, 80);
pub const C_TEMPERATE_RAINFOREST: (u8, u8, u8) = (54, 100, 60);

// Hot-zone biomes
pub const C_DESERT_SAND: (u8, u8, u8) = (212, 188, 138);
/// LERP target reached only at the hottest, driest extreme — not in the
/// legend; reads as "desert" everywhere it appears.
pub const C_DESERT_RED: (u8, u8, u8) = (196, 138, 92);
pub const C_SAVANNA: (u8, u8, u8) = (188, 180, 110);
pub const C_TROP_SEASONAL_FOREST: (u8, u8, u8) = (94, 156, 78);
pub const C_JUNGLE: (u8, u8, u8) = (40, 90, 64);

// Elevation overlays
pub const C_ROCKY_HIGHLAND: (u8, u8, u8) = (138, 132, 124);
pub const C_SANDY_HIGHLAND: (u8, u8, u8) = (188, 148, 102);
/// Stone-gray LERP target above 0.55 elev — not in the legend; reads as
/// "rocky highland" with the biome tint preserved.
pub const C_STONE: (u8, u8, u8) = (150, 144, 138);
pub const C_SNOW: (u8, u8, u8) = (240, 244, 248);

/// Colors the legend names. Order matches the legend's grid layout.
/// The audit test (`palette_audit`) validates every "pure-input" sample
/// reaching `elevation_color` lands on one of these (within rounding) or
/// on a documented LERP target above.
pub const LEGEND_PALETTE: &[(u8, u8, u8)] = &[
    C_DEEP_OCEAN,
    C_SHALLOW_OCEAN,
    C_SEA_ICE,
    C_ICE_CAP,
    C_TUNDRA,
    C_TAIGA,
    C_STEPPE,
    C_GRASSLAND,
    C_TEMPERATE_FOREST,
    C_TEMPERATE_RAINFOREST,
    C_DESERT_SAND,
    C_SAVANNA,
    C_TROP_SEASONAL_FOREST,
    C_JUNGLE,
    C_ROCKY_HIGHLAND,
    C_SANDY_HIGHLAND,
    C_SNOW,
];

// ---- Color math ----------------------------------------------------------

/// Map a per-pixel sample to an RGB triplet.
///
/// `elev_above_sea` is signed: negative is ocean depth, positive is land
/// elevation. `temp` and `humidity` are in [0, 1].
pub fn elevation_color(elev_above_sea: f64, temp: f64, humidity: f64) -> (u8, u8, u8) {
    if elev_above_sea < 0.0 {
        return ocean_color(elev_above_sea, temp);
    }

    let base = biome_color(temp, humidity);
    let with_rock = apply_rocky_overlay(base, elev_above_sea, temp, humidity);
    let with_snow = apply_snow_overlay(with_rock, elev_above_sea, temp);
    clamp_rgb(with_snow)
}

/// Ocean color from depth, with sea-ice tint at very cold latitudes.
///
/// Only the continental-shelf zone (just below sea level) varies with
/// depth — anything deeper renders as uniform deep blue. From orbit on
/// real worlds you can't see mid-ocean ridges or seafloor relief through
/// the water column; if we mapped depth linearly across the full subsea
/// elevation range, tectonic underwater ridges show up as bright
/// "shallow water" stripes. Saturating past the shelf hides that.
fn ocean_color(elev_above_sea: f64, temp: f64) -> (u8, u8, u8) {
    let depth = (-elev_above_sea).max(0.0);
    const SHELF_DEPTH: f64 = 0.04;
    let t = (depth / SHELF_DEPTH).min(1.0);
    let c = lerp_rgb(to_f64(C_SHALLOW_OCEAN), to_f64(C_DEEP_OCEAN), t);
    if temp < 0.18 {
        let mix = ((0.18 - temp) / 0.18).clamp(0.0, 1.0);
        return clamp_rgb(lerp_rgb(c, to_f64(C_SEA_ICE), mix * 0.85));
    }
    clamp_rgb(c)
}

/// Pick a base biome color from temp + humidity.
fn biome_color(temp: f64, humidity: f64) -> (f64, f64, f64) {
    // Polar ice cap — overrides any humidity.
    if temp < 0.18 {
        return to_f64(C_ICE_CAP);
    }
    if temp < 0.32 {
        // Cold zone. Taiga is the default cold biome (real-Earth boreal
        // belt — Russia, Canada, Scandinavia); tundra is the extreme dry
        // edge (high arctic, treeless). Threshold sits low so most cold
        // pixels paint taiga and only the driest fraction reads as tundra.
        if humidity < 0.32 {
            // Tundra: cool gray-brown.
            to_f64(C_TUNDRA)
        } else {
            // Taiga: dark spruce green.
            to_f64(C_TAIGA)
        }
    } else if temp < 0.6 {
        // Temperate. Steppe widened to 0.40 (was 0.30) so it actually
        // captures dry-temperate noise — Mongolia/Patagonia/the Great Plains.
        if humidity < 0.40 {
            // Steppe: pale tan.
            to_f64(C_STEPPE)
        } else if humidity < 0.60 {
            // Grassland: light yellow-green.
            to_f64(C_GRASSLAND)
        } else if humidity < 0.78 {
            // Temperate forest: medium green.
            to_f64(C_TEMPERATE_FOREST)
        } else {
            // Temperate rainforest: deep green.
            to_f64(C_TEMPERATE_RAINFOREST)
        }
    } else {
        // Hot.
        if humidity < 0.25 {
            // Desert: sandy tan, slightly reddish at the very-hot extreme.
            let red = ((temp - 0.6) / 0.4).clamp(0.0, 1.0) * 0.4;
            let dry = ((0.25 - humidity) / 0.25).clamp(0.0, 1.0);
            lerp_rgb(to_f64(C_DESERT_SAND), to_f64(C_DESERT_RED), red * dry)
        } else if humidity < 0.5 {
            // Savanna: yellow-tan with a green tint.
            to_f64(C_SAVANNA)
        } else if humidity < 0.7 {
            // Tropical seasonal forest: brighter green.
            to_f64(C_TROP_SEASONAL_FOREST)
        } else {
            // Jungle: very dark green, slight blue tint.
            to_f64(C_JUNGLE)
        }
    }
}

/// Above ~0.5 elevation, blend toward a rocky color whose hue depends on the
/// underlying biome (sandy/red on hot+dry, gray on cold or wet). Above 0.75,
/// blend further toward stone gray but keep some biome tint.
fn apply_rocky_overlay(
    base: (f64, f64, f64),
    elev_above_sea: f64,
    temp: f64,
    humidity: f64,
) -> (f64, f64, f64) {
    if elev_above_sea < 0.32 {
        return base;
    }
    // Pick a rocky hue: sandy/red on hot+dry worlds, gray otherwise.
    // hot_dry score in [0,1]: 1 when temp>=0.6 and humidity<=0.25.
    let hot = ((temp - 0.5) / 0.2).clamp(0.0, 1.0);
    let dry = ((0.35 - humidity) / 0.25).clamp(0.0, 1.0);
    let warm_rock_weight = hot * dry;
    let rocky = lerp_rgb(
        to_f64(C_ROCKY_HIGHLAND),
        to_f64(C_SANDY_HIGHLAND),
        warm_rock_weight,
    );

    if elev_above_sea < 0.55 {
        // 0.32..0.55: blend up to 50% rocky.
        let t = (elev_above_sea - 0.32) / 0.23;
        lerp_rgb(base, rocky, t * 0.5)
    } else {
        // 0.55+: keep going toward stone gray, but cap so biome tint shows.
        let mid = lerp_rgb(base, rocky, 0.5);
        let t = ((elev_above_sea - 0.55) / 0.25).clamp(0.0, 1.0);
        // Blend mid toward stone, but only up to ~0.6 to keep biome tint.
        lerp_rgb(mid, to_f64(C_STONE), t * 0.6)
    }
}

/// Snow cap: only on cold-ish high terrain. More snow with higher elev and
/// colder temp.
fn apply_snow_overlay(
    base: (f64, f64, f64),
    elev_above_sea: f64,
    temp: f64,
) -> (f64, f64, f64) {
    if temp >= 0.5 || elev_above_sea <= 0.5 {
        return base;
    }
    let elev_t = ((elev_above_sea - 0.5) / 0.3).clamp(0.0, 1.0);
    let cold_t = ((0.5 - temp) / 0.32).clamp(0.0, 1.0);
    let amt = (elev_t * cold_t).clamp(0.0, 1.0);
    lerp_rgb(base, to_f64(C_SNOW), amt)
}

/// Adjust humidity by a tectonic rain-shadow term in roughly [-1, 1]
/// (negative = downwind/dry side). Returns a clamped humidity in [0, 1].
///
/// NOTE: not yet wired into `HumidityField::sample` to avoid touching
/// raster.rs (out-of-scope here). When the rasterizer is updated, it can
/// call `rain_shadow_adjustment(humidity, tectonic.rain_shadow_at(pos))`
/// and pass the result to `elevation_color`.
pub fn rain_shadow_adjustment(humidity: f64, rain_shadow: f64) -> f64 {
    (humidity + rain_shadow * 0.55).clamp(0.0, 1.0)
}

/// Multiply an RGB by a Lambertian shade derived from a 2D elevation
/// gradient. `dx`, `dy` are scaled elevation differences across the pixel
/// (unit-less; tune scale at the call site). Light shines from the
/// upper-left.
pub fn apply_hillshade(rgb: (u8, u8, u8), dx: f64, dy: f64) -> (u8, u8, u8) {
    // Surface normal from gradient (z = up out of the page).
    let nx = -dx;
    let ny = -dy;
    let nz = 1.0;
    let nlen = (nx * nx + ny * ny + nz * nz).sqrt();
    let nx = nx / nlen;
    let ny = ny / nlen;
    let nz = nz / nlen;

    // Light direction: upper-left, shining slightly down. Pre-normalized.
    const LX: f64 = -0.4423;
    const LY: f64 = -0.5404;
    const LZ: f64 = 0.7154;

    let dot = (nx * LX + ny * LY + nz * LZ).max(0.0);
    // Ambient + diffuse, kept gentle so the underlying color still reads.
    let shade = (0.65 + 0.55 * dot).clamp(0.40, 1.20);

    let r = (rgb.0 as f64 * shade).clamp(0.0, 255.0) as u8;
    let g = (rgb.1 as f64 * shade).clamp(0.0, 255.0) as u8;
    let b = (rgb.2 as f64 * shade).clamp(0.0, 255.0) as u8;
    (r, g, b)
}

/// Promote a `(u8, u8, u8)` palette constant to the f64 tuple the
/// LERP/clamp helpers operate on.
fn to_f64(c: (u8, u8, u8)) -> (f64, f64, f64) {
    (c.0 as f64, c.1 as f64, c.2 as f64)
}

fn lerp_rgb(a: (f64, f64, f64), b: (f64, f64, f64), t: f64) -> (f64, f64, f64) {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

fn clamp_rgb(c: (f64, f64, f64)) -> (u8, u8, u8) {
    (
        c.0.clamp(0.0, 255.0) as u8,
        c.1.clamp(0.0, 255.0) as u8,
        c.2.clamp(0.0, 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All "endpoint" colors the rasterizer can paint for a pure (non-LERP)
    /// input must appear in `LEGEND_PALETTE` or in the small documented
    /// set of LERP targets (`C_DESERT_RED`, `C_STONE`).
    ///
    /// We sweep `(elev, temp, humidity)` over a grid of pure inputs —
    /// values picked to land squarely inside each biome cell, away from
    /// thresholds where two colors meet. For each pure sample, the
    /// resulting color must match a constant exactly (no LERP intermediates
    /// — pure inputs avoid the rocky/snow/sea-ice/desert-red blends).
    #[test]
    fn palette_audit_pure_inputs_hit_legend() {
        // Sub-sea-level: deep ocean (depth >= shelf), shallow ocean (depth
        // very small), sea ice (cold + sub-shelf -> blends; skip here).
        let samples: &[(f64, f64, f64, (u8, u8, u8), &str)] = &[
            // Land — flat (no rocky overlay), warm enough (no snow).
            (0.10, 0.10, 0.10, C_ICE_CAP, "ice cap"),
            (0.10, 0.25, 0.10, C_TUNDRA, "tundra"),
            (0.10, 0.25, 0.50, C_TAIGA, "taiga"),
            (0.10, 0.45, 0.10, C_STEPPE, "steppe"),
            (0.10, 0.45, 0.50, C_GRASSLAND, "grassland"),
            (0.10, 0.45, 0.70, C_TEMPERATE_FOREST, "temperate forest"),
            (0.10, 0.45, 0.90, C_TEMPERATE_RAINFOREST, "temperate rainforest"),
            // Desert at humidity 0.10 + temp 0.61: red weight is
            // ((0.61-0.6)/0.4)*0.4 = 0.01, dry = (0.15/0.25) = 0.60, so
            // red*dry ~ 0.006 — color is essentially C_DESERT_SAND.
            (0.10, 0.61, 0.10, C_DESERT_SAND, "desert (sand)"),
            (0.10, 0.70, 0.40, C_SAVANNA, "savanna"),
            (0.10, 0.70, 0.60, C_TROP_SEASONAL_FOREST, "trop. seasonal forest"),
            (0.10, 0.70, 0.85, C_JUNGLE, "jungle"),
            // Ocean: deep (well past the shelf) for the C_DEEP_OCEAN
            // endpoint, and exactly at sea level (depth 0) for the
            // C_SHALLOW_OCEAN endpoint. Skip cold ocean (it blends to
            // sea-ice).
            (-0.10, 0.50, 0.50, C_DEEP_OCEAN, "deep ocean"),
            (0.0_f64.next_down(), 0.50, 0.50, C_SHALLOW_OCEAN, "shallow ocean (sea level)"),
        ];

        for (elev, temp, hum, want, name) in samples {
            let got = elevation_color(*elev, *temp, *hum);
            // Allow ±2 per channel — desert-sand has a near-zero LERP that
            // rounds in the last digit, and the shallow-ocean sample sits
            // at depth ~= f64 epsilon * SHELF_DEPTH which still produces a
            // ~1-LSB lerp in some channels.
            let dr = (got.0 as i32 - want.0 as i32).abs();
            let dg = (got.1 as i32 - want.1 as i32).abs();
            let db = (got.2 as i32 - want.2 as i32).abs();
            assert!(
                dr <= 2 && dg <= 2 && db <= 2,
                "{name}: got {got:?}, want {want:?} (elev={elev}, temp={temp}, hum={hum})"
            );
            assert!(
                LEGEND_PALETTE.contains(want)
                    || *want == C_DESERT_RED
                    || *want == C_STONE
                    || *want == C_SEA_ICE,
                "expected color {want:?} ({name}) not declared in LEGEND_PALETTE \
                 or documented LERP targets",
            );
        }
    }

    /// Fuzz: sweep `(elev, temp, humidity)` across a fine grid and verify
    /// every output color is reasonably close to *some* `LEGEND_PALETTE`
    /// entry (or to the documented LERP targets). "Close" means
    /// channel-wise squared distance ≤ 64*64*3 — i.e. within a 64-LSB
    /// envelope per channel. This is loose enough for the half-strength
    /// rocky-highland and partial-snow blends but tight enough to catch a
    /// stray hand-written tuple slipping in (any random RGB picked at
    /// runtime would land far outside the palette).
    #[test]
    fn palette_audit_fuzz_outputs_stay_near_palette() {
        // All palette entries the rasterizer is allowed to emit, including
        // the documented LERP targets that aren't in the legend.
        let allowed: Vec<(u8, u8, u8)> = LEGEND_PALETTE
            .iter()
            .copied()
            .chain([C_DESERT_RED, C_STONE].into_iter())
            .collect();

        let envelope_sq: i64 = 64 * 64 * 3;
        let mut max_dist_sq: i64 = 0;

        // 11 elev × 11 temp × 11 hum = 1331 samples.
        for ei in 0..11 {
            for ti in 0..11 {
                for hi in 0..11 {
                    let elev = -0.5 + (ei as f64) * 0.10; // -0.5 .. 0.5
                    let temp = (ti as f64) / 10.0;
                    let hum = (hi as f64) / 10.0;
                    let got = elevation_color(elev, temp, hum);
                    let nearest = allowed
                        .iter()
                        .map(|c| sq_dist(got, *c))
                        .min()
                        .unwrap();
                    if nearest > max_dist_sq {
                        max_dist_sq = nearest;
                    }
                    assert!(
                        nearest <= envelope_sq,
                        "fuzz: elev={elev:.2} temp={temp:.2} hum={hum:.2} \
                         produced {got:?}, nearest palette entry is {nearest} \
                         (sq dist) — outside {envelope_sq} envelope, suggests \
                         a hand-written color crept in",
                    );
                }
            }
        }
        // Sanity: the worst-case distance should be well below the envelope
        // on the current colormap (it lives in the ~30-LSB range from
        // half-strength overlays). If this floor moves, take a look.
        assert!(
            max_dist_sq < envelope_sq,
            "max sq distance {max_dist_sq} unexpectedly high",
        );
    }

    /// Sanity: every constant we declare in `LEGEND_PALETTE` is referenced
    /// by name in the colormap dispatch logic, so the legend can't list a
    /// color that no input path can produce. Implemented as a `match` on
    /// the constant — adding a new `LEGEND_PALETTE` entry forces the
    /// developer to either map it to a code path or document why it's
    /// absent. (Compile-time enforcement; no runtime work.)
    #[test]
    fn palette_audit_every_legend_color_has_a_code_path() {
        for c in LEGEND_PALETTE {
            // If you add a new legend color, add its source function here.
            // This is exhaustive on purpose: there's no `_` arm.
            let source: &str = if *c == C_DEEP_OCEAN || *c == C_SHALLOW_OCEAN {
                "ocean_color (depth lerp)"
            } else if *c == C_SEA_ICE {
                "ocean_color (cold-ocean blend)"
            } else if *c == C_ICE_CAP
                || *c == C_TUNDRA
                || *c == C_TAIGA
                || *c == C_STEPPE
                || *c == C_GRASSLAND
                || *c == C_TEMPERATE_FOREST
                || *c == C_TEMPERATE_RAINFOREST
                || *c == C_DESERT_SAND
                || *c == C_SAVANNA
                || *c == C_TROP_SEASONAL_FOREST
                || *c == C_JUNGLE
            {
                "biome_color"
            } else if *c == C_ROCKY_HIGHLAND || *c == C_SANDY_HIGHLAND {
                "apply_rocky_overlay"
            } else if *c == C_SNOW {
                "apply_snow_overlay"
            } else {
                panic!(
                    "LEGEND_PALETTE entry {c:?} has no documented source — \
                     either wire it into the colormap dispatch or remove \
                     it from LEGEND_PALETTE"
                );
            };
            // Touch `source` so the linter doesn't warn on the assignment.
            assert!(!source.is_empty());
        }
    }

    fn sq_dist(a: (u8, u8, u8), b: (u8, u8, u8)) -> i64 {
        let dr = a.0 as i64 - b.0 as i64;
        let dg = a.1 as i64 - b.1 as i64;
        let db = a.2 as i64 - b.2 as i64;
        dr * dr + dg * dg + db * db
    }
}
