//! # Worldgen - A set of Traveller tools
//!
//! Worldgen started as a world generator, but has evolved into now two different (and possibly more)
//! tools.  The first is a web application for generating Traveller solar systems and supporting
//! materials for those systems. It takes a world name and a UWP for the main world
//! and can generate either or both of the following:
//!
//! 1. A full solar system with the main world at the center and up to two companion stars
//! 2. A trade table of available goods for the main world
//!
//! The second tool is a trade computer that can calculate trade goods, passengers, and cargo opportunities between worlds.
//! It not only generates trade tables and available passengers and freight based on source and destination world,
//! but allows all these to be selectable to generate a "manifest" of goods, passengers, and freight for a ship to transport and
//! then determine the profit and loss for the ship on that transit.
//!
//! The entire system is written in Rust using Leptos as a reactive front-end framework.

// Backend module is only available when the "backend" feature is enabled (native builds only)
#[cfg(feature = "backend")]
pub mod backend;

// Always-compiled modules — these are the library surface. A consumer
// depending on worldgen as a Cargo dep (with `default-features = false`)
// sees these and nothing else.
pub mod api;
pub mod seed;
pub mod sysmap;
pub mod systems;
pub mod trade;
pub mod util;
pub mod worldmap;

// Top-level re-exports of the library's "primary surface" — what an
// external consumer needs to write `worldgen::generate_system_png(...)`
// without digging into nested module paths. The constraint types come
// out next to them so consumers can build a `SystemConstraints` value
// in one `use` statement.
pub use api::{
    StarSpec, WorldgenError, build_constraints, generate_planet_png, generate_system_png,
    generate_system_png_scaled,
};
pub use systems::constraint::{Constraint, PartialUwp, SystemConstraints};
pub use systems::gas_giant::GasGiantSize;
pub use systems::system::{StarOrbit, StarSize, StarType};

// Frontend-only modules (Leptos UI, URL-driven logging). Gated so library
// consumers don't transitively pull in Leptos.
#[cfg(feature = "frontend")]
pub mod components;
#[cfg(feature = "frontend")]
pub mod logging;

// Trade-computer wire types, WebSocket client, and ship simulator. Shared
// between the WASM client and the native server (TradeState is the
// authoritative example — see CLAUDE.md), so compiled when either feature
// is on. Library consumers don't need any of this.
#[cfg(any(feature = "frontend", feature = "backend"))]
pub mod comms;
#[cfg(any(feature = "frontend", feature = "backend"))]
pub mod simulator;

/// Default UWP (Universal World Profile) used for initial world generation
pub const INITIAL_UWP: &str = "A788899-A";

/// Default name for the initial main world
pub const INITIAL_NAME: &str = "Main World";
