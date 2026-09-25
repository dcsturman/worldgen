//! Tests for the tidally locked climate model ([`ClimateModel::TideLocked`]).
//!
//! These check SPEC's acceptance case — concentric zones around the
//! substellar point, ice at the antistellar point rather than the poles,
//! liquid water only once the cold trap is full, settlements in the
//! terminator ring — against generated maps, since those are properties of
//! the whole pipeline rather than of any one function.

use super::biome::Biome;
use super::climate::ClimateModel;
use super::features::Feature;
use super::*;

fn locked() -> WorldDecorations {
    WorldDecorations::tide_locked(TideLock::default())
}

fn locked_at(lat: f64, lon: f64) -> WorldDecorations {
    WorldDecorations::tide_locked(TideLock {
        substellar: Some(LatLon::from_degrees(lat, lon).unwrap()),
    })
}

/// θ of every hex in degrees, in hex order.
fn hex_thetas(map: &WorldMap) -> Vec<f64> {
    map.grid
        .hexes
        .iter()
        .map(|h| map.climate.theta(&h.sphere_pos).unwrap().to_degrees())
        .collect()
}

fn is_water(b: Biome) -> bool {
    matches!(b, Biome::DeepOcean | Biome::ShallowOcean)
}

/// Print a per-θ-bin census of a locked map. Diagnostic only:
/// `cargo test --lib worldmap::tide_locked::census -- --ignored --nocapture`.
#[test]
#[ignore]
fn census() {
    for uwp in [
        "C530677-8",
        "C540677-8",
        "A788899-A",
        "C886977-8",
        "A78A799-A",
        "A300077-A",
    ] {
        for seed in [1u64, 2] {
            let map = generate_decorated(uwp, seed, None, &locked()).unwrap();
            let th = hex_thetas(&map);
            eprintln!(
                "--- {uwp} seed {seed} sea_level {:.3} {:?}",
                map.sea_level, map.climate
            );
            for (lo, hi) in [
                (0., 30.),
                (30., 70.),
                (70., 105.),
                (105., 140.),
                (140., 181.),
            ] {
                let idx: Vec<usize> = (0..th.len())
                    .filter(|&i| th[i] >= lo && th[i] < hi)
                    .collect();
                let n = idx.len() as f64;
                let t: f64 = idx
                    .iter()
                    .map(|&i| map.grid.hexes[i].temperature)
                    .sum::<f64>()
                    / n;
                let mut counts = std::collections::BTreeMap::<String, usize>::new();
                for &i in &idx {
                    *counts
                        .entry(format!("{:?}", map.grid.hexes[i].biome))
                        .or_default() += 1;
                }
                let cities = idx
                    .iter()
                    .filter(|&&i| {
                        map.grid.hexes[i]
                            .features
                            .iter()
                            .any(|f| matches!(f, Feature::City { .. }))
                    })
                    .count();
                eprintln!("  [{lo:>3},{hi:>3}) n={n:>3} T={t:.3} cities={cities} {counts:?}");
            }
        }
    }
}

/// Write locked sample renders to /tmp for visual inspection:
/// `cargo test --lib worldmap::tide_locked::dump_tide_locked_samples -- --ignored --nocapture`.
#[test]
#[ignore]
fn dump_tide_locked_samples() {
    let cases = [
        ("thin-dry", "C530677-8", locked()),
        ("thin4-dry", "C540677-8", locked()),
        ("garden", "A788899-A", locked()),
        ("earth", "C886977-8", locked()),
        ("waterworld", "A78A799-A", locked()),
        ("vacuum", "A300077-A", locked()),
        ("hyd1", "C541677-8", locked()),
        ("hyd3", "C543677-8", locked()),
        ("vertex", "C540677-8", locked_at(26.57, 36.0)),
        ("edge", "C540677-8", locked_at(0.0, 18.0)),
        ("pole", "C540677-8", locked_at(90.0, 0.0)),
    ];
    for (name, uwp, deco) in cases {
        let map = generate_decorated(uwp, 1, None, &deco).unwrap();
        let svg = render_svg(&map);
        let png = render_png(&map).unwrap();
        let tex = render_globe_texture(&map, TexSize::STANDARD, true).unwrap();
        let globe = render_globe_png(&map, 512, 0.0, TexSize::STANDARD).unwrap();
        std::fs::write(format!("/tmp/worldmap_tl_{name}.svg"), &svg).unwrap();
        std::fs::write(format!("/tmp/worldmap_tl_{name}.png"), &png).unwrap();
        std::fs::write(format!("/tmp/worldmap_tl_{name}_texture.png"), &tex).unwrap();
        std::fs::write(format!("/tmp/worldmap_tl_{name}_globe.png"), &globe).unwrap();
        eprintln!(
            "wrote /tmp/worldmap_tl_{name}.{{svg,png}} + _texture/_globe, uwp={uwp} deco={deco}"
        );
    }
}

// ---- Geometry ----------------------------------------------------------------

fn unit(lat: f64, lon: f64) -> [f64; 3] {
    LatLon::from_degrees(lat, lon).unwrap().to_unit_vector()
}

fn model_at(lat: f64, lon: f64) -> ClimateModel {
    let uwp = Uwp::parse("C540677-8").unwrap();
    ClimateModel::from_decorations(&locked_at(lat, lon), &uwp, 1)
}

#[test]
fn theta_is_the_great_circle_angle_from_the_substellar_point() {
    // Icosahedron vertex, mid-edge, pole, both sides of the antimeridian and
    // an arbitrary point: the grid's seams and vertices must not matter.
    for (lat, lon) in [
        (26.57, 36.0),
        (-26.57, 36.0),
        (0.0, 18.0),
        (90.0, 0.0),
        (0.0, 180.0),
        (10.0, 359.99),
        (-33.0, 211.5),
    ] {
        let m = model_at(lat, lon);
        let theta = |p: [f64; 3]| m.theta(&p).unwrap().to_degrees();
        assert!(theta(unit(lat, lon)) < 0.01, "substellar ({lat},{lon})");
        assert!(
            theta(unit(-lat, lon + 180.0)) > 179.99,
            "antistellar ({lat},{lon})"
        );
        // Ten degrees due north (or south, at the north pole) is ten degrees.
        let (nlat, nlon) = if lat + 10.0 <= 90.0 {
            (lat + 10.0, lon)
        } else {
            (lat - 10.0, lon)
        };
        assert!(
            (theta(unit(nlat, nlon)) - 10.0).abs() < 0.01,
            "meridian step ({lat},{lon})"
        );
        if lat == 0.0 {
            // Along the equator θ is the longitude difference, wrapping.
            assert!((theta(unit(0.0, lon + 7.0)) - 7.0).abs() < 0.01);
            assert!((theta(unit(0.0, lon - 7.0)) - 7.0).abs() < 0.01);
            assert!((theta(unit(0.0, lon + 90.0)) - 90.0).abs() < 0.01);
        }
    }
    // Rotating worlds have no substellar point at all.
    assert!(ClimateModel::Rotating.theta(&unit(0.0, 0.0)).is_none());
}

#[test]
fn hex_theta_covers_the_sphere_wherever_the_substellar_point_is() {
    // Near a vertex, on an edge and across the seam, some hex sits close to
    // the hot spot and some close to its antipode.
    for deco in [
        locked_at(26.57, 36.0),
        locked_at(0.0, 18.0),
        locked_at(0.0, 180.0),
        locked_at(-90.0, 0.0),
    ] {
        let map = generate_decorated("C540677-8", 3, None, &deco).unwrap();
        let th = hex_thetas(&map);
        let (lo, hi) = th
            .iter()
            .fold((180.0f64, 0.0f64), |(a, b), t| (a.min(*t), b.max(*t)));
        assert!(lo < 6.0 && hi > 174.0, "{deco}: θ range {lo:.1}..{hi:.1}");
    }
}

#[test]
fn derived_substellar_point_equals_its_explicit_spelling() {
    let lon = TideLock::default().resolve(5).lon_degrees();
    let a = generate_decorated("C886977-8", 5, None, &locked()).unwrap();
    let b = generate_decorated("C886977-8", 5, None, &locked_at(0.0, lon)).unwrap();
    assert_eq!(render_svg(&a), render_svg(&b));
    // And a different seed puts the hot spot somewhere else.
    let c = generate_decorated("C886977-8", 6, None, &locked()).unwrap();
    assert_ne!(a.climate.substellar(), c.climate.substellar());
}

#[test]
fn a_lock_reclimates_the_same_terrain() {
    // Resolving the lock draws nothing from the map RNG, so the elevation
    // field — and every hex's elevation — is the unlocked world's.
    for (uwp, seed) in [("A788899-A", 1), ("C540677-8", 9)] {
        let plain = generate(uwp, seed, None).unwrap();
        let tl = generate_decorated(uwp, seed, None, &locked()).unwrap();
        let e = |m: &WorldMap| m.grid.hexes.iter().map(|h| h.elevation).collect::<Vec<_>>();
        assert_eq!(e(&plain), e(&tl), "{uwp} seed {seed}");
    }
}

// ---- Temperature ---------------------------------------------------------------

#[test]
fn temperature_falls_monotonically_from_the_substellar_point() {
    for atmo in 0..=12u8 {
        let uwp = Uwp::parse(&format!(
            "C5{}5677-8",
            crate::util::value_to_ehex(atmo as u32)
        ))
        .unwrap();
        let ClimateModel::TideLocked(tl) = ClimateModel::from_decorations(&locked(), &uwp, 1)
        else {
            panic!("locked decoration built a rotating model");
        };
        let temps: Vec<f64> = (0..=720)
            .map(|i| tl.curve((i as f64 * 0.25).to_radians()))
            .collect();
        assert!(
            temps.windows(2).all(|w| w[1] < w[0]),
            "atmo {atmo}: curve not strictly decreasing"
        );
        assert!((temps[0] - tl.t_sub).abs() < 1e-12 && (temps[720] - tl.t_night).abs() < 1e-12);
    }
}

#[test]
fn thin_atmospheres_have_the_wider_day_night_spread() {
    let spread = |uwp: &str| {
        let ClimateModel::TideLocked(tl) =
            ClimateModel::from_decorations(&locked(), &Uwp::parse(uwp).unwrap(), 1)
        else {
            unreachable!()
        };
        tl.t_sub - tl.t_night
    };
    let s: Vec<f64> = [
        "C505677-8",
        "C535677-8",
        "C565677-8",
        "C585677-8",
        "C5A5677-8",
    ]
    .iter()
    .map(|u| spread(u))
    .collect();
    assert!(s.windows(2).all(|w| w[1] < w[0]), "spreads {s:?}");
}

#[test]
fn rotating_model_is_the_latitude_model() {
    // The byte-identity pins cover this end to end; this names the contract.
    let uwp = Uwp::parse("C886977-8").unwrap();
    let tf = climate::TempField::from_uwp(&uwp, 42);
    let hf = climate::HumidityField::from_uwp(&uwp, 43);
    let m = ClimateModel::Rotating;
    for (lat, lon) in [(0.0, 0.0), (45.0, 100.0), (-80.0, 250.0), (89.0, 3.0)] {
        let p = unit(lat, lon);
        assert_eq!(
            m.surface_temperature(&p, &tf, &uwp).to_bits(),
            climate::adjust_temperature(climate::temperature_at_wobbled(&p, &tf), &uwp).to_bits()
        );
        assert_eq!(
            m.humidity(&p, &hf, &uwp).to_bits(),
            hf.sample(&p, &uwp).to_bits()
        );
        assert_eq!(
            m.above_sea(0.3, &p, 0.1, 6, &tf).to_bits(),
            climate::amplify_elevation(0.3 - 0.1, 6).to_bits()
        );
        assert_eq!(m.water_potential(0.3, &p).to_bits(), 0.3f64.to_bits());
        assert!(!m.under_ice_sheet(&p, &tf));
    }
}

// ---- Whole-map acceptance --------------------------------------------------------

/// SPEC's acceptance worlds: thin atmosphere, hydrographics 0, populated.
/// Seeds these tests run on.
///
/// Deliberately *not* 1–10. The climate constants (spread per atmosphere,
/// terminator transport, trap capacity) were tuned while these tests ran on
/// seeds 1–10, so passing there only showed the model fits the worlds it was
/// fitted to. These three were never used during tuning: if the model only
/// worked by overfitting, this is where it would show. Three rather than ten
/// also keeps the suite quick — each seed is a whole map per UWP.
const HELD_OUT_SEEDS: [u64; 3] = [11, 12, 13];

const ACCEPTANCE: [&str; 2] = ["C530677-8", "C540677-8"];

#[test]
fn zones_are_rings_around_the_substellar_point() {
    for uwp in ACCEPTANCE {
        for seed in HELD_OUT_SEEDS {
            let map = generate_decorated(uwp, seed, None, &locked()).unwrap();
            let th = hex_thetas(&map);
            let bins = [0.0, 30.0, 70.0, 105.0, 140.0, 180.1];
            let mean_t: Vec<f64> = bins
                .windows(2)
                .map(|b| {
                    let v: Vec<f64> = map
                        .grid
                        .hexes
                        .iter()
                        .zip(&th)
                        .filter(|(_, t)| (b[0]..b[1]).contains(*t))
                        .map(|(h, _)| h.temperature)
                        .collect();
                    v.iter().sum::<f64>() / v.len() as f64
                })
                .collect();
            assert!(
                mean_t.windows(2).all(|w| w[1] < w[0]),
                "{uwp} seed {seed}: bin temperatures {mean_t:?}"
            );
            // The hot spot is desert: no forest, jungle or open water.
            for (h, t) in map.grid.hexes.iter().zip(&th) {
                if *t < 30.0 {
                    assert!(
                        !matches!(
                            h.biome,
                            Biome::Jungle
                                | Biome::TemperateForest
                                | Biome::Taiga
                                | Biome::DeepOcean
                                | Biome::ShallowOcean
                        ),
                        "{uwp} seed {seed}: {:?} at θ {t:.0}",
                        h.biome
                    );
                }
            }
            // Rings, not bands: temperature varies far less within a θ band
            // than within a latitude band of the same width.
            let spread_within = |key: &dyn Fn(usize) -> f64| {
                let mut total = 0.0;
                for b in 0..9 {
                    let (lo, hi) = (b as f64 * 20.0, b as f64 * 20.0 + 20.0);
                    let v: Vec<f64> = (0..th.len())
                        .filter(|&i| (lo..hi).contains(&key(i)))
                        .map(|i| map.grid.hexes[i].temperature)
                        .collect();
                    if v.len() > 1 {
                        let m = v.iter().sum::<f64>() / v.len() as f64;
                        total += v.iter().map(|x| (x - m).powi(2)).sum::<f64>();
                    }
                }
                total
            };
            let by_theta = spread_within(&|i| th[i]);
            let by_colat = spread_within(&|i| {
                90.0 - map.grid.hexes[i].sphere_pos[2]
                    .clamp(-1.0, 1.0)
                    .asin()
                    .to_degrees()
            });
            assert!(
                by_theta * 3.0 < by_colat,
                "{uwp} seed {seed}: θ {by_theta:.2} vs lat {by_colat:.2}"
            );
        }
    }
}

#[test]
fn ice_sits_at_the_antistellar_point_not_the_poles() {
    for uwp in ["C530677-8", "C540677-8", "C541677-8", "A788899-A"] {
        for seed in HELD_OUT_SEEDS {
            let map = generate_decorated(uwp, seed, None, &locked()).unwrap();
            let th = hex_thetas(&map);
            let ice: Vec<f64> = map
                .grid
                .hexes
                .iter()
                .zip(&th)
                .filter(|(h, _)| h.biome == Biome::IceCap)
                .map(|(_, t)| *t)
                .collect();
            assert!(
                ice.iter().all(|t| *t >= 105.0),
                "{uwp} seed {seed}: ice inside θ 105: {:?}",
                ice.iter().filter(|t| **t < 105.0).collect::<Vec<_>>()
            );
            // The geographic poles sit on the terminator ring: ordinary land.
            for (h, t) in map.grid.hexes.iter().zip(&th) {
                if h.sphere_pos[2].abs() > 0.97 {
                    assert!((75.0..=105.0).contains(t));
                    assert_ne!(h.biome, Biome::IceCap, "{uwp} seed {seed}: polar ice");
                }
            }
        }
        // Where there is water to trap, the antistellar region is the sheet.
        if uwp.as_bytes()[3] != b'0' {
            for seed in HELD_OUT_SEEDS {
                let map = generate_decorated(uwp, seed, None, &locked()).unwrap();
                let far: Vec<Biome> = map
                    .grid
                    .hexes
                    .iter()
                    .zip(hex_thetas(&map))
                    .filter(|(_, t)| *t >= 165.0)
                    .map(|(h, _)| h.biome)
                    .collect();
                let iced = far.iter().filter(|b| **b == Biome::IceCap).count();
                assert!(
                    iced * 10 >= far.len() * 8,
                    "{uwp} seed {seed}: antistellar {far:?}"
                );
            }
        }
    }
}

/// Signed height above water at `n` quasi-random sphere points, through the
/// same per-pixel function the flat raster and the globe use.
fn pixel_above(map: &WorldMap, n: usize) -> Vec<([f64; 3], f64)> {
    (0..n)
        .map(|i| {
            // Fibonacci lattice: even coverage, no RNG.
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let phi = i as f64 * std::f64::consts::PI * (3.0 - 5f64.sqrt());
            let r = (1.0 - z * z).sqrt();
            let p = [r * phi.cos(), r * phi.sin(), z];
            let e = map.elev_field.sample(&p);
            let above = map.climate.above_sea(
                e,
                &p,
                map.sea_level,
                map.uwp.hydrographics(),
                &map.temp_field,
            );
            (p, above)
        })
        .collect()
}

#[test]
fn trace_water_is_all_in_the_cold_trap() {
    // Hydrographics 0–1 fits in the trap: no open water anywhere, per hex or
    // per pixel — dayside basins are dry pans.
    for uwp in ["C530677-8", "C540677-8", "C541677-8", "A781899-A"] {
        for seed in HELD_OUT_SEEDS {
            let map = generate_decorated(uwp, seed, None, &locked()).unwrap();
            assert!(
                !map.grid.hexes.iter().any(|h| is_water(h.biome)),
                "{uwp} seed {seed}: open water on a trace-water world"
            );
            let wet = pixel_above(&map, 20_000)
                .iter()
                .filter(|(_, a)| *a < 0.0)
                .count();
            assert_eq!(wet, 0, "{uwp} seed {seed}: {wet} wet pixels");
        }
    }
}

#[test]
fn liquid_water_collects_at_the_inner_terminator_once_the_trap_is_full() {
    for uwp in ["C542677-8", "C543677-8"] {
        let wet_thetas: Vec<f64> = HELD_OUT_SEEDS
            .into_iter()
            .flat_map(|seed| {
                let map = generate_decorated(uwp, seed, None, &locked()).unwrap();
                pixel_above(&map, 4_000)
                    .into_iter()
                    .filter(|(_, a)| *a < 0.0)
                    .map(|(p, _)| map.climate.theta(&p).unwrap().to_degrees())
                    .collect::<Vec<_>>()
            })
            .collect();
        let total = wet_thetas.len();
        let inner = wet_thetas
            .iter()
            .filter(|t| (55.0..=100.0).contains(*t))
            .count();
        let hot = wet_thetas.iter().filter(|t| **t < 30.0).count();
        // 55–100° is 37% of the sphere; the ring must hold most of the water.
        assert!(total > 0, "{uwp}: no liquid water at all");
        assert!(
            inner * 10 >= total * 6,
            "{uwp}: only {inner}/{total} wet samples in the ring"
        );
        // θ < 30° is 6.7% of the sphere. A deep basin there can still hold a
        // salt sea, but far less than its share of the surface.
        assert!(
            hot * 25 <= total,
            "{uwp}: {hot}/{total} wet samples at the hot spot"
        );
    }
}

#[test]
fn settlements_and_starport_stay_in_the_terminator_ring() {
    // SPEC's acceptance worlds (dry, thin air) and the awkward substellar
    // geometries: land is free across the whole ring, so the bias toward its
    // core should show at full strength.
    let dry: Vec<(&str, WorldDecorations)> = vec![
        ("C530677-8", locked()),
        ("C540677-8", locked()),
        ("C540677-8", locked_at(26.57, 36.0)),
        ("C540677-8", locked_at(0.0, 18.0)),
        ("C540677-8", locked_at(90.0, 0.0)),
    ];
    // Wet worlds: the liquid-water ring floods the core first, so high
    // populations spill onto the dry margins — still never outside 60–120°.
    // The waterworld has no land at all and exercises the floating fallback.
    let wet: Vec<(&str, WorldDecorations)> = vec![
        ("A788899-A", locked()),
        ("C886977-8", locked()),
        ("A78A799-A", locked()),
        ("A8888AA-A", locked()),
    ];
    let core_share = |cases: &[(&str, WorldDecorations)]| {
        let (mut core, mut all) = (0usize, 0usize);
        for (uwp, deco) in cases {
            for seed in HELD_OUT_SEEDS {
                let map = generate_decorated(uwp, seed, None, deco).unwrap();
                let mut starports = 0;
                let mut cities = 0;
                for (h, t) in map.grid.hexes.iter().zip(hex_thetas(&map)) {
                    for f in &h.features {
                        if let Feature::City { starport, .. } = f {
                            assert!(
                                (60.0..=120.0).contains(&t),
                                "{uwp} {deco} seed {seed}: city at θ {t:.1}"
                            );
                            cities += 1;
                            starports += *starport as usize;
                            core += (75.0..=105.0).contains(&t) as usize;
                        }
                    }
                }
                assert!(
                    cities > 0,
                    "{uwp} {deco} seed {seed}: populated world has no cities"
                );
                assert_eq!(starports, 1, "{uwp} {deco} seed {seed}");
                all += cities;
            }
        }
        (core, all)
    };
    let (core, all) = core_share(&dry);
    assert!(
        core * 10 >= all * 8,
        "dry: only {core}/{all} settlements in θ 75–105"
    );
    let (core, all) = core_share(&wet);
    assert!(
        core * 10 >= all * 6,
        "wet: only {core}/{all} settlements in θ 75–105"
    );
}

#[test]
fn hexes_and_pixels_agree_on_where_the_water_is() {
    // The per-hex biome places the cities; the pixels are what a reader sees.
    // Sample every hex centre in the flat raster and the globe texture and
    // check water hexes are drawn as water and land hexes as land.
    use super::grid::{SHEET_HEIGHT, SHEET_WIDTH};
    let map = generate_decorated("C886977-8", 1, None, &locked()).unwrap();
    let (w, h) = (SHEET_WIDTH as u32, SHEET_HEIGHT as u32);
    let flat = raster::render_terrain(&map, w, h);
    let globe = build_equirect_texture(&map, 1024, 512).baked_rgb();
    let blue = |r: u8, g: u8, b: u8| b as i32 > r as i32 + 30 && b as i32 > g as i32;
    let (mut agree_flat, mut agree_globe, mut n) = (0, 0, 0);
    for hex in &map.grid.hexes {
        if hex.biome == Biome::IceCap || hex.centers_2d.is_empty() {
            continue; // sea ice and land ice both read white
        }
        let water = is_water(hex.biome);
        let (x, y) = hex.centers_2d[0];
        let i = ((y as u32).min(h - 1) * w + (x as u32).min(w - 1)) as usize * 4;
        agree_flat += (blue(flat[i], flat[i + 1], flat[i + 2]) == water) as usize;
        let p = hex.sphere_pos;
        let lon = p[1].atan2(p[0]).rem_euclid(2.0 * std::f64::consts::PI);
        let lat = p[2].clamp(-1.0, 1.0).asin();
        let tx = ((lon / (2.0 * std::f64::consts::PI) * 1024.0) as usize).min(1023);
        let ty = (((std::f64::consts::FRAC_PI_2 - lat) / std::f64::consts::PI * 512.0) as usize)
            .min(511);
        let j = (ty * 1024 + tx) * 3;
        agree_globe += (blue(globe[j], globe[j + 1], globe[j + 2]) == water) as usize;
        n += 1;
    }
    assert!(agree_flat * 100 >= n * 90, "flat: {agree_flat}/{n}");
    assert!(agree_globe * 100 >= n * 90, "globe: {agree_globe}/{n}");
}
