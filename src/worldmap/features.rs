//! Map features placed on top of the biome layer: cities and ice caps.
//! Population centers are sized to real-world inhabitant counts (Traveller
//! pop digit P → ~10^P inhabitants) and sized via a Zipf distribution so
//! large worlds get a few dominant cities plus a long tail. The starport
//! sits on the largest placed city and renders red.

use rand::Rng;
use rand_chacha::ChaCha8Rng;

use super::Uwp;
use super::biome::Biome;
use super::grid::Grid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    /// `starport` flags the single most important city per world; rendered red.
    City { tier: CityTier, starport: bool },
    PolarIce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CityTier {
    /// 10M+ inhabitants. Double-ring + dot.
    Megacity,
    /// 1M-10M.
    Major,
    /// 500K-1M.
    Minor,
    /// <500K. Only placed when it's the SOLE settlement on a low-pop world.
    Small,
}

pub fn place_features(grid: &mut Grid, uwp: &Uwp, rng: &mut ChaCha8Rng) {
    place_cities(grid, uwp, rng);
}

#[allow(dead_code)]
fn place_polar_ice(grid: &mut Grid) {
    // Reserved for future use. The IceCap biome already conveys the cap.
    let mut temps: Vec<f64> = grid.hexes.iter().map(|h| h.temperature).collect();
    temps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let cutoff = temps[(temps.len() as f64 * 0.05).floor() as usize];
    for hex in &mut grid.hexes {
        if hex.temperature <= cutoff {
            hex.features.push(Feature::PolarIce);
        }
    }
}

/// Generate Zipf-distributed city sizes for a given pop digit, then map each
/// to a `CityTier`. Returns sizes in descending order; index 0 is the
/// starport. Population 0 returns an empty Vec (no settlements at all).
fn city_sizes_for_pop(pop: u8, rng: &mut ChaCha8Rng) -> Vec<(CityTier, u64)> {
    if pop == 0 {
        return Vec::new();
    }
    // Pop digit P → total inhabitants roughly in [10^P, 10^(P+1)). Pick a
    // log-uniform multiplier in that decade so a "pop 6" world is 1M-10M,
    // not pinned exactly at 10M.
    let mantissa = rng.random_range(1.0..10.0_f64);
    let total = mantissa * 10f64.powi(pop as i32);
    // Urban fraction grows with pop digit: high-pop worlds clump harder.
    let urban_frac = (0.5 + 0.04 * pop as f64 + rng.random_range(0.0..0.10)).min(0.9);
    let urban = total * urban_frac;
    let alpha = 1.05_f64;

    // Truncate the Zipf tail at 500K (anything smaller is a "Small" hamlet
    // and is only allowed as the sole settlement, see fallback below). This
    // keeps high-pop worlds clumped into a handful of big cities rather than
    // a cloud of villages.
    const FLOOR: f64 = 500_000.0;
    const MAX_CITIES: usize = 32;
    let mut sizes: Vec<u64> = Vec::new();
    // Iteratively grow N: at each step compute size_n = urban / (H_N · n^alpha)
    // where H_N = sum_{k=1..N} 1/k^alpha. Adding terms shrinks earlier cities,
    // so re-evaluate full list each iteration.
    for n_total in 1..=MAX_CITIES {
        let h: f64 = (1..=n_total).map(|k| 1.0 / (k as f64).powf(alpha)).sum();
        let last = urban / (h * (n_total as f64).powf(alpha));
        if last < FLOOR && n_total > 1 {
            break;
        }
        sizes = (1..=n_total)
            .map(|k| (urban / (h * (k as f64).powf(alpha))) as u64)
            .collect();
        if last < FLOOR {
            break;
        }
    }
    // Low-pop fallback: no city reaches 500K. Keep exactly one settlement
    // (the starport host); classify will tag it Small. For pop > 0 we must
    // always have at least one settlement — that's where the starport sits.
    if sizes.is_empty() {
        let largest = (urban as u64).max(1);
        sizes = vec![largest];
    }

    sizes.into_iter().map(|s| (classify(s), s)).collect()
}

fn classify(size: u64) -> CityTier {
    if size >= 10_000_000 {
        CityTier::Megacity
    } else if size >= 1_000_000 {
        CityTier::Major
    } else if size >= 500_000 {
        CityTier::Minor
    } else {
        CityTier::Small
    }
}

fn place_cities(grid: &mut Grid, uwp: &Uwp, rng: &mut ChaCha8Rng) {
    let pop = uwp.population();
    let sizes = city_sizes_for_pop(pop, rng);
    if sizes.is_empty() {
        return;
    }

    let eligible: Vec<usize> = grid
        .hexes
        .iter()
        .enumerate()
        .filter(|(_, h)| city_weight(h.biome) > 0)
        .map(|(i, _)| i)
        .collect();
    if eligible.is_empty() {
        return;
    }

    let base_weights: Vec<u32> = eligible
        .iter()
        .map(|&i| city_weight(grid.hexes[i].biome))
        .collect();
    let mut taken: Vec<bool> = vec![false; eligible.len()];
    // Big cities (Megacity/Major) repel each other to avoid clumping; smaller
    // tiers place freely. Chord distance on the unit sphere; ~2 hexes ≈ 0.32.
    const EXCLUSION: f64 = 0.32;
    let mut anchors: Vec<[f64; 3]> = Vec::new();

    // Place largest first so the starport (index 0) lands on the best hex.
    for (rank, (tier, _size)) in sizes.iter().enumerate() {
        let needs_separation = matches!(tier, CityTier::Megacity | CityTier::Major);
        let mut weights: Vec<u32> = base_weights.clone();
        for (i, w) in weights.iter_mut().enumerate() {
            if taken[i] {
                *w = 0;
                continue;
            }
            if needs_separation {
                let pos = grid.hexes[eligible[i]].sphere_pos;
                for ap in &anchors {
                    let d = chord_dist(pos, *ap);
                    if d < EXCLUSION {
                        // Smooth 1/d² penalty: zero at d=0, full at exclusion.
                        let f = (d / EXCLUSION).clamp(0.0, 1.0);
                        *w = (*w as f64 * f * f) as u32;
                    }
                }
            }
        }
        let total: u32 = weights.iter().sum();
        if total == 0 {
            break;
        }
        let mut pick: u32 = rng.random_range(0..total);
        let mut chosen_local = 0usize;
        for (i, w) in weights.iter().enumerate() {
            if pick < *w {
                chosen_local = i;
                break;
            }
            pick -= *w;
        }
        let hex_idx = eligible[chosen_local];
        let starport = rank == 0;
        grid.hexes[hex_idx].features.push(Feature::City {
            tier: *tier,
            starport,
        });
        taken[chosen_local] = true;
        if needs_separation {
            anchors.push(grid.hexes[hex_idx].sphere_pos);
        }
    }
}

fn chord_dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn city_weight(b: Biome) -> u32 {
    match b {
        Biome::Grassland => 10,
        Biome::TemperateForest => 8,
        Biome::Steppe => 7,
        Biome::SavannaScrub => 6,
        Biome::Taiga => 4,
        Biome::Jungle => 4,
        Biome::Desert => 2,
        Biome::Tundra => 2,
        Biome::Highland => 3,
        Biome::Barren => 1,
        Biome::Mountain | Biome::IceCap => 0,
        Biome::DeepOcean | Biome::ShallowOcean => 0,
        Biome::Unassigned => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn count_tiers(pop: u8) -> (usize, usize, usize, usize) {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let sizes = city_sizes_for_pop(pop, &mut rng);
        let mut mega = 0;
        let mut major = 0;
        let mut minor = 0;
        let mut small = 0;
        for (t, _) in &sizes {
            match t {
                CityTier::Megacity => mega += 1,
                CityTier::Major => major += 1,
                CityTier::Minor => minor += 1,
                CityTier::Small => small += 1,
            }
        }
        (mega, major, minor, small)
    }

    #[test]
    fn pop_zero_has_no_settlements() {
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        assert!(city_sizes_for_pop(0, &mut rng).is_empty());
    }

    #[test]
    fn low_pop_yields_one_or_two_settlements() {
        // Pop 1-4: total 10s to 10K — should be just 1 settlement.
        for pop in 1..=4 {
            let mut rng = ChaCha8Rng::seed_from_u64(pop as u64);
            let sizes = city_sizes_for_pop(pop, &mut rng);
            assert_eq!(sizes.len(), 1, "pop {pop} should produce 1 settlement");
        }
    }

    #[test]
    fn pop_eight_has_megacities_or_majors() {
        let (mega, major, _minor, _small) = count_tiers(8);
        assert!(mega + major >= 1, "pop 8 should have at least one big city");
    }

    #[test]
    fn high_pop_has_megacities() {
        let (mega, _, _, _) = count_tiers(10);
        assert!(mega >= 1, "pop A should have megacities");
    }

    #[test]
    fn pop_six_has_a_handful_of_cities() {
        // Pop 6 should produce a handful (≤10) of cities with at least one
        // Major or Megacity dominating.
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let sizes = city_sizes_for_pop(6, &mut rng);
        assert!(!sizes.is_empty());
        assert!(sizes.len() <= 10, "pop 6 produced {} cities", sizes.len());
        assert!(matches!(sizes[0].0, CityTier::Major | CityTier::Megacity));
    }
}

