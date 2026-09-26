//! Callisto star-system generation.
//!
//! The replacement for Book 6 system generation. The rules live in
//! `docs/callisto/source.md` (the rulebook) and `docs/callisto/IMPLEMENTATION.md`
//! (how they map onto this crate); the rulebook's tables live, as data, in
//! `docs/callisto/tables.json`, so a table edit in the rulebook is a data
//! change here rather than a code change.

pub mod tables;
