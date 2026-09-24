//! Render backends for the system map.
//!
//! The [`Renderer`] trait abstracts the three primitives the scene needs —
//! filled circle, stroked ellipse, and baseline-anchored text — plus a pair
//! of grouping hooks ([`Renderer::begin_group`] / [`Renderer::end_group`])
//! that let the SVG backend wrap each interactive body in a `<g …>` element
//! while the raster backend ignores them.
//!
//! The scene-walk ([`render_scene`]) is written **once**, generic over the
//! trait, so the PNG and SVG outputs share all layout, geometry, and palette
//! logic and differ only in how the primitives land — raster pixels vs. SVG
//! tags. Neither format is derived from the other; they are two independent
//! sinks fed by one scene description. Mirrors the pattern in
//! `worldmap::render`.

pub mod png;
pub mod svg;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use crate::systems::gas_giant::GasGiant;
use crate::systems::system::{OrbitContent, Star, StarOrbit, System};
use crate::systems::system_tables::get_zone;
use crate::systems::world::World;

use super::colors::*;
use super::geometry::*;
use super::travel::{THRUSTS_G, brachistochrone_secs, format_duration};

pub use png::PngRenderer;
pub use svg::SvgRenderer;

/// Bundled font for label/legend text. Same DejaVu Sans the worldmap
/// renderer uses; shared by the PNG rasteriser ([`png::PngRenderer`]) and
/// the [`text_width`] layout helper.
pub(crate) const FONT_BYTES: &[u8] = include_bytes!("../../../assets/DejaVuSans.ttf");

// ---- Renderer abstraction -------------------------------------------------

/// What kind of body a [`BodyMeta`] describes. Serialised as the SVG
/// `data-kind` attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Star,
    World,
    GasGiant,
    Belt,
    Moon,
}

impl BodyKind {
    fn as_str(self) -> &'static str {
        match self {
            BodyKind::Star => "star",
            BodyKind::World => "world",
            BodyKind::GasGiant => "gas-giant",
            BodyKind::Belt => "belt",
            BodyKind::Moon => "moon",
        }
    }
}

/// Identity of an interactive body, handed to [`Renderer::begin_group`] to
/// wrap that body's drawing calls. The SVG backend serialises these as
/// `data-*` attributes on a `<g class="sysmap-body">`; the raster backend
/// ignores them. Built per-body, so owned `String`s keep the call sites
/// simple — the allocation cost is negligible against drawing.
pub struct BodyMeta {
    pub kind: BodyKind,
    pub name: String,
    pub uwp: Option<String>,
    pub orbit: Option<usize>,
    pub distance_mkm: Option<f32>,
    pub spectral: Option<String>,
}

impl BodyMeta {
    fn new(kind: BodyKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            uwp: None,
            orbit: None,
            distance_mkm: None,
            spectral: None,
        }
    }

    /// Attach an orbit slot index; also fills in the slot's distance in Mkm
    /// so the SVG carries `data-orbit` and `data-distance-mkm` together.
    fn orbit(mut self, orbit: usize) -> Self {
        self.orbit = Some(orbit);
        self.distance_mkm = Some(slot_distance_mkm(orbit));
        self
    }

    fn uwp(mut self, uwp: impl Into<String>) -> Self {
        self.uwp = Some(uwp.into());
        self
    }

    /// Attach the star's spectral classification (e.g. `"G2 V"`), serialised
    /// as the SVG `data-spectral` attribute on a star group.
    fn spectral(mut self, s: impl Into<String>) -> Self {
        self.spectral = Some(s.into());
        self
    }
}

/// The drawing surface the shared scene-walk renders into. Implemented by
/// [`PngRenderer`] (raster) and [`SvgRenderer`] (vector).
pub trait Renderer {
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, rgba: (u8, u8, u8, u8));
    fn stroke_ellipse(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rgba: (u8, u8, u8, u8),
        width: f32,
    );
    fn fill_text(&mut self, x: f32, y: f32, size: f32, text: &str, rgb: (u8, u8, u8));

    /// Open a group around an interactive body's drawing calls. Default is a
    /// no-op (the raster backend has no clickable structure).
    fn begin_group(&mut self, _meta: &BodyMeta) {}

    /// Close the most recently opened group. Default is a no-op.
    fn end_group(&mut self) {}

    /// Whether the backend prefers a single vector belt band over the
    /// per-asteroid scatter. The raster backend returns `false` (keeps the
    /// ~1400-circle scatter, preserving byte-identical output); the SVG
    /// backend returns `true` (one clickable annulus).
    fn vector_belts(&self) -> bool {
        false
    }
}

/// Pixel width of `text` at the given font size — used to right-align /
/// centre header and legend columns. Pure function of the bundled font, so
/// it lives outside the backends and is shared by the scene-walk.
pub(crate) fn text_width(text: &str, size: f32) -> f32 {
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

// ---- Scene entry ----------------------------------------------------------

/// Walk `system` and emit it into `r`. Backend-agnostic: the draw order and
/// every primitive call are identical regardless of sink, so the PNG output
/// is byte-for-byte unchanged from before the trait was introduced (the
/// group hooks are no-ops on the raster backend).
pub(crate) fn render_scene<R: Renderer + ?Sized>(r: &mut R, system: &System) {
    let max_orbit = max_populated_orbit(system).unwrap_or(0);
    // Lay out the central "contact" cluster (primary + any companions
    // whose orbit is `StarOrbit::Primary`). The cluster's effective
    // half-width drives the inner-orbit floor — a binary's two discs
    // push the closest orbit ring outward more than a lone primary would.
    let cluster = central_cluster(system);
    let min_orbit = min_orbit_radius_for(cluster.half_width());

    draw_orbit_rings(r, system, max_orbit, min_orbit);
    draw_jump_shadows(r, system, max_orbit, min_orbit, &cluster);
    for member in &cluster.members {
        r.begin_group(&BodyMeta::new(BodyKind::Star, member.name).spectral(member.star.to_string()));
        draw_star(r, member.star, member.cx, member.cy, member.radius);
        if cluster.members.len() > 1 {
            // For a multi-star contact group, label each star with its own
            // name so the user can tell them apart. The primary's name is
            // already in the header so we skip its label.
            if !member.is_primary {
                draw_label(
                    r,
                    member.cx + member.radius + 4.0,
                    member.cy + member.radius + 12.0,
                    member.name,
                );
            }
        }
        r.end_group();
    }
    draw_bodies(r, system, max_orbit, min_orbit);
    draw_companion_subsystems(r, system, max_orbit, min_orbit);
    draw_far_companions(r, system);
    draw_header(r, system);
    draw_legend(r, system);
}

fn max_populated_orbit(system: &System) -> Option<usize> {
    system
        .orbit_slots
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| slot.as_ref().map(|_| i))
        .max()
}

// ---- Orbits, star, bodies -------------------------------------------------

fn draw_orbit_rings<R: Renderer + ?Sized>(
    r: &mut R,
    system: &System,
    max_orbit: usize,
    min_orbit: f32,
) {
    // Stellar zones drive the orbit colour: inner=blue, habitable=green,
    // outer=red. Each ring is rendered in two passes — a wide, very
    // translucent halo for the soft "glow" feel from the inspiration
    // art, and a sharper line on top to anchor the eye.
    let zones = get_zone(&system.star);
    for (orbit, slot) in system.orbit_slots.iter().enumerate() {
        // Skip both genuinely empty (None) and intentionally Blocked
        // slots: the ring should only appear where there's an actual
        // body to anchor it. A lone orbit line in empty space implies
        // "something is here" — misleading.
        match slot {
            None | Some(OrbitContent::Blocked) => continue,
            _ => {}
        }
        let ring_r = orbit_radius_px(orbit, max_orbit, min_orbit);
        let (cr, cg, cb) = zone_color(orbit, zones.inner, zones.habitable);
        // Glow: thick & dim.
        r.stroke_ellipse(
            STAR_CX,
            STAR_CY,
            ring_r,
            ring_r * TILT_RATIO,
            (cr, cg, cb, 32),
            4.5,
        );
        // Line: thin & bright.
        r.stroke_ellipse(
            STAR_CX,
            STAR_CY,
            ring_r,
            ring_r * TILT_RATIO,
            (cr, cg, cb, 190),
            1.0,
        );
    }
}

/// Mark each star's 100-diameter jump shadow with a faint grey ellipse.
/// The central star's shadow is concentric with the orbit rings; a
/// companion star at `StarOrbit::System(N)` gets its own shadow drawn
/// around its placed position. Far companions are shadow-drawn over in
/// `draw_far_companions` since they live in a separate part of the canvas.
///
/// Gas giants and rocky worlds don't get a shadow — their 100D values are
/// tiny relative to orbit spacing.
fn draw_jump_shadows<R: Renderer + ?Sized>(
    r: &mut R,
    system: &System,
    max_orbit: usize,
    min_orbit: f32,
    cluster: &CentralCluster<'_>,
) {
    for (idx, member) in cluster.members.iter().enumerate() {
        let r_px = mkm_to_pixel_radius(jump_shadow_mkm(member.star), max_orbit, min_orbit);
        r.stroke_ellipse(
            member.cx,
            member.cy,
            r_px,
            r_px * TILT_RATIO,
            JUMP_SHADOW,
            1.0,
        );
        // Only label the primary's shadow to avoid stacking the same text
        // on overlapping ellipses.
        if idx == 0 {
            draw_shadow_label(r, member.cx, member.cy, r_px);
        }
    }

    // Each in-orbit companion's shadow, drawn around its position.
    for (orbit, slot) in system.orbit_slots.iter().enumerate() {
        let companion = match slot {
            Some(OrbitContent::Secondary) => system.secondary.as_deref(),
            Some(OrbitContent::Tertiary) => system.tertiary.as_deref(),
            _ => continue,
        };
        let Some(companion) = companion else { continue };
        let ring_r = orbit_radius_px(orbit, max_orbit, min_orbit);
        let theta = body_angle_rad(orbit);
        let (cx, cy) = body_position(ring_r, theta);
        let shadow_r = mkm_to_pixel_radius(jump_shadow_mkm(&companion.star), max_orbit, min_orbit);
        r.stroke_ellipse(cx, cy, shadow_r, shadow_r * TILT_RATIO, JUMP_SHADOW, 1.0);
        draw_shadow_label(r, cx, cy, shadow_r);
    }
}

/// Anchor the "Jump Shadow" text to the top of the shadow ellipse, just
/// above the ring so it sits over the dark background rather than across an
/// orbit ring or a body.
fn draw_shadow_label<R: Renderer + ?Sized>(r: &mut R, cx: f32, cy: f32, r_px: f32) {
    let label = "Jump Shadow";
    let size = 10.0;
    let width = text_width(label, size);
    let y = cy - r_px * TILT_RATIO - 3.0;
    r.fill_text(
        cx - width * 0.5,
        y,
        size,
        label,
        (JUMP_SHADOW.0, JUMP_SHADOW.1, JUMP_SHADOW.2),
    );
}

fn draw_star<R: Renderer + ?Sized>(r: &mut R, star: &Star, cx: f32, cy: f32, radius: f32) {
    let (sr, sg, sb) = star_color(star.star_type);
    // Soft halo: three concentric translucent discs of decreasing alpha.
    r.fill_circle(cx, cy, radius * 2.6, (sr, sg, sb, 18));
    r.fill_circle(cx, cy, radius * 1.7, (sr, sg, sb, 36));
    r.fill_circle(cx, cy, radius * 1.2, (sr, sg, sb, 80));
    r.fill_circle(cx, cy, radius, (sr, sg, sb, 255));
}

/// One star within the central contact-orbit cluster (primary + any
/// `StarOrbit::Primary` companions).
struct ClusterMember<'a> {
    star: &'a Star,
    name: &'a str,
    cx: f32,
    cy: f32,
    radius: f32,
    is_primary: bool,
}

struct CentralCluster<'a> {
    members: Vec<ClusterMember<'a>>,
}

impl CentralCluster<'_> {
    /// Maximum distance from `STAR_CX` to any star's outer edge — used to
    /// push orbit rings past the entire cluster.
    fn half_width(&self) -> f32 {
        self.members
            .iter()
            .map(|m| (m.cx - STAR_CX).abs() + m.radius)
            .fold(0.0_f32, f32::max)
    }
}

/// Collect the primary plus any `StarOrbit::Primary` companions and lay
/// them out horizontally so they touch (slight overlap for the
/// contact-binary look). Single-star systems return a one-member cluster
/// at the canvas centre, preserving the old behaviour.
fn central_cluster(system: &System) -> CentralCluster<'_> {
    let mut stars: Vec<(&Star, &str, bool)> = vec![(&system.star, system.name.as_str(), true)];
    if let Some(sec) = system.secondary.as_deref()
        && sec.orbit == StarOrbit::Primary
    {
        stars.push((&sec.star, sec.name.as_str(), false));
    }
    if let Some(ter) = system.tertiary.as_deref()
        && ter.orbit == StarOrbit::Primary
    {
        stars.push((&ter.star, ter.name.as_str(), false));
    }

    let radii: Vec<f32> = stars
        .iter()
        .map(|(s, _, _)| star_radius_px(s.size))
        .collect();
    let n = stars.len();

    if n == 1 {
        return CentralCluster {
            members: vec![ClusterMember {
                star: stars[0].0,
                name: stars[0].1,
                cx: STAR_CX,
                cy: STAR_CY,
                radius: radii[0],
                is_primary: true,
            }],
        };
    }

    // Place adjacent stars at distance `(r_a + r_b) * 0.85` — slight
    // overlap so the discs look like a contact pair rather than two
    // separated bodies.
    let mut centers: Vec<f32> = vec![0.0; n];
    for i in 1..n {
        centers[i] = centers[i - 1] + (radii[i - 1] + radii[i]) * 0.85;
    }
    let span = centers[n - 1];
    let offset = STAR_CX - span / 2.0;
    let members = stars
        .into_iter()
        .enumerate()
        .map(|(i, (star, name, is_primary))| ClusterMember {
            star,
            name,
            cx: centers[i] + offset,
            cy: STAR_CY,
            radius: radii[i],
            is_primary,
        })
        .collect();
    CentralCluster { members }
}

fn draw_bodies<R: Renderer + ?Sized>(r: &mut R, system: &System, max_orbit: usize, min_orbit: f32) {
    for (orbit, slot) in system.orbit_slots.iter().enumerate() {
        let Some(content) = slot else { continue };
        let ring_r = orbit_radius_px(orbit, max_orbit, min_orbit);
        let theta = body_angle_rad(orbit);
        let (cx, cy) = body_position(ring_r, theta);
        match content {
            OrbitContent::World(w) => {
                let belt = is_belt(w);
                let kind = if belt {
                    BodyKind::Belt
                } else {
                    BodyKind::World
                };
                r.begin_group(
                    &BodyMeta::new(kind, w.name.clone())
                        .orbit(orbit)
                        .uwp(w.to_uwp()),
                );
                // For a belt `draw_world` only emits the label and returns;
                // the scatter/band is then drawn over it (order preserved
                // from the original so the raster output is unchanged).
                draw_world(r, w, cx, cy);
                if belt {
                    draw_belt(r, ring_r, orbit);
                }
                r.end_group();
            }
            OrbitContent::GasGiant(gg) => {
                r.begin_group(&BodyMeta::new(BodyKind::GasGiant, gg.name.clone()).orbit(orbit));
                draw_gas_giant(r, gg, cx, cy);
                r.end_group();
            }
            OrbitContent::Secondary => {
                if let Some(sec) = system.secondary.as_deref() {
                    r.begin_group(
                        &BodyMeta::new(BodyKind::Star, sec.name.clone())
                            .orbit(orbit)
                            .spectral(sec.star.to_string()),
                    );
                    draw_companion_star(r, &sec.star, &sec.name, cx, cy);
                    r.end_group();
                }
            }
            OrbitContent::Tertiary => {
                if let Some(ter) = system.tertiary.as_deref() {
                    r.begin_group(
                        &BodyMeta::new(BodyKind::Star, ter.name.clone())
                            .orbit(orbit)
                            .spectral(ter.star.to_string()),
                    );
                    draw_companion_star(r, &ter.star, &ter.name, cx, cy);
                    r.end_group();
                }
            }
            OrbitContent::Blocked => {}
        }
    }
}

fn is_belt(w: &World) -> bool {
    // Planetoid belts in the existing system come through as a World with
    // size 0 marked as a belt; we infer from size==0 + an empty
    // hydro/atmosphere here. Worlds with size 0 are otherwise rare, so
    // this is a safe heuristic for v1.
    let uwp = w.to_uwp();
    uwp.starts_with('0') || uwp.contains("Belt") || w.name.eq_ignore_ascii_case("planetoid belt")
}

fn draw_world<R: Renderer + ?Sized>(r: &mut R, w: &World, cx: f32, cy: f32) {
    if is_belt(w) {
        // Belt scatter/band is drawn separately on the orbit ring itself;
        // skip the disc.
        draw_label_colored(r, cx, cy + 14.0, &w.name, LABEL_BELT);
        return;
    }
    let radius = world_radius_px(w.size);
    let (cr, cg, cb) = WORLD_DISC;
    r.fill_circle(cx, cy, radius, (cr, cg, cb, 255));
    draw_moons(r, &w.satellites.sats, cx, cy, radius);
    draw_label(r, cx + radius + 4.0, cy + 4.0, &w.name);
}

fn draw_gas_giant<R: Renderer + ?Sized>(r: &mut R, gg: &GasGiant, cx: f32, cy: f32) {
    let radius = gas_giant_radius_px(gg);
    let (cr, cg, cb) = GAS_GIANT_DISC;
    // Faint banding hint: a slightly darker inner ellipse.
    r.fill_circle(cx, cy, radius, (cr, cg, cb, 255));
    r.fill_circle(
        cx,
        cy - radius * 0.15,
        radius * 0.7,
        (
            cr.saturating_sub(20),
            cg.saturating_sub(20),
            cb.saturating_sub(20),
            200,
        ),
    );
    draw_moons(r, gg.satellites(), cx, cy, radius);
    draw_label_colored(r, cx + radius + 4.0, cy + 4.0, &gg.name, LABEL_GAS_GIANT);
}

/// Render a companion star (secondary or tertiary in a System orbit slot)
/// as a spectral-tinted disc sized by luminosity class, with a soft halo
/// and a name label.
fn draw_companion_star<R: Renderer + ?Sized>(r: &mut R, star: &Star, name: &str, cx: f32, cy: f32) {
    let (sr, sg, sb) = star_color(star.star_type);
    let radius = star_radius_px(star.size);
    r.fill_circle(cx, cy, radius * 2.4, (sr, sg, sb, 24));
    r.fill_circle(cx, cy, radius * 1.5, (sr, sg, sb, 90));
    r.fill_circle(cx, cy, radius, (sr, sg, sb, 255));
    draw_label(r, cx + radius + 4.0, cy + 4.0, name);
}

/// For each `Secondary`/`Tertiary` slot on the primary's orbit list, render
/// a miniature version of the companion's own orbit system next to the
/// companion star marker.
fn draw_companion_subsystems<R: Renderer + ?Sized>(
    r: &mut R,
    system: &System,
    max_orbit: usize,
    min_orbit: f32,
) {
    for (orbit, slot) in system.orbit_slots.iter().enumerate() {
        let companion = match slot {
            Some(OrbitContent::Secondary) => system.secondary.as_deref(),
            Some(OrbitContent::Tertiary) => system.tertiary.as_deref(),
            _ => continue,
        };
        let Some(companion) = companion else { continue };
        let ring_r = orbit_radius_px(orbit, max_orbit, min_orbit);
        let theta = body_angle_rad(orbit);
        let (cx, cy) = body_position(ring_r, theta);
        draw_inline_subsystem(r, companion, cx, cy, 70.0);
    }
}

/// `Far` companions don't appear in the primary's `orbit_slots`, so the
/// main draw loop never sees them. Render them in the bottom strip of the
/// canvas with their own central star and a full inline subsystem.
fn draw_far_companions<R: Renderer + ?Sized>(r: &mut R, system: &System) {
    let slots = [(360.0_f32, 770.0_f32), (1080.0, 770.0)];
    let mut slot_idx = 0usize;
    for companion in [system.secondary.as_deref(), system.tertiary.as_deref()]
        .into_iter()
        .flatten()
    {
        if companion.orbit != StarOrbit::Far {
            continue;
        }
        if slot_idx >= slots.len() {
            break;
        }
        let (cx, cy) = slots[slot_idx];
        slot_idx += 1;
        draw_far_companion(r, companion, cx, cy, "Far");
    }
}

fn draw_far_companion<R: Renderer + ?Sized>(
    r: &mut R,
    comp: &System,
    cx: f32,
    cy: f32,
    role: &str,
) {
    let radius = star_radius_px(comp.star.size);
    let (sr, sg, sb) = star_color(comp.star.star_type);
    r.begin_group(&BodyMeta::new(BodyKind::Star, comp.name.clone()).spectral(comp.star.to_string()));
    r.fill_circle(cx, cy, radius * 2.4, (sr, sg, sb, 24));
    r.fill_circle(cx, cy, radius * 1.5, (sr, sg, sb, 90));
    r.fill_circle(cx, cy, radius, (sr, sg, sb, 255));
    draw_label(
        r,
        cx + radius + 4.0,
        cy + 4.0,
        &format!("{} ({}, {})", comp.name, role, comp.star),
    );
    draw_inline_subsystem(r, comp, cx, cy, 110.0);
    r.end_group();
}

/// Draw a miniature version of `companion`'s orbit rings and bodies centred
/// on `(cx, cy)`, with the outermost orbit at `max_radius_px` from the
/// centre. The companion's central star is NOT drawn here — the caller does.
fn draw_inline_subsystem<R: Renderer + ?Sized>(
    r: &mut R,
    companion: &System,
    cx: f32,
    cy: f32,
    max_radius_px: f32,
) {
    let max_orb = companion
        .orbit_slots
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.as_ref().map(|_| i))
        .max();
    let Some(max_orb) = max_orb else {
        return;
    };
    if max_orb == 0 {
        return;
    }
    let star_r = star_radius_px(companion.star.size);
    let min_radius_px = (star_r + 4.0).max(8.0);
    if min_radius_px >= max_radius_px {
        return;
    }

    let zones = get_zone(&companion.star);

    // Orbit rings.
    for (o, slot) in companion.orbit_slots.iter().enumerate() {
        match slot {
            None | Some(OrbitContent::Blocked) => continue,
            _ => {}
        }
        let t = o as f32 / max_orb as f32;
        let ring_r = min_radius_px + t * (max_radius_px - min_radius_px);
        let (cr, cg, cb) = zone_color(o, zones.inner, zones.habitable);
        r.stroke_ellipse(cx, cy, ring_r, ring_r * TILT_RATIO, (cr, cg, cb, 28), 2.5);
        r.stroke_ellipse(cx, cy, ring_r, ring_r * TILT_RATIO, (cr, cg, cb, 170), 0.6);
    }

    // Bodies.
    for (o, slot) in companion.orbit_slots.iter().enumerate() {
        let Some(content) = slot else { continue };
        let t = o as f32 / max_orb as f32;
        let ring_r = min_radius_px + t * (max_radius_px - min_radius_px);
        let theta = body_angle_rad(o);
        let bx = cx + ring_r * theta.cos();
        let by = cy + ring_r * theta.sin() * TILT_RATIO;
        match content {
            OrbitContent::World(w) => {
                if is_belt(w) {
                    r.begin_group(
                        &BodyMeta::new(BodyKind::Belt, w.name.clone())
                            .orbit(o)
                            .uwp(w.to_uwp()),
                    );
                    draw_inline_belt(r, cx, cy, ring_r, o);
                    r.end_group();
                } else {
                    let wr = (world_radius_px(w.size) * 0.5).max(1.0);
                    r.begin_group(
                        &BodyMeta::new(BodyKind::World, w.name.clone())
                            .orbit(o)
                            .uwp(w.to_uwp()),
                    );
                    r.fill_circle(bx, by, wr, (WORLD_DISC.0, WORLD_DISC.1, WORLD_DISC.2, 255));
                    r.end_group();
                }
            }
            OrbitContent::GasGiant(gg) => {
                let gr = (gas_giant_radius_px(gg) * 0.55).max(2.0);
                r.begin_group(&BodyMeta::new(BodyKind::GasGiant, gg.name.clone()).orbit(o));
                r.fill_circle(
                    bx,
                    by,
                    gr,
                    (GAS_GIANT_DISC.0, GAS_GIANT_DISC.1, GAS_GIANT_DISC.2, 255),
                );
                r.end_group();
            }
            _ => {}
        }
    }
}

/// Miniature belt inside an inline subsystem. Raster sinks scatter ~200
/// rocks (byte-identical to the original); vector sinks emit a single thin
/// band so the SVG stays small.
fn draw_inline_belt<R: Renderer + ?Sized>(r: &mut R, cx: f32, cy: f32, ring_r: f32, orbit: usize) {
    if r.vector_belts() {
        r.stroke_ellipse(
            cx,
            cy,
            ring_r,
            ring_r * TILT_RATIO,
            (BELT_TONE_A.0, BELT_TONE_A.1, BELT_TONE_A.2, 170),
            5.0,
        );
        return;
    }
    let seed = (0x9E37_79B9_u64)
        .wrapping_mul(orbit as u64 + 1)
        .wrapping_add(0xABCD_1234);
    let mut rng = SmallRng::seed_from_u64(seed);
    for _ in 0..200 {
        let phi: f32 = rng.random_range(0.0..std::f32::consts::TAU);
        let dr: f32 = rng.random_range(-2.5..2.5);
        let rr = ring_r + dr;
        let x = cx + rr * phi.cos();
        let y = cy + rr * phi.sin() * TILT_RATIO;
        let tone = if rng.random_bool(0.55) {
            BELT_TONE_A
        } else {
            BELT_TONE_B
        };
        r.fill_circle(x, y, 0.7, (tone.0, tone.1, tone.2, 180));
    }
}

fn draw_moons<R: Renderer + ?Sized>(
    r: &mut R,
    moons: &[World],
    parent_cx: f32,
    parent_cy: f32,
    parent_r: f32,
) {
    if moons.is_empty() {
        return;
    }
    // More moons than fit: choose by population rather than taking the
    // first few in orbital order.
    //
    // Bulhai has eight, and Bulhai Freeport — a class-C port with 20,000
    // people, the reason the system is in an adventure at all — sits last
    // in orbital order. Taking the first four drew three empty rockballs
    // and a ring instead, so the Freeport was not on the map to hover
    // over. Population is the same signal `World::gen_name` already uses
    // to decide whether a body is worth a real name.
    //
    // Selection is by population, ties by orbital order; the survivors are
    // then drawn back in orbital order so inner moons stay inner. Both
    // sorts are stable and key off nothing but the slice, so the output
    // stays a pure function of the system.
    let mut chosen: Vec<(usize, &World)> = moons.iter().enumerate().collect();
    if chosen.len() > MAX_MOONS_DRAWN {
        chosen.sort_by_key(|(idx, m)| (std::cmp::Reverse(m.get_population()), *idx));
        chosen.truncate(MAX_MOONS_DRAWN);
        chosen.sort_by_key(|(idx, _)| *idx);
    }
    for (idx, (_, m)) in chosen.into_iter().enumerate() {
        let orbit_r = moon_orbit_radius_px(parent_r, idx);
        r.stroke_ellipse(
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
        r.begin_group(&BodyMeta::new(BodyKind::Moon, m.name.clone()).uwp(m.to_uwp()));
        r.fill_circle(mx, my, mr, (cr, cg, cb, 255));
        r.end_group();
    }
}

fn draw_label<R: Renderer + ?Sized>(r: &mut R, x: f32, y: f32, text: &str) {
    draw_label_colored(r, x, y, text, LABEL);
}

/// Body-name label in a specific colour. Worlds and stars stay white via
/// [`draw_label`]; gas giants and belts pass a tint so they read apart.
fn draw_label_colored<R: Renderer + ?Sized>(r: &mut R, x: f32, y: f32, text: &str, rgb: (u8, u8, u8)) {
    r.fill_text(x, y, 12.0, text, rgb);
}

/// Main-system planetoid belt. Raster sinks scatter ~1400 rocks
/// (byte-identical to the original); vector sinks emit a single
/// translucent two-tone band so the belt is one clickable region.
fn draw_belt<R: Renderer + ?Sized>(r: &mut R, ring_r: f32, orbit: usize) {
    if r.vector_belts() {
        let band = 2.0 * BELT_SCATTER_PX;
        r.stroke_ellipse(
            STAR_CX,
            STAR_CY,
            ring_r,
            ring_r * TILT_RATIO,
            (BELT_TONE_A.0, BELT_TONE_A.1, BELT_TONE_A.2, 150),
            band * 0.6,
        );
        r.stroke_ellipse(
            STAR_CX,
            STAR_CY,
            ring_r,
            ring_r * TILT_RATIO,
            (BELT_TONE_B.0, BELT_TONE_B.1, BELT_TONE_B.2, 120),
            band * 0.3,
        );
        return;
    }
    // Deterministic per-orbit seed: render is pure given the system, but
    // each belt looks unique because its seed differs by slot.
    let seed = (0x9E37_79B9_u64)
        .wrapping_mul(orbit as u64 + 1)
        .wrapping_add(0x1234_5678);
    let mut rng = SmallRng::seed_from_u64(seed);
    for _ in 0..BELT_SAMPLES {
        let theta: f32 = rng.random_range(0.0..std::f32::consts::TAU);
        let dr: f32 = rng.random_range(-BELT_SCATTER_PX..BELT_SCATTER_PX);
        let rr = ring_r + dr;
        let (x, y) = body_position(rr, theta);
        let tone = if rng.random_bool(0.55) {
            BELT_TONE_A
        } else {
            BELT_TONE_B
        };
        // Vary alpha slightly to avoid a flat-painted look.
        let alpha = rng.random_range(150..=240);
        r.fill_circle(x, y, 1.1, (tone.0, tone.1, tone.2, alpha));
    }
}

// ---- Header / legend ------------------------------------------------------

fn draw_header<R: Renderer + ?Sized>(r: &mut R, system: &System) {
    let x = 40.0;
    let mut y = 60.0;
    r.fill_text(x, y, 28.0, &system.name, LABEL);
    y += 36.0;
    let star_line = format!(
        "{}{} {}",
        format_star_type(&system.star),
        system.star.subtype,
        format_star_size(&system.star),
    );
    r.fill_text(x, y, 18.0, &star_line, LABEL_DIM);
    y += 24.0;
    let comp = match (&system.secondary, &system.tertiary) {
        (Some(_), Some(_)) => "Trinary system",
        (Some(_), None) | (None, Some(_)) => "Binary system",
        (None, None) => "Solitary star",
    };
    r.fill_text(x, y, 16.0, comp, LABEL_DIM);
}

fn format_star_type(star: &Star) -> String {
    format!("{:?}", star.star_type)
}

fn format_star_size(star: &Star) -> String {
    format!("{:?}", star.size)
}

// ---- Legend layout ----------------------------------------------------------
//
// The legend is a table: `Body | Mkm | 1G | 2G | 6G`. The name column keeps
// the left edge it always had; the numeric columns are right-aligned to fixed
// edges measured back from the canvas's right margin, so every row's digits
// line up regardless of how wide its name is.

/// Left edge of the name column. Unchanged from the single-column legend so
/// small systems look as they did, only with more columns to the right.
const LEGEND_X: f32 = CANVAS_W - 360.0;
/// Right edge of the last thrust column: a 24 px margin from the canvas
/// edge.
const LEGEND_RIGHT: f32 = CANVAS_W - 24.0;
/// Right-edge to right-edge spacing of the thrust columns. The widest time
/// string is four glyphs (`3.2h`, `12d`, `81w` — see
/// [`crate::sysmap::travel::format_duration`]), ~28 px at 12 px, leaving a
/// 12 px gutter.
const TIME_COL_PITCH: f32 = 40.0;
/// Right edge of the Mkm column to right edge of the first thrust column.
/// Wider than [`TIME_COL_PITCH`] so the gap between distance and times reads
/// as a column break rather than as one run of numbers.
const DIST_TO_TIME_GAP: f32 = 50.0;
/// Room reserved for the widest Mkm value (`1.47e6`) left of its right edge;
/// names are truncated to end short of it.
const DIST_COL_W: f32 = 48.0;
const LEGEND_FONT: f32 = 12.0;
const LEGEND_LINE_H: f32 = 18.0;
/// Baseline of the "System Objects" title; the column header and rows
/// follow it.
const LEGEND_TITLE_Y: f32 = 60.0;
/// Clearance kept between legend text and the outermost orbit ring where the
/// ring passes under the legend column. Above: a gas giant disc (12 px) on
/// the ring plus the row's descenders. Below: the disc, the body's label
/// (drawn level with it) and the next row's ascenders.
const RING_CLEAR_ABOVE: f32 = 20.0;
const RING_CLEAR_BELOW: f32 = 42.0;

/// Right edges of the thrust columns, in [`THRUSTS_G`] order.
fn time_col_right(i: usize) -> f32 {
    LEGEND_RIGHT - (THRUSTS_G.len() - 1 - i) as f32 * TIME_COL_PITCH
}

/// Right edge of the Mkm column.
fn dist_col_right() -> f32 {
    time_col_right(0) - DIST_TO_TIME_GAP
}

/// The vertical span the outermost orbit ring occupies at the legend's left
/// edge. The legend sits beside the orbit diagram, not beside empty space:
/// the ring's right-hand lobe reaches x = `STAR_CX + MAX_ORBIT_RADIUS` =
/// 1320, which is 80 px *inside* the name column, so any row whose baseline
/// lands in this band is drawn across the ring (and across whatever body sits
/// there). Derived from the same constants the ring is drawn with, so moving
/// the star or resizing the diagram moves the band with it.
fn ring_band_at_legend() -> (f32, f32) {
    let dx = ((LEGEND_X - STAR_CX) / MAX_ORBIT_RADIUS).clamp(-1.0, 1.0);
    let half = MAX_ORBIT_RADIUS * TILT_RATIO * (1.0 - dx * dx).sqrt();
    (STAR_CY - half, STAR_CY + half)
}

/// One row of the legend table.
#[derive(Debug, Clone, PartialEq)]
struct LegendRow {
    name: String,
    /// Parenthesised kind shown after the name (`"World"`, `"Star, Far"`);
    /// kept apart from `name` so truncation shortens the name and never
    /// the kind.
    kind: Option<String>,
    color: (u8, u8, u8),
    dist: String,
    /// Distance the travel-time columns are computed over. `None` prints a
    /// dash: the primary itself, a contact companion (no distance to
    /// travel) and a far companion (no stated distance).
    travel_mkm: Option<f32>,
    jump_limit: bool,
}

/// The legend's rows, top to bottom.
///
/// The primary's jump-limit row is placed **by distance among the bodies**
/// rather than as a footer. The question it answers is "which of these
/// worlds are inside the shadow, and how long to get clear?", and sorted
/// placement answers the first half without any reading at all: every row
/// above the line is inside. Its own time columns answer the second half. A
/// footer would make the reader compare its Mkm against every row's by eye.
///
/// A body exactly at the limit sorts below it — at 100D a ship can jump.
fn legend_rows(system: &System) -> Vec<LegendRow> {
    // Same radius the map's "Jump Shadow" ring is drawn at, so the row and
    // the ring can never disagree.
    let jump_mkm = jump_shadow_mkm(&system.star);
    let mut rows = vec![LegendRow {
        name: system.name.clone(),
        kind: None,
        color: LABEL,
        dist: "0.0".to_string(),
        travel_mkm: None,
        jump_limit: false,
    }];
    let mut jump_row = Some(LegendRow {
        name: "Jump limit (100D)".to_string(),
        kind: None,
        color: LABEL_DIM,
        dist: format_mkm(jump_mkm),
        travel_mkm: Some(jump_mkm),
        jump_limit: true,
    });

    for (orbit, slot) in system.orbit_slots.iter().enumerate() {
        let Some(content) = slot else { continue };
        let dist = slot_distance_mkm(orbit);
        let (name, kind): (&str, &str) = match content {
            OrbitContent::World(w) => (&w.name, if is_belt(w) { "Belt" } else { "World" }),
            OrbitContent::GasGiant(gg) => (&gg.name, "Gas Giant"),
            OrbitContent::Secondary => (
                system.secondary.as_ref().map_or("Secondary", |s| &s.name),
                "Star",
            ),
            OrbitContent::Tertiary => (
                system.tertiary.as_ref().map_or("Tertiary", |s| &s.name),
                "Star",
            ),
            OrbitContent::Blocked => continue,
        };
        if dist >= jump_mkm
            && let Some(j) = jump_row.take()
        {
            rows.push(j);
        }
        // Tint the legend name to match the on-map label so the panel and
        // the map agree at a glance: amber gas giants, tan belts, white rest.
        let color = match kind {
            "Gas Giant" => LABEL_GAS_GIANT,
            "Belt" => LABEL_BELT,
            _ => LABEL,
        };
        rows.push(LegendRow {
            name: name.to_string(),
            kind: Some(kind.to_string()),
            color,
            dist: format_mkm(dist),
            travel_mkm: Some(dist),
            jump_limit: false,
        });
    }
    // Every body is inside the shadow (a giant primary): the limit still
    // belongs after them and before the companions that have no distance.
    rows.extend(jump_row);

    // Companions that aren't at an orbital position — `Primary` (contact
    // binary) and `Far` — don't appear in `orbit_slots`, so append them so
    // the user can see every star the system contains.
    for companion in [system.secondary.as_deref(), system.tertiary.as_deref()]
        .into_iter()
        .flatten()
    {
        let (orbit_label, dist): (&str, &str) = match companion.orbit {
            StarOrbit::Far => ("Far", "Far"),
            StarOrbit::Primary => ("Contact", "—"),
            _ => continue,
        };
        rows.push(LegendRow {
            name: companion.name.clone(),
            kind: Some(format!("Star, {orbit_label}")),
            color: LABEL,
            dist: dist.to_string(),
            travel_mkm: None,
            jump_limit: false,
        });
    }
    rows
}

/// `name  (kind)`, with `name` cut short and ellipsised until the whole
/// string fits `max_w`. Before the travel-time columns the name column had
/// ~240 px and never needed this; it now has ~160, and a long generated name
/// plus a `(Gas Giant)` suffix can exceed that.
fn fit_row_label(name: &str, kind: Option<&str>, max_w: f32) -> String {
    let suffix = kind.map_or(String::new(), |k| format!("  ({k})"));
    let full = format!("{name}{suffix}");
    if text_width(&full, LEGEND_FONT) <= max_w {
        return full;
    }
    let chars: Vec<char> = name.chars().collect();
    (1..chars.len())
        .rev()
        .map(|n| {
            let head: String = chars[..n].iter().collect();
            format!("{}…{suffix}", head.trim_end())
        })
        .find(|s| text_width(s, LEGEND_FONT) <= max_w)
        .unwrap_or(full)
}

fn draw_legend_header<R: Renderer + ?Sized>(r: &mut R, y: f32) {
    r.fill_text(LEGEND_X, y, LEGEND_FONT, "Body", LABEL_DIM);
    draw_right(r, dist_col_right(), y, "Mkm", LABEL_DIM);
    for (i, g) in THRUSTS_G.iter().enumerate() {
        draw_right(r, time_col_right(i), y, &format!("{g}G"), LABEL_DIM);
    }
}

/// Right-align `text` to `right` at the legend font size.
fn draw_right<R: Renderer + ?Sized>(r: &mut R, right: f32, y: f32, text: &str, rgb: (u8, u8, u8)) {
    r.fill_text(right - text_width(text, LEGEND_FONT), y, LEGEND_FONT, text, rgb);
}

fn draw_legend<R: Renderer + ?Sized>(r: &mut R, system: &System) {
    r.fill_text(LEGEND_X, LEGEND_TITLE_Y, 14.0, "System Objects", LABEL);
    // Caption over the thrust columns, so "1G 2G 6G" reads as travel times
    // rather than as a property of each body.
    let caption = "Travel time from primary";
    r.fill_text(
        LEGEND_RIGHT - text_width(caption, 10.0),
        LEGEND_TITLE_Y,
        10.0,
        caption,
        LABEL_DIM,
    );
    let mut y = LEGEND_TITLE_Y + 22.0;
    draw_legend_header(r, y);
    y += LEGEND_LINE_H;

    // The table flows in up to two blocks: above the outermost ring where it
    // passes under the column, then — only for systems too big to fit —
    // resuming below it with the header repeated. Before travel times the
    // single block simply ran down through the ring, drawing rows across
    // outer-orbit bodies and their labels once a system passed ~13 rows.
    let (band_top, band_bottom) = ring_band_at_legend();
    let top_block_max_y = band_top - RING_CLEAR_ABOVE;
    let bottom_block_y = band_bottom + RING_CLEAR_BELOW;
    let max_y = CANVAS_H - 16.0;
    let name_max_w = dist_col_right() - DIST_COL_W - LEGEND_X;
    let mut in_top_block = true;

    for row in legend_rows(system) {
        if in_top_block && y > top_block_max_y {
            in_top_block = false;
            y = bottom_block_y;
            draw_legend_header(r, y);
            y += LEGEND_LINE_H;
        }
        if y > max_y {
            break;
        }
        if row.jump_limit {
            // A miniature of the map's jump-shadow ring, so the row reads as
            // "that grey ellipse" rather than as another body.
            r.stroke_ellipse(LEGEND_X - 9.0, y - 4.0, 5.0, 2.5, JUMP_SHADOW, 1.0);
        }
        let label = fit_row_label(&row.name, row.kind.as_deref(), name_max_w);
        r.fill_text(LEGEND_X, y, LEGEND_FONT, &label, row.color);
        draw_right(r, dist_col_right(), y, &row.dist, LABEL_DIM);
        for (i, g) in THRUSTS_G.iter().enumerate() {
            let t = row.travel_mkm.map_or_else(
                || "—".to_string(),
                |d| format_duration(brachistochrone_secs(f64::from(d), f64::from(*g))),
            );
            draw_right(r, time_col_right(i), y, &t, LABEL_DIM);
        }
        y += LEGEND_LINE_H;
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
    use crate::systems::world::World;

    fn regina() -> System {
        let mw = World::from_uwp("Regina", "A788899-A", false, true).unwrap();
        System::generate_system_seeded(0, mw)
    }

    #[test]
    fn jump_limit_row_sorts_among_bodies_by_distance() {
        let sys = regina();
        let rows = legend_rows(&sys);
        let jump = jump_shadow_mkm(&sys.star);
        assert_eq!(rows.iter().filter(|r| r.jump_limit).count(), 1);
        let idx = rows
            .iter()
            .position(|r| r.jump_limit)
            .expect("legend has a jump-limit row");
        assert_eq!(rows[idx].travel_mkm, Some(jump));
        // Everything listed above the limit is inside it; everything below
        // with a distance is at or beyond it.
        assert!(rows[..idx].iter().filter_map(|r| r.travel_mkm).all(|d| d < jump));
        assert!(
            rows[idx + 1..]
                .iter()
                .filter_map(|r| r.travel_mkm)
                .all(|d| d >= jump)
        );
    }

    #[test]
    fn long_names_truncate_but_keep_their_kind() {
        let max_w = dist_col_right() - DIST_COL_W - LEGEND_X;
        assert_eq!(
            fit_row_label("Regina", Some("World"), max_w),
            "Regina  (World)"
        );
        let long = fit_row_label("Extraordinarily Long Generated Name", Some("Gas Giant"), max_w);
        assert!(long.ends_with("…  (Gas Giant)"), "{long}");
        assert!(text_width(&long, LEGEND_FONT) <= max_w);
    }

    #[test]
    fn legend_columns_fit_the_canvas_and_clear_the_ring() {
        // The widest time the formatter can produce for a real orbit must
        // fit the column pitch with a gutter, and the last column must end
        // inside the canvas.
        assert!(text_width("115w", LEGEND_FONT) < TIME_COL_PITCH - 6.0);
        assert!(time_col_right(THRUSTS_G.len() - 1) <= CANVAS_W);
        // The top block has room for a dozen rows above the ring band, so
        // the common case never splits; the bottom block starts on-canvas.
        let (top, bottom) = ring_band_at_legend();
        assert!(top - RING_CLEAR_ABOVE > LEGEND_TITLE_Y + 22.0 + 12.0 * LEGEND_LINE_H);
        assert!(bottom + RING_CLEAR_BELOW < CANVAS_H - 16.0);
    }
}
