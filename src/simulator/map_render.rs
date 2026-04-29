//! Build a TravellerMap URL that visualizes a route taken during a
//! simulation run.
//!
//! We use the `/api/tile` endpoint, which returns a static PNG of a
//! rectangular region. Tile coordinates are computed from the route's
//! map-space centroid using TravellerMap's documented centring
//! formula:
//!
//! ```text
//! tile_x = ( map_center_x * scale - width  / 2) / width
//! tile_y = (-map_center_y * scale - height / 2) / height
//! ```
//!
//! Map-space (`hex_to_map_space`) is in parsecs measured from
//! Reference (Core 0140); the Tile API takes the same scale value
//! (pixels/parsec) but expresses location in tile-space.
//!
//! We use the Tile API rather than the iframe-based main page because
//! the main page silently ignores `scale` when the URL also names a
//! sector or hex, and even with `?p=x!y!logScale` was observed to fall
//! back to the galactic view in our embed. The Tile API has no SPA
//! URL-state machinery — what you ask for is what gets rendered.
//!
//! Sector world-space origins are hardcoded for the most common
//! sectors. For unknown sectors `hex_to_map_space` returns `None` and
//! the caller falls back to a sector-named link instead of an image.

use std::f64::consts::FRAC_PI_6;

/// `cos(30°)` — horizontal stretch factor for hex columns.
fn cos30() -> f64 {
    FRAC_PI_6.cos()
}

/// World-space `(sx, sy)` origin for sectors we hardcode. Verified against
/// `https://travellermap.com/api/coordinates?sector=NAME`.
pub fn sector_world_origin(sector: &str) -> Option<(i32, i32)> {
    let key: String = sector
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    match key.as_str() {
        "spinwardmarches" => Some((-4, -1)),
        "deneb" => Some((-3, -1)),
        "core" => Some((0, 0)),
        "trojanreach" => Some((-4, 0)),
        "reaversdeep" => Some((-4, 1)),
        "gvurrdon" => Some((-4, -2)),
        "tuglikki" => Some((-3, -2)),
        "corridor" => Some((-2, -1)),
        "vland" => Some((-1, -1)),
        "lishun" => Some((-1, 0)),
        "antares" => Some((0, -1)),
        "ilelish" => Some((-1, 1)),
        "fornast" => Some((1, 0)),
        "massilia" => Some((1, 1)),
        "diaspora" => Some((2, 1)),
        _ => None,
    }
}

/// Convert a (sector, hex) into TravellerMap map-space `(x, y)`.
/// Returns `None` if the sector isn't in the hardcoded table.
pub fn hex_to_map_space(sector: &str, hex_x: i32, hex_y: i32) -> Option<(f64, f64)> {
    let (sx, sy) = sector_world_origin(sector)?;
    let world_x = sx * 32 + (hex_x - 1);
    let world_y = sy * 40 + (hex_y - 40);
    let ix = world_x as f64 - 0.5;
    let iy = if world_x.rem_euclid(2) == 0 {
        world_y as f64 - 0.5
    } else {
        world_y as f64
    };
    Some((ix * cos30(), -iy))
}

/// One stop on the route map.
#[derive(Debug, Clone)]
pub struct MapWaypoint {
    /// Sector this hex belongs to.
    pub sector: String,
    /// Sector-relative hex column.
    pub hex_x: i32,
    /// Sector-relative hex row.
    pub hex_y: i32,
    /// CSS color name or RGB hex (e.g. `"green"`, `"%2351A9EE"`).
    pub color: &'static str,
}

/// Image dimensions for the rendered tile. Picked to match the size of
/// the route-map slot in the simulator UI: roughly 1000×563 (16:9).
pub const TILE_IMAGE_WIDTH: u32 = 1000;
pub const TILE_IMAGE_HEIGHT: u32 = 563;

/// Rendering options bit field for the Tile API.
///
/// 887 = NamesMajor + BordersMinor + BordersMajor + SectorsSelected +
///       SubsectorGrid + SectorGrid + WorldsCapitals + WorldsHomeworlds.
/// Same options used by the worked example in the TravellerMap docs.
const TILE_OPTIONS: u32 = 887;

/// Pick a `scale` (pixels-per-parsec) that frames every waypoint within
/// the rendered tile, leaving a few parsecs of margin. Defaults to 64
/// (subsector-ish) and zooms out from there if the route is too wide.
fn pick_scale(waypoints_map_space: &[(f64, f64)]) -> u32 {
    if waypoints_map_space.len() < 2 {
        return 64;
    }
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &(x, y) in waypoints_map_space {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    // Span in parsecs, padded so the outermost worlds aren't right on
    // the edge of the image.
    let span_x = (max_x - min_x).max(1.0) + 4.0;
    let span_y = (max_y - min_y).max(1.0) + 4.0;
    let scale_x = TILE_IMAGE_WIDTH as f64 / span_x;
    let scale_y = TILE_IMAGE_HEIGHT as f64 / span_y;
    let scale = scale_x.min(scale_y).round() as i32;
    // Clamp: below 16 the labels disappear, above 128 the route fills
    // the frame so tightly there's no surrounding context.
    scale.clamp(16, 128) as u32
}

/// Convert a map-space point into the tile-space coordinates expected
/// by the `/api/tile` endpoint. The formula centres the requested
/// region on `(map_x, map_y)` for a tile of `(width, height)` at the
/// given `scale` (pixels/parsec).
fn map_to_tile_centred(map_x: f64, map_y: f64, scale: u32, width: u32, height: u32) -> (f64, f64) {
    let s = scale as f64;
    let w = width as f64;
    let h = height as f64;
    let tx = (map_x * s - w / 2.0) / w;
    let ty = (-map_y * s - h / 2.0) / h;
    (tx, ty)
}

/// Everything the route-map component needs to draw the map plus an
/// overlaid path. The `<img src=image_url>` is the static rendered
/// tile; the SVG drawn on top uses `width`/`height` as its viewBox and
/// `waypoints_px` for circle/polyline coordinates.
pub struct RouteMapData {
    /// URL for the `<img>` tag — points at TravellerMap's Tile API.
    pub image_url: String,
    /// Per-waypoint pixel positions in the rendered image. Same length
    /// and order as the input waypoints. `None` entries correspond to
    /// waypoints whose sector wasn't in the hardcoded origin table —
    /// those simply get skipped from the overlay.
    pub waypoints_px: Vec<Option<(f64, f64)>>,
    /// Image width (also the SVG viewBox width).
    pub width: u32,
    /// Image height (also the SVG viewBox height).
    pub height: u32,
}

/// Build the [`RouteMapData`] needed to render the map + path overlay.
/// Returns `None` when no waypoint has a known sector — the caller
/// should fall back to [`build_plain_link_url`].
pub fn build_route_map_data(waypoints: &[MapWaypoint]) -> Option<RouteMapData> {
    // Collect map-space positions, preserving per-input slot so we can
    // produce pixel positions in the same order for the SVG overlay.
    let mut map_coords: Vec<Option<(f64, f64)>> = Vec::with_capacity(waypoints.len());
    let mut known: Vec<(f64, f64)> = Vec::new();
    for wp in waypoints {
        match hex_to_map_space(&wp.sector, wp.hex_x, wp.hex_y) {
            Some(p) => {
                map_coords.push(Some(p));
                known.push(p);
            }
            None => map_coords.push(None),
        }
    }
    if known.is_empty() {
        return None;
    }

    let cx = known.iter().map(|(x, _)| x).sum::<f64>() / known.len() as f64;
    let cy = known.iter().map(|(_, y)| y).sum::<f64>() / known.len() as f64;
    let scale = pick_scale(&known);
    let (tx, ty) = map_to_tile_centred(cx, cy, scale, TILE_IMAGE_WIDTH, TILE_IMAGE_HEIGHT);

    let image_url = format!(
        "https://travellermap.com/api/tile?x={:.4}&y={:.4}&w={}&h={}&scale={}&options={}&style=poster",
        tx, ty, TILE_IMAGE_WIDTH, TILE_IMAGE_HEIGHT, scale, TILE_OPTIONS,
    );

    // Pixel position of a map-space (mx, my) in our rendered image.
    // Map-space y is rimward-positive, but on the rendered tile rimward
    // is *down* (increasing pixel y), so the y term is `cy - my`.
    let s = scale as f64;
    let half_w = TILE_IMAGE_WIDTH as f64 / 2.0;
    let half_h = TILE_IMAGE_HEIGHT as f64 / 2.0;
    let waypoints_px: Vec<Option<(f64, f64)>> = map_coords
        .into_iter()
        .map(|opt| {
            opt.map(|(mx, my)| ((mx - cx) * s + half_w, (cy - my) * s + half_h))
        })
        .collect();

    Some(RouteMapData {
        image_url,
        waypoints_px,
        width: TILE_IMAGE_WIDTH,
        height: TILE_IMAGE_HEIGHT,
    })
}

/// Fallback link URL — used when the sector isn't in the hardcoded
/// map-space table. Returns a clickable URL to the main TravellerMap
/// page, not an image suitable for `<img>`.
pub fn build_plain_link_url(waypoint: &MapWaypoint) -> String {
    let hex_str = format!("{:02}{:02}", waypoint.hex_x, waypoint.hex_y);
    format!(
        "https://travellermap.com/?sector={}&hex={}&style=poster",
        url_encode(&waypoint.sector),
        hex_str,
    )
}

/// Minimal URL-component encoder. Mirrors `world_fetch::urlencode`.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
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
    fn regina_world_space_matches_api() {
        // Regina is Spinward Marches hex 1910. The TravellerMap
        // /api/coordinates endpoint returns world (x=-110, y=-70) for
        // this — we verified this against the live API.
        let (sx, sy) = sector_world_origin("Spinward Marches").unwrap();
        let world_x = sx * 32 + (19 - 1);
        let world_y = sy * 40 + (10 - 40);
        assert_eq!(world_x, -110);
        assert_eq!(world_y, -70);
    }

    #[test]
    fn regina_map_space_around_minus_95_70() {
        let (mx, my) = hex_to_map_space("Spinward Marches", 19, 10).unwrap();
        // ix = -110.5, iy = -70.5 (world_x is even).
        // x_map = -110.5 * cos(30°) ≈ -95.696
        // y_map = -(-70.5) = 70.5
        assert!((mx - (-95.696)).abs() < 0.01, "got mx={}", mx);
        assert!((my - 70.5).abs() < 0.01, "got my={}", my);
    }

    #[test]
    fn unknown_sector_returns_none() {
        assert!(hex_to_map_space("Wibblesector", 1, 1).is_none());
    }

    #[test]
    fn build_route_map_data_uses_tile_endpoint() {
        let wps = vec![
            MapWaypoint {
                sector: "Spinward Marches".to_string(),
                hex_x: 19,
                hex_y: 10,
                color: "green",
            },
            MapWaypoint {
                sector: "Spinward Marches".to_string(),
                hex_x: 20,
                hex_y: 11,
                color: "blue",
            },
        ];
        let data = build_route_map_data(&wps).unwrap();
        assert!(data.image_url.contains("/api/tile"));
        assert!(data.image_url.contains(&format!("w={}", TILE_IMAGE_WIDTH)));
        assert!(data.image_url.contains(&format!("h={}", TILE_IMAGE_HEIGHT)));
        assert_eq!(data.waypoints_px.len(), 2);
        assert!(data.waypoints_px[0].is_some());
        assert!(data.waypoints_px[1].is_some());
    }

    #[test]
    fn waypoint_pixel_position_centers_centroid() {
        // For a single waypoint, the centroid is that waypoint, so it
        // should land at the image centre.
        let wps = vec![MapWaypoint {
            sector: "Spinward Marches".to_string(),
            hex_x: 19,
            hex_y: 10,
            color: "green",
        }];
        let data = build_route_map_data(&wps).unwrap();
        let (px, py) = data.waypoints_px[0].unwrap();
        assert!((px - (TILE_IMAGE_WIDTH as f64 / 2.0)).abs() < 0.01, "px={}", px);
        assert!((py - (TILE_IMAGE_HEIGHT as f64 / 2.0)).abs() < 0.01, "py={}", py);
    }

    #[test]
    fn waypoint_pixel_y_increases_for_rimward_hex() {
        // Two waypoints — Regina (1910) and a hex 5 rows further
        // rimward (191510). The rimward one should have a *larger*
        // pixel_y because rimward is "down" on the rendered tile.
        let wps = vec![
            MapWaypoint {
                sector: "Spinward Marches".to_string(),
                hex_x: 19,
                hex_y: 10,
                color: "green",
            },
            MapWaypoint {
                sector: "Spinward Marches".to_string(),
                hex_x: 19,
                hex_y: 15,
                color: "blue",
            },
        ];
        let data = build_route_map_data(&wps).unwrap();
        let (_, py0) = data.waypoints_px[0].unwrap();
        let (_, py1) = data.waypoints_px[1].unwrap();
        assert!(py1 > py0, "expected rimward hex below coreward; got {} vs {}", py1, py0);
    }

    #[test]
    fn map_to_tile_matches_documented_example() {
        // Per TravellerMap docs: Regina at scale=64 in a 256×256 tile
        // is centred at (x ≈ -24.5, y ≈ -18). Our map-space for Regina
        // is (-95.696, 70.5).
        let (mx, my) = hex_to_map_space("Spinward Marches", 19, 10).unwrap();
        let (tx, ty) = map_to_tile_centred(mx, my, 64, 256, 256);
        assert!((tx - (-24.424)).abs() < 0.01, "got tx={}", tx);
        assert!((ty - (-18.125)).abs() < 0.01, "got ty={}", ty);
    }

    #[test]
    fn pick_scale_zooms_in_for_tight_routes() {
        // Two waypoints at adjacent hexes — should pick a high scale.
        let coords = vec![(-95.0, 70.0), (-94.0, 70.5)];
        let s = pick_scale(&coords);
        assert!(s >= 64, "expected zoomed-in scale, got {}", s);
    }

    #[test]
    fn pick_scale_zooms_out_for_wide_routes() {
        // Two waypoints 30 parsecs apart — should pick a low scale.
        let coords = vec![(-100.0, 70.0), (-70.0, 100.0)];
        let s = pick_scale(&coords);
        assert!(s < 64, "expected zoomed-out scale, got {}", s);
    }

    #[test]
    fn pick_scale_clamped_for_single_waypoint() {
        let coords = vec![(-95.0, 70.0)];
        assert_eq!(pick_scale(&coords), 64);
    }

    #[test]
    fn build_returns_none_when_no_known_waypoints() {
        let wps = vec![MapWaypoint {
            sector: "Wibblesector".to_string(),
            hex_x: 1,
            hex_y: 1,
            color: "blue",
        }];
        assert!(build_route_map_data(&wps).is_none());
    }

    #[test]
    fn sector_origin_keys_are_whitespace_insensitive() {
        assert_eq!(
            sector_world_origin("Spinward Marches"),
            sector_world_origin("SpinwardMarches")
        );
        assert_eq!(
            sector_world_origin("spinward marches"),
            sector_world_origin("Spinward Marches")
        );
    }

    #[test]
    fn url_encode_handles_spaces() {
        assert_eq!(url_encode("Spinward Marches"), "Spinward%20Marches");
        assert_eq!(url_encode("Regina"), "Regina");
    }
}
