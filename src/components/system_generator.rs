//! # System Generator Component
//! 
//! This module provides the main user interface component for generating complete
//! Traveller star systems from Universal World Profiles (UWPs). It combines world
//! input forms, system generation logic, and display components into a cohesive
//! interface for creating detailed stellar systems.
//! 
//! ## Component Overview
//! 
//! The `World` component serves as the primary interface for the system generator,
//! providing users with:
//! - Interactive world search and UWP input
//! - Real-time system generation based on world parameters
//! - Integration with Traveller Map services for official world data
//! - Comprehensive system visualization and data display
//! 
//! ## Key Features
//! 
//! ### Dynamic System Generation
//! - Automatically generates complete star systems when world parameters change
//! - Supports custom world names and UWP strings
//! - Integrates trade classification generation
//! - Handles coordinate system integration
//! 
//! ### Traveller Map Integration
//! - Connects to official Traveller Map services
//! - Allows selection of canonical worlds from the official universe
//! - Automatically populates UWP data from selected worlds
//! - Supports coordinate and zone classification import
//! 
//! ### Reactive State Management
//! - Uses Leptos reactive stores for complex state management
//! - Automatically updates system when world parameters change
//! - Maintains separate signals for different aspects of world data
//! - Provides context for child components
//! 
//! ## State Architecture
//! 
//! The component manages several interconnected reactive values:
//! 
//! ### Core State
//! - `main_world`: Store containing the primary world data
//! - `system`: Store containing the complete generated star system
//! - `main_world_name`: Signal for the world's name
//! - `upp`: Signal for the Universal World Profile string
//! 
//! ### Integration State
//! - `origin_coords`: Optional galactic coordinates for the world
//! - `origin_zone`: Travel zone classification (Green/Amber/Red)
//! 
//! ## Generation Process
//! 
//! The system generation follows this reactive flow:
//! 
//! 1. **Input Changes**: User modifies world name or UWP
//! 2. **World Parsing**: UWP string is validated and parsed
//! 3. **World Generation**: New world object created with parameters
//! 4. **Trade Classification**: Trade classes calculated from world stats
//! 5. **System Generation**: Complete star system built around main world
//! 6. **Display Update**: UI automatically updates to show new system
//! 
//! ## Error Handling
//! 
//! The component includes robust error handling for:
//! - Invalid UWP format strings
//! - Malformed world parameters
//! - System generation failures
//! - Network connectivity issues with Traveller Map
//! 
//! ## Usage Examples
//! 
//! ```rust
//! use leptos::prelude::*;
//! use worldgen::components::system_generator::World;
//! 
//! // Mount the system generator component
//! #[component]
//! fn App() -> impl IntoView {
//!     view! {
//!         <World />
//!     }
//! }
//! ```
//! 
//! ## Component Hierarchy
//! 
//! ```text
//! World (system_generator.rs)
//! ├── WorldSearch (traveller_map.rs) - World input and search
//! └── SystemView (system_view.rs) - System display and visualization
//! ```
//! 
//! ## Default Values
//! 
//! The component initializes with sensible defaults:
//! - **Name**: "Main World" (from `INITIAL_NAME` constant)
//! - **UWP**: "A788899-A" (from `INITIAL_UPP` constant)
//! - **Zone**: Green (safe for travel)
//! - **Coordinates**: None (custom world)
//! 
//! ## Printing Support
//! 
//! The component includes print functionality for generating hard copies
//! of system data, though this feature is currently disabled but available
//! for future enhancement.

#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

#[allow(unused_imports)]
use log::debug;

use leptos::prelude::*;
use reactive_stores::Store;

use crate::components::system_view::SystemView;
use crate::components::traveller_map::WorldSearch;
use crate::systems::system::System;
use crate::systems::world::World;
use crate::trade::ZoneClassification;

use crate::INITIAL_NAME;
use crate::INITIAL_UPP;

/// Print the current page (currently unused but available for future use)
/// 
/// Provides a wrapper around the browser's print functionality for generating
/// hard copies of system data. Currently disabled but maintained for potential
/// future print features.
/// 
/// ## Error Handling
/// 
/// Logs errors to the console if printing fails, but does not propagate
/// errors to avoid disrupting the main application flow.
#[allow(dead_code)]
fn print() {
    leptos::leptos_dom::helpers::window()
        .print()
        .unwrap_or_else(|e| log::error!("Error printing: {e:?}"));
}

/// Main system generator component
/// 
/// Provides the complete user interface for generating Traveller star systems
/// from world parameters. Combines input forms, system generation logic, and
/// display components into a cohesive interface.
/// 
/// ## Component Structure
/// 
/// The component creates and manages several reactive stores and signals:
/// - Global context stores for world and system data
/// - Local signals for user input (name, UWP, coordinates, zone)
/// - Reactive effects that trigger system regeneration
/// 
/// ## Reactive Behavior
/// 
/// When the user modifies the world name or UWP string, the component:
/// 1. Validates the new UWP format
/// 2. Creates a new world object with updated parameters
/// 3. Applies any coordinate and zone data from Traveller Map
/// 4. Generates trade classifications based on world statistics
/// 5. Creates a complete star system around the main world
/// 6. Updates the display automatically through reactive stores
/// 
/// ## Error Recovery
/// 
/// If UWP parsing fails, the component logs the error and continues
/// with the previous valid world data, preventing application crashes
/// from malformed input.
/// 
/// ## Integration Points
/// 
/// - **WorldSearch**: Handles world input and Traveller Map integration
/// - **SystemView**: Displays the generated system data and visualizations
/// - **Global Stores**: Provides world and system data to child components
/// 
/// ## Returns
/// 
/// A Leptos view containing:
/// - Application header with title
/// - World search and input form (hidden during printing)
/// - Complete system visualization and data display
#[component]
pub fn World() -> impl IntoView {
    // Initialize global context stores with default world and empty system
    provide_context(Store::new(
        World::from_upp(INITIAL_NAME.to_string(), INITIAL_UPP, false, true).unwrap(),
    ));
    provide_context(Store::new(System::default()));

    // Get references to the context stores for reactive updates
    let system = expect_context::<Store<System>>();
    let main_world = expect_context::<Store<World>>();

    // Create reactive signals for user input
    // Separate name signal to avoid circular dependencies in effects
    let main_world_name = RwSignal::new(main_world.read_untracked().name.clone());
    let origin_coords = RwSignal::new(None::<(i32, i32)>);
    let origin_zone = RwSignal::new(ZoneClassification::Green);
    let upp = RwSignal::new(INITIAL_UPP.to_string());

    // Main reactive effect: regenerate system when world parameters change
    Effect::new(move |_| {
        let upp = upp.get();
        let name = main_world_name.get();
        debug!("Building world {name} with UPP {upp}");
        
        // Attempt to parse the UWP string into a world object
        let Ok(mut w) = World::from_upp(name, upp.as_str(), false, true) else {
            // If parsing fails, log error and bail out to prevent crashes
            log::error!("Failed to parse UWP in hook to build main world: {upp}");
            return;
        };
        
        // Apply additional world data from Traveller Map integration
        w.coordinates = origin_coords.get();
        w.gen_trade_classes();
        
        // Update the global stores, triggering UI updates
        main_world.set(w);
        system.set(System::generate_system(main_world.get()));
    });

    view! {
        <div class:App>
        <h1 class="d-print-none">Solar System Generator</h1>
        <div class="d-print-none key-region world-entry-form">
            <WorldSearch label="Main World".to_string() name=main_world_name uwp=upp coords=origin_coords zone=origin_zone />
        </div>
        <SystemView />
        </div>
    }
}
