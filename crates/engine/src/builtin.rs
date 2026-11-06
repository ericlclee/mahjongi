//! Bases and add-ons shipped with the engine (embedded from `rulesets/`).

use crate::rules::{AddOn, Base};

const HK_FAN: &str = include_str!("../../../rulesets/bases/hk_fan.json");
const ADDONS: &str = include_str!("../../../rulesets/addons.json");

pub fn bases() -> Vec<Base> {
    vec![serde_json::from_str(HK_FAN).expect("hk_fan.json is valid")]
}

pub fn base(id: &str) -> Option<Base> {
    bases().into_iter().find(|b| b.id == id)
}

pub fn addons() -> Vec<AddOn> {
    serde_json::from_str(ADDONS).expect("addons.json is valid")
}
