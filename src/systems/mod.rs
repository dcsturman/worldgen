//! # Systems Module
//!
//! This module contains all the core functionality for generating Traveller solar systems,
//! including stellar mechanics, world generation, orbital mechanics, and system-wide
//! characteristics. It provides the building blocks for creating realistic star systems
//! with multiple stars, worlds, gas giants, and their satellites.
//!
//! ## Module Organization
//!
//! - [`astro`] - Astronomical calculations and stellar mechanics
//! - [`gas_giant`] - Gas giant generation and characteristics  
//! - [`has_satellites`] - Satellite generation for worlds and gas giants
//! - [`name_tables`] - Random name generation tables for worlds and features
//! - [`system`] - Main system generation logic and coordination
//! - [`system_tables`] - Lookup tables for system generation rules
//! - [`world`] - Individual world generation and Universal World Profile (UWP) handling
//!
//! ## Usage
//!
//! The primary entry point is the [`system::System`] struct, which coordinates
//! the generation of complete solar systems from a main world specification.
//! Individual components can also be used independently for specific generation tasks.
//!
//! ## Examples
//!
//! ```rust,ignore
//! use worldgen::systems::{system::System, world::World};
//!
//! // Generate a complete system from a main world UWP
//! let main_world = World::from_upp("Regina".to_string(), "A788899-A", false, true).unwrap();
//! let system = System::generate_system(main_world);
//! ```

pub mod astro;
pub mod gas_giant;
pub mod has_satellites;
pub mod name_tables;
pub mod system;
pub mod system_tables;
pub mod world;
