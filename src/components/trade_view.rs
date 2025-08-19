use leptos::prelude::*;
use reactive_stores::Store;

#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

use crate::world::World;
use crate::components::traveller_map::WorldSearch;

use trade::available_goods::AvailableGoodsTable;

#[component]
pub fn TradeView() -> impl IntoView {
    let main_world = expect_context::<Store<World>>();
    let available_goods = expect_context::<Store<AvailableGoodsTable>>();
    let available_passengers = RwSignal::new(None::<trade::available_passengers::AvailablePassengers>);

    // Broker skills
    let buyer_broker_skill = RwSignal::new(0);
    let seller_broker_skill = RwSignal::new(0);
    let steward_skill = RwSignal::new(0);
    let show_sell_price = RwSignal::new(false);

    
    // Destination world state
    let dest_world_name = RwSignal::new("".to_string());
    let dest_uwp = RwSignal::new("".to_string());
    let dest_coords = RwSignal::new(None);
    let distance = RwSignal::new(0);

    // Destination world object
    let dest_world = Memo::new(move |_| {
        let name = dest_world_name.get();
        let uwp = dest_uwp.get();
        if !name.is_empty() && uwp.len() == 9 {
            let mut world = World::from_upp(name, &uwp, false, false);
            world.gen_trade_classes();
            world.coordinates = dest_coords.get();
            Some(world)
        } else {
            None
        }
    });

    // Effect to reset show_sell_price when UPPs change
    Effect::new(move |_| {
        let _ = main_world.read().to_upp();
        let _ = dest_uwp.get();
        show_sell_price.set(false);
    });

    // Effect to update goods pricing when broker skills change, main world (trade classes) change, or destination world changes.
    Effect::new(move |_| {
        let mut ag = available_goods.write();
        ag.price_goods_to_buy(&main_world.read().get_trade_classes(), buyer_broker_skill.get(), seller_broker_skill.get());
        ag.price_goods_to_sell(
            dest_world.get().as_ref().map(|w| w.get_trade_classes()), 
            buyer_broker_skill.get(), 
            seller_broker_skill.get()
        );
        ag.sort_by_discount();
    });

    // Effect to calculate distance when coordinates change
    Effect::new(move |_| {
        if let (Some(origin), Some(dest)) = (main_world.read().coordinates, dest_coords.get()) {
            let calculated_distance = crate::components::traveller_map::calculate_hex_distance(
                origin.0, origin.1, dest.0, dest.1
            );
            distance.set(calculated_distance);
        }
    });

    // Effect to update passengers when destination world, distance, or steward skill changes
    Effect::new(move |_| {
        if let Some(world) = dest_world.get() {
            if main_world.read().coordinates.is_some() && world.coordinates.is_some() {
                available_passengers.set(Some(trade::available_passengers::AvailablePassengers::generate(
                    main_world.read().get_population(),
                    main_world.read().port,
                    None,
                    main_world.read().tech_level,
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
        <div class="output-region">
            <h2>"Trade Goods for " {move || main_world.read().name.clone()} " [" {move || main_world.read().trade_classes_string()}"]" </h2>

            <div class="destination-world-entry">
                <div>
                    <label for="player-broker-skill">"Player Broker Skill:"</label>
                    <input
                        type="number"
                        id="player-broker-skill"
                        min="0"
                        max="100"
                        value=move || buyer_broker_skill.get()
                        on:change=move |ev| {
                            buyer_broker_skill.set(event_target_value(&ev).parse().unwrap_or(0));
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
                            seller_broker_skill.set(event_target_value(&ev).parse().unwrap_or(0));
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

            <div class="destination-world-entry">
                <WorldSearch 
                    world_entry_label="Destination:".to_string() 
                    world_name=dest_world_name 
                    uwp=dest_uwp 
                    world_coordinates=dest_coords
                />
                <div>
                    <span>{move || {
                        if let Some(world) = dest_world.get() {
                            format!("[{}]", world.trade_classes_string())
                        } else {
                            "".to_string()
                        }
                    }}</span>
                </div>
                <div class="control-container">
                    <label for="distance">"Distance: "</label>
                    <input 
                        class="distance-input control-container"
                        type="number" 
                        id="distance"
                        value=move || distance.get().to_string()
                        on:input=move |ev| {
                            if let Ok(val) = event_target_value(&ev).parse::<i32>() {
                                distance.set(val);
                            }
                        }
                    />
                </div>
            </div>

            <Show when=move || available_passengers.get().is_some()>
                <PassengerView available_passengers=available_passengers />
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
                            }.into_any()
                        } else {
                            view! {
                                <tr>
                                    <th class="table-entry">"Good"</th>
                                    <th class="table-entry">"Quantity"</th>
                                    <th class="table-entry">"Base Price"</th>                                    
                                    <th class="table-entry">"Buy Price"</th>
                                    <th class="table-entry">"Discount"</th>
                                    <Show when=move || show_sell_price.get()>
                                        <th class="table-entry">"Sell Price"</th>
                                        <th class="table-entry">"Discount"</th>
                                    </Show>
                                    <Show when=move || !show_sell_price.get()>
                                        <th class="table-entry">
                                            <button class="sell-price-button" on:click=move |_| show_sell_price.set(true)>"Sell Price"</button>
                                        </th>
                                    </Show>
                                </tr>
                            }.into_any()
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
                                        let sell_discount_percent = (sell_price as f64 / good.base_cost as f64 * 100.0).round() as i32;
                                        view! {
                                            <tr>
                                                <td class="table-entry">{good.name.clone()}</td>
                                                <td class="table-entry">{good.quantity.to_string()}</td>
                                                <td class="table-entry">{good.base_cost.to_string()}</td>
                                                <td class="table-entry">{good.cost.to_string()}</td>
                                                <td class="table-entry">
                                                    {discount_percent.to_string()}"%"
                                                </td>
                                                <Show when=move || show_sell_price.get()>
                                                    <td class="table-entry">{sell_price.to_string()}</td>
                                                    <td class="table-entry">
                                                        {sell_discount_percent.to_string()}"%"
                                                    </td>
                                                </Show>
                                            </tr>
                                        }.into_any()
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
                                                <Show when=move || show_sell_price.get()>
                                                    <td class="table-entry">"-"</td>
                                                    <td class="table-entry">"-"</td>
                                                </Show>
                                            </tr>
                                        }.into_any()
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
fn PassengerView(available_passengers: RwSignal<Option<trade::available_passengers::AvailablePassengers>>) -> impl IntoView {
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
                                view! {
                                    <div class="freight-lot">{lot.size.to_string()}</div>
                                }
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
