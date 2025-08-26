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

/// Selector component that provides a user interface for selecting between different tools
#[component]
pub fn Selector() -> impl IntoView {
    let navigate_to_trade = move |_| {
        let _ = web_sys::window().unwrap().location().set_href("/trade");
    };

    let navigate_to_world = move |_| {
        let _ = web_sys::window().unwrap().location().set_href("/world");
    };

    view! {
        <div class:App>
            <h1>"Callisto Traveller Tools"</h1>
            <div class="selector-container">
                <div class="tool-card">
                    <h2>"Trade Computer"</h2>
                    <p>"Calculate trade goods, passengers, and cargo opportunities between worlds. Input origin and destination worlds to see available goods, prices, and profit margins for your merchant adventures."</p>
                    <button class="blue-button" on:click=navigate_to_trade>
                        "Launch Trade Computer"
                    </button>
                </div>
                <div class="tool-card">
                    <h2>"Solar System Generator"</h2>
                    <p>"Generate complete solar systems with detailed world data, orbital mechanics, and system composition. Perfect for referees creating new systems or players exploring uncharted space."</p>
                    <button class="blue-button" on:click=navigate_to_world>
                        "Launch System Generator"
                    </button>
                </div>
            </div>
        </div>
    }
}
