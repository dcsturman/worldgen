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
use tokio_tungstenite::WebSocketStream;
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

/// Per-connection bookkeeping. The ship name is None until the client
/// sends a SelectShip command — until that happens we don't know which
/// session their state-updates apply to and we drop them.
struct ClientInfo {
    sender: ClientSender,
    ship: Option<String>,
}

/// Shared state containing all connected clients
type Clients = Arc<RwLock<HashMap<ClientId, ClientInfo>>>;

/// Database wrapped in Arc for sharing across tasks
type SharedDb = Arc<Option<FirestoreDb>>;

/// In-memory cache of the latest trade state per ship name. Keyed by the
/// ship name the client sends with SelectShip — same key used in
/// Firestore. Lets the recalculate logic compare against the previous
/// value to detect what changed without round-tripping to Firestore.
type SharedStates = Arc<RwLock<HashMap<String, TradeState>>>;

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
    /// Per-ship cached trade state (used to detect changes and recalculate).
    /// Each entry corresponds to a `ship_name` selected by some client.
    states: SharedStates,
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
            states: Arc::new(RwLock::new(HashMap::new())),
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
            let states = self.states.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, addr, clients, next_id, db, states).await
                {
                    log::error!("Error handling connection from {}: {}", addr, e);
                }
            });
        }

        Ok(())
    }

    /// Broadcasts a TradeState update to clients viewing the named ship.
    ///
    /// # Arguments
    ///
    /// * `ship` - Ship name whose viewers should receive this update
    /// * `state` - The TradeState to broadcast
    ///
    /// # Returns
    ///
    /// The number of clients the message was successfully queued for
    pub async fn update_clients(&self, ship: &str, state: &TradeState) -> usize {
        broadcast_to_ship(&self.clients, ship, state).await
    }

    /// Returns the number of currently connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Handle a single trade-tool WebSocket connection that has *already*
    /// completed its handshake (used by the URL-dispatch loop in
    /// `bin/server.rs`).
    pub async fn handle_one_ws(
        &self,
        ws_stream: WebSocketStream<TcpStream>,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        handle_post_handshake(
            ws_stream,
            addr,
            self.clients.clone(),
            self.next_client_id.clone(),
            self.db.clone(),
            self.states.clone(),
        )
        .await
    }
}

/// Handles a single WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    clients: Clients,
    next_id: Arc<RwLock<ClientId>>,
    db: SharedDb,
    states: SharedStates,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    log::info!("WebSocket connection established: {}", addr);
    handle_post_handshake(ws_stream, addr, clients, next_id, db, states).await
}

/// Handles a single WebSocket connection whose handshake is already done.
///
/// On connect we don't send any state — the client must first send a
/// SelectShip command naming the ship session it wants to sync. This is
/// the multi-tenant entry point: each ship name maps to its own
/// Firestore document, and broadcasts only reach clients viewing the
/// same ship.
async fn handle_post_handshake(
    ws_stream: WebSocketStream<TcpStream>,
    addr: SocketAddr,
    clients: Clients,
    next_id: Arc<RwLock<ClientId>>,
    db: SharedDb,
    states: SharedStates,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    // Register the client with no ship selected yet.
    {
        let mut clients_guard = clients.write().await;
        clients_guard.insert(
            client_id,
            ClientInfo {
                sender: tx.clone(),
                ship: None,
            },
        );
    }

    log::info!("Client {} connected from {}", client_id, addr);

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
                        handle_trade_state_update(client_id, trade_state, &db, &clients, &states)
                            .await;
                    }
                    Ok(ServerMessage::Command(ServerCommand::Regenerate)) => {
                        handle_regenerate_command(client_id, &db, &clients, &states).await;
                    }
                    Ok(ServerMessage::Command(ServerCommand::SelectShip { ship_name })) => {
                        handle_select_ship(client_id, ship_name, &db, &clients, &states).await;
                    }
                    Ok(ServerMessage::Command(ServerCommand::ApplyMonthlyExpenses)) => {
                        handle_apply_monthly_expenses(client_id, &db, &clients, &states).await;
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

/// Broadcasts a TradeState to every client currently viewing `ship`.
/// Clients on other ships (or with no ship selected) are not notified —
/// each ship's session is isolated.
async fn broadcast_to_ship(clients: &Clients, ship: &str, state: &TradeState) -> usize {
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

    for (client_id, info) in clients_guard.iter() {
        if info.ship.as_deref() != Some(ship) {
            continue;
        }
        if info.sender.send(message.clone()).is_ok() {
            sent_count += 1;
        } else {
            log::warn!("Failed to queue message for client {}", client_id);
        }
    }

    log::debug!(
        "Broadcast TradeState to {} clients on ship {}",
        sent_count,
        ship
    );
    sent_count
}

/// Look up the ship name a given client has selected. Returns None if
/// the client hasn't sent SelectShip yet.
async fn current_ship_of(clients: &Clients, client_id: ClientId) -> Option<String> {
    clients
        .read()
        .await
        .get(&client_id)
        .and_then(|info| info.ship.clone())
}

/// Handle a client's SelectShip command. Switches this client's session
/// to `ship_name`, loads that ship's persisted state from the in-memory
/// cache (or Firestore on first use), and sends it back to just this
/// client.
async fn handle_select_ship(
    client_id: ClientId,
    ship_name: String,
    db: &SharedDb,
    clients: &Clients,
    states: &SharedStates,
) {
    let ship_name = ship_name.trim().to_string();
    if ship_name.is_empty() {
        log::warn!("Client {} sent SelectShip with empty name", client_id);
        return;
    }

    // Update this client's ship
    {
        let mut clients_guard = clients.write().await;
        if let Some(info) = clients_guard.get_mut(&client_id) {
            info.ship = Some(ship_name.clone());
        } else {
            log::warn!(
                "Client {} sent SelectShip but is no longer registered",
                client_id
            );
            return;
        }
    }

    // Try the cache first. On miss, load from Firestore (which itself
    // creates a default document if none exists).
    let cached = {
        let states_guard = states.read().await;
        states_guard.get(&ship_name).cloned()
    };

    let mut state = match cached {
        Some(s) => s,
        None => match get_trade_state(db, &ship_name).await {
            Ok(s) => s,
            Err(FirestoreError::SchemaError(e)) => {
                // Document exists but in an old/incompatible shape. Reset
                // it to a default so this ship can move forward.
                log::warn!(
                    "Schema mismatch for ship {}: {}. Resetting to default.",
                    ship_name,
                    e
                );
                let mut default_state = TradeState::default();
                default_state.ship.name = ship_name.clone();
                if let Err(save_err) = save_trade_state(db, &ship_name, &default_state).await {
                    log::error!(
                        "Failed to save default state for ship {} after schema error: {}",
                        ship_name,
                        save_err
                    );
                }
                default_state
            }
            Err(e) => {
                log::error!("Failed to load state for ship {}: {}", ship_name, e);
                return;
            }
        },
    };

    // Make `state.ship.name` the canonical record of the session key.
    // Newly-created states pick it up here; legacy documents that were
    // persisted before `Ship` carried a name field also get backfilled
    // (best-effort: we don't try to preserve other legacy fields).
    if state.ship.name != ship_name {
        state.ship.name = ship_name.clone();
    }
    states
        .write()
        .await
        .insert(ship_name.clone(), state.clone());

    // Send the state to just this client.
    let clients_guard = clients.read().await;
    if let Some(info) = clients_guard.get(&client_id) {
        match serde_json::to_string(&state) {
            Ok(json) => {
                if info.sender.send(Message::Text(json.into())).is_ok() {
                    log::info!("Sent state for ship {} to client {}", ship_name, client_id);
                } else {
                    log::warn!(
                        "Failed to queue state for ship {} to client {}",
                        ship_name,
                        client_id
                    );
                }
            }
            Err(e) => log::error!("Failed to serialize state for ship {}: {}", ship_name, e),
        }
    }
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
/// After recalculation, the updated state is broadcast to all clients
/// viewing the same ship.
async fn handle_trade_state_update(
    client_id: ClientId,
    mut state: TradeState,
    db: &SharedDb,
    clients: &Clients,
    states: &SharedStates,
) {
    let ship_name = match current_ship_of(clients, client_id).await {
        Some(s) => s,
        None => {
            log::warn!(
                "Client {} sent StateUpdate before SelectShip — dropping",
                client_id
            );
            return;
        }
    };

    // Get the previous state for this ship to detect what changed
    let prev_state = {
        let states_guard = states.read().await;
        states_guard.get(&ship_name).cloned()
    };

    // Detect what changed and recalculate as needed
    let recalculated = recalculate_trade_state(&mut state, prev_state.as_ref());

    if recalculated {
        log::info!(
            "Server recalculated trade state for ship {} due to world/skill changes",
            ship_name
        );
    }

    // Update the per-ship cache
    {
        let mut states_guard = states.write().await;
        states_guard.insert(ship_name.clone(), state.clone());
    }

    // Save to Firestore under this ship's session
    if let Err(e) = save_trade_state(db, &ship_name, &state).await {
        log::error!(
            "Failed to save trade state for ship {} to Firestore: {}",
            ship_name,
            e
        );
        // Continue to broadcast even if Firestore save fails
    }

    // Broadcast to all clients on this ship (sender included — server is
    // authoritative and may have rewritten fields they need to see).
    let sent_count = broadcast_to_ship(clients, &ship_name, &state).await;
    log::info!(
        "Broadcast trade state for ship {} to {} clients (recalculated: {})",
        ship_name,
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

    // Check if skills changed.
    //
    // The Ship Broker skill (`ship.broker_skill`) and the steward skill
    // (`ship.steward_skill`) live on the unified `Ship` config. Changing
    // any other Ship field (capacity, hardware, periodic costs, …) also
    // signals "skills_changed" — that's a slight over-trigger but keeps
    // the diff trivially correct now that `Ship` derives `PartialEq`.
    let skills_changed = prev_state.is_none()
        || prev_state.is_some_and(|prev| {
            prev.ship != state.ship || prev.system_broker_skill != state.system_broker_skill
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

    // Reprice goods if origin changed, dest changed, or skills changed.
    //
    // Mapping from the new `Ship` / `system_broker_skill` shape to the
    // `(buyer_broker_skill, supplier_broker_skill)` arguments expected
    // by `available_goods` and `ship_manifest`:
    //   - At origin, the player buys: buyer = ship.broker_skill,
    //     supplier = system_broker_skill.
    //   - At origin, the "sell back" preview reverses sides: buyer =
    //     system_broker_skill, supplier = ship.broker_skill.
    //   - The manifest pricing here mirrors the legacy call exactly
    //     (player as buyer, system as supplier) so existing behaviour
    //     is preserved.
    if (origin_changed || dest_changed || skills_changed)
        && let Some(world) = origin_world.as_ref()
    {
        // Price goods to buy at origin (player buying from system).
        state.available_goods.price_goods_to_buy(
            &world.get_trade_classes(),
            state.ship.broker_skill,
            state.system_broker_skill,
        );

        // Price goods to sell at the current world (system buying from
        // ship — sides reversed).
        state.available_goods.price_goods_to_sell(
            Some(world.get_trade_classes()),
            state.system_broker_skill,
            state.ship.broker_skill,
        );

        state.available_goods.sort_by_discount();
        log::info!("Repriced available goods");

        // Reprice ship manifest goods (preserves legacy argument order:
        // player-as-buyer, system-as-supplier).
        state.ship_manifest.price_goods(
            &origin_world,
            state.ship.broker_skill,
            state.system_broker_skill,
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
                state.ship.steward_skill as i32,
                state.ship.broker_skill as i32,
            );

            state.available_passengers = Some(passengers);
            recalculated = true;
            log::info!(
                "Regenerated passengers for route {} -> {} (distance: {} parsecs)",
                state.origin_world_name,
                state.dest_world_name,
                distance
            );
        } else {
            // Clear passengers if there's no destination
            if state.available_passengers.is_some() {
                state.available_passengers = None;
                recalculated = true;
                log::info!("Cleared passengers - no destination set");
            }
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
/// Scoped to the requesting client's currently-selected ship.
async fn handle_regenerate_command(
    client_id: ClientId,
    db: &SharedDb,
    clients: &Clients,
    states: &SharedStates,
) {
    let ship_name = match current_ship_of(clients, client_id).await {
        Some(s) => s,
        None => {
            log::warn!(
                "Client {} sent Regenerate before SelectShip — dropping",
                client_id
            );
            return;
        }
    };

    let mut state = match states.write().await.remove(&ship_name) {
        Some(s) => s,
        None => {
            log::warn!(
                "Received regenerate for ship {} but no state cached",
                ship_name
            );
            return;
        }
    };

    let origin_world = state.origin_world.clone();
    let dest_world = state.dest_world.clone();
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
                    state.ship.broker_skill,
                    state.system_broker_skill,
                );
                let dest_trade_classes = dest_world.as_ref().map(|w| w.get_trade_classes());
                new_table.price_goods_to_sell(
                    dest_trade_classes,
                    state.system_broker_skill,
                    state.ship.broker_skill,
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
            &origin_world,
            state.ship.broker_skill,
            state.system_broker_skill,
        );
        log::info!("Regenerated manifest prices");
    }

    // Regenerate passengers with fresh die rolls

    if let Some(origin) = origin_world.as_ref()
        && let Some(dest) = dest_world.as_ref()
    {
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
            state.ship.steward_skill as i32,
            state.ship.broker_skill as i32,
        );

        state.available_passengers = Some(passengers);
        log::info!("Regenerated passengers with fresh die rolls");
    }

    // Save updated state to Firestore under this ship's session
    if let Err(e) = save_trade_state(db, &ship_name, &state).await {
        log::error!(
            "Failed to save regenerated state for ship {} to Firestore: {}",
            ship_name,
            e
        );
    }

    // Update cache and broadcast to clients viewing this ship
    states
        .write()
        .await
        .insert(ship_name.clone(), state.clone());
    broadcast_to_ship(clients, &ship_name, &state).await;
}

/// Handles an `ApplyMonthlyExpenses` command from a client.
///
/// Subtracts one 28-day period of fixed expenses (mortgage +
/// maintenance + salary, computed by [`crate::trade::Ship::monthly_expenses`])
/// from the current ship's manifest profit, persists the updated state,
/// and broadcasts it to every client viewing this ship. Mirrors the
/// structure of `handle_regenerate_command` so the cache / Firestore /
/// broadcast invariants stay aligned across both commands.
///
/// If the requesting client hasn't selected a ship yet, or no cached
/// state exists for the selected ship, the command is dropped with a
/// warning — there's no sensible default `Ship` to compute expenses
/// against until at least one StateUpdate or SelectShip has been
/// processed.
async fn handle_apply_monthly_expenses(
    client_id: ClientId,
    db: &SharedDb,
    clients: &Clients,
    states: &SharedStates,
) {
    let ship_name = match current_ship_of(clients, client_id).await {
        Some(s) => s,
        None => {
            log::warn!(
                "Client {} sent ApplyMonthlyExpenses before SelectShip — dropping",
                client_id
            );
            return;
        }
    };

    let mut state = match states.write().await.remove(&ship_name) {
        Some(s) => s,
        None => {
            log::warn!(
                "Received ApplyMonthlyExpenses for ship {} but no state cached",
                ship_name
            );
            return;
        }
    };

    let expenses = state.ship.monthly_expenses();
    state.ship_manifest.profit -= expenses;
    log::info!(
        "Applied monthly expenses for ship {}: -{} credits (new profit: {})",
        ship_name,
        expenses,
        state.ship_manifest.profit
    );

    // Save updated state to Firestore under this ship's session
    if let Err(e) = save_trade_state(db, &ship_name, &state).await {
        log::error!(
            "Failed to save state for ship {} after applying monthly expenses: {}",
            ship_name,
            e
        );
    }

    // Update cache and broadcast to clients viewing this ship
    states
        .write()
        .await
        .insert(ship_name.clone(), state.clone());
    broadcast_to_ship(clients, &ship_name, &state).await;
}
