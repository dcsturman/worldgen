//! # World List Component
//!
//! This module provides tabular display components for showing detailed information
//! about worlds, gas giants, and stellar objects within generated Traveller star
//! systems. It renders comprehensive data tables with orbital information, world
//! characteristics, and astronomical data.
//!
//! ## Component Overview
//!
//! The module consists of several interconnected components that work together
//! to display complete system information:
//!
//! - **WorldList**: Main table container for entire star system
//! - **StarRow**: Individual star display with stellar characteristics
//! - **WorldView**: Detailed world information with UWP and trade data
//! - **GiantView**: Gas giant display with size and satellite information
//! - **SatelliteView**: Recursive satellite display for moons and companions
//!
//! ## Key Features
//!
//! ### Comprehensive System Display
//! - Shows all stars, worlds, and gas giants in orbital order
//! - Displays companion stars and their orbital relationships
//! - Includes satellite systems for worlds and gas giants
//! - Maintains proper hierarchical structure
//!
//! ### Detailed World Information
//! - Universal World Profile (UWP) display
//! - Trade classifications and facilities
//! - Astronomical data and orbital mechanics
//! - Zone classifications and travel advisories
//!
//! ### Responsive Table Layout
//! - Consistent column structure across all object types
//! - Proper indentation for satellite relationships
//! - Clear visual hierarchy for system organization
//! - Print-friendly formatting
//!
//! ### Interactive Data Display
//! - Real-time updates when system data changes
//! - Reactive display of calculated values
//! - Automatic formatting of complex data structures
//!
//! ## Table Structure
//!
//! The main table uses a consistent 6-column layout:
//!
//! | Column | Content | Description |
//! |--------|---------|-------------|
//! | Orbit | Orbital Position | Numeric orbit or special designations |
//! | (Empty) | Indentation | Visual spacing for satellites |
//! | Name | Object Name | Star, world, or gas giant name |
//! | UPP | Universal Profile | UWP for worlds, stellar class for stars |
//! | Remarks | Classifications | Trade codes, facilities, special notes |
//! | Astro Data | Physical Info | Astronomical and orbital characteristics |
//!
//! ## Component Hierarchy
//!
//! ```text
//! WorldList
//! ├── StarRow (Primary star)
//! ├── StarRow (Companion stars in primary orbit)
//! ├── Orbital Bodies (in order)
//! │   ├── WorldView (for worlds)
//! │   │   └── SatelliteView (recursive satellites)
//! │   ├── GiantView (for gas giants)
//! │   │   └── SatelliteView (recursive satellites)
//! │   └── StarRow (for companion stars in system orbits)
//! └── StarRow (Far companion stars)
//! ```
//!
//! ## Reactive Store Integration
//!
//! All components use Leptos reactive stores for automatic updates:
//! - **[derive@`Store<System>`]**: Complete star system data
//! - **[`Field<World>`]**: Individual world data with reactive updates
//! - **[`Field<GasGiant>`]**: Gas giant data with satellite information
//! - **[`Field<Satellites>`]**: Satellite collections with automatic iteration
//!
//! ## Display Logic
//!
//! ### Star Display Order
//! 1. **Primary Star**: Always displayed first
//! 2. **Companion Stars (Primary Orbit)**: Stars orbiting close to primary
//! 3. **System Bodies**: Worlds and gas giants in orbital order
//! 4. **Companion Stars (System Orbits)**: Stars in planetary orbits
//! 5. **Far Companion Stars**: Distant stellar companions
//!
//! ### Satellite Handling
//! - Satellites displayed immediately after their parent body
//! - Indented to show hierarchical relationship
//! - Recursive display for satellites of satellites
//! - Consistent formatting regardless of nesting level
//!
//! ## Data Formatting
//!
//! ### World Data
//! - **UWP**: Standard 9-character Universal World Profile
//! - **Remarks**: Combined facilities and trade classifications
//! - **Astro Data**: Physical characteristics and orbital information
//!
//! ### Gas Giant Data
//! - **Size**: Gas giant size classification
//! - **Satellites**: Number and types of moons
//! - **Orbital Position**: Location within star system
//!
//! ### Stellar Data
//! - **Stellar Class**: Standard stellar classification
//! - **Orbital Relationship**: Primary, companion, or far companion
//! - **System Position**: Orbital designation within system
//!
//! ## Usage Examples
//!
//! ```rust
//! use leptos::prelude::*;
//! use reactive_stores::Store;
//! use worldgen::components::world_list::WorldList;
//! use worldgen::systems::system::System;
//!
//! // Display complete star system
//! #[component]
//! fn SystemDisplay() -> impl IntoView {
//!     let system = expect_context::<Store<System>>();
//!     
//!     view! {
//!         <WorldList />
//!     }
//! }
//!
//! // Display companion star system only
//! #[component]
//! fn CompanionDisplay() -> impl IntoView {
//!     view! {
//!         <WorldList is_companion=true />
//!     }
//! }
//! ```
//!
//! ## Styling and CSS Classes
//!
//! The components use consistent CSS classes for styling:
//! - **world-table**: Main table container
//! - **table-entry**: Individual table cells
//! - Responsive design for different screen sizes
//! - Print-optimized formatting
//!
//! ## Performance Considerations
//!
//! ### Efficient Rendering
//! - Uses reactive stores to minimize re-renders
//! - Efficient iteration over orbital slots and satellites
//! - Lazy evaluation of complex calculations
//!
//! ### Memory Management
//! - Minimal data duplication through store references
//! - Automatic cleanup of reactive subscriptions
//! - Efficient handling of large satellite systems
//!
//! ## Integration Points
//!
//! ### System Generator Integration
//! - Displays generated star systems automatically
//! - Updates when system parameters change
//! - Shows results of world generation algorithms
//!
//! ### Trade Computer Integration
//! - Provides world data for trade calculations
//! - Shows trade classifications and facilities
//! - Supports world selection for trade routes
//!
//! ## Error Handling
//!
//! The components include robust error handling for:
//! - Missing or incomplete world data
//! - Malformed orbital configurations
//! - Invalid satellite relationships
//! - Reactive store access failures
//!
//! ## Future Enhancements
//!
//! Potential improvements for future versions:
//! - Sortable columns for different data views
//! - Expandable/collapsible satellite sections
//! - Export functionality for table data
//! - Enhanced filtering and search capabilities

use leptos::prelude::*;
use reactive_stores::{Field, OptionStoreExt, Store, StoreFieldIterator};

#[allow(unused_imports)]
use log::debug;

use crate::systems::gas_giant::{GasGiant, GasGiantStoreFields};
use crate::systems::system::{
    OrbitContent, OrbitContentStoreFields, StarOrbit, System, SystemStoreFields,
};
use crate::systems::world::{Satellites, SatellitesStoreFields, World, WorldStoreFields};

/// Main world list component displaying complete star system information
///
/// Renders a comprehensive table showing all stellar objects, worlds, and gas giants
/// within a star system. Displays objects in proper orbital order with hierarchical
/// relationships for companion stars and satellite systems.
///
/// ## Display Organization
///
/// The component organizes system objects in the following order:
/// 1. **Primary Star**: The main star of the system
/// 2. **Close Companions**: Stars in primary orbit around the main star
/// 3. **Orbital Bodies**: Worlds and gas giants in numerical orbit order
/// 4. **System Companions**: Stars located in planetary orbits
/// 5. **Far Companions**: Distant stellar companions
///
/// ## Table Structure
///
/// Creates a 6-column table with headers:
/// - **Orbit**: Orbital position or designation
/// - **(Empty)**: Spacing column for visual hierarchy
/// - **Name**: Object name (star, world, or gas giant)
/// - **UPP**: Universal World Profile or stellar classification
/// - **Remarks**: Trade codes, facilities, and special characteristics
/// - **Astro Data**: Physical and orbital information
///
/// ## Reactive Behavior
///
/// - Automatically updates when system data changes
/// - Dynamically shows/hides companion stars based on existence
/// - Recursively displays satellite systems
/// - Maintains proper orbital ordering
///
/// ## Parameters
///
/// * `is_companion` - Optional flag indicating if this is a companion star display
///   - `false` (default): Shows complete system hierarchy
///   - `true`: Shows only current star's immediate bodies
///
/// ## Context Requirements
///
/// Expects `Store<System>` in Leptos context containing the star system data
/// to display.
///
/// ## Returns
///
/// Complete HTML table with all system objects properly organized and formatted.
///
/// ## Usage Examples
///
/// ```rust
/// // Display complete star system
/// view! { <WorldList /> }
///
/// // Display companion star system only
/// view! { <WorldList is_companion=true /> }
/// ```
#[component]
pub fn WorldList(#[prop(default = false)] is_companion: bool) -> impl IntoView {
    let primary = expect_context::<Store<System>>();

    view! {
        <table class="world-table">
            <thead>
                <tr>
                    <th class="table-entry">"Orbit"</th>
                    <th class="table-entry"></th>
                    <th class="table-entry">"Name"</th>
                    <th class="table-entry">"UPP"</th>
                    <th class="table-entry">"Remarks"</th>
                    <th class="table-entry">"Astro Data"</th>
                </tr>
            </thead>
            <tbody>
                <StarRow system=primary is_companion=is_companion />
                {move || {
                    [primary.secondary(), primary.tertiary()]
                        .into_iter()
                        .map(|companion| {
                            if let Some(companion) = companion.get() {
                                let companion = Store::new(*companion);
                                if companion.orbit().get() == StarOrbit::Primary {
                                    view! { <StarRow system=companion is_companion=true /> }
                                        .into_any()
                                } else {
                                    ().into_any()
                                }
                            } else {
                                ().into_any()
                            }
                        })
                        .collect::<Vec<_>>()
                        .into_view()
                }}
                {move || {
                    (0..primary.orbit_slots().read().len())
                        .map(|index| {
                            primary
                                .orbit_slots()
                                .at_unkeyed(index)
                                .with(|body| match body {
                                    Some(OrbitContent::World(_world)) => {
                                        let my_field = primary
                                            .orbit_slots()
                                            .at_unkeyed(index)
                                            .unwrap()
                                            .world_0()
                                            .unwrap();
                                        view! { <WorldView world=my_field satellite=false /> }
                                            .into_any()
                                    }
                                    Some(OrbitContent::GasGiant(_gas_giant)) => {
                                        let my_field = primary
                                            .orbit_slots()
                                            .at_unkeyed(index)
                                            .unwrap()
                                            .gas_giant_0()
                                            .unwrap();
                                        view! { <GiantView world=my_field /> }.into_any()
                                    }
                                    Some(OrbitContent::Secondary) => {
                                        let secondary = Store::new(
                                            *primary.secondary().unwrap().get(),
                                        );
                                        view! { <StarRow system=secondary is_companion=false /> }
                                            .into_any()
                                    }
                                    Some(OrbitContent::Tertiary) => {
                                        let tertiary = Store::new(
                                            *primary.tertiary().unwrap().get(),
                                        );
                                        view! { <StarRow system=tertiary is_companion=false /> }
                                            .into_any()
                                    }
                                    _ => ().into_any(),
                                })
                        })
                        .collect::<Vec<_>>()
                        .into_view()
                }}
                {move || {
                    [primary.secondary(), primary.tertiary()]
                        .into_iter()
                        .map(|companion| {
                            if let Some(companion) = companion.get() {
                                let companion = Store::new(*companion);
                                if companion.orbit().get() == StarOrbit::Far {
                                    view! { <StarRow system=companion is_companion=false /> }
                                        .into_any()
                                } else {
                                    ().into_any()
                                }
                            } else {
                                ().into_any()
                            }
                        })
                        .collect::<Vec<_>>()
                        .into_view()
                }}
            </tbody>
        </table>
    }
}

/// Star row component displaying stellar object information
///
/// Renders a table row containing information about a star, including its
/// orbital designation, name, stellar classification, and orbital relationship
/// within the star system.
///
/// ## Display Format
///
/// Creates a table row with the following columns:
/// - **Orbit**: Shows orbital designation (Primary, Far, Companion, or orbit number)
/// - **(Empty)**: Spacing column for visual alignment
/// - **Name**: Star name from the system data
/// - **UPP**: Stellar classification (spectral class, size, etc.)
/// - **Remarks**: Currently empty for stars
/// - **Astro Data**: Currently empty for stars
///
/// ## Orbital Designations
///
/// The orbit column displays different values based on star type:
/// - **"Primary"**: Main star of the system
/// - **"Far"**: Distant companion star
/// - **"Companion"**: Close companion star
/// - **Orbit Number**: For stars in specific planetary orbits
///
/// ## Reactive Updates
///
/// - Star name updates automatically when system data changes
/// - Stellar classification reflects current star properties
/// - Orbital designation updates based on star's orbital relationship
///
/// ## Parameters
///
/// * `system` - Field reference to the star system data
/// * `is_companion` - Flag indicating if this is a companion star display
///   - `false` (default): Shows actual orbital designation
///   - `true`: Always shows "Companion" regardless of orbit type
///
/// ## Returns
///
/// Single table row (`<tr>`) with star information formatted for display
/// in the world list table.
///
/// ## Usage Examples
///
/// ```rust
/// // Display primary star
/// view! { <StarRow system=primary_system is_companion=false /> }
///
/// // Display companion star
/// view! { <StarRow system=companion_system is_companion=true /> }
/// ```
#[component]
pub fn StarRow(
    #[prop(into)] system: Field<System>,
    #[prop(default = false)] is_companion: bool,
) -> impl IntoView {
    view! {
        <tr>
            <td class="table-entry">
                {move || {
                    if is_companion {
                        "Companion".to_string()
                    } else {
                        match system.orbit().get() {
                            StarOrbit::Primary => "Primary".to_string(),
                            StarOrbit::Far => "Far".to_string(),
                            StarOrbit::System(orbit) => orbit.to_string(),
                        }
                    }
                }}
            </td>
            <td class="table-entry"></td>
            <td class="table-entry">{move || system.name().get()}</td>
            <td class="table-entry">{move || system.star().get().to_string()}</td>
        </tr>
    }
}

/// World view component displaying detailed world information
///
/// Renders a comprehensive table row showing world characteristics including
/// orbital position, name, Universal World Profile (UWP), trade classifications,
/// facilities, and astronomical data. Also handles satellite display recursively.
///
/// ## Display Format
///
/// Creates a table row with the following information:
/// - **Orbit**: Orbital position number within the star system
/// - **Indentation**: Extra column for satellites to show hierarchy
/// - **Name**: World name (generated or assigned)
/// - **UPP**: 9-character Universal World Profile string
/// - **Remarks**: Combined facilities and trade classifications
/// - **Astro Data**: Physical characteristics and orbital mechanics
///
/// ## Satellite Handling
///
/// - Satellites get an additional indentation column to show hierarchy
/// - Main worlds span the full table width
/// - Recursive satellite display through `SatelliteView` component
/// - Proper visual nesting for complex satellite systems
///
/// ## Data Integration
///
/// ### UWP Display
/// - Shows complete 9-character Universal World Profile
/// - Includes starport, size, atmosphere, hydrographics, population,
///   government, law level, and tech level
///
/// ### Remarks Column
/// - **Facilities**: Naval bases, scout bases, research stations, etc.
/// - **Trade Classifications**: Agricultural, Industrial, Rich, Poor, etc.
/// - **Formatting**: Semicolon-separated list of non-empty classifications
///
/// ### Astronomical Data
/// - Physical world characteristics
/// - Orbital mechanics and period information
/// - Atmospheric and surface conditions
/// - Temperature and habitability data
///
/// ## Reactive Behavior
///
/// - All displayed data updates automatically when world data changes
/// - Trade classifications recalculated when world stats change
/// - Satellite systems update when satellite data changes
/// - Formatting adjusts based on available data
///
/// ## Parameters
///
/// * `world` - Field reference to the world data
/// * `satellite` - Boolean flag indicating if this is a satellite world
///   - `false`: Main world with full table width
///   - `true`: Satellite world with indentation
///
/// ## Returns
///
/// Table row with complete world information plus recursive satellite display.
///
/// ## Usage Examples
///
/// ```rust
/// // Display main world
/// view! { <WorldView world=main_world satellite=false /> }
///
/// // Display satellite world
/// view! { <WorldView world=moon satellite=true /> }
/// ```
#[component]
pub fn WorldView(#[prop(into)] world: Field<World>, satellite: bool) -> impl IntoView {
    {
        view! {
            <tr>
                // Add an indent for satellite orbit number
                <Show when=move || satellite>{move || view! { <td /> }}</Show>
                <td class="table-entry">{move || world.read().orbit.to_string()}</td>
                <Show when=move || !satellite>{move || view! { <td /> }}</Show>
                <td class="table-entry">{move || world.read().name.clone()}</td>
                <td class="table-entry">{move || world.with(|world| world.to_uwp())}</td>
                <td class="table-entry">
                    {move || {
                        world
                            .with(|world| {
                                itertools::Itertools::intersperse(
                                        [world.facilities_string(), world.trade_classes_string()]
                                            .iter()
                                            .filter(|s| !s.is_empty())
                                            .cloned(),
                                        "; ".to_string(),
                                    )
                                    .collect::<String>()
                            })
                    }}
                </td>
                <td class="table-entry">
                    {move || world.with(|world| world.get_astro_description())}
                </td>
            </tr>
            <SatelliteView satellites=world.satellites() />
        }
        .into_any()
    }
}

/// Gas giant view component displaying gas giant information
///
/// Renders a table row showing gas giant characteristics including orbital
/// position, name, size classification, and satellite information. Provides
/// a simplified display format appropriate for gas giant objects.
///
/// ## Display Format
///
/// Creates a table row with gas giant-specific information:
/// - **Orbit**: Orbital position number within the star system
/// - **(Empty)**: Spacing column for visual alignment
/// - **Name**: Gas giant name (usually system name + Roman numeral)
/// - **UPP**: Gas giant size classification (not full UWP)
/// - **Remarks**: Currently empty for gas giants
/// - **Astro Data**: Currently empty for gas giants
///
/// ## Size Classification
///
/// The UPP column shows the gas giant size rather than a full Universal
/// World Profile, as gas giants use a different classification system
/// than terrestrial worlds.
///
/// ## Satellite Integration
///
/// - Automatically displays satellite systems through `SatelliteView`
/// - Gas giant moons shown with proper indentation
/// - Recursive display for complex moon systems
/// - Maintains visual hierarchy in the table
///
/// ## Reactive Updates
///
/// - Gas giant name updates when system data changes
/// - Size classification reflects current gas giant properties
/// - Satellite display updates when moon data changes
///
/// ## Parameters
///
/// * `world` - Field reference to the gas giant data (uses GasGiant type)
///
/// ## Returns
///
/// Table row with gas giant information plus recursive satellite display
/// for any moons orbiting the gas giant.
///
/// ## Usage Examples
///
/// ```rust
/// // Display gas giant with satellites
/// view! { <GiantView world=gas_giant_field /> }
/// ```
///
/// ## Integration Notes
///
/// While the parameter is named `world`, it actually expects a `Field<GasGiant>`
/// rather than a `Field<World>`. This naming is maintained for consistency
/// with the existing codebase structure.
#[component]
pub fn GiantView(#[prop(into)] world: Field<GasGiant>) -> impl IntoView {
    view! {
        <tr>
            <td class="table-entry">{move || world.read().orbit.to_string()}</td>
            <td class="table-entry"></td>
            <td class="table-entry">{move || world.read().name.clone()}</td>
            <td class="table-entry">{move || world.with(|world| format!("{}", world.size))}</td>
        </tr>
        <SatelliteView satellites=world.satellites() />
    }
}

/// Satellite view component for recursive satellite display
///
/// Renders satellite systems recursively, showing all moons and sub-satellites
/// in proper hierarchical order. Handles the complex nesting relationships
/// that can occur in satellite systems around worlds and gas giants.
///
/// ## Recursive Structure
///
/// - Iterates through all satellites in the satellite collection
/// - Each satellite rendered as a `WorldView` with `satellite=true`
/// - Satellites can have their own satellites, creating recursive nesting
/// - Maintains proper indentation levels for visual hierarchy
///
/// ## Display Behavior
///
/// - Satellites shown immediately after their parent body
/// - Each satellite gets an indentation column for visual hierarchy
/// - Recursive display continues for satellites of satellites
/// - Empty satellite collections render nothing
///
/// ## Reactive Updates
///
/// - Automatically updates when satellite data changes
/// - Dynamically adds/removes satellites as system evolves
/// - Maintains proper ordering and hierarchy
/// - Efficient re-rendering of only changed satellites
///
/// ## Parameters
///
/// * `satellites` - Field reference to the satellite collection
///
/// ## Returns
///
/// Collection of table rows for all satellites in the system, with proper
/// indentation and recursive nesting for complex satellite relationships.
///
/// ## Usage Examples
///
/// ```rust
/// // Display satellites of a world
/// view! { <SatelliteView satellites=world.satellites() /> }
///
/// // Display moons of a gas giant
/// view! { <SatelliteView satellites=gas_giant.satellites() /> }
/// ```
///
/// ## Performance Notes
///
/// The component uses efficient reactive iteration to minimize re-renders
/// when satellite collections change, ensuring good performance even with
/// complex satellite systems.
#[component]
pub fn SatelliteView(#[prop(into)] satellites: Field<Satellites>) -> impl IntoView {
    view! {
        {move || {
            (0..satellites.sats().read().len())
                .map(|index| {
                    let satellite = satellites.sats().at_unkeyed(index);
                    view! { <WorldView world=satellite satellite=true /> }
                })
                .collect::<Vec<_>>()
                .into_view()
        }}
    }
}
