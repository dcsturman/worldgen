//! Server-side TravellerMap client for the simulator.
//!
//! Candidate lookups use the `/api/jumpworlds` endpoint, which returns
//! every world within `jump` parsecs of a hex **including worlds in
//! adjacent sectors** — so a route can cross sector boundaries. Each
//! returned world carries its own sector name, sector-local hex, and
//! TravellerMap's absolute `WorldX`/`WorldY` coordinates.
//!
//! ## Absolute hex coordinates
//!
//! Cross-sector distance math needs one global grid. We use
//! `abs = (sx*32 + hx, sy*40 + hy)`, where `(sx, sy)` are the sector
//! offsets from `/api/coordinates`. Because the column shift `sx*32` is
//! always even, column parity is preserved, so `calculate_hex_distance`
//! (odd-q offset) stays exact across sector seams. TravellerMap's own
//! `WorldX`/`WorldY` relate to this grid by a fixed translation:
//! `abs = (WorldX + 1, WorldY + 40)` — verified against the live API
//! (Regina SM 1910 → WorldX/Y (-110, -70) → abs (-109, -30)).

use std::collections::HashMap;

use serde::Deserialize;

use crate::simulator::route::Candidate;
use crate::systems::world::{Facility, World};
use crate::trade::ZoneClassification;
use crate::util::calculate_hex_distance;

/// Attempts for the jumpworlds / coordinates calls before giving up. A
/// failure mid-run aborts the whole simulation, so we absorb transient
/// TravellerMap hiccups with a couple of retries.
const MAX_FETCH_ATTEMPTS: usize = 3;
/// Pause between retry attempts.
const RETRY_DELAY_MS: u64 = 500;

/// Errors fetching world data from TravellerMap.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// HTTP transport failure.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// TravellerMap returned a UWP that `World::from_uwp` rejected.
    #[error("invalid UWP from TravellerMap: {0}")]
    InvalidUwp(String),
    /// TravellerMap returned JSON we couldn't parse into our schema.
    #[error("malformed response: {0}")]
    Malformed(String),
}

/// Absolute hex from a sector offset `(sx, sy)` and a sector-local hex.
/// See the module docs for why this grid (and not raw `WorldX`/`WorldY`)
/// is the simulator's canonical coordinate space.
pub fn absolute_hex(offset: (i32, i32), hex: (i32, i32)) -> (i32, i32) {
    (offset.0 * 32 + hex.0, offset.1 * 40 + hex.1)
}

/// One world entry from the TravellerMap `/data/{sector}/{hex}` or
/// `/api/jumpworlds` endpoints (same schema).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WorldEntry {
    name: String,
    #[serde(rename = "UWP")]
    uwp: String,
    #[serde(default)]
    zone: Option<String>,
    #[serde(default)]
    allegiance: Option<String>,
    /// Population/Belts/Gas-giants code, e.g. `"709"`. The third digit is
    /// the number of gas giants — used by the pirate planner's
    /// wilderness-refuel filter.
    #[serde(default, rename = "PBG")]
    pbg: Option<String>,
    /// Base codes, e.g. `"N"` (naval), `"S"` (scout), `"A"` (naval+scout),
    /// `"D"` (depot). Mapped onto the world's facilities.
    #[serde(default)]
    bases: Option<String>,
    /// The world's own sector name (jumpworlds fills this; a neighbourhood
    /// near a boundary spans sectors).
    #[serde(default)]
    sector: Option<String>,
    /// Sector-local hex, e.g. `"1910"`.
    #[serde(default)]
    hex: Option<String>,
    /// TravellerMap absolute world coordinates. `abs = (world_x + 1,
    /// world_y + 40)` — see module docs.
    #[serde(default)]
    world_x: Option<i32>,
    #[serde(default)]
    world_y: Option<i32>,
}

/// Wrapper for world-list responses. The endpoints always return a
/// `Worlds` array — sometimes empty (treated as a 404).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WorldsEnvelope {
    worlds: Vec<WorldEntry>,
}

/// `/api/coordinates?sector=…` response (we only need the sector offsets).
#[derive(Debug, Deserialize)]
struct CoordinatesResponse {
    sx: i32,
    sy: i32,
}

/// One cached lookup result: a populated `World`, its TravellerMap
/// allegiance code (if any), and the system's gas-giant count. The
/// allegiance and gas-giant count are carried alongside `World` rather than
/// added to it so the systems-generation module stays free of
/// simulator-specific concepts. (Naval/scout bases *are* folded into the
/// `World`'s facilities, since `World` already models those.)
type CachedWorld = (World, Option<String>, u8);

/// Cache of TravellerMap lookups: single-hex world data, per-sector
/// offsets, and whole jump-neighbourhood candidate lists.
pub struct WorldCache {
    inner: HashMap<(String, i32, i32), Option<CachedWorld>>,
    /// Sector name → `(sx, sy)` offsets from `/api/coordinates`.
    sector_offsets: HashMap<String, (i32, i32)>,
    /// `(sector, hex_x, hex_y, jump)` → candidate list. The universe is
    /// static, so revisiting a hex re-serves the same neighbourhood
    /// without another network round-trip.
    jump_cache: HashMap<(String, i32, i32, i32), Vec<Candidate>>,
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
            sector_offsets: HashMap::new(),
            jump_cache: HashMap::new(),
            client,
        }
    }

    /// Look up a single world. Returns `Ok(None)` for empty hexes (404).
    /// Cached on the first lookup. The returned tuple includes the
    /// world's allegiance code (e.g. `"Im"`, `"AsT4"`).
    pub async fn fetch(
        &mut self,
        sector: &str,
        hex_x: i32,
        hex_y: i32,
    ) -> Result<Option<CachedWorld>, FetchError> {
        let key = (sector.to_string(), hex_x, hex_y);
        if let Some(cached) = self.inner.get(&key) {
            return Ok(cached.clone());
        }

        let entry = fetch_one(&self.client, sector, hex_x, hex_y).await?;
        self.inner.insert(key, entry.clone());
        Ok(entry)
    }

    /// The `(sx, sy)` offsets of a sector, from `/api/coordinates`.
    /// Cached per sector name — a run touches at most a handful.
    pub async fn sector_offset(&mut self, sector: &str) -> Result<(i32, i32), FetchError> {
        if let Some(&off) = self.sector_offsets.get(sector) {
            return Ok(off);
        }
        let url = format!(
            "{}/api/coordinates?sector={}",
            crate::util::travellermap_base_url(),
            urlencode(sector)
        );
        let body = get_with_retries(&self.client, &url).await?;
        let coords: CoordinatesResponse = serde_json::from_str(&body)
            .map_err(|e| FetchError::Malformed(format!("{}: {}", url, e)))?;
        self.sector_offsets
            .insert(sector.to_string(), (coords.sx, coords.sy));
        Ok((coords.sx, coords.sy))
    }

    /// Every world within `jump` parsecs of the given hex (excluding the
    /// hex itself), **across sector boundaries**. One `/api/jumpworlds`
    /// call; individual malformed entries are skipped so a single odd
    /// world can't sink the run.
    pub async fn candidates_within(
        &mut self,
        sector: &str,
        from_hex: (i32, i32),
        jump: i32,
    ) -> Result<Vec<Candidate>, FetchError> {
        let key = (sector.to_string(), from_hex.0, from_hex.1, jump);
        if let Some(cached) = self.jump_cache.get(&key) {
            return Ok(cached.clone());
        }

        let origin_abs = absolute_hex(self.sector_offset(sector).await?, from_hex);
        let url = format!(
            "{}/api/jumpworlds?sector={}&hex={:02}{:02}&jump={}",
            crate::util::travellermap_base_url(),
            urlencode(sector),
            from_hex.0,
            from_hex.1,
            jump
        );
        log::trace!("world_fetch: GET {}", url);
        let body = get_with_retries(&self.client, &url).await?;
        let envelope: WorldsEnvelope = serde_json::from_str(&body)
            .map_err(|e| FetchError::Malformed(format!("{}: {}", url, e)))?;

        let mut candidates: Vec<Candidate> = Vec::new();
        for entry in envelope.worlds {
            let entry_sector = entry
                .sector
                .clone()
                .unwrap_or_else(|| sector.to_string());
            let Some(hex) = entry.hex.as_deref().and_then(parse_hex) else {
                log::debug!(
                    "world_fetch: skipping {:?} in {} — missing/bad hex {:?}",
                    entry.name,
                    entry_sector,
                    entry.hex
                );
                continue;
            };
            // Absolute coords: prefer the entry's own WorldX/WorldY;
            // fall back to the sector-offset formula.
            let abs = match (entry.world_x, entry.world_y) {
                (Some(wx), Some(wy)) => (wx + 1, wy + 40),
                _ => absolute_hex(self.sector_offset(&entry_sector).await?, hex),
            };
            let d = calculate_hex_distance(origin_abs.0, origin_abs.1, abs.0, abs.1);
            if d == 0 || d > jump {
                // The origin world itself, or (belt & braces) something
                // outside jump range.
                continue;
            }
            match build_world(&entry, hex) {
                Ok((world, gas_giants)) => {
                    // Keep the single-hex cache warm too.
                    self.inner.insert(
                        (entry_sector.clone(), hex.0, hex.1),
                        Some((world.clone(), entry.allegiance.clone(), gas_giants)),
                    );
                    candidates.push(Candidate {
                        world,
                        sector: entry_sector,
                        abs,
                        distance: d,
                        allegiance: entry.allegiance,
                        gas_giants,
                    });
                }
                Err(e) => {
                    log::debug!(
                        "world_fetch: skipping {} {:02}{:02} in {} ({:?})",
                        entry.name,
                        hex.0,
                        hex.1,
                        entry_sector,
                        e
                    );
                }
            }
        }

        self.jump_cache.insert(key, candidates.clone());
        Ok(candidates)
    }
}

/// GET a URL, retrying transient failures a couple of times. Returns the
/// response body on the first 2xx.
async fn get_with_retries(client: &reqwest::Client, url: &str) -> Result<String, FetchError> {
    let mut last_err: Option<FetchError> = None;
    for attempt in 1..=MAX_FETCH_ATTEMPTS {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.text().await {
                Ok(body) => return Ok(body),
                Err(e) => last_err = Some(e.into()),
            },
            Ok(resp) => {
                last_err = Some(FetchError::Malformed(format!(
                    "{} returned status {}",
                    url,
                    resp.status()
                )));
            }
            Err(e) => last_err = Some(e.into()),
        }
        if attempt < MAX_FETCH_ATTEMPTS {
            log::debug!("world_fetch: retrying {} (attempt {})", url, attempt);
            tokio::time::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS)).await;
        }
    }
    Err(last_err.unwrap_or_else(|| FetchError::Malformed(format!("{}: no attempts made", url))))
}

/// Parse a TravellerMap 4-digit hex string (`"1910"`) into `(x, y)`.
fn parse_hex(hex: &str) -> Option<(i32, i32)> {
    if hex.len() != 4 {
        return None;
    }
    let x = hex[0..2].parse::<i32>().ok()?;
    let y = hex[2..4].parse::<i32>().ok()?;
    Some((x, y))
}

/// Build a populated `World` (plus gas-giant count) from a fetched entry.
/// `coords` are the sector-local hex, stored on `World.coordinates` to
/// match the wire-format `WorldRef` the frontend renders.
fn build_world(entry: &WorldEntry, coords: (i32, i32)) -> Result<(World, u8), FetchError> {
    let mut world = World::from_uwp(&entry.name, &entry.uwp, false, true)
        .map_err(|e| FetchError::InvalidUwp(format!("{}: {}", entry.uwp, e)))?;
    world.gen_trade_classes();
    world.coordinates = Some(coords);
    world.travel_zone = match entry.zone.as_deref() {
        Some("A") => ZoneClassification::Amber,
        Some("R") => ZoneClassification::Red,
        _ => ZoneClassification::Green,
    };

    // Map base codes onto the world's facilities (used by the pirate
    // simulator's encounter modifiers).
    if let Some(bases) = entry.bases.as_deref() {
        let mut facilities = Vec::new();
        if bases.contains('N') || bases.contains('A') {
            facilities.push(Facility::Naval);
        }
        if bases.contains('S') || bases.contains('A') {
            facilities.push(Facility::Scout);
        }
        if !facilities.is_empty() {
            world.set_facilities(facilities);
        }
    }

    // Gas giants are the third digit of the PBG code.
    let gas_giants = entry
        .pbg
        .as_deref()
        .and_then(|p| p.chars().nth(2))
        .and_then(|c| c.to_digit(16))
        .unwrap_or(0) as u8;

    Ok((world, gas_giants))
}

/// Fetch one hex from TravellerMap. Returns `Ok(None)` on 404 / empty
/// `Worlds` array, `Err` on transport or parse failure.
async fn fetch_one(
    client: &reqwest::Client,
    sector: &str,
    hex_x: i32,
    hex_y: i32,
) -> Result<Option<CachedWorld>, FetchError> {
    let hex = format!("{:02}{:02}", hex_x, hex_y);
    let encoded_sector = urlencode(sector);
    let url = format!(
        "{}/data/{}/{}",
        crate::util::travellermap_base_url(),
        encoded_sector,
        hex
    );
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

    let (world, gas_giants) = build_world(&entry, (hex_x, hex_y))?;
    Ok(Some((world, entry.allegiance, gas_giants)))
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

    #[test]
    fn parse_hex_valid_and_invalid() {
        assert_eq!(parse_hex("1910"), Some((19, 10)));
        assert_eq!(parse_hex("0140"), Some((1, 40)));
        assert_eq!(parse_hex("19"), None);
        assert_eq!(parse_hex("abcd"), None);
    }

    #[test]
    fn absolute_hex_matches_travellermap_worldxy() {
        // Regina: Spinward Marches (sx -4, sy -1) hex 1910. TravellerMap
        // reports WorldX/WorldY = (-110, -70); our grid is that plus
        // (1, 40) — verified against the live /api/coordinates endpoint.
        assert_eq!(absolute_hex((-4, -1), (19, 10)), (-109, -30));
        // Borite: Trojan Reach (sx -4, sy 0) hex 2219; WorldX/Y (-107, -21).
        assert_eq!(absolute_hex((-4, 0), (22, 19)), (-106, 19));
    }

    #[test]
    fn absolute_hex_distance_across_sector_seam() {
        // Aramis (Spinward Marches 2540, bottom row) and Labora (Trojan
        // Reach 2501, top row) sit on adjacent rows across the seam.
        let aramis = absolute_hex((-4, -1), (25, 40));
        let labora = absolute_hex((-4, 0), (25, 1));
        let d = calculate_hex_distance(aramis.0, aramis.1, labora.0, labora.1);
        assert_eq!(d, 1, "adjacent rows across the seam are 1 apart");
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
        let entry = res.unwrap();
        assert!(entry.is_some(), "Regina hex 19,10 should be present");
        let (_, allegiance, _gas) = entry.as_ref().unwrap();
        eprintln!("allegiance: {:?}", allegiance);
        assert!(
            allegiance.as_deref().unwrap_or("").starts_with("Im"),
            "Regina should be in Imperial space; got {:?}",
            allegiance
        );
        eprintln!("entry: {:?}", entry);
    }

    /// Live check that `/api/jumpworlds` crosses sector boundaries and our
    /// absolute-coordinate math agrees with TravellerMap's `WorldX`/`WorldY`.
    #[tokio::test]
    #[ignore]
    async fn jumpworlds_crosses_sector_boundary() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut cache = WorldCache::new();
        // Trojan Reach 2402 is near the coreward edge; jump 4 reaches the
        // Spinward Marches bottom rows (Aramis, Thisbe, …).
        let candidates = cache
            .candidates_within("Trojan Reach", (24, 2), 4)
            .await
            .expect("jumpworlds fetch should succeed");
        assert!(!candidates.is_empty());
        let cross: Vec<&Candidate> = candidates
            .iter()
            .filter(|c| c.sector == "Spinward Marches")
            .collect();
        assert!(
            !cross.is_empty(),
            "expected Spinward Marches worlds within jump-4 of Trojan Reach 2402"
        );
        for c in &candidates {
            assert!(
                c.distance >= 1 && c.distance <= 4,
                "{} distance {} out of range",
                c.world.name,
                c.distance
            );
        }
        // Offsets should agree with the hardcoded map table.
        assert_eq!(cache.sector_offset("Trojan Reach").await.unwrap(), (-4, 0));
        assert_eq!(
            cache.sector_offset("Spinward Marches").await.unwrap(),
            (-4, -1)
        );
    }
}
