use serde::{Deserialize, Serialize};
use std::fmt;

/// One of the 34 tile kinds (flowers are handled separately, see `wall::Piece`).
///
/// Index layout: 0-8 characters (m), 9-17 dots (p), 18-26 bamboo (s),
/// 27-30 winds (E S W N), 31-33 dragons (white, green, red).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct Tile(pub(crate) u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Suit {
    Man,
    Pin,
    Sou,
    Wind,
    Dragon,
}

impl Tile {
    pub const KINDS: usize = 34;

    pub fn new(index: u8) -> Option<Tile> {
        ((index as usize) < Self::KINDS).then_some(Tile(index))
    }

    /// Build a tile from a suit and a 1-based number (winds 1-4, dragons 1-3).
    pub fn from_suit_number(suit: Suit, number: u8) -> Option<Tile> {
        let (base, max) = match suit {
            Suit::Man => (0, 9),
            Suit::Pin => (9, 9),
            Suit::Sou => (18, 9),
            Suit::Wind => (27, 4),
            Suit::Dragon => (31, 3),
        };
        (1..=max).contains(&number).then(|| Tile(base + number - 1))
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn suit(self) -> Suit {
        match self.0 {
            0..=8 => Suit::Man,
            9..=17 => Suit::Pin,
            18..=26 => Suit::Sou,
            27..=30 => Suit::Wind,
            _ => Suit::Dragon,
        }
    }

    /// 1-9 for numbered tiles, `None` for honors.
    pub fn number(self) -> Option<u8> {
        (self.0 < 27).then(|| self.0 % 9 + 1)
    }

    pub fn is_honor(self) -> bool {
        self.0 >= 27
    }

    pub fn is_terminal(self) -> bool {
        matches!(self.number(), Some(1 | 9))
    }

    pub fn is_simple(self) -> bool {
        matches!(self.number(), Some(2..=8))
    }

    pub fn is_terminal_or_honor(self) -> bool {
        self.is_honor() || self.is_terminal()
    }

    /// True if this tile can be the lowest tile of a chow (1-7 of a numbered suit).
    pub fn can_start_chow(self) -> bool {
        self.0 < 27 && self.0 % 9 <= 6
    }

    pub(crate) fn offset(self, by: u8) -> Tile {
        Tile(self.0 + by)
    }

    pub fn all() -> impl Iterator<Item = Tile> {
        (0..Self::KINDS as u8).map(Tile)
    }
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (n, c) = match self.0 {
            0..=8 => (self.0 + 1, 'm'),
            9..=17 => (self.0 - 8, 'p'),
            18..=26 => (self.0 - 17, 's'),
            _ => (self.0 - 26, 'z'),
        };
        write!(f, "{n}{c}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        let one_m = Tile::from_suit_number(Suit::Man, 1).unwrap();
        let five_p = Tile::from_suit_number(Suit::Pin, 5).unwrap();
        let east = Tile::from_suit_number(Suit::Wind, 1).unwrap();
        assert!(one_m.is_terminal() && !one_m.is_simple() && !one_m.is_honor());
        assert!(five_p.is_simple() && !five_p.is_terminal_or_honor());
        assert!(east.is_honor() && east.number().is_none());
        assert!(Tile::from_suit_number(Suit::Dragon, 4).is_none());
    }

    #[test]
    fn chow_starts() {
        let seven_s = Tile::from_suit_number(Suit::Sou, 7).unwrap();
        let eight_s = Tile::from_suit_number(Suit::Sou, 8).unwrap();
        assert!(seven_s.can_start_chow());
        assert!(!eight_s.can_start_chow());
    }

    #[test]
    fn display() {
        assert_eq!(Tile(0).to_string(), "1m");
        assert_eq!(Tile(13).to_string(), "5p");
        assert_eq!(Tile(26).to_string(), "9s");
        assert_eq!(Tile(33).to_string(), "7z");
    }
}
