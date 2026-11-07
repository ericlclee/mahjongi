//! Tiny heuristic players. `choose` is used for tests and simulations; `eager` is the bot
//! that can sit at a table in multiplayer rooms.

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

/// The table bot: wins when it can, otherwise takes every pung and chow it is offered, and
/// throws a tile from whichever suit (or honours) it holds fewest of.
pub fn eager(game: &Game, seat: usize) -> Option<Action> {
    use crate::tile::Suit;
    let legal = game.legal_actions(seat);
    if legal.is_empty() {
        return None;
    }
    for wanted in [Action::Win, Action::Pung] {
        if legal.contains(&wanted) {
            return Some(wanted);
        }
    }
    if let Some(chow) = legal.iter().find(|a| matches!(a, Action::Chow { .. })) {
        return Some(*chow);
    }
    if legal.contains(&Action::Pass) {
        return Some(Action::Pass);
    }

    let view = game.view(seat);
    let mut tiles = view.hand.clone();
    tiles.extend(view.drawn);
    let group = |t: Tile| match t.suit() {
        Suit::Man => 0,
        Suit::Pin => 1,
        Suit::Sou => 2,
        Suit::Wind | Suit::Dragon => 3,
    };
    let mut size = [0usize; 4];
    for &t in &tiles {
        size[group(t)] += 1;
    }
    legal
        .iter()
        .filter_map(|a| match a {
            Action::Discard { tile } => Some(*tile),
            _ => None,
        })
        .min_by_key(|&t| (size[group(t)], group(t), t.index()))
        .map(|tile| Action::Discard { tile })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameConfig;
    use crate::{builtin, RoomOptions, Ruleset};

    #[test]
    fn eager_bots_finish_games() {
        for seed in 0..10u64 {
            let options = RoomOptions::default();
            let base = builtin::base("hk_fan").unwrap();
            let rules = Ruleset::resolve(&base, &builtin::addons(), &options).unwrap();
            let mut game = Game::new(GameConfig::new(rules, seed));
            for _ in 0..100_000 {
                if game.is_over() {
                    break;
                }
                if game.is_between_hands() {
                    game.next_hand().unwrap();
                    continue;
                }
                let seat = game.waiting_seats()[0];
                let action = eager(&game, seat).unwrap();
                game.apply(seat, action).unwrap();
            }
            assert!(game.is_over(), "seed {seed}");
        }
    }
}
