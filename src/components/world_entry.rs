use leptos::prelude::*;

#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

use crate::components::traveller_map::WorldSearch;

#[component]
pub fn WorldEntry(main_world_name: RwSignal<String>, uwp: RwSignal<String>, origin_coords: RwSignal<Option<(i32, i32)>>) -> impl IntoView {
    let search_traveller_map = RwSignal::new(true);

    view! {
        <>
            <div>
                <div class="world-text-entry">
                    <WorldSearch 
                        world_entry_label="World:".to_string()
                        world_name=main_world_name 
                        uwp=uwp 
                        world_coordinates=origin_coords
                        search_enabled=search_traveller_map.into()
                    />
                </div>
                <div class="control-container">
                    <div>
                        <input
                            type="checkbox"
                            id="search-traveller-map"
                            checked=move || search_traveller_map.get()
                            on:change=move |ev| {
                                search_traveller_map.set(event_target_checked(&ev));
                            }
                        />
                        <label for="search-traveller-map">"Search TravellerMap"</label>
                    </div>
                </div>
            </div>
        </>
    }
}
