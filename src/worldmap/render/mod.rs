//! Render backends. The `Renderer` trait abstracts the few primitives we
//! need (filled polygon, filled circle, stroked line, filled rect), letting
//! us share a single overlay pass between SVG (live UI) and PNG (roll20
//! export).
//!
//! Rendering is two layers: first a per-pixel terrain raster (continuous
//! color from elevation/humidity + hillshade) is splatted in as the
//! background; then face triangle outlines, the hex grid, and feature
//! glyphs are stroked on top. The SVG path embeds the raster as a base64
//! `<image>`; the PNG path writes it directly into the pixmap.

pub mod png;
pub mod svg;

use super::WorldMap;
use super::colormap::{
    C_DEEP_OCEAN, C_GRASSLAND, C_ICE_CAP, C_JUNGLE, C_ROCKY_HIGHLAND, C_SANDY_HIGHLAND,
    C_SAVANNA, C_SEA_ICE, C_SHALLOW_OCEAN, C_SNOW, C_STEPPE, C_TAIGA, C_TEMPERATE_FOREST,
    C_TEMPERATE_RAINFOREST, C_TROP_SEASONAL_FOREST, C_TUNDRA, C_DESERT_SAND,
};
use super::features::{CityTier, Feature};
use super::grid::{
    Face, HEXES_PER_EDGE, SHEET_HEIGHT, SHEET_WIDTH, TRIANGLE_SIDE, pointy_top_hex,
};
use super::raster;

#[derive(Clone, Copy, Debug)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(r, g, b, 255)
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(r, g, b, a)
    }
    /// Const-context conversion from a `colormap::C_*` palette tuple.
    /// Lets `LEGEND_TERRAINS` reference the palette constants directly so
    /// the legend can never drift from the rendered map.
    pub const fn from_palette(c: (u8, u8, u8)) -> Self {
        Self(c.0, c.1, c.2, 255)
    }
}

pub trait Renderer {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Color);
    fn fill_polygon(&mut self, points: &[(f64, f64)], color: Color);
    fn fill_circle(&mut self, cx: f64, cy: f64, r: f64, color: Color);
    fn stroke_line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: Color, width: f64);
    fn stroke_polyline(&mut self, points: &[(f64, f64)], color: Color, width: f64);
    /// Render `text` with its baseline anchored at `(x, y)` and pixel height
    /// `size` in sheet units. Implementations are expected to do simple
    /// left-to-right layout — no shaping, no kerning beyond what the font
    /// table provides. Used for the legend strip; not a general-purpose
    /// text API.
    fn fill_text(&mut self, x: f64, y: f64, size: f64, text: &str, color: Color);
}

/// Raster resolution for the live SVG. Rendered at 1.5× sheet units so the
/// browser's bilinear interpolation has more source pixels to work with —
/// keeps biome boundaries crisp at any zoom without bringing back the
/// gradient LERPs (which produced in-between colors throughout regions).
const SVG_RASTER_SCALE: f64 = 1.5;
pub const SVG_RASTER_W: u32 = (SHEET_WIDTH * SVG_RASTER_SCALE) as u32;
pub const SVG_RASTER_H: u32 = (SHEET_HEIGHT * SVG_RASTER_SCALE) as u32;

/// PNG export raster scale. 1× matches the SVG resolution and keeps PNG
/// generation under the browser's "unresponsive page" threshold; 2× was
/// nicer for VTT use but pushed the per-pixel pipeline (warp+Voronoi+
/// continentality) into 60s+ territory and the browser would prompt the
/// user to kill the tab.
const PNG_RASTER_SCALE: f64 = 1.0;

/// Height (sheet units) of the legend strip drawn below the map. Sits in
/// the band [SHEET_HEIGHT, SHEET_HEIGHT + LEGEND_HEIGHT] for both SVG and
/// PNG outputs so the downloaded image carries the key with it.
pub const LEGEND_HEIGHT: f64 = 135.0;

pub fn render_svg(map: &WorldMap) -> String {
    let raster = raster::render_terrain(map, SVG_RASTER_W, SVG_RASTER_H);
    assemble_svg(map, &raster)
}

/// Wrap a precomputed RGBA8 raster (already at SVG_RASTER_W × SVG_RASTER_H)
/// in the rest of the SVG: PNG-encode + embed, then draw the vector
/// overlay (face outlines, hex grid, rivers, cities, title block) and the
/// legend strip. Split out from `render_svg` so callers chunking the
/// per-pixel raster across event-loop turns can run the heavy raster
/// passes separately and call this last to assemble the final string.
pub fn assemble_svg(map: &WorldMap, raster: &[u8]) -> String {
    let total_h = SHEET_HEIGHT + LEGEND_HEIGHT;
    let mut r = svg::SvgRenderer::new(SHEET_WIDTH, total_h);
    match raster_to_png(raster, SVG_RASTER_W, SVG_RASTER_H) {
        Ok(png) => r.embed_png(&png, 0.0, 0.0, SHEET_WIDTH, SHEET_HEIGHT),
        Err(_) => {
            // Fall back to a flat space-color rect so the SVG isn't empty
            // if PNG encoding fails (it really shouldn't).
            r.fill_rect(0.0, 0.0, SHEET_WIDTH, SHEET_HEIGHT, Color::rgb(8, 10, 18));
        }
    }
    draw_overlay(&mut r, map);
    draw_legend(&mut r, 0.0, SHEET_HEIGHT, SHEET_WIDTH, LEGEND_HEIGHT);
    r.into_string()
}

pub fn render_png(map: &WorldMap) -> Result<Vec<u8>, String> {
    let total_h = SHEET_HEIGHT + LEGEND_HEIGHT;
    // Raster only the map portion; the legend is drawn vector-only over a
    // solid fill so we don't waste compute rasterizing a large empty band.
    let map_pixel_w = (SHEET_WIDTH * PNG_RASTER_SCALE).ceil() as u32;
    let map_pixel_h = (SHEET_HEIGHT * PNG_RASTER_SCALE).ceil() as u32;
    let raster = raster::render_terrain(map, map_pixel_w, map_pixel_h);
    let total_pixel_h = (total_h * PNG_RASTER_SCALE).ceil() as u32;
    let mut r = png::PngRenderer::from_raster_with_extra_height(
        &raster,
        map_pixel_w,
        map_pixel_h,
        total_pixel_h,
        SHEET_WIDTH,
        total_h,
    )?;
    draw_overlay(&mut r, map);
    draw_legend(&mut r, 0.0, SHEET_HEIGHT, SHEET_WIDTH, LEGEND_HEIGHT);
    r.encode()
}

/// Encode an RGBA8 buffer to PNG using tiny_skia (also our PNG backend).
fn raster_to_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        format!("tiny_skia::Pixmap::new failed for {width}x{height}")
    })?;
    pixmap.data_mut().copy_from_slice(rgba);
    pixmap.encode_png().map_err(|e| format!("PNG encode failed: {e}"))
}

/// Vector overlay: face triangle outlines, the hex grid, rivers, and
/// feature glyphs. Drawn on top of whatever raster background the renderer
/// was initialized with.
fn draw_overlay<R: Renderer + ?Sized>(r: &mut R, map: &WorldMap) {
    let grid = &map.grid;
    // Hex grid: a single regular pointy-top tessellation laid across the
    // whole sheet, independent of the per-face data hex grid. The
    // per-face barycentric layout is generated independently per face so
    // adjacent faces' hexes don't tessellate (point-meets-point at the
    // seam instead of edge-meets-edge); the per-pixel raster has already
    // done the icosahedral inverse-mapping for terrain, so the visible
    // grid can just be a clean honeycomb. Hexes whose centers fall outside
    // every face are skipped so nothing draws over the dark "space" gaps.
    let hex_stroke = Color::rgba(20, 20, 20, 130);
    let flat = TRIANGLE_SIDE / HEXES_PER_EDGE as f64;
    let r_apex = flat / (3.0_f64).sqrt();
    let vstep = 1.5 * r_apex;
    let n_cols = (SHEET_WIDTH / flat).ceil() as i32 + 1;
    let n_rows = (SHEET_HEIGHT / vstep).ceil() as i32 + 1;
    for row in 0..n_rows {
        let y = row as f64 * vstep + r_apex;
        let row_offset = if row % 2 == 1 { flat / 2.0 } else { 0.0 };
        for col in 0..n_cols {
            let x = col as f64 * flat + flat / 2.0 + row_offset;
            if !point_in_any_face(&grid.faces, x, y) {
                continue;
            }
            let poly = pointy_top_hex((x, y), flat);
            let pts = [
                poly[0], poly[1], poly[2], poly[3], poly[4], poly[5], poly[0],
            ];
            r.stroke_polyline(&pts, hex_stroke, 0.5);
        }
    }

    // Face triangle outlines — the icosahedral fold lines. Heavier so they
    // read clearly against the hex grid.
    // Match the legend's ink — softer than pure black, especially in PNG
    // where 1.6-pixel-wide raw-black lines alias harshly.
    let face_stroke = Color::rgba(
        super::colormap::C_INK.0,
        super::colormap::C_INK.1,
        super::colormap::C_INK.2,
        super::colormap::C_INK.3,
    );
    for face in &grid.faces {
        for tri in &face.unfolded_positions {
            let pts = [tri[0], tri[1], tri[2], tri[0]];
            r.stroke_polyline(&pts, face_stroke, 1.6);
        }
    }

    // Rivers — drawn after face outlines so they sit on the terrain raster
    // but under the city glyphs. Stroke width scales with drainage area so
    // major rivers read fatter than tributaries.
    let river_color = Color::rgba(60, 110, 170, 230);
    for river in &map.rivers {
        let w = river_stroke_width(river.mouth_drainage);
        for stroke in &river.strokes {
            if stroke.len() >= 2 {
                r.stroke_polyline(stroke, river_color, w);
            }
        }
    }

    // Feature glyphs (cities, polar ice). Sized off the hex pitch so they
    // stay readable at any TRIANGLE_SIDE.
    let glyph_radius = TRIANGLE_SIDE / super::grid::HEXES_PER_EDGE as f64 * 0.30;
    for hex in &grid.hexes {
        for center in &hex.centers_2d {
            for feat in &hex.features {
                draw_feature(r, *center, glyph_radius, *feat);
            }
        }
    }

    // Title block — small UWP+seed card in upper-left corner. Floats over
    // the page background, gives the output an "intentional publication"
    // frame rather than reading as a raw render.
    draw_title_block(r, map);
}

/// Decorative hex-shaped title card in the upper-left corner of the
/// output. Three lines: UWP, seed, and a hex-scale value computed from
/// the UWP's world-size digit so the user can translate hex distances to
/// real km. Sized so it reads but doesn't crowd the polar caps.
fn draw_title_block<R: Renderer + ?Sized>(r: &mut R, map: &WorldMap) {
    let cx = 82.0;
    let cy = 64.0;
    let radius = 64.0; // hex circum-radius, ~128px wide
    let card_fill = Color::rgba(248, 246, 232, 240); // warm cream against page
    let ink = Color::rgba(
        super::colormap::C_INK.0,
        super::colormap::C_INK.1,
        super::colormap::C_INK.2,
        super::colormap::C_INK.3,
    );

    let hex = pointy_top_hex((cx, cy), radius * 1.732); // flat_to_flat = r*sqrt(3)
    r.fill_polygon(&hex, card_fill);
    let outline: Vec<(f64, f64)> = hex.iter().chain(std::iter::once(&hex[0])).copied().collect();
    r.stroke_polyline(&outline, ink, 1.4);

    let label_size = 9.0;
    let value_size = 13.0;
    let uwp_str = format!("{}", map.uwp);
    // Display seed in decimal — matches the input field, so users can copy
    // it back without converting hex.
    let seed_str = format!("seed {}", map.seed);
    let hex_km = hex_size_km(map.uwp.size());
    let scale_str = format!("1 hex ≈ {} km", format_thousands(hex_km));
    // Rough text-centering: subtract ~0.27 × char_count × size.
    let center_x = |s: &str, size: f64| cx - (s.chars().count() as f64) * size * 0.27;
    r.fill_text(center_x(&uwp_str, value_size), cy - 13.0, value_size, &uwp_str, ink);
    r.fill_text(center_x(&seed_str, label_size), cy + 5.0, label_size, &seed_str, ink);
    r.fill_text(center_x(&scale_str, label_size), cy + 21.0, label_size, &scale_str, ink);
}

/// Hex flat-to-flat in km, derived from the UWP's world-size digit and
/// the icosahedral subdivision. Each face edge is HEXES_PER_EDGE hexes
/// across; the icosahedron's edge subtends ~1.107 rad on the unit sphere,
/// so a single hex spans `(1.107 / HEXES_PER_EDGE) × planet_radius` of
/// surface arc. Rounded to a nice 50-km bucket for display.
fn hex_size_km(size_digit: u8) -> u32 {
    // Traveller convention: world diameter ≈ size × 1,600 km, with
    // asteroid-belt-ish 800 km for size 0.
    let diameter_km = if size_digit == 0 {
        800.0
    } else {
        (size_digit as f64) * 1600.0
    };
    let radius_km = diameter_km * 0.5;
    const HEX_ARC_RAD: f64 = 1.107_148_717_794_090_5 / (HEXES_PER_EDGE as f64);
    let km = radius_km * HEX_ARC_RAD;
    ((km / 50.0).round() * 50.0).max(50.0) as u32
}

/// Format an integer with thousands-separators (e.g. 12000 → "12,000").
fn format_thousands(mut n: u32) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut groups: Vec<String> = Vec::new();
    while n > 0 {
        groups.push(format!("{:03}", n % 1000));
        n /= 1000;
    }
    let mut out = groups.pop().unwrap().trim_start_matches('0').to_string();
    if out.is_empty() {
        out.push('0');
    }
    while let Some(g) = groups.pop() {
        out.push(',');
        out.push_str(&g);
    }
    out
}

/// Whether `(x, y)` lies inside any face's unfolded triangle (i.e. on the
/// icosahedron silhouette rather than in a "space" gap).
fn point_in_any_face(faces: &[Face], x: f64, y: f64) -> bool {
    for face in faces {
        for tri in &face.unfolded_positions {
            if point_in_triangle(tri, x, y) {
                return true;
            }
        }
    }
    false
}

fn point_in_triangle(tri: &[(f64, f64); 3], x: f64, y: f64) -> bool {
    // Sign test against each edge; all-same-sign means inside. Centroid is
    // used to pick the "inside" sign so winding doesn't matter.
    let cx = (tri[0].0 + tri[1].0 + tri[2].0) / 3.0;
    let cy = (tri[0].1 + tri[1].1 + tri[2].1) / 3.0;
    for i in 0..3 {
        let a = tri[i];
        let b = tri[(i + 1) % 3];
        let inside_sign = edge_side(a, b, (cx, cy));
        if edge_side(a, b, (x, y)) * inside_sign < -1e-9 {
            return false;
        }
    }
    true
}

fn edge_side(a: (f64, f64), b: (f64, f64), p: (f64, f64)) -> f64 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

/// Map drainage area at the river's mouth to stroke width. Logarithmic so
/// continental rivers don't dwarf the rest of the rendering.
fn river_stroke_width(drainage: f64) -> f64 {
    let d = drainage.max(0.0);
    (0.6 + 0.5 * (1.0 + d).ln()).clamp(0.6, 3.0)
}

fn draw_feature<R: Renderer + ?Sized>(r: &mut R, c: (f64, f64), radius: f64, feat: Feature) {
    let (cx, cy) = c;
    // Near-black ink for normal cities; red for the starport.
    let normal_ink = Color::rgba(20, 20, 20, 230);
    let starport_ink = Color::rgba(180, 30, 30, 240);
    match feat {
        Feature::PolarIce => {
            r.fill_circle(cx, cy, radius * 0.9, Color::rgba(245, 248, 252, 130));
        }
        Feature::City { tier, starport } => {
            let ink = if starport { starport_ink } else { normal_ink };
            // Subtle pale halo behind the glyph so it reads against any
            // biome (forest, desert, snow). Sized just larger than the
            // outermost ring, with low alpha so it doesn't shout.
            let halo = Color::rgba(248, 250, 246, 110);
            let halo_r = match tier {
                CityTier::Megacity => radius * 1.00,
                CityTier::Major => radius * 0.90,
                CityTier::Minor => radius * 0.70,
                CityTier::Small => radius * 0.40,
            };
            r.fill_circle(cx, cy, halo_r, halo);
            match tier {
                CityTier::Small => {
                    // Filled dot — only used when it's the sole settlement.
                    r.fill_circle(cx, cy, radius * 0.22, ink);
                }
                CityTier::Minor => {
                    // Smaller ring + center dot.
                    draw_annulus(r, cx, cy, radius * 0.55, radius * 0.42, ink);
                    r.fill_circle(cx, cy, radius * 0.18, ink);
                }
                CityTier::Major => {
                    // Standard ring + center dot (matches old Capital sizing).
                    draw_annulus(r, cx, cy, radius * 0.75, radius * 0.58, ink);
                    r.fill_circle(cx, cy, radius * 0.22, ink);
                }
                CityTier::Megacity => {
                    // Double-ring + larger center dot to read as the biggest tier.
                    draw_annulus(r, cx, cy, radius * 0.85, radius * 0.66, ink);
                    draw_annulus(r, cx, cy, radius * 0.50, radius * 0.38, ink);
                    r.fill_circle(cx, cy, radius * 0.28, ink);
                }
            }
        }
    }
}

/// Terrain swatch entries for the bottom-strip legend. Built from the
/// `colormap::C_*` palette constants — there are no hand-written RGB
/// tuples here, so the legend swatches are guaranteed to match what the
/// rasterizer actually paints. To change a color, edit the constant in
/// `colormap.rs` and both sides update.
///
/// Three palette entries are intentionally omitted as they're LERP
/// targets the user reads as their nearest legend neighbor:
///   - `C_DESERT_RED` — only reached at the hottest, driest extreme;
///     reads as "Desert".
///   - `C_STONE` — partial-strength gray-rock blend above 0.55 elev that
///     keeps biome tint; reads as "Rocky highland".
///   - intermediate ocean depths (LERP between C_SHALLOW_OCEAN and
///     C_DEEP_OCEAN) — reads as the nearest of the two.
///
/// The audit test `colormap::tests::palette_audit_*` enforces this
/// closure: every base color the rasterizer emits for a "pure" input is
/// either in the legend or one of the documented LERP targets above.
const LEGEND_TERRAINS: &[(Color, &str)] = &[
    (Color::from_palette(C_DEEP_OCEAN), "Deep ocean"),
    (Color::from_palette(C_SHALLOW_OCEAN), "Coastal shelf"),
    (Color::from_palette(C_SEA_ICE), "Sea ice"),
    (Color::from_palette(C_ICE_CAP), "Ice cap"),
    (Color::from_palette(C_TUNDRA), "Tundra"),
    (Color::from_palette(C_TAIGA), "Taiga"),
    (Color::from_palette(C_STEPPE), "Steppe"),
    (Color::from_palette(C_GRASSLAND), "Grassland"),
    (Color::from_palette(C_TEMPERATE_FOREST), "Temperate forest"),
    (Color::from_palette(C_TEMPERATE_RAINFOREST), "Temp. rainforest"),
    (Color::from_palette(C_DESERT_SAND), "Desert"),
    (Color::from_palette(C_SAVANNA), "Savanna"),
    (Color::from_palette(C_TROP_SEASONAL_FOREST), "Trop. seasonal forest"),
    (Color::from_palette(C_JUNGLE), "Jungle"),
    (Color::from_palette(C_ROCKY_HIGHLAND), "Rocky highland"),
    (Color::from_palette(C_SANDY_HIGHLAND), "Sandy highland"),
    (Color::from_palette(C_SNOW), "Snow / glacier"),
];

/// Settlement entries for the legend (tier, starport flag, label). The
/// `starport` boolean is true for the last row so the glyph renders in
/// red — matches the live map's starport ink.
const LEGEND_SETTLEMENTS: &[(CityTier, bool, &str)] = &[
    (CityTier::Megacity, false, "Megacity (10M+)"),
    (CityTier::Major, false, "Major city (1M+)"),
    (CityTier::Minor, false, "Minor city (500K+)"),
    (CityTier::Small, false, "Small settlement"),
    (CityTier::Major, true, "Starport"),
];

/// Draw the legend strip into the rect (`x`, `y`, `w`, `h`) on the given
/// renderer. Layout is a 5-column × 4-row grid: 15 terrain swatches in the
/// first 3 rows, 5 settlement glyphs in the bottom row.
fn draw_legend<R: Renderer + ?Sized>(r: &mut R, x: f64, y: f64, w: f64, h: f64) {
    // Match the page color so the legend reads as part of the same printed
    // surface, not a separate cream card glued to the bottom.
    let bg = Color::rgba(
        raster::SPACE_RGB.0,
        raster::SPACE_RGB.1,
        raster::SPACE_RGB.2,
        255,
    );
    let ink = Color::rgba(20, 20, 20, 230);
    let border = Color::rgba(20, 20, 20, 200);

    r.fill_rect(x, y, w, h, bg);
    // Top border between map and legend, plus a thin frame on the other
    // three sides so the legend reads as a discrete labelled panel rather
    // than the bottom edge bleeding into nothing.
    r.stroke_line(x, y, x + w, y, border, 1.2);
    let frame = Color::rgba(20, 20, 20, 90);
    let inset = 4.0;
    r.stroke_line(x + inset, y + inset, x + inset, y + h - inset, frame, 0.6);
    r.stroke_line(
        x + w - inset,
        y + inset,
        x + w - inset,
        y + h - inset,
        frame,
        0.6,
    );
    r.stroke_line(
        x + inset,
        y + h - inset,
        x + w - inset,
        y + h - inset,
        frame,
        0.6,
    );

    let cols = 5;
    let rows = 5;
    let pad_x = 8.0;
    let pad_y = 6.0;
    let cell_w = (w - 2.0 * pad_x) / cols as f64;
    let cell_h = (h - 2.0 * pad_y) / rows as f64;
    let swatch_size = (cell_h * 0.7).min(20.0);
    let label_size = (cell_h * 0.45).clamp(8.0, 11.0);

    // Header rule + column-spanning area — small visual divider between
    // terrains and settlements (4 rows × 5 cols = 20 terrain slots; last
    // row reserved for settlements).
    let divider_y = y + pad_y + 4.0 * cell_h - 1.0;
    r.stroke_line(
        x + pad_x,
        divider_y,
        x + w - pad_x,
        divider_y,
        Color::rgba(20, 20, 20, 90),
        0.6,
    );

    for (i, (color, label)) in LEGEND_TERRAINS.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let cx0 = x + pad_x + col as f64 * cell_w;
        let cy0 = y + pad_y + row as f64 * cell_h;
        let swatch_cx = cx0 + swatch_size * 0.5 + 2.0;
        let swatch_cy = cy0 + cell_h * 0.5;
        draw_legend_hex(r, swatch_cx, swatch_cy, swatch_size * 0.5, *color, ink);
        let text_x = cx0 + swatch_size + 8.0;
        let text_y = swatch_cy + label_size * 0.35;
        r.fill_text(text_x, text_y, label_size, label, ink);
    }

    for (i, (tier, starport, label)) in LEGEND_SETTLEMENTS.iter().enumerate() {
        let col = i % cols;
        let row = 4;
        let cx0 = x + pad_x + col as f64 * cell_w;
        let cy0 = y + pad_y + row as f64 * cell_h;
        let glyph_cx = cx0 + swatch_size * 0.5 + 2.0;
        let glyph_cy = cy0 + cell_h * 0.5;
        // Slightly larger glyph radius than the swatch so the rings read
        // clearly at the legend's modest size.
        let glyph_r = swatch_size * 0.55;
        let feat = Feature::City { tier: *tier, starport: *starport };
        draw_feature(r, (glyph_cx, glyph_cy), glyph_r, feat);
        let text_x = cx0 + swatch_size + 8.0;
        let text_y = glyph_cy + label_size * 0.35;
        r.fill_text(text_x, text_y, label_size, label, ink);
    }
}

/// Draw a small filled pointy-top hex centered at (cx, cy) with circum-radius
/// `r`, plus a thin ink outline so light swatches still read on the off-white
/// legend background.
fn draw_legend_hex<R: Renderer + ?Sized>(
    r: &mut R,
    cx: f64,
    cy: f64,
    radius: f64,
    fill: Color,
    outline: Color,
) {
    use std::f64::consts::TAU;
    let mut pts = Vec::with_capacity(6);
    for i in 0..6 {
        // pointy-top: vertices at 30° + i*60°
        let theta = TAU * (i as f64) / 6.0 + std::f64::consts::FRAC_PI_2;
        pts.push((cx + radius * theta.cos(), cy + radius * theta.sin()));
    }
    r.fill_polygon(&pts, fill);
    let mut closed = pts.clone();
    closed.push(pts[0]);
    r.stroke_polyline(&closed, outline, 0.6);
}

/// Build an annulus as a single polygon: outer ring CCW followed by inner
/// ring CW. With non-zero winding (used by both backends) the inner loop
/// punches a hole, giving an unfilled ring without needing stroke_circle.
fn draw_annulus<R: Renderer + ?Sized>(
    r: &mut R,
    cx: f64,
    cy: f64,
    r_out: f64,
    r_in: f64,
    color: Color,
) {
    use std::f64::consts::TAU;
    let n = 24;
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(n * 2);
    for i in 0..n {
        let theta = TAU * (i as f64) / (n as f64);
        pts.push((cx + r_out * theta.cos(), cy + r_out * theta.sin()));
    }
    for i in 0..n {
        let theta = TAU * ((n - i) as f64) / (n as f64);
        pts.push((cx + r_in * theta.cos(), cy + r_in * theta.sin()));
    }
    r.fill_polygon(&pts, color);
}
