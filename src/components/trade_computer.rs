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
use std::rc::Rc;

use leptos::prelude::*;
#[allow(unused_imports)]
use log::{debug, error, info};

use crate::comms::TradeState;
use crate::comms::client::{Client, TradeSignals};
use crate::components::traveller_map::WorldSearch;
use crate::systems::world::World;

use crate::trade::available_goods::{AvailableGoodsTable, Good};

use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;
use crate::trade::table::TradeTable;
use crate::trade::{TradeClass, ZoneClassification};

use crate::util::Credits;

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
pub fn Trade(
    /// Optional WebSocket client for syncing state with server
    #[prop(optional)]
    client: Option<Rc<Client>>,
) -> impl IntoView {
    // The origin world is generated by the server and sent back to us
    // It starts as None until the server sends it
    let (origin_world, write_origin_world) = signal::<Option<World>>(None);

    // The destination world doesn't always exist - there is valid function w/o it.
    // So it's an Option and starts as value None.
    let (dest_world, write_dest_world) = signal::<Option<World>>(None);

    // Available goods table
    let (available_goods, write_available_goods) = signal(AvailableGoodsTable::default());

    // Available passengers (optional)
    let (available_passengers, write_available_passengers) =
        signal::<Option<AvailablePassengers>>(None);

    // Ship manifest
    let (ship_manifest, write_ship_manifest) = signal(ShipManifest::default());

    // Skills involved, both player and adversary.
    let (buyer_broker_skill, write_buyer_broker_skill) = signal::<i16>(0);
    let (seller_broker_skill, write_seller_broker_skill) = signal::<i16>(0);
    let (steward_skill, write_steward_skill) = signal::<i16>(0);

    // Toggle for including illegal goods in market generation
    let (illegal_goods, write_illegal_goods) = signal::<bool>(false);

    // Dialog state for manually adding goods to manifest
    let show_add_manual = RwSignal::new(false);

    let origin_world_name = RwSignal::new(String::new());
    let origin_uwp = RwSignal::new(String::new());

    let dest_world_name = RwSignal::new(
        dest_world
            .get_untracked()
            .as_ref()
            .map(|w| w.name.clone())
            .unwrap_or_default(),
    );
    let dest_uwp = RwSignal::new(
        dest_world
            .get_untracked()
            .as_ref()
            .map(|w| w.to_uwp())
            .unwrap_or_default(),
    );

    // World coordinates and zone signals (needed by server for distance and World generation)
    let origin_coords = RwSignal::new(
        origin_world
            .get_untracked()
            .as_ref()
            .and_then(|w| w.coordinates),
    );
    let origin_zone = RwSignal::new(
        origin_world
            .get_untracked()
            .as_ref()
            .map(|w| w.travel_zone)
            .unwrap_or(ZoneClassification::Green),
    );
    let dest_coords = RwSignal::new(
        dest_world
            .get_untracked()
            .as_ref()
            .and_then(|w| w.coordinates),
    );
    let dest_zone = RwSignal::new(
        dest_world
            .get_untracked()
            .as_ref()
            .map(|w| w.travel_zone)
            .unwrap_or(ZoneClassification::Green),
    );

    // Register signals with the client if provided
    if let Some(ref client) = client {
        let signals = TradeSignals {
            origin_world_name: origin_world_name.write_only(),
            origin_uwp: origin_uwp.write_only(),
            origin_coords: origin_coords.write_only(),
            origin_zone: origin_zone.write_only(),
            origin_world: write_origin_world,
            dest_world_name: dest_world_name.write_only(),
            dest_uwp: dest_uwp.write_only(),
            dest_coords: dest_coords.write_only(),
            dest_zone: dest_zone.write_only(),
            dest_world: write_dest_world,
            available_goods: write_available_goods,
            available_passengers: write_available_passengers,
            ship_manifest: write_ship_manifest,
            buyer_broker_skill: write_buyer_broker_skill,
            seller_broker_skill: write_seller_broker_skill,
            steward_skill: write_steward_skill,
            illegal_goods: write_illegal_goods,
        };
        client.register_signals(signals);

        // Set up Effect to send state to server on any signal change
        let client_for_effect = client.clone();
        Effect::new(move |_| {
            // Read all signals to track them (this registers them as dependencies)
            let current_origin_name = origin_world_name.get();
            let current_origin_uwp = origin_uwp.get();
            let current_origin_coords = origin_coords.get();
            let current_origin_zone = origin_zone.get();
            let current_dest_name = dest_world_name.get();
            let current_dest_uwp = dest_uwp.get();
            let current_dest_coords = dest_coords.get();
            let current_dest_zone = dest_zone.get();
            let current_goods = available_goods.get();
            let current_passengers = available_passengers.get();
            let current_manifest = ship_manifest.get();
            let current_buyer_skill = buyer_broker_skill.get();
            let current_seller_skill = seller_broker_skill.get();
            let current_steward = steward_skill.get();
            let current_illegal = illegal_goods.get();

            // Only send if the client is connected
            if !client_for_effect.is_connected() {
                debug!("Skipping send - client not connected");
                return;
            }

            // Don't send until we've received the initial state from the server
            // This prevents sending default values before the server has a chance to sync
            if !client_for_effect.has_received_initial_state() {
                debug!("Skipping send - waiting for initial state from server");
                return;
            }

            // Build the TradeState from name/uwp/coords/zone
            // Note: We don't send origin_world and dest_world to the server
            // The server generates them and sends them back to us
            let state = TradeState {
                version: 1, // Version for state compatibility
                origin_world_name: current_origin_name,
                origin_uwp: current_origin_uwp,
                origin_coords: current_origin_coords,
                origin_zone: current_origin_zone,
                origin_world: None, // Server generates and sends this back
                dest_world_name: current_dest_name,
                dest_uwp: current_dest_uwp,
                dest_coords: current_dest_coords,
                dest_zone: current_dest_zone,
                dest_world: None, // Server generates and sends this back
                available_goods: current_goods,
                available_passengers: current_passengers,
                ship_manifest: current_manifest,
                buyer_broker_skill: current_buyer_skill,
                seller_broker_skill: current_seller_skill,
                steward_skill: current_steward,
                illegal_goods: current_illegal,
            };

            // Skip sending if this is just an echo of what we received from server
            // This prevents infinite loops while still allowing user changes during
            // server updates to be sent
            if client_for_effect.is_echo_of_received(&state) {
                debug!("Skipping send - state matches last received from server");
                // Don't clear the stored state - keep it so we can continue detecting echoes
                return;
            }

            info!("Sending trade state update to server");
            client_for_effect.send_state(&state);
            // Clear the stored state after sending so we don't echo our own message back
            client_for_effect.clear_last_received();
        });
    }

    // Distance between worlds (calculated from coordinates)
    let distance = RwSignal::new(0);

    let dest_to_origin = move || {
        origin_world_name.set(dest_world_name.get());
        origin_uwp.set(dest_uwp.get());
        origin_coords.set(dest_coords.get());
        origin_zone.set(dest_zone.get());
        dest_world_name.set("".to_string());
        dest_uwp.set("".to_string());
        dest_coords.set(None);
        dest_zone.set(ZoneClassification::Green);
    };

    // Effect to recalculate distance whenever coordinates change
    // Distance is derived from coordinates, so we calculate it whenever they change
    // The user can still manually override the distance in the UI
    Effect::new(move |_| {
        if let (Some(origin), Some(dest)) = (origin_coords.get(), dest_coords.get()) {
            let calculated_distance =
                crate::util::calculate_hex_distance(origin.0, origin.1, dest.0, dest.1);
            distance.set(calculated_distance);
        } else {
            distance.set(0);
        }
    });

    view! {
        <div class:App>
            <h1 class="d-print-none">Trade Computer</h1>
            <div class="key-region world-entry-form">
                <div>
                    <WorldSearch
                        label="Current".to_string()
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
                <div style="display: flex; align-items: center; padding: 10px;">
                    <button
                        class="blue-button"
                        on:click=move |_| {
                            // Send regenerate command to server to re-roll prices and passengers
                            if let Some(ref c) = client {
                                c.send_regenerate();
                            }
                        }
                    >
                        "Regenerate"
                    </button>
                </div>
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
                                    distance.set(val);
                                }
                            }
                        />
                    </div>
                    <div>
                        <span>
                            "Current Classes: "
                            {move || {
                                match origin_world.read().as_ref() {
                                    Some(world) => format!(
                                        "[{}] {}",
                                        world.trade_classes_string(),
                                        world.travel_zone,
                                    ),
                                    None => String::new(),
                                }
                            }}
                        </span>
                    </div>
                    <div>
                        <span>
                            {move || {
                                if let Some(world) = dest_world.get() {
                                    format!(
                                        "Destination Trade Classes: [{}] {}",
                                        world.trade_classes_string(),
                                        world.travel_zone,
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
                                write_buyer_broker_skill
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
                                write_seller_broker_skill
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
                                write_steward_skill
                                    .set(event_target_value(&ev).parse().unwrap_or(0));
                            }
                        />
                    </div>
                </div>
                <div class="skill-entry">
                    <div>
                        <label for="include-illegal">"Include Illegal Goods:"</label>
                        <input
                            id="include-illegal"
                            type="checkbox"
                            prop:checked=move || illegal_goods.get()
                            on:change=move |ev| {
                                let checked = event_target_checked(&ev);
                                write_illegal_goods.set(checked);
                                // NOTE: Trade table regeneration is now handled by the server
                                // when it receives the updated illegal_goods value
                            }
                        />
                    </div>
                </div>

            </div>
            <ShipManifestView
                origin_swap=dest_to_origin
                origin_world=origin_world.into()
                _dest_world=dest_world.into()
                buyer_broker_skill=buyer_broker_skill.into()
                seller_broker_skill=seller_broker_skill.into()
                distance=distance
                ship_manifest=ship_manifest.into()
                write_ship_manifest=write_ship_manifest
                available_goods=available_goods.into()
                write_available_goods=write_available_goods
                available_passengers=available_passengers.into()
                show_add_manual=show_add_manual
            />

            <GoodsToSellView
                origin_world=origin_world.into()
                dest_world=dest_world.into()
                ship_manifest=ship_manifest.into()
                write_ship_manifest=write_ship_manifest
                show_add_manual=show_add_manual
            />

            <TradeView
                origin_world=origin_world.into()
                dest_world=dest_world.into()
                available_goods=available_goods.into()
                write_available_goods=write_available_goods
                available_passengers=available_passengers.into()
                ship_manifest=ship_manifest.into()
                write_ship_manifest=write_ship_manifest
            />

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

/// Row component for displaying the row in the speculative goods table for a single good.
///
/// Table view for goods currently in the manifest that are planned to be sold.
/// Sibling section beneath the manifest.
#[component]
fn GoodsToSellView(
    origin_world: Signal<Option<World>>,
    dest_world: Signal<Option<World>>,
    ship_manifest: Signal<ShipManifest>,
    write_ship_manifest: WriteSignal<ShipManifest>,
    show_add_manual: RwSignal<bool>,
) -> impl IntoView {
    let world_to_sell_on = Memo::new(move |_| {
        origin_world
            .get()
            .as_ref()
            .map(|w| format!("{} [{}]", &w.name, &w.trade_classes_string()))
            .unwrap_or_default()
    });

    view! {
        <div class="output-region">
            <div class="trade-header-row">
                // Add the name of either destination planet (if it exists) and its trade classes, or if
                // it doesn't exist the origin world and its trade classes.
                <h2>"Goods to Sell on " {move || world_to_sell_on.get()}</h2>
                <button
                    class="manifest-button manifest-add-good-button"
                    on:click=move |_| show_add_manual.set(true)
                >
                    "Manually Add Good"
                </button>
            </div>
            <table class="trade-table">
                <thead>
                    <tr>
                        <th class="table-entry">"Good"</th>
                        <th class="table-entry">"Quantity"</th>
                        <th class="table-entry">"Base Price"</th>
                        <th class="table-entry">"Purchase Price"</th>
                        <th class="table-entry">"Sell Price"</th>
                        <th class="table-entry">"Profit"</th>
                        <th class="table-entry">"Sell"</th>
                        <Show when=move || dest_world.get().is_some()>
                            <th class="table-entry">"Dest Trade Mods"</th>
                        </Show>
                    </tr>
                </thead>
                <tbody>
                    {move || {
                        ship_manifest
                            .with(|manifest| {
                                if manifest.trade_goods.is_empty() {
                                    view! {
                                        <tr>
                                            <td class="table-entry" colspan="6">
                                                "No goods to sell"
                                            </td>
                                        </tr>
                                    }
                                        .into_any()
                                } else {
                                    let mut sell_goods = manifest.trade_goods.goods().to_vec();
                                    sell_goods
                                        .sort_by(|a, b| {
                                            let a_ratio = a.sell_price.unwrap_or(0) as f64
                                                / a.buy_cost as f64;
                                            let b_ratio = b.sell_price.unwrap_or(0) as f64
                                                / b.buy_cost as f64;
                                            b_ratio
                                                .partial_cmp(&a_ratio)
                                                .unwrap_or(std::cmp::Ordering::Equal)
                                        });
                                    sell_goods
                                        .into_iter()
                                        .map(|good| {

                                            view! {
                                                <SellGoodRow
                                                    good_index=good.source_index
                                                    dest_world=dest_world
                                                    ship_manifest=ship_manifest
                                                    write_ship_manifest=write_ship_manifest
                                                />
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .into_any()
                                }
                            })
                    }}
                </tbody>
            </table>
        </div>
    }
}

#[component]
fn SellGoodRow(
    good_index: i16,
    dest_world: Signal<Option<World>>,
    ship_manifest: Signal<ShipManifest>,
    write_ship_manifest: WriteSignal<ShipManifest>,
) -> impl IntoView {
    let good = Memo::new(move |_| {
        ship_manifest.with(|manifest| {
            manifest
                .trade_goods
                .get_by_index(good_index)
                .cloned()
                .unwrap_or_default()
        })
    });

    let trade_table = TradeTable::global();

    // If there is a destination world, show the relevant trade class modifiers for this good based on the destination world's trade classes.
    let trade_mods = Memo::new(move |_| {
        if let Some(dest) = dest_world.get() {
            let dest_classes = dest.get_trade_classes();
            dest_classes
                .iter()
                .filter_map(|class| {
                    let modifier = trade_table
                        .get(good_index)
                        .and_then(|entry|
                            // Usually it will be the sale modifier for this trade class.
                            // If there isn't one, take the negative of any relevant purchase_dm.
                            entry.sale_dm.get(class).copied().or_else(|| entry.purchase_dm.get(class).map(|&v| -v)))
                        .unwrap_or(0);
                    if modifier != 0 {
                        Some(format!("{}: {:+}", class, modifier))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            String::new()
        }
    });

    let update_sold = move |ev| {
        let current_good = good.get_untracked();
        let new_value = event_target_value(&ev)
            .parse::<i32>()
            .unwrap_or(0)
            .clamp(0, current_good.quantity);
        write_ship_manifest.update(|manifest| {
            manifest.trade_goods.update_good(Good {
                transacted: new_value,
                ..current_good
            });
        });
    };

    let sell_cost_comment = move || good.get().sell_price_comment.clone();
    view! {
        <tr prop:title=sell_cost_comment>
            <td class="table-entry">{move || good.get().name.clone()}</td>
            <td class="table-entry">{move || good.get().quantity.to_string()}</td>
            <td class="table-entry">{move || good.get().base_cost.to_string()}</td>
            <td class="table-entry">{move || good.get().buy_cost.to_string()}</td>
            <td class="table-entry">
                {move || {
                    if let Some(sp) = good.get().sell_price {
                        sp.to_string()
                    } else {
                        "-".to_string()
                    }
                }}
            </td>
            <td class="table-entry">
                {move || {
                    if let Some(sp) = good.get().sell_price {
                        let pct = (((sp as f64 / good.get().buy_cost as f64) * 100.0) - 100.0)
                            .round() as i32;
                        format!("{}%", pct)
                    } else {
                        "-".to_string()
                    }
                }}
            </td>
            <td class="table-entry">
                <input
                    type="number"
                    min="0"
                    max=move || good.get().quantity
                    prop:value=move || good.get().transacted
                    on:input=update_sold
                    class=move || {
                        if good.get().transacted > 0 {
                            "purchased-input purchased-input-active"
                        } else {
                            "purchased-input"
                        }
                    }
                />
            </td>
            <Show when=move || dest_world.get().is_some()>
                <td class="table-entry">{move || trade_mods.get()}</td>
            </Show>
        </tr>
    }
}

/// This can be in one of two modes: where we are showing sale prices, or we are not
/// as defined by `show_sell_price`.
///
/// # Arguments
///
/// * `good` - The good to display
/// * `dest_classes` - Optional slice of trade classes for the destination world (used to give a hint on trading actions)
/// * `write_available_goods` - Write signal for the available goods table
#[component]
pub fn BuyGoodRow(
    good: Good,
    dest_classes: Option<Vec<TradeClass>>,
    write_available_goods: WriteSignal<AvailableGoodsTable>,
) -> impl IntoView {
    // Closure to handle changes in the amount purchased input (does NOT update manifest until Process Trades)
    let update_purchased = move |ev| {
        let new_value = event_target_value(&ev)
            .parse::<i32>()
            .unwrap_or(0)
            .clamp(0, good.quantity);
        write_available_goods.update(|ag| {
            if let Some(good) = ag
                .goods
                .iter_mut()
                .find(|g| g.source_index == good.source_index)
            {
                good.transacted = new_value;
            }
        });
    };

    let discount_percent =
        (good.buy_cost as f64 / good.base_cost as f64 * 100.0 - 100.0).round() as i32;
    let buy_cost_comment = move || good.buy_cost_comment.clone();

    let trade_table = TradeTable::global();

    // If there is a destination world, show the relevant trade class modifiers for this good based on the destination world's trade classes.
    // This is just there to help someone trading knowing what might sell well at the destination!
    let trade_mods = if let Some(dest_classes) = dest_classes {
        dest_classes
            .iter()
            .filter_map(|class| {
                let modifier = trade_table
                    .get(good.source_index)
                    .and_then(|entry|
                        // Usually it will be the sale modifier for this trade class.
                        // If there isn't one, take the negative of any relevant purchase_dm.
                        entry.sale_dm.get(class).copied().or_else(|| entry.purchase_dm.get(class).map(|&v| -v)))
                    .unwrap_or(0);
                if modifier != 0 {
                    Some(format!("{}: {:+}", class, modifier))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        String::new()
    };

    view! {
        <tr title=buy_cost_comment.clone()>
            <td class="table-entry">{good.name.clone()}</td>
            <td class="table-entry">{(good.quantity - good.transacted).to_string()}</td>
            <td class="table-entry">{good.base_cost.to_string()}</td>
            <td class="table-entry">{good.buy_cost.to_string()}</td>
            <td class="table-entry">{discount_percent.to_string()}"%"</td>
            <td class="table-entry">
                <input
                    type="number"
                    min="0"
                    max=good.quantity
                    prop:value=good.transacted
                    on:input=update_purchased
                    class=move || {
                        if good.transacted > 0 {
                            "purchased-input purchased-input-active"
                        } else {
                            "purchased-input"
                        }
                    }
                />
            </td>
            <td class="table-entry">{trade_mods}</td>
        </tr>
    }
    .into_any()
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
pub fn TradeView(
    origin_world: Signal<Option<World>>,
    dest_world: Signal<Option<World>>,
    available_goods: Signal<AvailableGoodsTable>,
    write_available_goods: WriteSignal<AvailableGoodsTable>,
    available_passengers: Signal<Option<AvailablePassengers>>,
    ship_manifest: Signal<ShipManifest>,
    write_ship_manifest: WriteSignal<ShipManifest>,
) -> impl IntoView {
    view! {
        <div class="output-region">
            <h2 class="trade-header-title">
                "Trade Goods for " {move || {
                    origin_world.read().as_ref().map(|w| w.name.clone()).unwrap_or_default()
                }}
                <span class="trade-header-classifications">
                    " [" {move || {
                        origin_world.read().as_ref().map(|w| w.trade_classes_string()).unwrap_or_default()
                    }} "]"
                </span>
                <Show when=move || {
                    dest_world.get().is_some()
                }>
                    {move || {
                        if let Some(dw) = dest_world.get() {
                            view! {
                                <span>
                                    " -> " {dw.name.clone()}
                                    <span class="trade-header-classifications">
                                        " ["{dw.trade_classes_string()}"]"
                                    </span>

                                </span>
                            }
                                .into_any()
                        } else {
                            ().into_any()
                        }
                    }}
                </Show>
            </h2>

            <Show when=move || available_passengers.get().is_some()>
                <PassengerView
                    available_passengers=available_passengers
                    ship_manifest=ship_manifest
                    write_ship_manifest=write_ship_manifest
                />
            </Show>
            <h4 class="trade-section">"Goods to Buy"</h4>
            <table class="trade-table">
                <thead>
                    {move || {
                        view! {
                            <tr>
                                <th class="table-entry">"Good"</th>
                                <th class="table-entry">"Quantity"</th>
                                <th class="table-entry">"Base Price"</th>
                                <th class="table-entry">"Buy Price"</th>
                                <th class="table-entry">"Premium"</th>
                                <th class="table-entry">"Purchased"</th>
                                <Show when=move || dest_world.get().is_some()>
                                    <th class="table-entry">"Dest Trade Mods"</th>
                                </Show>
                            </tr>
                        }
                            .into_any()
                    }}
                </thead>
                <tbody>
                    {move || {
                        if available_goods.read().is_empty() {
                            view! {
                                <tr>
                                    <td colspan="6">"No goods available"</td>
                                </tr>
                            }
                                .into_any()
                        } else {
                            let mut goods_vec = available_goods.read().goods().to_vec();
                            goods_vec
                                .sort_by(|a, b| {
                                    let a_ratio = a.buy_cost as f64 / a.base_cost as f64;
                                    let b_ratio = b.buy_cost as f64 / b.base_cost as f64;
                                    a_ratio
                                        .partial_cmp(&b_ratio)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                });
                            goods_vec
                                .into_iter()
                                .map(|good| {
                                    // For each good, show the row displaying it.
                                    view! {
                                        <BuyGoodRow
                                            good=good
                                            dest_classes=dest_world.get().as_ref().map(|w| w.get_trade_classes())
                                            write_available_goods=write_available_goods
                                        />
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
fn PassengerView(
    available_passengers: Signal<Option<AvailablePassengers>>,
    ship_manifest: Signal<ShipManifest>,
    write_ship_manifest: WriteSignal<ShipManifest>,
) -> impl IntoView {
    let add_high_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining = passengers.high - ship_manifest.read().high_passengers;
            if remaining > 0 {
                write_ship_manifest.update(|manifest| {
                    manifest.high_passengers += 1;
                });
            }
        }
    };

    let add_medium_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining = passengers.medium - ship_manifest.read().medium_passengers;
            if remaining > 0 {
                write_ship_manifest.update(|manifest| {
                    manifest.medium_passengers += 1;
                });
            }
        }
    };

    let add_basic_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining = passengers.basic - ship_manifest.read().basic_passengers;
            if remaining > 0 {
                write_ship_manifest.update(|manifest| {
                    manifest.basic_passengers += 1;
                });
            }
        }
    };

    let add_low_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining = passengers.low - ship_manifest.read().low_passengers;
            if remaining > 0 {
                write_ship_manifest.update(|manifest| {
                    manifest.low_passengers += 1;
                });
            }
        }
    };

    view! {
        <h4 class="trade-section">"Available Passengers"</h4>
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

        <h4 class="trade-section">"Available Freight (tons)"</h4>
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
                            .filter_map(|(index, lot)| {
                                let is_selected = ship_manifest
                                    .read()
                                    .freight_lot_indices
                                    .contains(&index);
                                if is_selected {
                                    return None;
                                }
                                let lot_size = lot.size;
                                let toggle_freight = move |_| {
                                    write_ship_manifest
                                        .update(|manifest| {
                                            if let Some(pos) = manifest
                                                .freight_lot_indices
                                                .iter()
                                                .position(|&i| i == index)
                                            {
                                                manifest.freight_lot_indices.remove(pos);
                                            } else {
                                                manifest.freight_lot_indices.push(index);
                                            }
                                        });
                                };
                                Some(

                                    view! {
                                        <button class="freight-lot" on:click=toggle_freight>
                                            {lot_size.to_string()}
                                        </button>
                                    },
                                )
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
/// - All amounts displayed using Credit formatting (picking Cr, KCr, or MCr as appropriate)
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
fn ShipManifestView(
    origin_swap: impl Fn() + Clone + 'static,
    origin_world: Signal<Option<World>>,
    _dest_world: Signal<Option<World>>,
    buyer_broker_skill: Signal<i16>,
    seller_broker_skill: Signal<i16>,
    distance: RwSignal<i32>,
    ship_manifest: Signal<ShipManifest>,
    write_ship_manifest: WriteSignal<ShipManifest>,
    available_goods: Signal<AvailableGoodsTable>,
    write_available_goods: WriteSignal<AvailableGoodsTable>,
    available_passengers: Signal<Option<AvailablePassengers>>,
    show_add_manual: RwSignal<bool>,
) -> impl IntoView {
    let manual_selected_index = RwSignal::new(11i16);
    let manual_qty_input = RwSignal::new(String::new());
    let manual_purchase_price_input = RwSignal::new(String::new());

    let remove_high_passenger = move |_| {
        write_ship_manifest.update(|manifest| {
            if manifest.high_passengers > 0 {
                manifest.high_passengers -= 1;
            }
        });
    };

    let remove_medium_passenger = move |_| {
        write_ship_manifest.update(|manifest| {
            if manifest.medium_passengers > 0 {
                manifest.medium_passengers -= 1;
            }
        });
    };

    let remove_basic_passenger = move |_| {
        write_ship_manifest.update(|manifest| {
            if manifest.basic_passengers > 0 {
                manifest.basic_passengers -= 1;
            }
        });
    };

    let remove_low_passenger = move |_| {
        write_ship_manifest.update(|manifest| {
            if manifest.low_passengers > 0 {
                manifest.low_passengers -= 1;
            }
        });
    };

    let on_reset = move |_| {
        // Confirm reset
        let win = leptos::leptos_dom::helpers::window();
        let proceed = win
            .confirm_with_message(
                "Reset manifest? This will clear passengers, freight, trade goods, and sell plans.",
            )
            .unwrap_or(false);
        if !proceed {
            return;
        }

        // Clear manifest and persisted storage, and clear purchased amounts in available goods
        write_ship_manifest.set(ShipManifest::default());

        // Zero out purchased in available goods so Buy inputs show 0
        write_available_goods.update(|ag| {
            for g in ag.goods.iter_mut() {
                g.transacted = 0;
            }
        });
    };

    view! {
        <div class="output-region">
            <div class="trade-header-row">
                <h2>"Ship Manifest"</h2>
                <button class="manifest-button" title="Reset" on:click=on_reset>
                    "Reset"
                </button>
            </div>

            <div class="manifest-summary">
                {move || {
                    let manifest = ship_manifest.get();
                    let cargo_tons = available_passengers
                        .read()
                        .as_ref()
                        .map(|p| manifest.total_freight_tons(p))
                        .unwrap_or(0);
                    let goods_tons: i32 = manifest.trade_goods_tonnage()
                        + available_goods.read().total_transacted_size();
                    let total_cargo = cargo_tons + goods_tons;
                    let total_passengers = manifest.total_passengers_not_low();
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
                    <button class="manifest-item passenger-button" on:click=remove_high_passenger>
                        <span class="manifest-label">"High:"</span>
                        <span class="manifest-value">
                            {move || ship_manifest.read().high_passengers}
                        </span>
                    </button>
                    <button class="manifest-item passenger-button" on:click=remove_medium_passenger>
                        <span class="manifest-label">"Medium:"</span>
                        <span class="manifest-value">
                            {move || ship_manifest.read().medium_passengers}
                        </span>
                    </button>
                    <button class="manifest-item passenger-button" on:click=remove_basic_passenger>
                        <span class="manifest-label">"Basic:"</span>
                        <span class="manifest-value">
                            {move || ship_manifest.read().basic_passengers}
                        </span>
                    </button>
                    <button class="manifest-item passenger-button" on:click=remove_low_passenger>
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
                        let manifest = ship_manifest.get();
                        let buy_goods = available_goods.read();
                        let cargo_tons = available_passengers
                            .read()
                            .as_ref()
                            .map(|p| manifest.total_freight_tons(p))
                            .unwrap_or(0);
                        let goods_tons = manifest.trade_goods_tonnage()
                            + buy_goods.total_transacted_size();
                        let goods_cost = buy_goods.total_buy_cost();
                        let goods_proceeds = manifest.trade_goods_proceeds();
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
                                <span class="manifest-value">
                                    {Credits::from(goods_cost).as_string()}
                                </span>
                            </div>
                            <div class="manifest-item">
                                <span class="manifest-label">"Goods Proceeds:"</span>
                                <span class="manifest-value">
                                    {Credits::from(goods_proceeds).as_string()}
                                </span>
                            </div>
                        }
                    }}
                </div>
            </div>

            <div class="manifest-section">
                <h5>"Freight Lots"</h5>
                <div class="freight-grid">
                    {move || {
                        if let Some(passengers) = available_passengers.get() {
                            let indices = ship_manifest.read().freight_lot_indices.clone();
                            if indices.is_empty() {
                                view! { <div>"No freight selected"</div> }.into_any()
                            } else {
                                indices
                                    .into_iter()
                                    .filter_map(|index| {
                                        passengers
                                            .freight_lots
                                            .get(index)
                                            .map(|lot| {
                                                let remove = move |_| {
                                                    write_ship_manifest
                                                        .update(|manifest| {
                                                            if let Some(pos) = manifest
                                                                .freight_lot_indices
                                                                .iter()
                                                                .position(|&i| i == index)
                                                            {
                                                                manifest.freight_lot_indices.remove(pos);
                                                            }
                                                        });
                                                };
                                                view! {
                                                    <button class="freight-lot" on:click=remove>
                                                        {lot.size.to_string()}
                                                    </button>
                                                }
                                            })
                                    })
                                    .collect::<Vec<_>>()
                                    .into_any()
                            }
                        } else {
                            view! { <div>"No freight available"</div> }.into_any()
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
                                Credits::from(revenue).as_string()
                            }}
                        </span>
                    </div>
                    <div class="manifest-item">
                        <span class="manifest-label">"Freight Revenue:"</span>
                        <span class="manifest-value">
                            {move || {
                                let manifest = ship_manifest.get();
                                let revenue = if let Some(passengers) = available_passengers.get() {
                                    manifest.freight_revenue(distance.get(), &passengers) as i64
                                } else {
                                    0
                                };
                                Credits::from(revenue).as_string()
                            }}
                        </span>
                    </div>
                    <div class="manifest-item">
                        <span class="manifest-label">"Goods Profit:"</span>
                        <span class="manifest-value">
                            {move || {
                                let manifest = ship_manifest.get();
                                let cost = available_goods.read().total_buy_cost() as i64;
                                let proceeds = manifest.trade_goods_proceeds();
                                let profit = proceeds - cost;
                                Credits::from(profit).as_string()
                            }}
                        </span>
                    </div>
                    <div class="manifest-item">
                        <span class="manifest-label">"Total:"</span>
                        <span class="manifest-value">
                            {move || {
                                let manifest = ship_manifest.get();
                                let passenger_revenue = manifest.passenger_revenue(distance.get())
                                    as i64;
                                let freight_revenue = if let Some(passengers) = available_passengers
                                    .get()
                                {
                                    manifest.freight_revenue(distance.get(), &passengers) as i64
                                } else {
                                    0
                                };
                                let goods_profit = manifest.trade_goods_proceeds()
                                    - available_goods.read().total_buy_cost() as i64;
                                let total = passenger_revenue + freight_revenue + goods_profit;
                                Credits::from(total).as_string()
                            }}
                        </span>
                    </div>
                    <div class="manifest-row-break"></div>
                    <div class="manifest-unboxed-item">
                        <button
                            class="manifest-button manifest-execute-trades-button"
                            on:click=move |_| {
                                debug!("ON BUTTON: pricing goods.");
                                write_ship_manifest
                                    .update(|manifest| {
                                        manifest
                                            .process_trades(
                                                distance.get(),
                                                &available_goods.read().goods,
                                                &available_passengers.get(),
                                            );
                                        manifest
                                            .price_goods(
                                                &origin_world.get(),
                                                buyer_broker_skill.get(),
                                                seller_broker_skill.get(),
                                            );
                                        write_available_goods.update(|ag| ag.zero_transacted());
                                        origin_swap();
                                    });
                            }
                        >
                            "Execute Trades"
                        </button>
                    </div>
                    <div class="manifest-item">
                        <span class="manifest-label">"Profit:"</span>
                        <span class=move || {
                            if ship_manifest.read().profit < 0 {
                                "manifest-value manifest-negative"
                            } else {
                                "manifest-value"
                            }
                        }>{move || Credits::from(ship_manifest.read().profit).as_string()}</span>
                    </div>

                </div>

                <Show when=move || show_add_manual.get()>
                    <div
                        class="tg-modal-backdrop"
                        on:click=move |_| show_add_manual.set(false)
                    ></div>
                    <div class="tg-modal-panel">
                        <h5 class="tg-modal-textstyle">"Add Trade Good"</h5>
                        <div class="modal-body">
                            <label class="modal-label">"Trade Good"</label>
                            <select
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    if let Ok(idx) = v.parse::<i16>() {
                                        manual_selected_index.set(idx);
                                    }
                                }
                                prop:value=move || manual_selected_index.get().to_string()
                            >
                                {move || {
                                    let table = TradeTable::global();
                                    let mut entries: Vec<_> = table.entries().cloned().collect();
                                    entries.sort_by_key(|e| e.index);
                                    entries
                                        .into_iter()
                                        .map(|entry| {
                                            let label = format!("{:>2} - {}", entry.index, entry.name);
                                            view! {
                                                <option value=entry.index.to_string()>{label}</option>
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                }}
                            </select>
                            <label class="modal-label">"Quantity (tons)"</label>
                            <input
                                type="number"
                                min="1"
                                prop:value=manual_qty_input
                                on:input=move |ev| manual_qty_input.set(event_target_value(&ev))
                            />
                            <label class="modal-label">"Purchase Price (Cr)"</label>
                            <input
                                type="number"
                                min="0"
                                prop:value=manual_purchase_price_input
                                on:input=move |ev| manual_purchase_price_input.set(event_target_value(&ev))
                                />
                        </div>
                        <div class="modal-actions">
                            <button
                                class="tg-btn tg-btn-cancel"
                                on:click=move |_| show_add_manual.set(false)
                            >
                                "Cancel"
                            </button>
                            <button
                                class="tg-btn tg-btn-done"
                                on:click=move |_| {
                                    let qty_txt = manual_qty_input.get();
                                    let qty = qty_txt.parse::<i32>().unwrap_or(0);
                                    if qty <= 0 && let Some(d) = web_sys::window()
                                            .and_then(|w| w.document())
                                        && let Some(err) = d.get_element_by_id("tg-modal-error") {
                                                err.set_text_content(
                                                    Some("Please enter a quantity greater than zero."),
                                                );
                                            }
                                    let purchase_price_txt = manual_purchase_price_input.get();
                                    let purchase_price = purchase_price_txt.parse::<i32>().unwrap_or(0);
                                    if purchase_price < 0 && let Some(d) = web_sys::window()
                                        .and_then(|w| w.document())
                                         && let Some(err) = d.get_element_by_id("tg-modal-error") {
                                                err.set_text_content(
                                                    Some("Please enter a purchase price greater than zero."),
                                            );
                                    }
                                    let table = TradeTable::default();
                                    if let Some(entry) = table.get(manual_selected_index.get()) {
                                        let good = Good {
                                            name: entry.name.clone(),
                                            quantity: qty,
                                            transacted: 0,
                                            base_cost: entry.base_cost,
                                            buy_cost: purchase_price,
                                            buy_cost_comment: String::new(),
                                            sell_price: None,
                                            sell_price_comment: String::new(),
                                            source_index: entry.index,
                                            quantity_roll: qty / entry.quantity.multiplier as i32,
                                            buy_price_roll: None,
                                            sell_price_roll: None,
                                        };
                                        write_ship_manifest
                                            .update(|manifest| {
                                                manifest.update_trade_good(good);
                                            });
                                        manual_qty_input.set(String::new());
                                        manual_purchase_price_input.set(String::new());
                                        if let Some(d) = web_sys::window()
                                            .and_then(|w| w.document())
                                            && let Some(err) = d.get_element_by_id("tg-modal-error") {
                                                err.set_text_content(None);
                                            }
                                        show_add_manual.set(false);
                                    }
                                }
                            >
                                "Done"
                            </button>
                            <div id="tg-modal-error" class="tg-error"></div>
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}
