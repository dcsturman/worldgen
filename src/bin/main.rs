use leptos::prelude::*;
use worldgen::components::trade_computer::Trade;
use worldgen::components::system_generator::World;
use worldgen::components::selector::Selector;
use worldgen::logging;

#[component]
fn App() -> impl IntoView {
    let path = web_sys::window()
        .unwrap()
        .location()
        .pathname()
        .unwrap_or_default();
    
    if path.contains("world") || path.contains("system") {
        view! { <World /> }.into_any()
    } else if path.contains("trade") {
        view! { <Trade /> }.into_any()
    } else {
        view! { <Selector /> }.into_any()
    }
}

fn main() {
    console_error_panic_hook::set_once();
    logging::init_from_url();
    mount_to_body(App);
}
