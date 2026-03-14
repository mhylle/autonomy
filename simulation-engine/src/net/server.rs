use std::sync::Arc;
use tokio::sync::broadcast;
use crossbeam_channel::Sender;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use tracing::{info, warn, debug};

/// Viewport bounds sent by the viewer for server-side entity filtering.
#[derive(Debug, Clone, Copy)]
pub struct ViewportBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub zoom: f32,
}

impl Default for ViewportBounds {
    fn default() -> Self {
        // Default to a very large viewport so everything is visible
        // until the client sends its actual bounds.
        Self {
            x: 0.0,
            y: 0.0,
            width: 10_000.0,
            height: 10_000.0,
            zoom: 1.0,
        }
    }
}

/// Commands sent from the viewer to control simulation.
#[derive(Debug)]
pub enum ViewerCommand {
    Pause,
    Resume,
    SetSpeed(f64),
    SubscribeViewport(ViewportBounds),
}

/// Shared state between WebSocket handler instances.
#[derive(Clone)]
pub struct ServerState {
    /// Broadcast channel for tick deltas (serialized bytes).
    pub tick_tx: broadcast::Sender<Vec<u8>>,
    /// Latest world snapshot (protobuf bytes) for new connections.
    pub snapshot: Arc<tokio::sync::RwLock<Vec<u8>>>,
    /// Channel to send viewer commands to the simulation loop.
    pub command_tx: Sender<ViewerCommand>,
    /// Current viewport bounds from the viewer (single-viewer model).
    pub viewport: Arc<std::sync::RwLock<ViewportBounds>>,
}

impl ServerState {
    pub fn new(command_tx: Sender<ViewerCommand>) -> Self {
        // Small buffer — we want lagged receivers to skip, not accumulate.
        let (tick_tx, _) = broadcast::channel(4);
        Self {
            tick_tx,
            snapshot: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            command_tx,
            viewport: Arc::new(std::sync::RwLock::new(ViewportBounds::default())),
        }
    }
}

/// Start the WebSocket server on the given port.
pub async fn start_server(state: ServerState, port: u16) {
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!(addr = %addr, "starting WebSocket server");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_connection(socket, state))
}

async fn handle_connection(socket: WebSocket, state: ServerState) {
    let (mut sender, mut receiver) = socket.split();
    info!("new WebSocket client connected");

    // Send current world snapshot to the newly connected client.
    {
        let snapshot_bytes = state.snapshot.read().await;
        if !snapshot_bytes.is_empty() {
            if let Err(e) = sender
                .send(Message::Binary(snapshot_bytes.clone().into()))
                .await
            {
                warn!("failed to send snapshot: {}", e);
                return;
            }
        }
    }

    // Subscribe to tick deltas broadcast channel.
    let mut tick_rx = state.tick_tx.subscribe();

    // Spawn a task to forward tick deltas to the client.
    // Key: if the receiver lags (can't keep up), we skip old messages
    // and only send the latest one. This prevents unbounded buffer growth.
    let send_task = tokio::spawn(async move {
        loop {
            match tick_rx.recv().await {
                Ok(delta_bytes) => {
                    // Drain any additional queued messages — only send the latest
                    let mut latest = delta_bytes;
                    while let Ok(newer) = tick_rx.try_recv() {
                        latest = newer;
                    }

                    if sender
                        .send(Message::Binary(latest.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // Receiver fell behind — skip stale messages, this is fine
                    debug!(skipped = n, "viewer lagged, skipping old deltas");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Read incoming messages (commands from client).
    let command_tx = state.command_tx.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        match value.get("type").and_then(|t| t.as_str()) {
                            Some("pause") => {
                                let _ = command_tx.send(ViewerCommand::Pause);
                            }
                            Some("resume") => {
                                let _ = command_tx.send(ViewerCommand::Resume);
                            }
                            Some("set_speed") => {
                                if let Some(speed) = value.get("speed").and_then(|s| s.as_f64()) {
                                    let _ = command_tx.send(ViewerCommand::SetSpeed(speed));
                                }
                            }
                            Some("subscribe_viewport") => {
                                let x = value.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let y = value.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                let width = value.get("width").and_then(|v| v.as_f64()).unwrap_or(10_000.0);
                                let height = value.get("height").and_then(|v| v.as_f64()).unwrap_or(10_000.0);
                                let zoom = value.get("zoom").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                                let bounds = ViewportBounds { x, y, width, height, zoom };
                                debug!(?bounds, "viewport update");
                                let _ = command_tx.send(ViewerCommand::SubscribeViewport(bounds));
                            }
                            _ => {
                                warn!("unknown command type: {}", text);
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    info!("WebSocket client disconnected");
}
