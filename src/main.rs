mod worldgen;
mod name_tables;
mod system_tables;
mod app;
mod systemview;
mod worldtable;

use console_error_panic_hook;
use leptos::prelude::*;

use crate::app::App;

fn main() {
    console_error_panic_hook::set_once();

    mount_to_body(App);
}