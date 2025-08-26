//! # System View Component
//! 
//! This module provides comprehensive visualization components for displaying generated
//! Traveller star systems. It creates human-readable descriptions of stellar systems,
//! including primary and companion stars, orbital bodies, and system-wide statistics.
//! 
//! ## Component Overview
//! 
//! The system view consists of three main components that work together to present
//! a complete picture of a generated star system:
//! 
//! - **SystemView**: Main container and system header with primary star information
//! - **SystemPreamble**: Statistical summary of system contents and companion stars
//! - **SystemMain**: Detailed orbital listings and recursive companion star display
//! 
//! ## Key Features
//! 
//! ### Reactive Display
//! - Automatically updates when system data changes
//! - Uses Leptos reactive stores for efficient re-rendering
//! - Maintains consistency across all display components
//! 
//! ### Multi-Star System Support
//! - Handles primary, secondary, and tertiary star systems
//! - Recursive display of companion star orbital bodies
//! - Proper orbital relationship descriptions
//! 
//! ### Comprehensive Statistics
//! - Counts and displays gas giants, planetoids, and satellites
//! - Calculates habitable zones for all stars
//! - Provides grammatically correct quantity descriptions
//! 
//! ### Orbital Mechanics Display
//! - Shows orbital positions and relationships
//! - Describes star types and classifications
//! - Indicates habitable zones and their orbital positions
//! 
//! ## Component Hierarchy
//! 
//! ```text
//! SystemView (main container)
//! ├── System Header (name and primary star info)
//! ├── SystemPreamble (statistics and companion star summary)
//! └── SystemMain (detailed orbital listings)
//!     ├── WorldList (primary star orbital bodies)
//!     ├── Secondary Star Section (if present)
//!     │   └── SystemMain (recursive, is_companion=true)
//!     └── Tertiary Star Section (if present)
//!         └── SystemMain (recursive, is_companion=true)
//! ```
//! 
//! ## State Management
//! 
//! Components expect two context stores:
//! - `Store<System>`: Complete system data including all stars and orbital bodies
//! - `Store<World>`: Main world data for system naming and primary world info
//! 
//! ## Display Logic
//! 
//! ### Habitable Zone Calculation
//! Uses stellar luminosity and type to determine habitable orbital ranges,
//! displaying either the specific orbital position or indicating no habitable zone.
//! 
//! ### Quantity Formatting
//! Provides grammatically correct singular/plural forms for system statistics,
//! with special handling for zero quantities (omitted from display).
//! 
//! ### Companion Star Descriptions
//! Generates contextual descriptions based on orbital relationships:
//! - **Primary Contact**: Close binary stars
//! - **Far Orbit**: Distant companion stars
//! - **System Orbit**: Companions at specific orbital distances
//! 
//! ## Usage Examples
//! 
//! ```rust
//! use leptos::prelude::*;
//! use reactive_stores::Store;
//! use worldgen::components::system_view::SystemView;
//! 
//! // Provide required context and render system view
//! #[component]
//! fn App() -> impl IntoView {
//!     provide_context(Store::new(system));
//!     provide_context(Store::new(main_world));
//!     
//!     view! {
//!         <SystemView />
//!     }
//! }
//! ```
//! 
//! ## Styling Classes
//! 
//! - `.output-region`: Main container for system display
//! - Print-friendly formatting for hard copy generation
//! - Bootstrap-compatible responsive layout

use leptos::context::Provider;
use leptos::prelude::*;
use reactive_stores::{Store, Subfield};

use crate::components::world_list::WorldList;
use crate::systems::has_satellites::HasSatellites;
use crate::systems::system::{OrbitContent, StarOrbit, System, SystemStoreFields};
use crate::systems::system_tables::get_habitable;
use crate::systems::world::World;

/// Generate habitable zone description for a star system
/// 
/// Calculates the habitable zone orbital position for a given star and returns
/// a human-readable description. The habitable zone is the orbital range where
/// liquid water can exist on planetary surfaces.
/// 
/// ## Parameters
/// 
/// * `system` - Star system containing the star to analyze
/// 
/// ## Returns
/// 
/// String describing either:
/// - " with a habitable zone at orbit X" (if zone exists within system)
/// - " with no habitable zone" (if zone is outside orbital limits)
/// 
/// ## Examples
/// 
/// ```rust
/// let description = habitable_clause(&system);
/// // Returns: " with a habitable zone at orbit 3"
/// // Or: " with no habitable zone"
/// ```
fn habitable_clause(system: &System) -> String {
    let habitable = get_habitable(&system.star);
    if habitable > -1 && habitable <= system.get_max_orbits() as i32 {
        format!(" with a habitable zone at orbit {habitable}")
    } else {
        " with no habitable zone".to_string()
    }
}

/// Main system view component displaying complete star system information
/// 
/// Renders the primary interface for viewing generated star systems, including
/// the system name, primary star characteristics, and comprehensive system details.
/// Serves as the top-level container for all system visualization components.
/// 
/// ## Display Structure
/// 
/// 1. **System Header**: System name derived from main world
/// 2. **Primary Star Info**: Star type, classification, and habitable zone
/// 3. **System Preamble**: Statistical summary and companion star overview
/// 4. **System Main**: Detailed orbital body listings and companion star details
/// 
/// ## Context Requirements
/// 
/// Expects two reactive stores in Leptos context:
/// - `Store<System>`: Complete system data for all stars and orbital bodies
/// - `Store<World>`: Main world data for system naming and identification
/// 
/// ## Reactive Behavior
/// 
/// All displayed information updates automatically when the underlying system
/// or world data changes, providing real-time feedback during system generation.
/// 
/// ## Returns
/// 
/// Leptos view containing the complete system display with proper HTML structure
/// and CSS classes for styling and print compatibility.
#[component]
pub fn SystemView() -> impl IntoView {
    let primary = expect_context::<Store<System>>();
    let main_world = expect_context::<Store<World>>();
    view! {
        <div class="output-region">
            <h2>"The " {move || main_world.read().name.clone()} " System"</h2>
            "The primary star of the "
            {move || main_world.read().name.clone()}
            " system is "
            <b>{move || primary.name().get()}</b>
            ", a "
            {move || primary.star().get().to_string()}
            " star"
            {move || habitable_clause(&primary.read())}
            ". "
            <SystemPreamble />
            <br />
            <br />
            <SystemMain />
        </div>
    }
}

/// Create a closure for generating companion star descriptions
/// 
/// Returns a closure that generates contextual descriptions for secondary or
/// tertiary stars based on their orbital relationships to the primary star.
/// The closure captures the system subfield and star type for reactive updates.
/// 
/// ## Parameters
/// 
/// * `system` - Reactive subfield containing optional companion star data
/// * `kind` - Star type descriptor ("secondary" or "tertiary")
/// 
/// ## Returns
/// 
/// Closure that returns formatted description string based on star's orbital position:
/// - **Primary Contact**: Close binary configuration
/// - **Far Orbit**: Distant companion with habitable zone info
/// - **System(N)**: Specific orbital position with habitable zone info
/// - **None**: Empty string if no companion star exists
/// 
/// ## Orbital Descriptions
/// 
/// - **Primary**: "It has a {kind} contact star {name}, which is a {type} star."
/// - **Far**: "It has a {kind} star {name} in far orbit, which is a {type} star{habitable}."
/// - **System(N)**: "It has a {kind} star {name} at orbit {N}, which is a {type} star{habitable}."
/// 
/// ## Usage
/// 
/// ```rust
/// let secondary_desc = lead_builder(system.secondary(), "secondary");
/// let description = secondary_desc(); // Call closure to get current description
/// ```
fn lead_builder(
    system: Subfield<Store<System>, System, Option<Box<System>>>,
    kind: &str,
) -> impl '_ + Fn() -> String {
    move || {
        system.with(|subsystem| {
            if let Some(subsystem) = subsystem {
                match subsystem.orbit {
                    StarOrbit::Primary => {
                        format!(
                            " It has a {} contact star {}, which is a {} star.",
                            kind,
                            subsystem.name.clone(),
                            subsystem.star
                        )
                    }

                    StarOrbit::Far => {
                        format!(
                            " It has a {} star {} in far orbit, which is a {} star{}.",
                            kind,
                            subsystem.name.clone(),
                            subsystem.star,
                            habitable_clause(subsystem)
                        )
                    }
                    StarOrbit::System(orbit) => {
                        format!(
                            " It has a {} star {} at orbit {}, which is a {} star{}.",
                            kind,
                            subsystem.name.clone(),
                            orbit,
                            subsystem.star,
                            habitable_clause(subsystem)
                        )
                    }
                }
            } else {
                "".to_string()
            }
        })
    }
}

/// Format quantity with proper singular/plural grammar
/// 
/// Generates grammatically correct quantity descriptions for system statistics,
/// handling zero quantities, singular forms, and plural forms appropriately.
/// 
/// ## Parameters
/// 
/// * `quantity` - Number of items to describe
/// * `singular` - Singular form of the noun (e.g., "star", "satellite")
/// 
/// ## Returns
/// 
/// Formatted string:
/// - `0`: Empty string (quantity omitted from display)
/// - `1`: "1 {singular}" (e.g., "1 star")
/// - `>1`: "{quantity} {singular}s" (e.g., "3 stars")
/// 
/// ## Examples
/// 
/// ```rust
/// assert_eq!(quantity_suffix(0, "star"), "");
/// assert_eq!(quantity_suffix(1, "star"), "1 star");
/// assert_eq!(quantity_suffix(3, "star"), "3 stars");
/// assert_eq!(quantity_suffix(1, "gas giant"), "1 gas giant");
/// assert_eq!(quantity_suffix(2, "gas giant"), "2 gas giants");
/// ```
fn quantity_suffix(quantity: usize, singular: &str) -> String {
    if quantity == 0 {
        "".to_string()
    } else if quantity == 1 {
        format!("1 {singular}")
    } else {
        format!("{quantity} {singular}s")
    }
}

/// System preamble component displaying statistical summary and companion stars
/// 
/// Renders a comprehensive overview of system contents including counts of various
/// orbital bodies and descriptions of companion stars. Provides context for the
/// detailed orbital listings that follow.
/// 
/// ## Display Elements
/// 
/// ### System Statistics
/// - **Additional Stars**: Count of secondary/tertiary stars
/// - **Gas Giants**: Total number across all stars in system
/// - **Planetoids**: Count of planetoid belts and asteroid fields
/// - **Satellites**: Total moons and satellites of all orbital bodies
/// 
/// ### Companion Star Descriptions
/// - **Secondary Star**: Orbital relationship and characteristics
/// - **Tertiary Star**: Orbital relationship and characteristics
/// 
/// ## Reactive Calculations
/// 
/// All counts are calculated reactively from the system store:
/// - Filters orbital slots by content type
/// - Sums satellite counts across all orbital bodies
/// - Updates automatically when system changes
/// 
/// ## Grammar and Formatting
/// 
/// - Uses proper singular/plural forms for all quantities
/// - Omits zero quantities from display
/// - Joins multiple items with commas and proper conjunction
/// - Provides complete sentences with proper punctuation
/// 
/// ## Context Requirements
/// 
/// Expects `Store<System>` and `Store<World>` in Leptos context for accessing
/// system data and main world information.
/// 
/// ## Returns
/// 
/// Leptos view containing the formatted system summary with conditional display
/// based on actual system contents.
#[component]
pub fn SystemPreamble() -> impl IntoView {
    let system = expect_context::<Store<System>>();
    let main_world = expect_context::<Store<World>>();

    let secondary_lead = lead_builder(system.secondary(), "secondary");
    let tertiary_lead = lead_builder(system.tertiary(), "tertiary");

    let num_stars = move || system.read().count_stars() as usize - 1;
    let num_gas_giants = move || {
        system
            .read()
            .orbit_slots
            .iter()
            .filter(|&body| matches!(&body, Some(OrbitContent::GasGiant(_))))
            .count()
    };
    let num_planetoids = move || {
        system.read().orbit_slots.iter().filter(|&body| matches!(&body, Some(OrbitContent::World(world)) if world.name == "Planetoid Belt")).count()
    };
    let num_satellites = move || {
        system
            .read()
            .orbit_slots
            .iter()
            .filter_map(|body| match body {
                Some(OrbitContent::World(world)) => Some(world.get_num_satellites()),
                Some(OrbitContent::GasGiant(gas_giant)) => Some(gas_giant.get_num_satellites()),
                _ => None,
            })
            .sum::<usize>()
    };

    view! {
        <span>
            <span>
                <Show when=move || {
                    num_gas_giants() + num_stars() + num_planetoids() + num_satellites() > 0
                }>
                    {move || {
                        view! {
                            {main_world.read().name.clone()}
                            " has "
                            {move || {
                                itertools::Itertools::intersperse(
                                        [
                                            quantity_suffix(num_stars(), "star"),
                                            {
                                                if num_gas_giants() >= 2 {
                                                    format!("{} gas giants", num_gas_giants())
                                                } else if num_gas_giants() == 1 {
                                                    "1 gas giant".to_string()
                                                } else {
                                                    "".to_string()
                                                }
                                            },
                                            {
                                                if num_planetoids() >= 2 {
                                                    format!("{} planetoids", num_planetoids())
                                                } else if num_planetoids() == 1 {
                                                    "1 planetoid".to_string()
                                                } else {
                                                    "".to_string()
                                                }
                                            },
                                            {
                                                if num_satellites() >= 2 {
                                                    format!("{} satellites", num_satellites())
                                                } else if num_satellites() == 1 {
                                                    "1 satellite".to_string()
                                                } else {
                                                    "".to_string()
                                                }
                                            },
                                        ]
                                            .iter()
                                            .filter(|x| !x.is_empty())
                                            .cloned(),
                                        ", ".to_string(),
                                    )
                                    .collect::<String>()
                            }}
                        }
                    }} "."
                </Show>
            </span>
            {secondary_lead}

            {tertiary_lead}
        </span>
    }
}

/// Main system content component displaying detailed orbital information
/// 
/// Renders comprehensive orbital body listings for a star system, including
/// recursive display of companion star systems. Handles both primary system
/// display and companion star subsystem display through the `is_companion` flag.
/// 
/// ## Component Behavior
/// 
/// ### Primary System Mode (`is_companion = false`)
/// - Displays complete orbital body list for the primary star
/// - Shows secondary star section with recursive SystemMain
/// - Shows tertiary star section with recursive SystemMain
/// - Provides full system hierarchy visualization
/// 
/// ### Companion System Mode (`is_companion = true`)
/// - Displays only the orbital bodies for the current companion star
/// - Used recursively for secondary and tertiary star systems
/// - Maintains proper context isolation for each star's orbital bodies
/// 
/// ## Recursive Structure
/// 
/// The component calls itself recursively to display companion star systems:
/// 1. **Primary Display**: Shows main star's orbital bodies
/// 2. **Secondary Section**: Creates new context and renders secondary star's bodies
/// 3. **Tertiary Section**: Creates new context and renders tertiary star's bodies
/// 
/// ## Context Management
/// 
/// For companion stars, creates isolated reactive contexts:
/// - Wraps companion star data in new `Store<System>`
/// - Provides isolated context to prevent data conflicts
/// - Maintains proper reactive boundaries between star systems
/// 
/// ## Display Format
/// 
/// ```text
/// [WorldList for current star]
/// 
/// [System Name]'s secondary star [Star Name]:
/// [Recursive SystemMain for secondary]
/// 
/// [System Name]'s tertiary star [Star Name]:
/// [Recursive SystemMain for tertiary]
/// ```
/// 
/// ## Parameters
/// 
/// * `is_companion` - Flag indicating if this is a companion star display
///   - `false`: Primary system mode with full hierarchy
///   - `true`: Companion mode showing only current star's bodies
/// 
/// ## Context Requirements
/// 
/// Expects `Store<System>` in Leptos context containing the current star system
/// data to display.
/// 
/// ## Returns
/// 
/// Leptos view containing the orbital body listings and any companion star
/// sections, with proper spacing and hierarchical organization.
#[component]
pub fn SystemMain(#[prop(default = false)] is_companion: bool) -> impl IntoView {
    let system = expect_context::<Store<System>>();

    view! {
        <div>
            <WorldList is_companion=is_companion />
            <br />
            {move || {
                if let Some(secondary) = system.secondary().get() {
                    let secondary = Store::new(*secondary);
                    view! {
                        {system.read().name.clone()}
                        "'s secondary star "
                        {secondary.name().get()}
                        :
                        <br />
                        <Provider value=secondary>
                            <SystemMain is_companion=true />
                        </Provider>
                        <br />
                    }
                        .into_any()
                } else {
                    ().into_any()
                }
            }}
            {move || {
                if let Some(tertiary) = system.tertiary().get() {
                    let tertiary = Store::new(*tertiary);
                    view! {
                        {system.read().name.clone()}
                        "'s tertiary star "
                        {tertiary.name().get()}
                        :
                        <br />
                        <Provider value=tertiary>
                            <SystemMain is_companion=true />
                        </Provider>
                        <br />
                    }
                        .into_any()
                } else {
                    ().into_any()
                }
            }}
        </div>
    }
}
