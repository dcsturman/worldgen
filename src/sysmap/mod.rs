//! # System map renderer
//!
//! Produces a top-down (slightly tilted) pictorial render of a
//! [`crate::systems::system::System`] as a PNG. The view is modelled
//! on Traveller fan-art system charts: concentric tilted ellipses,
//! the central star coloured by spectral type, terrestrial worlds
//! and gas giants placed on their orbits, planetoid belts drawn as
//! seeded rock scatter, and moons drawn near their parent body.
//!
//! The renderer is **pure**: same `&System` always produces the same
//! PNG bytes (no rng inside; belt scatter uses a deterministic seed
//! derived from the orbit slot). Only `from_uwp` randomness — gas
//! giant radii, names, etc. — flows in through the system itself.
//!
//! # Entry point
//!
//! - [`render_png`] returns a `Vec<u8>` of PNG bytes for embedding in
//!   an `<img src="data:image/png;base64,...">`.
//!
//! # Architecture
//!
//! - [`geometry`] handles slot→pixel projection, tilt, and per-body
//!   sizing.
//! - [`colors`] holds the star-spectral-type → tint table and the
//!   palette constants.
//! - This file holds the actual tiny-skia drawing.

pub mod colors;
pub mod geometry;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use tiny_skia::{Color as SkColor, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::systems::gas_giant::GasGiant;
use crate::systems::system::{OrbitContent, Star, System};
use crate::systems::world::World;

use colors::*;
use geometry::*;

/// Bundled font for label/legend text. Same DejaVu Sans the worldmap
/// renderer uses.
const FONT_BYTES: &[u8] = include_bytes!("../../assets/DejaVuSans.ttf");

/// Render the given system to an opaque PNG and return the encoded bytes.
///
/// On any tiny-skia setup failure (typically out-of-memory for the
/// target pixmap) returns a `String` describing the cause; the caller
/// is expected to surface this in the UI.
pub fn render_png(system: &System) -> Result<Vec<u8>, String> {
    let mut pm = Pixmap::new(CANVAS_W as u32, CANVAS_H as u32)
        .ok_or_else(|| format!("Pixmap::new failed for {CANVAS_W}x{CANVAS_H}"))?;
    paint_background(&mut pm);

    let max_orbit = max_populated_orbit(system).unwrap_or(0);
    // The central star's drawn radius drives the inner-orbit floor:
    // a Ia supergiant pushes the closest orbit ring outward; a tiny D
    // dwarf lets the rings draw in close.
    let central_r = star_radius_px(system.star.size);
    let min_orbit = min_orbit_radius_for(central_r);

    draw_orbit_rings(&mut pm, system, max_orbit, min_orbit);
    draw_star(&mut pm, &system.star, central_r);
    draw_bodies(&mut pm, system, max_orbit, min_orbit);
    draw_header(&mut pm, system);
    draw_legend(&mut pm, system);

    pm.encode_png()
        .map_err(|e| format!("PNG encode failed: {e}"))
}

fn max_populated_orbit(system: &System) -> Option<usize> {
    system
        .orbit_slots
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| slot.as_ref().map(|_| i))
        .max()
}

fn paint_background(pm: &mut Pixmap) {
    pm.fill(SkColor::from_rgba8(BG.0, BG.1, BG.2, 255));
}

// ---- Primitives -----------------------------------------------------------

fn fill_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, rgba: (u8, u8, u8, u8)) {
    if r <= 0.0 {
        return;
    }
    if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
        let mut paint = Paint::default();
        paint.set_color(SkColor::from_rgba8(rgba.0, rgba.1, rgba.2, rgba.3));
        paint.anti_alias = true;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// Stroke a tilted ellipse (axis-aligned, x-radius `rx`, y-radius `ry`).
/// Uses the standard four-cubic kappa approximation.
fn stroke_ellipse(
    pm: &mut Pixmap,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rgba: (u8, u8, u8, u8),
    width: f32,
) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    const KAPPA: f32 = 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(cx + rx, cy);
    pb.cubic_to(
        cx + rx,
        cy + KAPPA * ry,
        cx + KAPPA * rx,
        cy + ry,
        cx,
        cy + ry,
    );
    pb.cubic_to(
        cx - KAPPA * rx,
        cy + ry,
        cx - rx,
        cy + KAPPA * ry,
        cx - rx,
        cy,
    );
    pb.cubic_to(
        cx - rx,
        cy - KAPPA * ry,
        cx - KAPPA * rx,
        cy - ry,
        cx,
        cy - ry,
    );
    pb.cubic_to(
        cx + KAPPA * rx,
        cy - ry,
        cx + rx,
        cy - KAPPA * ry,
        cx + rx,
        cy,
    );
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(SkColor::from_rgba8(rgba.0, rgba.1, rgba.2, rgba.3));
        paint.anti_alias = true;
        let stroke = Stroke {
            width,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

/// Rasterise `text` into the pixmap with its baseline at `(x, y)`.
///
/// No shaping or kerning beyond `ab_glyph::ScaleFont::kern`. Glyphs
/// are composited source-over against the existing pixmap contents.
fn fill_text(pm: &mut Pixmap, x: f32, y: f32, size: f32, text: &str, rgb: (u8, u8, u8)) {
    let font = match FontRef::try_from_slice(FONT_BYTES) {
        Ok(f) => f,
        Err(_) => return,
    };
    if size < 1.0 {
        return;
    }
    let scaled = font.as_scaled(PxScale::from(size));

    let pw = pm.width() as i32;
    let ph = pm.height() as i32;
    let stride = pm.width() as usize;
    let pixel_bytes = pm.data_mut();

    let mut pen_x = x;
    let mut prev: Option<ab_glyph::GlyphId> = None;
    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        if let Some(prev_id) = prev {
            pen_x += scaled.kern(prev_id, gid);
        }
        let glyph = gid.with_scale_and_position(PxScale::from(size), ab_glyph::point(pen_x, y));
        let advance = scaled.h_advance(gid);
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                let cov = cov.clamp(0.0, 1.0);
                if cov <= 0.0 {
                    return;
                }
                let px = bounds.min.x as i32 + gx as i32;
                let py = bounds.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= pw || py >= ph {
                    return;
                }
                let idx = (py as usize * stride + px as usize) * 4;
                let a = cov;
                let sr = (rgb.0 as f32 * a) as u8;
                let sg = (rgb.1 as f32 * a) as u8;
                let sb = (rgb.2 as f32 * a) as u8;
                let sa = (a * 255.0) as u8;
                let dr = pixel_bytes[idx];
                let dg = pixel_bytes[idx + 1];
                let db = pixel_bytes[idx + 2];
                let da = pixel_bytes[idx + 3];
                let inv_sa = 1.0 - (sa as f32 / 255.0);
                pixel_bytes[idx] = (sr as f32 + dr as f32 * inv_sa).round().clamp(0.0, 255.0) as u8;
                pixel_bytes[idx + 1] =
                    (sg as f32 + dg as f32 * inv_sa).round().clamp(0.0, 255.0) as u8;
                pixel_bytes[idx + 2] =
                    (sb as f32 + db as f32 * inv_sa).round().clamp(0.0, 255.0) as u8;
                pixel_bytes[idx + 3] =
                    (sa as f32 + da as f32 * inv_sa).round().clamp(0.0, 255.0) as u8;
            });
        }
        pen_x += advance;
        prev = Some(gid);
    }
}

/// Pixel width of `text` at the given font size — used to right-align
/// header / legend columns.
fn text_width(text: &str, size: f32) -> f32 {
    let Ok(font) = FontRef::try_from_slice(FONT_BYTES) else {
        return 0.0;
    };
    let scaled = font.as_scaled(PxScale::from(size));
    let mut w = 0.0_f32;
    let mut prev: Option<ab_glyph::GlyphId> = None;
    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        if let Some(p) = prev {
            w += scaled.kern(p, gid);
        }
        w += scaled.h_advance(gid);
        prev = Some(gid);
    }
    w
}

// ---- Orbits, star, bodies -------------------------------------------------

fn draw_orbit_rings(pm: &mut Pixmap, system: &System, max_orbit: usize, min_orbit: f32) {
    // Stellar zones drive the orbit colour: inner=blue, habitable=green,
    // outer=red. Each ring is rendered in two passes — a wide, very
    // translucent halo for the soft "glow" feel from the inspiration
    // art, and a sharper line on top to anchor the eye.
    let zones = crate::systems::system_tables::get_zone(&system.star);
    for (orbit, slot) in system.orbit_slots.iter().enumerate() {
        // Skip both genuinely empty (None) and intentionally Blocked
        // slots: the ring should only appear where there's an actual
        // body to anchor it. A lone orbit line in empty space implies
        // "something is here" — misleading.
        match slot {
            None | Some(OrbitContent::Blocked) => continue,
            _ => {}
        }
        let r = orbit_radius_px(orbit, max_orbit, min_orbit);
        let (cr, cg, cb) = zone_color(orbit, zones.inner, zones.habitable);
        // Glow: thick & dim.
        stroke_ellipse(
            pm,
            STAR_CX,
            STAR_CY,
            r,
            r * TILT_RATIO,
            (cr, cg, cb, 32),
            4.5,
        );
        // Line: thin & bright.
        stroke_ellipse(
            pm,
            STAR_CX,
            STAR_CY,
            r,
            r * TILT_RATIO,
            (cr, cg, cb, 190),
            1.0,
        );
    }
}

fn draw_star(pm: &mut Pixmap, star: &Star, radius: f32) {
    let (r, g, b) = star_color(star.star_type);
    // Soft halo: three concentric translucent discs of decreasing
    // alpha. Cheap glow effect; reads correctly against the dark bg.
    fill_circle(pm, STAR_CX, STAR_CY, radius * 2.6, (r, g, b, 18));
    fill_circle(pm, STAR_CX, STAR_CY, radius * 1.7, (r, g, b, 36));
    fill_circle(pm, STAR_CX, STAR_CY, radius * 1.2, (r, g, b, 80));
    fill_circle(pm, STAR_CX, STAR_CY, radius, (r, g, b, 255));
}

fn draw_bodies(pm: &mut Pixmap, system: &System, max_orbit: usize, min_orbit: f32) {
    for (orbit, slot) in system.orbit_slots.iter().enumerate() {
        let Some(content) = slot else { continue };
        let ring_r = orbit_radius_px(orbit, max_orbit, min_orbit);
        let theta = body_angle_rad(orbit);
        let (cx, cy) = body_position(ring_r, theta);
        match content {
            OrbitContent::World(w) => draw_world(pm, w, cx, cy),
            OrbitContent::GasGiant(gg) => draw_gas_giant(pm, gg, cx, cy),
            OrbitContent::Secondary => {
                if let Some(sec) = system.secondary.as_deref() {
                    draw_companion_star(pm, &sec.star, &sec.name, cx, cy);
                }
            }
            OrbitContent::Tertiary => {
                if let Some(ter) = system.tertiary.as_deref() {
                    draw_companion_star(pm, &ter.star, &ter.name, cx, cy);
                }
            }
            OrbitContent::Blocked => {}
        }
        // Belts: a World with size 0 in our model is a planetoid belt.
        // Render as scatter rather than a single disc.
        if let OrbitContent::World(w) = content
            && is_belt(w)
        {
            draw_belt(pm, ring_r, orbit);
        }
    }
}

fn is_belt(w: &World) -> bool {
    // Planetoid belts in the existing system come through as a World
    // with size 0 marked as a belt; we infer from size==0 + an empty
    // hydro/atmosphere here. Worlds with size 0 are otherwise rare,
    // so this is a safe heuristic for v1. Refine later if needed.
    let uwp = w.to_uwp();
    uwp.starts_with('0') || uwp.contains("Belt") || w.name.eq_ignore_ascii_case("planetoid belt")
}

fn draw_world(pm: &mut Pixmap, w: &World, cx: f32, cy: f32) {
    if is_belt(w) {
        // Belt scatter is drawn separately on the orbit ring itself;
        // skip the disc.
        draw_label(pm, cx, cy + 14.0, &w.name);
        return;
    }
    let r = world_radius_px(w.size);
    let (cr, cg, cb) = WORLD_DISC;
    fill_circle(pm, cx, cy, r, (cr, cg, cb, 255));
    draw_moons(pm, &w.satellites.sats, cx, cy, r);
    draw_label(pm, cx + r + 4.0, cy + 4.0, &w.name);
}

fn draw_gas_giant(pm: &mut Pixmap, gg: &GasGiant, cx: f32, cy: f32) {
    let r = gas_giant_radius_px(gg);
    let (cr, cg, cb) = GAS_GIANT_DISC;
    // Faint banding hint: a slightly darker inner ellipse.
    fill_circle(pm, cx, cy, r, (cr, cg, cb, 255));
    fill_circle(
        pm,
        cx,
        cy - r * 0.15,
        r * 0.7,
        (
            cr.saturating_sub(20),
            cg.saturating_sub(20),
            cb.saturating_sub(20),
            200,
        ),
    );
    draw_moons(pm, gg.satellites(), cx, cy, r);
    draw_label(pm, cx + r + 4.0, cy + 4.0, &gg.name);
}

/// Render a companion star (secondary or tertiary in a System orbit
/// slot) as a spectral-tinted disc sized by luminosity class, with a
/// soft halo and a name label. Sized so a giant reads clearly larger
/// than a main-sequence dwarf and the central star (22 px) stays the
/// visual anchor.
fn draw_companion_star(pm: &mut Pixmap, star: &Star, name: &str, cx: f32, cy: f32) {
    let (r, g, b) = star_color(star.star_type);
    let radius = star_radius_px(star.size);
    // Three-layer halo. Inner halo alpha is high enough that the
    // spectral colour reads cleanly even against a saturated zone
    // orbit ring underneath.
    fill_circle(pm, cx, cy, radius * 2.4, (r, g, b, 24));
    fill_circle(pm, cx, cy, radius * 1.5, (r, g, b, 90));
    fill_circle(pm, cx, cy, radius, (r, g, b, 255));
    draw_label(pm, cx + radius + 4.0, cy + 4.0, name);
}

fn draw_moons(pm: &mut Pixmap, moons: &[World], parent_cx: f32, parent_cy: f32, parent_r: f32) {
    if moons.is_empty() {
        return;
    }
    // Each moon gets its own miniature tilted orbit concentric with
    // the parent. We use the same TILT_RATIO as the system itself so
    // the moon orbits read as scaled-down versions of the main rings.
    // Angular position is a golden-angle fan keyed by moon index;
    // pure function of order, no rng, so the same system always
    // renders identically.
    for (idx, m) in moons.iter().take(MAX_MOONS_DRAWN).enumerate() {
        let orbit_r = moon_orbit_radius_px(parent_r, idx);
        stroke_ellipse(
            pm,
            parent_cx,
            parent_cy,
            orbit_r,
            orbit_r * TILT_RATIO,
            ORBIT_RING,
            0.6,
        );
        let theta = moon_angle_rad(idx);
        let mx = parent_cx + orbit_r * theta.cos();
        let my = parent_cy + orbit_r * theta.sin() * TILT_RATIO;
        let mr = moon_radius_px(m.size);
        let (cr, cg, cb) = MOON_DISC;
        fill_circle(pm, mx, my, mr, (cr, cg, cb, 255));
    }
}

fn draw_label(pm: &mut Pixmap, x: f32, y: f32, text: &str) {
    fill_text(pm, x, y, 12.0, text, LABEL);
}

fn draw_belt(pm: &mut Pixmap, ring_r: f32, orbit: usize) {
    // Deterministic per-orbit seed: render is pure given the system,
    // but each belt looks unique because its seed differs by slot.
    let seed = (0x9E37_79B9_u64)
        .wrapping_mul(orbit as u64 + 1)
        .wrapping_add(0x1234_5678);
    let mut rng = SmallRng::seed_from_u64(seed);
    for _ in 0..BELT_SAMPLES {
        let theta: f32 = rng.random_range(0.0..std::f32::consts::TAU);
        let dr: f32 = rng.random_range(-BELT_SCATTER_PX..BELT_SCATTER_PX);
        let r = ring_r + dr;
        let (x, y) = body_position(r, theta);
        let tone = if rng.random_bool(0.55) {
            BELT_TONE_A
        } else {
            BELT_TONE_B
        };
        // Vary alpha slightly to avoid a flat-painted look.
        let alpha = rng.random_range(150..=240);
        fill_circle(pm, x, y, 1.1, (tone.0, tone.1, tone.2, alpha));
    }
}

// ---- Header / legend ------------------------------------------------------

fn draw_header(pm: &mut Pixmap, system: &System) {
    let x = 40.0;
    let mut y = 60.0;
    fill_text(pm, x, y, 28.0, &system.name, LABEL);
    y += 36.0;
    let star_line = format!(
        "{}{} {}",
        format_star_type(&system.star),
        system.star.subtype,
        format_star_size(&system.star),
    );
    fill_text(pm, x, y, 18.0, &star_line, LABEL_DIM);
    y += 24.0;
    let comp = match (&system.secondary, &system.tertiary) {
        (Some(_), Some(_)) => "Trinary system",
        (Some(_), None) | (None, Some(_)) => "Binary system",
        (None, None) => "Solitary star",
    };
    fill_text(pm, x, y, 16.0, comp, LABEL_DIM);
}

fn format_star_type(star: &Star) -> String {
    format!("{:?}", star.star_type)
}

fn format_star_size(star: &Star) -> String {
    format!("{:?}", star.size)
}

fn draw_legend(pm: &mut Pixmap, system: &System) {
    let x_label = CANVAS_W - 360.0;
    let x_dist = CANVAS_W - 180.0;
    let mut y = 60.0;
    let line_h = 18.0;
    fill_text(pm, x_label, y, 14.0, "System Objects", LABEL);
    y += 22.0;
    fill_text(pm, x_label, y, 12.0, "Body", LABEL_DIM);
    fill_text(pm, x_dist, y, 12.0, "Mkm", LABEL_DIM);
    y += line_h;
    fill_text(pm, x_label, y, 12.0, &system.name, LABEL);
    fill_text(pm, x_dist, y, 12.0, "0.0", LABEL_DIM);
    y += line_h;
    for (orbit, slot) in system.orbit_slots.iter().enumerate() {
        let Some(content) = slot else { continue };
        let dist = slot_distance_mkm(orbit);
        let (name, kind): (String, &str) = match content {
            OrbitContent::World(w) => (w.name.clone(), if is_belt(w) { "Belt" } else { "World" }),
            OrbitContent::GasGiant(gg) => (gg.name.clone(), "Gas Giant"),
            OrbitContent::Secondary => (
                system
                    .secondary
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "Secondary".to_string()),
                "Star",
            ),
            OrbitContent::Tertiary => (
                system
                    .tertiary
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "Tertiary".to_string()),
                "Star",
            ),
            OrbitContent::Blocked => continue,
        };
        let row = format!("{name}  ({kind})");
        fill_text(pm, x_label, y, 12.0, &row, LABEL);
        let dist_str = format_mkm(dist);
        let dw = text_width(&dist_str, 12.0);
        // right-align distance column
        fill_text(pm, x_dist + 60.0 - dw, y, 12.0, &dist_str, LABEL_DIM);
        y += line_h;
        if y > CANVAS_H - 24.0 {
            break;
        }
    }
}

fn format_mkm(d: f32) -> String {
    if d < 1000.0 {
        format!("{d:.1}")
    } else if d < 1_000_000.0 {
        format!("{:.0}", d)
    } else {
        format!("{:.2e}", d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::system::System;
    use crate::systems::world::World;

    #[test]
    fn renders_minimal_system_to_png() {
        let mw = World::from_uwp("Regina", "A788899-A", false, true).unwrap();
        let sys = System::generate_system(mw);
        let bytes = render_png(&sys).expect("render");
        // PNG magic
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        // Optional dump for human inspection: SYSMAP_DUMP=/tmp/foo.png
        if let Ok(path) = std::env::var("SYSMAP_DUMP") {
            std::fs::write(&path, &bytes).expect("dump");
        }
    }
}
