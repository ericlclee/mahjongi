//! Pure, deterministic mahjong rules engine. No I/O, no storage.

pub mod decompose;
pub mod hand;
pub mod tile;
pub mod wall;

pub use decompose::{win_shapes, winning_tiles, Decomposition, Set, SetKind, WinShape};
pub use hand::{Counts, ParseError};
pub use tile::{Suit, Tile};
pub use wall::{Piece, Wall};
