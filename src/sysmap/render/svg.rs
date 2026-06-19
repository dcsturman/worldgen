//! SVG string-builder backend for the system map.
//!
//! Emits resolution-independent `<circle>` / `<ellipse>` / `<text>` in the
//! logical 1600×900 coordinate space (one `viewBox`, no per-call scaling —
//! the browser scales the whole document). Each interactive body is wrapped
//! in a `<g class="sysmap-body" data-…>` element via [`begin_group`] /
//! [`end_group`] so consuming apps can attach click/hover handlers and read
//! the body's identity straight off the DOM.
//!
//! [`begin_group`]: Renderer::begin_group
//! [`end_group`]: Renderer::end_group

use std::fmt::Write;

use super::{BodyMeta, Renderer};
use crate::sysmap::colors::BG;
use crate::util::escape_xml;

pub struct SvgRenderer {
    width: f32,
    height: f32,
    body: String,
}

impl SvgRenderer {
    pub fn new(width: f32, height: f32) -> Self {
        let mut s = Self {
            width,
            height,
            body: String::with_capacity(32 * 1024),
        };
        // Opaque background rect, matching the PNG backend's BG fill so the
        // two outputs read identically.
        let _ = write!(
            s.body,
            r#"<rect x="0" y="0" width="{w:.2}" height="{h:.2}" fill="rgb({r},{g},{b})"/>"#,
            w = width,
            h = height,
            r = BG.0,
            g = BG.1,
            b = BG.2,
        );
        s
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

fn fmt_rgba(c: (u8, u8, u8, u8)) -> String {
    if c.3 == 255 {
        format!("rgb({},{},{})", c.0, c.1, c.2)
    } else {
        format!("rgba({},{},{},{:.3})", c.0, c.1, c.2, c.3 as f32 / 255.0)
    }
}

fn fmt_rgb(c: (u8, u8, u8)) -> String {
    format!("rgb({},{},{})", c.0, c.1, c.2)
}

impl Renderer for SvgRenderer {
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, rgba: (u8, u8, u8, u8)) {
        if r <= 0.0 {
            return;
        }
        let _ = write!(
            self.body,
            r#"<circle cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}" fill="{c}"/>"#,
            c = fmt_rgba(rgba),
        );
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
        let _ = write!(
            self.body,
            r#"<ellipse cx="{cx:.2}" cy="{cy:.2}" rx="{rx:.2}" ry="{ry:.2}" fill="none" stroke="{c}" stroke-width="{w:.2}"/>"#,
            c = fmt_rgba(rgba),
            w = width,
        );
    }

    fn fill_text(&mut self, x: f32, y: f32, size: f32, text: &str, rgb: (u8, u8, u8)) {
        // Generic sans-serif stack so the SVG renders consistently in the
        // browser even though the PNG path uses the bundled DejaVu Sans.
        let _ = write!(
            self.body,
            r#"<text x="{x:.2}" y="{y:.2}" font-size="{s:.2}" font-family="DejaVu Sans, Verdana, Helvetica, Arial, sans-serif" fill="{c}">{t}</text>"#,
            s = size,
            c = fmt_rgb(rgb),
            t = escape_xml(text),
        );
    }

    fn begin_group(&mut self, meta: &BodyMeta) {
        let _ = write!(
            self.body,
            r#"<g class="sysmap-body" data-kind="{k}" data-name="{n}""#,
            k = meta.kind.as_str(),
            n = escape_xml(&meta.name),
        );
        if let Some(uwp) = &meta.uwp {
            let _ = write!(self.body, r#" data-uwp="{}""#, escape_xml(uwp));
        }
        if let Some(spectral) = &meta.spectral {
            let _ = write!(self.body, r#" data-spectral="{}""#, escape_xml(spectral));
        }
        if let Some(orbit) = meta.orbit {
            let _ = write!(self.body, r#" data-orbit="{orbit}""#);
        }
        if let Some(dist) = meta.distance_mkm {
            let _ = write!(self.body, r#" data-distance-mkm="{dist:.1}""#);
        }
        self.body.push('>');
    }

    fn end_group(&mut self) {
        self.body.push_str("</g>");
    }

    fn vector_belts(&self) -> bool {
        true
    }
}
