//! # Components Module
//!
//! This module contains all the user interface components for the Worldgen web application.
//! Built using the Leptos reactive framework, these components provide interactive interfaces
//! for generating Traveller star systems, computing trade routes, and displaying results.
//!
//! ## Component Architecture
//!
//! The application follows a modular component design where each major feature is
//! implemented as a separate component with its own state management and user interface.
//! Components communicate through Leptos signals and reactive stores for state sharing.
//!
//! ## Available Components
//!
//! ### Core Application Components
//!
//! - [`selector`] - Main application selector for choosing between tools
//! - [`system_generator`] - Complete star system generation interface
//! - [`trade_computer`] - Trade route calculation and manifest management
//!
//! ### Display and Visualization Components
//!
//! - [`system_view`] - Visual representation of generated star systems
//! - [`world_list`] - Tabular display of worlds and their characteristics
//! - [`traveller_map`] - Integration with Traveller Map services
//!
//! ## Component Responsibilities
//!
//! ### [`selector`]
//! The main entry point component that presents users with options to:
//! - Generate complete star systems from world UWPs
//! - Calculate trade routes and cargo manifests
//! - Access different tools based on URL routing
//!
//! ### [`system_generator`]
//! Comprehensive system generation interface providing:
//! - Input forms for world names and Universal World Profiles (UWPs)
//! - System generation controls and options
//! - Integration with system display components
//! - Export and sharing functionality
//!
//! ### [`trade_computer`]
//! Advanced trade calculation interface featuring:
//! - Source and destination world selection
//! - Available goods market generation
//! - Passenger and freight opportunity calculation
//! - Ship manifest creation and profit/loss analysis
//!
//! ### [`system_view`]
//! Visual system representation showing:
//! - Orbital diagrams with stars, worlds, and gas giants
//! - Stellar zone boundaries and habitable regions
//! - Interactive elements for detailed world information
//! - Scalable display for different system complexities
//!
//! ### [`world_list`]
//! Tabular data presentation including:
//! - Sortable columns for world characteristics
//! - UWP display with trade classifications
//! - Astronomical data and orbital information
//! - Export capabilities for generated data
//!
//! ### [`traveller_map`]
//! External service integration for:
//! - Traveller Map API connectivity
//! - Sector and subsector data retrieval
//! - Coordinate system integration
//! - Official universe data cross-referencing
//!
//! ## State Management
//!
//! Components use Leptos reactive primitives for state management:
//! - **Signals**: For simple reactive values
//! - **Stores**: For complex nested data structures
//! - **Resources**: For asynchronous data loading
//! - **Actions**: For user-triggered operations
//!
//! ## Styling and Layout
//!
//! All components are styled using:
//! - Bootstrap CSS framework for responsive layout
//! - Custom CSS for Traveller-specific styling
//! - Embedded styles within Leptos view macros
//! - Consistent color schemes and typography
//!
//! ## Usage Examples
//!
//! ```rust,ignore
//! use leptos::prelude::*;
//! use worldgen::components::{selector::Selector, system_generator::World};
//!
//! #[component]
//! fn App() -> impl IntoView {
//!     let path = window().location().pathname().unwrap_or_default();
//!     
//!     if path.contains("world") {
//!         view! { <World /> }
//!     } else {
//!         view! { <Selector /> }
//!     }
//! }
//! ```
//!
//! ## Component Communication
//!
//! Components communicate through several mechanisms:
//! - **Props**: For parent-to-child data flow
//! - **Callbacks**: For child-to-parent event handling
//! - **Global Stores**: For application-wide state
//! - **URL Parameters**: For shareable application state
//!
//! ## Responsive Design
//!
//! All components are designed to work across different screen sizes:
//! - Mobile-first responsive design principles
//! - Collapsible navigation and panels
//! - Touch-friendly interactive elements
//! - Scalable text and spacing

pub mod selector;
pub mod system_generator;
pub mod system_view;
pub mod trade_computer;
pub mod traveller_map;
pub mod world_list;
