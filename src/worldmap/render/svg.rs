//! SVG string-builder backend.

use std::fmt::Write;

use super::{Color, Renderer};

pub struct SvgRenderer {
    width: f64,
    height: f64,
    body: String,
}

impl SvgRenderer {
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width,
            height,
            body: String::with_capacity(64 * 1024),
        }
    }

    /// Embed a raster image (already PNG-encoded) as an SVG `<image>`,
    /// stretched to fill the given rect. Used to splat the rasterized
    /// terrain in as the background before vector overlays go on top.
    pub fn embed_png(&mut self, png_bytes: &[u8], x: f64, y: f64, w: f64, h: f64) {
        let b64 = base64_encode(png_bytes);
        let _ = write!(
            self.body,
            r#"<image x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" preserveAspectRatio="none" href="data:image/png;base64,{b64}"/>"#,
        );
    }

    pub fn into_string(self) -> String {
        let mut out = String::with_capacity(self.body.len() + 256);
        let _ = write!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" preserveAspectRatio="xMidYMid meet" shape-rendering="geometricPrecision">"#,
            w = self.width,
            h = self.height,
        );
        out.push_str(&self.body);
        out.push_str("</svg>");
        out
    }
}

/// Minimal RFC 4648 base64 encoder. Inlined to avoid pulling in a crate
/// for the one place we need it (PNG → data URL for SVG embed).
fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let b0 = data[i];
        let b1 = data[i + 1];
        let b2 = data[i + 2];
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        out.push(TABLE[(b2 & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = data.len() - i;
    if rem == 1 {
        let b0 = data[i];
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[((b0 & 0x03) << 4) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let b0 = data[i];
        let b1 = data[i + 1];
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        out.push(TABLE[((b1 & 0x0f) << 2) as usize] as char);
        out.push('=');
    }
    out
}

fn fmt_color(c: Color) -> String {
    if c.3 == 255 {
        format!("rgb({},{},{})", c.0, c.1, c.2)
    } else {
        format!(
            "rgba({},{},{},{:.3})",
            c.0,
            c.1,
            c.2,
            c.3 as f32 / 255.0
        )
    }
}

impl Renderer for SvgRenderer {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, color: Color) {
        let _ = write!(
            self.body,
            r#"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" fill="{c}"/>"#,
            c = fmt_color(color),
        );
    }

    fn fill_polygon(&mut self, points: &[(f64, f64)], color: Color) {
        self.body.push_str("<polygon points=\"");
        for (i, (x, y)) in points.iter().enumerate() {
            if i > 0 {
                self.body.push(' ');
            }
            let _ = write!(self.body, "{x:.2},{y:.2}");
        }
        let _ = write!(self.body, "\" fill=\"{}\"/>", fmt_color(color));
    }

    fn fill_circle(&mut self, cx: f64, cy: f64, r: f64, color: Color) {
        let _ = write!(
            self.body,
            r#"<circle cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}" fill="{c}"/>"#,
            c = fmt_color(color),
        );
    }

    fn stroke_line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, color: Color, width: f64) {
        let _ = write!(
            self.body,
            r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{c}" stroke-width="{w}" stroke-linecap="round"/>"#,
            c = fmt_color(color),
            w = width,
        );
    }

    fn stroke_polyline(&mut self, points: &[(f64, f64)], color: Color, width: f64) {
        self.body.push_str("<polyline points=\"");
        for (i, (x, y)) in points.iter().enumerate() {
            if i > 0 {
                self.body.push(' ');
            }
            let _ = write!(self.body, "{x:.2},{y:.2}");
        }
        let _ = write!(
            self.body,
            r#"" fill="none" stroke="{c}" stroke-width="{w}" stroke-linejoin="round"/>"#,
            c = fmt_color(color),
            w = width,
        );
    }

    fn fill_text(&mut self, x: f64, y: f64, size: f64, text: &str, color: Color) {
        // Use a generic sans-serif so the SVG renders consistently in the
        // browser even though the PNG path uses a bundled DejaVu Sans.
        let _ = write!(
            self.body,
            r#"<text x="{x:.2}" y="{y:.2}" font-size="{s:.2}" font-family="DejaVu Sans, Verdana, Helvetica, Arial, sans-serif" fill="{c}">{t}</text>"#,
            s = size,
            c = fmt_color(color),
            t = escape_xml(text),
        );
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
