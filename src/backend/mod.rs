//! # Server Module
//!
//! This module contains server-side functionality for the Worldgen application,
//! including Firestore integration and shared state management.
//!
//! The firebase specific pieces are only compiled when the `ssr` feature is enabled.
//! However, the state definition is shared between the server and client as is the key server-side function
//! update_trade_state (so that it has both a server and client implementation).
//!
//! ## Sessions
//!
//! The server supports multiple concurrent sessions. Sessions are lazily loaded from
//! Firestore on first access - when a server function needs session state, it's automatically
//! loaded from Firestore if not already in the cache. If the session doesn't exist in Firestore,
//! a default state is created and saved.
//!
//! ## WebSocket Signals
//!
//! This module provides server-to-client signals via `leptos_ws` for real-time updates.
//! Each signal is session-specific (prefixed with session_id):
//! - `dest_world` - The destination world (optional)
//! - `available_goods` - Available goods table for the current origin world
//! - `available_passengers` - Available passengers and freight
//! - `ship_manifest` - Ship manifest containing cargo, passengers, and profit
//! - `buyer_broker_skill` - Player's broker skill
//! - `seller_broker_skill` - System broker skill
//! - `steward_skill` - Steward skill
//! - `illegal_goods` - Whether to include illegal goods

#[cfg(feature = "ssr")]
pub mod firestore;

pub mod state;

#[cfg(feature = "ssr")]
use std::collections::HashMap;
#[cfg(feature = "ssr")]
use std::sync::{Arc, RwLock};

#[cfg(feature = "ssr")]
use log::{debug, error, info, warn};

use leptos::prelude::*;
use state::TradeState;

pub use crate::systems::world::World;
pub use crate::trade::available_goods::AvailableGoodsTable;
pub use crate::trade::available_passengers::AvailablePassengers;
pub use crate::trade::ship_manifest::ShipManifest;

/// Default session ID used by clients until proper session management is implemented
pub const DEFAULT_SESSION_ID: &str = "default";

/// Signal name constants for WebSocket signals (base names, will be prefixed with session_id)
pub mod signal_names {
    pub const ORIGIN_WORLD: &str = "origin_world";
    pub const DEST_WORLD: &str = "dest_world";
    pub const AVAILABLE_GOODS: &str = "available_goods";
    pub const AVAILABLE_PASSENGERS: &str = "available_passengers";
    pub const SHIP_MANIFEST: &str = "ship_manifest";
    pub const BUYER_BROKER_SKILL: &str = "buyer_broker_skill";
    pub const SELLER_BROKER_SKILL: &str = "seller_broker_skill";
    pub const STEWARD_SKILL: &str = "steward_skill";
    pub const ILLEGAL_GOODS: &str = "illegal_goods";

    /// Creates a session-specific signal name
    pub fn for_session(session_id: &str, signal_name: &str) -> String {
        format!("{}:{}", session_id, signal_name)
    }
}

/// Type alias for the session cache - maps session IDs to their cached TradeState
#[cfg(feature = "ssr")]
pub type SessionCache = Arc<RwLock<HashMap<String, TradeState>>>;

/// Internal function to update trade state (can be called from server-side code)
///
/// This function compares the incoming state with the cached state and only
/// performs a Firestore write if they differ, reducing unnecessary database operations.
///
/// # Arguments
/// * `session_id` - The session to update
/// * `state` - The new state to save
/// * `session_cache` - The session cache
/// * `firestore_db` - The Firestore database client
#[cfg(feature = "ssr")]
async fn update_trade_state_internal(
    session_id: String,
    state: TradeState,
    session_cache: SessionCache,
    firestore_db: ::firestore::FirestoreDb,
) -> Result<(), ServerFnError> {
    debug!("Updating trade state on session '{session_id}'");
    // Check if state has changed from cached version
    let should_save = {
        let cache = session_cache.read().unwrap();
        match cache.get(&session_id) {
            Some(cached) => {
                let changed = *cached != state;
                if changed {
                    info!("🔄 Trade state changed for session {}", session_id);
                } else {
                    info!(
                        "⏭️  Trade state unchanged for session {}, skipping Firestore write",
                        session_id
                    );
                }
                changed
            }
            None => {
                info!(
                    "🆕 Session {} not in cache, will save to Firestore",
                    session_id
                );
                true // Not in cache, should save
            }
        }
    };

    if should_save {
        info!(
            "💾 Saving trade state for session {} to Firestore",
            session_id
        );

        // Save to Firestore
        match firestore::save_trade_state(&firestore_db, &session_id, &state).await {
            Ok(_) => {
                info!(
                    "✅ Successfully saved trade state for session {} to Firestore",
                    session_id
                );
            }
            Err(e) => {
                error!(
                    "❌ Failed to save trade state for session {} to Firestore: {:?}",
                    session_id, e
                );
                return Err(ServerFnError::new(format!("Firestore error: {:?}", e)));
            }
        }

        // Update the cache
        let mut cache = session_cache.write().unwrap();
        cache.insert(session_id, state);
    }

    Ok(())
}

/// Updates the trade state for a specific session (server function wrapper)
///
/// This is the public server function that can be called from the client.
/// It delegates to the internal implementation.
///
/// # Arguments
/// * `session_id` - The session to update
/// * `state` - The new state to save
#[server]
pub async fn update_trade_state(
    session_id: String,
    state: TradeState,
) -> Result<(), ServerFnError> {
    let session_cache: SessionCache = use_context().ok_or_else(|| {
        error!("💥 Not finding session_cache in update_trade_state.");
        ServerFnError::new("session_cache not found in context")
    })?;

    let firestore_db: ::firestore::FirestoreDb = use_context().ok_or_else(|| {
        error!("💥 Not finding firestore_db in update_trade_state.");
        ServerFnError::new("firestore_db not found in context")
    })?;

    update_trade_state_internal(session_id, state, session_cache, firestore_db).await
}

// ============================================================================
// Signal Helper
// ============================================================================
//
// Helper function to create a ReadOnlySignal to update clients to changes to the trade state.
// Its used to disseminate changes to all clients.

use leptos::prelude::Signal;
use leptos_ws::ReadOnlySignal;
use serde::{Deserialize, Serialize};

/// Creates a reactive Signal derived from a ReadOnlySignal (client-side)
///
/// Wraps a leptos_ws ReadOnlySignal in a Leptos Signal::derive, making it:
/// - Reactive: accessing the signal tracks the underlying ReadOnlySignal
/// - Copy: can be moved into multiple closures without cloning
///
/// The ReadOnlySignal is only captured in one closure (inside Signal::derive),
/// avoiding move/borrow issues while maintaining full reactivity.
///
/// Note: The signal is read-only on the client. To update it, call a server function
/// which will use get_server_signal() to update the value.
pub fn get_signal<T>(
    session_id: &str,
    signal_name: &str,
    default: T,
) -> Signal<T>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static + PartialEq,
{
    let full_signal_name = signal_names::for_session(session_id, signal_name);
    log::debug!("📡 get_signal: Creating signal '{}'", full_signal_name);

    // Create the remote signal
    let ws_signal = ReadOnlySignal::new(&full_signal_name, default.clone()).unwrap_or_else(|e| {
        panic!("Creating signal {full_signal_name} failed: {e:?}");
    });

    // Wrap it in a derived signal - now it's Copy and reactive!
    Signal::derive(move || ws_signal.get())
}

/// Gets a ReadOnlySignal for server-side use (server-side only)
///
/// This is used by server functions to update signals that broadcast to all clients.
/// Unlike get_signal which returns a derived Signal for client use, this directly
/// returns the ReadOnlySignal so it can be modified with .set() or .update().
///
/// Note: ReadOnlySignal is NOT Copy, so it can only be used once or must be cloned.
#[cfg(feature = "ssr")]
pub fn get_server_signal<T>(session_id: &str, signal_name: &str, default: T) -> ReadOnlySignal<T>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static + PartialEq,
{
    let full_signal_name = signal_names::for_session(session_id, signal_name);

    // Create/get the remote signal
    ReadOnlySignal::new(&full_signal_name, default).unwrap_or_else(|e| {
        panic!("Creating signal {full_signal_name} failed: {e:?}");
    })
}

// ============================================================================
// WebSocket Signal Server Functions
// ============================================================================
//
// These server functions update the corresponding WebSocket signals, which
// automatically push updates to connected clients. Each function also updates
// the TradeState via update_trade_state, which handles caching and Firestore persistence.
// All signals are session-specific.

/// Helper to get the current trade state for a session from the cache
///
/// If the session is not in the cache, it will attempt to load it from Firestore.
/// If it doesn't exist in Firestore, it will create a default state and save it.
#[cfg(feature = "ssr")]
async fn get_session_state(session_id: &str) -> Result<TradeState, ServerFnError> {
    info!("🎯 Attempting to get session state for session '{session_id}'");
    let session_cache: SessionCache = use_context().ok_or_else(|| {
        error!("🔥 Unable to obtain session cache from context for session '{session_id}'!");
        ServerFnError::new("session_cache not found in context")
    })?;

    // First, check if it's in the cache
    {
        let cache = session_cache.read().unwrap();
        if let Some(state) = cache.get(session_id) {
            info!("✅ Found session '{session_id}' in cache");
            return Ok(state.clone());
        }
    }

    // Not in cache, try to load from Firestore
    info!("📥 Session '{session_id}' not in cache, loading from Firestore");

    let firestore_db: ::firestore::FirestoreDb = use_context().ok_or_else(|| {
        error!("🔥 Unable to obtain Firestore DB from context!");
        ServerFnError::new("firestore_db not found in context")
    })?;

    // get_trade_state will create a default and save it if it doesn't exist
    let state = firestore::get_trade_state(&firestore_db, session_id)
        .await
        .map_err(|e| {
            error!(
                "🔥 Failed to load session '{}' from Firestore: {:?}",
                session_id, e
            );
            ServerFnError::new(format!("Failed to load session from Firestore: {}", e))
        })?;

    // Add to cache for future requests
    {
        let mut cache = session_cache.write().unwrap();
        cache.insert(session_id.to_string(), state.clone());
        info!("✅ Loaded session '{session_id}' from Firestore and added to cache");
    }

    Ok(state)
}

/// Macro to generate setter functions for session state fields
///
/// Usage: `generate_setter!(set_dest_world, dest_world, DEST_WORLD, Option<World>, None);`
///
/// This generates a server function that:
/// 1. Updates the session state field
/// 2. Persists to Firestore
/// 3. Updates the WebSocket signal to broadcast to all clients
macro_rules! generate_setter {
    ($fn_name:ident, $field:ident, $signal_const:ident, $type:ty, $default:expr) => {
        #[server]
        pub async fn $fn_name(session_id: String, value: $type) -> Result<(), ServerFnError> {
            info!(
                "🚨 {} called for session {} via server function",
                stringify!($fn_name),
                session_id
            );

            // Update state and persist to Firestore
            let mut state = match get_session_state(&session_id).await {
                Ok(state) => state,
                Err(e) => {
                    error!(
                        "🔥 Failed to get session state for session '{}' in {}: {:?}",
                        session_id,
                        stringify!($fn_name),
                        e
                    );
                    return Err(e);
                }
            };
            state.$field = value.clone();

            info!(
                "Updating trade state in {}.",
                signal_names::for_session(&session_id, signal_names::$signal_const)
            );
            let res = update_trade_state(session_id.clone(), state).await;
            info!(
                "Trade state updated in {}.  Try to set signal",
                signal_names::for_session(&session_id, signal_names::$signal_const)
            );

            // Update WebSocket signal to broadcast to all clients
            info!(
                "🚨 Setting signal {} to {:?}.",
                signal_names::for_session(&session_id, signal_names::$signal_const),
                value
            );
            let signal = get_server_signal(&session_id, signal_names::$signal_const, $default);
            signal.set(value);

            info!(
                "Signal set in {}.",
                signal_names::for_session(&session_id, signal_names::$signal_const)
            );

            res
        }
    };
}

// Generate setter functions for all session state fields
generate_setter!(
    set_origin_world,
    origin_world,
    ORIGIN_WORLD,
    World,
    World::default()
);

generate_setter!(set_dest_world, dest_world, DEST_WORLD, Option<World>, None);
generate_setter!(
    set_buyer_broker_skill,
    buyer_broker_skill,
    BUYER_BROKER_SKILL,
    i16,
    0i16
);
generate_setter!(
    set_seller_broker_skill,
    seller_broker_skill,
    SELLER_BROKER_SKILL,
    i16,
    0i16
);
generate_setter!(set_steward_skill, steward_skill, STEWARD_SKILL, i16, 0i16);
generate_setter!(set_illegal_goods, illegal_goods, ILLEGAL_GOODS, bool, false);
generate_setter!(
    set_available_goods,
    available_goods,
    AVAILABLE_GOODS,
    AvailableGoodsTable,
    AvailableGoodsTable::default()
);
generate_setter!(
    set_available_passengers,
    available_passengers,
    AVAILABLE_PASSENGERS,
    Option<AvailablePassengers>,
    None
);
generate_setter!(
    set_ship_manifest,
    ship_manifest,
    SHIP_MANIFEST,
    ShipManifest,
    ShipManifest::default()
);
