//! Dice for the Callisto generator.
//!
//! Every roll goes through a [`Roller`], so the same generator code runs on
//! the seeded worldgen RNG in production and on a script of listed results
//! in tests: the rulebook's worked examples (Section 12) give each roll as a
//! result ("2D = 5"), and [`Scripted`] replays them in order.

use std::collections::VecDeque;

use crate::util::{roll_1d6, roll_2d6};

/// A source of dice results. Only the dice the rulebook uses: 1D, 2D, and
/// D3 (one die halved, rounded up).
pub trait Roller {
    fn d1(&mut self) -> i32;
    fn d2(&mut self) -> i32;
    fn d3(&mut self) -> i32 {
        (self.d1() + 1) / 2
    }
}

/// The worldgen RNG: the thread-local stream that `util::RngScope` seeds, so
/// a Callisto system regenerates identically from its system seed.
pub struct Rng;

impl Roller for Rng {
    fn d1(&mut self) -> i32 {
        roll_1d6()
    }
    fn d2(&mut self) -> i32 {
        roll_2d6()
    }
}

/// Replays listed results, as the worked examples print them. Each entry is
/// the *total* for one roll, whichever dice it was; a D3 is listed as its
/// result too.
///
/// Running out of results panics with the roll it was asked for, which is
/// what a test wants: the generator made a roll the example doesn't list.
pub struct Scripted {
    rolls: VecDeque<(Kind, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    D1,
    D2,
    D3,
}

impl Scripted {
    pub fn new(rolls: &[(Kind, i32)]) -> Self {
        Scripted {
            rolls: rolls.iter().copied().collect(),
        }
    }

    /// Results left unused. A worked example that leaves rolls over has
    /// taken a different path from the one the example describes.
    pub fn remaining(&self) -> usize {
        self.rolls.len()
    }

    fn next(&mut self, want: Kind) -> i32 {
        let (kind, n) = self
            .rolls
            .pop_front()
            .unwrap_or_else(|| panic!("script ran out: generator asked for a {want:?}"));
        assert_eq!(kind, want, "script listed a {kind:?} = {n} but the generator rolled {want:?}");
        n
    }
}

impl Roller for Scripted {
    fn d1(&mut self) -> i32 {
        self.next(Kind::D1)
    }
    fn d2(&mut self) -> i32 {
        self.next(Kind::D2)
    }
    fn d3(&mut self) -> i32 {
        self.next(Kind::D3)
    }
}
