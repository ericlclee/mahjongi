//! Scoring a winning hand against a resolved ruleset.

use crate::decompose::{win_shapes, WinShape};
use crate::pattern::WinContext;
use crate::rules::Ruleset;
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HandMatch {
    pub id: String,
    pub name: String,
    /// How many times the pattern matched (e.g. dragon pungs).
    pub count: u32,
    /// Value this match contributes.
    pub value: u32,
    pub qualifying: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScoreResult {
    pub shape: WinShape,
    pub matches: Vec<HandMatch>,
    /// Qualifying value (checked against the threshold), clamped to the limit.
    pub qualifying: u32,
    /// Total value used for payout, clamped to the limit.
    pub value: u32,
    /// Base points from the payout table.
    pub points: u32,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScoreError {
    NotAWin,
    BelowThreshold { have: u32, need: u32 },
}

impl fmt::Display for ScoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScoreError::NotAWin => write!(f, "not a winning hand"),
            ScoreError::BelowThreshold { have, need } => {
                write!(f, "hand is worth {have} but {need} is needed to win")
            }
        }
    }
}

impl std::error::Error for ScoreError {}

/// Score a hand, choosing the highest-scoring reading, and check the win threshold.
pub fn score(rules: &Ruleset, ctx: &WinContext) -> Result<ScoreResult, ScoreError> {
    let shapes = win_shapes(&ctx.concealed, &ctx.melds);
    if shapes.is_empty() {
        return Err(ScoreError::NotAWin);
    }

    let mut best: Option<ScoreResult> = None;
    for shape in shapes {
        let result = evaluate(rules, ctx, shape);
        let better = best
            .as_ref()
            .is_none_or(|b| (result.value, result.qualifying) > (b.value, b.qualifying));
        if better {
            best = Some(result);
        }
    }
    let best = best.expect("at least one shape");

    if best.qualifying < rules.threshold {
        return Err(ScoreError::BelowThreshold {
            have: best.qualifying,
            need: rules.threshold,
        });
    }
    Ok(best)
}

fn evaluate(rules: &Ruleset, ctx: &WinContext, shape: WinShape) -> ScoreResult {
    let open = ctx.is_open();

    let mut matches = Vec::new();
    for hand in &rules.hands {
        let count = hand.pattern.eval(ctx, &shape);
        if count == 0 {
            continue;
        }
        let each = if open { hand.open_value } else { hand.value };
        let value = each * if hand.per_match { count } else { 1 };
        if value == 0 {
            continue;
        }
        matches.push(HandMatch {
            id: hand.id.clone(),
            name: hand.name.clone(),
            count,
            value,
            qualifying: hand.qualifying,
        });
    }

    let mut excluded: BTreeSet<&str> = BTreeSet::new();
    for m in &matches {
        if let Some(hand) = rules.hands.iter().find(|h| h.id == m.id) {
            excluded.extend(hand.excludes.iter().map(String::as_str));
        }
    }
    matches.retain(|m| !excluded.contains(m.id.as_str()));

    let total: u32 = matches.iter().map(|m| m.value).sum();
    let qualifying: u32 = matches
        .iter()
        .filter(|m| m.qualifying)
        .map(|m| m.value)
        .sum();
    let value = total.min(rules.limit);

    ScoreResult {
        shape,
        matches,
        qualifying: qualifying.min(rules.limit),
        value,
        points: rules.payout_points(value),
    }
}
