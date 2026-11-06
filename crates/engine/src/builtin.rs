//! Bases and add-ons shipped with the engine (embedded from `rulesets/`).

use crate::rules::{AddOn, Base};

const HK_FAN: &str = include_str!("../../../rulesets/bases/hk_fan.json");
const HK_CLASSIC: &str = include_str!("../../../rulesets/bases/hk_classic.json");
const SG_TAI: &str = include_str!("../../../rulesets/bases/sg_tai.json");
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
