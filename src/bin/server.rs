//! # Trade State WebSocket Server Binary
//!
//! This binary runs the WebSocket server for trade state synchronization.
//! It handles connections from multiple clients and broadcasts state updates
//! between them, with Firestore persistence.
//!
//! ## Environment Variables
//!
//! - `GOOGLE_APPLICATION_CREDENTIALS` - Path to GCP service account credentials
//! - `GCP_PROJECT` - GCP project ID
//! - `FIRESTORE_DATABASE_ID` - Firestore database ID (use "debug" to run without Firestore)
//! - `RUST_LOG` - Log level (e.g., "info", "debug", "trace")
//! - `WS_PORT` - WebSocket server port (default: 8081)
//! - `WS_HOST` - WebSocket server host (default: "0.0.0.0")

use std::net::SocketAddr;

use worldgen::backend::server::TradeServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Install the rustls crypto provider (ring) before any TLS operations
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize logging
    env_logger::init();

    // Get server address from environment or use defaults
    let host = std::env::var("WS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("WS_PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse::<u16>()
        .expect("WS_PORT must be a valid port number");

    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("Invalid server address");

    log::info!("Starting Trade State WebSocket Server...");
    log::info!("Listening on: {}", addr);

    // Create and run the server
    let server = TradeServer::new(addr).await?;
    server.run().await?;

    Ok(())
}
