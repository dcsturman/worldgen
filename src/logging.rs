/// Initialize logging based on URL parameters
///
/// Checks for ?log=level&module=prefix URL parameters and initializes wasm_logger accordingly
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
