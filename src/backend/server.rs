//! # Trade State WebSocket Server
//!
//! This module provides a WebSocket server for handling trade state updates
//! and broadcasting them to connected clients.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message;

use crate::backend::TradeState;

/// Unique identifier for connected clients
type ClientId = u64;

/// Sender half of a channel for sending messages to a specific client
type ClientSender = mpsc::UnboundedSender<Message>;

/// Shared state containing all connected clients
type Clients = Arc<RwLock<HashMap<ClientId, ClientSender>>>;

/// The trade state server that manages WebSocket connections and state broadcasting
pub struct TradeServer {
    /// Address the server listens on
    addr: SocketAddr,
    /// Connected clients
    clients: Clients,
    /// Counter for generating unique client IDs
    next_client_id: Arc<RwLock<ClientId>>,
}

impl TradeServer {
    /// Creates a new TradeServer bound to the specified address
    ///
    /// # Arguments
    ///
    /// * `addr` - The socket address to listen on (e.g., "127.0.0.1:8080")
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            clients: Arc::new(RwLock::new(HashMap::new())),
            next_client_id: Arc::new(RwLock::new(0)),
        }
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

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, addr, clients, next_id).await {
                    log::error!("Error handling connection from {}: {}", addr, e);
                }
            });
        }

        Ok(())
    }

    /// Handles a single WebSocket connection
    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        clients: Clients,
        next_id: Arc<RwLock<ClientId>>,
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
            clients_guard.insert(client_id, tx);
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
                    match serde_json::from_str::<TradeState>(&text) {
                        Ok(trade_state) => {
                            handle_trade_state_update(trade_state);
                        }
                        Err(e) => {
                            log::warn!(
                                "Failed to deserialize TradeState from client {}: {}",
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
        let json = match serde_json::to_string(state) {
            Ok(j) => j,
            Err(e) => {
                log::error!("Failed to serialize TradeState: {}", e);
                return 0;
            }
        };

        let message = Message::Text(json.into());
        let clients_guard = self.clients.read().await;
        let mut sent_count = 0;

        for (client_id, sender) in clients_guard.iter() {
            if sender.send(message.clone()).is_ok() {
                sent_count += 1;
            } else {
                log::warn!("Failed to queue message for client {}", client_id);
            }
        }

        log::debug!("Broadcast TradeState to {} clients", sent_count);
        sent_count
    }

    /// Returns the number of currently connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}

/// Handler for processing received TradeState updates
///
/// This function is called whenever a client sends a TradeState update.
/// The implementation is intentionally left empty for the user to fill in
/// with their desired behavior.
///
/// # Arguments
///
/// * `state` - The received TradeState from a client
fn handle_trade_state_update(_state: TradeState) {
    // TODO: Implement trade state update handling logic
    // This is intentionally left empty for the user to implement
}
