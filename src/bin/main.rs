//! # Worldgen Main Application Entry Point
//!
//! This is the main entry point for the Worldgen web application.
//! It sets up routing based on URL paths and renders the appropriate components.

use std::rc::Rc;

use web_sys::js_sys::{Function, Object, Reflect};

use leptos::prelude::*;
use log::{error, info};
use worldgen::comms::Client;
use worldgen::components::selector::Selector;
use worldgen::components::system_generator::World;
use worldgen::components::trade_computer::Trade;
use worldgen::logging;

const GA_MEASUREMENT_ID: &str = "G-L26P5SCYR2";

/// Get the WebSocket URL for trade state synchronization
///
/// Uses the current page's host to construct a WebSocket URL.
/// In production, this will be proxied through nginx to the backend server.
fn get_ws_url() -> String {
    if let Some(window) = web_sys::window()
        && let Ok(location) = window.location().host()
    {
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

/// Track page view for analytics
fn track_page_view(_path: &str) {
    if let Some(window) = web_sys::window()
        && let Ok(gtag) = Reflect::get(&window, &"gtag".into())
    {
        let _ = Function::from(gtag).call3(
            &window,
            &"config".into(),
            &GA_MEASUREMENT_ID.into(),
            &Object::new(),
        );
    }
}

/// Main application component that handles routing based on URL path
#[component]
fn App() -> impl IntoView {
    let path = web_sys::window()
        .unwrap()
        .location()
        .pathname()
        .unwrap_or_default();

    // Track the page view
    track_page_view(&path);

    if path.contains("world") {
        view! { <World /> }.into_any()
    } else if path.contains("trade") {
        // Create WebSocket client for trade state synchronization
        let ws_url = get_ws_url();
        match Client::new(&ws_url) {
            Ok(c) => {
                info!("WebSocket client created, connecting to {}", ws_url);
                let client = Rc::new(c);
                view! { <Trade client=client /> }.into_any()
            }
            Err(e) => {
                error!("Failed to create WebSocket client: {}", e);
                view! { <Trade /> }.into_any()
            }
        }
    } else {
        view! { <Selector /> }.into_any()
    }
}

/// Application entry point
///
/// Sets up panic hooks, initializes logging from URL parameters,
/// and mounts the main App component to the document body.  App
/// simply provides a selector for the two main applications based on the
/// URL path.  See index.html for the entry point and routing to the appropriate
/// URLs.  This means if you go to the root URL, you will see the selector.  If you
/// go to the /world URL, you will see the world generator.  If you go to
/// the /trade URL, you will see the trade computer.
fn main() {
    console_error_panic_hook::set_once();
    logging::init_from_url();
    mount_to_body(App);
}
