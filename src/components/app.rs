use leptos::prelude::*;
use reactive_stores::Store;
#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

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
    let main_world_name = RwSignal::new(INITIAL_NAME.to_string());
    let (has_gen, set_has_gen) = signal(false);
    let (show_trade, set_show_trade) = signal(false);

    provide_context(Store::new(System::default()));
    provide_context(Store::new(AvailableGoodsTable::new()));
    

    view! {
        <div class:App>
            <h1 class="d-print-none">Solar System Generator</h1>
            <WorldEntryForm main_world_name set_has_gen set_show_trade />
            <Show when=move || {
                has_gen.get()
            }>{move || view! { <SystemView main_world_name /> }}</Show>
            <Show when=move || {
                show_trade.get()
            }>{move || view! { <TradeView main_world_name/> }}</Show>
            <br />
        </div>
    }
}

fn print() {
    leptos::leptos_dom::helpers::window()
        .print()
        .unwrap_or_else(|e| log::error!("Error printing: {e:?}"));
}

#[component]
fn WorldEntryForm(
    main_world_name: RwSignal<String>,
    set_has_gen: WriteSignal<bool>,
    set_show_trade: WriteSignal<bool>,
) -> impl IntoView {
    let system = expect_context::<Store<System>>();
    let trade = expect_context::<Store<AvailableGoodsTable>>();

    let upp = RwSignal::new(INITIAL_UPP.to_string());

    let main_world = move || World::from_upp(main_world_name.get(), &upp.get(), false, true);

    let handle_submit = move |_e| {
        let new_system = System::generate_system(main_world());
        set_has_gen.set(true);
        system.set(new_system);
    };

    let handle_trade = move |_e| {
        let trade_table = TradeTable::standard().expect("Failed to create standard trade table");
        let mut available_goods = AvailableGoodsTable::for_world(
            &trade_table,
            &main_world().get_trade_classes(),
            main_world().get_population(),
            false,
        )
        .expect("Failed to create available goods table");

        // Create a signal for the broker skills
        let buyer_broker_skill = RwSignal::new(0);
        let seller_broker_skill = RwSignal::new(0);

        // Price the goods based on broker skills
        available_goods.price_goods(buyer_broker_skill.get(), seller_broker_skill.get());

        // Sort by discount
        available_goods.sort_by_discount();
        trade.set(available_goods);
        set_show_trade.set(true);
    };

    view! {
        <div class="d-print-none world-entry-form">
            <WorldEntry world_name=main_world_name uwp=upp />
            <div id:entry-buttons>
                <button class:blue-button type="button" on:click=handle_submit>
                    "System"
                </button>
                <button class:blue-button type="button" on:click=handle_trade>
                    "Trade Tables"
                </button>
                                <button class:blue-button type="button" on:click=|_| print()>
                    "Print"
                </button>
            </div>
        </div>
    }
}
