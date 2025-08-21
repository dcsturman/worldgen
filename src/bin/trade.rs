use leptos::prelude::*;
use worldgen::components::trade_computer::Trade;
use worldgen::logging;

fn main() {
    // get reasonable errors in the Javascript console from Leptos
    console_error_panic_hook::set_once();
    // Check for parameters like debug parameters in the URL.
    logging::init_from_url();
    // Mount the app to the body (run the App)
    mount_to_body(Trade);
}
