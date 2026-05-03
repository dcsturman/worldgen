//! PNG (raster) backend via tiny-skia. Same primitive interface as SVG.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use tiny_skia::{
    Color as SkColor, FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

use super::{Color, Renderer};

/// Bundled open-source font (Bitstream Vera / DejaVu license) used to
/// rasterize the legend labels into the PNG export. Embedded so the binary
/// is self-contained — no font lookups at runtime.
const FONT_BYTES: &[u8] = include_bytes!("../../../assets/DejaVuSans.ttf");

pub struct PngRenderer {
    pixmap: Pixmap,
    transform: Transform,
}

impl PngRenderer {
    pub fn new(width: f64, height: f64, scale: f64) -> Result<Self, String> {
        let w = (width * scale).ceil() as u32;
        let h = (height * scale).ceil() as u32;
        let pixmap = Pixmap::new(w, h).ok_or_else(|| {
            format!("tiny_skia::Pixmap::new failed for {w}x{h}")
        })?;
        Ok(Self {
            pixmap,
            transform: Transform::from_scale(scale as f32, scale as f32),
        })
    }

    /// Create a renderer pre-initialized with an RGBA8 raster. The raster
    /// must be `pixel_w × pixel_h` and is treated as opaque (alpha is
    /// preserved as-is). The overlay transform maps `(width, height)` user
    /// units to the pixmap, so vector primitives drawn afterward sit on top
    /// at the correct scale.
    pub fn from_raster(
        rgba: &[u8],
        pixel_w: u32,
        pixel_h: u32,
        width: f64,
        height: f64,
    ) -> Result<Self, String> {
        let expected = (pixel_w as usize) * (pixel_h as usize) * 4;
        if rgba.len() != expected {
            return Err(format!(
                "raster size mismatch: got {} bytes, expected {expected} for {pixel_w}x{pixel_h}",
                rgba.len()
            ));
        }
        let mut pixmap = Pixmap::new(pixel_w, pixel_h).ok_or_else(|| {
            format!("tiny_skia::Pixmap::new failed for {pixel_w}x{pixel_h}")
        })?;
        // tiny_skia stores premultiplied RGBA. Our raster pixels are all
        // alpha=255, so straight RGBA == premultiplied for them — direct
        // copy is correct.
        pixmap.data_mut().copy_from_slice(rgba);
        let sx = pixel_w as f32 / width as f32;
        let sy = pixel_h as f32 / height as f32;
        Ok(Self {
            pixmap,
            transform: Transform::from_scale(sx, sy),
        })
    }

    /// Same as `from_raster`, but allocates a taller pixmap (`total_pixel_h`
    /// rows) and copies the raster into the top `pixel_h` rows. The bottom
    /// band is left transparent for the legend strip to be drawn on top.
    /// `width` / `total_height` are the user-space extents the transform
    /// maps to the full pixmap.
    pub fn from_raster_with_extra_height(
        rgba: &[u8],
        pixel_w: u32,
        pixel_h: u32,
        total_pixel_h: u32,
        width: f64,
        total_height: f64,
    ) -> Result<Self, String> {
        let expected = (pixel_w as usize) * (pixel_h as usize) * 4;
        if rgba.len() != expected {
            return Err(format!(
                "raster size mismatch: got {} bytes, expected {expected} for {pixel_w}x{pixel_h}",
                rgba.len()
            ));
        }
        if total_pixel_h < pixel_h {
            return Err(format!(
                "total_pixel_h {total_pixel_h} smaller than pixel_h {pixel_h}"
            ));
        }
        let mut pixmap = Pixmap::new(pixel_w, total_pixel_h).ok_or_else(|| {
            format!("tiny_skia::Pixmap::new failed for {pixel_w}x{total_pixel_h}")
        })?;
        let row_bytes = (pixel_w as usize) * 4;
        let dst = pixmap.data_mut();
        dst[..rgba.len()].copy_from_slice(rgba);
        // Sanity: we wrote exactly `pixel_h` rows of `row_bytes` bytes.
        debug_assert_eq!(rgba.len(), pixel_h as usize * row_bytes);

        let sx = pixel_w as f32 / width as f32;
        let sy = total_pixel_h as f32 / total_height as f32;
        Ok(Self {
            pixmap,
            transform: Transform::from_scale(sx, sy),
        })
    }

    pub fn encode(self) -> Result<Vec<u8>, String> {
        self.pixmap
            .encode_png()
            .map_err(|e| format!("PNG encode failed: {e}"))
    }
}

fn to_paint(c: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(SkColor::from_rgba8(c.0, c.1, c.2, c.3));
    paint.anti_alias = true;
    paint
}

impl Renderer for PngRenderer {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Color) {
        if let Some(rect) = Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
            self.pixmap
                .fill_rect(rect, &to_paint(color), self.transform, None);
        }
    }

    fn fill_polygon(&mut self, points: &[(f64, f64)], color: Color) {
        if points.len() < 3 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(points[0].0 as f32, points[0].1 as f32);
        for p in &points[1..] {
            pb.line_to(p.0 as f32, p.1 as f32);
        }
        pb.close();
        if let Some(path) = pb.finish() {
            self.pixmap.fill_path(
                &path,
                &to_paint(color),
                FillRule::Winding,
                self.transform,
                None,
            );
        }
    }

    fn fill_circle(&mut self, cx: f64, cy: f64, r: f64, color: Color) {
        if let Some(path) = PathBuilder::from_circle(cx as f32, cy as f32, r as f32) {
            self.pixmap.fill_path(
                &path,
                &to_paint(color),
                FillRule::Winding,
                self.transform,
                None,
            );
        }
    }

    fn stroke_line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: Color, width: f64) {
        let mut pb = PathBuilder::new();
        pb.move_to(x1 as f32, y1 as f32);
        pb.line_to(x2 as f32, y2 as f32);
        if let Some(path) = pb.finish() {
            let stroke = Stroke {
                width: width as f32,
                ..Default::default()
            };
            self.pixmap
                .stroke_path(&path, &to_paint(color), &stroke, self.transform, None);
        }
    }

    fn stroke_polyline(&mut self, points: &[(f64, f64)], color: Color, width: f64) {
        if points.len() < 2 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(points[0].0 as f32, points[0].1 as f32);
        for p in &points[1..] {
            pb.line_to(p.0 as f32, p.1 as f32);
        }
        if let Some(path) = pb.finish() {
            let stroke = Stroke {
                width: width as f32,
                ..Default::default()
            };
            self.pixmap
                .stroke_path(&path, &to_paint(color), &stroke, self.transform, None);
        }
    }

    fn fill_text(&mut self, x: f64, y: f64, size: f64, text: &str, color: Color) {
        // tiny_skia has no text path. Use ab_glyph to outline each glyph
        // and splat its coverage into the pixmap directly. We render at
        // pixel scale (post-transform) so the rasterized glyphs aren't
        // re-blurred by the user→pixel scale.
        //
        // Layout is single-line, no shaping, no kerning beyond what
        // `kern_unscaled` provides via `as_scaled`.
        let font = match FontRef::try_from_slice(FONT_BYTES) {
            Ok(f) => f,
            Err(_) => return,
        };
        // Pixel scale: convert user-space size to pixmap pixels via the
        // transform's vertical scale (sx == sy here).
        let pixel_size = (size * self.transform.sy as f64) as f32;
        if pixel_size < 1.0 {
            return;
        }
        let scaled = font.as_scaled(PxScale::from(pixel_size));
        let ascent = scaled.ascent();

        let pixmap_w = self.pixmap.width() as i32;
        let pixmap_h = self.pixmap.height() as i32;

        // Origin in pixmap pixels. `(x, y)` is the baseline anchor in user
        // space; convert to pixmap space via the transform.
        let origin_x = (x as f32) * self.transform.sx;
        let origin_y = (y as f32) * self.transform.sy;

        let mut pen_x = origin_x;
        let mut prev: Option<ab_glyph::GlyphId> = None;
        for ch in text.chars() {
            let glyph_id = font.glyph_id(ch);
            if let Some(prev_id) = prev {
                pen_x += scaled.kern(prev_id, glyph_id);
            }
            let glyph = glyph_id.with_scale_and_position(
                PxScale::from(pixel_size),
                ab_glyph::point(pen_x, origin_y),
            );
            let advance = scaled.h_advance(glyph_id);
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                // ab_glyph returns coverage in [0,1]; we composite it as
                // alpha against the requested color.
                let stride = self.pixmap.width() as usize;
                let pixel_bytes = self.pixmap.data_mut();
                let _ = ascent; // (kept for future baseline tweaks)
                outlined.draw(|gx, gy, cov| {
                    let cov = cov.clamp(0.0, 1.0);
                    if cov <= 0.0 {
                        return;
                    }
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;
                    if px < 0 || py < 0 || px >= pixmap_w || py >= pixmap_h {
                        return;
                    }
                    let idx = (py as usize * stride + px as usize) * 4;
                    let a = (color.3 as f32 / 255.0) * cov;
                    // Premultiplied source.
                    let sr = (color.0 as f32 * a).round() as u8;
                    let sg = (color.1 as f32 * a).round() as u8;
                    let sb = (color.2 as f32 * a).round() as u8;
                    let sa = (a * 255.0).round() as u8;
                    // Source-over composite: out = src + dst*(1-src.a).
                    let dr = pixel_bytes[idx];
                    let dg = pixel_bytes[idx + 1];
                    let db = pixel_bytes[idx + 2];
                    let da = pixel_bytes[idx + 3];
                    let inv_sa = 1.0 - (sa as f32 / 255.0);
                    let or = (sr as f32 + dr as f32 * inv_sa).round().clamp(0.0, 255.0) as u8;
                    let og = (sg as f32 + dg as f32 * inv_sa).round().clamp(0.0, 255.0) as u8;
                    let ob = (sb as f32 + db as f32 * inv_sa).round().clamp(0.0, 255.0) as u8;
                    let oa = (sa as f32 + da as f32 * inv_sa).round().clamp(0.0, 255.0) as u8;
                    // tiny_skia's pixmap stores premultiplied RGBA. Our
                    // composite already produced premultiplied output.
                    pixel_bytes[idx] = or;
                    pixel_bytes[idx + 1] = og;
                    pixel_bytes[idx + 2] = ob;
                    pixel_bytes[idx + 3] = oa;
                });
            }
            pen_x += advance;
            prev = Some(glyph_id);
        }
    }
}
