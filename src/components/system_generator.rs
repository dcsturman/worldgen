#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

#[allow(unused_imports)]
use log::debug;

use leptos::prelude::*;
use reactive_stores::Store;

use crate::components::system_view::SystemView;
use crate::components::traveller_map::WorldSearch;
use crate::systems::system::System;
use crate::systems::world::World;

use crate::INITIAL_NAME;
use crate::INITIAL_UPP;

// I may want this later so allowing it to stay without a warning.
#[allow(dead_code)]
fn print() {
    leptos::leptos_dom::helpers::window()
        .print()
        .unwrap_or_else(|e| log::error!("Error printing: {e:?}"));
}

#[component]
pub fn World() -> impl IntoView {
    provide_context(Store::new(
        World::from_upp(INITIAL_NAME.to_string(), INITIAL_UPP, false, true).unwrap(),
    ));
    provide_context(Store::new(System::default()));

    let system = expect_context::<Store<System>>();
    let main_world = expect_context::<Store<World>>();

    // When changed should change the name of main_world through an effect.
    // But we want it separate to avoid loops in the first Effect we create.
    let main_world_name = RwSignal::new(main_world.read_untracked().name.clone());
    let origin_coords = RwSignal::new(None::<(i32, i32)>);

    let upp = RwSignal::new(INITIAL_UPP.to_string());

    Effect::new(move |_| {
        let upp = upp.get();
        let name = main_world_name.get();
        debug!("Building world {name} with UPP {upp}");
        let Ok(mut w) = World::from_upp(name, upp.as_str(), false, true) else {
            // If the upp is not properly structured, then just give up and bail out.
            log::error!("Failed to parse UPP in hook to build main world: {upp}");
            return;
        };
        w.coordinates = origin_coords.get();
        w.gen_trade_classes();
        main_world.set(w);
        system.set(System::generate_system(main_world.get()));
    });

    view! {
        <div class:App>
        <h1 class="d-print-none">Solar System Generator</h1>
        <div class="d-print-none key-region world-entry-form">
            <WorldSearch label="Main World".to_string() name=main_world_name uwp=upp coords=origin_coords />
        </div>
        <SystemView />
        </div>
    }
}
