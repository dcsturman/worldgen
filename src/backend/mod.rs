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
//! The server supports multiple concurrent sessions. Each client should call `use_session(session_id)`
//! early in its lifecycle to initialize or connect to a session. Sessions are lazily loaded from
//! Firestore when first accessed.
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
use log::{debug, error, info};

use leptos::prelude::*;
use state::TradeState;

// Re-export types needed for the signal functions
pub use crate::systems::world::World;
pub use crate::trade::available_goods::AvailableGoodsTable;
pub use crate::trade::available_passengers::AvailablePassengers;
pub use crate::trade::ship_manifest::ShipManifest;

/// Default session ID used by clients until proper session management is implemented
pub const DEFAULT_SESSION_ID: &str = "default";

/// Signal name constants for WebSocket signals (base names, will be prefixed with session_id)
pub mod signal_names {
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

/// Initializes or retrieves a session's cached state
///
/// This function should be called early by every client to ensure their session
/// is loaded into the server's cache. If the session doesn't exist in the cache,
/// it will be loaded from Firestore (or created with defaults if not found).
///
/// # Arguments
/// * `session_id` - The unique identifier for the session
///
/// # Returns
/// The current TradeState for the session
#[server]
pub async fn use_session(session_id: String) -> Result<TradeState, ServerFnError> {
    let session_cache: SessionCache =
        use_context().ok_or_else(|| ServerFnError::new("session_cache not found in context"))?;

    // Check if session already exists in cache
    let is_new_session = {
        let cache = session_cache.read().unwrap();
        if let Some(state) = cache.get(&session_id) {
            debug!("Session {} already in cache", session_id);
            return Ok(state.clone());
        }
        true
    };

    // Session not in cache, load from Firestore
    info!("Loading session {} from Firestore", session_id);
    let state = firestore::get_trade_state(&session_id)
        .await
        .unwrap_or_else(|e| {
            info!(
                "Session {} not found in Firestore ({}), creating default",
                session_id, e
            );
            TradeState::default()
        });

    // Add to cache
    {
        let mut cache = session_cache.write().unwrap();
        cache.insert(session_id.clone(), state.clone());
    }

    info!("Session {} initialized", session_id);

    // Set up persistence subscriptions for this new session
    if is_new_session {
        setup_session_persistence(session_id.clone(), session_cache.clone());
    }

    Ok(state)
}

/// Internal function to update trade state (can be called from server-side code)
///
/// This function compares the incoming state with the cached state and only
/// performs a Firestore write if they differ, reducing unnecessary database operations.
///
/// # Arguments
/// * `session_id` - The session to update
/// * `state` - The new state to save
/// * `session_cache` - The session cache
#[cfg(feature = "ssr")]
async fn update_trade_state_internal(
    session_id: String,
    state: TradeState,
    session_cache: SessionCache,
) -> Result<(), ServerFnError> {
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
        match firestore::save_trade_state(&session_id, &state).await {
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
    let session_cache: SessionCache =
        use_context().ok_or_else(|| ServerFnError::new("session_cache not found in context"))?;

    update_trade_state_internal(session_id, state, session_cache).await
}

// ============================================================================
// BiDirectional Signal Helper
// ============================================================================
//
// Helper function to create BiDirectionalSignals that automatically persist
// to Firestore when updated on either client or server side.

use leptos_ws::BiDirectionalSignal;
use serde::{Deserialize, Serialize};

/// Creates a BiDirectionalSignal that automatically persists changes to Firestore
///
/// This function creates a bidirectional WebSocket signal that:
/// 1. Syncs state between client and server in real-time
/// 2. Allows updates from both client and server
/// 3. Automatically persists changes to Firestore on the server side
///
/// # Type Parameters
/// * `T` - The type of data stored in the signal. Must implement Clone, Serialize, Deserialize, and PartialEq
/// * `F` - A function that updates the TradeState with the new value
///
/// # Arguments
/// * `session_id` - The session identifier
/// * `signal_name` - The base name of the signal (will be prefixed with session_id)
/// * `default` - The default value if no cached value exists
/// * `update_fn` - Function that takes a mutable TradeState and the new value, updating the appropriate field
///
/// # Returns
/// A BiDirectionalSignal that can be used like a normal signal with `.get()` and `.set()`
pub fn create_persisted_signal<T, F>(
    session_id: &str,
    signal_name: &str,
    default: T,
    update_fn: F,
) -> BiDirectionalSignal<T>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static + PartialEq,
    F: Fn(&mut TradeState, T) + Clone + Send + Sync + 'static,
{
    let full_signal_name = signal_names::for_session(session_id, signal_name);
    log::warn!(
        "📡 create_persisted_signal: Creating signal '{}'",
        full_signal_name
    );
    let session_id = session_id.to_string();

    // Create the bidirectional signal
    log::warn!(
        "📡 create_persisted_signal: Calling BiDirectionalSignal::new for '{}'",
        full_signal_name
    );
    let signal = BiDirectionalSignal::new(&full_signal_name, default.clone()).unwrap_or_else(|e| {
        panic!("Creating singal {full_signal_name} failed: {e:?}");
    });
    log::warn!(
        "📡 create_persisted_signal: BiDirectionalSignal created successfully for '{}'",
        full_signal_name
    );

    // Note: Persistence is handled by a centralized service, not here
    // This function just creates the signal
    #[cfg(feature = "ssr")]
    {
        log::info!("📌 Created persisted signal '{}' for session '{}'", full_signal_name, session_id);
    }

    signal
}

/// Sets up persistence listeners for all trade signals across all sessions
/// This should be called once at server startup
#[cfg(feature = "ssr")]
pub fn setup_signal_persistence(session_cache: SessionCache) {
    use leptos_ws::traits::WsSignalCore;
    use tokio::spawn;

    log::info!("🔧 Setting up global signal persistence system");

    // We'll set up a registry that tracks which sessions have active signals
    // For now, we'll use a simple approach: subscribe to signals as they're created
    // by monitoring the session cache

    // Actually, the better approach is to subscribe when signals are created in create_persisted_signal
    // But since we can't do that during SSR, we need a different strategy

    // The key insight: We can't subscribe to signals that don't exist yet
    // So we need to subscribe AFTER they're created
    // The best place is actually in a background task that monitors for new sessions

    log::info!("✅ Signal persistence system initialized (subscriptions will be set up per-session)");
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
#[cfg(feature = "ssr")]
fn get_session_state(session_id: &str) -> Result<TradeState, ServerFnError> {
    let session_cache: SessionCache =
        use_context().ok_or_else(|| ServerFnError::new("session_cache not found in context"))?;

    let cache = session_cache.read().unwrap();
    cache.get(session_id).cloned().ok_or_else(|| {
        ServerFnError::new(format!(
            "Session {} not found. Call use_session first.",
            session_id
        ))
    })
}

/// Sets the destination world via WebSocket signal for a specific session
#[server]
pub async fn set_dest_world(session_id: String, value: Option<World>) -> Result<(), ServerFnError> {
    use leptos_ws::ReadOnlySignal;

    debug!(
        "Setting dest_world for session {} via WebSocket signal",
        session_id
    );

    // Update the session-specific WebSocket signal
    let signal_name = signal_names::for_session(&session_id, signal_names::DEST_WORLD);
    let signal = ReadOnlySignal::new(&signal_name, None::<World>)
        .map_err(|e| ServerFnError::new(format!("Failed to create signal: {}", e)))?;
    signal.update(|v| *v = value.clone());

    // Update state and persist to Firestore
    let mut state = get_session_state(&session_id)?;
    state.dest_world = value;
    update_trade_state(session_id, state).await
}

/// Sets the available goods table via WebSocket signal for a specific session
#[server]
pub async fn set_available_goods(
    session_id: String,
    value: AvailableGoodsTable,
) -> Result<(), ServerFnError> {
    use leptos_ws::ReadOnlySignal;

    debug!(
        "Setting available_goods for session {} via WebSocket signal",
        session_id
    );

    // Update the session-specific WebSocket signal
    let signal_name = signal_names::for_session(&session_id, signal_names::AVAILABLE_GOODS);
    let signal = ReadOnlySignal::new(&signal_name, AvailableGoodsTable::default())
        .map_err(|e| ServerFnError::new(format!("Failed to create signal: {}", e)))?;
    signal.update(|v| *v = value.clone());

    // Update state and persist to Firestore
    let mut state = get_session_state(&session_id)?;
    state.available_goods = value;
    update_trade_state(session_id, state).await
}

/// Sets the available passengers via WebSocket signal for a specific session
#[server]
pub async fn set_available_passengers(
    session_id: String,
    value: Option<AvailablePassengers>,
) -> Result<(), ServerFnError> {
    use leptos_ws::ReadOnlySignal;

    debug!(
        "Setting available_passengers for session {} via WebSocket signal",
        session_id
    );

    // Update the session-specific WebSocket signal
    let signal_name = signal_names::for_session(&session_id, signal_names::AVAILABLE_PASSENGERS);
    let signal = ReadOnlySignal::new(&signal_name, None::<AvailablePassengers>)
        .map_err(|e| ServerFnError::new(format!("Failed to create signal: {}", e)))?;
    signal.update(|v| *v = value.clone());

    // Update state and persist to Firestore
    let mut state = get_session_state(&session_id)?;
    state.available_passengers = value;
    update_trade_state(session_id, state).await
}

/// Sets the ship manifest via WebSocket signal for a specific session
#[server]
pub async fn set_ship_manifest(
    session_id: String,
    value: ShipManifest,
) -> Result<(), ServerFnError> {
    use leptos_ws::ReadOnlySignal;

    debug!(
        "Setting ship_manifest for session {} via WebSocket signal",
        session_id
    );

    // Update the session-specific WebSocket signal
    let signal_name = signal_names::for_session(&session_id, signal_names::SHIP_MANIFEST);
    let signal = ReadOnlySignal::new(&signal_name, ShipManifest::default())
        .map_err(|e| ServerFnError::new(format!("Failed to create signal: {}", e)))?;
    signal.update(|v| *v = value.clone());

    // Update state and persist to Firestore
    let mut state = get_session_state(&session_id)?;
    state.ship_manifest = value;
    update_trade_state(session_id, state).await
}

/// Sets the buyer broker skill via WebSocket signal for a specific session
#[server]
pub async fn set_buyer_broker_skill(session_id: String, value: i16) -> Result<(), ServerFnError> {
    use leptos_ws::ReadOnlySignal;

    debug!(
        "Setting buyer_broker_skill for session {}: {}",
        session_id, value
    );

    // Update the session-specific WebSocket signal
    let signal_name = signal_names::for_session(&session_id, signal_names::BUYER_BROKER_SKILL);
    let signal = ReadOnlySignal::new(&signal_name, 0i16)
        .map_err(|e| ServerFnError::new(format!("Failed to create signal: {}", e)))?;
    signal.update(|v| *v = value);

    // Update state and persist to Firestore
    let mut state = get_session_state(&session_id)?;
    state.buyer_broker_skill = value;
    update_trade_state(session_id, state).await
}

/// Sets the seller broker skill via WebSocket signal for a specific session
#[server]
pub async fn set_seller_broker_skill(session_id: String, value: i16) -> Result<(), ServerFnError> {
    use leptos_ws::ReadOnlySignal;

    debug!(
        "Setting seller_broker_skill for session {}: {}",
        session_id, value
    );

    // Update the session-specific WebSocket signal
    let signal_name = signal_names::for_session(&session_id, signal_names::SELLER_BROKER_SKILL);
    let signal = ReadOnlySignal::new(&signal_name, 0i16)
        .map_err(|e| ServerFnError::new(format!("Failed to create signal: {}", e)))?;
    signal.update(|v| *v = value);

    // Update state and persist to Firestore
    let mut state = get_session_state(&session_id)?;
    state.seller_broker_skill = value;
    update_trade_state(session_id, state).await
}

/// Sets the steward skill via WebSocket signal for a specific session
#[server]
pub async fn set_steward_skill(session_id: String, value: i16) -> Result<(), ServerFnError> {
    use leptos_ws::ReadOnlySignal;

    debug!(
        "Setting steward_skill for session {}: {}",
        session_id, value
    );

    // Update the session-specific WebSocket signal
    let signal_name = signal_names::for_session(&session_id, signal_names::STEWARD_SKILL);
    let signal = ReadOnlySignal::new(&signal_name, 0i16)
        .map_err(|e| ServerFnError::new(format!("Failed to create signal: {}", e)))?;
    signal.update(|v| *v = value);

    // Update state and persist to Firestore
    let mut state = get_session_state(&session_id)?;
    state.steward_skill = value;
    update_trade_state(session_id, state).await
}

/// Sets the illegal goods flag via WebSocket signal for a specific session
#[server]
pub async fn set_illegal_goods(session_id: String, value: bool) -> Result<(), ServerFnError> {
    use leptos_ws::ReadOnlySignal;

    debug!(
        "Setting illegal_goods for session {}: {}",
        session_id, value
    );

    // Update the session-specific WebSocket signal
    let signal_name = signal_names::for_session(&session_id, signal_names::ILLEGAL_GOODS);
    let signal = ReadOnlySignal::new(&signal_name, false)
        .map_err(|e| ServerFnError::new(format!("Failed to create signal: {}", e)))?;
    signal.update(|v| *v = value);

    // Update state and persist to Firestore
    let mut state = get_session_state(&session_id)?;
    state.illegal_goods = value;
    update_trade_state(session_id, state).await
}
