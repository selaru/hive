pub mod hub;

use crate::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use uuid::Uuid;

/// WebSocket handler for per-project event stream.
pub async fn project_ws_handler(
    ws:      WebSocketUpgrade,
    State(s): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Response {
    ws.on_upgrade(move |socket| handle_project_ws(socket, s, id))
}

/// WebSocket handler for board-level events (all projects).
pub async fn board_ws_handler(
    ws:      WebSocketUpgrade,
    State(s): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_board_ws(socket, s))
}

async fn handle_project_ws(socket: WebSocket, s: Arc<AppState>, project_id: Uuid) {
    let channel = format!("project:{}:events", project_id);
    let mut rx   = s.ws_hub.subscribe(&channel).await;
    let (mut sender, mut receiver) = socket.split();

    loop {
        tokio::select! {
            Ok(event) = rx.recv() => {
                if sender.send(Message::Text(event.into())).await.is_err() {
                    break; // client disconnected
                }
            }
            Some(msg) = receiver.next() => {
                // Handle ping/close from client
                match msg {
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
        }
    }
}

async fn handle_board_ws(socket: WebSocket, s: Arc<AppState>) {
    let mut rx = s.ws_hub.subscribe("board:events").await;
    let (mut sender, mut receiver) = socket.split();

    loop {
        tokio::select! {
            Ok(event) = rx.recv() => {
                if sender.send(Message::Text(event.into())).await.is_err() { break; }
            }
            Some(msg) = receiver.next() => {
                match msg {
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
        }
    }
}
