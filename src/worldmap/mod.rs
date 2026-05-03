//! World map generator from a Traveller UWP.
//!
//! Produces an icosahedral hex-grid map laid out as a Mongoose-style
//! 5 + 10 + 5 strip (north polar, equator zigzag, south polar). Terrain is
//! generated procedurally from the UWP: 3D simplex-noise fBm sampled on the
//! unit sphere drives elevation; sea level is set by the hydrographics
//! percentile; biome is `(elevation, temperature, humidity)` with humidity
//! biased by atmosphere code. Each hex draws a dominant-biome fill plus
//! stipple sub-marks for sub-hex variation, giving an "from orbit" feel
//! while staying usable as a hex map.
//!
//! ```text
//! UWP + seed -> Grid -> elevation -> climate -> biome (+ sub-samples)
//!                                                  -> features (mountains/ice/cities)
//!                                                  -> render (SVG / PNG)
//! ```

pub mod biome;
pub mod climate;
pub mod colormap;
pub mod features;
pub mod grid;
pub mod noise;
pub mod raster;
pub mod render;
pub mod rivers;
pub mod tectonics;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Parsed UWP digits as base-16 numerics: [starport, size, atmo, hydro,
/// pop, gov, law, tech]. Starport is parsed loosely (A=10, B=11, ..., X=0).
#[derive(Clone, Copy, Debug)]
pub struct Uwp(pub [u8; 8]);

#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("UWP must be at least 8 chars (got {0:?})")]
    TooShort(String),
    #[error("UWP digit {1} at position {0} is not a valid hex digit")]
    BadDigit(usize, char),
}

impl Uwp {
    pub fn parse(uwp: &str) -> Result<Self, MapError> {
        let body: String = uwp.chars().filter(|c| *c != '-').take(8).collect();
        if body.len() < 8 {
            return Err(MapError::TooShort(uwp.to_string()));
        }
        let mut out = [0u8; 8];
        for (i, c) in body.chars().enumerate() {
            let v = match c {
                'X' => 0,
                'A'..='F' => 10 + (c as u8 - b'A'),
                'a'..='f' => 10 + (c as u8 - b'a'),
                '0'..='9' => c as u8 - b'0',
                _ => return Err(MapError::BadDigit(i, c)),
            };
            out[i] = v;
        }
        Ok(Uwp(out))
    }
    pub fn size(&self) -> u8 { self.0[1] }
    pub fn atmosphere(&self) -> u8 { self.0[2] }
    pub fn hydrographics(&self) -> u8 { self.0[3] }
    pub fn population(&self) -> u8 { self.0[4] }
    pub fn tech_level(&self) -> u8 { self.0[7] }
}

pub struct WorldMap {
    pub uwp: Uwp,
    pub seed: u64,
    pub grid: grid::Grid,
    /// Kept on the map so the rasterizer can re-sample per pixel without
    /// re-deriving the field from the seed. The elevation field internally
    /// composes its noise samples with the tectonic field, so callers don't
    /// need to mix them manually.
    pub elev_field: noise::ElevationField,
    pub humidity_field: climate::HumidityField,
    pub sea_level: f64,
    /// Major rivers as polylines in unfolded sheet coords. Computed after
    /// elevation is finalized so it sees tectonic-uplifted terrain.
    pub rivers: Vec<rivers::RiverPath>,
}

/// Generate a complete world map from a UWP and seed.
pub fn generate(uwp: &str, seed: u64) -> Result<WorldMap, MapError> {
    let uwp = Uwp::parse(uwp)?;
    let mix = uwp.0.iter().enumerate().fold(0u64, |a, (i, b)| {
        a.wrapping_add((*b as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15 ^ (i as u64)))
    });
    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ mix);

    // Derive sub-seeds up front so each pass uses an independent stream.
    let elev_seed: u64 = rng.random();
    let humidity_seed: u64 = rng.random();
    let biome_seed: u64 = rng.random();
    let feature_seed: u64 = rng.random();
    let tectonic_seed: u64 = rng.random();

    // Tectonics first — its field is owned by the elevation field so every
    // downstream sample (per-hex, per-pixel raster, sub-samples) gets the
    // tectonic uplift baked in.
    let tectonic_field = tectonics::TectonicField::from_uwp(&uwp, tectonic_seed);
    let elev_field = noise::ElevationField::from_uwp(&uwp, elev_seed)
        .with_tectonics(tectonic_field);
    let humidity_field = climate::HumidityField::from_uwp(&uwp, humidity_seed);

    let mut grid = grid::Grid::build();
    noise::compute_elevation(&mut grid, &elev_field);
    climate::compute_climate(&mut grid, &uwp, &humidity_field);

    let sea_level = biome::compute_sea_level(&grid, &uwp);
    biome::assign_biomes(
        &mut grid,
        &uwp,
        &elev_field,
        &humidity_field,
        sea_level,
        biome_seed,
    );
    features::place_features(
        &mut grid,
        &uwp,
        &mut ChaCha8Rng::seed_from_u64(feature_seed),
    );

    let mut map = WorldMap {
        uwp,
        seed,
        grid,
        elev_field,
        humidity_field,
        sea_level,
        rivers: Vec::new(),
    };
    // Rivers depend on finalized elevation, so compute last.
    map.rivers = rivers::compute_rivers(&map);
    Ok(map)
}

pub fn render_svg(map: &WorldMap) -> String {
    render::render_svg(map)
}

pub fn render_png(map: &WorldMap) -> Result<Vec<u8>, String> {
    render::render_png(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_uwp() {
        let u = Uwp::parse("A788899-A").unwrap();
        assert_eq!(u.0[0], 10); // A
        assert_eq!(u.size(), 7);
        assert_eq!(u.atmosphere(), 8);
        assert_eq!(u.hydrographics(), 8);
        assert_eq!(u.population(), 8);
        assert_eq!(u.tech_level(), 10);
    }

    #[test]
    fn end_to_end_generate_runs() {
        let map = generate("A788899-A", 0xDEADBEEF).unwrap();
        // Same shape as build()
        assert_eq!(map.grid.faces.len(), 20);
        // Some hexes should be water and some land for a Hyd-8 world.
        let water_count = map
            .grid
            .hexes
            .iter()
            .filter(|h| {
                matches!(
                    h.biome,
                    biome::Biome::DeepOcean | biome::Biome::ShallowOcean
                )
            })
            .count();
        assert!(water_count > 0 && water_count < map.grid.hexes.len());
    }

    #[test]
    fn svg_output_is_nonempty_and_well_formed() {
        let map = generate("A788899-A", 0xCAFEBABE).unwrap();
        let svg = render_svg(&map);
        assert!(svg.starts_with("<svg "));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("<polygon"));
    }

    #[test]
    fn png_output_decodes() {
        let map = generate("A788899-A", 0xFEEDFACE).unwrap();
        let bytes = render_png(&map).unwrap();
        assert!(bytes.len() > 1000);
        assert_eq!(&bytes[0..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn deterministic_for_same_seed() {
        let a = render_svg(&generate("A788899-A", 1).unwrap());
        let b = render_svg(&generate("A788899-A", 1).unwrap());
        assert_eq!(a, b);
    }

    #[test]
    fn different_seed_changes_output() {
        let a = render_svg(&generate("A788899-A", 1).unwrap());
        let b = render_svg(&generate("A788899-A", 2).unwrap());
        assert_ne!(a, b);
    }

    #[test]
    fn waterworld_is_mostly_ocean() {
        // Hydrographics is the 4th UWP digit (position 3).
        // "A78A899-A" → pos3 = 'A' = 10 (water world).
        let map = generate("A78A899-A", 1).unwrap();
        assert_eq!(map.uwp.hydrographics(), 10);
        let water = map
            .grid
            .hexes
            .iter()
            .filter(|h| {
                matches!(
                    h.biome,
                    biome::Biome::DeepOcean | biome::Biome::ShallowOcean
                )
            })
            .count();
        let frac = water as f64 / map.grid.hexes.len() as f64;
        assert!(
            frac > 0.85,
            "expected >85% water on a water world, got {frac}"
        );
    }

    /// Write sample renders to /tmp for visual inspection. Ignored by default;
    /// run with `cargo test --lib worldmap::tests::dump_samples -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_samples() {
        let cases = [
            ("garden", "A788899-A"),  // hyd 8 garden
            ("earth", "C886977-8"),   // real-Earth UWP (hyd 6)
            ("waterworld", "A78A899-A"),
            ("desert", "A780899-A"),
            ("ice", "A300077-A"),     // small + thin atmo + low pop
            ("urban", "A8888AA-A"),   // pop A
        ];
        for (name, uwp) in cases {
            let map = generate(uwp, 1).unwrap();
            let svg = render_svg(&map);
            let png = render_png(&map).unwrap();
            std::fs::write(format!("/tmp/worldmap_{name}.svg"), &svg).unwrap();
            std::fs::write(format!("/tmp/worldmap_{name}.png"), &png).unwrap();
            eprintln!(
                "wrote /tmp/worldmap_{name}.svg ({} bytes), .png ({} bytes), uwp={uwp} hex={}",
                svg.len(),
                png.len(),
                map.grid.hexes.len(),
            );
        }
    }

    /// Bucketize every visible pixel of a world by the colormap's
    /// (temp, humidity, elev) thresholds and print counts. Ignored by
    /// default; run with `cargo test --lib worldmap::tests::biome_pixel_census -- --ignored --nocapture`.
    /// Censuses both the original "garden" UWP and an Earth-equivalent
    /// UWP (C886977-8, hyd 6 = ~60% water, like real Earth) so we can
    /// see how distribution shifts when there's more land area.
    #[test]
    #[ignore]
    fn biome_pixel_census() {
        for (label, uwp) in &[
            ("garden (A788899-A, hyd 8)", "A788899-A"),
            ("Earth (C886977-8, hyd 6)", "C886977-8"),
        ] {
            census_one(label, uwp);
        }
    }

    fn census_one(label: &str, uwp: &str) {
        use grid::{SHEET_HEIGHT, SHEET_WIDTH, xy_to_sphere};

        let map = generate(uwp, 1).unwrap();
        let tec = map.elev_field.tectonics();

        let w = SHEET_WIDTH as u32;
        let h = SHEET_HEIGHT as u32;
        let mut counts = std::collections::BTreeMap::<&'static str, u64>::new();
        let bump = |c: &mut std::collections::BTreeMap<&'static str, u64>, k: &'static str| {
            *c.entry(k).or_insert(0) += 1;
        };

        let bounded: Vec<_> = map
            .grid
            .faces
            .iter()
            .flat_map(|f| f.unfolded_positions.iter().cloned().collect::<Vec<_>>())
            .collect();

        let in_silhouette = |x: f64, y: f64| -> bool {
            for tri in &bounded {
                let cx = (tri[0].0 + tri[1].0 + tri[2].0) / 3.0;
                let cy = (tri[0].1 + tri[1].1 + tri[2].1) / 3.0;
                let mut inside = true;
                for i in 0..3 {
                    let a = tri[i];
                    let b = tri[(i + 1) % 3];
                    let s = (b.0 - a.0) * (cy - a.1) - (b.1 - a.1) * (cx - a.0);
                    let p = (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0);
                    if s * p < -1e-9 {
                        inside = false;
                        break;
                    }
                }
                if inside {
                    return true;
                }
            }
            false
        };

        for py in 0..h {
            let sy = (py as f64 + 0.5) * (SHEET_HEIGHT / h as f64);
            for px in 0..w {
                let sx = (px as f64 + 0.5) * (SHEET_WIDTH / w as f64);
                if !in_silhouette(sx, sy) {
                    bump(&mut counts, "space");
                    continue;
                }
                let sphere = xy_to_sphere(sx, sy);
                let e = map.elev_field.sample(&sphere);
                let above = climate::amplify_elevation(e - map.sea_level);
                if above < 0.0 {
                    bump(&mut counts, "ocean");
                    continue;
                }
                let raw_t = climate::temperature_at(&sphere, climate::TEMP_AMPLITUDE);
                let t = climate::apply_lapse(climate::adjust_temperature(raw_t, &map.uwp), above);
                let mut hu = map.humidity_field.sample(&sphere, &map.uwp);
                if let Some(tf) = tec {
                    hu = colormap::rain_shadow_adjustment(hu, tf.rain_shadow_at(&sphere));
                }
                hu = climate::apply_altitude_drying(hu, above);
                // Match raster pipeline: dry continental interiors.
                let cont = raster::continentality(&map.elev_field, map.sea_level, sx, sy);
                hu = raster::apply_continentality(hu, cont);

                // Mirror colormap::biome_color's decision tree.
                let base = if t < 0.18 {
                    "ice"
                } else if t < 0.32 {
                    if hu < 0.32 { "tundra" } else { "taiga" }
                } else if t < 0.6 {
                    if hu < 0.40 { "steppe" }
                    else if hu < 0.60 { "grassland" }
                    else if hu < 0.78 { "temperate forest" }
                    else { "temperate rainforest" }
                } else if hu < 0.25 {
                    "desert"
                } else if hu < 0.5 {
                    "savanna"
                } else if hu < 0.7 {
                    "tropical seasonal forest"
                } else {
                    "jungle"
                };
                // Rocky/snow overlays — match the colormap thresholds in
                // apply_rocky_overlay (>=0.32) and apply_snow_overlay
                // (>=0.5 with temp<0.5).
                let label: &'static str = if above >= 0.32 {
                    if t < 0.5 && above >= 0.5 { "snowy peak" }
                    else if t >= 0.6 && hu < 0.35 { "rocky highland (sandy)" }
                    else { "rocky highland (gray)" }
                } else {
                    base
                };
                bump(&mut counts, label);
            }
        }

        let total: u64 = counts.values().sum();
        let in_sil: u64 = total - counts.get("space").copied().unwrap_or(0);
        let land: u64 = in_sil - counts.get("ocean").copied().unwrap_or(0);
        eprintln!("--- biome census, {label}, seed 1 ---");
        eprintln!("total pixels: {total}, in-silhouette: {in_sil}, land: {land}");
        let mut entries: Vec<_> = counts.iter().collect();
        entries.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        for (name, c) in entries {
            let pct_sil = 100.0 * (*c as f64) / (in_sil as f64);
            let pct_land = if matches!(*name, "ocean" | "space") {
                0.0
            } else {
                100.0 * (*c as f64) / (land.max(1) as f64)
            };
            if pct_land > 0.0 {
                eprintln!("  {name:30} {c:8}   {pct_sil:5.2}% sil   {pct_land:5.2}% land");
            } else {
                eprintln!("  {name:30} {c:8}   {pct_sil:5.2}% sil");
            }
        }
        eprintln!();
    }

    /// Honest audit: classify every rendered pixel by its distance to the
    /// nearest legend palette entry. Buckets are L1 (chebyshev) distances
    /// in 0–255 RGB space. A correctly-shared palette would put nearly all
    /// pixels in the 0 bucket; LERPs and hillshade pushes them into wider
    /// buckets. Run with:
    ///   cargo test --lib --release worldmap::tests::audit_render_pixels_against_palette \
    ///     -- --ignored --nocapture
    #[test]
    #[ignore]
    fn audit_render_pixels_against_palette() {
        use crate::worldmap::colormap::LEGEND_PALETTE;
        let map = generate("C886977-8", 1).unwrap();
        // Render at SVG resolution so we hit the same pipeline the user sees.
        let w = grid::SHEET_WIDTH as u32;
        let h = grid::SHEET_HEIGHT as u32;
        let rgba = raster::render_terrain(&map, w, h);

        let mut buckets = [0u64; 7]; // 0, ≤5, ≤10, ≤20, ≤40, ≤80, >80
        let mut total = 0u64;
        let mut worst = (0u8, 0u8, 0u8, 0i32);

        for chunk in rgba.chunks_exact(4) {
            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];
            // Skip pure space pixels (the icosahedron silhouette mask).
            if (r, g, b) == raster::SPACE_RGB {
                continue;
            }
            total += 1;
            let mut min_d = i32::MAX;
            for &(pr, pg, pb) in LEGEND_PALETTE {
                let d = (r as i32 - pr as i32).abs()
                    .max((g as i32 - pg as i32).abs())
                    .max((b as i32 - pb as i32).abs());
                if d < min_d {
                    min_d = d;
                }
            }
            let bucket = if min_d == 0 { 0 }
                else if min_d <= 5 { 1 }
                else if min_d <= 10 { 2 }
                else if min_d <= 20 { 3 }
                else if min_d <= 40 { 4 }
                else if min_d <= 80 { 5 }
                else { 6 };
            buckets[bucket] += 1;
            if min_d > worst.3 {
                worst = (r, g, b, min_d);
            }
        }

        eprintln!("--- pixel-vs-palette audit, Earth UWP, seed 1 ---");
        eprintln!("total non-space pixels: {total}");
        let labels = ["= palette  ", "≤5 LSB    ", "≤10 LSB   ", "≤20 LSB   ", "≤40 LSB   ", "≤80 LSB   ", ">80 LSB   "];
        for (i, label) in labels.iter().enumerate() {
            let c = buckets[i];
            let pct = 100.0 * c as f64 / total as f64;
            eprintln!("  {label} {c:8}  {pct:5.2}%");
        }
        eprintln!("worst pixel: rgb({}, {}, {})  distance from nearest palette = {}",
                  worst.0, worst.1, worst.2, worst.3);
    }

    #[test]
    fn desert_world_is_mostly_dry() {
        // "A780899-A" → pos3 = '0' = 0 (desert world).
        let map = generate("A780899-A", 1).unwrap();
        assert_eq!(map.uwp.hydrographics(), 0);
        let water = map
            .grid
            .hexes
            .iter()
            .filter(|h| {
                matches!(
                    h.biome,
                    biome::Biome::DeepOcean | biome::Biome::ShallowOcean
                )
            })
            .count();
        let frac = water as f64 / map.grid.hexes.len() as f64;
        assert!(frac < 0.10, "expected <10% water on a desert, got {frac}");
    }
}
