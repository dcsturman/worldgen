mod astro;
mod components;
mod gas_giant;
mod has_satellites;
mod name_tables;
mod system;
mod system_tables;
mod util;
mod world;

use leptos::prelude::*;

use crate::components::app::App;

fn main() {
    console_error_panic_hook::set_once();

    // Check URL parameters for debug flag
    let window = web_sys::window().unwrap();
    let url = window.location().search().unwrap_or_default();

    // Parse log level and module from URL parameters
    let params: std::collections::HashMap<String, String> = url
        .strip_prefix("?")
        .unwrap_or(&url)
        .split('&')
        .filter_map(|param| {
            let mut parts = param.split('=');
            Some((parts.next()?.to_string(), parts.next()?.to_string()))
        })
        .collect();

    if let Some(log_param) = params.get("log") {
        let log_level = match log_param.to_lowercase().as_str() {
            "error" => log::Level::Error,
            "warn" => log::Level::Warn,
            "info" => log::Level::Info,
            "debug" => log::Level::Debug,
            "trace" => log::Level::Trace,
            _ => return, // Invalid log level, don't initialize logger
        };

        let mut config = wasm_logger::Config::new(log_level);

        // Add module prefix if specified
        if let Some(module) = params.get("module") {
            config = config.module_prefix(module);
        } else {
            // Default to worldgen if no module specified
            config = config.module_prefix("worldgen");
        }

        wasm_logger::init(config);
    }
    mount_to_body(App);
}
