//! # Worldgen WebSocket Server Binary
//!
//! Runs the WebSocket server backend. Two endpoints share one TCP port:
//!
//! - `/ws/trade` — the multi-client trade-tool sync server.
//! - `/ws/simulator` — the streaming ship-simulator server.
//!
//! ## Environment Variables
//!
//! - `GOOGLE_APPLICATION_CREDENTIALS` - Path to GCP service account credentials
//! - `GCP_PROJECT` - GCP project ID
//! - `FIRESTORE_DATABASE_ID` - Firestore database ID (use "debug" to run without Firestore)
//! - `RUST_LOG` - Log level (e.g., "info", "debug", "trace")
//! - `WS_PORT` - WebSocket server port (default: 8081)
//! - `WS_HOST` - WebSocket server host (default: "0.0.0.0")
//! - `SENTRY_DSN` - If set, initializes Sentry for crash reporting

use std::net::SocketAddr;
use std::sync::Arc;

use firestore::FirestoreDb;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

use worldgen::backend::firestore::initialize_firestore;
use worldgen::backend::server::TradeServer;
use worldgen::backend::simulator_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Sentry must be initialized before logging so panics are reported.
    let _sentry_guard = std::env::var("SENTRY_DSN").ok().map(|dsn| {
        sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                traces_sample_rate: 0.1,
                ..Default::default()
            },
        ))
    });

    // Install the rustls crypto provider (ring) before any TLS operations.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize logging from RUST_LOG environment variable.
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .init();

    if _sentry_guard.is_some() {
        log::info!("Sentry: enabled");
    } else {
        log::info!("Sentry: disabled (no SENTRY_DSN env var)");
    }

    // Get server address from environment or use defaults.
    let host = std::env::var("WS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("WS_PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse::<u16>()
        .expect("WS_PORT must be a valid port number");

    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("Invalid server address");

    log::info!("Starting Worldgen WebSocket server on {}", addr);

    // Build the trade server (which owns its own Firestore handle for trade
    // state) and grab a shared db handle for the simulator.
    let trade_server = TradeServer::new(addr).await?;
    let db: Arc<Option<FirestoreDb>> = Arc::new(initialize_firestore().await.unwrap_or_else(|e| {
        log::error!(
            "simulator: Firestore init failed ({}); running without persistence",
            e
        );
        None
    }));

    // We have two listening modes: the existing TradeServer.run() owns
    // the listener, OR we own the listener and dispatch by URL path.
    // We can't have both, so we replace TradeServer.run() with our own
    // accept loop that peeks the request URI before deciding which
    // handler to invoke.
    let trade_server = Arc::new(trade_server);
    let listener = TcpListener::bind(&addr).await?;
    log::info!(
        "Listening on: {} (trade: /ws/trade, simulator: /ws/simulator)",
        addr
    );

    while let Ok((stream, peer_addr)) = listener.accept().await {
        let trade_server = trade_server.clone();
        let db = db.clone();
        tokio::spawn(async move {
            if let Err(e) = dispatch(stream, peer_addr, trade_server, db).await {
                log::error!("Connection from {} ended with error: {}", peer_addr, e);
                sentry::capture_message(
                    &format!("connection error from {}: {}", peer_addr, e),
                    sentry::Level::Error,
                );
            }
        });
    }

    Ok(())
}

/// Dispatch one accepted TCP stream to either the trade-tool handler
/// or the simulator handler based on the WebSocket request URI.
async fn dispatch(
    stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    trade_server: Arc<TradeServer>,
    sim_db: Arc<Option<FirestoreDb>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Capture the request URI during the handshake.
    let captured_path: Arc<RwLock<String>> = Arc::new(RwLock::new(String::new()));
    let path_writer = captured_path.clone();
    #[allow(clippy::result_large_err)]
    let copy_path = move |req: &Request, response: Response| -> Result<Response, _> {
        let path = req.uri().path().to_string();
        if let Ok(mut g) = path_writer.try_write() {
            *g = path;
        }
        Ok(response)
    };

    let ws_stream = tokio_tungstenite::accept_hdr_async(stream, copy_path).await?;
    let path = captured_path.read().await.clone();
    log::info!("connection from {} requested path {}", peer_addr, path);

    if path.starts_with("/ws/simulator") {
        simulator_server::handle_simulator_ws(ws_stream, sim_db).await?;
    } else {
        // Default to trade — preserves the legacy bare-`/` behaviour.
        trade_server.handle_one_ws(ws_stream, peer_addr).await?;
    }
    Ok(())
}
