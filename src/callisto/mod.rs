//! Callisto star-system generation.
//!
//! The replacement for Book 6 system generation. The rules live in
//! `docs/callisto/source.md` (the rulebook) and `docs/callisto/IMPLEMENTATION.md`
//! (how they map onto this crate); the rulebook's tables live, as data, in
//! `docs/callisto/tables.json`, so a table edit in the rulebook is a data
//! change here rather than a code change.
//!
//! [`generate::generate_from_constraints_seeded`] is the entry point, taking
//! the same [`crate::SystemConstraints`] Book 6 does and returning the same
//! [`crate::systems::system::System`], with [`layout::Layout`] saying where
//! each orbit really is.

pub mod body;
pub mod dice;
pub mod fill;
pub mod generate;
pub mod layout;
pub mod orbits;
pub mod populate;
pub mod star;
pub mod stars;
pub mod tables;
