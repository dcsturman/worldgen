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
use leptos::prelude::*;
use leptos::task::spawn_local;
#[allow(unused_imports)]
use log::{debug, error, info, warn};

use crate::backend::{
    get_signal, set_available_goods, set_available_passengers, set_buyer_broker_skill,
    set_dest_world, set_illegal_goods, set_origin_world, set_seller_broker_skill,
    set_ship_manifest, set_steward_skill, signal_names, DEFAULT_SESSION_ID,
};
use crate::components::traveller_map::WorldSearch;
use crate::systems::world::World;

use crate::trade::available_goods::{AvailableGoodsTable, Good};

use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;
use crate::trade::table::TradeTable;
use crate::trade::ZoneClassification;

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
pub fn Trade() -> impl IntoView {
    log::info!("🎯 Trade component: Starting initialization");

    // Session ID for this client
    let session_id = DEFAULT_SESSION_ID;

    // The main world always exists (starts with a default value) - keeping as local signal for now
    let origin_world = get_signal(session_id, signal_names::ORIGIN_WORLD, World::default());
    // Create remote signals that persist to Firestore
    let dest_world = get_signal(session_id, signal_names::DEST_WORLD, None::<World>);

    let available_goods = get_signal(
        session_id,
        signal_names::AVAILABLE_GOODS,
        AvailableGoodsTable::default(),
    );

    let available_passengers = get_signal(
        session_id,
        signal_names::AVAILABLE_PASSENGERS,
        None::<AvailablePassengers>,
    );

    let ship_manifest = get_signal(
        session_id,
        signal_names::SHIP_MANIFEST,
        ShipManifest::default(),
    );

    // Buyer broker skill - ReadOnlySignal (server) + RwSignal (local UI)
    let buyer_broker_skill = get_signal(session_id, signal_names::BUYER_BROKER_SKILL, 0i16);
    let seller_broker_skill = get_signal(session_id, signal_names::SELLER_BROKER_SKILL, 0i16);
    let steward_skill = get_signal(session_id, signal_names::STEWARD_SKILL, 0i16);
    let illegal_goods = get_signal(session_id, signal_names::ILLEGAL_GOODS, false);

    // Dialog state for manually adding goods to manifest
    let show_add_manual = RwSignal::new(false);
    let o_world = origin_world.get();
    let origin_world_name = RwSignal::new(o_world.name.clone());
    let origin_uwp = RwSignal::new(o_world.to_uwp());

    let origin_coords = RwSignal::new(o_world.coordinates);
    let origin_zone = RwSignal::new(o_world.travel_zone);

    let d_world = dest_world.get();
    let dest_world_name =
        RwSignal::new(d_world.clone().map(|w| w.name.clone()).unwrap_or_default());
    let dest_uwp = RwSignal::new(d_world.clone().map_or("".to_string(), |w| w.to_uwp()));
    let dest_coords = RwSignal::new(d_world.clone().and_then(|w| w.coordinates));
    let dest_zone = RwSignal::new(
        d_world
            .clone()
            .map_or(ZoneClassification::Green, |w| w.travel_zone),
    );

    // Distance between worlds
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

    info!("🎯 All signals created.");
    // Effect to recalculate distance whenever origin or destination world coordinates change
    Effect::new(move |_| {
        info!("🔄 Effect: Recalculating distance based on world coordinate changes");
        let origin = origin_world.get();
        let dest = dest_world.get();

        distance.set(compute_distance(origin, dest));
    });

    // Effect to keep origin world updated based on changes in name or uwp.
    // If name or uwp changes, update origin_world.
    Effect::new(move |prev: Option<(String, String)>| {
        info!("🔄 Effect: origin_world update running");
        if let Some((prev_name, prev_uwp)) = &prev {
            if *prev_name == origin_world_name.get() && *prev_uwp == origin_uwp.get() {
                return (prev_name.to_string(), prev_uwp.to_string());
            }
        }

        let name = origin_world_name.get();
        let uwp = origin_uwp.get();
        if !name.is_empty() && uwp.len() == 9 {
            let Ok(mut world) = World::from_upp(&name, &uwp, false, false) else {
                log::error!("Failed to parse UPP in hook to build origin world: {uwp}");
                return (name, uwp);
            };
            world.gen_trade_classes();
            world.coordinates = origin_coords.get();
            world.travel_zone = origin_zone.get();
            world.gen_trade_classes();

            let world_send = world.clone();
            spawn_local(async move {
                if let Err(e) = set_origin_world(session_id.to_string(), world_send).await {
                    error!("Failed to set_origin_world for session {session_id}: {e:?}");
                }
            });

            // Now update available goods
            let ag = AvailableGoodsTable::for_world(
                TradeTable::global(),
                &world.get_trade_classes(),
                world.get_population(),
                illegal_goods.get(),
            )
            .unwrap();

            spawn_local(async move {
                debug!(
                    "📤 CLIENT: About to call set_available_goods with {} goods",
                    ag.goods.len()
                );
                match serde_json::to_string(&ag) {
                    Ok(json) => debug!("📤 CLIENT: Serialized AvailableGoodsTable: {}", json),
                    Err(e) => error!(
                        "❌ CLIENT: Failed to serialize AvailableGoodsTable to JSON: {}",
                        e
                    ),
                }
                if let Err(e) = set_available_goods(session_id.to_string(), ag).await {
                    error!(
                        "❌ CLIENT: Failed to set_available_goods on session {session_id}: {e:?}"
                    );
                }
            });
        } else {
            // If we don't have a valid name, reset other UI elements to reasonable defaults.
            origin_zone.set(ZoneClassification::Green);
            distance.set(0);
        }
        (name, uwp)
    });

    // Effect to keep destination world updated based on changes in name or uwp.
    // If name or uwp changes, update dest_world.
    Effect::new(move |prev: Option<(String, String)>| {
        info!("🔄 Effect: dest_world update running");
        if let Some((prev_name, prev_uwp)) = &prev {
            if *prev_name == dest_world_name.get() && *prev_uwp == dest_uwp.get() {
                return (prev_name.to_string(), prev_uwp.to_string());
            }
        }

        let name = dest_world_name.get();
        let uwp = dest_uwp.get();

        if !name.is_empty() && uwp.len() == 9 {
            debug!("🎯 In dest_update effect - have a valid name and uwp");
            let Ok(mut world) = World::from_upp(&name, &uwp, false, false) else {
                log::error!("Failed to parse UPP in hook to build destination world: {uwp}");
                return (name, uwp);
            };
            world.gen_trade_classes();
            world.coordinates = dest_coords.get();
            world.travel_zone = dest_zone.get();

            spawn_local(async move {
                if let Err(e) = set_dest_world(session_id.to_string(), Some(world)).await {
                    log::error!(
                        "Failed to set_dest_world to Some() value on session {session_id}: {e:?}"
                    );
                }
            });
        } else {
            debug!("⏭️ Effect for dest_world: Don't yet have a valid name ({name}) or uwp ({uwp})");
            // If we don't have a valid name, reset other UI elements to reasonable defaults.
            spawn_local(async move {
                if let Err(e) = set_dest_world(session_id.to_string(), None).await {
                    log::error!(
                        "Failed to set_dest_world to None value on session {session_id}: {e:?}"
                    );
                }
            });
            dest_zone.set(ZoneClassification::Green);
            distance.set(0);
        }
        (name, uwp)
    });

    // Effect to regenerate passengers if origin, destination, or skills change.
    Effect::new(move |_| {
        info!("🔄 Effect: Regenerating passengers based on world and distance changes");
        let origin = origin_world.get();
        let dest = dest_world.get();

        if distance.get() > 0 && dest.is_some() {
            let dest = dest.unwrap();
            let mut ap_option = untrack(|| available_passengers.get());

            let ap = ap_option.get_or_insert_with(AvailablePassengers::default);

            ap.generate(
                origin.get_population(),
                origin.port,
                origin.travel_zone,
                origin.tech_level,
                dest.get_population(),
                dest.port,
                dest.travel_zone,
                dest.tech_level,
                distance.get(),
                i32::from(steward_skill.get()),
                i32::from(buyer_broker_skill.get()),
            );

            let ap_to_send = ap_option.clone();
            spawn_local(async move {
                if let Err(e) = set_available_passengers(session_id.to_string(), ap_to_send).await {
                    log::error!(
                        "Failed to set_available_passengers on session {session_id}: {e:?}"
                    );
                }
            });
        }
    });

    Effect::new(move |_| {
        let dest_name = dest_world
            .get()
            .map(|d| d.name)
            .unwrap_or_else(|| "NO NAME".to_string());

        warn!("******* DEST CHANGED WITH NAME {dest_name}");
    });

    Effect::new(move |_| {
        let origin_name = origin_world.get().name;

        warn!("******* ORIGIN CHANGED WITH NAME {origin_name}");
    });

    // Effect to recalculate goods pricing and manifest pricing when skills or world parameters change (using saved rolls, not regenerating)
    Effect::new(move |_| {
        info!("🔄 Effect: Regenerating goods based on world or skill changes.");
        let buyer = buyer_broker_skill.get();
        let supplier = seller_broker_skill.get();
        let dest_world = dest_world.get();

        // Check if destination world changed (not just skills)
        let current_dest_name = dest_world.as_ref().map(|w| w.name.clone());

        let mut current_goods = untrack(|| available_goods.get());
        current_goods.price_goods_to_sell(
            dest_world.as_ref().map(|w| w.get_trade_classes()),
            supplier,
            buyer,
        );
        current_goods.sort_by_discount();

        spawn_local(async move {
            debug!(
                "📤 CLIENT: About to call set_available_goods (repricing) with {} goods",
                current_goods.goods.len()
            );
            match serde_json::to_string(&current_goods) {
                Ok(json) => debug!(
                    "📤 CLIENT: Serialized AvailableGoodsTable (repricing): {}",
                    json
                ),
                Err(e) => error!(
                    "❌ CLIENT: Failed to serialize AvailableGoodsTable to JSON (repricing): {}",
                    e
                ),
            }
            if let Err(e) = set_available_goods(session_id.to_string(), current_goods).await {
                log::error!(
                    "❌ CLIENT: Failed to set_available_goods on session {session_id}: {e:?}"
                );
            }
        });

        // Reprice the manifest
        // Manifest goods are sold at the destination, so use dest_world for pricing
        let mut manifest = untrack(|| ship_manifest.get());

        manifest.price_goods(
            &dest_world,
            buyer_broker_skill.get(),
            seller_broker_skill.get(),
        );

        spawn_local(async move {
            if let Err(e) = set_ship_manifest(session_id.to_string(), manifest).await {
                error!("Failed to set_ship_manifest for session {session_id}: {e:?}");
            }
        });

        current_dest_name
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
                <div style="display: flex; align-items: center; padding: 10px;">
                    <button
                        class="blue-button"
                        on:click=move |_| {
                            let origin = origin_world.get();
                            let session_id = DEFAULT_SESSION_ID.to_string();
                            let session_id2 = session_id.clone();
                            let session_id3 = session_id.clone();

                            // Update available_goods
                            let mut ag = available_goods.get();
                            ag.reset_die_rolls();
                            ag.price_goods_to_buy(
                                &origin.get_trade_classes(),
                                buyer_broker_skill.get(),
                                seller_broker_skill.get(),
                            );
                            ag.sort_by_discount();
                            let ag_clone = ag.clone();
                            spawn_local(async move {
                                let _ = set_available_goods(session_id, ag_clone).await;
                            });

                            // Update ship_manifest
                            let mut manifest = ship_manifest.get();
                            manifest.reset_die_rolls();
                            manifest.price_goods(
                                &Some(origin.clone()),
                                buyer_broker_skill.get(),
                                seller_broker_skill.get(),
                            );
                            let manifest_clone = manifest.clone();
                            spawn_local(async move {
                                let _ = set_ship_manifest(session_id2, manifest_clone).await;
                            });

                            // Update available_passengers
                            if let Some(world) = dest_world.get() {
                                if distance.get() > 0 {
                                    let mut passengers = available_passengers.get().unwrap_or_default();
                                    passengers.reset_die_rolls();
                                    passengers.generate(
                                        origin.get_population(),
                                        origin.port,
                                        origin.travel_zone,
                                        origin.tech_level,
                                        world.get_population(),
                                        world.port,
                                        world.travel_zone,
                                        world.tech_level,
                                        distance.get(),
                                        i32::from(steward_skill.get()),
                                        i32::from(buyer_broker_skill.get()),
                                    );
                                    let session_id3_clone = session_id3.clone();
                                    spawn_local(async move {
                                        let _ = set_available_passengers(session_id3_clone, Some(passengers)).await;
                                    });
                                } else {
                                    let session_id3_clone = session_id3.clone();
                                    spawn_local(async move {
                                        let _ = set_available_passengers(session_id3_clone, None).await;
                                    });
                                }
                            } else {
                                spawn_local(async move {
                                    let _ = set_available_passengers(session_id3, None).await;
                                });
                            }
                        }
                    >
                        "Generate"
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
                            "Origin Classes: "
                            {move || {
                                format!(
                                    "[{}] {}",
                                    origin_world.get().trade_classes_string(),
                                    origin_world.get().travel_zone,
                                )
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
                                let value: i16 = event_target_value(&ev).parse().unwrap_or(0);
                                log::trace!("📝 buyer_broker_skill: user input({})", value);
                                let session = session_id.to_string();
                                spawn_local(async move {
                                    log::trace!("📤 buyer_broker_skill: calling server function({})", value);
                                    if let Err(e) = set_buyer_broker_skill(session, value).await {
                                        log::error!("❌ buyer_broker_skill: server function failed: {:?}", e);
                                    }
                                });
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
                                let value: i16 = event_target_value(&ev).parse().unwrap_or(0);
                                let session = session_id.to_string();
                                spawn_local(async move {
                                    let _ = set_seller_broker_skill(session, value).await;
                                });
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
                                let value: i16 = event_target_value(&ev).parse().unwrap_or(0);
                                let session = session_id.to_string();
                                spawn_local(async move {
                                    let _ = set_steward_skill(session, value).await;
                                });
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
                                let session = session_id.to_string();
                                let session2 = session.clone();
                                spawn_local(async move {
                                    let _ = set_illegal_goods(session, checked).await;
                                });
                                let ag = AvailableGoodsTable::for_world(
                                        TradeTable::global(),
                                        &origin_world.get().get_trade_classes(),
                                        origin_world.get().get_population(),
                                        checked,
                                    )
                                    .unwrap();
                                spawn_local(async move {
                                    let _ = set_available_goods(session2, ag).await;
                                });
                            }
                        />
                    </div>
                </div>

            </div>
            <ShipManifestView
                origin_swap=dest_to_origin
                _origin_world=origin_world
                dest_world=dest_world
                buyer_broker_skill=buyer_broker_skill
                seller_broker_skill=seller_broker_skill
                distance=distance
                ship_manifest=ship_manifest
                available_goods=available_goods
                available_passengers=available_passengers
                show_add_manual=show_add_manual
            />

            <GoodsToSellView
                origin_world=origin_world
                dest_world=dest_world
                ship_manifest=ship_manifest
                show_add_manual=show_add_manual
            />

            <TradeView
                origin_world=origin_world
                dest_world=dest_world
                available_goods=available_goods
                available_passengers=available_passengers
                ship_manifest=ship_manifest
            />

        </div>
    }
}

fn compute_distance(origin: World, dest: Option<World>) -> i32 {
    if let Some(dest) = dest {
        if let (Some(o_coord), Some(d_coord)) = (origin.coordinates, dest.coordinates) {
            return crate::components::traveller_map::calculate_hex_distance(
                o_coord.0, o_coord.1, d_coord.0, d_coord.1,
            );
        }
    }
    0
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
    origin_world: Signal<World>,
    dest_world: Signal<Option<World>>,
    ship_manifest: Signal<ShipManifest>,
    show_add_manual: RwSignal<bool>,
) -> impl IntoView {
    let world_to_sell_on = Memo::new(move |_| {
        let world_name_classes = dest_world
            .get()
            .as_ref()
            .map(|w| (w.name.clone(), w.trade_classes_string()))
            .unwrap_or_else(|| {
                (
                    origin_world.get().name.clone(),
                    origin_world.get().trade_classes_string(),
                )
            });
        format!("{} [{}]", world_name_classes.0, world_name_classes.1)
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
                                                    ship_manifest=ship_manifest
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
    ship_manifest: Signal<ShipManifest>,
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

    let update_sold = move |ev| {
        let current_good = good.get_untracked();
        let new_value = event_target_value(&ev)
            .parse::<i32>()
            .unwrap_or(0)
            .clamp(0, current_good.quantity);
        let mut manifest = ship_manifest.get();
        manifest.trade_goods.update_good(Good {
            transacted: new_value,
            ..current_good
        });
        let session_id = DEFAULT_SESSION_ID.to_string();
        spawn_local(async move {
            let _ = set_ship_manifest(session_id, manifest).await;
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
        </tr>
    }
}

/// This can be in one of two modes: where we are showing sale prices, or we are not
/// as defined by `show_sell_price`.
///
/// # Arguments
///
/// * `good` - The good to display
/// * `available_goods` - Write signal for the available goods table
/// * `ship_manifest` - Signal for the ship manifest
/// * `ship_manifest` - Write signal for the ship manifest
/// * `show_sell_price` - Signal for whether to show sell prices
#[component]
pub fn BuyGoodRow(
    good: Good,
    available_goods: Signal<AvailableGoodsTable>,
) -> impl IntoView {
    // Closure to handle changes in the amount purchased input (does NOT update manifest until Process Trades)
    let update_purchased = move |ev| {
        let new_value = event_target_value(&ev)
            .parse::<i32>()
            .unwrap_or(0)
            .clamp(0, good.quantity);
        let mut ag = available_goods.get();
        if let Some(good) = ag
            .goods
            .iter_mut()
            .find(|g| g.source_index == good.source_index)
        {
            good.transacted = new_value;
        }
        let session_id = DEFAULT_SESSION_ID.to_string();
        spawn_local(async move {
            let _ = set_available_goods(session_id, ag).await;
        });
    };

    let discount_percent =
        (good.buy_cost as f64 / good.base_cost as f64 * 100.0 - 100.0).round() as i32;
    let buy_cost_comment = move || good.buy_cost_comment.clone();

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
    origin_world: Signal<World>,
    dest_world: Signal<Option<World>>,
    available_goods: Signal<AvailableGoodsTable>,
    available_passengers: Signal<Option<AvailablePassengers>>,
    ship_manifest: Signal<ShipManifest>,
) -> impl IntoView {
    view! {
        <div class="output-region">
            <h2 class="trade-header-title">
                "Trade Goods for " {move || origin_world.get().name.clone()}
                <span class="trade-header-classifications">
                    " [" {move || origin_world.get().trade_classes_string()} "]"
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
                                            available_goods=available_goods
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
) -> impl IntoView {
    let add_high_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining =
                passengers.high - ship_manifest.read().high_passengers;
            if remaining > 0 {
                let mut manifest = ship_manifest.get();
                manifest.high_passengers += 1;
                let session_id = DEFAULT_SESSION_ID.to_string();
                spawn_local(async move {
                    let _ = set_ship_manifest(session_id, manifest).await;
                });
            }
        }
    };

    let add_medium_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining =
                passengers.medium - ship_manifest.read().medium_passengers;
            if remaining > 0 {
                let mut manifest = ship_manifest.get();
                manifest.medium_passengers += 1;
                let session_id = DEFAULT_SESSION_ID.to_string();
                spawn_local(async move {
                    let _ = set_ship_manifest(session_id, manifest).await;
                });
            }
        }
    };

    let add_basic_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining =
                passengers.basic - ship_manifest.read().basic_passengers;
            if remaining > 0 {
                let mut manifest = ship_manifest.get();
                manifest.basic_passengers += 1;
                let session_id = DEFAULT_SESSION_ID.to_string();
                spawn_local(async move {
                    let _ = set_ship_manifest(session_id, manifest).await;
                });
            }
        }
    };

    let add_low_passenger = move |_| {
        if let Some(passengers) = available_passengers.get() {
            let remaining =
                passengers.low - ship_manifest.read().low_passengers;
            if remaining > 0 {
                let mut manifest = ship_manifest.get();
                manifest.low_passengers += 1;
                let session_id = DEFAULT_SESSION_ID.to_string();
                spawn_local(async move {
                    let _ = set_ship_manifest(session_id, manifest).await;
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
                                    let mut manifest = ship_manifest.get();
                                    if let Some(pos) = manifest
                                        .freight_lot_indices
                                        .iter()
                                        .position(|&i| i == index)
                                    {
                                        manifest.freight_lot_indices.remove(pos);
                                    } else {
                                        manifest.freight_lot_indices.push(index);
                                    }
                                    let session_id = DEFAULT_SESSION_ID.to_string();
                                    spawn_local(async move {
                                        let _ = set_ship_manifest(session_id, manifest).await;
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
    _origin_world: Signal<World>,
    dest_world: Signal<Option<World>>,
    buyer_broker_skill: Signal<i16>,
    seller_broker_skill: Signal<i16>,
    distance: RwSignal<i32>,
    ship_manifest: Signal<ShipManifest>,
    available_goods: Signal<AvailableGoodsTable>,
    available_passengers: Signal<Option<AvailablePassengers>>,
    show_add_manual: RwSignal<bool>,
) -> impl IntoView {
    let manual_selected_index = RwSignal::new(11i16);
    let manual_qty_input = RwSignal::new(String::new());

    let remove_high_passenger = move |_| {
        let mut manifest = ship_manifest.get();
        if manifest.high_passengers > 0 {
            manifest.high_passengers -= 1;
        }
        let session_id = DEFAULT_SESSION_ID.to_string();
        spawn_local(async move {
            let _ = set_ship_manifest(session_id, manifest).await;
        });
    };

    let remove_medium_passenger = move |_| {
        let mut manifest = ship_manifest.get();
        if manifest.medium_passengers > 0 {
            manifest.medium_passengers -= 1;
        }
        let session_id = DEFAULT_SESSION_ID.to_string();
        spawn_local(async move {
            let _ = set_ship_manifest(session_id, manifest).await;
        });
    };

    let remove_basic_passenger = move |_| {
        let mut manifest = ship_manifest.get();
        if manifest.basic_passengers > 0 {
            manifest.basic_passengers -= 1;
        }
        let session_id = DEFAULT_SESSION_ID.to_string();
        spawn_local(async move {
            let _ = set_ship_manifest(session_id, manifest).await;
        });
    };

    let remove_low_passenger = move |_| {
        let mut manifest = ship_manifest.get();
        if manifest.low_passengers > 0 {
            manifest.low_passengers -= 1;
        }
        let session_id = DEFAULT_SESSION_ID.to_string();
        spawn_local(async move {
            let _ = set_ship_manifest(session_id, manifest).await;
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
        let manifest = ShipManifest::default();
        let session_id = DEFAULT_SESSION_ID.to_string();
        let session_id2 = session_id.clone();
        spawn_local(async move {
            let _ = set_ship_manifest(session_id, manifest).await;
        });

        // Zero out purchased in available goods so Buy inputs show 0
        let mut ag = available_goods.get();
        for g in ag.goods.iter_mut() {
            g.transacted = 0;
        }
        spawn_local(async move {
            let _ = set_available_goods(session_id2, ag).await;
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
                                                    let mut manifest = ship_manifest.get();
                                                    if let Some(pos) = manifest
                                                        .freight_lot_indices
                                                        .iter()
                                                        .position(|&i| i == index)
                                                    {
                                                        manifest.freight_lot_indices.remove(pos);
                                                    }
                                                    let session_id = DEFAULT_SESSION_ID.to_string();
                                                    spawn_local(async move {
                                                        let _ = set_ship_manifest(session_id, manifest).await;
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
                                let mut manifest = ship_manifest.get();
                                manifest
                                    .process_trades(
                                        distance.get(),
                                        &available_goods.read().goods,
                                        &available_passengers.get(),
                                    );
                                manifest
                                    .price_goods(
                                        &dest_world.get(),
                                        buyer_broker_skill.get(),
                                        seller_broker_skill.get(),
                                    );
                                let session_id = DEFAULT_SESSION_ID.to_string();
                                let session_id2 = session_id.clone();
                                spawn_local(async move {
                                    let _ = set_ship_manifest(session_id, manifest).await;
                                });

                                let mut ag = available_goods.get();
                                ag.zero_transacted();
                                spawn_local(async move {
                                    let _ = set_available_goods(session_id2, ag).await;
                                });

                                origin_swap();
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
                                    if qty <= 0 {
                                        if let Some(d) = web_sys::window()
                                            .and_then(|w| w.document())
                                        {
                                            if let Some(err) = d.get_element_by_id("tg-modal-error") {
                                                err.set_text_content(
                                                    Some("Please enter a quantity greater than zero."),
                                                );
                                            }
                                        }
                                    }
                                    let table = TradeTable::default();
                                    if let Some(entry) = table.get(manual_selected_index.get()) {
                                        let good = Good {
                                            name: entry.name.clone(),
                                            quantity: qty,
                                            transacted: 0,
                                            base_cost: entry.base_cost,
                                            buy_cost: entry.base_cost,
                                            buy_cost_comment: String::new(),
                                            sell_price: None,
                                            sell_price_comment: String::new(),
                                            source_index: entry.index,
                                            quantity_roll: qty / entry.quantity.multiplier as i32,
                                            buy_price_roll: None,
                                            sell_price_roll: None,
                                        };
                                        let mut manifest = ship_manifest.get();
                                        manifest.update_trade_good(good);
                                        let session_id = DEFAULT_SESSION_ID.to_string();
                                        spawn_local(async move {
                                            let _ = set_ship_manifest(session_id, manifest).await;
                                        });
                                        manual_qty_input.set(String::new());
                                        if let Some(d) = web_sys::window()
                                            .and_then(|w| w.document())
                                        {
                                            if let Some(err) = d.get_element_by_id("tg-modal-error") {
                                                err.set_text_content(None);
                                            }
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
