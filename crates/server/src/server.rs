//! Room and lobby logic. Pure of any networking: clients are channels of JSON strings,
//! so the whole thing can be driven directly from tests.

use crate::protocol::*;
use mahjongi_engine::{
    builtin, Action, AddOn, Base, Game, GameConfig, RoomOptions, Ruleset, SEATS,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;

pub type ConnId = u64;

const ROOM_IDLE_LIMIT: Duration = Duration::from_secs(15 * 60);

struct Conn {
    tx: UnboundedSender<String>,
    /// Room code and seat this connection plays in.
    at: Option<(String, usize)>,
}

struct SeatInfo {
    name: String,
    token: String,
    conn: Option<ConnId>,
}

struct Room {
    seats: [Option<SeatInfo>; SEATS],
    host: usize,
    settings: RoomSettings,
    game: Option<Game>,
    acks: [bool; SEATS],
    /// When the server will play for whoever is still deciding.
    deadline: Option<Instant>,
    empty_since: Option<Instant>,
}

pub struct Server {
    rooms: HashMap<String, Room>,
    conns: HashMap<ConnId, Conn>,
    next_conn: ConnId,
    bases: Vec<Base>,
    addons: Vec<AddOn>,
}

impl Room {
    /// Start a fresh countdown if a game is running with a timer and someone has to decide.
    fn reset_deadline(&mut self) {
        let running = self
            .game
            .as_ref()
            .is_some_and(|g| !g.is_over() && !g.is_between_hands());
        self.deadline = (running && self.settings.turn_secs > 0)
            .then(|| Instant::now() + Duration::from_secs(self.settings.turn_secs as u64));
    }
}

/// What the server plays when time runs out: pass on a claim, otherwise throw back the
/// tile just drawn (or the last tile of the hand).
fn timeout_action(game: &Game, seat: usize) -> Option<Action> {
    let legal = game.legal_actions(seat);
    if legal.contains(&Action::Pass) {
        return Some(Action::Pass);
    }
    let drawn = game.view(seat).drawn;
    legal
        .iter()
        .copied()
        .find(|a| matches!(a, Action::Discard { tile } if Some(*tile) == drawn))
        .or_else(|| {
            legal
                .iter()
                .rev()
                .copied()
                .find(|a| matches!(a, Action::Discard { .. }))
        })
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

fn clean_name(raw: &str) -> Result<String, String> {
    let name: String = raw.trim().chars().take(20).collect();
    if name.is_empty() {
        Err("please enter a name".to_string())
    } else {
        Ok(name)
    }
}

fn random_token() -> String {
    format!(
        "{:016x}{:016x}",
        rand::random::<u64>(),
        rand::random::<u64>()
    )
}

fn random_code() -> String {
    const LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    (0..4)
        .map(|_| LETTERS[rand::random::<usize>() % LETTERS.len()] as char)
        .collect()
}

impl Server {
    pub fn new() -> Server {
        Server {
            rooms: HashMap::new(),
            conns: HashMap::new(),
            next_conn: 1,
            bases: builtin::bases(),
            addons: builtin::addons(),
        }
    }

    pub fn connect(&mut self, tx: UnboundedSender<String>) -> ConnId {
        let id = self.next_conn;
        self.next_conn += 1;
        self.conns.insert(id, Conn { tx, at: None });
        id
    }

    pub fn disconnect(&mut self, conn: ConnId) {
        let Some(c) = self.conns.remove(&conn) else {
            return;
        };
        if let Some((code, seat)) = c.at {
            self.detach_seat(&code, seat, conn);
        }
    }

    /// Remove rooms nobody has been connected to for a while.
    pub fn sweep(&mut self) {
        self.rooms.retain(|_, room| {
            room.empty_since
                .is_none_or(|since| since.elapsed() < ROOM_IDLE_LIMIT)
        });
    }

    /// Play for anyone who has run out of time. Call about once a second.
    pub fn tick(&mut self) {
        self.tick_at(Instant::now());
    }

    pub fn tick_at(&mut self, now: Instant) {
        let due: Vec<String> = self
            .rooms
            .iter()
            .filter(|(_, r)| r.deadline.is_some_and(|d| d <= now))
            .map(|(code, _)| code.clone())
            .collect();
        for code in due {
            let Some(room) = self.rooms.get_mut(&code) else {
                continue;
            };
            if let Some(game) = room.game.as_mut() {
                for seat in game.waiting_seats() {
                    if let Some(action) = timeout_action(game, seat) {
                        let _ = game.apply(seat, action);
                    }
                }
            }
            room.reset_deadline();
            self.broadcast(&code);
        }
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    pub fn handle(&mut self, conn: ConnId, msg: ClientMsg) {
        let result = match msg {
            ClientMsg::Create { name } => self.create(conn, &name),
            ClientMsg::Join { code, name, token } => self.join(conn, &code, &name, token),
            ClientMsg::SetOptions { settings } => self.set_options(conn, settings),
            ClientMsg::Start => self.start(conn),
            ClientMsg::Act { action } => self.act(conn, action),
            ClientMsg::NextHand => self.next_hand(conn),
            ClientMsg::Leave => self.leave(conn),
            ClientMsg::Ping => Ok(()),
        };
        if let Err(message) = result {
            self.send(conn, &ServerMsg::Error { message });
        }
    }

    // ---- message handlers ----

    fn create(&mut self, conn: ConnId, name: &str) -> Result<(), String> {
        self.require_free(conn)?;
        let name = clean_name(name)?;
        let mut code = random_code();
        while self.rooms.contains_key(&code) {
            code = random_code();
        }
        let token = random_token();
        let mut seats: [Option<SeatInfo>; SEATS] = Default::default();
        seats[0] = Some(SeatInfo {
            name,
            token: token.clone(),
            conn: Some(conn),
        });
        self.rooms.insert(
            code.clone(),
            Room {
                seats,
                host: 0,
                settings: RoomSettings::default(),
                game: None,
                acks: [false; SEATS],
                deadline: None,
                empty_since: None,
            },
        );
        self.set_location(conn, &code, 0);
        self.send(
            conn,
            &ServerMsg::Joined {
                code: code.clone(),
                seat: 0,
                token,
            },
        );
        self.broadcast(&code);
        Ok(())
    }

    fn join(
        &mut self,
        conn: ConnId,
        code: &str,
        name: &str,
        token: Option<String>,
    ) -> Result<(), String> {
        self.require_free(conn)?;
        let code = code.trim().to_uppercase();
        let room = self.rooms.get_mut(&code).ok_or("room not found")?;

        let returning = token.filter(|t| !t.is_empty()).and_then(|t| {
            room.seats
                .iter()
                .position(|s| s.as_ref().is_some_and(|s| s.token == t))
        });

        let (seat, token, displaced) = match returning {
            Some(seat) => {
                let info = room.seats[seat].as_mut().expect("seat was matched");
                let displaced = info.conn.replace(conn);
                (seat, info.token.clone(), displaced)
            }
            None => {
                if room.game.is_some() {
                    return Err("that game has already started".to_string());
                }
                let name = clean_name(name)?;
                let seat = room
                    .seats
                    .iter()
                    .position(Option::is_none)
                    .ok_or("that room is full")?;
                let token = random_token();
                room.seats[seat] = Some(SeatInfo {
                    name,
                    token: token.clone(),
                    conn: Some(conn),
                });
                (seat, token, None)
            }
        };
        room.empty_since = None;

        if let Some(old) = displaced {
            if let Some(c) = self.conns.get_mut(&old) {
                c.at = None;
            }
        }
        self.set_location(conn, &code, seat);
        self.send(
            conn,
            &ServerMsg::Joined {
                code: code.clone(),
                seat,
                token,
            },
        );
        self.broadcast(&code);
        Ok(())
    }

    fn set_options(&mut self, conn: ConnId, mut settings: RoomSettings) -> Result<(), String> {
        let (code, seat) = self.location(conn)?;
        {
            let room = &self.rooms[&code];
            if seat != room.host {
                return Err("only the host can change the rules".to_string());
            }
            if room.game.as_ref().is_some_and(|g| !g.is_over()) {
                return Err("rules are locked while a game is running".to_string());
            }
        }
        if !(1..=4).contains(&settings.rounds) {
            return Err("rounds must be between 1 and 4".to_string());
        }
        if settings.turn_secs != 0 && !(5..=300).contains(&settings.turn_secs) {
            return Err("turn timer must be off or between 5 and 300 seconds".to_string());
        }
        if !(-10..=10).contains(&settings.threshold_offset) {
            return Err("threshold change must be between -10 and 10".to_string());
        }
        // Drop switched-off hands that this base and add-on combination no longer has.
        let all = self.resolve(&RoomSettings {
            disabled_hands: Vec::new(),
            ..settings.clone()
        })?;
        settings
            .disabled_hands
            .retain(|id| all.hands.iter().any(|h| &h.id == id));
        self.resolve(&settings)?;
        self.rooms.get_mut(&code).expect("room exists").settings = settings;
        self.broadcast(&code);
        Ok(())
    }

    fn start(&mut self, conn: ConnId) -> Result<(), String> {
        let (code, seat) = self.location(conn)?;
        let settings = {
            let room = &self.rooms[&code];
            if seat != room.host {
                return Err("only the host can start the game".to_string());
            }
            if room.game.as_ref().is_some_and(|g| !g.is_over()) {
                return Err("the game is already running".to_string());
            }
            if room.seats.iter().any(Option::is_none) {
                return Err("waiting for four players".to_string());
            }
            room.settings.clone()
        };
        let rules = self.resolve(&settings)?;
        let mut config = GameConfig::new(rules, rand::random());
        config.flowers = settings.flowers;
        config.rounds = settings.rounds;
        let room = self.rooms.get_mut(&code).expect("room exists");
        room.game = Some(Game::new(config));
        room.acks = [false; SEATS];
        room.reset_deadline();
        self.broadcast(&code);
        Ok(())
    }

    fn act(&mut self, conn: ConnId, action: Action) -> Result<(), String> {
        let (code, seat) = self.location(conn)?;
        let room = self.rooms.get_mut(&code).expect("room exists");
        let game = room.game.as_mut().ok_or("the game has not started")?;
        game.apply(seat, action).map_err(|e| e.to_string())?;
        room.reset_deadline();
        self.broadcast(&code);
        Ok(())
    }

    fn next_hand(&mut self, conn: ConnId) -> Result<(), String> {
        let (code, seat) = self.location(conn)?;
        let room = self.rooms.get_mut(&code).expect("room exists");
        let game = room.game.as_mut().ok_or("the game has not started")?;
        if !game.is_between_hands() {
            return Err("there is no hand waiting to continue".to_string());
        }
        room.acks[seat] = true;
        let everyone_ready = room
            .seats
            .iter()
            .zip(room.acks)
            .all(|(s, acked)| s.as_ref().is_none_or(|s| s.conn.is_none()) || acked);
        if everyone_ready {
            game.next_hand().map_err(|e| e.to_string())?;
            room.acks = [false; SEATS];
            room.reset_deadline();
        }
        self.broadcast(&code);
        Ok(())
    }

    fn leave(&mut self, conn: ConnId) -> Result<(), String> {
        let (code, seat) = self.location(conn)?;
        if let Some(c) = self.conns.get_mut(&conn) {
            c.at = None;
        }
        self.detach_seat(&code, seat, conn);
        Ok(())
    }

    // ---- helpers ----

    fn require_free(&self, conn: ConnId) -> Result<(), String> {
        match self.conns.get(&conn) {
            Some(Conn { at: None, .. }) => Ok(()),
            Some(_) => Err("you are already in a room".to_string()),
            None => Err("unknown connection".to_string()),
        }
    }

    fn location(&self, conn: ConnId) -> Result<(String, usize), String> {
        self.conns
            .get(&conn)
            .and_then(|c| c.at.clone())
            .ok_or_else(|| "you are not in a room".to_string())
    }

    fn set_location(&mut self, conn: ConnId, code: &str, seat: usize) {
        if let Some(c) = self.conns.get_mut(&conn) {
            c.at = Some((code.to_string(), seat));
        }
    }

    /// A connection stopped playing at `seat`: free the seat in the lobby, hold it in a game.
    fn detach_seat(&mut self, code: &str, seat: usize, conn: ConnId) {
        let Some(room) = self.rooms.get_mut(code) else {
            return;
        };
        let owns_seat = room.seats[seat]
            .as_ref()
            .is_some_and(|s| s.conn == Some(conn));
        if !owns_seat {
            return;
        }
        if room.game.is_none() {
            room.seats[seat] = None;
            if room.host == seat {
                if let Some(next) = room.seats.iter().position(Option::is_some) {
                    room.host = next;
                }
            }
        } else if let Some(info) = room.seats[seat].as_mut() {
            info.conn = None;
        }
        let nobody_connected = room.seats.iter().flatten().all(|s| s.conn.is_none());
        if nobody_connected {
            if room.game.is_none() {
                self.rooms.remove(code);
                return;
            }
            room.empty_since = Some(Instant::now());
        }
        self.broadcast(code);
    }

    fn resolve(&self, settings: &RoomSettings) -> Result<Ruleset, String> {
        let base = self
            .bases
            .iter()
            .find(|b| b.id == settings.base)
            .ok_or_else(|| format!("unknown ruleset '{}'", settings.base))?;
        let options = RoomOptions {
            addons: settings.addons.clone(),
            disabled_hands: settings.disabled_hands.clone(),
            threshold_offset: settings.threshold_offset,
        };
        Ruleset::resolve(base, &self.addons, &options).map_err(|e| e.to_string())
    }

    fn summary(rules: &Ruleset) -> RulesSummary {
        RulesSummary {
            unit: rules.model.clone(),
            threshold: rules.threshold,
            limit: rules.limit,
            payout: rules.payout.clone(),
            hands: rules
                .hands
                .iter()
                .map(|h| HandValue {
                    id: h.id.clone(),
                    name: h.name.clone(),
                    value: h.value,
                    open_value: h.open_value,
                })
                .collect(),
        }
    }

    fn send(&self, conn: ConnId, msg: &ServerMsg) {
        if let (Some(c), Ok(text)) = (self.conns.get(&conn), serde_json::to_string(msg)) {
            let _ = c.tx.send(text);
        }
    }

    fn broadcast(&self, code: &str) {
        let Some(room) = self.rooms.get(code) else {
            return;
        };
        for (seat, info) in room.seats.iter().enumerate() {
            let Some(conn) = info.as_ref().and_then(|s| s.conn) else {
                continue;
            };
            if let Some(msg) = self.message_for(code, room, seat) {
                self.send(conn, &msg);
            }
        }
    }

    fn message_for(&self, code: &str, room: &Room, seat: usize) -> Option<ServerMsg> {
        let rules = self.resolve(&room.settings).ok()?;
        if let Some(game) = &room.game {
            return Some(ServerMsg::State {
                code: code.to_string(),
                seat,
                host: room.host,
                names: room
                    .seats
                    .iter()
                    .map(|s| s.as_ref().map(|s| s.name.clone()).unwrap_or_default())
                    .collect(),
                connected: room
                    .seats
                    .iter()
                    .map(|s| s.as_ref().is_some_and(|s| s.conn.is_some()))
                    .collect(),
                acks: room.acks.to_vec(),
                rules: Self::summary(&rules),
                turn_secs: room.settings.turn_secs,
                turn_left_ms: room
                    .deadline
                    .map(|d| d.saturating_duration_since(Instant::now()).as_millis() as u64),
                view: game.view(seat),
            });
        }
        let all = self
            .resolve(&RoomSettings {
                disabled_hands: Vec::new(),
                ..room.settings.clone()
            })
            .ok()?;
        Some(ServerMsg::Lobby {
            code: code.to_string(),
            seat,
            host: room.host,
            players: room
                .seats
                .iter()
                .map(|s| {
                    s.as_ref().map(|s| PlayerInfo {
                        name: s.name.clone(),
                        connected: s.conn.is_some(),
                    })
                })
                .collect(),
            settings: room.settings.clone(),
            rules: Self::summary(&rules),
            all_hands: all
                .hands
                .iter()
                .map(|h| HandRef {
                    id: h.id.clone(),
                    name: h.name.clone(),
                })
                .collect(),
            addons: self
                .addons
                .iter()
                .filter(|a| {
                    self.bases
                        .iter()
                        .find(|b| b.id == room.settings.base)
                        .is_some_and(|b| a.supported_by(b))
                })
                .map(|a| AddOnInfo {
                    id: a.id.clone(),
                    name: a.name.clone(),
                    description: a.description.clone(),
                    conflicts: a.conflicts.clone(),
                })
                .collect(),
            bases: self
                .bases
                .iter()
                .map(|b| BaseInfo {
                    id: b.id.clone(),
                    name: b.name.clone(),
                })
                .collect(),
            can_start: room.seats.iter().all(Option::is_some),
        })
    }
}
