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

pub mod components;
pub mod logging;
pub mod systems;
pub mod trade;
pub mod util;

/// Default UWP (Universal World Profile) used for initial world generation
pub const INITIAL_UPP: &str = "A788899-A";

/// Default name for the initial main world
pub const INITIAL_NAME: &str = "Main World";
