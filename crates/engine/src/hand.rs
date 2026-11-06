use crate::tile::Tile;
use std::fmt;

/// How many of each of the 34 tile kinds a hand holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Counts([u8; Tile::KINDS]);

impl Default for Counts {
    fn default() -> Self {
        Counts([0; Tile::KINDS])
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ParseError {
    BadChar(char),
    SuitWithoutDigits,
    TrailingDigits,
    InvalidNumber(char),
    TooManyCopies(Tile),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::BadChar(c) => write!(f, "unexpected character '{c}'"),
            ParseError::SuitWithoutDigits => write!(f, "suit letter with no digits before it"),
            ParseError::TrailingDigits => write!(f, "digits not followed by a suit letter"),
            ParseError::InvalidNumber(c) => write!(f, "invalid tile number '{c}'"),
            ParseError::TooManyCopies(t) => write!(f, "more than 4 copies of {t}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl Counts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, tile: Tile) -> u8 {
        self.0[tile.index()]
    }

    pub fn total(&self) -> usize {
        self.0.iter().map(|&c| c as usize).sum()
    }

    pub fn add(&mut self, tile: Tile) {
        self.add_n(tile, 1);
    }

    pub fn add_n(&mut self, tile: Tile, n: u8) {
        self.0[tile.index()] += n;
    }

    /// Remove `n` copies. Panics if the hand holds fewer (internal invariant).
    pub fn remove_n(&mut self, tile: Tile, n: u8) {
        self.0[tile.index()] -= n;
    }

    pub fn from_tiles(tiles: &[Tile]) -> Self {
        let mut counts = Counts::new();
        for &t in tiles {
            counts.add(t);
        }
        counts
    }

    /// Parse standard notation, e.g. `"123m456p789s1122z"`.
    /// m = characters, p = dots, s = bamboo, z = honors (1-4 winds E S W N, 5-7 dragons).
    pub fn parse(s: &str) -> Result<Counts, ParseError> {
        let mut counts = Counts::new();
        let mut pending: Vec<char> = Vec::new();
        for ch in s.chars() {
            match ch {
                '0'..='9' => pending.push(ch),
                'm' | 'p' | 's' | 'z' => {
                    if pending.is_empty() {
                        return Err(ParseError::SuitWithoutDigits);
                    }
                    let (base, max) = match ch {
                        'm' => (0u8, 9u8),
                        'p' => (9, 9),
                        's' => (18, 9),
                        _ => (27, 7),
                    };
                    for d in pending.drain(..) {
                        let n = d as u8 - b'0';
                        if n == 0 || n > max {
                            return Err(ParseError::InvalidNumber(d));
                        }
                        let tile = Tile(base + n - 1);
                        if counts.get(tile) == 4 {
                            return Err(ParseError::TooManyCopies(tile));
                        }
                        counts.add(tile);
                    }
                }
                c if c.is_whitespace() => {}
                c => return Err(ParseError::BadChar(c)),
            }
        }
        if pending.is_empty() {
            Ok(counts)
        } else {
            Err(ParseError::TrailingDigits)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notation() {
        let c = Counts::parse("123m456p789s1122z").unwrap();
        assert_eq!(c.total(), 13);
        assert_eq!(c.get(Tile(0)), 1);
        assert_eq!(c.get(Tile(27)), 2);
        assert_eq!(c.get(Tile(28)), 2);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(Counts::parse("12"), Err(ParseError::TrailingDigits));
        assert_eq!(Counts::parse("m"), Err(ParseError::SuitWithoutDigits));
        assert_eq!(Counts::parse("0m"), Err(ParseError::InvalidNumber('0')));
        assert_eq!(Counts::parse("8z"), Err(ParseError::InvalidNumber('8')));
        assert_eq!(Counts::parse("1x"), Err(ParseError::BadChar('x')));
        assert_eq!(
            Counts::parse("11111m"),
            Err(ParseError::TooManyCopies(Tile(0)))
        );
    }
}
