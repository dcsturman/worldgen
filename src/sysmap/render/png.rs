//! tiny-skia raster backend for the system map.
//!
//! Holds a [`Pixmap`] and the active render scale. All scene geometry is
//! expressed in logical 1600×900 space; this backend scales paths via a
//! `Transform` and scales stroke widths / text position+size by `scale`
//! itself (tiny-skia interprets stroke widths and ab_glyph rasterises in
//! destination-pixel space, neither of which the transform reaches). At
//! `scale == 1.0` the transform is the identity matrix, so the output is
//! byte-identical to the pre-refactor renderer.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use tiny_skia::{Color as SkColor, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use super::{FONT_BYTES, Renderer};
use crate::sysmap::colors::BG;

pub struct PngRenderer {
    pm: Pixmap,
    scale: f32,
}

impl PngRenderer {
    /// Allocate a `logical_w × logical_h` canvas at `scale × native`
    /// resolution and paint the background. Returns `Err` on allocation
    /// failure (typically out-of-memory for the target pixmap).
    pub fn new(logical_w: f32, logical_h: f32, scale: f32) -> Result<Self, String> {
        let pw = (logical_w * scale).round() as u32;
        let ph = (logical_h * scale).round() as u32;
        let mut pm =
            Pixmap::new(pw, ph).ok_or_else(|| format!("Pixmap::new failed for {pw}x{ph}"))?;
        pm.fill(SkColor::from_rgba8(BG.0, BG.1, BG.2, 255));
        Ok(Self { pm, scale })
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.pm
            .encode_png()
            .map_err(|e| format!("PNG encode failed: {e}"))
    }
}

impl Renderer for PngRenderer {
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, rgba: (u8, u8, u8, u8)) {
        if r <= 0.0 {
            return;
        }
        // Geometry math stays in logical 1600×900 space; the `Transform`
        // scales the rasterized path into pixel space. At `scale == 1.0`
        // this is bitwise identity (Transform::from_scale(1, 1) is the
        // identity matrix in tiny-skia), so the legacy output is byte-
        // preserved.
        if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
            let mut paint = Paint::default();
            paint.set_color(SkColor::from_rgba8(rgba.0, rgba.1, rgba.2, rgba.3));
            paint.anti_alias = true;
            self.pm.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::from_scale(self.scale, self.scale),
                None,
            );
        }
    }

    fn stroke_ellipse(
        &mut self,
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
        pb.cubic_to(cx + rx, cy + KAPPA * ry, cx + KAPPA * rx, cy + ry, cx, cy + ry);
        pb.cubic_to(cx - KAPPA * rx, cy + ry, cx - rx, cy + KAPPA * ry, cx - rx, cy);
        pb.cubic_to(cx - rx, cy - KAPPA * ry, cx - KAPPA * rx, cy - ry, cx, cy - ry);
        pb.cubic_to(cx + KAPPA * rx, cy - ry, cx + rx, cy - KAPPA * ry, cx + rx, cy);
        pb.close();
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(SkColor::from_rgba8(rgba.0, rgba.1, rgba.2, rgba.3));
            paint.anti_alias = true;
            // Stroke widths in tiny-skia are interpreted in destination
            // (post-transform) pixel space, so we have to scale the width
            // ourselves; the `Transform` only handles path geometry.
            let stroke = Stroke {
                width: width * self.scale,
                ..Default::default()
            };
            self.pm.stroke_path(
                &path,
                &paint,
                &stroke,
                Transform::from_scale(self.scale, self.scale),
                None,
            );
        }
    }

    fn fill_text(&mut self, x: f32, y: f32, size: f32, text: &str, rgb: (u8, u8, u8)) {
        let font = match FontRef::try_from_slice(FONT_BYTES) {
            Ok(f) => f,
            Err(_) => return,
        };
        // ab_glyph rasterizes directly into the pixel buffer and ignores the
        // tiny-skia transform, so we scale position and font size at the
        // entry. Callers pass logical 1600×900-space coordinates and font
        // sizes; `text_width` / right-alignment math also lives in logical
        // space, which works because both `x` and `pen_x` advances scale by
        // the same factor.
        let x = x * self.scale;
        let y = y * self.scale;
        let size = size * self.scale;
        if size < 1.0 {
            return;
        }
        let scaled = font.as_scaled(PxScale::from(size));

        let pw = self.pm.width() as i32;
        let ph = self.pm.height() as i32;
        let stride = self.pm.width() as usize;
        let pixel_bytes = self.pm.data_mut();

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
                    pixel_bytes[idx] =
                        (sr as f32 + dr as f32 * inv_sa).round().clamp(0.0, 255.0) as u8;
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

    // begin_group / end_group / vector_belts use the trait defaults:
    // grouping is a no-op (raster has no clickable structure) and belts
    // render as the per-asteroid scatter, preserving byte-identical output.
}
