mod worldgen;
mod astrodata;
mod name_tables;
mod system_tables;
mod components;

use console_error_panic_hook;
use leptos::prelude::*;

use crate::components::app::App;

fn main() {
    console_error_panic_hook::set_once();

    mount_to_body(App);
}