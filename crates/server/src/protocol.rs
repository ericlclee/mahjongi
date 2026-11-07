//! Messages exchanged over the WebSocket, as JSON with a `type` tag.

use mahjongi_engine::{Action, View};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSettings {
    pub base: String,
    pub addons: Vec<String>,
    pub disabled_hands: Vec<String>,
    pub threshold_offset: i32,
    pub flowers: bool,
    pub rounds: u32,
    /// Seconds each player gets to act; 0 means no limit.
    #[serde(default)]
    pub turn_secs: u32,
}

impl Default for RoomSettings {
    fn default() -> Self {
        RoomSettings {
            base: "hk_fan".to_string(),
            addons: Vec::new(),
            disabled_hands: Vec::new(),
            threshold_offset: 0,
            flowers: false,
            rounds: 1,
            turn_secs: 0,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Create {
        name: String,
    },
    Join {
        code: String,
        name: String,
        #[serde(default)]
        token: Option<String>,
    },
    SetOptions {
        settings: RoomSettings,
    },
    Start,
    Act {
        action: Action,
    },
    /// Stretch the pause after a discard (only if you could claim the tile).
    Hold,
    NextHand,
    Leave,
    Ping,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlayerInfo {
    pub name: String,
    pub connected: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct HandRef {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AddOnInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub conflicts: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BaseInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct HandValue {
    pub id: String,
    pub name: String,
    pub value: u32,
    pub open_value: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct RulesSummary {
    /// What hands are counted in, e.g. "fan" or "tai".
    pub unit: String,
    pub threshold: u32,
    pub limit: u32,
    pub payout: Vec<u32>,
    pub hands: Vec<HandValue>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)] // messages are built, serialized and dropped immediately
pub enum ServerMsg {
    /// Sent to a client after it creates or joins a room (keep the token to reconnect).
    Joined {
        code: String,
        seat: usize,
        token: String,
    },
    Lobby {
        code: String,
        seat: usize,
        host: usize,
        players: Vec<Option<PlayerInfo>>,
        settings: RoomSettings,
        rules: RulesSummary,
        /// Every hand in the chosen base plus add-ons, before any are switched off.
        all_hands: Vec<HandRef>,
        addons: Vec<AddOnInfo>,
        bases: Vec<BaseInfo>,
        can_start: bool,
    },
    State {
        code: String,
        seat: usize,
        host: usize,
        names: Vec<String>,
        connected: Vec<bool>,
        /// Who has asked to continue to the next hand.
        acks: Vec<bool>,
        rules: RulesSummary,
        /// Seconds allowed per decision; 0 means no limit.
        turn_secs: u32,
        /// Time left before the server plays for whoever is slow, when a timer is running.
        turn_left_ms: Option<u64>,
        /// Time left before the next draw, while a discard can still be claimed.
        claim_left_ms: Option<u64>,
        /// Whether you can stretch that pause (you could claim the tile and nobody has yet).
        can_hold: bool,
        view: View,
    },
    Error {
        message: String,
    },
}
