//! Filling a star's orbits: rulebook steps 9 to 13.
//!
//! [`Slot`]s are the orbits while they're being filled. Pinned bodies go in
//! first, then the giants (Section 5.4), the ice belt (Section 5.5), and
//! Table 21 for the rest; the counts are then matched to any published ones,
//! and each body gets its codes (Section 7). The refuelling line (Section
//! 5.6) is read off the result.

use crate::callisto::body::{BodyClass, Fuel, GiantKind, IceAvailability, TransitFuel};
use crate::callisto::dice::Roller;
use crate::callisto::fill::{self, Codes, FirstGiant, Filled, Known};
use crate::callisto::orbits::{Gap, Zone, sig2};
use crate::callisto::star::{StarData, days_at_thrust_1};
use crate::systems::constraint::PartialUwp;

/// One orbit while it is filled.
#[derive(Debug, Clone)]
pub struct Slot {
    pub position: f32,
    pub fill: Fill,
    /// The Book 6 orbit number a source pinned here, if any; kept so the
    /// override facts aimed at that orbit can find it afterwards.
    pub pinned_orbit: Option<i32>,
}

/// What an orbit holds.
#[derive(Debug, Clone)]
pub enum Fill {
    Open,
    MainWorld,
    Giant {
        kind: GiantKind,
        name: Option<String>,
    },
    /// Empty by Table 21, by a source, or cleared by a hot Jupiter.
    Empty,
    Body(Body),
}

/// A belt, world, sub-Neptune or icy dwarf.
#[derive(Debug, Clone)]
pub struct Body {
    pub class: BodyClass,
    pub name: Option<String>,
    /// Columns a source gave.
    pub uwp: Option<PartialUwp>,
    /// Set once the codes are rolled.
    pub codes: Option<Codes>,
    /// Rolled on Table 21 rather than placed by a source or the ice rule;
    /// only these are changed when counts are matched.
    pub rolled: bool,
    /// The charted ice belt of Table 19.
    pub ice_belt: bool,
}

impl Body {
    pub fn new(class: BodyClass) -> Self {
        Body {
            class,
            name: None,
            uwp: None,
            codes: None,
            rolled: false,
            ice_belt: false,
        }
    }
}

impl Slot {
    pub fn zone(&self) -> Zone {
        Zone::at(self.position)
    }

    fn is_open(&self) -> bool {
        matches!(self.fill, Fill::Open)
    }

    fn is_cold_or_outer_open(&self) -> bool {
        self.is_open() && matches!(self.zone(), Zone::Cold | Zone::Outer)
    }
}

/// A body a source pins to a Book 6 orbit number.
#[derive(Debug, Clone)]
pub struct Pin {
    pub book6_orbit: i32,
    pub position_hd: f32,
    pub fill: Fill,
}

/// Put pinned bodies in the orbit nearest their stated distance, or a new
/// orbit at it when nothing free lies within a Table 12 step (×1.25).
pub fn place_pins(slots: &mut Vec<Slot>, pins: Vec<Pin>, notes: &mut Vec<String>) {
    for pin in pins {
        let nearest = slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_open())
            .min_by(|a, b| {
                let da = (a.1.position / pin.position_hd).ln().abs();
                let db = (b.1.position / pin.position_hd).ln().abs();
                da.total_cmp(&db)
            })
            .map(|(i, s)| (i, (s.position / pin.position_hd).ln().abs()));
        match nearest {
            Some((i, d)) if d <= 1.25f32.ln() => {
                slots[i].fill = pin.fill;
                slots[i].pinned_orbit = Some(pin.book6_orbit);
            }
            _ => {
                let position = sig2(pin.position_hd);
                let at = slots.iter().position(|s| s.position > position).unwrap_or(slots.len());
                slots.insert(
                    at,
                    Slot {
                        position,
                        fill: pin.fill,
                        pinned_orbit: Some(pin.book6_orbit),
                    },
                );
                notes.push(format!(
                    "Added an orbit at {position} HD for a body a source places at Book 6 orbit {}",
                    pin.book6_orbit
                ));
            }
        }
    }
}

/// Place giants one at a time (Section 5.4). `first` is Table 18's roll for
/// the first; later giants take the next free Cold or Outer orbit outward.
/// With no Cold or Outer orbit free, an orbit is added at 2.7 HD or 1.75 ×
/// the outermost — for every giant when the count is published, once at
/// most when it was rolled (the rest are dropped).
pub fn place_giants(
    slots: &mut Vec<Slot>,
    giants: Vec<(GiantKind, Option<String>)>,
    first: Option<FirstGiant>,
    count_known: bool,
    gaps: &[Gap],
    notes: &mut Vec<String>,
) {
    let mut last: Option<usize> = None;
    let mut added = 0;
    for (n, (kind, name)) in giants.into_iter().enumerate() {
        let fill = Fill::Giant { kind, name };
        let cold_or_outer_from = |slots: &[Slot], from: usize| {
            slots
                .iter()
                .enumerate()
                .skip(from)
                .find(|(_, s)| s.is_cold_or_outer_open())
                .map(|(i, _)| i)
        };
        let target = if n == 0 {
            let named = match first {
                Some(FirstGiant::Zone(z)) => slots
                    .iter()
                    .position(|s| s.is_open() && s.zone() == z),
                Some(FirstGiant::Migrated) => slots
                    .iter()
                    .position(|s| s.is_open() && matches!(s.zone(), Zone::Temperate | Zone::Hot)),
                Some(FirstGiant::HotJupiter) => {
                    slots.iter().position(|s| s.is_open() && s.zone() == Zone::Inner)
                }
                None => None,
            };
            named.or_else(|| cold_or_outer_from(slots, 0))
        } else {
            last.and_then(|l| cold_or_outer_from(slots, l + 1))
                .or_else(|| cold_or_outer_from(slots, 0))
        };
        let target = match target {
            Some(t) => Some(t),
            None if count_known || added == 0 => {
                added += 1;
                let outermost = slots.last().map_or(0.0, |s| s.position);
                let mut position = sig2((outermost * 1.75).max(2.7));
                while gaps.iter().any(|g| g.contains(position)) {
                    position = sig2(position * 1.75);
                }
                slots.push(Slot {
                    position,
                    fill: Fill::Open,
                    pinned_orbit: None,
                });
                notes.push(format!("Added an orbit at {position} HD for a giant planet"));
                Some(slots.len() - 1)
            }
            None => {
                notes.push("A rolled giant planet found no orbit and was dropped".to_string());
                None
            }
        };
        let Some(t) = target else { continue };
        let hot_jupiter = n == 0 && first == Some(FirstGiant::HotJupiter) && slots[t].zone() == Zone::Inner;
        slots[t].fill = fill;
        last = Some(t);
        // A hot Jupiter has cleared every orbit inside it.
        if hot_jupiter {
            for s in slots[..t].iter_mut().filter(|s| s.is_open()) {
                s.fill = Fill::Empty;
            }
        }
    }
}

/// Table 19's charted ice belt: the outermost orbit beyond the last giant
/// (or the outermost orbit, with no giants), if it is free. Returns whether a
/// belt was placed; if not, the ice is in the last giant's moons.
pub fn place_ice_belt(slots: &mut [Slot], allowed: bool) -> bool {
    if !allowed {
        return false;
    }
    let last_giant = slots.iter().rposition(|s| matches!(s.fill, Fill::Giant { .. }));
    let candidate = slots.len().checked_sub(1).filter(|&i| last_giant.is_none_or(|g| i > g));
    match candidate {
        Some(i) if slots[i].is_open() => {
            let mut belt = Body::new(BodyClass::Belt);
            belt.ice_belt = true;
            slots[i].fill = Fill::Body(belt);
            true
        }
        _ => false,
    }
}

/// Roll Table 21 for every open orbit.
pub fn fill_open(slots: &mut [Slot], small_star: bool, roller: &mut impl Roller) {
    for i in 0..slots.len() {
        if !slots[i].is_open() {
            continue;
        }
        let near_giant = [i.checked_sub(1), Some(i + 1)]
            .into_iter()
            .flatten()
            .filter_map(|j| slots.get(j))
            .any(|s| matches!(s.fill, Fill::Giant { .. }));
        slots[i].fill = match fill::fill(slots[i].zone(), small_star, near_giant, roller) {
            Filled::Empty => Fill::Empty,
            Filled::Body(class) => {
                let mut b = Body::new(class);
                b.rolled = true;
                Fill::Body(b)
            }
        };
    }
}

/// Make the rolled orbits agree with published counts (TravellerMap's PBG
/// belts and its `W` total). The rulebook fixes the counts but not how Table
/// 21 meets them; this keeps as many rolls as it can, changing the outermost
/// rolled orbits first. `None` leaves that count as rolled.
pub fn match_counts(
    slots: &mut Vec<Slot>,
    belts: Option<usize>,
    others: Option<usize>,
    gaps: &[Gap],
) {
    let rolled = |slots: &[Slot], pred: &dyn Fn(&Fill) -> bool| -> Vec<usize> {
        slots
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, s)| match &s.fill {
                Fill::Body(b) => b.rolled && pred(&s.fill),
                Fill::Empty => s.pinned_orbit.is_none() && pred(&s.fill),
                _ => false,
            })
            .map(|(i, _)| i)
            .collect()
    };
    let is_belt = |f: &Fill| matches!(f, Fill::Body(b) if b.class == BodyClass::Belt);
    let is_other = |f: &Fill| matches!(f, Fill::Body(b) if b.class != BodyClass::Belt);
    let is_empty = |f: &Fill| matches!(f, Fill::Empty);
    let count = |slots: &[Slot], pred: &dyn Fn(&Fill) -> bool| {
        slots.iter().filter(|s| pred(&s.fill)).count()
    };
    let make = |class| {
        let mut b = Body::new(class);
        b.rolled = true;
        Fill::Body(b)
    };

    if let Some(want) = belts {
        while count(slots, &is_belt) > want {
            let Some(&i) = rolled(slots, &is_belt).first() else { break };
            slots[i].fill = Fill::Empty;
        }
        while count(slots, &is_belt) < want {
            let i = rolled(slots, &is_empty).first().copied().or_else(|| {
                if others.is_some() { None } else { rolled(slots, &is_other).first().copied() }
            });
            match i {
                Some(i) => slots[i].fill = make(BodyClass::Belt),
                None => {
                    push_orbit(slots, gaps, make(BodyClass::Belt));
                }
            }
        }
    }
    if let Some(want) = others {
        while count(slots, &is_other) > want {
            let Some(&i) = rolled(slots, &is_other).first() else { break };
            slots[i].fill = Fill::Empty;
        }
        while count(slots, &is_other) < want {
            match rolled(slots, &is_empty).last().copied() {
                Some(i) => slots[i].fill = make(BodyClass::World),
                None => push_orbit(slots, gaps, make(BodyClass::World)),
            }
        }
    }
}

/// A new orbit beyond the outermost, for a body a published count needs and
/// no orbit is left for.
fn push_orbit(slots: &mut Vec<Slot>, gaps: &[Gap], fill: Fill) {
    let outermost = slots.last().map_or(0.1, |s| s.position);
    let mut position = sig2(outermost * 1.75);
    while gaps.iter().any(|g| g.contains(position)) {
        position = sig2(position * 1.75);
    }
    slots.push(Slot {
        position,
        fill,
        pinned_orbit: None,
    });
}

/// Roll each body's codes (Section 7.1 to 7.4; Table 22 for the others).
pub fn roll_codes(slots: &mut [Slot], small_star: bool, roller: &mut impl Roller) {
    for s in slots.iter_mut() {
        let zone = s.zone();
        if let Fill::Body(b) = &mut s.fill
            && b.codes.is_none()
        {
            let known = b.uwp.as_ref().map_or(Known::default(), |u| Known {
                size: u.size.map(i32::from),
                atmosphere: u.atmosphere.map(i32::from),
                hydro: u.hydro.map(i32::from),
            });
            b.codes = Some(fill::codes(b.class, zone, small_star, known, roller));
        }
    }
}

/// Is the body in this slot a place to land and melt ice (Section 5.5)?
pub fn is_ice_source(slot: &Slot, ice: IceAvailability) -> bool {
    let cold = matches!(slot.zone(), Zone::Cold | Zone::Outer);
    let Fill::Body(b) = &slot.fill else { return false };
    let codes = b.codes.as_ref();
    b.ice_belt
        || b.class == BodyClass::IcyDwarf
        || (cold && b.class == BodyClass::Belt)
        || (cold && codes.is_some_and(|c| c.composition == Some(crate::callisto::body::Composition::IceRock)))
        || (cold && codes.is_some_and(|c| c.hydro >= 1))
        || (ice == IceAvailability::Rich && slot.zone() == Zone::Outer)
}

/// The refuelling line (Section 5.6), with `names[i]` the name slot `i` ends
/// up with.
pub fn fuel_line(
    slots: &[Slot],
    names: &[String],
    ice: IceAvailability,
    ice_in_giant_moons: bool,
    star: &StarData,
    main_world: Option<usize>,
) -> Fuel {
    let giant = |s: &Slot| matches!(s.fill, Fill::Giant { .. });
    let sources: Vec<usize> = slots
        .iter()
        .enumerate()
        .filter(|(_, s)| giant(s) || is_ice_source(s, ice))
        .map(|(i, _)| i)
        .collect();
    let transit = if slots.iter().any(giant) {
        TransitFuel::GiantPlanet
    } else if ice != IceAvailability::Sparse
        || ice_in_giant_moons
        || slots.iter().any(|s| is_ice_source(s, ice))
    {
        TransitFuel::ChartedIce
    } else {
        TransitFuel::UnchartedIce
    };
    // The nearest source by travel time: the time is read at the further of
    // the two positions, so the innermost source wins.
    let local = main_world.and_then(|mw| {
        let mw_pos = slots[mw].position;
        sources
            .iter()
            .map(|&i| (i, slots[i].position.max(mw_pos)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, pos)| (i, days_at_thrust_1(pos * star.hd_mkm)))
    });
    Fuel {
        ice,
        transit,
        local_days: local.map(|(_, d)| d),
        local_source: local.map(|(i, _)| names[i].clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callisto::dice::{Kind::*, Scripted};

    fn slots(positions: &[f32]) -> Vec<Slot> {
        positions
            .iter()
            .map(|&position| Slot {
                position,
                fill: Fill::Open,
                pinned_orbit: None,
            })
            .collect()
    }

    /// Example 12.1: one giant, no Cold or Outer orbit, so one is added at
    /// 1.75 × 1.6 = 2.8.
    #[test]
    fn a_giant_with_nowhere_to_go_gets_an_orbit() {
        let mut s = slots(&[0.45, 0.61, 1.0, 1.6]);
        s[2].fill = Fill::MainWorld;
        let mut notes = Vec::new();
        place_giants(
            &mut s,
            vec![(GiantKind::SaturnClass, None)],
            Some(FirstGiant::Zone(Zone::Cold)),
            false,
            &[],
            &mut notes,
        );
        assert_eq!(s.len(), 5);
        assert_eq!(s[4].position, 2.8);
        assert!(matches!(s[4].fill, Fill::Giant { kind: GiantKind::SaturnClass, .. }));
        // Nothing lies beyond it, so the ice is in its moons.
        assert!(!place_ice_belt(&mut s, true));
    }

    /// Noricum: two ice giants into the first free Outer orbits.
    #[test]
    fn later_giants_go_outward() {
        let mut s = slots(&[0.36, 0.74, 1.4, 2.0, 34.0, 76.0, 100.0]);
        s[2].fill = Fill::MainWorld;
        place_giants(
            &mut s,
            vec![(GiantKind::IceGiant, None), (GiantKind::IceGiant, None)],
            Some(FirstGiant::Zone(Zone::Outer)),
            true,
            &[],
            &mut Vec::new(),
        );
        assert!(matches!(s[4].fill, Fill::Giant { .. }));
        assert!(matches!(s[5].fill, Fill::Giant { .. }));
        // The orbit beyond the last giant takes the charted ice belt.
        assert!(place_ice_belt(&mut s, true));
        assert!(matches!(&s[6].fill, Fill::Body(b) if b.ice_belt));
    }

    #[test]
    fn a_hot_jupiter_clears_the_orbits_inside_it() {
        let mut s = slots(&[0.1, 0.2, 0.3, 1.0]);
        s[3].fill = Fill::MainWorld;
        place_giants(
            &mut s,
            vec![(GiantKind::JupiterClass, None)],
            Some(FirstGiant::HotJupiter),
            false,
            &[],
            &mut Vec::new(),
        );
        assert!(matches!(s[0].fill, Fill::Giant { .. }));
        let mut s = slots(&[0.1, 0.2, 0.3, 1.0]);
        s[1].fill = Fill::Empty;
        s[3].fill = Fill::MainWorld;
        place_giants(
            &mut s,
            vec![(GiantKind::JupiterClass, None)],
            Some(FirstGiant::HotJupiter),
            false,
            &[],
            &mut Vec::new(),
        );
        assert!(matches!(s[0].fill, Fill::Giant { .. }));
    }

    #[test]
    fn counts_are_matched_to_the_published_ones() {
        let mut s = slots(&[0.3, 0.5, 1.0, 1.5, 2.0, 3.0]);
        s[2].fill = Fill::MainWorld;
        // Rolls: World, World, World, Belt, World (Inner 7, Hot 7, Temperate
        // 7, Cold 4, Outer 7).
        fill_open(&mut s, false, &mut Scripted::new(&[(D2, 7), (D2, 7), (D2, 7), (D2, 4), (D2, 7)]));
        match_counts(&mut s, Some(0), Some(2), &[]);
        let belts = s.iter().filter(|x| matches!(&x.fill, Fill::Body(b) if b.class == BodyClass::Belt)).count();
        let others = s.iter().filter(|x| matches!(&x.fill, Fill::Body(b) if b.class != BodyClass::Belt)).count();
        assert_eq!((belts, others), (0, 2));
        // The inner rolls are the ones kept.
        assert!(matches!(s[0].fill, Fill::Body(_)));
        assert!(matches!(s[1].fill, Fill::Body(_)));
    }
}
