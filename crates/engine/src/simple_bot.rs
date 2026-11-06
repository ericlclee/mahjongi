//! A tiny heuristic player used for tests and simulations. It is not wired into
//! the multiplayer game (which has no bots yet).

use crate::game::{Action, Game};
use crate::tile::Tile;

/// Pick an action for `seat`, or `None` if that seat has nothing to decide.
pub fn choose(game: &Game, seat: usize) -> Option<Action> {
    let legal = game.legal_actions(seat);
    if legal.is_empty() {
        return None;
    }
    if legal.contains(&Action::Win) {
        return Some(Action::Win);
    }
    if legal.contains(&Action::Pung) {
        return Some(Action::Pung);
    }
    if let Some(kong) = legal
        .iter()
        .find(|a| matches!(a, Action::Kong { .. }))
        .copied()
    {
        return Some(kong);
    }
    if legal.contains(&Action::Pass) {
        return Some(Action::Pass);
    }
    legal
        .iter()
        .filter_map(|a| match a {
            Action::Discard { tile } => Some((usefulness(game, seat, *tile), *tile)),
            _ => None,
        })
        .min_by_key(|(u, t)| (*u, t.index()))
        .map(|(_, tile)| Action::Discard { tile })
}

/// Higher means more worth keeping: pairs/triplets and close neighbours.
fn usefulness(game: &Game, seat: usize, tile: Tile) -> u32 {
    let hand = &game.player(seat).hand;
    let mut u = hand.get(tile) as u32 * 4;
    if tile.number().is_some() {
        let n = tile.number().unwrap_or(0) as i32;
        for d in [-2i32, -1, 1, 2] {
            let m = n + d;
            if (1..=9).contains(&m) {
                let other = Tile((tile.index() as i32 + d) as u8);
                let weight = if d.abs() == 1 { 2 } else { 1 };
                u += hand.get(other) as u32 * weight;
            }
        }
    }
    u
}
