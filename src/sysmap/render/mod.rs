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
}

impl BodyMeta {
    fn new(kind: BodyKind, name: impl Into<String>) -> Self {
        Self {
            kind,
            name: name.into(),
            uwp: None,
            orbit: None,
            distance_mkm: None,
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
        r.begin_group(&BodyMeta::new(BodyKind::Star, member.name));
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

    let radii: Vec<f32> = stars.iter().map(|(s, _, _)| star_radius_px(s.size)).collect();
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

fn draw_bodies<R: Renderer + ?Sized>(
    r: &mut R,
    system: &System,
    max_orbit: usize,
    min_orbit: f32,
) {
    for (orbit, slot) in system.orbit_slots.iter().enumerate() {
        let Some(content) = slot else { continue };
        let ring_r = orbit_radius_px(orbit, max_orbit, min_orbit);
        let theta = body_angle_rad(orbit);
        let (cx, cy) = body_position(ring_r, theta);
        match content {
            OrbitContent::World(w) => {
                let belt = is_belt(w);
                let kind = if belt { BodyKind::Belt } else { BodyKind::World };
                r.begin_group(&BodyMeta::new(kind, w.name.clone()).orbit(orbit).uwp(w.to_uwp()));
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
                    r.begin_group(&BodyMeta::new(BodyKind::Star, sec.name.clone()).orbit(orbit));
                    draw_companion_star(r, &sec.star, &sec.name, cx, cy);
                    r.end_group();
                }
            }
            OrbitContent::Tertiary => {
                if let Some(ter) = system.tertiary.as_deref() {
                    r.begin_group(&BodyMeta::new(BodyKind::Star, ter.name.clone()).orbit(orbit));
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
        draw_label(r, cx, cy + 14.0, &w.name);
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
    draw_label(r, cx + radius + 4.0, cy + 4.0, &gg.name);
}

/// Render a companion star (secondary or tertiary in a System orbit slot)
/// as a spectral-tinted disc sized by luminosity class, with a soft halo
/// and a name label.
fn draw_companion_star<R: Renderer + ?Sized>(
    r: &mut R,
    star: &Star,
    name: &str,
    cx: f32,
    cy: f32,
) {
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
    r.begin_group(&BodyMeta::new(BodyKind::Star, comp.name.clone()));
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
                    draw_inline_belt(r, cx, cy, ring_r, o);
                } else {
                    let wr = (world_radius_px(w.size) * 0.5).max(1.0);
                    r.fill_circle(bx, by, wr, (WORLD_DISC.0, WORLD_DISC.1, WORLD_DISC.2, 255));
                }
            }
            OrbitContent::GasGiant(gg) => {
                let gr = (gas_giant_radius_px(gg) * 0.55).max(2.0);
                r.fill_circle(
                    bx,
                    by,
                    gr,
                    (GAS_GIANT_DISC.0, GAS_GIANT_DISC.1, GAS_GIANT_DISC.2, 255),
                );
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
    // Each moon gets its own miniature tilted orbit concentric with the
    // parent. Angular position is a golden-angle fan keyed by moon index;
    // pure function of order, no rng.
    for (idx, m) in moons.iter().take(MAX_MOONS_DRAWN).enumerate() {
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
    r.fill_text(x, y, 12.0, text, LABEL);
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

fn draw_legend<R: Renderer + ?Sized>(r: &mut R, system: &System) {
    let x_label = CANVAS_W - 360.0;
    let x_dist = CANVAS_W - 180.0;
    let mut y = 60.0;
    let line_h = 18.0;
    r.fill_text(x_label, y, 14.0, "System Objects", LABEL);
    y += 22.0;
    r.fill_text(x_label, y, 12.0, "Body", LABEL_DIM);
    r.fill_text(x_dist, y, 12.0, "Mkm", LABEL_DIM);
    y += line_h;
    r.fill_text(x_label, y, 12.0, &system.name, LABEL);
    r.fill_text(x_dist, y, 12.0, "0.0", LABEL_DIM);
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
        r.fill_text(x_label, y, 12.0, &row, LABEL);
        let dist_str = format_mkm(dist);
        let dw = text_width(&dist_str, 12.0);
        // right-align distance column
        r.fill_text(x_dist + 60.0 - dw, y, 12.0, &dist_str, LABEL_DIM);
        y += line_h;
        if y > CANVAS_H - 24.0 {
            break;
        }
    }
    // Companions that aren't at an orbital position — `Primary` (contact
    // binary) and `Far` — don't appear in `orbit_slots`, so append them so
    // the user can see every star the system contains.
    for companion in [system.secondary.as_deref(), system.tertiary.as_deref()]
        .into_iter()
        .flatten()
    {
        let (orbit_label, dist_str): (&str, &str) = match companion.orbit {
            StarOrbit::Far => ("Far", "Far"),
            StarOrbit::Primary => ("Contact", "—"),
            _ => continue,
        };
        let row = format!("{}  (Star, {orbit_label})", companion.name);
        r.fill_text(x_label, y, 12.0, &row, LABEL);
        let dw = text_width(dist_str, 12.0);
        r.fill_text(x_dist + 60.0 - dw, y, 12.0, dist_str, LABEL_DIM);
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
