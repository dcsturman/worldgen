#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;
use leptos::prelude::*;
use reactive_stores::Store;

use trade::available_goods::AvailableGoodsTable;
use trade::table::TradeTable;

use crate::components::system_view::SystemView;
use crate::components::trade_view::TradeView;
use crate::components::world_entry::WorldEntry;
use crate::system::System;
use crate::world::World;

const INITIAL_UPP: &str = "A788899-A";
const INITIAL_NAME: &str = "Main World";

#[component]
pub fn App() -> impl IntoView {
    let (show_system, set_show_system) = signal(false);
    let (show_trade, set_show_trade) = signal(false);

    provide_context(Store::new(World::from_upp(INITIAL_NAME.to_string(), INITIAL_UPP, false, true)));
    provide_context(Store::new(System::default()));
    provide_context(Store::new(TradeTable::standard().unwrap()));
    provide_context(Store::new(AvailableGoodsTable::new()));

    view! {
        <div class:App>
            <h1 class="d-print-none">Solar System Generator</h1>
            <WorldEntryForm show_system set_show_system show_trade set_show_trade />
            <Show when=move || {
                show_system.get()
            }>{move || view! { <SystemView /> }}</Show>
            <Show when=move || {
                show_trade.get()
            }>{move || view! { <TradeView /> }}</Show>
            <br />
        </div>
    }
}

// I may want this later so allowing it to stay without a warning.
#[allow(dead_code)]
fn print() {
    leptos::leptos_dom::helpers::window()
        .print()
        .unwrap_or_else(|e| log::error!("Error printing: {e:?}"));
}

#[component]
fn WorldEntryForm(
    show_system: ReadSignal<bool>,
    set_show_system: WriteSignal<bool>,
    show_trade: ReadSignal<bool>,
    set_show_trade: WriteSignal<bool>,
) -> impl IntoView {
    let system = expect_context::<Store<System>>();
    let main_world = expect_context::<Store<World>>();
    let available_goods = expect_context::<Store<AvailableGoodsTable>>();
    let trade_table = expect_context::<Store<TradeTable>>();

    // When changed should change the name of main_world through an effect.
    // But we want it separate to avoid loops in the first Effect we create.
    let main_world_name = RwSignal::new(main_world.read_untracked().name.clone());
    let origin_coords = RwSignal::new(None::<(i32, i32)>);

    let upp = RwSignal::new(INITIAL_UPP.to_string());

    Effect::new(move |_| {
        let upp = upp.get();
        let name = main_world_name.get();
        let mut w = World::from_upp(name, upp.as_str(), false, true);
        w.coordinates = origin_coords.get();
        w.gen_trade_classes();
        main_world.set(w);
        system.set(System::generate_system(main_world.get()));
    });

    // Regenerate the available goods when the UPP of the main world changes (not the name)
    Effect::new(move |_| {
        let upp = upp.get(); // Only track UPP changes
        
        // Create a temporary world just to get trade classes and population
        let mut temp_world = World::from_upp("temp".to_string(), upp.as_str(), false, true);
        temp_world.gen_trade_classes();
        
        let mut new_ag = AvailableGoodsTable::for_world(
            &trade_table.get(),
            &temp_world.get_trade_classes(),
            temp_world.get_population(),
            false,
        )
        .expect("Failed to create available goods table");
        
        // Apply default pricing (0 broker skills)
        new_ag.price_goods_to_buy(&temp_world.get_trade_classes(), 0, 0);
        new_ag.price_goods_to_sell(None, 0, 0);
        new_ag.sort_by_discount();
        
        available_goods.set(new_ag);
    });

    let handle_system_check = move |ev| set_show_system.set(event_target_checked(&ev));
    let handle_trade_check = move |ev| set_show_trade.set(event_target_checked(&ev));

    view! {
        <div class="d-print-none world-entry-form">
            <WorldEntry main_world_name=main_world_name uwp=upp origin_coords=origin_coords />
            <div id:entry-controls>
                <div class="control-container">
                    <div>
                        <input
                            type="checkbox"
                            id="show-system-box"
                            checked=move || show_system.get()
                            on:change=handle_system_check
                        />
                        <label for="show-system-box">"System"</label>
                    </div>
                </div>
                <div class="control-container">
                    <div>
                        <input
                            type="checkbox"
                            id="show-trade-box"
                            checked=move || show_trade.get()
                            on:change=handle_trade_check
                        />
                        <label for="show-trade-box">"Trade"</label>
                    </div>
                </div>
            </div>
        </div>
    }
}
