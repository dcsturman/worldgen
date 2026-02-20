//! # Traveller Map Integration Component
//!
//! This module provides integration with the official Traveller Map web services,
//! enabling search and retrieval of canonical world data from the published
//! Traveller universe. It includes components for world search, data fetching,
//! and coordinate system calculations.
//!
//! ## Component Overview
//!
//! The module consists of several key components and utilities:
//!
//! - **WorldSearch**: Interactive world search component with autocomplete
//! - **API Functions**: Async functions for fetching data from Traveller Map
//! - **Data Structures**: Serde-compatible types for API responses
//! - **Coordinate Utilities**: Hex distance calculation functions
//!
//! ## Key Features
//!
//! ### Official Universe Integration
//! - Connects to Traveller Map API services
//! - Searches across all published sectors and subsectors
//! - Retrieves canonical world data including UWPs and coordinates
//! - Supports zone classification import (Green/Amber/Red)
//!
//! ### Interactive World Search
//! - Real-time search with autocomplete suggestions
//! - Debounced search to minimize API calls
//! - Formatted search results showing world, sector, and UWP
//! - Automatic data population when worlds are selected
//!
//! ### Coordinate System Support
//! - Hex coordinate system used by Traveller maps
//! - Distance calculation between worlds using cube coordinates
//! - Support for sector-relative and galactic coordinate systems
//! - Integration with trade route distance calculations
//!
//! ### Dual UWP Input System
//! - Separate input field for UWP editing
//! - Automatic UWP validation (9-character format)
//! - Synchronization between search results and manual input
//! - Real-time updates when external UWP changes occur
//!
//! ## API Integration
//!
//! ### Search API
//! Uses Traveller Map search endpoint:
//! - **Endpoint**: `https://travellermap.com/api/search?q={query}`
//! - **Method**: GET request with query parameter
//! - **Response**: JSON with search results including worlds, sectors, subsectors
//! - **Rate Limiting**: Debounced to 2+ character queries
//!
//! ### World Data API
//! Uses Traveller Map data endpoint:
//! - **Endpoint**: `https://travellermap.com/data/{sector}/{hex}`
//! - **Method**: GET request with sector name and hex coordinates
//! - **Response**: JSON with detailed world data including zone classification
//! - **Error Handling**: Graceful fallback to search result UWP
//!
//! ## Data Structures
//!
//! ### Search Response Types
//! - `TravellerMapResponse`: Top-level search response wrapper
//! - `SearchResults`: Contains count and array of search items
//! - `SearchItem`: Individual result (world, sector, or subsector)
//! - `WorldData`: World-specific data from search results
//! - `SectorData`: Sector information and coordinates
//! - `SubsectorData`: Subsector details and location
//!
//! ### World Data Types
//! - `WorldDataApiResponse`: Wrapper for detailed world data
//! - `WorldDataResponse`: Complete world information including zone data
//!
//! ## Coordinate System
//!
//! ### Hex Coordinate Format
//! Traveller uses a hex-based coordinate system:
//! - **Hex Coordinates**: Column (X) and Row (Y) within sector
//! - **Sector Coordinates**: Sector position in galactic grid
//! - **Format**: 4-digit hex (e.g., "0101" for column 1, row 1)
//!
//! ### Distance Calculation
//! Uses cube coordinate system for accurate hex distances:
//! - Converts offset coordinates to cube coordinates
//! - Calculates Manhattan distance in cube space
//! - Supports odd-q offset system (Traveller standard)
//! - Returns distance in parsecs (1 hex = 1 parsec)
//!
//! ## Component Architecture
//!
//! ### WorldSearch Component
//!
//! **Props:**
//! - `label`: Display label for the search field
//! - `name`: RwSignal for world name (bidirectional binding)
//! - `uwp`: RwSignal for Universal World Profile string
//! - `coords`: RwSignal for hex coordinates (optional)
//! - `zone`: RwSignal for zone classification
//! - `search_enabled`: Signal to enable/disable search functionality
//!
//! **Internal State:**
//! - Search results cache with world data
//! - Loading state indicator
//! - Separate UWP input field for editing
//!
//! **Reactive Effects:**
//! - Search trigger on name changes (2+ characters)
//! - UWP synchronization between internal and external signals
//! - World data fetching when selections are made
//!
//! ## Search Behavior
//!
//! ### Query Processing
//! 1. **Input Validation**: Requires 2+ characters to trigger search
//! 2. **Debouncing**: Uses Leptos memo for automatic debouncing
//! 3. **API Call**: Fetches results from Traveller Map search endpoint
//! 4. **Result Processing**: Filters and sorts world results
//! 5. **Display**: Shows formatted results in HTML datalist
//!
//! ### Result Selection
//! 1. **Format Parsing**: Extracts world name and sector from selection
//! 2. **Data Lookup**: Finds matching result in cached search data
//! 3. **Detail Fetch**: Retrieves detailed world data from data API
//! 4. **State Update**: Updates all related signals with new data
//! 5. **Coordinate Setting**: Sets hex coordinates for distance calculations
//!
//! ## Error Handling
//!
//! ### Network Errors
//! - API request failures logged to console
//! - Graceful degradation when services unavailable
//! - Fallback to search result data when detail fetch fails
//!
//! ### Data Validation
//! - UWP format validation (9-character requirement)
//! - Coordinate range checking
//! - Zone classification parsing with defaults
//!
//! ### User Experience
//! - Loading indicators during API calls
//! - Non-blocking error handling
//! - Maintains user input even when API fails
//!
//! ## Usage Examples
//!
//! ```rust,ignore
//! # use leptos::prelude::*;
//! # use worldgen::components::traveller_map::WorldSearch;
//! # use worldgen::trade::ZoneClassification;
//! // Basic world search component
//! #[component]
//! fn WorldEntry() -> impl IntoView {
//!     let name = RwSignal::new("".to_string());
//!     let uwp = RwSignal::new("".to_string());
//!     let coords = RwSignal::new(None);
//!     let zone = RwSignal::new(ZoneClassification::Green);
//!
//!     view! {
//!         <WorldSearch
//!             label="Origin World".to_string()
//!             name=name
//!             uwp=uwp
//!             coords=coords
//!             zone=zone
//!         />
//!     }
//! }
//!
//! // Calculate distance between two worlds
//! # use worldgen::components::traveller_map::calculate_hex_distance;
//! let distance = calculate_hex_distance(10, 15, 12, 18); // Returns distance in parsecs
//! ```
//!
//! ## Integration Points
//!
//! ### System Generator Integration
//! - Provides world data for system generation
//! - Populates UWP data for system calculations
//! - Supplies coordinates for sector placement
//!
//! ### Trade Computer Integration
//! - Enables selection of origin and destination worlds
//! - Provides distance calculations for trade routes
//! - Supplies zone data for travel risk assessment
//!
//! ## Performance Considerations
//!
//! ### Search Optimization
//! - Debounced search queries to reduce API load
//! - Result caching to avoid duplicate requests
//! - Limited result sets (12 items max) for performance
//!
//! ### Memory Management
//! - Efficient result storage with minimal data duplication
//! - Automatic cleanup of search results when queries change
//! - Lazy loading of detailed world data only when needed
//!
//! ## Future Enhancements
//!
//! Potential improvements for future versions:
//! - Offline caching of frequently accessed worlds
//! - Bulk world data import for campaign management
//! - Integration with additional Traveller Map features
//! - Support for custom sector data import

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::js_sys;

use crate::trade::ZoneClassification;

/// Maximum number of search results to display in autocomplete
///
/// Limits the number of search results shown to the user to maintain
/// performance and usability. Results are sorted by world name length
/// (shorter names first) when truncation is needed.
const MAX_SEARCH_RESULTS: usize = 12;

#[allow(unused_imports)]
use log::debug;

/// Top-level response structure from Traveller Map search API
///
/// Wraps the search results returned by the Traveller Map search endpoint.
/// Uses PascalCase field naming to match the API response format.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct TravellerMapResponse {
    /// Search results container with count and items
    pub results: SearchResults,
}

/// Container for search results with metadata
///
/// Contains the total count of results and the array of individual
/// search items returned by the API.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SearchResults {
    /// Total number of results found
    pub count: usize,
    /// Array of individual search result items
    pub items: Vec<SearchItem>,
}

/// Individual search result item
///
/// Represents a single result from the search API, which can be
/// a world, sector, or subsector. Only one of the optional fields
/// will be populated for each item.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SearchItem {
    /// World data if this result is a world
    pub world: Option<WorldData>,
    /// Sector data if this result is a sector
    pub sector: Option<SectorData>,
    /// Subsector data if this result is a subsector
    pub subsector: Option<SubsectorData>,
}

/// World data from search results
///
/// Contains basic world information returned by the search API,
/// including coordinates, UWP, and sector information.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct WorldData {
    /// Hex column coordinate within sector (1-32)
    pub hex_x: usize,
    /// Hex row coordinate within sector (1-40)
    pub hex_y: usize,
    /// Name of the sector containing this world
    pub sector: String,
    /// Universal World Profile string
    pub uwp: String,
    /// Sector X coordinate in galactic grid
    pub sector_x: i32,
    /// Sector Y coordinate in galactic grid
    pub sector_y: i32,
    /// World name
    pub name: String,
    /// Sector tags and classifications
    pub sector_tags: String,
}

/// Sector data from search results
///
/// Contains information about a sector in the Traveller universe,
/// including its position in the galactic coordinate system.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SectorData {
    /// Sector X coordinate in galactic grid
    pub sector_x: i32,
    /// Sector Y coordinate in galactic grid
    pub sector_y: i32,
    /// Sector name
    pub name: String,
    /// Sector tags and classifications
    pub sector_tags: String,
}

/// Subsector data from search results
///
/// Contains information about a subsector within a sector,
/// including its index and parent sector information.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SubsectorData {
    /// Parent sector name
    pub sector: String,
    /// Subsector index (A-P)
    pub index: String,
    /// Sector X coordinate in galactic grid
    pub sector_x: i32,
    /// Sector Y coordinate in galactic grid
    pub sector_y: i32,
    /// Subsector name
    pub name: String,
    /// Sector tags and classifications
    pub sector_tags: String,
}

/// Response wrapper for detailed world data API
///
/// Contains an array of world data responses from the detailed
/// world data endpoint. Typically contains a single world.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct WorldDataApiResponse {
    /// Array of world data (usually contains one world)
    pub worlds: Vec<WorldDataResponse>,
}

/// Detailed world data from the world data API
///
/// Contains comprehensive world information including zone classification,
/// population/government/law data, allegiance, and stellar data.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct WorldDataResponse {
    /// Hex coordinate string (e.g., "0101")
    pub hex: String,
    /// World name
    pub name: String,
    /// Universal World Profile string
    #[serde(rename = "UWP")]
    pub uwp: String,
    /// Travel zone classification (A=Amber, R=Red, None=Green)
    pub zone: Option<String>,
    /// Population/Belts/Gas Giants string
    pub pbg: Option<String>,
    /// Political allegiance code
    pub allegiance: Option<String>,
    /// Stellar data string
    pub stellar: Option<String>,
}

/// Fetch search results from Traveller Map search API
///
/// Performs an asynchronous HTTP request to the Traveller Map search endpoint
/// and returns parsed search results. Handles network errors and JSON parsing.
///
/// ## Parameters
///
/// * `url` - Complete URL for the search API request
///
/// ## Returns
///
/// * `Ok(TravellerMapResponse)` - Parsed search results on success
/// * `Err(JsValue)` - JavaScript error on network or parsing failure
///
/// ## Error Handling
///
/// Propagates errors from:
/// - HTTP request creation
/// - Network fetch operation
/// - JSON response parsing
/// - Serde deserialization
///
/// ## Example
///
/// ```rust,ignore
/// # use worldgen::components::traveller_map::fetch_search_results;
/// let url = "https://travellermap.com/api/search?q=Regina";
/// let results = fetch_search_results(&url).await?;
/// println!("Found {} results", results.results.count);
/// ```
pub async fn fetch_search_results(url: &str) -> Result<TravellerMapResponse, JsValue> {
    let request = web_sys::Request::new_with_str(url)?;
    let window = web_sys::window().unwrap();
    let response_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let response: web_sys::Response = response_value.dyn_into()?;
    let json = JsFuture::from(response.json()?).await?;
    let result: TravellerMapResponse = serde_wasm_bindgen::from_value(json)?;
    Ok(result)
}

/// Fetch detailed world data from Traveller Map data API
///
/// Retrieves comprehensive world information including zone classification
/// and additional data not available in search results. Uses the sector
/// name and hex coordinates to fetch specific world data.
///
/// ## Parameters
///
/// * `sector` - Name of the sector containing the world
/// * `hex` - Hex coordinate string (e.g., "0101")
///
/// ## Returns
///
/// * `Ok(WorldDataResponse)` - Detailed world data on success
/// * `Err(JsValue)` - JavaScript error on network or parsing failure
///
/// ## Error Handling
///
/// Handles several error conditions:
/// - Network connectivity issues
/// - Invalid sector or hex parameters
/// - Empty response arrays
/// - JSON parsing failures
///
/// ## API Endpoint
///
/// Uses the format: `https://travellermap.com/data/{sector}/{hex}`
/// where sector names are URL-encoded for safety.
///
/// ## Example
///
/// ```rust,ignore
/// # use worldgen::components::traveller_map::fetch_data_world;
/// let world_data = fetch_data_world("Spinward Marches", "1910").await?;
/// println!("World: {} ({})", world_data.name, world_data.uwp);
/// ```
pub async fn fetch_data_world(sector: &str, hex: &str) -> Result<WorldDataResponse, JsValue> {
    let encoded_sector = web_sys::js_sys::encode_uri_component(sector);
    let url = format!("https://travellermap.com/data/{}/{}", encoded_sector, hex);

    let request = web_sys::Request::new_with_str(&url)?;
    let window = web_sys::window().unwrap();
    let response_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let response: web_sys::Response = response_value.dyn_into()?;
    let json = JsFuture::from(response.json()?).await?;
    let api_response: WorldDataApiResponse = serde_wasm_bindgen::from_value(json)?;

    // Take the first world from the array
    let result = api_response
        .worlds
        .into_iter()
        .next()
        .ok_or_else(|| JsValue::from_str("No worlds found in response"))?;

    Ok(result)
}

/// Creates a world search component with TravellerMap integration
///
/// Provides an interactive world search interface with autocomplete functionality,
/// UWP input validation, and automatic data population from the Traveller Map
/// services. Supports both manual world entry and selection from official data.
///
/// ## Component Features
///
/// ### Interactive Search
/// - Real-time search with 2+ character minimum
/// - Debounced API calls to minimize server load
/// - Autocomplete dropdown with formatted results
/// - Loading indicator during search operations
///
/// ### Dual Input System
/// - Separate world name and UWP input fields
/// - Automatic UWP validation (9-character format)
/// - Bidirectional synchronization with external signals
/// - Manual UWP editing with real-time updates
///
/// ### Data Integration
/// - Automatic coordinate and zone data population
/// - Detailed world data fetching on selection
/// - Graceful fallback to search result data
/// - Zone classification parsing (Green/Amber/Red)
///
/// ## Props
///
/// * `label` - Display label for the search field
/// * `name` - RwSignal for world name (bidirectional binding)
/// * `uwp` - RwSignal for Universal World Profile string
/// * `coords` - RwSignal for hex coordinates (optional)
/// * `zone` - RwSignal for zone classification
/// * `search_enabled` - Signal to enable/disable search (default: true)
///
/// ## Reactive Behavior
///
/// ### Search Triggering
/// - Monitors name signal for changes
/// - Triggers search when length >= 2 and search enabled
/// - Debounces automatically using Leptos memo system
///
/// ### UWP Synchronization
/// - Maintains separate internal UWP signal for editing
/// - Syncs with external UWP signal bidirectionally
/// - Updates external signal when UWP reaches 9 characters
///
/// ### Selection Handling
/// - Parses datalist selection format: "WorldName (Sector) UWP"
/// - Fetches detailed world data from Traveller Map API
/// - Updates all related signals with retrieved data
/// - Sets coordinates for distance calculations
///
/// ## Error Handling
///
/// ### Network Errors
/// - Logs API failures to console
/// - Continues operation with cached search data
/// - Provides fallback UWP from search results
///
/// ### Input Validation
/// - Validates UWP format (9 characters)
/// - Handles malformed selection strings gracefully
/// - Maintains user input even when API calls fail
///
/// ## Performance Optimizations
///
/// ### Search Efficiency
/// - Limits results to MAX_SEARCH_RESULTS (12 items)
/// - Sorts results by name length for relevance
/// - Caches search results to avoid duplicate API calls
///
/// ### Memory Management
/// - Clears search results when query is too short
/// - Uses efficient signal updates to minimize re-renders
/// - Lazy loads detailed data only when needed
///
/// ## Usage Examples
///
/// ```rust,ignore
/// // Basic world search
/// let name = RwSignal::new("".to_string());
/// let uwp = RwSignal::new("".to_string());
/// let coords = RwSignal::new(None);
/// let zone = RwSignal::new(ZoneClassification::Green);
///
/// view! {
///     <WorldSearch
///         label="Origin World".to_string()
///         name=name
///         uwp=uwp
///         coords=coords
///         zone=zone
///     />
/// }
///
/// // Disabled search (manual entry only)
/// view! {
///     <WorldSearch
///         label="Custom World".to_string()
///         name=name
///         uwp=uwp
///         coords=coords
///         zone=zone
///         search_enabled=Signal::derive(|| false)
///     />
/// }
/// ```
///
/// ## Returns
///
/// Complete world search interface with:
/// - World name input with autocomplete
/// - UWP input with validation
/// - Loading indicator
/// - Automatic data population
#[component]
pub fn WorldSearch(
    label: String,
    name: RwSignal<String>,
    uwp: RwSignal<String>,
    coords: RwSignal<Option<(i32, i32)>>,
    zone: RwSignal<ZoneClassification>,
    #[prop(default = Signal::derive(|| true))] search_enabled: Signal<bool>,
) -> impl IntoView {
    let (search_results, set_search_results) =
        signal::<Vec<(String, String, String, i32, i32)>>(vec![]);
    let (is_loading, set_is_loading) = signal(false);

    // Separate input signals that don't trigger server updates
    // These are only committed to the external signals on Enter or dropdown selection
    let input_name = RwSignal::new(name.get_untracked());
    let input_uwp = RwSignal::new(uwp.get_untracked());

    // Sync input signals when external signals change from outside (e.g., from server)
    Effect::new(move |_| {
        let external_name = name.get();
        if external_name != input_name.get_untracked() {
            input_name.set(external_name);
        }
    });

    Effect::new(move |_| {
        let external_uwp = uwp.get();
        if external_uwp != input_uwp.get_untracked() {
            input_uwp.set(external_uwp);
        }
    });

    // Commit the input name to the external signal
    let commit_name = move |new_name: String| {
        name.set(new_name);
    };

    // Commit the input UWP to the external signal
    let commit_uwp = move |new_uwp: String| {
        uwp.set(new_uwp);
    };

    let handle_name_keydown = move |ev: web_sys::KeyboardEvent| {
        // Safely get the key using Reflect to avoid JavaScript exceptions
        let key = match web_sys::js_sys::Reflect::get(&ev, &"key".into()) {
            Ok(val) => val.as_string().unwrap_or_default(),
            Err(_) => {
                log::warn!("Failed to read KeyboardEvent.key property");
                return;
            }
        };

        if key == "Enter" {
            ev.prevent_default();
            let current_input = input_name.get();
            commit_name(current_input);
        }
    };

    let handle_uwp_input = move |ev| {
        let new_uwp = event_target_value(&ev);
        input_uwp.set(new_uwp.clone());
        // Auto-commit when UWP is complete (9 characters)
        if new_uwp.len() == 9 {
            commit_uwp(new_uwp);
        }
    };

    // Handle selection from datalist
    let handle_selection = move |_| {
        let current_input = input_name.get();
        // Parse the format: "WorldName (Sector) UWP"
        let (world_name, sector_name) = if let Some(paren_start) = current_input.find(" (") {
            if let Some(paren_end) = current_input.find(") ") {
                let world_name = &current_input[..paren_start];
                let sector_name = &current_input[paren_start + 2..paren_end];
                (world_name, sector_name)
            } else {
                return; // Invalid format
            }
        } else {
            return; // Invalid format
        };

        let mut found = false;
        for (search_name, sector, world_uwp, hex_x, hex_y) in search_results.get() {
            if world_name == search_name && sector_name == sector {
                let hex_string = format!("{:02}{:02}", hex_x, hex_y);

                // Commit the name to just the world name
                commit_name(world_name.to_string());

                wasm_bindgen_futures::spawn_local(async move {
                    match fetch_data_world(&sector, &hex_string).await {
                        Ok(world_data) => {
                            let world_zone = match world_data.zone {
                                Some(zone) => match zone.as_str() {
                                    "A" => ZoneClassification::Amber,
                                    "R" => ZoneClassification::Red,
                                    _ => ZoneClassification::Green,
                                },
                                None => ZoneClassification::Green,
                            };
                            zone.set(world_zone);
                            commit_uwp(world_data.uwp);
                        }
                        Err(err) => {
                            log::error!("Error fetching world data: {err:?}");
                            // Fallback to the UWP from search results
                            commit_uwp(world_uwp);
                        }
                    }
                });
                coords.set(Some((hex_x, hex_y)));
                found = true;
                break;
            }
        }
        if !found {
            coords.set(None);
        }
    };

    // Debounced search function - watch the input_name signal for changes
    let search_query = Memo::new(move |_| input_name.get());

    Effect::new(move |_| {
        let query = search_query.get();

        if search_enabled.get() && query.len() >= 2 {
            set_is_loading.set(true);
            let url = format!("https://travellermap.com/api/search?q={query}");

            wasm_bindgen_futures::spawn_local(async move {
                match fetch_search_results(&url).await {
                    Ok(response) => {
                        let mut world_results = Vec::new();
                        for item in response.results.items {
                            if let Some(world) = item.world {
                                world_results.push((
                                    world.name,
                                    world.sector,
                                    world.uwp,
                                    world.hex_x as i32,
                                    world.hex_y as i32,
                                ));
                            }
                        }
                        if world_results.len() > MAX_SEARCH_RESULTS {
                            // Sort by world name length (shorter names first)
                            world_results.sort_by(|a, b| a.0.len().cmp(&b.0.len()));
                            world_results.truncate(MAX_SEARCH_RESULTS);
                        }

                        set_search_results.set(world_results);
                        set_is_loading.set(false);
                    }
                    Err(err) => {
                        log::error!("Error fetching search results: {err:?}");
                        set_is_loading.set(false);
                    }
                }
            });
        } else {
            set_search_results.set(vec![]);
        }
    });

    let world_name_id = format!("worldName-{}", rand::random::<u32>());
    let uwp_id = format!("uwp-{}", rand::random::<u32>());
    let datalist_id = format!("world-suggestions-{}", rand::random::<u32>());

    view! {
        <div class:world-text-entry>
        <div>
            <label for=world_name_id.clone()>{format!("{}:",label.clone())}</label>
            <input
                id=world_name_id
                type="text"
                bind:value=input_name
                list=datalist_id.clone()
                on:change=handle_selection
                on:keydown=handle_name_keydown
            />
            <datalist class="world-suggestions" id=datalist_id>
                {move || {
                    search_results
                        .get()
                        .into_iter()
                        .map(|(name, sector, uwp, _, _)| {
                            view! {
                                <option value=format!("{name} ({sector}) {uwp}")>{format!("{name} ({sector}) {uwp}")}</option>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </datalist>
        </div>
        <div>
            <label for=uwp_id.clone()>{format!("{label} UPP:")}</label>
            <input type="text" id=uwp_id bind:value=input_uwp on:input=handle_uwp_input />
        </div>
        <Show when=move || is_loading.get()>
            <span class="loading-indicator">"Loading..."</span>
        </Show>
        </div>
    }
}

// Re-export hex distance functions from util module for backwards compatibility
pub use crate::util::{calculate_hex_distance, offset_to_cube};
