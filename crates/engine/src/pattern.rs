//! Data-driven hand patterns. A pattern is a small expression built from
//! primitives, loaded from JSON, and evaluated against a winning shape.

use crate::decompose::{Decomposition, Set, SetKind, WinShape};
use crate::hand::Counts;
use crate::tile::{Suit, Tile};
use serde::{Deserialize, Serialize};

/// Everything a pattern may look at when judging a winning hand.
#[derive(Clone, Debug)]
pub struct WinContext {
    /// Tiles in hand, including the winning tile.
    pub concealed: Counts,
    /// Declared melds.
    pub melds: Vec<Set>,
    pub winning_tile: Tile,
    /// True if won on a self-drawn tile.
    pub self_draw: bool,
    /// The winner's seat wind (a wind tile).
    pub seat_wind: Tile,
    /// The prevailing round wind (a wind tile).
    pub round_wind: Tile,
}

impl WinContext {
    /// True if any set was formed by calling a discard.
    pub fn is_open(&self) -> bool {
        self.melds.iter().any(|m| m.open)
    }

    /// Every distinct tile kind in the hand, concealed and melded.
    pub fn tiles_present(&self) -> Vec<Tile> {
        let mut present = [false; Tile::KINDS];
        for t in Tile::all().filter(|&t| self.concealed.get(t) > 0) {
            present[t.index()] = true;
        }
        for m in &self.melds {
            let span = if m.kind == SetKind::Chow { 3 } else { 1 };
            for i in 0..span {
                present[m.tile.offset(i).index()] = true;
            }
        }
        Tile::all().filter(|t| present[t.index()]).collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileFilter {
    Dragon,
    Wind,
    Honor,
    Terminal,
    TerminalOrHonor,
    Simple,
    SeatWind,
    RoundWind,
    /// Dragons, plus the seat and round winds.
    ValueTile,
}

impl TileFilter {
    pub fn matches(self, tile: Tile, ctx: &WinContext) -> bool {
        match self {
            TileFilter::Dragon => tile.suit() == Suit::Dragon,
            TileFilter::Wind => tile.suit() == Suit::Wind,
            TileFilter::Honor => tile.is_honor(),
            TileFilter::Terminal => tile.is_terminal(),
            TileFilter::TerminalOrHonor => tile.is_terminal_or_honor(),
            TileFilter::Simple => tile.is_simple(),
            TileFilter::SeatWind => tile == ctx.seat_wind,
            TileFilter::RoundWind => tile == ctx.round_wind,
            TileFilter::ValueTile => {
                tile.suit() == Suit::Dragon || tile == ctx.seat_wind || tile == ctx.round_wind
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Pattern {
    SelfDraw,
    /// No set was formed by calling a discard.
    Concealed,
    /// Standard shape where every set is a chow.
    AllChows,
    /// Standard shape where every set is a pung or kong.
    AllPungs,
    SevenPairs,
    ThirteenOrphans,
    /// Every tile is from one numbered suit. `honors: true` requires at least
    /// one honor tile as well (a "mixed" flush); `false` allows none (a "pure" flush).
    OneSuit {
        honors: bool,
    },
    /// Every tile in the hand matches the filter.
    AllTiles {
        filter: TileFilter,
    },
    /// At least `min` pungs/kongs of tiles matching the filter. Evaluates to the
    /// number of such pungs (useful with `per_match`).
    PungCount {
        filter: TileFilter,
        min: u32,
    },
    PairIs {
        filter: TileFilter,
    },
    PairIsNot {
        filter: TileFilter,
    },
    And {
        all: Vec<Pattern>,
    },
    Or {
        any: Vec<Pattern>,
    },
    Not {
        inner: Box<Pattern>,
    },
}

fn standard(shape: &WinShape) -> Option<&Decomposition> {
    match shape {
        WinShape::Standard(d) => Some(d),
        _ => None,
    }
}

impl Pattern {
    /// 0 means no match. Most patterns give 1 on a match; `PungCount` gives the count.
    pub fn eval(&self, ctx: &WinContext, shape: &WinShape) -> u32 {
        match self {
            Pattern::SelfDraw => ctx.self_draw as u32,
            Pattern::Concealed => !ctx.is_open() as u32,
            Pattern::AllChows => {
                standard(shape).map_or(0, |d| d.sets.iter().all(|s| s.kind == SetKind::Chow) as u32)
            }
            Pattern::AllPungs => {
                standard(shape).map_or(0, |d| d.sets.iter().all(|s| s.kind != SetKind::Chow) as u32)
            }
            Pattern::SevenPairs => matches!(shape, WinShape::SevenPairs) as u32,
            Pattern::ThirteenOrphans => matches!(shape, WinShape::ThirteenOrphans) as u32,
            Pattern::OneSuit { honors } => one_suit(ctx, *honors) as u32,
            Pattern::AllTiles { filter } => {
                ctx.tiles_present().iter().all(|&t| filter.matches(t, ctx)) as u32
            }
            Pattern::PungCount { filter, min } => {
                let n = standard(shape).map_or(0, |d| {
                    d.sets
                        .iter()
                        .filter(|s| s.kind != SetKind::Chow && filter.matches(s.tile, ctx))
                        .count() as u32
                });
                if n >= *min {
                    n
                } else {
                    0
                }
            }
            Pattern::PairIs { filter } => {
                standard(shape).map_or(0, |d| filter.matches(d.pair, ctx) as u32)
            }
            Pattern::PairIsNot { filter } => {
                standard(shape).map_or(0, |d| !filter.matches(d.pair, ctx) as u32)
            }
            Pattern::And { all } => all.iter().all(|p| p.eval(ctx, shape) > 0) as u32,
            Pattern::Or { any } => any.iter().any(|p| p.eval(ctx, shape) > 0) as u32,
            Pattern::Not { inner } => (inner.eval(ctx, shape) == 0) as u32,
        }
    }
}

fn one_suit(ctx: &WinContext, want_honors: bool) -> bool {
    let mut suits = [false; 3];
    let mut honors = false;
    for t in ctx.tiles_present() {
        match t.suit() {
            Suit::Man => suits[0] = true,
            Suit::Pin => suits[1] = true,
            Suit::Sou => suits[2] = true,
            _ => honors = true,
        }
    }
    suits.iter().filter(|&&s| s).count() == 1 && honors == want_honors
}
