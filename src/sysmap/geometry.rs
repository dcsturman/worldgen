//! Geometry helpers for the system-map renderer.
//!
//! Two coordinate systems matter here:
//!
//! - **System-radial:** the orbit number 0..=19. Real distances span
//!   ~30 Mkm to ~5.9 Bkm — a 200,000× ratio. Linear plotting collapses
//!   the inner system to a dot, so we use a *log-of-distance* mapping.
//!   Conveniently, the orbit table is roughly geometric in slot index
//!   (each slot ≈2× the previous past slot 6), so plotting linearly in
//!   slot index *is* log-of-distance. We do that.
//! - **Pixel:** standard top-left origin. The orbit pattern is centred
//!   on `STAR_CX, STAR_CY`. Each orbit is an ellipse whose y-axis is
//!   shrunk by `TILT_RATIO` to give the oblique "tilted bowl" look of
//!   the reference image.
//!
//! Body angular position uses the golden-angle fan (137.5°) seeded by
//! orbit slot index, which scatters bodies cleanly around the rings
//! without obvious lining-up.

use crate::systems::gas_giant::{GasGiant, GasGiantSize};
use crate::systems::system::StarSize;
use crate::systems::system_tables::get_orbital_distance;

/// Canvas pixel dimensions. Matches a 1.75:1 aspect roughly comparable
/// to the reference image so the layout reads the same way.
pub const CANVAS_W: f32 = 1600.0;
pub const CANVAS_H: f32 = 900.0;

/// Centre of the orbit pattern. Pushed left of dead-centre so the
/// right-hand legend column has room to breathe.
pub const STAR_CX: f32 = 720.0;
pub const STAR_CY: f32 = CANVAS_H * 0.5;

/// Maximum semi-major axis in pixels for the outermost populated orbit.
/// Sized so the ellipse never crosses into the legend column.
pub const MAX_ORBIT_RADIUS: f32 = 600.0;
/// Absolute floor on the inner orbit radius: the inner ring never sits
/// closer than this to the canvas centre, even for tiny dwarf stars.
pub const MIN_ORBIT_RADIUS_FLOOR: f32 = 40.0;
/// Padding (pixels) between the central star's disc edge and the first
/// orbit ring. For supergiants this is what pushes the inner rings out
/// so they don't slice through the disc.
pub const STAR_ORBIT_PADDING: f32 = 12.0;

/// Vertical squash factor for the tilt projection. cos(70°) ≈ 0.342;
/// a slightly less aggressive 0.36 keeps inner ellipses readable.
pub const TILT_RATIO: f32 = 0.36;

/// Belt-scatter ring half-width in pixels; samples are scattered within
/// `±BELT_SCATTER_PX` along the radial axis.
pub const BELT_SCATTER_PX: f32 = 9.0;
/// How many tiny rocks make up a belt's scatter. Higher = denser belt.
pub const BELT_SAMPLES: usize = 1400;

/// Effective minimum orbit radius (pixels) given the central star's
/// disc radius — orbits are pushed outward enough that the innermost
/// ring sits clear of the star.
pub fn min_orbit_radius_for(central_star_r: f32) -> f32 {
    (central_star_r + STAR_ORBIT_PADDING).max(MIN_ORBIT_RADIUS_FLOOR)
}

/// Convert an orbit slot index (0..=max_orbit) to a pixel ring radius
/// using a linear-in-slot mapping. Because the underlying distance
/// table is geometric, this corresponds to a log mapping in km. The
/// `min_radius` is computed from the central star size — see
/// [`min_orbit_radius_for`].
pub fn orbit_radius_px(orbit: usize, max_orbit: usize, min_radius: f32) -> f32 {
    if max_orbit == 0 {
        return min_radius;
    }
    let t = orbit as f32 / max_orbit as f32;
    min_radius + t * (MAX_ORBIT_RADIUS - min_radius)
}

/// Project an orbit + angle to a pixel centre. `theta_rad` is measured
/// from the +x axis (3 o'clock), positive counter-clockwise.
pub fn body_position(orbit_radius_px: f32, theta_rad: f32) -> (f32, f32) {
    let x = STAR_CX + orbit_radius_px * theta_rad.cos();
    let y = STAR_CY + orbit_radius_px * theta_rad.sin() * TILT_RATIO;
    (x, y)
}

/// Deterministic angular placement for a body in slot `orbit`. Uses
/// the golden angle (137.5°) so consecutive orbits don't line up,
/// while still being a pure function of slot index (no rng).
pub fn body_angle_rad(orbit: usize) -> f32 {
    // 137.5° in radians = 2.39996; we offset by ~30° so the innermost
    // orbit doesn't sit dead-on the +x axis.
    let golden = 2.399_963_2_f32;
    let offset = std::f32::consts::FRAC_PI_6;
    offset + (orbit as f32) * golden
}

/// Disc radius (pixels) for a terrestrial world of the given UWP `size`
/// digit (0..=10). Deliberately small so worlds don't overlap their
/// neighbour orbits — labels carry the precision; the disc is a marker.
pub fn world_radius_px(uwp_size: i32) -> f32 {
    let s = uwp_size.clamp(0, 10) as f32;
    2.0 + s * 0.28
}

/// Disc radius (pixels) for a gas giant given its real radius in km.
/// Maps the spec's 20,000–100,000 km range to 7–12 px, with `Small`
/// skewed to the bottom of the band so the size class is unambiguous.
pub fn gas_giant_radius_px(gg: &GasGiant) -> f32 {
    let km = gg.radius_km as f32;
    let t = ((km - 20_000.0) / 80_000.0).clamp(0.0, 1.0);
    let base = 7.0 + t * 5.0;
    match gg.size {
        GasGiantSize::Small => base - 0.3,
        GasGiantSize::Large => base,
    }
}

/// Disc radius (pixels) for a moon. Tiny — moons sit on miniature
/// orbits around their parent, so they need to read as smaller than
/// any world.
pub fn moon_radius_px(uwp_size: i32) -> f32 {
    let s = uwp_size.max(0) as f32;
    (1.0 + s * 0.1).clamp(1.0, 2.0)
}

/// Disc radius (pixels) for any star — central or companion — keyed
/// to its luminosity class. Sizes aren't physically to scale (a real
/// B3 III is ~12x bigger than a G2V), but they preserve the ordering
/// so a III giant reads clearly larger than a V dwarf and a Ia
/// supergiant dominates. Used for both the central star and any
/// secondary/tertiary companion, so giant companions of dwarf
/// primaries correctly out-mass their host on the map.
pub fn star_radius_px(size: StarSize) -> f32 {
    match size {
        StarSize::D => 3.0,
        StarSize::VI => 11.0,
        StarSize::V => 16.0,
        StarSize::IV => 21.0,
        StarSize::III => 27.0,
        StarSize::II => 33.0,
        StarSize::Ib => 38.0,
        StarSize::Ia => 42.0,
    }
}

/// Pixel gap from a parent body's edge out to its first moon orbit.
pub const MOON_ORBIT_GAP: f32 = 4.0;
/// Radial pixel step between successive moon orbits.
pub const MOON_ORBIT_STEP: f32 = 3.0;
/// Maximum moons drawn around a single parent. Beyond this their
/// orbits collide with adjacent system orbit rings; the legend would
/// still list them if/when a moon legend is added.
pub const MAX_MOONS_DRAWN: usize = 4;

/// Semi-major axis (pixels) for the `moon_idx`-th moon orbit around
/// a parent whose disc radius is `parent_r`.
pub fn moon_orbit_radius_px(parent_r: f32, moon_idx: usize) -> f32 {
    parent_r + MOON_ORBIT_GAP + moon_idx as f32 * MOON_ORBIT_STEP
}

/// Deterministic angular position (radians) for the `moon_idx`-th
/// moon. Golden-angle fan so consecutive moons don't line up.
pub fn moon_angle_rad(moon_idx: usize) -> f32 {
    let golden = 2.399_963_2_f32;
    let offset = std::f32::consts::FRAC_PI_4;
    offset + (moon_idx as f32) * golden
}

/// Real distance (millions of km) for a primary-orbit slot. Convenience
/// re-export so renderer code doesn't reach into `system_tables`.
pub fn slot_distance_mkm(orbit: usize) -> f32 {
    get_orbital_distance(orbit as i32)
}
