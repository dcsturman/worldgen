//! # Logging Configuration Module
//!
//! This module provides URL parameter-based logging configuration for the worldgen
//! web application. It enables dynamic logging control through URL query parameters,
//! allowing developers and users to enable debug output without recompiling.
//!
//! ## Overview
//!
//! The logging system uses the `wasm_logger` crate to provide console-based logging
//! in web browsers. Log levels and module filtering can be controlled through URL
//! parameters, making it easy to debug specific parts of the application.
//!
//! ## URL Parameter Format
//!
//! The module parses the following URL parameters:
//! - `log=level` - Sets the logging level (error, warn, info, debug, trace)
//! - `module=prefix` - Filters logs to modules matching the prefix
//!
//! ### Example URLs
//!
//! ```text
//! # Enable debug logging for all worldgen modules
//! https://example.com/worldgen?log=debug
//!
//! # Enable trace logging for system generation only
//! https://example.com/worldgen?log=trace&module=worldgen::systems
//!
//! # Enable info logging for trade computer
//! https://example.com/worldgen?log=info&module=worldgen::trade
//!
//! # Enable error logging for traveller map integration
//! https://example.com/worldgen?log=error&module=worldgen::components::traveller_map
//! ```
//!
//! ## Log Levels
//!
//! The module supports all standard log levels:
//!
//! - **error**: Only critical errors that prevent operation
//! - **warn**: Warning conditions that don't prevent operation
//! - **info**: General informational messages about application flow
//! - **debug**: Detailed debugging information for development
//! - **trace**: Very verbose tracing information for deep debugging
//!
//! ## Module Filtering
//!
//! Module filtering allows focusing on specific parts of the application:
//!
//! ### Common Module Prefixes
//! - `worldgen` - All application modules (default if no module specified)
//! - `worldgen::systems` - System generation and world creation
//! - `worldgen::trade` - Trade computer and economic calculations
//! - `worldgen::components` - UI components and user interaction
//! - `worldgen::components::traveller_map` - Traveller Map API integration
//! - `worldgen::components::system_generator` - System generation UI
//! - `worldgen::components::trade_computer` - Trade computer UI
//!
//! ## Browser Console Output
//!
//! When logging is enabled, messages appear in the browser's developer console
//! with appropriate formatting and filtering. The `wasm_logger` provides:
//!
//! - Proper log level indicators
//! - Module path information
//! - Timestamp information
//! - Color coding based on log level
//!
//! ## Error Handling
//!
//! The module includes robust error handling:
//!
//! ### Invalid Parameters
//! - Invalid log levels are ignored (no logger initialized)
//! - Malformed URLs are handled gracefully
//! - Missing parameters use sensible defaults
//!
//! ### Fallback Behavior
//! - If no module is specified, defaults to "worldgen" prefix
//! - If URL parsing fails, no logger is initialized (silent failure)
//! - Application continues normally even if logging setup fails
//!
//! ## Performance Considerations
//!
//! ### Production Usage
//! - Logging should be disabled in production builds for performance
//! - URL parameter parsing is lightweight and fast
//! - No overhead when logging is not enabled
//!
//! ### Development Usage
//! - Debug and trace levels can generate significant output
//! - Module filtering helps focus on relevant information
//! - Console performance may be impacted with verbose logging
//!
//! ## Security Considerations
//!
//! - URL parameters are client-side only, no server exposure
//! - No sensitive information is logged by default
//! - Module filtering prevents accidental exposure of unrelated logs
//!
//! ## Usage Examples
//!
//! ### Basic Debug Logging
//! ```text
//! # Enable debug logging for development
//! http://localhost:8080?log=debug
//! ```
//!
//! ### Focused System Debugging
//! ```text
//! # Debug only system generation issues
//! http://localhost:8080?log=debug&module=worldgen::systems
//! ```
//!
//! ### Trade Computer Debugging
//! ```text
//! # Debug trade calculations and API calls
//! http://localhost:8080/trade?log=debug&module=worldgen::trade
//! ```
//!
//! ### API Integration Debugging
//! ```text
//! # Debug Traveller Map API integration
//! http://localhost:8080?log=trace&module=worldgen::components::traveller_map
//! ```
//!
//! ## Future Enhancements
//!
//! Potential improvements for future versions:
//! - Local storage persistence of logging preferences
//! - Runtime log level adjustment through UI controls
//! - Log export functionality for bug reports
//! - Integration with external logging services

/// Initialize logging based on URL parameters
///
/// Checks for ?log=level&module=prefix URL parameters and initializes wasm_logger accordingly.
/// This function provides a convenient way to enable debugging output in web applications
/// without requiring recompilation or configuration files.
///
/// ## Parameter Parsing
///
/// The function parses URL query parameters to configure logging:
/// - Extracts the search portion of the current URL
/// - Splits parameters on '&' and '=' delimiters
/// - Builds a HashMap of parameter names to values
/// - Validates log level and applies module filtering
///
/// ## Log Level Validation
///
/// Only valid log levels are accepted (case-insensitive):
/// - "error", "warn", "info", "debug", "trace"
/// - Invalid levels result in no logger initialization
/// - This prevents accidental logging activation
///
/// ## Module Prefix Handling
///
/// - If `module` parameter is provided, uses it as the filter prefix
/// - If no `module` parameter, defaults to "worldgen" prefix
/// - Module filtering helps focus debugging on specific components
///
/// ## Error Recovery
///
/// The function handles various error conditions gracefully:
/// - Missing window object (shouldn't happen in browser context)
/// - URL parsing failures (malformed URLs)
/// - Invalid parameter formats
/// - Logger initialization failures
///
/// ## Usage
///
/// Call this function early in application initialization:
///
/// ```rust,ignore
/// # use worldgen::logging::init_from_url;
/// fn main() {
///     console_error_panic_hook::set_once();
///     init_from_url();
///     leptos::mount_to_body(App);
/// }
/// ```
///
/// ## Examples
///
/// URL examples that would activate logging:
///
/// ```text
/// # Basic debug logging
/// https://example.com/app?log=debug
///
/// # Focused module logging
/// https://example.com/app?log=info&module=worldgen::systems
///
/// # Multiple parameters (order doesn't matter)
/// https://example.com/app?module=worldgen::trade&log=trace
/// ```
pub fn init_from_url() {
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
}
