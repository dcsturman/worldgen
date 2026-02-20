//! # Trade State WebSocket Server
//!
//! This module provides a WebSocket server for handling trade state updates
//! and broadcasting them to connected clients.
//!
//! The server is authoritative for trade table generation and pricing calculations.
//! When clients send state updates with changed world names/UWPs or skills, the server
//! recalculates the trade table and prices before broadcasting to all clients.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use firestore::FirestoreDb;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc};
use tokio_tungstenite::tungstenite::Message;

use crate::backend::TradeState;
use crate::backend::firestore::{
    FirestoreError, get_trade_state, initialize_firestore, save_trade_state,
};
use crate::comms::{ServerCommand, ServerMessage};
use crate::systems::world::World;
use crate::trade::available_goods::AvailableGoodsTable;
use crate::trade::table::TradeTable;
use crate::util::calculate_hex_distance;

/// Unique identifier for connected clients
type ClientId = u64;

/// Sender half of a channel for sending messages to a specific client
type ClientSender = mpsc::UnboundedSender<Message>;

/// Shared state containing all connected clients
type Clients = Arc<RwLock<HashMap<ClientId, ClientSender>>>;

/// Database wrapped in Arc for sharing across tasks
type SharedDb = Arc<Option<FirestoreDb>>;

/// Current trade state wrapped in Arc for sharing across tasks
type SharedState = Arc<RwLock<Option<TradeState>>>;

/// Default session ID for shared state (all users see the same state)
pub const DEFAULT_SESSION: &str = "default";

/// The trade state server that manages WebSocket connections and state broadcasting
pub struct TradeServer {
    /// Address the server listens on
    addr: SocketAddr,
    /// Connected clients
    clients: Clients,
    /// Counter for generating unique client IDs
    next_client_id: Arc<RwLock<ClientId>>,
    /// Firestore database connection (None if running in debug mode without Firestore)
    db: SharedDb,
    /// Current trade state (used to detect changes and recalculate)
    current_state: SharedState,
}

impl TradeServer {
    /// Creates a new TradeServer bound to the specified address
    ///
    /// Initializes the Firestore database connection using environment variables.
    /// If the database ID is set to "debug", the server runs without a Firestore connection.
    ///
    /// # Arguments
    ///
    /// * `addr` - The socket address to listen on (e.g., "127.0.0.1:8080")
    ///
    /// # Errors
    ///
    /// Returns `FirestoreError` if the Firestore initialization fails
    pub async fn new(addr: SocketAddr) -> Result<Self, FirestoreError> {
        let db = initialize_firestore().await?;

        Ok(Self {
            addr,
            clients: Arc::new(RwLock::new(HashMap::new())),
            next_client_id: Arc::new(RwLock::new(0)),
            db: Arc::new(db),
            current_state: Arc::new(RwLock::new(None)),
        })
    }

    /// Returns a reference to the Firestore database connection
    pub fn db(&self) -> &Option<FirestoreDb> {
        &self.db
    }

    /// Starts the WebSocket server and begins accepting connections
    ///
    /// This method runs indefinitely, accepting new connections and spawning
    /// tasks to handle each client.
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(&self.addr).await?;
        log::info!("Trade server listening on: {}", self.addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let clients = self.clients.clone();
            let next_id = self.next_client_id.clone();
            let db = self.db.clone();
            let current_state = self.current_state.clone();

            tokio::spawn(async move {
                if let Err(e) =
                    handle_connection(stream, addr, clients, next_id, db, current_state).await
                {
                    log::error!("Error handling connection from {}: {}", addr, e);
                }
            });
        }

        Ok(())
    }

    /// Broadcasts a TradeState update to all connected clients
    ///
    /// # Arguments
    ///
    /// * `state` - The TradeState to broadcast to all clients
    ///
    /// # Returns
    ///
    /// The number of clients the message was successfully queued for
    pub async fn update_clients(&self, state: &TradeState) -> usize {
        broadcast_to_clients(&self.clients, state, None).await
    }

    /// Returns the number of currently connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}

/// Handles a single WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    clients: Clients,
    next_id: Arc<RwLock<ClientId>>,
    db: SharedDb,
    current_state: SharedState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    log::info!("WebSocket connection established: {}", addr);

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Generate a unique client ID
    let client_id = {
        let mut id = next_id.write().await;
        let current_id = *id;
        *id += 1;
        current_id
    };

    // Create a channel for sending messages to this client
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Register the client
    {
        let mut clients_guard = clients.write().await;
        clients_guard.insert(client_id, tx.clone());
    }

    log::info!("Client {} connected from {}", client_id, addr);

    // Load the current state from Firestore and send it to the new client
    // Also update the shared current_state if we loaded from Firestore
    match get_trade_state(&db, DEFAULT_SESSION).await {
        Ok(state) => {
            // Update the shared current state
            {
                let mut state_guard = current_state.write().await;
                *state_guard = Some(state.clone());
            }
            match serde_json::to_string(&state) {
                Ok(json) => {
                    if tx.send(Message::Text(json.into())).is_ok() {
                        log::info!("Sent initial state to client {}", client_id);
                    } else {
                        log::warn!("Failed to queue initial state for client {}", client_id);
                    }
                }
                Err(e) => {
                    log::error!(
                        "Failed to serialize initial state for client {}: {}",
                        client_id,
                        e
                    );
                }
            }
        }
        Err(FirestoreError::SchemaError(e)) => {
            // Schema mismatch - use default state and overwrite the old document
            log::warn!(
                "Schema mismatch detected: {}. Using default state and overwriting old document.",
                e
            );
            let default_state = TradeState::default();
            if let Err(save_err) = save_trade_state(&db, DEFAULT_SESSION, &default_state).await {
                log::error!(
                    "Failed to save default state after schema error: {}",
                    save_err
                );
            }
            // Update the shared current state
            {
                let mut state_guard = current_state.write().await;
                *state_guard = Some(default_state.clone());
            }
            // Send the default state to the client
            match serde_json::to_string(&default_state) {
                Ok(json) => {
                    if tx.send(Message::Text(json.into())).is_ok() {
                        log::info!(
                            "Sent default state to client {} after schema migration",
                            client_id
                        );
                    } else {
                        log::warn!("Failed to queue default state for client {}", client_id);
                    }
                }
                Err(e) => {
                    log::error!(
                        "Failed to serialize default state for client {}: {}",
                        client_id,
                        e
                    );
                }
            }
        }
        Err(e) => {
            log::error!(
                "Failed to load initial state for client {}: {}",
                client_id,
                e
            );
        }
    }

    // Spawn a task to forward messages from the channel to the WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Process incoming messages
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Try to parse as a ServerMessage (which can be either a state update or a command)
                match serde_json::from_str::<ServerMessage>(&text) {
                    Ok(ServerMessage::StateUpdate(trade_state)) => {
                        handle_trade_state_update(trade_state, &db, &clients, &current_state).await;
                    }
                    Ok(ServerMessage::Command(ServerCommand::Regenerate)) => {
                        handle_regenerate_command(&db, &clients, &current_state).await;
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to deserialize message from client {}: {}",
                            client_id,
                            e
                        );
                    }
                }
            }
            Ok(Message::Close(_)) => {
                log::info!("Client {} requested close", client_id);
                break;
            }
            Ok(Message::Ping(data)) => {
                // Pong is handled automatically by tungstenite
                log::trace!("Received ping from client {}: {:?}", client_id, data);
            }
            Ok(Message::Pong(_)) => {
                log::trace!("Received pong from client {}", client_id);
            }
            Ok(_) => {
                // Ignore binary and other message types
            }
            Err(e) => {
                log::error!("Error receiving message from client {}: {}", client_id, e);
                break;
            }
        }
    }

    // Remove the client from the list
    {
        let mut clients_guard = clients.write().await;
        clients_guard.remove(&client_id);
    }

    // Abort the send task
    send_task.abort();

    log::info!("Client {} disconnected", client_id);
    Ok(())
}

/// Broadcasts a TradeState to all connected clients, optionally excluding one client
///
/// # Arguments
///
/// * `clients` - The shared clients map
/// * `state` - The TradeState to broadcast
/// * `exclude_client` - Optional client ID to exclude from the broadcast
///
/// # Returns
///
/// The number of clients the message was successfully queued for
async fn broadcast_to_clients(
    clients: &Clients,
    state: &TradeState,
    exclude_client: Option<ClientId>,
) -> usize {
    let json = match serde_json::to_string(state) {
        Ok(j) => j,
        Err(e) => {
            log::error!("Failed to serialize TradeState: {}", e);
            return 0;
        }
    };

    let message = Message::Text(json.into());
    let clients_guard = clients.read().await;
    let mut sent_count = 0;

    for (client_id, sender) in clients_guard.iter() {
        // Skip the excluded client (the one who sent the update)
        if exclude_client == Some(*client_id) {
            continue;
        }

        if sender.send(message.clone()).is_ok() {
            sent_count += 1;
        } else {
            log::warn!("Failed to queue message for client {}", client_id);
        }
    }

    log::debug!("Broadcast TradeState to {} clients", sent_count);
    sent_count
}

/// Handler for processing received TradeState updates
///
/// The server is authoritative for trade table generation and pricing.
/// When world names/UWPs or skills change, the server recalculates:
/// - AvailableGoodsTable when origin world changes
/// - Buy/sell prices when skills or worlds change
/// - Ship manifest prices when destination or skills change
/// - Available passengers when worlds, distance, or skills change
///
/// After recalculation, the updated state is broadcast to ALL clients (including sender).
///
/// # Arguments
///
/// * `state` - The received TradeState from a client
/// * `db` - The Firestore database connection
/// * `clients` - The shared clients map
/// * `current_state` - The shared current state for detecting changes
async fn handle_trade_state_update(
    mut state: TradeState,
    db: &SharedDb,
    clients: &Clients,
    current_state: &SharedState,
) {
    // Get the previous state to detect what changed
    let prev_state = {
        let state_guard = current_state.read().await;
        state_guard.clone()
    };

    // Detect what changed and recalculate as needed
    let recalculated = recalculate_trade_state(&mut state, prev_state.as_ref());

    if recalculated {
        log::info!("Server recalculated trade state due to world/skill changes");
    }

    // Update the shared current state
    {
        let mut state_guard = current_state.write().await;
        *state_guard = Some(state.clone());
    }

    // Save to Firestore
    if let Err(e) = save_trade_state(db, DEFAULT_SESSION, &state).await {
        log::error!("Failed to save trade state to Firestore: {}", e);
        // Continue to broadcast even if Firestore save fails
    }

    // Broadcast to ALL clients (server is authoritative, so sender gets the recalculated state too)
    let sent_count = broadcast_to_clients(clients, &state, None).await;
    log::info!(
        "Broadcast trade state update to {} clients (recalculated: {})",
        sent_count,
        recalculated
    );
}

/// Recalculates trade state when world names/UWPs or skills change
///
/// Returns true if any recalculation was performed.
fn recalculate_trade_state(state: &mut TradeState, prev_state: Option<&TradeState>) -> bool {
    let mut recalculated = false;

    // Parse origin world from name/UWP, setting coordinates and zone
    let origin_world = if !state.origin_world_name.is_empty() && state.origin_uwp.len() == 9 {
        match World::from_upp(&state.origin_world_name, &state.origin_uwp, false, false) {
            Ok(mut world) => {
                world.gen_trade_classes();
                // Set coordinates and zone from state
                world.coordinates = state.origin_coords;
                world.travel_zone = state.origin_zone;
                Some(world)
            }
            Err(e) => {
                log::error!("Failed to parse origin UWP '{}': {}", state.origin_uwp, e);
                None
            }
        }
    } else {
        None
    };

    // Parse destination world from name/UWP, setting coordinates and zone
    let dest_world = if !state.dest_world_name.is_empty() && state.dest_uwp.len() == 9 {
        match World::from_upp(&state.dest_world_name, &state.dest_uwp, false, false) {
            Ok(mut world) => {
                world.gen_trade_classes();
                // Set coordinates and zone from state
                world.coordinates = state.dest_coords;
                world.travel_zone = state.dest_zone;
                Some(world)
            }
            Err(e) => {
                log::error!("Failed to parse dest UWP '{}': {}", state.dest_uwp, e);
                None
            }
        }
    } else {
        None
    };

    // Calculate distance from coordinates if both are available
    let distance = match (state.origin_coords, state.dest_coords) {
        (Some((ox, oy)), Some((dx, dy))) => calculate_hex_distance(ox, oy, dx, dy),
        _ => 0,
    };

    // Check if origin world changed (need to regenerate trade table)
    let origin_changed = prev_state.is_none()
        || prev_state.is_some_and(|prev| {
            prev.origin_world_name != state.origin_world_name
                || prev.origin_uwp != state.origin_uwp
                || prev.origin_coords != state.origin_coords
                || prev.origin_zone != state.origin_zone
                || prev.illegal_goods != state.illegal_goods
        });

    // Check if destination world changed
    let dest_changed = prev_state.is_none()
        || prev_state.is_some_and(|prev| {
            prev.dest_world_name != state.dest_world_name
                || prev.dest_uwp != state.dest_uwp
                || prev.dest_coords != state.dest_coords
                || prev.dest_zone != state.dest_zone
        });

    // Check if skills changed
    let skills_changed = prev_state.is_none()
        || prev_state.is_some_and(|prev| {
            prev.buyer_broker_skill != state.buyer_broker_skill
                || prev.seller_broker_skill != state.seller_broker_skill
                || prev.steward_skill != state.steward_skill
        });

    // Regenerate trade table if origin world changed
    if origin_changed && let Some(ref world) = origin_world {
        match AvailableGoodsTable::for_world(
            TradeTable::global(),
            &world.get_trade_classes(),
            world.get_population(),
            state.illegal_goods,
        ) {
            Ok(new_table) => {
                state.available_goods = new_table;
                recalculated = true;
                log::info!(
                    "Regenerated trade table for origin world: {}",
                    state.origin_world_name
                );
            }
            Err(e) => {
                log::error!("Failed to generate trade table: {}", e);
            }
        }
    }

    // Reprice goods if origin changed, dest changed, or skills changed
    if (origin_changed || dest_changed || skills_changed)
        && let Some(world) = origin_world.as_ref()
    {
        // Price goods to buy at origin
        state.available_goods.price_goods_to_buy(
            &world.get_trade_classes(),
            state.buyer_broker_skill,
            state.seller_broker_skill,
        );

        // Price goods to sell at destination
        let dest_trade_classes = dest_world.as_ref().map(|w| w.get_trade_classes());
        state.available_goods.price_goods_to_sell(
            dest_trade_classes,
            state.seller_broker_skill,
            state.buyer_broker_skill,
        );

        state.available_goods.sort_by_discount();
        log::info!("Repriced available goods");

        // Reprice ship manifest goods
        state.ship_manifest.price_goods(
            &origin_world,
            state.buyer_broker_skill,
            state.seller_broker_skill,
        );
        recalculated = true;
        log::info!("Repriced ship manifest");

        // Regenerate passengers if worlds or skills changed and we have both worlds
        if let Some(dest) = &dest_world {
            let mut passengers = state.available_passengers.take().unwrap_or_default();
            // Reset die rolls so we get fresh random values
            passengers.reset_die_rolls();

            passengers.generate(
                world.get_population(),
                world.port,
                world.travel_zone,
                world.tech_level,
                dest.get_population(),
                dest.port,
                dest.travel_zone,
                dest.tech_level,
                distance,
                state.steward_skill as i32,
                state.buyer_broker_skill as i32,
            );

            state.available_passengers = Some(passengers);
            recalculated = true;
            log::info!(
                "Regenerated passengers for route {} -> {} (distance: {} parsecs)",
                state.origin_world_name,
                state.dest_world_name,
                distance
            );
        }
    }

    // Store the generated world objects in the state so they're sent back to clients
    // This ensures the client and server always have the same World objects
    state.origin_world = origin_world;
    state.dest_world = dest_world;

    recalculated
}

/// Handles a regenerate command from a client
///
/// This re-rolls all random values (prices, passengers) without changing the state.
/// It's used when the user clicks the "Generate" button to get different random values.
async fn handle_regenerate_command(db: &SharedDb, clients: &Clients, current_state: &SharedState) {
    let mut state = match current_state.write().await.take() {
        Some(s) => s,
        None => {
            log::warn!("Received regenerate command but no state available");
            return;
        }
    };

    // Parse origin world
    let origin_world = if !state.origin_world_name.is_empty() && state.origin_uwp.len() == 9 {
        match World::from_upp(&state.origin_world_name, &state.origin_uwp, false, false) {
            Ok(mut world) => {
                world.gen_trade_classes();
                world.coordinates = state.origin_coords;
                world.travel_zone = state.origin_zone;
                Some(world)
            }
            Err(e) => {
                log::error!("Failed to parse origin UWP '{}': {}", state.origin_uwp, e);
                None
            }
        }
    } else {
        None
    };

    // Parse destination world
    let dest_world = if !state.dest_world_name.is_empty() && state.dest_uwp.len() == 9 {
        match World::from_upp(&state.dest_world_name, &state.dest_uwp, false, false) {
            Ok(mut world) => {
                world.gen_trade_classes();
                world.coordinates = state.dest_coords;
                world.travel_zone = state.dest_zone;
                Some(world)
            }
            Err(e) => {
                log::error!("Failed to parse dest UWP '{}': {}", state.dest_uwp, e);
                None
            }
        }
    } else {
        None
    };

    // Calculate distance
    let distance = match (state.origin_coords, state.dest_coords) {
        (Some((ox, oy)), Some((dx, dy))) => calculate_hex_distance(ox, oy, dx, dy),
        _ => 0,
    };

    // Regenerate trade table with fresh die rolls
    if let Some(ref world) = origin_world {
        match AvailableGoodsTable::for_world(
            TradeTable::global(),
            &world.get_trade_classes(),
            world.get_population(),
            state.illegal_goods,
        ) {
            Ok(mut new_table) => {
                // Reset die rolls to get fresh random values
                new_table.reset_die_rolls();
                new_table.price_goods_to_buy(
                    &world.get_trade_classes(),
                    state.buyer_broker_skill,
                    state.seller_broker_skill,
                );
                let dest_trade_classes = dest_world.as_ref().map(|w| w.get_trade_classes());
                new_table.price_goods_to_sell(
                    dest_trade_classes,
                    state.seller_broker_skill,
                    state.buyer_broker_skill,
                );
                new_table.sort_by_discount();
                state.available_goods = new_table;
                log::info!("Regenerated trade table prices");
            }
            Err(e) => {
                log::error!("Failed to regenerate trade table: {}", e);
            }
        }
    }

    // Regenerate manifest with fresh die rolls
    if origin_world.is_some() {
        state.ship_manifest.reset_die_rolls();
        state.ship_manifest.price_goods(
            &dest_world,
            state.buyer_broker_skill,
            state.seller_broker_skill,
        );
        log::info!("Regenerated manifest prices");
    }

    // Regenerate passengers with fresh die rolls
    if origin_world.is_some() && dest_world.is_some() {
        let origin = origin_world.as_ref().unwrap();
        let dest = dest_world.as_ref().unwrap();

        let mut passengers = state.available_passengers.take().unwrap_or_default();
        passengers.reset_die_rolls();

        passengers.generate(
            origin.get_population(),
            origin.port,
            origin.travel_zone,
            origin.tech_level,
            dest.get_population(),
            dest.port,
            dest.travel_zone,
            dest.tech_level,
            distance,
            state.steward_skill as i32,
            state.buyer_broker_skill as i32,
        );

        state.available_passengers = Some(passengers);
        log::info!("Regenerated passengers with fresh die rolls");
    }

    // Store the world objects in the state so they're sent back to clients
    state.origin_world = origin_world;
    state.dest_world = dest_world;

    // Save updated state to Firestore
    if let Err(e) = save_trade_state(db, DEFAULT_SESSION, &state).await {
        log::error!("Failed to save regenerated state to Firestore: {}", e);
    }

    // Update shared state and broadcast to all clients
    *current_state.write().await = Some(state.clone());
    broadcast_to_clients(clients, &state, None).await;
}
