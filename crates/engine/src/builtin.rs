//! Bases and add-ons shipped with the engine (embedded from `rulesets/`).

use crate::decompose::Set;
use crate::hand::Counts;
use crate::rules::{AddOn, Base};
use crate::tile::Tile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const HK_FAN: &str = include_str!("../../../rulesets/bases/hk_fan.json");
const HK_CLASSIC: &str = include_str!("../../../rulesets/bases/hk_classic.json");
const SG_TAI: &str = include_str!("../../../rulesets/bases/sg_tai.json");
const EXAMPLES: &str = include_str!("../../../rulesets/examples.json");
const ADDONS: &str = include_str!("../../../rulesets/addons.json");

pub fn bases() -> Vec<Base> {
    [HK_FAN, HK_CLASSIC, SG_TAI]
        .iter()
        .map(|text| serde_json::from_str(text).expect("built-in base is valid JSON"))
        .collect()
}

pub fn base(id: &str) -> Option<Base> {
    bases().into_iter().find(|b| b.id == id)
}

pub fn addons() -> Vec<AddOn> {
    serde_json::from_str(ADDONS).expect("addons.json is valid")
}

/// An example of a hand, for showing people what it looks like.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Example {
    /// Space-separated groups in tile notation, e.g. "123m 456p 777z 55s". A group of four
    /// identical tiles is a kong.
    pub tiles: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Example {
    /// The tiles of each group, in order.
    pub fn groups(&self) -> Vec<Vec<Tile>> {
        self.tiles
            .split_whitespace()
            .map(|g| {
                let counts = Counts::parse(g).expect("example tiles are valid notation");
                (0..34)
                    .filter_map(Tile::new)
                    .flat_map(|t| std::iter::repeat_n(t, counts.get(t) as usize))
                    .collect()
            })
            .collect()
    }

    /// The hand as concealed tiles plus kongs, ready to score.
    pub fn split(&self) -> (Counts, Vec<Set>) {
        let mut concealed = Counts::new();
        let mut kongs = Vec::new();
        for group in self.groups() {
            if group.len() == 4 && group.iter().all(|&t| t == group[0]) {
                kongs.push(Set::kong(group[0], false));
            } else {
                group.iter().for_each(|&t| concealed.add(t));
            }
        }
        (concealed, kongs)
    }
}

/// Example hands by hand id, shared by every base and add-on.
pub fn examples() -> HashMap<String, Example> {
    serde_json::from_str(EXAMPLES).expect("examples.json is valid")
}
