use leptos::prelude::*;
use worldgen::components::system_generator::World;
use worldgen::logging;

fn main() {
    console_error_panic_hook::set_once();
    logging::init_from_url();
    mount_to_body(World);
}
