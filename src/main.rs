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

    mount_to_body(App);
}
