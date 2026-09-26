//! What Callisto knows about a star's orbits, carried on [`System`].
//!
//! Book 6 systems leave `System::callisto` empty and are drawn from their
//! orbit slots exactly as before. A Callisto system fills it: one
//! [`OrbitInfo`] per orbit slot, in the same order, giving the slot the real
//! position the rulebook placed it at.
//!
//! [`System`]: crate::systems::system::System

use crate::callisto::orbits::{Gap, Zone};
use crate::callisto::star::StarData;

/// One star's Callisto layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The star's Table 5/6 figures.
    pub star: StarData,
    /// One entry per `System::orbit_slots` slot, same index.
    pub orbits: Vec<OrbitInfo>,
    /// Positions (HD) rolled inside a companion's gap and crossed out.
    pub crossed_out: Vec<f32>,
    /// The ranges companions keep clear, in this star's HD.
    pub gaps: Vec<Gap>,
    /// For a companion: its separation from the star it orbits, in *that*
    /// star's HD and in Mkm.
    pub separation: Option<Separation>,
    /// Things the generator had to do or could not do, in words, for the
    /// record (a companion moved to keep the habitable zone stable, a main
    /// world hosted by a companion). Stage 3 turns these into `Fit`.
    pub notes: Vec<String>,
}

/// Where an orbit slot sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitInfo {
    pub position_hd: f32,
    pub distance_mkm: f32,
    pub zone: Zone,
}

/// A companion's separation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Separation {
    pub hd: f32,
    pub mkm: f32,
}

impl Layout {
    /// Where orbit slot `slot` sits, if the layout covers it.
    pub fn orbit(&self, slot: usize) -> Option<&OrbitInfo> {
        self.orbits.get(slot)
    }
}

/// Book 6 orbit numbers in AU (IMPLEMENTATION.md §3), for converting the
/// orbit numbers that overrides and `main_world_orbit` still use.
const BOOK6_ORBIT_AU: [f32; 20] = [
    0.2, 0.4, 0.7, 1.0, 1.6, 2.8, 5.2, 10.0, 19.6, 38.8, 77.2, 154.0, 307.0, 615.0, 1230.0,
    2500.0, 4900.0, 9800.0, 19600.0, 39500.0,
];

/// A Book 6 orbit number as a distance in Mkm.
pub fn book6_orbit_mkm(orbit: i32) -> f32 {
    let i = orbit.clamp(0, BOOK6_ORBIT_AU.len() as i32 - 1) as usize;
    BOOK6_ORBIT_AU[i] * 149.6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book6_orbit_3_is_one_au() {
        assert_eq!(book6_orbit_mkm(3), 149.6);
        assert_eq!(book6_orbit_mkm(-1), book6_orbit_mkm(0));
    }
}
