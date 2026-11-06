//! Four WebSocket clients play a whole game against a real running server.

use futures_util::{SinkExt, StreamExt};
use mahjongi_server::server::Server;
use mahjongi_server::{app, Shared};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

async fn start_server() -> (String, Shared) {
    let shared: Shared = Arc::new(Mutex::new(Server::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = app(shared.clone(), PathBuf::from("../../web"));
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (format!("ws://{addr}/ws"), shared)
}

async fn send(ws: &mut Ws, msg: Value) {
    ws.send(Message::Text(msg.to_string().into()))
        .await
        .unwrap();
}

async fn recv(ws: &mut Ws) -> Value {
    loop {
        match ws.next().await.expect("socket open").unwrap() {
            Message::Text(t) => return serde_json::from_str(&t).unwrap(),
            _ => continue,
        }
    }
}

async fn recv_type(ws: &mut Ws, kind: &str) -> Value {
    loop {
        let msg = recv(ws).await;
        if msg["type"] == kind {
            return msg;
        }
    }
}

/// How worth keeping a tile is: pairs and near neighbours score higher.
fn usefulness(counts: &[u32; 34], tile: usize) -> u32 {
    let mut u = counts[tile] * 4;
    if tile < 27 {
        for d in [-2i32, -1, 1, 2] {
            let other = tile as i32 + d;
            if (0..27).contains(&other) && (other / 9) as usize == tile / 9 {
                u += counts[other as usize] * if d.abs() == 1 { 2 } else { 1 };
            }
        }
    }
    u
}

/// Decide what to send for a `state` message, if anything.
fn decide(state: &Value) -> Option<Value> {
    let view = &state["view"];
    let legal = view["legal"].as_array()?;
    if legal.is_empty() {
        let seat = state["seat"].as_u64()? as usize;
        let waiting = view["phase"] == "hand_over" && state["acks"][seat] == false;
        return waiting.then(|| json!({"type": "next_hand"}));
    }
    let has = |kind: &str| legal.iter().find(|a| a["type"] == kind).cloned();
    let action = has("win")
        .or_else(|| has("pung"))
        .or_else(|| has("kong"))
        .or_else(|| has("pass"))
        .or_else(|| {
            let mut counts = [0u32; 34];
            for t in view["hand"]
                .as_array()?
                .iter()
                .chain(view["drawn"].as_u64().map(|_| &view["drawn"]))
            {
                counts[t.as_u64()? as usize] += 1;
            }
            legal
                .iter()
                .filter(|a| a["type"] == "discard")
                .min_by_key(|a| {
                    let t = a["tile"].as_u64().unwrap() as usize;
                    (usefulness(&counts, t), t)
                })
                .cloned()
        })?;
    Some(json!({"type": "act", "action": action}))
}

/// Play until the game is over; returns the final state message.
async fn play_to_the_end(mut ws: Ws) -> Value {
    loop {
        let msg = recv(&mut ws).await;
        if msg["type"] != "state" {
            continue;
        }
        if msg["view"]["phase"] == "game_over" {
            return msg;
        }
        if let Some(reply) = decide(&msg) {
            send(&mut ws, reply).await;
        }
    }
}

#[tokio::test]
async fn four_clients_play_a_full_game() {
    let (url, shared) = start_server().await;

    let mut host = connect_async(&url).await.unwrap().0;
    send(&mut host, json!({"type": "create", "name": "Host"})).await;
    let joined = recv_type(&mut host, "joined").await;
    let code = joined["code"].as_str().unwrap().to_string();

    let mut sockets = vec![host];
    for i in 1..4 {
        let mut ws = connect_async(&url).await.unwrap().0;
        send(
            &mut ws,
            json!({"type": "join", "code": code, "name": format!("Player{i}")}),
        )
        .await;
        recv_type(&mut ws, "joined").await;
        sockets.push(ws);
    }

    // Lenient threshold so the bots actually finish hands with wins.
    send(
        &mut sockets[0],
        json!({"type": "set_options", "settings": {
            "base": "hk_fan", "addons": [], "disabled_hands": [],
            "threshold_offset": -3, "flowers": true, "rounds": 1
        }}),
    )
    .await;
    send(&mut sockets[0], json!({"type": "start"})).await;

    let players: Vec<_> = sockets
        .into_iter()
        .map(|ws| tokio::spawn(play_to_the_end(ws)))
        .collect();

    let mut finals = Vec::new();
    for p in players {
        let result = tokio::time::timeout(Duration::from_secs(120), p)
            .await
            .expect("game finished in time")
            .unwrap();
        finals.push(result);
    }

    for f in &finals {
        let scores: Vec<i64> = f["view"]["scores"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_i64().unwrap())
            .collect();
        assert_eq!(scores.iter().sum::<i64>(), 0, "scores are zero-sum");
        assert_eq!(f["view"]["phase"], "game_over");
        assert!(f["view"]["hand_no"].as_u64().unwrap() >= 4);
        assert_eq!(f["names"][3], "Player3");
    }
    // Everyone saw the same scores.
    assert!(finals
        .iter()
        .all(|f| f["view"]["scores"] == finals[0]["view"]["scores"]));
    assert_eq!(shared.lock().unwrap().room_count(), 1);
}

#[tokio::test]
async fn bad_messages_get_an_error_not_a_crash() {
    let (url, _) = start_server().await;
    let mut ws = connect_async(&url).await.unwrap().0;
    ws.send(Message::Text("not json".into())).await.unwrap();
    let err = recv_type(&mut ws, "error").await;
    assert!(err["message"].as_str().unwrap().contains("understand"));
    send(&mut ws, json!({"type": "start"})).await;
    let err = recv_type(&mut ws, "error").await;
    assert!(err["message"].as_str().unwrap().contains("not in a room"));
}
