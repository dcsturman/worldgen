use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct TravellerMapResponse {
    results: SearchResults,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct SearchResults {
    count: usize,
    items: Vec<SearchItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct SearchItem {
    world: Option<WorldData>,
    sector: Option<SectorData>,
    subsector: Option<SubsectorData>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct WorldData {
    hex_x: usize,
    hex_y: usize,
    sector: String,
    uwp: String,
    sector_x: i32,
    sector_y: i32,
    name: String,
    sector_tags: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct SectorData {
    sector_x: i32,
    sector_y: i32,
    name: String,
    sector_tags: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct SubsectorData {
    sector: String,
    index: String,
    sector_x: i32,
    sector_y: i32,
    name: String,
    sector_tags: String,
}

#[component]
pub fn WorldEntry(world_name: RwSignal<String>, uwp: RwSignal<String>) -> impl IntoView {
    let search_traveller_map = RwSignal::new(true);
    let (search_results, set_search_results) = signal::<Vec<(String, String, String)>>(vec![]);
    let (is_loading, set_is_loading) = signal(false);

    // Separate signal for the input field
    let input_uwp = RwSignal::new(uwp.get_untracked());

    // Sync input_uwp when uwp changes from outside
    Effect::new(move |_| {
        let external_uwp = uwp.get();
        if external_uwp != input_uwp.get_untracked() {
            input_uwp.set(external_uwp);
        }
    });

    // Debounced search function
    let search_query = Memo::new(move |_| world_name.get());

    let handle_uwp_input = move |ev| {
        let new_uwp = event_target_value(&ev);
        input_uwp.set(new_uwp.clone());
        if new_uwp.len() == 9 {
            uwp.set(new_uwp);
        }
        // If invalid length, don't update uwp signal but let user keep typing
    };

    Effect::new(move |_| {
        let query = search_query.get();

        if search_traveller_map.get() && query.len() >= 2 {
            set_is_loading.set(true);

            // Create the search URL
            let url = format!("https://travellermap.com/api/search?q={query}");

            // Perform the search
            wasm_bindgen_futures::spawn_local(async move {
                match fetch_search_results(&url).await {
                    Ok(response) => {
                        // Process the results
                        let mut world_results = Vec::new();

                        for item in response.results.items {
                            if let Some(world) = item.world {
                                world_results.push((world.name, world.sector, world.uwp));
                            }
                        }

                        // Limit to 6 results
                        if world_results.len() > 6 {
                            world_results.truncate(6);
                        }

                        set_search_results.set(world_results);
                        set_is_loading.set(false);
                    }
                    Err(err) => {
                        log::error!("Error fetching search results: {err:?}");
                        set_is_loading.set(false);
                    }
                }
            });
        } else {
            // Clear results if search is disabled or query is too short
            set_search_results.set(vec![]);
        }
    });

    // Handle selection from datalist
    let handle_selection = move |_| {
        let current_name = world_name.get();
        // Find the matching result
        for (name, _, world_uwp) in search_results.get() {
            if current_name == name {
                uwp.set(world_uwp);
                break;
            }
        }
    };

    view! {
        <>
            <div>
                <div class="world-text-entry">
                    <div>
                        <label for:worldName>"World Name:"</label>
                        <input
                            id="worldName"
                            class:entry-input
                            type="text"
                            bind:value=world_name
                            list="world-suggestions"
                            on:input=handle_selection
                        />
                        // Datalist for suggestions - moved here to be adjacent to the input
                        <datalist id="world-suggestions">
                            {move || {
                                search_results
                                    .get()
                                    .into_iter()
                                    .map(|(name, sector, uwp)| {
                                        view! {
                                            <option value=name
                                                .clone()>{format!("{name}/{sector}/{uwp}")}</option>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </datalist>
                    </div>

                    <div>
                        <label for:upp>"UPP:"</label>
                        <input type="text" id="upp" bind:value=input_uwp on:input=handle_uwp_input />
                    </div>
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
            // Loading indicator
            <Show when=move || is_loading.get()>
                <span class="loading-indicator">"Loading..."</span>
            </Show>
        </>
    }
}

async fn fetch_search_results(url: &str) -> Result<TravellerMapResponse, JsValue> {
    // Create a request
    let request = web_sys::Request::new_with_str(url)?;

    // Fetch the response
    let window = web_sys::window().unwrap();
    let response_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let response: web_sys::Response = response_value.dyn_into()?;

    // Parse the JSON
    let json = JsFuture::from(response.json()?).await?;
    let result: TravellerMapResponse = serde_wasm_bindgen::from_value(json)?;

    Ok(result)
}
