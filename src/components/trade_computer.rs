use leptos::prelude::*;
use reactive_stores::Store;

#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

use log::debug;

use crate::components::traveller_map::WorldSearch;
use crate::systems::world::World;
use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::table::TradeTable;

use crate::INITIAL_NAME;
use crate::INITIAL_UPP;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ShowSellPriceType(bool);

#[component]
pub fn Trade() -> impl IntoView {
    // The main world always exists (starts with a default value) and we use that type in the context.
    provide_context(Store::new(
        World::from_upp(INITIAL_NAME.to_string(), INITIAL_UPP, false, true).unwrap(),
    ));
    // The destination world doesn't always exist - there is valid function w/o it.  So its an Option and starts as value None.
    // Important to remember this as given the way Leptos_store works, this is the way you differentiate between the main world
    // and the destination world in the state.
    provide_context(Store::new(None::<World>));
    provide_context(Store::new(AvailableGoodsTable::new()));
    provide_context(Store::new(None::<AvailablePassengers>));
    // Used for "show sell price"
    provide_context(Store::new(ShowSellPriceType(false)));

    let origin_world = expect_context::<Store<World>>();
    let dest_world = expect_context::<Store<Option<World>>>();
    let trade_table = TradeTable::default();
    let available_goods = expect_context::<Store<AvailableGoodsTable>>();
    let available_passengers = expect_context::<Store<Option<AvailablePassengers>>>();
    let show_sell_price = expect_context::<Store<ShowSellPriceType>>();

    // Skills involved, both player and adversary.
    let buyer_broker_skill = RwSignal::new(0);
    let seller_broker_skill = RwSignal::new(0);
    let steward_skill = RwSignal::new(0);

    let origin_world_name = RwSignal::new(origin_world.read_untracked().name.clone());
    let origin_uwp = RwSignal::new(origin_world.read_untracked().to_upp());
    let origin_coords = RwSignal::new(origin_world.read_untracked().coordinates);
    let dest_world_name = RwSignal::new("".to_string());
    let dest_uwp = RwSignal::new("".to_string());
    let dest_coords = RwSignal::new(None);
    let distance = RwSignal::new(0);

    // Keep origin world updated based on changes in name or uwp.
    Effect::new(move |_| {
        let name = origin_world_name.get();
        let uwp = origin_uwp.get();
        debug!("In first Effect: name = {name}, uwp = {uwp}");
        if !name.is_empty() && uwp.len() == 9 {
            let Ok(mut world) = World::from_upp(name, &uwp, false, false) else {
                log::error!("Failed to parse UPP in hook to build origin world: {uwp}");
                return;
            };
            world.gen_trade_classes();
            world.coordinates = dest_coords.get();

            origin_world.set(world);

            // Now update available goods
            let ag = AvailableGoodsTable::for_world(
                &trade_table,
                &origin_world.read().get_trade_classes(),
                origin_world.read().get_population(),
                false,
            )
            .unwrap();
            available_goods.set(ag);
        }
    });

    // Keep destination world updated based on changes in name or uwp.
    Effect::new(move |_| {
        let name = dest_world_name.get();
        let uwp = dest_uwp.get();

        if !name.is_empty() && uwp.len() == 9 {
            let Ok(mut world) = World::from_upp(name, &uwp, false, false) else {
                log::error!("Failed to parse UPP in hook to build destination world: {uwp}");
                dest_world.set(None);
                return;
            };
            world.gen_trade_classes();
            world.coordinates = dest_coords.get();
            dest_world.set(Some(world));
        } else {
            dest_world.set(None);
        }
    });

    Effect::new(move |_| {
        console_log("Updating goods pricing");
        let mut ag = available_goods.write();
        ag.price_goods_to_buy(
            &origin_world.read().get_trade_classes(),
            buyer_broker_skill.get(),
            seller_broker_skill.get(),
        );
        ag.price_goods_to_sell(
            dest_world.get().as_ref().map(|w| w.get_trade_classes()),
            buyer_broker_skill.get(),
            seller_broker_skill.get(),
        );
        ag.sort_by_discount();
    });

    // Effect to reset show_sell_price when either origin or destination world changes.
    Effect::new(move |_| {
        let _ = origin_world.get();
        let _ = dest_world.get();
        show_sell_price.set(ShowSellPriceType(false));
    });

    // Effect to calculate distance when coordinates change
    Effect::new(move |_| {
        if let (Some(origin), Some(dest)) = (origin_coords.get(), dest_coords.get()) {
            debug!(
                "Calculating distance ({},{}) to ({},{}).",
                origin.0, origin.1, dest.0, dest.1
            );
            let calculated_distance = crate::components::traveller_map::calculate_hex_distance(
                origin.0, origin.1, dest.0, dest.1,
            );
            console_log(format!("Calculated distance: {calculated_distance}").as_str());
            distance.set(calculated_distance);
        }
    });

    // Effect to update passengers when destination world, distance, or steward skill changes
    Effect::new(move |_| {
        if let Some(world) = dest_world.get() {
            if distance.get() > 0 {
                available_passengers.set(Some(AvailablePassengers::generate(
                    origin_world.read().get_population(),
                    origin_world.read().port,
                    None,
                    origin_world.read().tech_level,
                    world.get_population(),
                    world.port,
                    None,
                    world.tech_level,
                    distance.get(),
                    steward_skill.get(),
                )));
            } else {
                available_passengers.set(None);
            }
        } else {
            available_passengers.set(None);
        }
    });

    view! {
        <div class:App>
            <h1 class="d-print-none">Trade Computer</h1>
            <div class="key-region world-entry-form">
                <div>
                    <WorldSearch
                        label="Origin".to_string()
                        name=origin_world_name
                        uwp=origin_uwp
                        coords=origin_coords
                    />

                </div>
                <WorldSearch
                    label="Destination".to_string()
                    name=dest_world_name
                    uwp=dest_uwp
                    coords=dest_coords
                />
            </div>
            <div class:key-region>
                <div class:skill-entry>
                    <div>
                        <label for="distance">"Distance: "</label>
                        <input
                            class="distance-input"
                            type="number"
                            id="distance"
                            value=move || distance.get().to_string()
                            on:input=move |ev| {
                                if let Ok(val) = event_target_value(&ev).parse::<i32>() {
                                    debug!("Setting distance to {val}");
                                    distance.set(val);
                                }
                            }
                        />
                    </div>
                    <div>
                        <span>
                            "Origin Classes: "
                            {move || format!("[{}]", origin_world.read().trade_classes_string())}
                        </span>
                    </div>
                    <div>
                        <span>
                            {move || {
                                if let Some(world) = dest_world.get() {
                                    format!(
                                        "Destination Trade Classes: [{}]",
                                        world.trade_classes_string(),
                                    )
                                } else {
                                    "".to_string()
                                }
                            }}
                        </span>
                    </div>
                </div>
                <div class="skill-entry">
                    <div>
                        <label for="player-broker-skill">"Player Broker Skill:"</label>
                        <input
                            type="number"
                            id="player-broker-skill"
                            min="0"
                            max="100"
                            value=move || buyer_broker_skill.get()
                            on:change=move |ev| {
                                buyer_broker_skill
                                    .set(event_target_value(&ev).parse().unwrap_or(0));
                            }
                        />
                    </div>
                    <div>
                        <label for="system-broker-skill">"System Broker Skill:"</label>
                        <input
                            type="number"
                            id="system-broker-skill"
                            min="0"
                            max="100"
                            value=move || seller_broker_skill.get()
                            on:change=move |ev| {
                                seller_broker_skill
                                    .set(event_target_value(&ev).parse().unwrap_or(0));
                            }
                        />
                    </div>
                    <div>
                        <label for="steward-skill">"Steward Skill:"</label>
                        <input
                            type="number"
                            id="steward-skill"
                            min="0"
                            max="100"
                            value=move || steward_skill.get()
                            on:change=move |ev| {
                                steward_skill.set(event_target_value(&ev).parse().unwrap_or(0));
                            }
                        />
                    </div>
                </div>
            </div>

            <TradeView />

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
pub fn TradeView() -> impl IntoView {
    let origin_world = expect_context::<Store<World>>();
    let dest_world = expect_context::<Store<Option<World>>>();

    let available_goods = expect_context::<Store<AvailableGoodsTable>>();
    let available_passengers = expect_context::<Store<Option<AvailablePassengers>>>();

    let show_sell_price = expect_context::<Store<ShowSellPriceType>>();

    view! {
        <div class="output-region">
            <h2>
                "Trade Goods for " {move || origin_world.read().name.clone()} " ["
                {move || origin_world.read().trade_classes_string()}"]"
            </h2>
            <Show when=move || available_passengers.get().is_some()>
                <PassengerView />
            </Show>

            <h4 style="font-size: 14pt;">"Speculation Goods"</h4>
            <table class="trade-table">
                <thead>
                    {move || {
                        if dest_world.get().is_none() {
                            view! {
                                <tr>
                                    <th class="table-entry">"Good"</th>
                                    <th class="table-entry">"Quantity"</th>
                                    <th class="table-entry">"Base Price"</th>
                                    <th class="table-entry">"Buy Price"</th>
                                    <th class="table-entry">"Discount"</th>
                                </tr>
                            }
                                .into_any()
                        } else {
                            view! {
                                <tr>
                                    <th class="table-entry">"Good"</th>
                                    <th class="table-entry">"Quantity"</th>
                                    <th class="table-entry">"Base Price"</th>
                                    <th class="table-entry">"Buy Price"</th>
                                    <th class="table-entry">"Discount"</th>
                                    <Show when=move || show_sell_price.read().0>
                                        <th class="table-entry">"Sell Price"</th>
                                        <th class="table-entry">"Discount"</th>
                                    </Show>
                                    <Show when=move || !show_sell_price.read().0>
                                        <th class="table-entry">
                                            <button
                                                class="sell-price-button"
                                                on:click=move |_| {
                                                    show_sell_price.set(ShowSellPriceType(true))
                                                }
                                            >
                                                "Sell Price"
                                            </button>
                                        </th>
                                    </Show>
                                </tr>
                            }
                                .into_any()
                        }
                    }}
                </thead>
                <tbody>
                    {move || {
                        if available_goods.read().is_empty() {
                            view! {
                                <tr>
                                    <td colspan="5">"No goods available"</td>
                                </tr>
                            }
                                .into_any()
                        } else {
                            available_goods
                                .get()
                                .goods()
                                .iter()
                                .map(|good| {
                                    let discount_percent = (good.cost as f64 / good.base_cost as f64
                                        * 100.0)
                                        .round() as i32;
                                    if let Some(sell_price) = good.sell_price {
                                        let sell_discount_percent = (sell_price as f64
                                            / good.base_cost as f64 * 100.0)
                                            .round() as i32;
                                        view! {
                                            <tr>
                                                <td class="table-entry">{good.name.clone()}</td>
                                                <td class="table-entry">{good.quantity.to_string()}</td>
                                                <td class="table-entry">{good.base_cost.to_string()}</td>
                                                <td class="table-entry">{good.cost.to_string()}</td>
                                                <td class="table-entry">
                                                    {discount_percent.to_string()}"%"
                                                </td>
                                                <Show when=move || show_sell_price.read().0>
                                                    <td class="table-entry">{sell_price.to_string()}</td>
                                                    <td class="table-entry">
                                                        {sell_discount_percent.to_string()}"%"
                                                    </td>
                                                </Show>
                                            </tr>
                                        }
                                            .into_any()
                                    } else {
                                        view! {
                                            <tr>
                                                <td class="table-entry">{good.name.clone()}</td>
                                                <td class="table-entry">{good.quantity.to_string()}</td>
                                                <td class="table-entry">{good.base_cost.to_string()}</td>
                                                <td class="table-entry">{good.cost.to_string()}</td>
                                                <td class="table-entry">
                                                    {discount_percent.to_string()}"%"
                                                </td>
                                                <Show when=move || show_sell_price.read().0>
                                                    <td class="table-entry">"-"</td>
                                                    <td class="table-entry">"-"</td>
                                                </Show>
                                            </tr>
                                        }
                                            .into_any()
                                    }
                                })
                                .collect::<Vec<_>>()
                                .into_any()
                        }
                    }}
                </tbody>
            </table>

        </div>
    }
}

#[component]
fn PassengerView() -> impl IntoView {
    let available_passengers = expect_context::<Store<Option<AvailablePassengers>>>();
    view! {
        <h4 style="font-size: 14pt;">"Available Passengers"</h4>
        <div class="passengers-grid">
            <div class="passenger-type">
                <h4>"High"</h4>
                <div class="passenger-count">
                    {move || {
                        if let Some(passengers) = available_passengers.get() {
                            passengers.high.to_string()
                        } else {
                            "0".to_string()
                        }
                    }}
                </div>
            </div>
            <div class="passenger-type">
                <h4>"Medium"</h4>
                <div class="passenger-count">
                    {move || {
                        if let Some(passengers) = available_passengers.get() {
                            passengers.medium.to_string()
                        } else {
                            "0".to_string()
                        }
                    }}
                </div>
            </div>
            <div class="passenger-type">
                <h4>"Basic"</h4>
                <div class="passenger-count">
                    {move || {
                        if let Some(passengers) = available_passengers.get() {
                            passengers.basic.to_string()
                        } else {
                            "0".to_string()
                        }
                    }}
                </div>
            </div>
            <div class="passenger-type">
                <h4>"Low"</h4>
                <div class="passenger-count">
                    {move || {
                        if let Some(passengers) = available_passengers.get() {
                            passengers.low.to_string()
                        } else {
                            "0".to_string()
                        }
                    }}
                </div>
            </div>
        </div>

        <h4 style="font-size: 14pt;">"Available Freight (tons)"</h4>
        <div class="freight-grid">
            {move || {
                if let Some(passengers) = available_passengers.get() {
                    if passengers.freight_lots.is_empty() {
                        view! { <div>"No freight available"</div> }.into_any()
                    } else {
                        let mut sorted_lots = passengers.freight_lots.clone();
                        sorted_lots.sort_by(|a, b| b.size.cmp(&a.size));
                        sorted_lots
                            .iter()
                            .map(|lot| {

                                view! { <div class="freight-lot">{lot.size.to_string()}</div> }
                            })
                            .collect::<Vec<_>>()
                            .into_any()
                    }
                } else {
                    view! { <div>"No freight available"</div> }.into_any()
                }
            }}
        </div>
    }
}
