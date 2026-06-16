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
/// GET `url` and return the decoded JSON value, logging a categorized error
/// (to the browser console via `log`) on any failure so problems with the
/// configured TravellerMap service are diagnosable. Distinguishes:
/// - couldn't reach the server at all (network / CORS / DNS / wrong host),
/// - reached it but got a non-2xx HTTP status (404, 500, …),
/// - got a body that isn't valid JSON (e.g. an HTML error page).
///
/// Every message includes the full request URL so a misconfigured
/// `TRAVELLERMAP_URL` is obvious in the console.
async fn fetch_json(url: &str) -> Result<JsValue, JsValue> {
    let request = web_sys::Request::new_with_str(url)?;
    let window = web_sys::window().unwrap();
    let response_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| {
            log::error!(
                "TravellerMap request to {url} could not reach the server \
                 (network/CORS/DNS — is the TravellerMap URL correct and CORS-enabled?): {e:?}"
            );
            e
        })?;
    let response: web_sys::Response = response_value.dyn_into()?;
    if !response.ok() {
        let msg = format!(
            "TravellerMap request to {url} returned HTTP {} {}",
            response.status(),
            response.status_text(),
        );
        log::error!("{msg}");
        return Err(JsValue::from_str(&msg));
    }
    JsFuture::from(response.json()?).await.map_err(|e| {
        log::error!(
            "TravellerMap response from {url} was not valid JSON \
             (is this a TravellerMap-compatible endpoint?): {e:?}"
        );
        e
    })
}

pub async fn fetch_search_results(url: &str) -> Result<TravellerMapResponse, JsValue> {
    let json = fetch_json(url).await?;
    serde_wasm_bindgen::from_value(json).map_err(|e| {
        log::error!(
            "TravellerMap search response from {url} didn't match the expected schema: {e}"
        );
        JsValue::from(e)
    })
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
    let url = format!(
        "{}/data/{}/{}",
        crate::util::travellermap_base_url(),
        encoded_sector,
        hex
    );

    let json = fetch_json(&url).await?;
    let api_response: WorldDataApiResponse = serde_wasm_bindgen::from_value(json).map_err(|e| {
        log::error!("TravellerMap world data from {url} didn't match the expected schema: {e}");
        JsValue::from(e)
    })?;

    // Take the first world from the array
    api_response.worlds.into_iter().next().ok_or_else(|| {
        log::warn!("TravellerMap returned no world for {url} (empty 'worlds' array)");
        JsValue::from_str("No worlds found in response")
    })
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
    /// Optional: when present, the sector name of the selected world is
    /// written here on selection (e.g., "Spinward Marches"). Cleared when
    /// the name is cleared. The trade tool ignores this; the simulator
    /// uses it to query TravellerMap for in-jump-range candidates.
    #[prop(optional)]
    sector: Option<RwSignal<String>>,
    /// Whether to render the editable UWP input. When `false`, the UWP is
    /// still set via autocomplete (so the parent's `uwp` signal is filled),
    /// but the user can't type into it. Useful when callers display the
    /// UWP in a read-only summary elsewhere.
    #[prop(default = true)]
    show_uwp: bool,
    /// Optional: when present, the world's "Stellar" string from
    /// Traveller Map (e.g. "G2 V K2 V") is written here on selection.
    /// Used by the system generator to autopopulate Star rows.
    #[prop(optional)]
    stellar: Option<RwSignal<Option<String>>>,
    /// Optional: when present, the world's PBG string (3 chars,
    /// population/belts/gas-giants) is written here on selection. Used
    /// to autopopulate Belt and GasGiant rows.
    #[prop(optional)]
    pbg: Option<RwSignal<Option<String>>>,
) -> impl IntoView {
    let (search_results, set_search_results) =
        signal::<Vec<(String, String, String, i32, i32)>>(vec![]);
    let (is_loading, set_is_loading) = signal(false);
    // The query string that `search_results` corresponds to. Updated
    // when the autocomplete fetch completes. `handle_selection` only
    // trusts the cached `search_results` when this matches the current
    // input — otherwise the cache is stale (typing faster than the
    // network, or a prior fetch resolved last) and a fresh lookup is
    // fired instead.
    let (last_resolved_query, set_last_resolved_query) = signal::<String>(String::new());

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
        name.set(new_name.clone());
        // Clear coords, zone, and sector if name is empty
        if new_name.is_empty() {
            coords.set(None);
            zone.set(ZoneClassification::Green);
            if let Some(s) = sector {
                s.set(String::new());
            }
        }
    };

    // Commit the input UWP to the external signal
    let commit_uwp = move |new_uwp: String| {
        uwp.set(new_uwp.clone());
        // Clear coords and zone if UWP is empty
        if new_uwp.is_empty() {
            coords.set(None);
            zone.set(ZoneClassification::Green);
        }
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
        // Auto-commit when UWP is complete (9 characters) or empty
        if new_uwp.len() == 9 || new_uwp.is_empty() {
            commit_uwp(new_uwp);
        }
    };

    // Given a picked world from a search response, write everything
    // downstream: name, sector, coords, then asynchronously fetch the
    // detailed world data (zone, stellar, pbg, full UWP) and write
    // those too. Used by both the cached path and the fresh-fetch
    // fallback in `handle_selection`.
    let commit_world = move |search_name: String,
                             result_sector: String,
                             world_uwp: String,
                             hex_x: i32,
                             hex_y: i32| {
        commit_name(search_name);
        if let Some(s) = sector {
            s.set(result_sector.clone());
        }
        coords.set(Some((hex_x, hex_y)));
        let hex_string = format!("{:02}{:02}", hex_x, hex_y);
        let sector_for_fetch = result_sector;
        wasm_bindgen_futures::spawn_local(async move {
            match fetch_data_world(&sector_for_fetch, &hex_string).await {
                Ok(world_data) => {
                    let world_zone = match world_data.zone {
                        Some(z) => match z.as_str() {
                            "A" => ZoneClassification::Amber,
                            "R" => ZoneClassification::Red,
                            _ => ZoneClassification::Green,
                        },
                        None => ZoneClassification::Green,
                    };
                    zone.set(world_zone);
                    if let Some(s) = stellar {
                        s.set(world_data.stellar.clone());
                    }
                    if let Some(p) = pbg {
                        p.set(world_data.pbg.clone());
                    }
                    commit_uwp(world_data.uwp);
                }
                Err(err) => {
                    log::error!("Error fetching world data: {err:?}");
                    commit_uwp(world_uwp);
                }
            }
        });
    };

    // Clear the downstream state (coords/zone/sector) when the typed
    // input maps to no known world. Leaves the raw text in the name
    // field so the user can see what they entered.
    let clear_downstream = move || {
        coords.set(None);
        if let Some(s) = sector {
            s.set(String::new());
        }
    };

    // Pick the best entry from a result list given a typed bare name:
    // exact (case-insensitive) match preferred, else first hit.
    fn pick_from(
        results: &[(String, String, String, i32, i32)],
        typed: &str,
    ) -> Option<(String, String, String, i32, i32)> {
        results
            .iter()
            .find(|(n, ..)| n.eq_ignore_ascii_case(typed))
            .cloned()
            .or_else(|| results.first().cloned())
    }

    // The bare name of the world most recently committed by `do_commit`.
    // Used to detect & suppress the duplicate `change` fire that
    // follows a dropdown click: after path 1 picks (and disambiguates
    // via sector) the right world, `commit_name` resets the input to
    // the bare name. The subsequent `change` event would then run
    // `do_commit` again with the bare name and — for ambiguous names
    // like "Noricum" that exist in multiple sectors — would pick the
    // *wrong* entry via `pick_from`'s first-match fallback.
    let last_committed_name: StoredValue<String> = StoredValue::new(String::new());

    // Commit logic. Takes the current input text as a parameter
    // (rather than reading the signal) so it can be called from both
    // `on:change` (blur/Enter) and `on:input` (Firefox doesn't fire
    // `change` for datalist selections — only `input` — so we have
    // to catch the dropdown-click case from there).
    //
    // Logic:
    //   1. Dropdown click → input is "Name (Sector) UWP". Look up the
    //      exact entry in the cached search results (which is what
    //      built the dropdown). Fall through on a cache miss.
    //   2. Bare typed name + cache is aligned with that name
    //      (`last_resolved_query` matches) → pick from the cache.
    //   3. Otherwise → cache is stale or empty for this input. Fire a
    //      fresh lookup against TravellerMap for the exact text and
    //      commit on its result.
    let do_commit = move |current_input: String| {
        if current_input.trim().is_empty() {
            return;
        }

        // Path 1: dropdown's formatted "Name (Sector) UWP" string.
        let parens = current_input.find(" (").and_then(|start| {
            current_input.find(") ").map(|end| {
                (
                    current_input[..start].to_string(),
                    current_input[start + 2..end].to_string(),
                )
            })
        });

        let typed_name = if let Some((world_name, sector_name)) = parens {
            let results = search_results.get();
            if let Some(hit) = results
                .iter()
                .find(|(n, s, ..)| n == &world_name && s == &sector_name)
                .cloned()
            {
                last_committed_name.set_value(hit.0.clone());
                commit_world(hit.0, hit.1, hit.2, hit.3, hit.4);
                return;
            }
            // Dropdown formatted string but the cache rotated out from
            // under us — strip to the bare name and continue.
            world_name
        } else {
            current_input.trim().to_string()
        };

        // Bare-name dedup: suppress the duplicate `change`-event
        // commit that follows a dropdown click. Once path 1 (or any
        // prior commit) has set `last_committed_name`, the very next
        // bare-name commit that matches it is treated as a re-fire of
        // the same user action and skipped. Any subsequent edit clears
        // the guard implicitly — a later path-2/3 commit will set its
        // own `last_committed_name` and the cycle restarts.
        if last_committed_name.get_value() == typed_name {
            return;
        }

        // Path 2: bare name + cache aligned.
        if last_resolved_query.get_untracked() == typed_name
            && let Some(hit) = pick_from(&search_results.get(), &typed_name)
        {
            last_committed_name.set_value(hit.0.clone());
            commit_world(hit.0, hit.1, hit.2, hit.3, hit.4);
            return;
        }

        // Path 3: fresh, definitive lookup. Spawns one extra HTTP call
        // on commit, which is the price of not trusting a possibly
        // stale autocomplete cache.
        let typed_for_fetch = typed_name.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let url = format!(
                "{}/api/search?q={typed_for_fetch}",
                crate::util::travellermap_base_url()
            );
            match fetch_search_results(&url).await {
                Ok(response) => {
                    let results: Vec<(String, String, String, i32, i32)> = response
                        .results
                        .items
                        .into_iter()
                        .filter_map(|item| {
                            item.world
                                .map(|w| (w.name, w.sector, w.uwp, w.hex_x as i32, w.hex_y as i32))
                        })
                        .collect();
                    match pick_from(&results, &typed_for_fetch) {
                        Some(hit) => {
                            last_committed_name.set_value(hit.0.clone());
                            commit_world(hit.0, hit.1, hit.2, hit.3, hit.4);
                        }
                        None => clear_downstream(),
                    }
                }
                Err(e) => {
                    log::error!("On-commit lookup failed: {e:?}");
                    clear_downstream();
                }
            }
        });
    };

    let handle_selection = move |_| do_commit(input_name.get());

    let handle_name_input = move |ev: web_sys::Event| {
        let new_val = event_target_value(&ev);
        // If the field is empty, commit it to clear the destination.
        if new_val.is_empty() {
            commit_name(new_val);
            return;
        }
        // Firefox doesn't fire `change` for datalist selections (only
        // `input`), so a click on a dropdown option never reaches
        // `handle_selection` until the user finally blurs. Detect the
        // dropdown's "Name (Sector) UWP" pattern here and commit
        // immediately so the input doesn't appear stuck on the
        // formatted string. `bind:value` and our handler both fire on
        // the same `input` event and their relative order is
        // unspecified, so we set `input_name` explicitly before
        // committing rather than trusting it's already in sync.
        if new_val.contains(" (") && new_val.contains(") ") {
            input_name.set(new_val.clone());
            do_commit(new_val);
        }
    };

    // Debounced search function - watch the input_name signal for changes
    let search_query = Memo::new(move |_| input_name.get());

    Effect::new(move |_| {
        let query = search_query.get();

        if search_enabled.get() && query.len() >= 2 {
            set_is_loading.set(true);
            let url = format!(
                "{}/api/search?q={query}",
                crate::util::travellermap_base_url()
            );
            // The async block needs to know what query it was fired
            // for so it can stamp `last_resolved_query` correctly.
            let query_for_resolve = query.clone();

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
                            world_results.sort_by_key(|a| a.0.len());
                            world_results.truncate(MAX_SEARCH_RESULTS);
                        }

                        set_search_results.set(world_results);
                        set_last_resolved_query.set(query_for_resolve);
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
            set_last_resolved_query.set(query);
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
                on:input=handle_name_input
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
        <Show when=move || show_uwp>
            <div>
                <label for=uwp_id.clone()>{format!("{label} UWP:")}</label>
                <input type="text" id=uwp_id.clone() bind:value=input_uwp on:input=handle_uwp_input />
            </div>
        </Show>
        <Show when=move || is_loading.get()>
            <span class="loading-indicator">"Loading..."</span>
        </Show>
        </div>
    }
}

// Re-export hex distance functions from util module for backwards compatibility
pub use crate::util::{calculate_hex_distance, offset_to_cube};
