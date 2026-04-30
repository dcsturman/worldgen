//! WebSocket handler for the ship-simulator endpoint.
//!
//! One simulation per connection. The lifecycle is:
//!
//! 1. Client opens `/ws/simulator`.
//! 2. Client sends a single `ClientMessage::RunSimulation(params)`.
//! 3. Server streams `ServerMessage::Step` frames until the run finishes.
//! 4. Server sends exactly one `ServerMessage::Done` or `ServerMessage::Error`.
//! 5. Server closes the connection.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use futures_util::stream::SplitStream;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use crate::simulator::executor::run_simulation;
use crate::simulator::protocol::{ClientMessage, ServerMessage};
use crate::simulator::types::{SimulationResult, SimulationStep};
use crate::simulator::world_fetch::WorldCache;

/// Handle a single simulator WebSocket connection from start to finish.
///
/// This is independent of the trade-tool [`crate::backend::server::TradeServer`] —
/// it doesn't share clients, state, or the broadcast machinery.
pub async fn handle_simulator_connection(
    stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    log::info!("simulator: WebSocket connection established");
    handle_ws(ws_stream).await
}

/// Handle a simulator WebSocket once the handshake is already done.
/// Used by the dispatch path in `bin/server.rs` that needs to peek the
/// HTTP path before deciding which handler to call.
pub async fn handle_simulator_ws(
    ws_stream: WebSocketStream<TcpStream>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    handle_ws(ws_stream).await
}

async fn handle_ws(
    ws_stream: WebSocketStream<TcpStream>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Read the first message — must be a RunSimulation.
    let first = match ws_receiver.next().await {
        Some(Ok(Message::Text(t))) => t,
        Some(Ok(Message::Close(_))) => {
            log::info!("simulator: client closed before sending params");
            return Ok(());
        }
        Some(Ok(_)) => {
            send_error(&mut ws_sender, "expected text message").await;
            return Ok(());
        }
        Some(Err(e)) => {
            log::error!("simulator: ws receive error: {}", e);
            return Ok(());
        }
        None => {
            log::info!("simulator: client disconnected before sending params");
            return Ok(());
        }
    };

    let client_msg: ClientMessage = match serde_json::from_str(&first) {
        Ok(m) => m,
        Err(e) => {
            send_error(
                &mut ws_sender,
                &format!("could not parse RunSimulation: {}", e),
            )
            .await;
            return Ok(());
        }
    };

    let ClientMessage::RunSimulation(params) = client_msg;

    log::info!(
        "simulator: starting run for {} ({}-{}) jump={} cargo={}",
        params.home_world.name,
        params.home_world.sector,
        params.home_world.uwp,
        params.jump,
        params.cargo_capacity
    );

    // Bridge: executor task pushes steps over an mpsc channel; this
    // task reads them and pipes them out as text frames. When the
    // executor finishes, it returns its result over a oneshot.
    let (step_tx, mut step_rx) = mpsc::unbounded_channel::<SimulationStep>();
    let (result_tx, result_rx) =
        tokio::sync::oneshot::channel::<Result<SimulationResult, String>>();

    tokio::spawn(async move {
        let mut cache = WorldCache::new();
        let res = run_simulation(params, &mut cache, |step| {
            // Drop steps if the writer is gone; the executor keeps going.
            let _ = step_tx.send(step);
        })
        .await;
        let res = res.map_err(|e| e.to_string());
        let _ = result_tx.send(res);
    });

    // Pipe steps as they arrive.
    while let Some(step) = step_rx.recv().await {
        let msg = ServerMessage::Step(step);
        match serde_json::to_string(&msg) {
            Ok(json) => {
                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                    log::warn!("simulator: client closed mid-run");
                    return Ok(());
                }
            }
            Err(e) => {
                log::error!("simulator: failed to serialize step: {}", e);
            }
        }
    }

    // Executor has finished; pull its result.
    let result = match result_rx.await {
        Ok(r) => r,
        Err(_) => {
            send_error(&mut ws_sender, "executor task dropped").await;
            drain_until_close(&mut ws_receiver, Duration::from_secs(2)).await;
            return Ok(());
        }
    };

    match result {
        Ok(r) => {
            let done = ServerMessage::Done(r);
            match serde_json::to_string(&done) {
                Ok(json) => {
                    let _ = ws_sender.send(Message::Text(json.into())).await;
                }
                Err(e) => {
                    log::error!("simulator: failed to serialize Done: {}", e);
                }
            }
        }
        Err(message) => {
            send_error(&mut ws_sender, &message).await;
        }
    }

    // Drive a clean WebSocket close: send Close, then read until the client
    // sends its Close back (or we time out). If we drop the receiver with
    // unread data still in the kernel buffer, Linux turns the socket close
    // into a TCP RST — which the browser reports as code 1006 even though
    // the run finished cleanly.
    let _ = ws_sender.send(Message::Close(None)).await;
    drain_until_close(&mut ws_receiver, Duration::from_secs(2)).await;
    log::info!("simulator: connection closed");
    Ok(())
}

async fn drain_until_close(
    ws_receiver: &mut SplitStream<WebSocketStream<TcpStream>>,
    timeout: Duration,
) {
    let _ = tokio::time::timeout(timeout, async {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Close(_)) | Err(_) => break,
                _ => continue,
            }
        }
    })
    .await;
}

async fn send_error<S>(sender: &mut S, message: &str)
where
    S: SinkExt<Message> + Unpin,
    <S as futures_util::Sink<Message>>::Error: std::fmt::Display,
{
    let err = ServerMessage::Error {
        message: message.to_string(),
    };
    match serde_json::to_string(&err) {
        Ok(json) => {
            if let Err(e) = sender.send(Message::Text(json.into())).await {
                log::warn!("simulator: failed to send error frame: {}", e);
            }
        }
        Err(e) => {
            log::error!("simulator: failed to serialize Error: {}", e);
        }
    }
    let _ = sender.send(Message::Close(None)).await;
}
