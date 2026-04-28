//! Server-side TravellerMap client for the simulator.
//!
//! In v1 the simulator stays inside one sector. To build the candidate
//! list for a single jump we enumerate every hex within `jump` parsecs
//! of the current location, fetch each from `https://travellermap.com/`,
//! and turn the populated hexes into [`Candidate`]s. Empty hexes (404
//! responses) are cached as `None` so a re-visit doesn't pay the
//! network cost again.

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::future::join_all;
use serde::Deserialize;
use tokio::sync::Semaphore;

use crate::simulator::route::Candidate;
use crate::systems::world::World;
use crate::trade::ZoneClassification;
use crate::util::calculate_hex_distance;

/// Sector-relative hex column range. TravellerMap subsectors are 8x10 each
/// and a sector is 4x4 subsectors, giving 32 columns and 40 rows.
const SECTOR_HEX_X_RANGE: std::ops::RangeInclusive<i32> = 1..=32;
/// Sector-relative hex row range. See [`SECTOR_HEX_X_RANGE`].
const SECTOR_HEX_Y_RANGE: std::ops::RangeInclusive<i32> = 1..=40;

/// Max number of concurrent TravellerMap fetches. The public service
/// resets connections aggressively when we hammer it with > ~10
/// parallel requests, so we throttle hard.
const MAX_CONCURRENT_FETCHES: usize = 4;

/// Errors fetching world data from TravellerMap.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// HTTP transport failure.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// TravellerMap returned a UWP that `World::from_upp` rejected.
    #[error("invalid UWP from TravellerMap: {0}")]
    InvalidUwp(String),
    /// TravellerMap returned JSON we couldn't parse into our schema.
    #[error("malformed response: {0}")]
    Malformed(String),
}

/// One world entry from the TravellerMap `/data/{sector}/{hex}` endpoint.
///
/// We keep our own deserialize struct here (instead of reusing the one
/// in `components/traveller_map.rs`) because that one is wired to
/// `wasm-bindgen` and lives in a frontend module agent 3 may be touching.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WorldEntry {
    name: String,
    #[serde(rename = "UWP")]
    uwp: String,
    #[serde(default)]
    zone: Option<String>,
}

/// Wrapper for `/data/{sector}/{hex}` responses. The endpoint always
/// returns a `Worlds` array — usually a single element, sometimes empty
/// (treated as a 404).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WorldsEnvelope {
    worlds: Vec<WorldEntry>,
}

/// Cache of TravellerMap world data lookups, keyed by
/// `(sector_name, hex_x, hex_y)`. `None` = empty hex (404 from
/// TravellerMap), so subsequent fetches return immediately.
pub struct WorldCache {
    inner: HashMap<(String, i32, i32), Option<World>>,
    client: reqwest::Client,
}

impl Default for WorldCache {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldCache {
    /// Build an empty cache with a fresh HTTP client.
    pub fn new() -> Self {
        // TravellerMap rejects requests without a User-Agent header
        // (returns connection-reset on the TLS handshake), so set one
        // explicitly. We also keep the connection pool small to avoid
        // tripping the public service's rate limiter.
        let client = reqwest::Client::builder()
            .user_agent("worldgen-simulator/2.0 (+https://github.com/dcsturman/worldgen)")
            .pool_max_idle_per_host(2)
            .build()
            .expect("reqwest client must build");
        Self {
            inner: HashMap::new(),
            client,
        }
    }

    /// Look up a single world. Returns `Ok(None)` for empty hexes (404).
    /// Cached on the first lookup.
    pub async fn fetch(
        &mut self,
        sector: &str,
        hex_x: i32,
        hex_y: i32,
    ) -> Result<Option<World>, FetchError> {
        let key = (sector.to_string(), hex_x, hex_y);
        if let Some(cached) = self.inner.get(&key) {
            return Ok(cached.clone());
        }

        let world = fetch_one(&self.client, sector, hex_x, hex_y).await?;
        self.inner.insert(key, world.clone());
        Ok(world)
    }

    /// Find every world within `jump` parsecs of the given hex (excluding
    /// the hex itself). All fetches run in parallel; empty hexes and any
    /// individual fetch failures are skipped silently — we'd rather make
    /// progress on the run than abort because one neighbouring hex
    /// hiccupped.
    pub async fn candidates_within(
        &mut self,
        sector: &str,
        from_hex: (i32, i32),
        jump: i32,
    ) -> Result<Vec<Candidate>, FetchError> {
        // Build the list of hexes worth fetching.
        let mut targets: Vec<(i32, i32, i32)> = Vec::new();
        for x in SECTOR_HEX_X_RANGE {
            for y in SECTOR_HEX_Y_RANGE {
                if (x, y) == from_hex {
                    continue;
                }
                let d = calculate_hex_distance(from_hex.0, from_hex.1, x, y);
                if d > 0 && d <= jump {
                    targets.push((x, y, d));
                }
            }
        }

        // Split into already-cached hits and ones that need fetching.
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut to_fetch: Vec<(i32, i32, i32)> = Vec::new();
        for (x, y, d) in targets {
            let key = (sector.to_string(), x, y);
            if let Some(cached) = self.inner.get(&key) {
                if let Some(world) = cached {
                    candidates.push(Candidate {
                        world: world.clone(),
                        distance: d,
                    });
                }
            } else {
                to_fetch.push((x, y, d));
            }
        }

        // Fetch the rest in parallel, throttled by a semaphore to avoid
        // overwhelming TravellerMap (which resets connections under
        // load).
        let client = self.client.clone();
        let sector_owned = sector.to_string();
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_FETCHES));
        let futs = to_fetch.iter().map(|&(x, y, _d)| {
            let client = client.clone();
            let sector = sector_owned.clone();
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire_owned().await.ok();
                let res = fetch_one(&client, &sector, x, y).await;
                (x, y, res)
            }
        });
        let results = join_all(futs).await;

        for ((x, y, d), (rx, ry, res)) in to_fetch.iter().zip(results) {
            debug_assert_eq!((*x, *y), (rx, ry));
            let key = (sector.to_string(), *x, *y);
            match res {
                Ok(Some(world)) => {
                    self.inner.insert(key, Some(world.clone()));
                    candidates.push(Candidate {
                        world,
                        distance: *d,
                    });
                }
                Ok(None) => {
                    self.inner.insert(key, None);
                }
                Err(e) => {
                    log::debug!(
                        "world_fetch: skipping hex {:02}{:02} in {} ({:?})",
                        x,
                        y,
                        sector,
                        e
                    );
                    // Don't cache; transient errors might recover.
                }
            }
        }

        Ok(candidates)
    }
}

/// Fetch one hex from TravellerMap. Returns `Ok(None)` on 404 / empty
/// `Worlds` array, `Err` on transport or parse failure.
async fn fetch_one(
    client: &reqwest::Client,
    sector: &str,
    hex_x: i32,
    hex_y: i32,
) -> Result<Option<World>, FetchError> {
    let hex = format!("{:02}{:02}", hex_x, hex_y);
    let encoded_sector = urlencode(sector);
    let url = format!("https://travellermap.com/data/{}/{}", encoded_sector, hex);
    log::trace!("world_fetch: GET {}", url);

    let response = client.get(&url).send().await?;
    let status = response.status();

    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        return Err(FetchError::Malformed(format!(
            "{} returned status {}",
            url, status
        )));
    }

    let body = response.text().await?;
    if body.trim().is_empty() {
        return Ok(None);
    }

    let envelope: WorldsEnvelope = serde_json::from_str(&body)
        .map_err(|e| FetchError::Malformed(format!("{}: {}", url, e)))?;
    let entry = match envelope.worlds.into_iter().next() {
        Some(e) => e,
        None => return Ok(None),
    };

    let mut world = World::from_upp(&entry.name, &entry.uwp, false, true)
        .map_err(|e| FetchError::InvalidUwp(format!("{}: {}", entry.uwp, e)))?;
    world.gen_trade_classes();
    world.coordinates = Some((hex_x, hex_y));
    world.travel_zone = match entry.zone.as_deref() {
        Some("A") => ZoneClassification::Amber,
        Some("R") => ZoneClassification::Red,
        _ => ZoneClassification::Green,
    };

    Ok(Some(world))
}

/// Minimal URL component encoder — enough to handle spaces and other
/// punctuation in sector names like `"Spinward Marches"`. We don't
/// need the full RFC 3986 set because TravellerMap sector names are
/// ASCII letters, digits, and spaces.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            // UTF-8 percent-encode.
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).as_bytes() {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_handles_spaces() {
        assert_eq!(urlencode("Spinward Marches"), "Spinward%20Marches");
        assert_eq!(urlencode("Regina"), "Regina");
        assert_eq!(urlencode("a/b"), "a%2Fb");
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_one_regina() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let _ = env_logger::Builder::from_default_env()
            .is_test(true)
            .try_init();
        let client = reqwest::Client::builder()
            .user_agent("worldgen-simulator/2.0")
            .build()
            .unwrap();
        let res = fetch_one(&client, "Spinward Marches", 19, 10).await;
        eprintln!("result: {:?}", res);
        assert!(res.is_ok());
        let world = res.unwrap();
        assert!(world.is_some(), "Regina hex 19,10 should be present");
        eprintln!("world: {:?}", world);
    }
}
