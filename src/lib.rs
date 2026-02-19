//! # Worldgen - A set of Traveller tools
//!
//! Worldgen started as a world generator, but has evolved into now two different (and possibly more)
//! tools.  The first is a web application for generating Traveller solar systems and supporting
//! materials for those systems. It takes a world name and a UWP for the main world
//! and can generate either or both of the following:
//!
//! 1. A full solar system with the main world at the center and up to two companion stars
//! 2. A trade table of available goods for the main world
//!
//! The second tool is a trade computer that can calculate trade goods, passengers, and cargo opportunities between worlds.
//! It not only generates trade tables and available passengers and freight based on source and destination world,
//! but allows all these to be selectable to generate a "manifest" of goods, passengers, and freight for a ship to transport and
//! then determine the profit and loss for the ship on that transit.
//!
//! The entire system is written in Rust using Leptos as a reactive front-end framework.

pub mod backend;
pub mod components;
pub mod logging;
pub mod systems;
pub mod trade;
pub mod util;

#[cfg(feature = "hydrate")]
use leptos::prelude::*;

#[cfg(feature = "hydrate")]
use log::debug;

#[cfg(feature = "hydrate")]
use web_sys::js_sys::{Function, Object, Reflect};

/// Default UWP (Universal World Profile) used for initial world generation
pub const INITIAL_UPP: &str = "A788899-A";

/// Default name for the initial main world
pub const INITIAL_NAME: &str = "Main World";

#[cfg(feature = "hydrate")]
const GA_MEASUREMENT_ID: &str = "G-L26P5SCYR2";
/// Track page view for analytics
#[cfg(feature = "hydrate")]
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
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::components::app::App;
    use leptos_ws::WsSignals;
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    leptos::mount::hydrate_body(move || {
        debug!("WASM: Starting hydration, initializing MetaContext");
        leptos_meta::provide_meta_context();

        // Initialize WsSignals for client-side WebSocket connection
        let ws_signals = WsSignals::new();
        provide_context(ws_signals);
        leptos_ws::provide_websocket();
        debug!("Leptos_ws Websocket manager initialized.");

        debug!("WASM: Inside reactive scope, providing MetaContext");
        // This is the magic line that the JS is looking for
        view! { <App /> }
    });
}
