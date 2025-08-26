//! # Trade Computer Application Entry Point
//!
//! This is a standalone entry point for the trade computer component,
//! allowing it to be built and deployed separately from the main application.
//! Its not used by the main worldgen application, but could be used to deploy
//! the trade computer as a separate application.

use leptos::prelude::*;
use worldgen::components::trade_computer::Trade;
use worldgen::logging;

/// Trade application entry point
///
/// Sets up panic hooks, initializes logging from URL parameters,
/// and mounts the Trade component directly to the document body.
fn main() {
    // get reasonable errors in the Javascript console from Leptos
    console_error_panic_hook::set_once();
    // Check for parameters like debug parameters in the URL.
    logging::init_from_url();
    // Mount the app to the body (run the App)
    mount_to_body(Trade);
}
