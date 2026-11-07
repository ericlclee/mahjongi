//! The game state machine: dealing, turns, claims, kongs, flowers, payments
//! and dealer rotation for a four-player game. Pure and deterministic: the same
//! config and the same sequence of steps always give the same game.

use crate::decompose::{Set, SetKind};
use crate::hand::Counts;
use crate::pattern::WinContext;
use crate::rules::Ruleset;
use crate::score::{score, HandMatch, ScoreError, ScoreResult};
use crate::tile::Tile;
use crate::wall::{Piece, Wall};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const SEATS: usize = 4;
/// Tiles at the back of the wall that are never drawn normally.
pub const DEAD_WALL: usize = 14;

#[derive(Clone, Debug)]
pub struct GameConfig {
    pub rules: Ruleset,
    pub flowers: bool,
    /// Wind rounds to play (1 = East round only, 4 = a full game).
    pub rounds: u32,
    /// Payments to or from the dealer are doubled.
    pub dealer_double: bool,
    /// After every discard, wait for `close_claims` before the next draw, even when nobody
    /// can claim, so the pause does not reveal who could. Answers that claim the tile still
    /// resolve as soon as everyone eligible has answered.
    pub claim_window: bool,
    pub seed: u64,
}

impl GameConfig {
    pub fn new(rules: Ruleset, seed: u64) -> Self {
        GameConfig {
            rules,
            flowers: false,
            rounds: 1,
            dealer_double: true,
            claim_window: false,
            seed,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Discard {
        tile: Tile,
    },
    /// Win on your own draw, or on the current discard.
    Win,
    /// Claim the current discard for a pung.
    Pung,
    /// Declare a kong: concealed or added on your turn, or claimed from a discard.
    Kong {
        tile: Tile,
    },
    /// Claim the current discard for a chow whose lowest tile is `low`.
    Chow {
        low: Tile,
    },
    Pass,
}

/// One entry of the replayable game log.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Step {
    Act {
        seat: usize,
        action: Action,
    },
    NextHand,
    /// The claim window closed; anyone who had not answered passes.
    CloseClaims,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    HandStart { dealer: usize },
    Discard { seat: usize, tile: Tile },
    Pung { seat: usize, tile: Tile },
    Chow { seat: usize, tile: Tile },
    Kong { seat: usize, tile: Tile },
    Win { seat: usize, from: Option<usize> },
    Draw,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameError {
    /// The action is not legal for that seat right now.
    Illegal,
    /// `next_hand` was called while a hand is still in play, or the game is over.
    NotBetweenHands,
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::Illegal => write!(f, "that action is not allowed right now"),
            GameError::NotBetweenHands => write!(f, "no hand is waiting to be started"),
        }
    }
}

impl std::error::Error for GameError {}

#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub hand: Counts,
    pub melds: Vec<Set>,
    pub discards: Vec<Tile>,
    pub flowers: Vec<u8>,
}

impl PlayerState {
    fn tile_count(&self) -> usize {
        let melded: usize = self
            .melds
            .iter()
            .map(|m| if m.kind == SetKind::Kong { 4 } else { 3 })
            .sum();
        self.hand.total() + melded + self.discards.len() + self.flowers.len()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WinSummary {
    pub winner: usize,
    /// The discarder, for a win on a discard.
    pub from: Option<usize>,
    pub self_draw: bool,
    pub winning_tile: Tile,
    pub matches: Vec<HandMatch>,
    pub value: u32,
    pub points: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Outcome {
    /// `None` for a drawn hand (wall exhausted).
    pub win: Option<WinSummary>,
    /// Score change per seat for this hand. Always sums to zero.
    pub transfers: [i32; SEATS],
    /// Everyone's concealed tiles at the end of the hand (winner's includes the winning tile).
    pub hands: Vec<Vec<Tile>>,
    pub melds: Vec<Vec<Set>>,
}

#[derive(Clone, Debug)]
struct Claim {
    discard: Tile,
    from: usize,
    /// Eligible seats in turn order after the discarder, with their legal actions.
    options: Vec<(usize, Vec<Action>)>,
    responses: Vec<(usize, Action)>,
}

#[derive(Clone, Debug)]
enum Phase {
    /// `seat` must discard (or win/kong). `drew` is true if they just drew a tile.
    Discard {
        seat: usize,
        drew: bool,
    },
    Claim(Claim),
    HandOver,
    GameOver,
}

#[derive(Clone, Debug, Serialize)]
pub struct PublicPlayer {
    pub melds: Vec<Set>,
    pub discards: Vec<Tile>,
    pub flowers: Vec<u8>,
    pub hand_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClaimView {
    pub discard: Tile,
    pub from: usize,
    /// How many players still have to answer (not which).
    pub waiting: usize,
}

/// What one seat is allowed to see.
#[derive(Clone, Debug, Serialize)]
pub struct View {
    pub seat: usize,
    /// "discard", "claim", "hand_over" or "game_over".
    pub phase: &'static str,
    pub turn: Option<usize>,
    pub dealer: usize,
    pub round_wind: usize,
    pub hand_no: u32,
    pub rounds: u32,
    pub wall_remaining: usize,
    pub scores: [i32; SEATS],
    pub players: Vec<PublicPlayer>,
    /// Own concealed tiles, sorted, not including `drawn`.
    pub hand: Vec<Tile>,
    pub drawn: Option<Tile>,
    pub legal: Vec<Action>,
    pub claim: Option<ClaimView>,
    pub last_event: Option<Event>,
    pub outcome: Option<Outcome>,
    pub threshold: u32,
    pub limit: u32,
}

#[derive(Clone, Debug)]
pub struct Game {
    config: GameConfig,
    wall: Wall,
    players: [PlayerState; SEATS],
    dealer: usize,
    /// How many times the dealer role has passed on (4 per wind round).
    rotations: u32,
    hand_no: u32,
    phase: Phase,
    scores: [i32; SEATS],
    last_drawn: Option<Tile>,
    last_event: Option<Event>,
    outcome: Option<Outcome>,
    history: Vec<Step>,
}

impl Game {
    pub fn new(config: GameConfig) -> Game {
        let mut game = Game {
            wall: Wall::new(config.flowers, config.seed),
            config,
            players: Default::default(),
            dealer: 0,
            rotations: 0,
            hand_no: 0,
            phase: Phase::HandOver,
            scores: [0; SEATS],
            last_drawn: None,
            last_event: None,
            outcome: None,
            history: Vec::new(),
        };
        game.start_hand();
        game
    }

    /// Rebuild a game from its config and logged steps.
    pub fn replay(config: GameConfig, steps: &[Step]) -> Result<Game, GameError> {
        let mut game = Game::new(config);
        for step in steps {
            match *step {
                Step::Act { seat, action } => game.apply(seat, action)?,
                Step::NextHand => game.next_hand()?,
                Step::CloseClaims => game.close_claims()?,
            }
        }
        Ok(game)
    }

    pub fn history(&self) -> &[Step] {
        &self.history
    }

    pub fn scores(&self) -> [i32; SEATS] {
        self.scores
    }

    pub fn dealer(&self) -> usize {
        self.dealer
    }

    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::GameOver)
    }

    pub fn is_between_hands(&self) -> bool {
        matches!(self.phase, Phase::HandOver)
    }

    pub fn player(&self, seat: usize) -> &PlayerState {
        &self.players[seat]
    }

    pub fn config(&self) -> &GameConfig {
        &self.config
    }

    pub fn outcome(&self) -> Option<&Outcome> {
        self.outcome.as_ref()
    }

    /// Every physical piece in the game: used to check nothing is ever lost or duplicated.
    pub fn total_pieces(&self) -> usize {
        self.wall.len()
            + self
                .players
                .iter()
                .map(PlayerState::tile_count)
                .sum::<usize>()
    }

    /// Seats that currently have something to decide.
    pub fn waiting_seats(&self) -> Vec<usize> {
        (0..SEATS)
            .filter(|&s| !self.legal_actions(s).is_empty())
            .collect()
    }

    // ---- legal actions ----

    pub fn legal_actions(&self, seat: usize) -> Vec<Action> {
        match &self.phase {
            Phase::Discard { seat: s, drew } if *s == seat => {
                let p = &self.players[seat];
                let mut out = Vec::new();
                if *drew && self.try_win(seat, None, true).is_ok() {
                    out.push(Action::Win);
                }
                if *drew {
                    for tile in Tile::all() {
                        let in_hand = p.hand.get(tile);
                        let added = in_hand >= 1
                            && p.melds
                                .iter()
                                .any(|m| m.kind == SetKind::Pung && m.tile == tile);
                        if in_hand == 4 || added {
                            out.push(Action::Kong { tile });
                        }
                    }
                }
                for tile in Tile::all().filter(|&t| p.hand.get(t) > 0) {
                    out.push(Action::Discard { tile });
                }
                out
            }
            Phase::Claim(c) => {
                if c.responses.iter().any(|(s, _)| *s == seat) {
                    return Vec::new();
                }
                c.options
                    .iter()
                    .find(|(s, _)| *s == seat)
                    .map(|(_, opts)| opts.clone())
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }

    // ---- applying actions ----

    pub fn apply(&mut self, seat: usize, action: Action) -> Result<(), GameError> {
        if seat >= SEATS || !self.legal_actions(seat).contains(&action) {
            return Err(GameError::Illegal);
        }
        self.history.push(Step::Act { seat, action });
        if matches!(self.phase, Phase::Claim(_)) {
            self.respond_to_claim(seat, action);
        } else {
            self.take_turn(seat, action);
        }
        Ok(())
    }

    /// True while a discard is waiting to be claimed or passed on.
    pub fn in_claim(&self) -> bool {
        matches!(self.phase, Phase::Claim(_))
    }

    /// End the claim window: anyone who has not answered passes, and the best claim (or the
    /// next draw) happens.
    pub fn close_claims(&mut self) -> Result<(), GameError> {
        if !self.in_claim() {
            return Err(GameError::Illegal);
        }
        self.history.push(Step::CloseClaims);
        self.resolve_claim();
        Ok(())
    }

    /// Start the next hand once a hand has finished.
    pub fn next_hand(&mut self) -> Result<(), GameError> {
        if !self.is_between_hands() {
            return Err(GameError::NotBetweenHands);
        }
        self.history.push(Step::NextHand);
        self.start_hand();
        Ok(())
    }

    fn take_turn(&mut self, seat: usize, action: Action) {
        match action {
            Action::Discard { tile } => {
                self.players[seat].hand.remove_n(tile, 1);
                self.players[seat].discards.push(tile);
                self.last_drawn = None;
                self.last_event = Some(Event::Discard { seat, tile });
                self.open_claims(seat, tile);
            }
            Action::Win => {
                let result = self
                    .try_win(seat, None, true)
                    .expect("win was checked as legal");
                self.finish_win(seat, None, true, result);
            }
            Action::Kong { tile } => {
                let p = &mut self.players[seat];
                if p.hand.get(tile) == 4 {
                    p.hand.remove_n(tile, 4);
                    p.melds.push(Set::kong(tile, false));
                } else {
                    p.hand.remove_n(tile, 1);
                    if let Some(m) = p
                        .melds
                        .iter_mut()
                        .find(|m| m.kind == SetKind::Pung && m.tile == tile)
                    {
                        m.kind = SetKind::Kong;
                    }
                }
                self.last_event = Some(Event::Kong { seat, tile });
                self.draw_replacement(seat);
            }
            // Other actions are never legal during a turn.
            _ => {}
        }
    }

    fn open_claims(&mut self, from: usize, tile: Tile) {
        let mut options = Vec::new();
        for offset in 1..SEATS {
            let seat = (from + offset) % SEATS;
            let mut opts = Vec::new();
            if self.try_win(seat, Some(tile), false).is_ok() {
                opts.push(Action::Win);
            }
            let held = self.players[seat].hand.get(tile);
            if held >= 2 {
                opts.push(Action::Pung);
            }
            if held >= 3 {
                opts.push(Action::Kong { tile });
            }
            if offset == 1 {
                opts.extend(self.chow_options(seat, tile));
            }
            if !opts.is_empty() {
                opts.push(Action::Pass);
                options.push((seat, opts));
            }
        }
        if options.is_empty() && !self.config.claim_window {
            self.next_draw((from + 1) % SEATS);
        } else {
            self.phase = Phase::Claim(Claim {
                discard: tile,
                from,
                options,
                responses: Vec::new(),
            });
        }
    }

    fn chow_options(&self, seat: usize, tile: Tile) -> Vec<Action> {
        let Some(n) = tile.number() else {
            return Vec::new();
        };
        let hand = &self.players[seat].hand;
        let mut out = Vec::new();
        for start in n.saturating_sub(2).max(1)..=n.min(7) {
            let low = Tile(tile.0 - (n - start));
            let has_others = (0..3)
                .map(|i| low.offset(i))
                .filter(|&t| t != tile)
                .all(|t| hand.get(t) > 0);
            if has_others {
                out.push(Action::Chow { low });
            }
        }
        out
    }

    fn respond_to_claim(&mut self, seat: usize, action: Action) {
        let finished = {
            let Phase::Claim(claim) = &mut self.phase else {
                return;
            };
            claim.responses.push((seat, action));
            claim.responses.len() == claim.options.len()
                && (!self.config.claim_window
                    || claim.responses.iter().any(|(_, a)| *a != Action::Pass))
        };
        if finished {
            self.resolve_claim();
        }
    }

    fn resolve_claim(&mut self) {
        let Phase::Claim(claim) = self.phase.clone() else {
            return;
        };

        let rank = |a: &Action| match a {
            Action::Win => 3,
            Action::Pung | Action::Kong { .. } => 2,
            Action::Chow { .. } => 1,
            _ => 0,
        };
        let from = claim.from;
        let tile = claim.discard;
        let best = claim
            .responses
            .iter()
            .filter(|(_, a)| rank(a) > 0)
            // Highest priority wins; ties go to whoever sits closest after the discarder.
            .max_by_key(|(s, a)| (rank(a), SEATS - (s + SEATS - from) % SEATS))
            .copied();

        let Some((claimer, action)) = best else {
            self.next_draw((from + 1) % SEATS);
            return;
        };

        match action {
            Action::Win => {
                let result = self
                    .try_win(claimer, Some(tile), false)
                    .expect("win was checked as legal");
                self.finish_win(claimer, Some(from), false, result);
            }
            Action::Pung => {
                self.take_discard(from);
                let p = &mut self.players[claimer];
                p.hand.remove_n(tile, 2);
                p.melds.push(Set::pung(tile, true));
                self.last_drawn = None;
                self.last_event = Some(Event::Pung {
                    seat: claimer,
                    tile,
                });
                self.phase = Phase::Discard {
                    seat: claimer,
                    drew: false,
                };
            }
            Action::Kong { .. } => {
                self.take_discard(from);
                let p = &mut self.players[claimer];
                p.hand.remove_n(tile, 3);
                p.melds.push(Set::kong(tile, true));
                self.last_event = Some(Event::Kong {
                    seat: claimer,
                    tile,
                });
                self.draw_replacement(claimer);
            }
            Action::Chow { low } => {
                self.take_discard(from);
                let p = &mut self.players[claimer];
                for t in (0..3).map(|i| low.offset(i)).filter(|&t| t != tile) {
                    p.hand.remove_n(t, 1);
                }
                p.melds.push(Set::chow(low, true));
                self.last_drawn = None;
                self.last_event = Some(Event::Chow {
                    seat: claimer,
                    tile,
                });
                self.phase = Phase::Discard {
                    seat: claimer,
                    drew: false,
                };
            }
            _ => {}
        }
    }

    /// Remove the claimed tile from the discarder's pile.
    fn take_discard(&mut self, from: usize) {
        self.players[from].discards.pop();
    }

    // ---- drawing ----

    fn draw_into_hand(&mut self, seat: usize, mut from_back: bool) -> Option<Tile> {
        loop {
            let piece = if from_back {
                self.wall.draw_replacement()
            } else {
                self.wall.draw()
            }?;
            match piece {
                Piece::Tile(t) => {
                    self.players[seat].hand.add(t);
                    return Some(t);
                }
                Piece::Flower(f) => {
                    self.players[seat].flowers.push(f);
                    from_back = true;
                }
            }
        }
    }

    fn next_draw(&mut self, seat: usize) {
        if self.wall.len() <= DEAD_WALL {
            return self.end_in_draw();
        }
        match self.draw_into_hand(seat, false) {
            Some(t) => {
                self.last_drawn = Some(t);
                self.phase = Phase::Discard { seat, drew: true };
            }
            None => self.end_in_draw(),
        }
    }

    fn draw_replacement(&mut self, seat: usize) {
        match self.draw_into_hand(seat, true) {
            Some(t) => {
                self.last_drawn = Some(t);
                self.phase = Phase::Discard { seat, drew: true };
            }
            None => self.end_in_draw(),
        }
    }

    // ---- winning and ending hands ----

    fn seat_wind(&self, seat: usize) -> Tile {
        Tile(27 + ((seat + SEATS - self.dealer) % SEATS) as u8)
    }

    fn round_wind_index(&self) -> usize {
        (self.rotations as usize / SEATS) % 4
    }

    fn context(&self, seat: usize, extra: Option<Tile>, self_draw: bool) -> WinContext {
        let p = &self.players[seat];
        let mut concealed = p.hand.clone();
        if let Some(t) = extra {
            concealed.add(t);
        }
        WinContext {
            concealed,
            melds: p.melds.clone(),
            winning_tile: extra.or(self.last_drawn).unwrap_or(Tile(0)),
            self_draw,
            seat_wind: self.seat_wind(seat),
            round_wind: Tile(27 + self.round_wind_index() as u8),
        }
    }

    fn try_win(
        &self,
        seat: usize,
        extra: Option<Tile>,
        self_draw: bool,
    ) -> Result<ScoreResult, ScoreError> {
        score(&self.config.rules, &self.context(seat, extra, self_draw))
    }

    fn payments(&self, winner: usize, from: Option<usize>, points: u32) -> [i32; SEATS] {
        let mut t = [0i32; SEATS];
        let owed = |loser: usize| {
            let double =
                self.config.dealer_double && (winner == self.dealer || loser == self.dealer);
            points as i32 * if double { 2 } else { 1 }
        };
        let losers = (0..SEATS).filter(|&s| s != winner);
        match from {
            None => {
                for l in losers {
                    t[l] -= owed(l);
                    t[winner] += owed(l);
                }
            }
            Some(discarder) => {
                let total: i32 = losers.map(owed).sum();
                t[discarder] -= total;
                t[winner] += total;
            }
        }
        t
    }

    fn final_hands(&self, winner_extra: Option<(usize, Tile)>) -> (Vec<Vec<Tile>>, Vec<Vec<Set>>) {
        let hands = (0..SEATS)
            .map(|s| {
                let mut counts = self.players[s].hand.clone();
                if let Some((w, t)) = winner_extra {
                    if w == s {
                        counts.add(t);
                    }
                }
                Tile::all()
                    .flat_map(|t| std::iter::repeat_n(t, counts.get(t) as usize))
                    .collect()
            })
            .collect();
        let melds = self.players.iter().map(|p| p.melds.clone()).collect();
        (hands, melds)
    }

    fn finish_win(&mut self, winner: usize, from: Option<usize>, self_draw: bool, r: ScoreResult) {
        let winning_tile = match (from, self.last_drawn) {
            (Some(_), _) => self.claim_tile().unwrap_or(Tile(0)),
            (None, t) => t.unwrap_or(Tile(0)),
        };
        let transfers = self.payments(winner, from, r.points);
        let extra = from.map(|_| (winner, winning_tile));
        let (hands, melds) = self.final_hands(extra);
        let outcome = Outcome {
            win: Some(WinSummary {
                winner,
                from,
                self_draw,
                winning_tile,
                matches: r.matches,
                value: r.value,
                points: r.points,
            }),
            transfers,
            hands,
            melds,
        };
        self.last_event = Some(Event::Win { seat: winner, from });
        let dealer_continues = winner == self.dealer;
        self.finish_hand(outcome, dealer_continues);
    }

    fn claim_tile(&self) -> Option<Tile> {
        match &self.phase {
            Phase::Claim(c) => Some(c.discard),
            _ => None,
        }
    }

    fn end_in_draw(&mut self) {
        let (hands, melds) = self.final_hands(None);
        let outcome = Outcome {
            win: None,
            transfers: [0; SEATS],
            hands,
            melds,
        };
        self.last_event = Some(Event::Draw);
        self.finish_hand(outcome, true);
    }

    fn finish_hand(&mut self, outcome: Outcome, dealer_continues: bool) {
        for (score, change) in self.scores.iter_mut().zip(outcome.transfers) {
            *score += change;
        }
        self.outcome = Some(outcome);
        if !dealer_continues {
            self.dealer = (self.dealer + 1) % SEATS;
            self.rotations += 1;
        }
        self.phase = if self.rotations >= self.config.rounds * SEATS as u32 {
            Phase::GameOver
        } else {
            Phase::HandOver
        };
    }

    fn start_hand(&mut self) {
        self.hand_no += 1;
        let seed = self
            .config
            .seed
            .wrapping_add((self.hand_no as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        self.wall = Wall::new(self.config.flowers, seed);
        self.players = Default::default();
        self.outcome = None;
        for _ in 0..13 {
            for i in 0..SEATS {
                let seat = (self.dealer + i) % SEATS;
                self.draw_into_hand(seat, false);
            }
        }
        let dealer = self.dealer;
        let first = self
            .draw_into_hand(dealer, false)
            .expect("a fresh wall has tiles");
        self.last_drawn = Some(first);
        self.last_event = Some(Event::HandStart { dealer });
        self.phase = Phase::Discard {
            seat: dealer,
            drew: true,
        };
    }

    // ---- what a player sees ----

    pub fn view(&self, seat: usize) -> View {
        let (phase, turn) = match &self.phase {
            Phase::Discard { seat, .. } => ("discard", Some(*seat)),
            Phase::Claim(_) => ("claim", None),
            Phase::HandOver => ("hand_over", None),
            Phase::GameOver => ("game_over", None),
        };

        let mut hand_counts = self.players[seat].hand.clone();
        let drawn = match self.phase {
            Phase::Discard {
                seat: s,
                drew: true,
            } if s == seat => self.last_drawn,
            _ => None,
        };
        if let Some(t) = drawn {
            hand_counts.remove_n(t, 1);
        }
        let hand = Tile::all()
            .flat_map(|t| std::iter::repeat_n(t, hand_counts.get(t) as usize))
            .collect();

        let claim = match &self.phase {
            Phase::Claim(c) => Some(ClaimView {
                discard: c.discard,
                from: c.from,
                // With a claim window the count would reveal who holds what, so hide it.
                waiting: if self.config.claim_window {
                    0
                } else {
                    c.options.len() - c.responses.len()
                },
            }),
            _ => None,
        };

        View {
            seat,
            phase,
            turn,
            dealer: self.dealer,
            round_wind: self.round_wind_index(),
            hand_no: self.hand_no,
            rounds: self.config.rounds,
            wall_remaining: self.wall.len().saturating_sub(DEAD_WALL),
            scores: self.scores,
            players: self
                .players
                .iter()
                .map(|p| PublicPlayer {
                    melds: p.melds.clone(),
                    discards: p.discards.clone(),
                    flowers: p.flowers.clone(),
                    hand_count: p.hand.total(),
                })
                .collect(),
            hand,
            drawn,
            legal: self.legal_actions(seat),
            claim,
            last_event: self.last_event,
            outcome: self.outcome.clone(),
            threshold: self.config.rules.threshold,
            limit: self.config.rules.limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;
    use crate::rules::RoomOptions;

    fn game_with_threshold(offset: i32) -> Game {
        let options = RoomOptions {
            threshold_offset: offset,
            ..Default::default()
        };
        let rules = Ruleset::resolve(
            &builtin::base("hk_fan").unwrap(),
            &builtin::addons(),
            &options,
        )
        .unwrap();
        Game::new(GameConfig::new(rules, 1))
    }

    fn set_hand(game: &mut Game, seat: usize, notation: &str) {
        game.players[seat].hand = Counts::parse(notation).unwrap();
    }

    #[test]
    fn deal_gives_dealer_fourteen_and_others_thirteen() {
        let game = game_with_threshold(0);
        assert_eq!(game.players[0].hand.total(), 14);
        for seat in 1..4 {
            assert_eq!(game.players[seat].hand.total(), 13);
        }
        assert_eq!(game.total_pieces(), 136);
        assert_eq!(game.waiting_seats(), vec![0]);
    }

    #[test]
    fn only_the_player_to_act_can_act() {
        let mut game = game_with_threshold(0);
        let tile = game.legal_actions(0).into_iter().find_map(|a| match a {
            Action::Discard { tile } => Some(tile),
            _ => None,
        });
        let action = Action::Discard {
            tile: tile.expect("dealer can discard"),
        };
        assert_eq!(game.apply(1, action), Err(GameError::Illegal));
        assert_eq!(game.next_hand(), Err(GameError::NotBetweenHands));
        assert_eq!(game.apply(0, Action::Pass), Err(GameError::Illegal));
        assert!(game.apply(0, action).is_ok());
    }

    #[test]
    fn chow_options_cover_each_position() {
        let mut game = game_with_threshold(0);
        set_hand(&mut game, 1, "3467m");
        let five_m = Tile::new(4).unwrap();
        let lows: Vec<u8> = game
            .chow_options(1, five_m)
            .into_iter()
            .filter_map(|a| match a {
                Action::Chow { low } => Some(low.0),
                _ => None,
            })
            .collect();
        assert_eq!(lows, vec![2, 3, 4]);
        assert!(game.chow_options(1, Tile::new(27).unwrap()).is_empty());
    }

    #[test]
    fn pung_outranks_chow() {
        let mut game = game_with_threshold(0);
        let five_p = Tile::new(13).unwrap();
        game.players[0].hand.add(five_p);
        set_hand(&mut game, 1, "46p");
        set_hand(&mut game, 2, "55p1m");
        game.apply(0, Action::Discard { tile: five_p }).unwrap();

        assert!(game
            .legal_actions(1)
            .iter()
            .any(|a| matches!(a, Action::Chow { .. })));
        assert!(game.legal_actions(2).contains(&Action::Pung));

        game.apply(
            1,
            Action::Chow {
                low: Tile::new(12).unwrap(),
            },
        )
        .unwrap();
        game.apply(2, Action::Pung).unwrap();

        assert_eq!(game.players[2].melds, vec![Set::pung(five_p, true)]);
        assert_eq!(game.players[1].melds, vec![]);
        assert!(game.players[0].discards.is_empty());
        assert_eq!(game.waiting_seats(), vec![2]);
    }

    fn windowed_game() -> Game {
        let mut config = game_with_threshold(0).config.clone();
        config.claim_window = true;
        Game::new(config)
    }

    #[test]
    fn claim_window_waits_even_when_nobody_can_claim() {
        let mut game = windowed_game();
        let nine_s = Tile::new(26).unwrap();
        game.players[0].hand.add(nine_s);
        for s in 1..SEATS {
            set_hand(&mut game, s, "1112223334445z");
        }
        game.apply(0, Action::Discard { tile: nine_s }).unwrap();
        assert!(game.in_claim());
        assert!(game.waiting_seats().is_empty());
        assert_eq!(game.view(2).claim.unwrap().waiting, 0);

        game.close_claims().unwrap();
        assert!(!game.in_claim());
        assert_eq!(game.view(1).turn, Some(1));
        assert!(game.close_claims().is_err());
    }

    #[test]
    fn claim_window_holds_after_passes_but_not_after_a_pung() {
        let five_p = Tile::new(13).unwrap();

        let mut game = windowed_game();
        game.players[0].hand.add(five_p);
        set_hand(&mut game, 2, "55p");
        game.apply(0, Action::Discard { tile: five_p }).unwrap();
        game.apply(2, Action::Pass).unwrap();
        // Everyone eligible passed, but the window is still open.
        assert!(game.in_claim());
        game.close_claims().unwrap();
        assert_eq!(game.view(1).turn, Some(1));

        let mut game = windowed_game();
        game.players[0].hand.add(five_p);
        set_hand(&mut game, 2, "55p");
        game.apply(0, Action::Discard { tile: five_p }).unwrap();
        game.apply(2, Action::Pung).unwrap();
        assert!(!game.in_claim(), "a claim does not wait for the window");
        assert_eq!(game.view(2).turn, Some(2));
    }

    #[test]
    fn everyone_passing_moves_on_to_the_next_draw() {
        let mut game = game_with_threshold(0);
        let five_p = Tile::new(13).unwrap();
        game.players[0].hand.add(five_p);
        set_hand(&mut game, 2, "55p");
        game.apply(0, Action::Discard { tile: five_p }).unwrap();
        game.apply(2, Action::Pass).unwrap();
        assert_eq!(game.players[0].discards, vec![five_p]);
        assert_eq!(game.waiting_seats(), vec![1]);
    }

    #[test]
    fn win_outranks_pung_and_discarder_pays() {
        let mut game = game_with_threshold(-3);
        let three_p = Tile::new(11).unwrap();
        game.players[0].hand.add(three_p);
        set_hand(&mut game, 2, "33p");
        set_hand(&mut game, 3, "123m456m789m12p55p");
        game.apply(0, Action::Discard { tile: three_p }).unwrap();

        assert!(game.legal_actions(3).contains(&Action::Win));
        game.apply(2, Action::Pung).unwrap();
        game.apply(3, Action::Win).unwrap();

        let outcome = game.outcome().unwrap();
        let win = outcome.win.as_ref().unwrap();
        assert_eq!((win.winner, win.from, win.self_draw), (3, Some(0), false));
        assert_eq!(win.points, 2);
        // Dealer (seat 0) discarded: pays 4 + 2 + 2 because payments to the dealer double.
        assert_eq!(outcome.transfers, [-8, 0, 0, 8]);
        assert_eq!(game.scores(), [-8, 0, 0, 8]);
        // A non-dealer won, so the deal passes on.
        assert_eq!(game.dealer(), 1);
    }

    #[test]
    fn below_threshold_hands_cannot_win() {
        let mut game = game_with_threshold(0); // default 3 fan
        let three_p = Tile::new(11).unwrap();
        game.players[0].hand.add(three_p);
        set_hand(&mut game, 3, "123m456m789m12p55p");
        game.apply(0, Action::Discard { tile: three_p }).unwrap();
        // Common hand is only 1 fan, so seat 3 gets no claim and play moves on.
        assert!(game.legal_actions(3).is_empty());
    }

    #[test]
    fn payments_are_zero_sum() {
        let game = game_with_threshold(0);
        assert_eq!(game.dealer, 0);
        // Non-dealer self-draw: dealer pays double.
        assert_eq!(game.payments(1, None, 2), [-4, 8, -2, -2]);
        // Dealer self-draw: everyone pays double.
        assert_eq!(game.payments(0, None, 2), [12, -4, -4, -4]);
        // Non-dealer wins off another non-dealer's discard.
        assert_eq!(game.payments(1, Some(2), 2), [0, 8, -8, 0]);
        for t in [
            game.payments(2, None, 5),
            game.payments(0, Some(3), 7),
            game.payments(3, Some(0), 1),
        ] {
            assert_eq!(t.iter().sum::<i32>(), 0);
        }
    }
}
