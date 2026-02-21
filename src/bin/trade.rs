//! # Trade Computer Application Entry Point
//!
//! This is a standalone entry point for the trade computer component,
//! allowing it to be built and deployed separately from the main application.
//! Its not used by the main worldgen application, but could be used to deploy
//! the trade computer as a separate application.

use std::rc::Rc;

use leptos::prelude::*;
use log::{error, info};
use worldgen::comms::Client;
use worldgen::components::trade_computer::Trade;
use worldgen::logging;

/// Get the WebSocket URL for trade state synchronization
///
/// Uses the current page's host to construct a WebSocket URL.
/// - With --local-dev: Connects directly to backend on 8081
/// - Without --local-dev: Uses same host (nginx proxies /ws/* to backend)
fn get_ws_url() -> String {
    if let Some(window) = web_sys::window()
        && let Ok(location) = window.location().host()
    {
        let protocol = if window.location().protocol().unwrap_or_default() == "https:" {
            "wss"
        } else {
            "ws"
        };

        // Local development mode: connect directly to backend on 8081
        #[cfg(feature = "local-dev")]
        {
            if location.starts_with("localhost") {
                return "ws://localhost:8081/ws/trade".to_string();
            }
        }

        // Docker/Production: connect to same host (nginx proxies /ws/* to backend)
        return format!("{}://{}/ws/trade", protocol, location);
    }
    // Fallback
    "ws://localhost:8081/ws/trade".to_string()
}

/// Trade application entry point
///
/// Sets up panic hooks, initializes logging from URL parameters,
/// and mounts the Trade component directly to the document body.
fn main() {
    // get reasonable errors in the Javascript console from Leptos
    console_error_panic_hook::set_once();
    // Check for parameters like debug parameters in the URL.
    logging::init_from_url();

    // Create WebSocket client for trade state synchronization
    let ws_url = get_ws_url();
    match Client::new(&ws_url) {
        Ok(c) => {
            info!("WebSocket client created, connecting to {}", ws_url);
            let client = Rc::new(c);
            // Mount the app to the body (run the App)
            mount_to_body(move || view! { <Trade client=client.clone() /> });
        }
        Err(e) => {
            error!("Failed to create WebSocket client: {}", e);
            // Mount without client
            mount_to_body(|| view! { <Trade /> });
        }
    }
}
