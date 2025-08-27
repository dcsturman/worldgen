//! # Worldgen Main Application Entry Point
//!
//! This is the main entry point for the Worldgen web application.
//! It sets up routing based on URL paths and renders the appropriate components.

use web_sys::js_sys::{Function, Object, Reflect};

use leptos::prelude::*;
use worldgen::components::selector::Selector;
use worldgen::components::system_generator::World;
use worldgen::components::trade_computer::Trade;
use worldgen::logging;

const GA_MEASUREMENT_ID: &str = "G-L26P5SCYR2";
/// Track page view for analytics
fn track_page_view(_path: &str) {
    if let Some(window) = web_sys::window() {
        if let Ok(gtag) = Reflect::get(&window, &"gtag".into()) {
            let _ = Function::from(gtag).call3(
                &window,
                &"config".into(),
                &GA_MEASUREMENT_ID.into(),
                &Object::new(),
            );
        }
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
        view! { <Trade /> }.into_any()
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
