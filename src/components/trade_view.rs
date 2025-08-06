use leptos::prelude::*;
use reactive_stores::Store;

use trade::available_goods::AvailableGoodsTable;

#[component]
pub fn TradeView(main_world_name: RwSignal<String>) -> impl IntoView {
    let available_goods = expect_context::<Store<AvailableGoodsTable>>();

    view! {
        <div class="output-region">
            <h2>"Trade Goods for " {move || main_world_name.get()}</h2>

            <table class="trade-table">
                <thead>
                    <tr>
                        <th class="table-entry">"Good"</th>
                        <th class="table-entry">"Quantity"</th>
                        <th class="table-entry">"Price"</th>
                        <th class="table-entry">"Base Price"</th>
                        <th class="table-entry">"Discount"</th>
                    </tr>
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
                                    view! {
                                        <tr>
                                            <td class="table-entry">{good.name.clone()}</td>
                                            <td class="table-entry">{good.quantity.to_string()}</td>
                                            <td class="table-entry">{good.cost.to_string()}</td>
                                            <td class="table-entry">{good.base_cost.to_string()}</td>
                                            <td class="table-entry">
                                                {discount_percent.to_string()}"%"
                                            </td>
                                        </tr>
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
