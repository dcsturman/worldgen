//! # Trade State WebSocket Client
//!
//! This module provides a WebSocket client for syncing trade state
//! with the server and other connected clients.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;
use log::{debug, error, info, warn};
use wasm_bindgen::prelude::*;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use super::TradeState;
use crate::systems::world::World;
use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::available_passengers::AvailablePassengers;
use crate::trade::ship_manifest::ShipManifest;

/// Holds the write signals for all trade state fields
#[derive(Clone)]
pub struct TradeSignals {
    pub origin_world: WriteSignal<World>,
    pub dest_world: WriteSignal<Option<World>>,
    pub available_goods: WriteSignal<AvailableGoodsTable>,
    pub available_passengers: WriteSignal<Option<AvailablePassengers>>,
    pub ship_manifest: WriteSignal<ShipManifest>,
    pub buyer_broker_skill: WriteSignal<i16>,
    pub seller_broker_skill: WriteSignal<i16>,
    pub steward_skill: WriteSignal<i16>,
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
        let ws = WebSocket::new(server_url).map_err(|e| format!("Failed to create WebSocket: {:?}", e))?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let signals: Rc<RefCell<Option<TradeSignals>>> = Rc::new(RefCell::new(None));
        let last_received_state: Rc<RefCell<Option<TradeState>>> = Rc::new(RefCell::new(None));

        // Set up message handler
        let signals_clone = signals.clone();
        let last_received_clone = last_received_state.clone();
        let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            if let Some(text) = e.data().as_string() {
                handle_message(&text, &signals_clone, &last_received_clone);
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

        // Set up open handler
        let onopen_callback = Closure::<dyn FnMut()>::new(move || {
            info!("WebSocket connection established");
        });
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        // Set up close handler
        let onclose_callback = Closure::<dyn FnMut(_)>::new(move |e: CloseEvent| {
            info!("WebSocket connection closed: code={}, reason={}", e.code(), e.reason());
        });
        ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        onclose_callback.forget();

        Ok(Self { ws, signals, last_received_state })
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
}

/// Compare two TradeState instances for equality (ignoring version)
fn states_equal(a: &TradeState, b: &TradeState) -> bool {
    // Compare all fields except version
    a.origin_world == b.origin_world
        && a.dest_world == b.dest_world
        && a.available_goods == b.available_goods
        && a.available_passengers == b.available_passengers
        && a.ship_manifest == b.ship_manifest
        && a.buyer_broker_skill == b.buyer_broker_skill
        && a.seller_broker_skill == b.seller_broker_skill
        && a.steward_skill == b.steward_skill
        && a.illegal_goods == b.illegal_goods
}

/// Handle incoming WebSocket messages
fn handle_message(
    text: &str,
    signals: &Rc<RefCell<Option<TradeSignals>>>,
    last_received: &Rc<RefCell<Option<TradeState>>>,
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

    // Store the received state so we can detect echoes
    *last_received.borrow_mut() = Some(state.clone());

    // Update each signal with the new state values
    // Using set() which will trigger reactivity only if the value changed
    signals.origin_world.set(state.origin_world);
    signals.dest_world.set(state.dest_world);
    signals.available_goods.set(state.available_goods);
    signals.available_passengers.set(state.available_passengers);
    signals.ship_manifest.set(state.ship_manifest);
    signals.buyer_broker_skill.set(state.buyer_broker_skill);
    signals.seller_broker_skill.set(state.seller_broker_skill);
    signals.steward_skill.set(state.steward_skill);
    signals.illegal_goods.set(state.illegal_goods);

    info!("Trade state updated from server");
}

