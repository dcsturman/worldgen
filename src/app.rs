use leptos::prelude::*;
use reactive_stores::Store;

use crate::worldgen::{System, World, generate_system};
use crate::systemview::SystemView;

const INITIAL_UPP: &str = "A788899-A";
const INITIAL_NAME: &str = "Main World";

#[component]
pub fn App() -> impl IntoView {
    let main_world_name = RwSignal::new(INITIAL_NAME.to_string());
    let (has_gen, set_has_gen) = signal(false);

    provide_context(Store::new(System::default()));

    view! {
        <div class:App>
            <h1 class="d-print-none">Solar System Generator</h1>
            <WorldEntryForm main_world_name set_has_gen />
            <Show when=move || {
                has_gen.get()
            }>{move || view! { <SystemView main_world_name /> }}</Show>
            <br />
        </div>
    }
}

fn print() {
    leptos::leptos_dom::helpers::window().print().unwrap_or_else(|e| log::error!("Error printing: {:?}", e));
}

#[component]
fn WorldEntryForm(main_world_name: RwSignal<String>, set_has_gen: WriteSignal<bool>) -> impl IntoView {
    let state = expect_context::<Store<System>>();

    let upp = RwSignal::new(INITIAL_UPP.to_string());

    let handle_submit = move |_e| {
        let new_system = generate_system(World::from_upp(main_world_name.get(), &upp.get(), false, true));
        set_has_gen.set(true);
        state.set(new_system);
    };

    view! {
        <div class="d-print-none world-entry-form">
            <div id:entry-data>
                <div class:world-entry-element>
                    <label for:worldName>"World Name:"</label>
                    <input id="worldName" class:entry-input type="text" bind:value=main_world_name />
                </div>
                <div class:world-entry-element>
                    <label for:upp>"UPP:"</label>
                    <input class:entry-input type="text" id="upp" bind:value=upp />
                </div>
            </div>
            <div id:entry-buttons>
                <button class:blue-button type="button" on:click=handle_submit>
                    "Generate"
                </button>
                <button class:blue-button type="button" on:click=|_| print() >
                    "Print"
                </button>
            </div>
        </div>
    }

}