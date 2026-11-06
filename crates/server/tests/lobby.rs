//! Drives the room logic directly through channels, no sockets involved.

use mahjongi_server::protocol::ClientMsg;
use mahjongi_server::server::{ConnId, Server};
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

struct Client {
    id: ConnId,
    rx: UnboundedReceiver<String>,
}

impl Client {
    fn new(server: &mut Server) -> Client {
        let (tx, rx) = unbounded_channel();
        Client {
            id: server.connect(tx),
            rx,
        }
    }

    fn drain(&mut self) -> Vec<Value> {
        let mut out = Vec::new();
        while let Ok(text) = self.rx.try_recv() {
            out.push(serde_json::from_str(&text).unwrap());
        }
        out
    }

    fn last_of(&mut self, kind: &str) -> Option<Value> {
        self.drain().into_iter().rev().find(|m| m["type"] == kind)
    }
}

fn send(server: &mut Server, client: &Client, msg: Value) {
    server.handle(client.id, serde_json::from_value::<ClientMsg>(msg).unwrap());
}

fn error_text(msgs: &[Value]) -> Option<String> {
    msgs.iter()
        .find(|m| m["type"] == "error")
        .map(|m| m["message"].as_str().unwrap().to_string())
}

/// Create a room with four seated players.
fn full_room(server: &mut Server) -> (String, Vec<Client>, Vec<String>) {
    let mut host = Client::new(server);
    send(server, &host, json!({"type": "create", "name": "Host"}));
    let joined = host.last_of("joined").expect("joined");
    let code = joined["code"].as_str().unwrap().to_string();
    let mut tokens = vec![joined["token"].as_str().unwrap().to_string()];
    let mut clients = vec![host];
    for i in 1..4 {
        let mut c = Client::new(server);
        send(
            server,
            &c,
            json!({"type": "join", "code": code, "name": format!("P{i}")}),
        );
        let joined = c.last_of("joined").expect("joined");
        assert_eq!(joined["seat"], i);
        tokens.push(joined["token"].as_str().unwrap().to_string());
        clients.push(c);
    }
    (code, clients, tokens)
}

#[test]
fn four_players_fill_a_room_and_see_the_lobby() {
    let mut server = Server::new();
    let (code, mut clients, _) = full_room(&mut server);
    assert_eq!(code.len(), 4);

    let lobby = clients[2].last_of("lobby").expect("lobby");
    assert_eq!(lobby["can_start"], true);
    assert_eq!(lobby["host"], 0);
    assert_eq!(lobby["players"][3]["name"], "P3");
    assert_eq!(lobby["rules"]["threshold"], 3);
    assert!(lobby["all_hands"].as_array().unwrap().len() >= 16);
    assert!(!lobby["addons"].as_array().unwrap().is_empty());

    // A fifth player is turned away.
    let mut extra = Client::new(&mut server);
    send(
        &mut server,
        &extra,
        json!({"type": "join", "code": code.to_lowercase(), "name": "Late"}),
    );
    assert_eq!(
        error_text(&extra.drain()).as_deref(),
        Some("that room is full")
    );
}

#[test]
fn only_the_host_changes_rules_and_starts() {
    let mut server = Server::new();
    let (_, mut clients, _) = full_room(&mut server);
    clients.iter_mut().for_each(|c| {
        c.drain();
    });

    let settings = json!({
        "base": "hk_fan", "addons": ["all_simples", "lenient_threshold"],
        "disabled_hands": ["seven_pairs"], "threshold_offset": 0,
        "flowers": true, "rounds": 2
    });
    send(
        &mut server,
        &clients[1],
        json!({"type": "set_options", "settings": settings}),
    );
    assert!(error_text(&clients[1].drain())
        .unwrap()
        .contains("only the host"));
    send(&mut server, &clients[1], json!({"type": "start"}));
    assert!(error_text(&clients[1].drain())
        .unwrap()
        .contains("only the host"));

    send(
        &mut server,
        &clients[0],
        json!({"type": "set_options", "settings": settings}),
    );
    let lobby = clients[3].last_of("lobby").expect("everyone is updated");
    assert_eq!(lobby["rules"]["threshold"], 2);
    assert_eq!(lobby["settings"]["rounds"], 2);
    let hand_ids: Vec<&str> = lobby["rules"]["hands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["id"].as_str().unwrap())
        .collect();
    assert!(hand_ids.contains(&"all_simples"));
    assert!(!hand_ids.contains(&"seven_pairs"));
    // The switched-off hand still shows in the toggle list.
    let all: Vec<&str> = lobby["all_hands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["id"].as_str().unwrap())
        .collect();
    assert!(all.contains(&"seven_pairs"));
}

#[test]
fn bad_rule_choices_are_rejected() {
    let mut server = Server::new();
    let (_, mut clients, _) = full_room(&mut server);
    clients[0].drain();
    let base = json!({
        "base": "hk_fan", "addons": [], "disabled_hands": [],
        "threshold_offset": 0, "flowers": false, "rounds": 1
    });
    for (field, value, expected) in [
        ("addons", json!(["nope"]), "unknown add-on"),
        (
            "addons",
            json!(["lenient_threshold", "strict_threshold"]),
            "conflict",
        ),
        ("rounds", json!(9), "rounds"),
        ("threshold_offset", json!(50), "threshold"),
        ("base", json!("mystery"), "unknown ruleset"),
    ] {
        let mut settings = base.clone();
        settings[field] = value;
        send(
            &mut server,
            &clients[0],
            json!({"type": "set_options", "settings": settings}),
        );
        let err = error_text(&clients[0].drain()).unwrap_or_default();
        assert!(err.contains(expected), "{field}: got '{err}'");
    }
}

#[test]
fn start_needs_four_players_then_deals() {
    let mut server = Server::new();
    let mut host = Client::new(&mut server);
    send(
        &mut server,
        &host,
        json!({"type": "create", "name": "Solo"}),
    );
    host.drain();
    send(&mut server, &host, json!({"type": "start"}));
    assert!(error_text(&host.drain()).unwrap().contains("four players"));

    let (_, mut clients, _) = full_room(&mut server);
    clients.iter_mut().for_each(|c| {
        c.drain();
    });
    send(&mut server, &clients[0], json!({"type": "start"}));

    let dealer = clients[0].last_of("state").expect("state");
    assert_eq!(dealer["view"]["phase"], "discard");
    assert_eq!(dealer["view"]["hand"].as_array().unwrap().len(), 13);
    assert!(dealer["view"]["drawn"].is_number());
    assert!(!dealer["view"]["legal"].as_array().unwrap().is_empty());

    let other = clients[2].last_of("state").expect("state");
    assert_eq!(other["view"]["hand"].as_array().unwrap().len(), 13);
    assert!(other["view"]["legal"].as_array().unwrap().is_empty());
    assert_eq!(other["names"][0], "Host");

    // Settings are locked and late joiners cannot take a seat once the game is on.
    send(
        &mut server,
        &clients[1],
        json!({"type": "act", "action": {"type": "pass"}}),
    );
    assert!(error_text(&clients[1].drain()).is_some());
    let mut late = Client::new(&mut server);
    send(
        &mut server,
        &late,
        json!({"type": "join", "code": dealer["code"], "name": "Late"}),
    );
    assert!(error_text(&late.drain())
        .unwrap()
        .contains("already started"));
}

#[test]
fn a_dropped_player_can_rejoin_with_their_token() {
    let mut server = Server::new();
    let (code, mut clients, tokens) = full_room(&mut server);
    send(&mut server, &clients[0], json!({"type": "start"}));
    clients.iter_mut().for_each(|c| {
        c.drain();
    });

    let dropped = clients.remove(2);
    server.disconnect(dropped.id);
    let state = clients[0].last_of("state").expect("others are told");
    assert_eq!(state["connected"], json!([true, true, false, true]));

    let mut back = Client::new(&mut server);
    send(
        &mut server,
        &back,
        json!({"type": "join", "code": code, "name": "ignored", "token": tokens[2]}),
    );
    let msgs = back.drain();
    let find = |kind: &str| msgs.iter().find(|m| m["type"] == kind).cloned();
    assert_eq!(find("joined").expect("rejoined")["seat"], 2);
    let state = find("state").expect("gets the table back");
    assert_eq!(state["seat"], 2);
    assert_eq!(state["view"]["hand"].as_array().unwrap().len(), 13);

    // A wrong token does not take the seat.
    let mut thief = Client::new(&mut server);
    send(
        &mut server,
        &thief,
        json!({"type": "join", "code": code, "name": "Thief", "token": "bogus"}),
    );
    assert!(error_text(&thief.drain()).is_some());
}

#[test]
fn lobby_seats_free_up_and_empty_rooms_vanish() {
    let mut server = Server::new();
    let (code, mut clients, _) = full_room(&mut server);

    // Host leaves the lobby: next player becomes host.
    let host = clients.remove(0);
    server.disconnect(host.id);
    let lobby = clients[0].last_of("lobby").expect("lobby");
    assert_eq!(lobby["host"], 1);
    assert_eq!(lobby["can_start"], false);

    // The freed seat can be taken again.
    let mut newcomer = Client::new(&mut server);
    send(
        &mut server,
        &newcomer,
        json!({"type": "join", "code": code, "name": "New"}),
    );
    assert_eq!(newcomer.last_of("joined").unwrap()["seat"], 0);

    for c in clients {
        server.disconnect(c.id);
    }
    server.disconnect(newcomer.id);
    assert_eq!(server.room_count(), 0);
}

fn timed_settings(secs: u32) -> Value {
    json!({
        "base": "hk_fan", "addons": [], "disabled_hands": [],
        "threshold_offset": 0, "flowers": false, "rounds": 1, "turn_secs": secs
    })
}

#[test]
fn turn_timer_must_be_off_or_a_sensible_length() {
    let mut server = Server::new();
    let (_, mut clients, _) = full_room(&mut server);
    clients[0].drain();
    send(
        &mut server,
        &clients[0],
        json!({"type": "set_options", "settings": timed_settings(2)}),
    );
    assert!(error_text(&clients[0].drain())
        .unwrap()
        .contains("turn timer"));
    send(
        &mut server,
        &clients[0],
        json!({"type": "set_options", "settings": timed_settings(30)}),
    );
    let lobby = clients[0].last_of("lobby").expect("lobby");
    assert_eq!(lobby["settings"]["turn_secs"], 30);
}

#[test]
fn slow_players_are_played_for_when_the_timer_runs_out() {
    let mut server = Server::new();
    let (_, mut clients, _) = full_room(&mut server);
    send(
        &mut server,
        &clients[0],
        json!({"type": "set_options", "settings": timed_settings(10)}),
    );
    send(&mut server, &clients[0], json!({"type": "start"}));
    let before = clients[0].last_of("state").expect("state");
    assert_eq!(before["turn_secs"], 10);
    let left = before["turn_left_ms"].as_u64().expect("timer is running");
    assert!(left > 8_000 && left <= 10_000, "{left}");
    assert!(!before["view"]["legal"].as_array().unwrap().is_empty());
    clients.iter_mut().for_each(|c| {
        c.drain();
    });

    // Nothing happens before the deadline.
    server.tick_at(std::time::Instant::now() + std::time::Duration::from_secs(5));
    assert!(clients[0].last_of("state").is_none());

    // After it, the dealer's drawn tile is thrown and everyone sees the discard.
    server.tick_at(std::time::Instant::now() + std::time::Duration::from_secs(11));
    let after = clients[0].last_of("state").expect("table moves on");
    assert_eq!(after["view"]["last_event"]["type"], "discard");
    assert_eq!(after["view"]["last_event"]["tile"], before["view"]["drawn"]);
}

#[test]
fn no_timer_means_no_deadline() {
    let mut server = Server::new();
    let (_, mut clients, _) = full_room(&mut server);
    send(&mut server, &clients[0], json!({"type": "start"}));
    let state = clients[0].last_of("state").expect("state");
    assert_eq!(state["turn_secs"], 0);
    assert!(state["turn_left_ms"].is_null());
    clients.iter_mut().for_each(|c| {
        c.drain();
    });
    server.tick_at(std::time::Instant::now() + std::time::Duration::from_secs(3600));
    assert!(clients[0].last_of("state").is_none());
}

#[test]
fn the_host_can_pick_another_ruleset() {
    let mut server = Server::new();
    let (_, mut clients, _) = full_room(&mut server);
    let lobby = clients[0].last_of("lobby").expect("lobby");
    assert!(lobby["bases"].as_array().unwrap().len() >= 3);
    assert_eq!(lobby["rules"]["unit"], "fan");

    let mut settings = timed_settings(0);
    settings["base"] = json!("sg_tai");
    send(
        &mut server,
        &clients[0],
        json!({"type": "set_options", "settings": settings}),
    );
    let lobby = clients[1].last_of("lobby").expect("lobby");
    assert_eq!(lobby["rules"]["unit"], "tai");
    assert_eq!(lobby["rules"]["threshold"], 1);
    assert!(lobby["addons"].as_array().unwrap().is_empty());
}

#[test]
fn the_table_does_not_wait_forever_on_a_disconnected_player() {
    use std::time::{Duration, Instant};
    let mut server = Server::new();
    let (_, mut clients, _) = full_room(&mut server);
    send(&mut server, &clients[0], json!({"type": "start"}));
    // The dealer drops out right as it becomes their turn.
    let dealer = clients.remove(0);
    server.disconnect(dealer.id);
    clients.iter_mut().for_each(|c| {
        c.drain();
    });

    let t0 = Instant::now();
    server.tick_at(t0);
    server.tick_at(t0 + Duration::from_secs(5));
    assert!(clients[0].last_of("state").is_none(), "grace period first");

    server.tick_at(t0 + Duration::from_secs(11));
    let state = clients[0].last_of("state").expect("server played for them");
    assert_eq!(state["view"]["last_event"]["type"], "discard");
    assert_eq!(state["view"]["last_event"]["seat"], 0);
}
