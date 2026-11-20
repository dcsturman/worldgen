//! # Selector Component
//!
//! This component provides a user interface for selecting between different tools.
//! It displays two options:
//! - Trade Computer: For calculating trade opportunities between worlds
//! - Solar System Generator: For generating complete solar systems
//!
//! ## Usage
//!
//! ```rust
//! use leptos::prelude::*;
//! use worldgen::components::selector::Selector;
//!
//! #[component]
//! fn App() -> impl IntoView {
//!     view! { <Selector /> }
//! }
//! ```
//!
//! ## Navigation
//!
//! The component redirects users to different tools based on URL paths:
//! - `/trade`: Navigate to the trade computer
//! - `/world`: Navigate to the solar system generator
//!
//! ## Styling
//!
//! The component uses Bootstrap CSS for responsive layout and custom CSS for Traveller-specific styling.
//!
//! ## Example
//!
//! ```rust
//! use leptos::prelude::*;
//! use worldgen::components::selector::Selector;
//!
//! #[component]
//! fn App() -> impl IntoView {
//!     view! { <Selector /> }
//! }
//! ```

use leptos::prelude::*;

use crate::util::custom_travellermap_url;

/// Canonical upstream site, linked from the Traveller Map card.
const UPSTREAM_TRAVELLERMAP_URL: &str = "https://travellermap.com";

/// Selector component that provides a user interface for selecting between different tools
#[component]
pub fn Selector() -> impl IntoView {
    // Only surface the Traveller Map card when this build points at our own
    // self-hosted instance (a custom TRAVELLERMAP_URL). On a stock build that
    // targets the upstream https://travellermap.com there's nothing of ours to
    // launch, so the card is absent.
    let traveller_map_card = custom_travellermap_url().map(|url| {
        let navigate_to_traveller_map = move |_| {
            let _ = web_sys::window().unwrap().location().set_href(url);
        };
        view! {
            <div class="tool-card">
                <h2>"Traveller Map"</h2>
                <p>
                    "This is an independent, client-side reimplementation of "
                    <a href=UPSTREAM_TRAVELLERMAP_URL target="_blank" rel="noopener noreferrer">
                        "The Traveller Map"
                    </a>
                    ", written with client-side rendering and Rust + WebAssembly to be much faster."
                </p>
                <button class="blue-button" on:click=navigate_to_traveller_map>
                    "Launch Traveller Map"
                </button>
            </div>
        }
    });

    view! {
        <div class:App>
            <h1>"Callisto Traveller Tools"</h1>
            <div class="selector-container">
                <div class="tool-card">
                    <h2>"Trade Computer"</h2>
                    <p>"Calculate trade goods, passengers, and cargo opportunities between worlds. Input origin and destination worlds to see available goods, prices, and profit margins for your merchant adventures."</p>
                    <a class="blue-button" href="/trade">
                        "Launch Trade Computer"
                    </a>
                </div>
                <div class="tool-card">
                    <h2>"Ship Simulator"</h2>
                    <p>"Simulate a trader plying multiple worlds for profit. Pick a home port, configure your ship and crew, and watch a greedy route planner make trade decisions across the stars."</p>
                    <a class="blue-button" href="/simulator">
                        "Launch Ship Simulator"
                    </a>
                </div>
                <div class="tool-card">
                    <h2>"Pirate Simulator"</h2>
                    <p>"Run a corsair instead of a merchant — pick a hideout, set your doctrine, raid prey, fence your loot, and build (or outrun) a reputation."</p>
                    <a class="blue-button" href="/pirate-simulator">
                        "Launch Pirate Simulator"
                    </a>
                </div>
                <div class="tool-card">
                    <h2>"Solar System Generator"</h2>
                    <p>"Generate complete solar systems with detailed world data, orbital mechanics, and system composition. Perfect for referees creating new systems or players exploring uncharted space."</p>
                    <a class="blue-button" href="/world">
                        "Launch System Generator"
                    </a>
                </div>
                <div class="tool-card">
                    <h2>"World Map Generator"</h2>
                    <p>"Generate an icosahedral hex map of a single world from its UWP. Procedural terrain (continents, biomes, mountains, ice caps, cities) rendered as both an interactive SVG and a PNG you can upload to roll20."</p>
                    <a class="blue-button" href="/worldmap">
                        "Launch World Map"
                    </a>
                </div>
                {traveller_map_card}
            </div>
        </div>
    }
}
