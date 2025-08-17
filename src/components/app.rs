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
    let main_world = expect_context::<Store<World>>();
    let main_world_name = RwSignal::new(main_world.read().name.clone());
    view! {
        <div class:App>
            <h1 class="d-print-none">Solar System Generator</h1>
            <WorldEntryForm show_system set_show_system show_trade set_show_trade />
            <Show when=move || {
                show_system.get()
            }>{move || view! { <SystemView main_world_name=main_world_name /> }}</Show>
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
    let main_world_name = RwSignal::new(main_world.read().name.clone());

    let buyer_broker_skill = RwSignal::new(0);
    let seller_broker_skill = RwSignal::new(0);

    let upp = RwSignal::new(INITIAL_UPP.to_string());

    Effect::new(move |_| {
        console_log("Generating world");
        let upp = upp.get();
        let name = main_world_name.get();
        let mut w = World::from_upp(name, upp.as_str(), false, true);
        w.gen_trade_classes();
        main_world.set(w);
        system.set(System::generate_system(main_world.get()));
    });

    // Regenerate the available goods when the main world changes
    Effect::new(move |_| {
        console_log("Generating available goods");
        let mut new_ag = AvailableGoodsTable::for_world(
            &trade_table.get(),
            &main_world.read().get_trade_classes(),
            main_world.read().get_population(),
            false,
        )
        .expect("Failed to create available goods table");
        new_ag.price_goods(buyer_broker_skill.get(), seller_broker_skill.get());
        new_ag.sort_by_discount(); 
        available_goods.set(new_ag);
    });

    Effect::new(move |_| {
        console_log("Pricing goods");
        let mut ag = available_goods.write();
        // Price the goods based on broker skills and sort them
        ag.price_goods(buyer_broker_skill.get(), seller_broker_skill.get());
        ag.sort_by_discount();
    });

    let handle_system_check = move |ev| set_show_system.set(event_target_checked(&ev));
    let handle_trade_check = move |ev| set_show_trade.set(event_target_checked(&ev));

    view! {
        <div class="d-print-none world-entry-form">
            <WorldEntry main_world_name=main_world_name uwp=upp />
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
                <Show when=move || show_trade.get()>
                <div class="control-container">
                    <label for="buyer-broker-skill">"Buyer Broker Skill:"</label>
                    <input
                        type="number"
                        id="buyer-broker-skill"
                        min="0"
                        max="100"
                        value=move || buyer_broker_skill.get()
                        on:change=move |ev| {
                            buyer_broker_skill.set(event_target_value(&ev).parse().unwrap_or(0));
                        }
                    />
                </div>
                </Show>
                <Show when=move || show_trade.get()>
                <div class="control-container">
                    <label for="seller-broker-skill">"Seller Broker Skill:"</label>
                    <input
                        type="number"
                        id="seller-broker-skill"
                        min="0"
                        max="100"
                        value=move || seller_broker_skill.get()
                        on:change=move |ev| {
                            seller_broker_skill.set(event_target_value(&ev).parse().unwrap_or(0));
                        }
                    />
                </div>
                </Show>
            </div>
        </div>
    }
}
