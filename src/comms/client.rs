//! # Trade State WebSocket Client
//!
//! This module provides a WebSocket client for syncing trade state
//! with the server and other connected clients.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leptos::prelude::*;
use log::{debug, error, info, warn};
use wasm_bindgen::prelude::*;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use super::{ServerCommand, ServerMessage, TradeState};
use crate::systems::world::World;
use crate::trade::Ship;
use crate::trade::ZoneClassification;
use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;

/// Write-side handles for every reactive field the WebSocket client
/// pushes into when a new [`TradeState`] arrives from the server.
///
/// ## Contract
///
/// `TradeSignals` is **write-only**. Every field is a [`WriteSignal`];
/// the trade-computer component owns the matching read sides
/// ([`ReadSignal`] / [`Memo`] / [`Signal`]) and registers these write
/// halves with the [`Client`] via [`Client::register_signals`]. When a
/// state update lands on the WebSocket, the client deserialises it into
/// a [`TradeState`] and calls `set` on each of these signals; the
/// component re-renders from the corresponding read sides.
///
/// ## Field shape
///
/// - World fields (`origin_*`, `dest_*`) split per attribute the client
///   may want to bind individually. The client sends `name`, `uwp`,
///   `coords`, `zone` to the server; the server fills the `*_world`
///   fields and sends the populated [`World`] back.
/// - `ship` is the unified [`Ship`] config — capacity, crew, hardware,
///   ship-broker / steward / leadership skills, periodic costs.
///   Carried as a single signal because every field is server-mirrored
///   and the component splits it back out via memos on the read side.
/// - `system_broker_skill` is the planet-side counterparty broker.
///   Replaces the legacy `buyer_broker_skill` / `seller_broker_skill`
///   pair (the player's broker is now `ship.broker_skill`).
#[derive(Clone)]
pub struct TradeSignals {
    pub origin_world_name: WriteSignal<String>,
    pub origin_uwp: WriteSignal<String>,
    pub origin_coords: WriteSignal<Option<(i32, i32)>>,
    pub origin_zone: WriteSignal<ZoneClassification>,
    pub origin_world: WriteSignal<Option<World>>,
    pub dest_world_name: WriteSignal<String>,
    pub dest_uwp: WriteSignal<String>,
    pub dest_coords: WriteSignal<Option<(i32, i32)>>,
    pub dest_zone: WriteSignal<ZoneClassification>,
    pub dest_world: WriteSignal<Option<World>>,
    pub available_goods: WriteSignal<AvailableGoodsTable>,
    pub available_passengers: WriteSignal<Option<AvailablePassengers>>,
    pub ship_manifest: WriteSignal<ShipManifest>,
    /// Unified ship configuration (Ship Broker skill, steward skill,
    /// capacity, crew, hardware, periodic costs). The component owns the
    /// read side and may project individual sub-fields via memos.
    pub ship: WriteSignal<Ship>,
    /// Planet-side broker on the current trade leg. The Ship Broker
    /// skill lives on `ship.broker_skill`, not here.
    pub system_broker_skill: WriteSignal<i16>,
    pub illegal_goods: WriteSignal<bool>,
}

/// WebSocket client for trade state synchronization
pub struct Client {
    /// The WebSocket connection
    ws: WebSocket,
    /// Registered signals (set after component initialization)
    signals: Rc<RefCell<Option<TradeSignals>>>,
    /// Last state received from server (used to detect echoes)
    /// If current state matches this exactly, we don't send it back
    last_received_state: Rc<RefCell<Option<TradeState>>>,
    /// Whether we have received the initial state from the server
    /// This prevents sending default state before server has a chance to send us the real state
    received_initial_state: Rc<Cell<bool>>,
    /// Currently-selected ship. The server scopes this connection's state
    /// to the named ship's Firestore session. We hold it here so we can
    /// re-send it if the WebSocket reconnects, and so `set_ship` can fire
    /// before the WS has finished opening (it queues until `onopen`).
    pending_ship: Rc<RefCell<Option<String>>>,
}

impl Client {
    /// Creates a new Client and connects to the WebSocket server
    ///
    /// # Arguments
    ///
    /// * `server_url` - The WebSocket server URL (e.g., "ws://localhost:8080")
    ///
    /// # Errors
    ///
    /// Returns an error string if the WebSocket connection fails
    pub fn new(server_url: &str) -> Result<Self, String> {
        let ws = WebSocket::new(server_url)
            .map_err(|e| format!("Failed to create WebSocket: {:?}", e))?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let signals: Rc<RefCell<Option<TradeSignals>>> = Rc::new(RefCell::new(None));
        let last_received_state: Rc<RefCell<Option<TradeState>>> = Rc::new(RefCell::new(None));
        let received_initial_state: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let pending_ship: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

        // Set up message handler
        let signals_clone = signals.clone();
        let last_received_clone = last_received_state.clone();
        let received_initial_clone = received_initial_state.clone();
        let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Some(text) = e.data().as_string() {
                handle_message(
                    &text,
                    &signals_clone,
                    &last_received_clone,
                    &received_initial_clone,
                );
            }
        });
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();

        // Set up error handler
        let onerror_callback = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            error!("WebSocket error: {:?}", e.message());
        });
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();

        // Set up open handler. If a ship name was queued before the socket
        // finished opening (the common case — the component reads it from
        // localStorage on mount, before the WS handshake completes), send
        // the SelectShip command now so the server can hand us that ship's
        // state.
        let pending_ship_for_open = pending_ship.clone();
        let ws_for_open = ws.clone();
        let onopen_callback = Closure::<dyn FnMut()>::new(move || {
            info!("WebSocket connection established");
            if let Some(ship_name) = pending_ship_for_open.borrow().clone() {
                let msg = ServerMessage::Command(ServerCommand::SelectShip { ship_name });
                match serde_json::to_string(&msg) {
                    Ok(json) => {
                        if let Err(e) = ws_for_open.send_with_str(&json) {
                            error!("Failed to send queued SelectShip on open: {:?}", e);
                        } else {
                            debug!("Sent queued SelectShip on connection open");
                        }
                    }
                    Err(e) => error!("Failed to serialize queued SelectShip: {}", e),
                }
            }
        });
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        // Set up close handler
        let onclose_callback = Closure::<dyn FnMut(_)>::new(move |e: CloseEvent| {
            info!(
                "WebSocket connection closed: code={}, reason={}",
                e.code(),
                e.reason()
            );
        });
        ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        onclose_callback.forget();

        Ok(Self {
            ws,
            signals,
            last_received_state,
            received_initial_state,
            pending_ship,
        })
    }

    /// Tell the server which ship's session this connection should sync.
    ///
    /// Resets the "received initial state" flag so the next StateUpdate
    /// effect won't fire until the server's reply for the new ship has
    /// arrived. If the WebSocket is already open we send immediately;
    /// otherwise we hold the name and the `onopen` handler will send it.
    /// Calling this with the same name as before is harmless — the server
    /// just sends that ship's state again.
    pub fn set_ship(&self, ship_name: String) {
        *self.pending_ship.borrow_mut() = Some(ship_name.clone());
        self.received_initial_state.set(false);
        *self.last_received_state.borrow_mut() = None;

        if !self.is_connected() {
            // Will be sent by onopen when the socket finishes opening.
            debug!("set_ship: socket not open yet, queued ship name");
            return;
        }

        let msg = ServerMessage::Command(ServerCommand::SelectShip { ship_name });
        match serde_json::to_string(&msg) {
            Ok(json) => {
                if let Err(e) = self.ws.send_with_str(&json) {
                    error!("Failed to send SelectShip: {:?}", e);
                } else {
                    debug!("Sent SelectShip command to server");
                }
            }
            Err(e) => error!("Failed to serialize SelectShip: {}", e),
        }
    }

    /// Register signals with the client for receiving updates
    ///
    /// # Arguments
    ///
    /// * `signals` - The TradeSignals struct containing all write signals
    pub fn register_signals(&self, signals: TradeSignals) {
        *self.signals.borrow_mut() = Some(signals);
        info!("Trade signals registered with client");
    }

    /// Send a TradeState update to the server
    ///
    /// # Arguments
    ///
    /// * `state` - The TradeState to send
    pub fn send_state(&self, state: &TradeState) {
        match serde_json::to_string(state) {
            Ok(json) => {
                if let Err(e) = self.ws.send_with_str(&json) {
                    error!("Failed to send trade state: {:?}", e);
                } else {
                    debug!("Sent trade state update to server");
                }
            }
            Err(e) => {
                error!("Failed to serialize trade state: {}", e);
            }
        }
    }

    /// Send a regenerate command to the server to re-roll prices and passengers
    pub fn send_regenerate(&self) {
        let msg = ServerMessage::Command(ServerCommand::Regenerate);
        match serde_json::to_string(&msg) {
            Ok(json) => {
                if let Err(e) = self.ws.send_with_str(&json) {
                    error!("Failed to send regenerate command: {:?}", e);
                } else {
                    debug!("Sent regenerate command to server");
                }
            }
            Err(e) => {
                error!("Failed to serialize regenerate command: {}", e);
            }
        }
    }

    /// Ask the server to subtract one 28-day period of fixed expenses
    /// (mortgage + maintenance + salary) from the current ship's manifest
    /// profit. The server applies the deduction, persists the new state,
    /// and broadcasts it back to all clients on this ship.
    pub fn send_apply_monthly_expenses(&self) {
        let msg = ServerMessage::Command(ServerCommand::ApplyMonthlyExpenses);
        match serde_json::to_string(&msg) {
            Ok(json) => {
                if let Err(e) = self.ws.send_with_str(&json) {
                    error!("Failed to send apply_monthly_expenses command: {:?}", e);
                } else {
                    debug!("Sent apply_monthly_expenses command to server");
                }
            }
            Err(e) => {
                error!("Failed to serialize apply_monthly_expenses command: {}", e);
            }
        }
    }

    /// Check if the WebSocket connection is open
    pub fn is_connected(&self) -> bool {
        self.ws.ready_state() == WebSocket::OPEN
    }

    /// Check if the given state matches what we last received from server
    ///
    /// This is used to prevent sending state back to the server when it's
    /// just an echo of what we received. If user has made additional changes,
    /// the state won't match and we'll send it.
    pub fn is_echo_of_received(&self, state: &TradeState) -> bool {
        if let Some(ref last) = *self.last_received_state.borrow() {
            states_equal(last, state)
        } else {
            false
        }
    }

    /// Clear the last received state (call after successfully sending)
    ///
    /// This ensures that if the user makes the same change again, it will be sent.
    pub fn clear_last_received(&self) {
        *self.last_received_state.borrow_mut() = None;
    }

    /// Check if we have received the initial state from the server
    ///
    /// This is used to prevent sending default state before the server has a chance
    /// to send us the real state. Once we receive the first message from the server,
    /// this returns true and we can start sending user changes.
    pub fn has_received_initial_state(&self) -> bool {
        self.received_initial_state.get()
    }
}

/// Compare two TradeState instances for equality (ignoring version)
fn states_equal(a: &TradeState, b: &TradeState) -> bool {
    // Compare all fields except version
    // Note: We compare the client-sent fields (name, UWP, coords, zone) but NOT the World objects
    // The World objects are generated by the server and we don't send them back to the server.
    // `Ship` derives `PartialEq`, so a single `==` covers every ship-side
    // field (Ship Broker skill, steward, capacity, hardware, costs, …).
    a.origin_world_name == b.origin_world_name
        && a.origin_uwp == b.origin_uwp
        && a.origin_coords == b.origin_coords
        && a.origin_zone == b.origin_zone
        && a.dest_world_name == b.dest_world_name
        && a.dest_uwp == b.dest_uwp
        && a.dest_coords == b.dest_coords
        && a.dest_zone == b.dest_zone
        && a.available_goods == b.available_goods
        && a.available_passengers == b.available_passengers
        && a.ship_manifest == b.ship_manifest
        && a.ship == b.ship
        && a.system_broker_skill == b.system_broker_skill
        && a.illegal_goods == b.illegal_goods
}

/// Handle incoming WebSocket messages
fn handle_message(
    text: &str,
    signals: &Rc<RefCell<Option<TradeSignals>>>,
    last_received: &Rc<RefCell<Option<TradeState>>>,
    received_initial_state: &Rc<Cell<bool>>,
) {
    let signals_opt = signals.borrow();
    let Some(signals) = signals_opt.as_ref() else {
        warn!("Received trade state update but no signals registered yet");
        return;
    };

    let state: TradeState = match serde_json::from_str(text) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to deserialize trade state: {}", e);
            return;
        }
    };

    debug!("Received trade state update from server");

    // Mark that we've received the initial state from the server
    received_initial_state.set(true);

    // Store the received state so we can detect echoes
    *last_received.borrow_mut() = Some(state.clone());

    // Update each signal with the new state values
    // Using set() which will trigger reactivity only if the value changed
    signals.origin_world_name.set(state.origin_world_name);
    signals.origin_uwp.set(state.origin_uwp);
    signals.origin_coords.set(state.origin_coords);
    signals.origin_zone.set(state.origin_zone);
    signals.origin_world.set(state.origin_world);
    signals.dest_world_name.set(state.dest_world_name);
    signals.dest_uwp.set(state.dest_uwp);
    signals.dest_coords.set(state.dest_coords);
    signals.dest_zone.set(state.dest_zone);
    signals.dest_world.set(state.dest_world);
    signals.available_goods.set(state.available_goods);
    signals.available_passengers.set(state.available_passengers);
    signals.ship_manifest.set(state.ship_manifest);
    signals.ship.set(state.ship.clone());
    signals.system_broker_skill.set(state.system_broker_skill);
    signals.illegal_goods.set(state.illegal_goods);

    info!("Trade state updated from server");
}
