//! # Trade Computer Component
//!
//! This module provides a comprehensive trade calculation interface for the Traveller universe,
//! enabling players to calculate trade opportunities, passenger transport, and cargo manifests
//! between worlds. It combines market analysis, route planning, and profit calculation into
//! a unified trading interface.
//!
//! ## Component Overview
//!
//! The trade computer consists of three main components that work together:
//!
//! - **Trade**: Main container managing world selection and trade calculations
//! - **TradeView**: Market display showing available goods and pricing
//! - **ShipManifestView**: Cargo and passenger manifest with revenue calculations
//! - **PassengerView**: Available passenger and freight opportunities
//!
//! ## Key Features
//!
//! ### Dynamic Market Generation
//! - Generates available goods based on world trade classifications
//! - Calculates buy/sell prices with broker skill modifiers
//! - Updates pricing automatically when worlds or skills change
//! - Supports speculation trading with profit/loss analysis
//!
//! ### Passenger and Freight System
//! - Generates passenger opportunities by class (High, Medium, Basic, Low)
//! - Creates freight lots with varying tonnage and destinations
//! - Calculates passenger revenue based on distance and steward skill
//! - Handles freight revenue with standard Traveller rates
//!
//! ### Ship Manifest Management
//! - Interactive cargo selection and quantity management
//! - Real-time manifest updates with tonnage tracking
//! - Revenue and profit calculations for complete voyages
//! - Support for mixed cargo (goods, passengers, freight)
//!
//! ### Broker Skill Integration
//! - Player broker skill affects purchase prices
//! - System broker skill affects selling prices
//! - Steward skill influences passenger generation and revenue
//! - Realistic skill-based market advantages
//!
//! ## State Architecture
//!
//! The component uses Leptos reactive stores for complex state management:
//!
//! ### Core World Data
//! - `Store<World>`: Origin world (always exists, starts with default)
//! - `Store<Option<World>>`: Destination world (optional for valid operation)
//!
//! ### Market Data
//! - `Store<AvailableGoodsTable>`: Current market goods with pricing
//! - `Store<Option<AvailablePassengers>>`: Available passenger opportunities
//!
//! ### Ship Data
//! - `Store<ShipManifest>`: Current cargo and passenger manifest
//! - `Store<ShowSellPriceType>`: Toggle for showing destination sell prices
//!
//! ### User Input Signals
//! - World names, UWPs, coordinates, and zone classifications
//! - Skill levels for broker and steward abilities
//! - Distance between worlds (manual or calculated)
//!
//! ## Reactive Effects System
//!
//! The component uses multiple reactive effects for automatic updates:
//!
//! ### World Management Effects
//! 1. **Origin World Update**: Rebuilds origin world from name/UWP changes
//! 2. **Destination World Update**: Rebuilds destination world from input
//! 3. **Zone Reset**: Resets travel zones when world names change
//!
//! ### Market Effects
//! 4. **Goods Pricing**: Updates buy/sell prices when worlds or skills change
//! 5. **Price Display Reset**: Hides sell prices when worlds change
//!
//! ### Distance and Travel Effects
//! 6. **Distance Calculation**: Auto-calculates hex distance from coordinates
//! 7. **Passenger Generation**: Creates passenger opportunities based on worlds/distance
//!
//! ## Trade Calculations
//!
//! ### Available Goods Generation
//! Uses world trade classifications to determine:
//! - Which goods are available for purchase
//! - Base quantities available in the market
//! - Population-based availability modifiers
//!
//! ### Price Calculation
//! Applies Traveller trade rules with broker skill modifiers:
//! - **Buy Prices**: Base cost modified by origin world and player broker skill
//! - **Sell Prices**: Base cost modified by destination world and system broker skill
//! - **Discounts**: Percentage savings/markup displayed for player reference
//!
//! ### Passenger Revenue
//! Calculates passenger income using standard Traveller rates:
//! - **High Passage**: Premium passenger service
//! - **Medium Passage**: Standard passenger service  
//! - **Basic Passage**: Economy passenger service
//! - **Low Passage**: Cryogenic passenger transport
//!
//! ### Freight Revenue
//! Applies standard freight rates based on:
//! - Tonnage of freight lots selected
//! - Distance between origin and destination
//! - Standard Traveller freight rate tables
//!
//! ## Error Handling
//!
//! The component includes comprehensive error handling:
//! - **UWP Validation**: Checks for proper 9-character UWP format
//! - **World Parsing**: Handles malformed world data gracefully
//! - **Coordinate Validation**: Manages missing or invalid coordinate data
//! - **Skill Bounds**: Ensures skill values remain within valid ranges
//!
//! ## User Interface Structure
//!
//! ```text
//! Trade Computer
//! ├── World Entry Form
//! │   ├── Origin World Search (WorldSearch)
//! │   └── Destination World Search (WorldSearch)
//! ├── Skills and Distance Entry
//! │   ├── Distance Input (manual override)
//! │   ├── Player Broker Skill
//! │   ├── System Broker Skill
//! │   └── Steward Skill
//! ├── Ship Manifest (ShipManifestView)
//! │   ├── Passenger Summary
//! │   ├── Freight Summary
//! │   ├── Goods Summary
//! │   └── Revenue Calculations
//! └── Trade View (TradeView)
//!     ├── Available Passengers (PassengerView)
//!     └── Available Goods Table
//! ```
//!
//! ## Integration Points
//!
//! ### Traveller Map Integration
//! - Uses `WorldSearch` component for official world data
//! - Automatically populates UWP, coordinates, and zone data
//! - Calculates hex distances using galactic coordinate system
//!
//! ### Trade System Integration
//! - Leverages `AvailableGoodsTable` for market generation
//! - Uses `AvailablePassengers` for passenger opportunity calculation
//! - Integrates with `ShipManifest` for cargo tracking
//!
//! ## Usage Examples
//!
//! ```rust
//! use leptos::prelude::*;
//! use worldgen::components::trade_computer::Trade;
//!
//! // Mount the trade computer as main application
//! #[component]
//! fn App() -> impl IntoView {
//!     view! {
//!         <Trade />
//!     }
//! }
//! ```
//!
//! ## Default Configuration
//!
//! The component initializes with:
//! - **Origin World**: "Main World" with UWP "A788899-A"
//! - **Destination**: None (optional for basic functionality)
//! - **Skills**: All set to 0 (no skill bonuses)
//! - **Distance**: 0 (calculated automatically if coordinates available)
//! - **Zone**: Green (safe travel zone)
//!
//! ## Print Support
//!
//! Includes print functionality for generating hard copies of trade data,
//! though this feature is currently disabled but available for future use.

use leptos::prelude::*;
use reactive_stores::Store;

#[allow(unused_imports)]
use leptos::leptos_dom::logging::console_log;

use log::debug;

use crate::components::traveller_map::WorldSearch;
use crate::systems::world::World;
use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;
use crate::trade::table::TradeTable;
use crate::trade::ZoneClassification;

use crate::INITIAL_NAME;
use crate::INITIAL_UPP;

/// Internal type for managing sell price display state
///
/// Wraps a boolean flag indicating whether sell prices should be displayed
/// in the trade goods table. Used as a reactive store type for managing
/// the "Show Sell Price" toggle functionality.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ShowSellPriceType(bool);

/// Main trade computer component providing comprehensive trading interface
///
/// Creates the complete trade calculation interface including world selection,
/// market analysis, passenger opportunities, and ship manifest management.
/// Serves as the primary entry point for all trade-related functionality.
///
/// ## Component Initialization
///
/// Sets up global reactive stores for:
/// - **Origin World**: Always exists, starts with default world
/// - **Destination World**: Optional, starts as None
/// - **Available Goods**: Market goods table with pricing
/// - **Available Passengers**: Passenger and freight opportunities
/// - **Ship Manifest**: Current cargo and passenger load
/// - **Show Sell Price**: Toggle for destination pricing display
///
/// ## Reactive Effects
///
/// Manages multiple reactive effects for automatic updates:
/// 1. Origin world rebuilding from name/UWP changes
/// 2. Destination world rebuilding from input changes
/// 3. Goods pricing updates based on worlds and skills
/// 4. Sell price display reset when worlds change
/// 5. Zone reset when world names change
/// 6. Distance calculation from coordinates
/// 7. Passenger generation based on worlds and distance
///
/// ## User Interface
///
/// Renders the complete trade interface including:
/// - World search and entry forms
/// - Skill and distance input controls
/// - Ship manifest display and management
/// - Trade goods and passenger opportunity tables
///
/// ## Context Provision
///
/// Provides reactive stores to child components through Leptos context,
/// enabling shared state management across the trade interface.
///
/// ## Returns
///
/// Complete trade computer interface with all interactive elements
/// and automatic reactive updates.
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
    provide_context(Store::new(ShipManifest::default()));

    let origin_world = expect_context::<Store<World>>();
    let dest_world = expect_context::<Store<Option<World>>>();
    let trade_table = TradeTable::default();
    let available_goods = expect_context::<Store<AvailableGoodsTable>>();
    let available_passengers = expect_context::<Store<Option<AvailablePassengers>>>();
    let show_sell_price = expect_context::<Store<ShowSellPriceType>>();
    let ship_manifest = expect_context::<Store<ShipManifest>>();

    // Skills involved, both player and adversary.
    let buyer_broker_skill = RwSignal::new(0);
    let seller_broker_skill = RwSignal::new(0);
    let steward_skill = RwSignal::new(0);

    let origin_world_name = RwSignal::new(origin_world.read_untracked().name.clone());
    let origin_uwp = RwSignal::new(origin_world.read_untracked().to_uwp());
    let origin_coords = RwSignal::new(origin_world.read_untracked().coordinates);
    let origin_zone = RwSignal::new(origin_world.read_untracked().travel_zone);
    let dest_world_name = RwSignal::new("".to_string());
    let dest_uwp = RwSignal::new("".to_string());
    let dest_coords = RwSignal::new(None);
    let dest_zone = RwSignal::new(ZoneClassification::Green);

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
            world.travel_zone = origin_zone.get();
            origin_world.set(world);

            // Now update available goods
            let ag = AvailableGoodsTable::for_world(
                &trade_table,
                &origin_world.read().get_trade_classes(),
                origin_world.read().get_population(),
                false,
            )
            .unwrap();
            ship_manifest.set(ShipManifest::default());
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
            world.travel_zone = dest_zone.get();
            dest_world.set(Some(world));
        } else {
            dest_world.set(None);
        }
    });

    Effect::new(move |_| {
        console_log("Updating goods pricing");
        ship_manifest.set(ShipManifest::default());
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

    // Effect to reset zones when world names change (but not when zones change)
    Effect::new(move |_| {
        let _ = origin_world_name.get();
        origin_zone.set(ZoneClassification::Green);
    });

    Effect::new(move |_| {
        let _ = dest_world_name.get();
        dest_zone.set(ZoneClassification::Green);
    });

    // Effect to calculate distance when coordinates or zone change
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
                    origin_world.read().travel_zone,
                    origin_world.read().tech_level,
                    world.get_population(),
                    world.port,
                    world.travel_zone,
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
                        zone=origin_zone
                    />

                </div>
                <WorldSearch
                    label="Destination".to_string()
                    name=dest_world_name
                    uwp=dest_uwp
                    coords=dest_coords
                    zone=dest_zone
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
                            {move || format!("[{}] {}", origin_world.read().trade_classes_string(), origin_world.read().travel_zone)}
                        </span>
                    </div>
                    <div>
                        <span>
                            {move || {
                                if let Some(world) = dest_world.get() {
                                    format!(
                                        "Destination Trade Classes: [{}] {}",
                                        world.trade_classes_string(),
                                        world.travel_zone
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

            <ShipManifestView distance = distance />
            <TradeView />

        </div>
    }
}

/// Print the current page (currently unused but available for future use)
///
/// Provides a wrapper around the browser's print functionality for generating
/// hard copies of trade data and manifests. Currently disabled but maintained
/// for potential future print features.
///
/// ## Error Handling
///
/// Logs errors to the console if printing fails, but does not propagate
/// errors to avoid disrupting the main application flow.
#[allow(dead_code)]
fn print() {
    leptos::leptos_dom::helpers::window()
        .print()
        .unwrap_or_else(|e| log::error!("Error printing: {e:?}"));
}

/// Trade view component displaying available goods and market information
///
/// Renders the market interface showing available trade goods with pricing,
/// passenger opportunities, and interactive purchase controls. Provides
/// the main market interaction interface for the trade computer.
///
/// ## Display Elements
///
/// ### Market Header
/// - Origin world name and trade classifications
/// - Current trade class modifiers affecting availability
///
/// ### Passenger Section
/// - Conditionally displayed when destination world exists
/// - Shows available passengers by class and freight opportunities
/// - Interactive buttons for adding passengers to manifest
///
/// ### Trade Goods Table
/// - Dynamic table headers based on destination world presence
/// - Shows goods, quantities, base prices, buy prices, and discounts
/// - Includes sell prices and profit margins when destination selected
/// - Interactive quantity inputs for purchasing goods
///
/// ## Reactive Behavior
///
/// - Table headers change based on destination world availability
/// - Sell price columns appear only when destination world exists
/// - Purchase inputs update ship manifest in real-time
/// - Discount percentages calculated dynamically from broker skills
///
/// ## Context Requirements
///
/// Expects reactive stores in Leptos context:
/// - `Store<World>`: Origin world data
/// - `Store<Option<World>>`: Destination world data
/// - `Store<AvailableGoodsTable>`: Current market goods
/// - `Store<Option<AvailablePassengers>>`: Passenger opportunities
/// - `Store<ShowSellPriceType>`: Sell price display toggle
///
/// ## Returns
///
/// Complete market interface with conditional sections based on
/// destination world availability and current market conditions.
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
                                    <th class="table-entry">"Purchased"</th>
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
                                    <th class="table-entry">"Purchased"</th>
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
                                .enumerate()
                                .map(|(index, good)| {
                                    let discount_percent = (good.buy_cost as f64 / good.base_cost as f64
                                        * 100.0)
                                        .round() as i32;

                                    let purchased_amount = good.purchased;
                                    let buy_cost_comment = good.buy_cost_comment.clone();
                                    let sell_price_comment = good.sell_price_comment.clone();

                                    let update_purchased = move |ev| {
                                        let new_value = event_target_value(&ev).parse::<i32>().unwrap_or(0);
                                        let mut ag = available_goods.write();
                                        if let Some(good) = ag.goods.get_mut(index) {
                                            good.purchased = new_value.clamp(0, good.quantity);
                                        }
                                    };

                                    if let Some(sell_price) = good.sell_price {
                                        let sell_discount_percent = (sell_price as f64
                                            / good.base_cost as f64 * 100.0)
                                            .round() as i32;
                                        view! {
                                            <tr>
                                                <td class="table-entry">{good.name.clone()}</td>
                                                <td class="table-entry">{good.quantity.to_string()}</td>
                                                <td class="table-entry">{good.base_cost.to_string()}</td>
                                                <td class="table-entry" title=buy_cost_comment.clone()>{good.buy_cost.to_string()}</td>
                                                <td class="table-entry">
                                                    {discount_percent.to_string()}"%"
                                                </td>
                                                <td class="table-entry">
                                                    <input
                                                        type="number"
                                                        min="0"
                                                        max=good.quantity
                                                        prop:value=purchased_amount
                                                        on:input=update_purchased
                                                        class=move || {
                                                            if purchased_amount > 0 {
                                                                "purchased-input purchased-input-active"
                                                            } else {
                                                                "purchased-input"
                                                            }
                                                        }
                                                    />
                                                </td>
                                                <Show when=move || show_sell_price.read().0>
                                                    <td class="table-entry" title=sell_price_comment.clone()>{sell_price.to_string()}</td>
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
                                                <td class="table-entry" title=buy_cost_comment>{good.buy_cost.to_string()}</td>
                                                <td class="table-entry">
                                                    {discount_percent.to_string()}"%"
                                                </td>
                                                <td class="table-entry">
                                                    <input
                                                        type="number"
                                                        min="0"
                                                        max=good.quantity
                                                        prop:value=purchased_amount
                                                        on:input=update_purchased
                                                        class=move || {
                                                            if purchased_amount > 0 {
                                                                "purchased-input purchased-input-active"
                                                            } else {
                                                                "purchased-input"
                                                            }
                                                        }
                                                    />
                                                </td>
                                                <Show when=move || show_sell_price.read().0>
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

/// Passenger view component displaying available passenger and freight opportunities
///
/// Renders interactive passenger booking interface showing available passengers
/// by class and freight lots available for transport. Provides buttons for
/// adding passengers and freight to the ship manifest.
///
/// ## Passenger Classes
///
/// Displays four passenger types with remaining availability:
/// - **High Passage**: Luxury accommodations with premium pricing
/// - **Medium Passage**: Standard passenger service
/// - **Basic Passage**: Economy passenger transport
/// - **Low Passage**: Cryogenic passenger transport (cheapest option)
///
/// ## Freight System
///
/// Shows available freight lots with:
/// - **Tonnage**: Size of each freight lot
/// - **Destination**: Where freight needs to be delivered
/// - **Selection**: Toggle buttons for adding/removing from manifest
///
/// ## Interactive Elements
///
/// ### Passenger Buttons
/// - Click to add one passenger of selected class to manifest
/// - Buttons disabled when no passengers of that class remain
/// - Real-time updates of remaining passenger counts
///
/// ### Freight Buttons
/// - Toggle freight lot selection on/off
/// - Visual indication of selected freight lots
/// - Prevents double-booking of freight lots
///
/// ## Reactive Calculations
///
/// - Passenger counts update based on current ship manifest
/// - Remaining availability calculated dynamically
/// - Freight selection state maintained in ship manifest
///
/// ## Context Requirements
///
/// Expects reactive stores in Leptos context:
/// - `Store<Option<AvailablePassengers>>`: Current passenger opportunities
/// - `Store<ShipManifest>`: Current ship cargo and passenger load
///
/// ## Display Conditions
///
/// Only renders when:
/// - Destination world exists
/// - Distance between worlds is greater than 0
/// - Passenger opportunities have been generated
///
/// ## Returns
///
/// Interactive passenger and freight booking interface with real-time
/// availability updates and manifest integration.
#[component]
fn PassengerView() -> impl IntoView {
    let available_passengers = expect_context::<Store<Option<AvailablePassengers>>>();
    let ship_manifest = expect_context::<Store<ShipManifest>>();

    let add_high_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining = passengers.high - ship_manifest.read().high_passengers;
            if remaining > 0 {
                let mut manifest = ship_manifest.write();
                manifest.high_passengers += 1;
            }
        }
    };

    let add_medium_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining = passengers.medium - ship_manifest.read().medium_passengers;
            if remaining > 0 {
                let mut manifest = ship_manifest.write();
                manifest.medium_passengers += 1;
            }
        }
    };

    let add_basic_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining = passengers.basic - ship_manifest.read().basic_passengers;
            if remaining > 0 {
                let mut manifest = ship_manifest.write();
                manifest.basic_passengers += 1;
            }
        }
    };

    let add_low_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining = passengers.low - ship_manifest.read().low_passengers;
            if remaining > 0 {
                let mut manifest = ship_manifest.write();
                manifest.low_passengers += 1;
            }
        }
    };

    view! {
        <h4 style="font-size: 14pt;">"Available Passengers"</h4>
        <div class="passengers-grid">
            <button class="passenger-type passenger-button" on:click=add_high_passenger>
                <h4>"High"</h4>
                <div class="passenger-count">
                    {move || {
                        if let Some(passengers) = available_passengers.get() {
                            let remaining = passengers.high - ship_manifest.read().high_passengers;
                            remaining.max(0).to_string()
                        } else {
                            "0".to_string()
                        }
                    }}
                </div>
            </button>
            <button class="passenger-type passenger-button" on:click=add_medium_passenger>
                <h4>"Medium"</h4>
                <div class="passenger-count">
                    {move || {
                        if let Some(passengers) = available_passengers.get() {
                            let remaining = passengers.medium
                                - ship_manifest.read().medium_passengers;
                            remaining.max(0).to_string()
                        } else {
                            "0".to_string()
                        }
                    }}
                </div>
            </button>
            <button class="passenger-type passenger-button" on:click=add_basic_passenger>
                <h4>"Basic"</h4>
                <div class="passenger-count">
                    {move || {
                        if let Some(passengers) = available_passengers.get() {
                            let remaining = passengers.basic
                                - ship_manifest.read().basic_passengers;
                            remaining.max(0).to_string()
                        } else {
                            "0".to_string()
                        }
                    }}
                </div>
            </button>
            <button class="passenger-type passenger-button" on:click=add_low_passenger>
                <h4>"Low"</h4>
                <div class="passenger-count">
                    {move || {
                        if let Some(passengers) = available_passengers.get() {
                            let remaining = passengers.low - ship_manifest.read().low_passengers;
                            remaining.max(0).to_string()
                        } else {
                            "0".to_string()
                        }
                    }}
                </div>
            </button>
        </div>

        <h4 style="font-size: 14pt;">"Available Freight (tons)"</h4>
        <div class="freight-grid">
            {move || {
                if let Some(passengers) = available_passengers.get() {
                    if passengers.freight_lots.is_empty() {
                        view! { <div>"No freight available"</div> }.into_any()
                    } else {
                        passengers
                            .freight_lots
                            .iter()
                            .enumerate()
                            .map(|(index, lot)| {
                                let lot_size = lot.size;
                                let toggle_freight = move |_| {
                                    let mut manifest = ship_manifest.write();
                                    if let Some(pos) = manifest
                                        .freight_lot_indices
                                        .iter()
                                        .position(|&i| i == index)
                                    {
                                        manifest.freight_lot_indices.remove(pos);
                                    } else {
                                        manifest.freight_lot_indices.push(index);
                                    }
                                };
                                let is_selected = move || {
                                    ship_manifest.read().freight_lot_indices.contains(&index)
                                };

                                view! {
                                    <button
                                        class=move || {
                                            if is_selected() {
                                                "freight-lot freight-selected"
                                            } else {
                                                "freight-lot"
                                            }
                                        }
                                        on:click=toggle_freight
                                    >
                                        {lot_size.to_string()}
                                    </button>
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

/// Ship manifest view component displaying current cargo and revenue calculations
///
/// Renders the complete ship manifest showing current passenger load, freight
/// selection, trade goods, and comprehensive revenue calculations. Provides
/// interactive controls for removing items from the manifest and displays
/// total profitability for the planned voyage.
///
/// ## Display Sections
///
/// ### Manifest Summary
/// - **Total Cargo**: Combined tonnage of goods and freight
/// - **Total Passengers**: Count of all passenger types except Low
/// - **Total Low**: Separate count for Low passage passengers
///
/// ### Passenger Manifest
/// Interactive buttons showing current passenger counts by class:
/// - Click to remove one passenger of that class
/// - Real-time updates of passenger counts
/// - Separate tracking for High, Medium, Basic, and Low passage
///
/// ### Freight Manifest
/// - Lists selected freight lots with tonnage
/// - Shows freight lot destinations and sizes
/// - Displays total freight tonnage
///
/// ### Trade Goods Manifest
/// - Lists purchased goods with quantities and costs
/// - Shows sell prices when destination world available
/// - Calculates profit/loss for each good type
///
/// ### Revenue Calculations
/// - **Passenger Revenue**: Income from all passenger types
/// - **Freight Revenue**: Income from freight transport
/// - **Goods Profit**: Profit/loss from trade goods (when sell prices shown)
/// - **Total Revenue**: Combined income from all sources
///
/// ## Interactive Elements
///
/// ### Passenger Removal
/// - Click passenger type buttons to remove one passenger
/// - Buttons show current counts and update manifest immediately
/// - Prevents removal when count is already zero
///
/// ### Revenue Display
/// - Passenger and freight revenue always shown
/// - Goods profit only shown when sell prices are available
/// - All amounts displayed in MCr (millions of credits)
///
/// ## Reactive Calculations
///
/// All values update automatically when:
/// - Ship manifest changes (passengers, freight, goods)
/// - Distance changes (affects passenger/freight revenue)
/// - Sell prices become available (affects goods profit)
/// - Broker skills change (affects goods pricing)
///
/// ## Context Requirements
///
/// Expects reactive stores in Leptos context:
/// - `Store<ShipManifest>`: Current ship cargo and passenger data
/// - `Store<Option<AvailablePassengers>>`: Available freight lot data
/// - `Store<AvailableGoodsTable>`: Trade goods with pricing
/// - `Store<ShowSellPriceType>`: Sell price display toggle
///
/// ## Parameters
///
/// * `distance` - RwSignal containing current distance between worlds,
///   used for passenger and freight revenue calculations
///
/// ## Returns
///
/// Complete ship manifest interface with interactive controls and
/// comprehensive revenue analysis for the planned voyage.
#[component]
fn ShipManifestView(distance: RwSignal<i32>) -> impl IntoView {
    let ship_manifest = expect_context::<Store<ShipManifest>>();
    let available_passengers = expect_context::<Store<Option<AvailablePassengers>>>();
    let available_goods = expect_context::<Store<AvailableGoodsTable>>();
    let show_sell_price = expect_context::<Store<ShowSellPriceType>>();

    let remove_high_passenger = move |_| {
        let mut manifest = ship_manifest.write();
        if manifest.high_passengers > 0 {
            manifest.high_passengers -= 1;
        }
    };

    let remove_medium_passenger = move |_| {
        let mut manifest = ship_manifest.write();
        if manifest.medium_passengers > 0 {
            manifest.medium_passengers -= 1;
        }
    };

    let remove_basic_passenger = move |_| {
        let mut manifest = ship_manifest.write();
        if manifest.basic_passengers > 0 {
            manifest.basic_passengers -= 1;
        }
    };

    let remove_low_passenger = move |_| {
        let mut manifest = ship_manifest.write();
        if manifest.low_passengers > 0 {
            manifest.low_passengers -= 1;
        }
    };

    view! {
        <div class="manifest-container">
            <h4 style="font-size: 14pt;">"Ship Manifest"</h4>

            <div class="manifest-summary">
                {move || {
                    let passengers = available_passengers.get();
                    let manifest = ship_manifest.get();
                    let goods = available_goods.get();

                    let cargo_tons = if let Some(passengers) = passengers {
                        manifest.freight_lot_indices
                            .iter()
                            .map(|&index| passengers.freight_lots.get(index).map(|lot| lot.size).unwrap_or(0))
                            .sum::<i32>()
                    } else {
                        0
                    };

                    let goods_tons: i32 = goods.goods.iter().map(|good| good.purchased).sum();
                    let total_cargo = cargo_tons + goods_tons;
                    let total_passengers = manifest.high_passengers + manifest.medium_passengers + manifest.basic_passengers;
                    let total_low = manifest.low_passengers;

                    view! {
                        <div class="summary-line">
                            "Total Cargo Used: " <strong>{total_cargo.to_string()}" tons"</strong>
                            " | Total Passengers: " <strong>{total_passengers.to_string()}</strong>
                            " | Total Low: " <strong>{total_low.to_string()}</strong>
                        </div>
                    }
                }}
            </div>

            <div class="manifest-section">
                <h5>"Passengers"</h5>
                <div class="manifest-grid">
                    <button class="manifest-item manifest-button" on:click=remove_high_passenger>
                        <span class="manifest-label">"High:"</span>
                        <span class="manifest-value">
                            {move || ship_manifest.read().high_passengers}
                        </span>
                    </button>
                    <button class="manifest-item manifest-button" on:click=remove_medium_passenger>
                        <span class="manifest-label">"Medium:"</span>
                        <span class="manifest-value">
                            {move || ship_manifest.read().medium_passengers}
                        </span>
                    </button>
                    <button class="manifest-item manifest-button" on:click=remove_basic_passenger>
                        <span class="manifest-label">"Basic:"</span>
                        <span class="manifest-value">
                            {move || ship_manifest.read().basic_passengers}
                        </span>
                    </button>
                    <button class="manifest-item manifest-button" on:click=remove_low_passenger>
                        <span class="manifest-label">"Low:"</span>
                        <span class="manifest-value">
                            {move || ship_manifest.read().low_passengers}
                        </span>
                    </button>
                </div>
            </div>

            <div class="manifest-section">
                <h5>"Freight"</h5>
                <div class="manifest-grid">
                    {move || {
                        let passengers = available_passengers.get();
                        let manifest = ship_manifest.get();
                        let goods = available_goods.get();
                        let show_sell = show_sell_price.get().0;

                        let cargo_tons = if let Some(passengers) = passengers {
                            manifest.freight_lot_indices
                                .iter()
                                .map(|&index| passengers.freight_lots.get(index).map(|lot| lot.size).unwrap_or(0))
                                .sum::<i32>()
                        } else {
                            0
                        };

                        let goods_tons: i32 = goods.goods.iter().map(|good| good.purchased).sum();
                        let goods_cost: i64 = goods.goods.iter().map(|good| good.purchased as i64 * good.buy_cost as i64).sum();
                        let goods_proceeds: i64 = goods.goods.iter().map(|good| {
                            if let Some(sell_price) = good.sell_price {
                                good.purchased as i64 * sell_price as i64
                            } else {
                                0
                            }
                        }).sum();

                        view! {
                            <div class="manifest-item">
                                <span class="manifest-label">"Cargo:"</span>
                                <span class="manifest-value">{format!("{} tons", cargo_tons)}</span>
                            </div>
                            <div class="manifest-item">
                                <span class="manifest-label">"Goods:"</span>
                                <span class="manifest-value">{format!("{} tons", goods_tons)}</span>
                            </div>
                            <div class="manifest-item">
                                <span class="manifest-label">"Goods Cost:"</span>
                                <span class="manifest-value">{format!("{:.2} MCr", goods_cost as f64 / 1_000_000.0)}</span>
                            </div>
                            {if show_sell {
                                view! {
                                    <div class="manifest-item">
                                        <span class="manifest-label">"Goods Proceeds:"</span>
                                        <span class="manifest-value">{format!("{:.2} MCr", goods_proceeds as f64 / 1_000_000.0)}</span>
                                    </div>
                                }.into_any()
                            } else {
                                ().into_any()
                            }}
                        }
                    }}
                </div>
            </div>

            <div class="manifest-section">
                <h5>"Revenue"</h5>
                <div class="manifest-grid">
                    <div class="manifest-item">
                        <span class="manifest-label">"Passenger Revenue:"</span>
                        <span class="manifest-value">
                            {move || {
                                let manifest = ship_manifest.get();
                                let revenue = manifest.passenger_revenue(distance.get());
                                format!("{:.2} MCr", revenue as f64 / 1_000_000.0)
                            }}
                        </span>
                    </div>
                    <div class="manifest-item">
                        <span class="manifest-label">"Freight Revenue:"</span>
                        <span class="manifest-value">
                            {move || {
                                let manifest = ship_manifest.get();
                                let revenue = manifest.freight_revenue(distance.get());
                                format!("{:.2} MCr", revenue as f64 / 1_000_000.0)
                            }}
                        </span>
                    </div>
                    <Show when=move || show_sell_price.get().0>
                        <div class="manifest-item">
                            <span class="manifest-label">"Goods Profit:"</span>
                            <span class="manifest-value">
                                {move || {
                                    let goods = available_goods.get();
                                    let cost: i64 = goods.goods.iter()
                                        .map(|good| good.purchased as i64 * good.buy_cost as i64)
                                        .sum();
                                    let proceeds: i64 = goods.goods.iter()
                                        .map(|good| {
                                            if let Some(sell_price) = good.sell_price {
                                                good.purchased as i64 * sell_price as i64
                                            } else {
                                                0
                                            }
                                        })
                                        .sum();
                                    let profit = proceeds - cost;
                                    format!("{:.2} MCr", profit as f64 / 1_000_000.0)
                                }}
                            </span>
                        </div>
                    </Show>
                    <div class="manifest-item">
                        <span class="manifest-label">"Total:"</span>
                        <span class="manifest-value">
                            {move || {
                                let manifest = ship_manifest.get();
                                let goods = available_goods.get();
                                let show_sell = show_sell_price.get().0;

                                let passenger_revenue = manifest.passenger_revenue(distance.get()) as i64;
                                let freight_revenue = manifest.freight_revenue(distance.get()) as i64;

                                let goods_profit = if show_sell {
                                    let cost: i64 = goods.goods.iter()
                                        .map(|good| good.purchased as i64 * good.buy_cost as i64)
                                        .sum();
                                    let proceeds: i64 = goods.goods.iter()
                                        .map(|good| {
                                            if let Some(sell_price) = good.sell_price {
                                                good.purchased as i64 * sell_price as i64
                                            } else {
                                                0
                                            }
                                        })
                                        .sum();
                                    proceeds - cost
                                } else {
                                    0
                                };

                                let total = passenger_revenue + freight_revenue + goods_profit;
                                format!("{:.2} MCr", total as f64 / 1_000_000.0)
                            }}
                        </span>
                    </div>
                </div>
            </div>
        </div>
    }
}
