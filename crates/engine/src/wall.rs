use crate::tile::Tile;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// Anything that can sit in the wall: a regular tile or one of 8 flowers/seasons.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Piece {
    Tile(Tile),
    /// Flower/season index 0-7.
    Flower(u8),
}

/// A shuffled wall. Shuffling is fully determined by the seed, so games can be replayed.
#[derive(Clone, Debug)]
pub struct Wall {
    pieces: Vec<Piece>,
}

impl Wall {
    /// 136 tiles, or 144 with the 8 flowers.
    pub fn new(include_flowers: bool, seed: u64) -> Wall {
        let mut pieces: Vec<Piece> = Tile::all()
            .flat_map(|t| std::iter::repeat_n(Piece::Tile(t), 4))
            .collect();
        if include_flowers {
            pieces.extend((0..8).map(Piece::Flower));
        }
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        pieces.shuffle(&mut rng);
        Wall { pieces }
    }

    pub fn len(&self) -> usize {
        self.pieces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// Normal draw from the front of the wall.
    pub fn draw(&mut self) -> Option<Piece> {
        if self.pieces.is_empty() {
            None
        } else {
            Some(self.pieces.remove(0))
        }
    }

    /// Replacement draw from the back (after a kong or a flower).
    pub fn draw_replacement(&mut self) -> Option<Piece> {
        self.pieces.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(Wall::new(false, 1).len(), 136);
        assert_eq!(Wall::new(true, 1).len(), 144);
    }

    #[test]
    fn four_of_each_tile() {
        let mut wall = Wall::new(true, 7);
        let mut tiles = [0u8; Tile::KINDS];
        let mut flowers = [0u8; 8];
        while let Some(p) = wall.draw() {
            match p {
                Piece::Tile(t) => tiles[t.index()] += 1,
                Piece::Flower(f) => flowers[f as usize] += 1,
            }
        }
        assert!(tiles.iter().all(|&c| c == 4));
        assert!(flowers.iter().all(|&c| c == 1));
    }

    #[test]
    fn seed_is_deterministic() {
        let mut a = Wall::new(true, 42);
        let mut b = Wall::new(true, 42);
        let mut c = Wall::new(true, 43);
        let (da, db, dc): (Vec<_>, Vec<_>, Vec<_>) = (
            (0..20).filter_map(|_| a.draw()).collect(),
            (0..20).filter_map(|_| b.draw()).collect(),
            (0..20).filter_map(|_| c.draw()).collect(),
        );
        assert_eq!(da, db);
        assert_ne!(da, dc);
    }

    #[test]
    fn replacement_draws_from_the_back() {
        let mut wall = Wall::new(false, 3);
        let last = *wall.pieces.last().unwrap();
        assert_eq!(wall.draw_replacement(), Some(last));
        assert_eq!(wall.len(), 135);
    }
}
