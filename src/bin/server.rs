//! # Worldgen WebSocket Server Binary
//!
//! Runs the WebSocket server backend. Three endpoints share one TCP port:
//!
//! - `/ws/trade` — the multi-client trade-tool sync server.
//! - `/ws/simulator` — the streaming ship-simulator server.
//! - `/ws/captains-log` — the streaming Vertex AI captain's-log
//!   summary server.
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
use std::time::Instant;

use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};

use worldgen::backend::captains_log_server;
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

    // The trade server owns its own Firestore handle for trade state.
    // The simulator runs without persistence.
    let trade_server = TradeServer::new(addr).await?;

    // We have two listening modes: the existing TradeServer.run() owns
    // the listener, OR we own the listener and dispatch by URL path.
    // We can't have both, so we replace TradeServer.run() with our own
    // accept loop that peeks the request URI before deciding which
    // handler to invoke.
    let trade_server = Arc::new(trade_server);

    // Captain's-log shared state: the GCP project id (read from env)
    // and the global rate limiter shared across every captains-log
    // connection. The rate limiter holds the Instant of the last
    // accepted request across the entire process.
    let captains_log_project: Arc<String> = Arc::new(
        std::env::var("GCP_PROJECT")
            .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT"))
            .unwrap_or_default(),
    );
    let captains_log_global_limiter: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

    let listener = TcpListener::bind(&addr).await?;
    log::info!(
        "Listening on: {} (trade: /ws/trade, simulator: /ws/simulator, captains-log: /ws/captains-log)",
        addr
    );

    while let Ok((stream, peer_addr)) = listener.accept().await {
        let trade_server = trade_server.clone();
        let captains_log_project = captains_log_project.clone();
        let captains_log_global_limiter = captains_log_global_limiter.clone();
        tokio::spawn(async move {
            if let Err(e) = dispatch(
                stream,
                peer_addr,
                trade_server,
                captains_log_project,
                captains_log_global_limiter,
            )
            .await
            {
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

/// Dispatch one accepted TCP stream to either the trade-tool handler,
/// the simulator handler, or the captain's-log handler based on the
/// WebSocket request URI.
async fn dispatch(
    stream: tokio::net::TcpStream,
    peer_addr: SocketAddr,
    trade_server: Arc<TradeServer>,
    captains_log_project: Arc<String>,
    captains_log_global_limiter: Arc<Mutex<Option<Instant>>>,
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
        simulator_server::handle_simulator_ws(ws_stream).await?;
    } else if path.starts_with("/ws/captains-log") {
        captains_log_server::handle_captains_log_ws(
            ws_stream,
            peer_addr,
            captains_log_project,
            captains_log_global_limiter,
        )
        .await;
    } else {
        // Default to trade — preserves the legacy bare-`/` behaviour.
        trade_server.handle_one_ws(ws_stream, peer_addr).await?;
    }
    Ok(())
}
