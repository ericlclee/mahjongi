//! Bases, add-ons and resolving them into a playable ruleset.

use crate::pattern::Pattern;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

fn yes() -> bool {
    true
}

/// A hand: a matcher plus a value for each scoring model that supports it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct HandDef {
    pub id: String,
    pub name: String,
    pub pattern: Pattern,
    /// Value per scoring model, e.g. `{"fan": 3}`. A model with no entry
    /// does not support this hand.
    pub values: BTreeMap<String, u32>,
    /// Value when the hand is open; defaults to `values`.
    #[serde(default)]
    pub open_values: BTreeMap<String, u32>,
    /// Counts toward the win threshold. `false` means payout only (like dora).
    #[serde(default = "yes")]
    pub qualifying: bool,
    /// Multiply the value by the pattern's match count (e.g. one fan per dragon pung).
    #[serde(default)]
    pub per_match: bool,
    /// Hands dropped from the score when this one matches.
    #[serde(default)]
    pub excludes: Vec<String>,
}

/// A complete default rule set for one scoring model.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Base {
    pub id: String,
    pub name: String,
    /// Scoring model key used in `HandDef::values`, e.g. "fan".
    pub model: String,
    /// Capabilities add-ons may require, e.g. "fan". The model name is always provided.
    #[serde(default)]
    pub provides: Vec<String>,
    /// Minimum qualifying value needed to win.
    pub threshold: u32,
    /// Maximum value any hand total is clamped to.
    pub limit: u32,
    /// Base points by value; values past the end use the last entry.
    pub payout: Vec<u32>,
    pub hands: Vec<HandDef>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    AddHand {
        hand: HandDef,
    },
    RemoveHand {
        id: String,
    },
    SetValue {
        hand: String,
        model: String,
        value: u32,
    },
    SetOpenValue {
        hand: String,
        model: String,
        value: u32,
    },
    ShiftThreshold {
        by: i32,
    },
}

/// An optional layer applied on top of a base.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct AddOn {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Capabilities the base must provide.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Add-ons that cannot be enabled together with this one.
    #[serde(default)]
    pub conflicts: Vec<String>,
    /// Lower runs first.
    #[serde(default)]
    pub order: i32,
    pub ops: Vec<Op>,
}

impl AddOn {
    /// Whether a base can use this add-on (for hiding unsupported ones in the UI).
    pub fn supported_by(&self, base: &Base) -> bool {
        self.requires
            .iter()
            .all(|r| *r == base.model || base.provides.contains(r))
    }
}

/// What a room picks on top of a base.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct RoomOptions {
    pub addons: Vec<String>,
    pub disabled_hands: Vec<String>,
    /// Raise (positive) or lower (negative) the win threshold.
    pub threshold_offset: i32,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedHand {
    pub id: String,
    pub name: String,
    pub pattern: Pattern,
    pub value: u32,
    pub open_value: u32,
    pub qualifying: bool,
    pub per_match: bool,
    pub excludes: Vec<String>,
}

/// A base with add-ons and room options applied, ready for scoring.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ruleset {
    pub base_id: String,
    pub model: String,
    pub addons: Vec<String>,
    pub threshold: u32,
    pub limit: u32,
    pub payout: Vec<u32>,
    pub hands: Vec<ResolvedHand>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RuleError {
    UnknownAddOn(String),
    UnknownHand(String),
    DuplicateHand(String),
    Conflict(String, String),
    MissingCapability { addon: String, needs: String },
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleError::UnknownAddOn(id) => write!(f, "unknown add-on '{id}'"),
            RuleError::UnknownHand(id) => write!(f, "unknown hand '{id}'"),
            RuleError::DuplicateHand(id) => write!(f, "hand '{id}' already exists"),
            RuleError::Conflict(a, b) => write!(f, "add-ons '{a}' and '{b}' conflict"),
            RuleError::MissingCapability { addon, needs } => {
                write!(f, "add-on '{addon}' needs '{needs}', which the base lacks")
            }
        }
    }
}

impl std::error::Error for RuleError {}

impl Ruleset {
    pub fn resolve(
        base: &Base,
        catalog: &[AddOn],
        options: &RoomOptions,
    ) -> Result<Ruleset, RuleError> {
        // Look up enabled add-ons (each once), keeping catalog position for stable ordering.
        let mut enabled: Vec<(usize, &AddOn)> = Vec::new();
        for id in &options.addons {
            let (pos, addon) = catalog
                .iter()
                .enumerate()
                .find(|(_, a)| a.id == *id)
                .ok_or_else(|| RuleError::UnknownAddOn(id.clone()))?;
            if !enabled.iter().any(|(p, _)| *p == pos) {
                enabled.push((pos, addon));
            }
        }

        for (_, addon) in &enabled {
            if let Some(missing) = addon
                .requires
                .iter()
                .find(|r| **r != base.model && !base.provides.contains(r))
            {
                return Err(RuleError::MissingCapability {
                    addon: addon.id.clone(),
                    needs: missing.clone(),
                });
            }
        }

        for (i, (_, a)) in enabled.iter().enumerate() {
            for (_, b) in &enabled[i + 1..] {
                if a.conflicts.contains(&b.id) || b.conflicts.contains(&a.id) {
                    return Err(RuleError::Conflict(a.id.clone(), b.id.clone()));
                }
            }
        }

        enabled.sort_by_key(|(pos, a)| (a.order, *pos));

        let mut hands = base.hands.clone();
        let mut shift: i64 = 0;
        for (_, addon) in &enabled {
            for op in &addon.ops {
                match op {
                    Op::AddHand { hand } => {
                        if hands.iter().any(|h| h.id == hand.id) {
                            return Err(RuleError::DuplicateHand(hand.id.clone()));
                        }
                        hands.push(hand.clone());
                    }
                    Op::RemoveHand { id } => {
                        let at = hands
                            .iter()
                            .position(|h| h.id == *id)
                            .ok_or_else(|| RuleError::UnknownHand(id.clone()))?;
                        hands.remove(at);
                    }
                    Op::SetValue { hand, model, value } => {
                        find_hand(&mut hands, hand)?
                            .values
                            .insert(model.clone(), *value);
                    }
                    Op::SetOpenValue { hand, model, value } => {
                        find_hand(&mut hands, hand)?
                            .open_values
                            .insert(model.clone(), *value);
                    }
                    Op::ShiftThreshold { by } => shift += *by as i64,
                }
            }
        }

        for id in &options.disabled_hands {
            let at = hands
                .iter()
                .position(|h| h.id == *id)
                .ok_or_else(|| RuleError::UnknownHand(id.clone()))?;
            hands.remove(at);
        }

        let resolved = hands
            .into_iter()
            .filter_map(|h| {
                let value = *h.values.get(&base.model)?;
                let open_value = h.open_values.get(&base.model).copied().unwrap_or(value);
                Some(ResolvedHand {
                    id: h.id,
                    name: h.name,
                    pattern: h.pattern,
                    value,
                    open_value,
                    qualifying: h.qualifying,
                    per_match: h.per_match,
                    excludes: h.excludes,
                })
            })
            .collect();

        let threshold =
            (base.threshold as i64 + shift + options.threshold_offset as i64).max(0) as u32;

        Ok(Ruleset {
            base_id: base.id.clone(),
            model: base.model.clone(),
            addons: enabled.iter().map(|(_, a)| a.id.clone()).collect(),
            threshold,
            limit: base.limit,
            payout: base.payout.clone(),
            hands: resolved,
        })
    }

    /// Base points for a total value, from the payout table.
    pub fn payout_points(&self, value: u32) -> u32 {
        let i = (value as usize).min(self.payout.len().saturating_sub(1));
        self.payout.get(i).copied().unwrap_or(0)
    }
}

fn find_hand<'a>(hands: &'a mut [HandDef], id: &str) -> Result<&'a mut HandDef, RuleError> {
    hands
        .iter_mut()
        .find(|h| h.id == id)
        .ok_or_else(|| RuleError::UnknownHand(id.to_string()))
}
