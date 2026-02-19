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
/// In production, this will be proxied through nginx to the backend server.
fn get_ws_url() -> String {
    if let Some(window) = web_sys::window()
        && let Ok(location) = window.location().host() {
            let protocol = if window.location().protocol().unwrap_or_default() == "https:" {
                "wss"
            } else {
                "ws"
            };
            return format!("{}://{}/ws/trade", protocol, location);
        }
    // Fallback for local development
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
