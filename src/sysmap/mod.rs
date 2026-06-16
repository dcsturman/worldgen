//! # System map renderer
//!
//! Produces a top-down (slightly tilted) pictorial render of a
//! [`crate::systems::system::System`]. The view is modelled on Traveller
//! fan-art system charts: concentric tilted ellipses, the central star
//! coloured by spectral type, terrestrial worlds and gas giants placed on
//! their orbits, planetoid belts drawn as seeded rock scatter, and moons
//! drawn near their parent body.
//!
//! The renderer is **pure**: same `&System` always produces the same
//! output (no rng inside; belt scatter uses a deterministic seed derived
//! from the orbit slot). Only `from_uwp` randomness — gas giant radii,
//! names, etc. — flows in through the system itself.
//!
//! # Entry points
//!
//! - [`render_png`] / [`render_png_scaled`] return PNG bytes for embedding
//!   in an `<img src="data:image/png;base64,…">`.
//! - [`render_svg`] returns a resolution-independent SVG string whose
//!   bodies are wrapped in `<g class="sysmap-body" data-…>` groups so a
//!   consuming web app can make individual bodies clickable.
//!
//! # Architecture
//!
//! Both outputs are produced from a single scene-walk
//! ([`render::render_scene`]) generic over the [`render::Renderer`] trait —
//! neither format is derived from the other; they are two sinks fed by the
//! same description.
//!
//! - [`geometry`] handles slot→pixel projection, tilt, and per-body sizing.
//! - [`colors`] holds the star-spectral-type → tint table and palette.
//! - [`render`] holds the `Renderer` trait, the shared scene-walk, and the
//!   PNG (tiny-skia) and SVG (string-builder) backends.

pub mod colors;
pub mod geometry;
pub mod render;

use crate::systems::system::System;
use geometry::{CANVAS_H, CANVAS_W};
use render::{PngRenderer, SvgRenderer};

/// Render the given system to an opaque PNG and return the encoded bytes.
///
/// Equivalent to [`render_png_scaled`] called with `scale = 1.0`. On any
/// tiny-skia setup failure (typically out-of-memory for the target pixmap)
/// returns a `String` describing the cause.
pub fn render_png(system: &System) -> Result<Vec<u8>, String> {
    render_png_scaled(system, 1.0)
}

/// Render the given system to an opaque PNG at the requested pixel scale.
/// `scale = 1.0` matches the legacy 1600×900 output; `scale = 2.0` is
/// 3200×1800; higher values scale proportionally with no composition
/// change — same layout, same orbit positions, same label placement, just
/// more pixels. Fonts, stroke widths, dot radii, and the legend all scale
/// uniformly so relative sizes are preserved.
///
/// Returns an error if `scale < 1.0` or not finite. The scale is not fed
/// into any RNG — same `(system, scale)` always produces the same bytes.
pub fn render_png_scaled(system: &System, scale: f32) -> Result<Vec<u8>, String> {
    if !scale.is_finite() || scale < 1.0 {
        return Err(format!(
            "render scale must be a finite value >= 1.0, got {scale}"
        ));
    }
    let mut r = PngRenderer::new(CANVAS_W, CANVAS_H, scale)?;
    render::render_scene(&mut r, system);
    r.encode()
}

/// Render the given system to an SVG string.
///
/// The SVG is resolution-independent (a single `viewBox="0 0 1600 900"`),
/// so there's no scale parameter — the browser scales the whole document.
/// Each interactive body (star, world, gas giant, belt, moon) is wrapped
/// in a `<g class="sysmap-body" data-kind=… data-name=… data-uwp=…
/// data-orbit=… data-distance-mkm=…>` element so a consuming web app can
/// attach click/hover handlers and read the body's identity off the DOM.
///
/// Pure: same `&System` always produces the same string.
pub fn render_svg(system: &System) -> String {
    let mut r = SvgRenderer::new(CANVAS_W, CANVAS_H);
    render::render_scene(&mut r, system);
    r.into_string()
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

    #[test]
    fn renders_contact_binary() {
        use crate::systems::constraint::{Constraint, SystemConstraints};
        use crate::systems::system::{StarOrbit, StarSize, StarType};
        let mut cs = SystemConstraints::from_main_world("Binar", "A788899-A").unwrap();
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::Primary),
            spectral: Some(StarType::G),
            subtype: Some(2),
            size: Some(StarSize::V),
        });
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::Primary),
            spectral: Some(StarType::K),
            subtype: Some(5),
            size: Some(StarSize::V),
        });
        let sys = System::generate_from_constraints(cs).expect("generated");
        assert!(sys.secondary.is_some());
        assert_eq!(sys.secondary.as_ref().unwrap().orbit, StarOrbit::Primary);
        let bytes = render_png(&sys).expect("render");
        if let Ok(path) = std::env::var("SYSMAP_DUMP_BINARY") {
            std::fs::write(&path, &bytes).expect("dump");
        }
    }

    #[test]
    fn renders_noricum_three_star_system() {
        use crate::systems::constraint::{Constraint, SystemConstraints};
        use crate::systems::system::{StarOrbit, StarSize, StarType};
        let mut cs = SystemConstraints::from_main_world("Noricum", "D8867BB-1").unwrap();
        cs.bodies.push(Constraint::Star {
            orbit: Some(StarOrbit::Primary),
            spectral: Some(StarType::G),
            subtype: Some(2),
            size: Some(StarSize::V),
        });
        cs.bodies.push(Constraint::Star {
            // Pin secondary to a System orbit so the test isn't flaky on
            // the random companion-orbit roll.
            orbit: Some(StarOrbit::System(8)),
            spectral: Some(StarType::M),
            subtype: Some(9),
            size: Some(StarSize::V),
        });
        cs.bodies.push(Constraint::Star {
            // Force tertiary to Far so we exercise the far-companion slot.
            orbit: Some(StarOrbit::Far),
            spectral: Some(StarType::M),
            subtype: Some(6),
            size: Some(StarSize::V),
        });
        let sys = System::generate_from_constraints(cs).expect("generated");
        let bytes = render_png(&sys).expect("render");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        if let Ok(path) = std::env::var("SYSMAP_DUMP_NORICUM") {
            std::fs::write(&path, &bytes).expect("dump");
        }
    }

    // ---- SVG ----------------------------------------------------------

    #[test]
    fn renders_minimal_system_to_svg() {
        let mw = World::from_uwp("Regina", "A788899-A", false, true).unwrap();
        let sys = System::generate_system(mw);
        let svg = render_svg(&sys);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // At least the main world should be a clickable body group.
        assert!(
            svg.contains(r#"class="sysmap-body""#),
            "expected at least one body group"
        );
        if let Ok(path) = std::env::var("SYSMAP_DUMP_SVG") {
            std::fs::write(&path, &svg).expect("dump");
        }
    }

    #[test]
    fn svg_render_is_deterministic() {
        let mw = World::from_uwp("Regina", "A788899-A", false, true).unwrap();
        let sys = System::generate_system(mw);
        assert_eq!(render_svg(&sys), render_svg(&sys));
    }

    #[test]
    fn svg_always_emits_a_star_group() {
        // The central star is always grouped, so every render has at least
        // one clickable body carrying kind + name — the one invariant that
        // holds regardless of what `generate_system` rolls.
        let mw = World::from_uwp("Regina", "A788899-A", false, true).unwrap();
        let sys = System::generate_system(mw);
        let svg = render_svg(&sys);
        assert!(svg.contains(r#"data-kind="star""#), "no star group emitted");
        assert!(svg.contains("data-name="), "star group missing data-name");
    }

    #[test]
    fn svg_world_group_carries_full_identity() {
        // A world body must emit kind + name + uwp + orbit + distance data
        // attributes. Seeded generation so the system reliably contains a
        // world (the non-seeded path can roll an all-gas-giant system).
        let mw = World::from_uwp("Regina", "A788899-A", false, true).unwrap();
        let sys = System::generate_system_seeded(0, mw);
        let svg = render_svg(&sys);
        assert!(svg.contains(r#"data-kind="world""#), "no world group");
        assert!(svg.contains("data-uwp="), "world group missing data-uwp");
        assert!(
            svg.contains("data-orbit="),
            "world group missing data-orbit"
        );
        assert!(
            svg.contains("data-distance-mkm="),
            "world group missing data-distance-mkm"
        );
    }

    #[test]
    fn svg_escapes_xml_in_names() {
        // XML metacharacters must be escaped in both the group attributes
        // and text content. Tested at the renderer level so it doesn't
        // depend on the (random) names `generate_system` assigns.
        use render::{BodyKind, BodyMeta, Renderer, SvgRenderer};
        let mut r = SvgRenderer::new(100.0, 100.0);
        r.begin_group(&BodyMeta {
            kind: BodyKind::World,
            name: "A & B <test>".to_string(),
            uwp: Some("X<1>".to_string()),
            orbit: None,
            distance_mkm: None,
        });
        r.fill_text(0.0, 0.0, 12.0, "A & B <test>", (255, 255, 255));
        r.end_group();
        let svg = r.into_string();
        assert!(!svg.contains("A & B <test>"), "raw unescaped name leaked");
        assert!(
            svg.contains("A &amp; B &lt;test&gt;"),
            "escaped name not found in {svg}"
        );
    }

    #[test]
    fn svg_pop_zero_main_world_renders() {
        // Mirrors the pop-0 crash world (The Beyond 2720); must not panic
        // and must still produce a body group.
        let mw = World::from_uwp("Aacheon", "E410000-0", false, true).unwrap();
        let sys = System::generate_system(mw);
        let svg = render_svg(&sys);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(r#"class="sysmap-body""#));
    }
}
