//! Star-type colour mapping and palette constants for the system map
//! renderer. Colours follow the Morgan–Keenan main-sequence colours:
//! O is blue, M is red, with G ≈ Sol-yellow in the middle.
//!
//! Returned tuples are `(R, G, B)` 8-bit. The renderer tints star discs
//! and their halos from these.

use crate::systems::system::StarType;

/// RGB tint for a star of the given spectral type. Values are eyeballed
/// from the standard MK colour chart (chromaticity translated to sRGB
/// at moderate brightness).
pub fn star_color(t: StarType) -> (u8, u8, u8) {
    match t {
        StarType::O => (155, 176, 255),
        StarType::B => (170, 191, 255),
        StarType::A => (213, 224, 255),
        StarType::F => (248, 247, 255),
        StarType::G => (255, 244, 234),
        StarType::K => (255, 210, 161),
        StarType::M => (255, 167, 100),
    }
}

/// Background fill for the whole canvas. Almost-black, slightly blue
/// to stay distinct from pure black UI chrome.
pub const BG: (u8, u8, u8) = (4, 6, 14);

/// Faint white-blue used for moon orbits and any fallback rings. The
/// main system orbits are coloured by stellar zone via [`zone_color`].
pub const ORBIT_RING: (u8, u8, u8, u8) = (180, 200, 230, 100);

/// Light-grey ring used to mark a star's 100-diameter jump shadow —
/// the "no-jump" volume around each star. Transparent enough that
/// it doesn't fight the zone-coloured orbit rings underneath.
pub const JUMP_SHADOW: (u8, u8, u8, u8) = (190, 190, 200, 110);

/// Inner zone (and hot/inside) — cool electric blue. The convention
/// here is *visual*, not physical: inner=close-and-busy reads as a
/// cool blue, matching the inspiration art.
pub const ZONE_INNER: (u8, u8, u8) = (80, 150, 240);
/// Habitable zone — calm green so liveable worlds stand out at a glance.
pub const ZONE_HABITABLE: (u8, u8, u8) = (90, 210, 140);
/// Outer zone — warm red-orange so the cold reaches still read as a
/// distinct band rather than fading into the background.
pub const ZONE_OUTER: (u8, u8, u8) = (235, 110, 90);

/// Pick a zone tint for an orbit slot. Boundary rule matches the
/// existing system-generation code: `orbit <= inner` is the inner
/// band (hot/inside collapsed in), `inner < orbit <= habitable` is
/// the habitable band, anything further out is the outer zone.
pub fn zone_color(orbit: usize, inner: i32, habitable: i32) -> (u8, u8, u8) {
    let o = orbit as i32;
    if o <= inner {
        ZONE_INNER
    } else if o <= habitable {
        ZONE_HABITABLE
    } else {
        ZONE_OUTER
    }
}

/// Default disc colour for terrestrial worlds. The renderer further
/// shades by hydro/atmosphere if those are exposed; v1 just uses this
/// flat tone.
pub const WORLD_DISC: (u8, u8, u8) = (180, 175, 160);

/// Default disc colour for gas giants. A warm beige-tan reads well
/// against the dark background and avoids confusion with stars.
pub const GAS_GIANT_DISC: (u8, u8, u8) = (210, 175, 120);

/// Disc colour for moons. Slightly darker than worlds so they read
/// clearly as subordinate.
pub const MOON_DISC: (u8, u8, u8) = (160, 155, 145);

/// Two warm tones the belt-scatter alternates between (with random
/// jitter), matching the orange-grey feel of the reference image.
pub const BELT_TONE_A: (u8, u8, u8) = (200, 160, 120);
pub const BELT_TONE_B: (u8, u8, u8) = (140, 120, 110);

/// Default text colour for body labels and the legend.
pub const LABEL: (u8, u8, u8) = (220, 220, 230);
/// Subdued text colour for secondary legend rows (distance, AU).
pub const LABEL_DIM: (u8, u8, u8) = (150, 155, 170);
/// Highlight colour for "Amber Zone" and similar advisories.
pub const AMBER: (u8, u8, u8) = (240, 180, 60);
