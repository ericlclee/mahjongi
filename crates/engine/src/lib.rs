//! Pure, deterministic mahjong rules engine. No I/O, no storage.

pub mod builtin;
pub mod decompose;
pub mod game;
pub mod hand;
pub mod pattern;
pub mod rules;
pub mod score;
pub mod simple_bot;
pub mod tile;
pub mod wall;

pub use decompose::{win_shapes, winning_tiles, Decomposition, Set, SetKind, WinShape};
pub use game::{
    Action, Event, Game, GameConfig, GameError, Outcome, PlayerState, Step, View, WinSummary,
    DEAD_WALL, SEATS,
};
pub use hand::{Counts, ParseError};
pub use pattern::{Pattern, TileFilter, WinContext};
pub use rules::{AddOn, Base, HandDef, Op, ResolvedHand, RoomOptions, RuleError, Ruleset};
pub use score::{score, HandMatch, ScoreError, ScoreResult};
pub use tile::{Suit, Tile};
pub use wall::{Piece, Wall};
