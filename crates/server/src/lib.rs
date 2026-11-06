//! Multiplayer server: rooms and lobby logic, a WebSocket endpoint, and static hosting
//! for the web client. Everything lives in memory; nothing is remembered after a game.

pub mod protocol;
pub mod server;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use protocol::{ClientMsg, ServerMsg};
use server::Server;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tower_http::services::ServeDir;

pub type Shared = Arc<Mutex<Server>>;

/// Build the HTTP app: `/ws` for play, everything else served from `web_dir`.
pub fn app(shared: Shared, web_dir: PathBuf) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new(web_dir))
        .with_state(shared)
}

/// Periodically drop rooms that have been empty for a long time.
pub fn spawn_sweeper(shared: Shared) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            shared.lock().expect("server lock").sweep();
        }
    });
}

async fn ws_handler(ws: WebSocketUpgrade, State(shared): State<Shared>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| client_loop(socket, shared))
}

async fn client_loop(socket: WebSocket, shared: Shared) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let conn = shared.lock().expect("server lock").connect(tx.clone());

    let writer = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = stream.next().await {
        match message {
            Message::Text(text) => match serde_json::from_str::<ClientMsg>(&text) {
                Ok(msg) => shared.lock().expect("server lock").handle(conn, msg),
                Err(_) => {
                    let error = ServerMsg::Error {
                        message: "could not understand that message".to_string(),
                    };
                    if let Ok(json) = serde_json::to_string(&error) {
                        let _ = tx.send(json);
                    }
                }
            },
            Message::Close(_) => break,
            _ => {}
        }
    }

    shared.lock().expect("server lock").disconnect(conn);
    writer.abort();
}
