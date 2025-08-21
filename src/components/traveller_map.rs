use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct TravellerMapResponse {
    pub results: SearchResults,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SearchResults {
    pub count: usize,
    pub items: Vec<SearchItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SearchItem {
    pub world: Option<WorldData>,
    pub sector: Option<SectorData>,
    pub subsector: Option<SubsectorData>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct WorldData {
    pub hex_x: usize,
    pub hex_y: usize,
    pub sector: String,
    pub uwp: String,
    pub sector_x: i32,
    pub sector_y: i32,
    pub name: String,
    pub sector_tags: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SectorData {
    pub sector_x: i32,
    pub sector_y: i32,
    pub name: String,
    pub sector_tags: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct SubsectorData {
    pub sector: String,
    pub index: String,
    pub sector_x: i32,
    pub sector_y: i32,
    pub name: String,
    pub sector_tags: String,
}

pub async fn fetch_search_results(url: &str) -> Result<TravellerMapResponse, JsValue> {
    let request = web_sys::Request::new_with_str(url)?;
    let window = web_sys::window().unwrap();
    let response_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let response: web_sys::Response = response_value.dyn_into()?;
    let json = JsFuture::from(response.json()?).await?;
    let result: TravellerMapResponse = serde_wasm_bindgen::from_value(json)?;
    Ok(result)
}

/// Creates a world search component with TravellerMap integration
#[component]
pub fn WorldSearch(
    label: String,
    name: RwSignal<String>,
    uwp: RwSignal<String>,
    coords: RwSignal<Option<(i32, i32)>>,
    #[prop(default = Signal::derive(|| true))] search_enabled: Signal<bool>,
) -> impl IntoView {
    let (search_results, set_search_results) =
        signal::<Vec<(String, String, String, i32, i32)>>(vec![]);
    let (is_loading, set_is_loading) = signal(false);

    // Separate signal for the UWP input field
    let input_uwp = RwSignal::new(uwp.get_untracked());

    // Sync input_uwp when uwp changes from outside
    Effect::new(move |_| {
        let external_uwp = uwp.get();
        if external_uwp != input_uwp.get_untracked() {
            input_uwp.set(external_uwp);
        }
    });

    let handle_uwp_input = move |ev| {
        let new_uwp = event_target_value(&ev);
        input_uwp.set(new_uwp.clone());
        if new_uwp.len() == 9 {
            uwp.set(new_uwp);
        }
    };

    // Handle selection from datalist
    let handle_selection = move |_| {
        let current_name = name.get();
        let mut found = false;
        for (name, _, world_uwp, hex_x, hex_y) in search_results.get() {
            if current_name == name {
                uwp.set(world_uwp);
                coords.set(Some((hex_x, hex_y)));
                found = true;
                break;
            }
        }
        if !found {
            coords.set(None);
        }
    };

    // Debounced search function
    let search_query = Memo::new(move |_| name.get());

    Effect::new(move |_| {
        let query = search_query.get();

        if search_enabled.get() && query.len() >= 2 {
            set_is_loading.set(true);
            let url = format!("https://travellermap.com/api/search?q={query}");

            wasm_bindgen_futures::spawn_local(async move {
                match fetch_search_results(&url).await {
                    Ok(response) => {
                        let mut world_results = Vec::new();
                        for item in response.results.items {
                            if let Some(world) = item.world {
                                world_results.push((
                                    world.name,
                                    world.sector,
                                    world.uwp,
                                    world.hex_x as i32,
                                    world.hex_y as i32,
                                ));
                            }
                        }
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
            set_search_results.set(vec![]);
        }
    });

    let world_name_id = format!("worldName-{}", rand::random::<u32>());
    let uwp_id = format!("uwp-{}", rand::random::<u32>());
    let datalist_id = format!("world-suggestions-{}", rand::random::<u32>());

    view! {
        <div class:world-text-entry>
        <div>
            <label for=world_name_id.clone()>{format!("{}:",label.clone())}</label>
            <input
                id=world_name_id
                type="text"
                bind:value=name
                list=datalist_id.clone()
                on:input=handle_selection
            />
            <datalist class="world-suggestions" id=datalist_id>
                {move || {
                    search_results
                        .get()
                        .into_iter()
                        .map(|(name, sector, uwp, _, _)| {
                            view! {
                                <option value=name.clone()>{format!("{name}/{sector}/{uwp}")}</option>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </datalist>
        </div>
        <div>
            <label for=uwp_id.clone()>{format!("{label} UPP:")}</label>
            <input type="text" id=uwp_id bind:value=input_uwp on:input=handle_uwp_input />
        </div>
        <Show when=move || is_loading.get()>
            <span class="loading-indicator">"Loading..."</span>
        </Show>
        </div>
    }
}

/// Calculate distance between two hex coordinates on a Traveller map
/// Uses cube coordinate system for efficient hex distance calculation
pub fn calculate_hex_distance(hex_x1: i32, hex_y1: i32, hex_x2: i32, hex_y2: i32) -> i32 {
    // Convert offset coordinates to cube coordinates
    let (x1, y1, z1) = offset_to_cube(hex_x1, hex_y1);
    let (x2, y2, z2) = offset_to_cube(hex_x2, hex_y2);

    // Calculate distance using cube coordinates
    ((x1 - x2).abs() + (y1 - y2).abs() + (z1 - z2).abs()) / 2
}

/// Convert offset hex coordinates to cube coordinates
/// Uses odd-q offset coordinate system (Traveller standard)
/// In Traveller maps: flat top/bottom, pointy left/right, odd columns offset up
pub fn offset_to_cube(col: i32, row: i32) -> (i32, i32, i32) {
    let x = col;
    let z = row - (col + (col & 1)) / 2;
    let y = -x - z;
    (x, y, z)
}
