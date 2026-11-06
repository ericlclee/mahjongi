use mahjongi_engine::builtin;
use mahjongi_engine::*;

fn ctx(concealed: &str, melds: Vec<Set>, self_draw: bool) -> WinContext {
    WinContext {
        concealed: Counts::parse(concealed).unwrap(),
        melds,
        winning_tile: Tile::new(0).unwrap(),
        self_draw,
        seat_wind: Tile::new(27).unwrap(),  // east
        round_wind: Tile::new(27).unwrap(), // east
    }
}

fn rules(addons: &[&str], offset: i32) -> Ruleset {
    let options = RoomOptions {
        addons: addons.iter().map(|s| s.to_string()).collect(),
        disabled_hands: vec![],
        threshold_offset: offset,
    };
    Ruleset::resolve(
        &builtin::base("hk_fan").unwrap(),
        &builtin::addons(),
        &options,
    )
    .unwrap()
}

fn ids(result: &ScoreResult) -> Vec<&str> {
    result.matches.iter().map(|m| m.id.as_str()).collect()
}

#[test]
fn builtin_data_loads() {
    assert_eq!(builtin::bases().len(), 3);
    assert_eq!(builtin::addons().len(), 4);
    let r = rules(&[], 0);
    assert_eq!(r.threshold, 3);
    assert_eq!(r.hands.len(), 16);
}

#[test]
fn rejects_non_wins() {
    let r = rules(&[], 0);
    assert_eq!(
        score(&r, &ctx("124m456p789s11122z", vec![], false)),
        Err(ScoreError::NotAWin)
    );
}

#[test]
fn common_hand_is_below_default_threshold() {
    let hand = ctx("123m456m789m123p55p", vec![], false);
    assert_eq!(
        score(&rules(&[], 0), &hand),
        Err(ScoreError::BelowThreshold { have: 1, need: 3 })
    );
    // Lowering the threshold by 2 lets it win, worth 1 fan.
    let result = score(&rules(&[], -2), &hand).unwrap();
    assert_eq!(ids(&result), vec!["common_hand"]);
    assert_eq!((result.qualifying, result.value, result.points), (1, 1, 2));
}

#[test]
fn self_draw_adds_a_fan() {
    let hand = ctx("123m456m789m123p55p", vec![], true);
    let result = score(&rules(&[], -2), &hand).unwrap();
    assert_eq!(ids(&result), vec!["common_hand", "self_draw"]);
    assert_eq!(result.value, 2);
}

#[test]
fn pure_flush_all_pungs_takes_best_reading() {
    // 111222333555m + 99m can also be read as three chows + a pung,
    // but the all-pungs reading scores higher: 3 + 6 = 9 fan.
    let hand = ctx("11122233355599m", vec![], false);
    let result = score(&rules(&[], 0), &hand).unwrap();
    assert_eq!(ids(&result), vec!["all_pungs", "pure_one_suit"]);
    assert_eq!((result.value, result.points), (9, 96));
}

#[test]
fn dragon_pung_is_one_fan_each() {
    let hand = ctx("123m45699p789s777z", vec![], false);
    let result = score(&rules(&[], -2), &hand).unwrap();
    assert_eq!(ids(&result), vec!["dragon_pung"]);
    assert_eq!(result.value, 1);
    assert_eq!(
        score(&rules(&[], 0), &hand),
        Err(ScoreError::BelowThreshold { have: 1, need: 3 })
    );
}

#[test]
fn big_three_dragons_excludes_individual_dragon_pungs() {
    let hand = ctx("123m99p555666777z", vec![], false);
    let result = score(&rules(&[], 0), &hand).unwrap();
    assert_eq!(ids(&result), vec!["big_three_dragons"]);
    assert_eq!((result.value, result.points), (8, 64));
}

#[test]
fn limit_hands_are_clamped() {
    let hand = ctx("19m19p19s12345677z", vec![], false);
    let result = score(&rules(&[], 0), &hand).unwrap();
    assert_eq!(ids(&result), vec!["thirteen_orphans"]);
    assert_eq!(
        (result.qualifying, result.value, result.points),
        (10, 10, 128)
    );
}

#[test]
fn open_hands_use_open_values_and_wind_pungs_score() {
    // Open east pung (seat and round wind = east) + one suit characters.
    let melds = vec![Set::pung(Tile::new(27).unwrap(), true)];
    let hand = ctx("123m456m789m55m", melds, false);
    let result = score(&rules(&[], 0), &hand).unwrap();
    assert_eq!(
        ids(&result),
        vec!["seat_wind_pung", "round_wind_pung", "mixed_one_suit"]
    );
    // 1 + 1 + 2 (open mixed one suit is 2, not 3)
    assert_eq!(result.value, 4);
}

#[test]
fn seven_pairs_scores_four() {
    let result = score(&rules(&[], 0), &ctx("1133m5588p2277s44z", vec![], false)).unwrap();
    assert_eq!(ids(&result), vec!["seven_pairs"]);
    assert_eq!(result.value, 4);
}

#[test]
fn addon_changes_a_value() {
    let result = score(
        &rules(&["seven_pairs_6fan"], 0),
        &ctx("1133m5588p2277s44z", vec![], false),
    )
    .unwrap();
    assert_eq!(result.value, 6);
}

#[test]
fn addon_adds_a_hand() {
    let hand = ctx("234m567p234s66p678s", vec![], false);
    // 234m 567p 234s 678s + 66p
    let with = score(&rules(&["all_simples"], 0), &hand);
    let without = score(&rules(&[], -2), &hand).unwrap();
    assert_eq!(ids(&without), vec!["common_hand"]);
    // all simples (1) + common hand (1) = 2, still under the default threshold of 3
    assert_eq!(with, Err(ScoreError::BelowThreshold { have: 2, need: 3 }));
    let lenient = score(&rules(&["all_simples", "lenient_threshold"], 0), &hand).unwrap();
    assert_eq!(ids(&lenient), vec!["common_hand", "all_simples"]);
    assert_eq!(lenient.value, 2);
}

#[test]
fn thresholds_shift_via_addons_and_options() {
    assert_eq!(rules(&["lenient_threshold"], 0).threshold, 2);
    assert_eq!(rules(&["strict_threshold"], 0).threshold, 4);
    assert_eq!(rules(&["strict_threshold"], 2).threshold, 6);
    assert_eq!(rules(&[], -10).threshold, 0);
}

#[test]
fn conflicting_addons_are_rejected() {
    let options = RoomOptions {
        addons: vec!["lenient_threshold".into(), "strict_threshold".into()],
        ..Default::default()
    };
    let err = Ruleset::resolve(
        &builtin::base("hk_fan").unwrap(),
        &builtin::addons(),
        &options,
    )
    .unwrap_err();
    assert!(matches!(err, RuleError::Conflict(..)));
}

#[test]
fn unknown_addon_and_hand_are_rejected() {
    let base = builtin::base("hk_fan").unwrap();
    let catalog = builtin::addons();
    let bad_addon = RoomOptions {
        addons: vec!["nope".into()],
        ..Default::default()
    };
    assert_eq!(
        Ruleset::resolve(&base, &catalog, &bad_addon),
        Err(RuleError::UnknownAddOn("nope".into()))
    );
    let bad_hand = RoomOptions {
        disabled_hands: vec!["nope".into()],
        ..Default::default()
    };
    assert_eq!(
        Ruleset::resolve(&base, &catalog, &bad_hand),
        Err(RuleError::UnknownHand("nope".into()))
    );
}

#[test]
fn addon_needing_missing_capability_is_unsupported() {
    let base = builtin::base("hk_fan").unwrap();
    let needs_dora = AddOn {
        id: "dora".into(),
        name: "Dora".into(),
        description: String::new(),
        requires: vec!["dora".into()],
        conflicts: vec![],
        order: 0,
        ops: vec![],
    };
    assert!(!needs_dora.supported_by(&base));
    let options = RoomOptions {
        addons: vec!["dora".into()],
        ..Default::default()
    };
    assert_eq!(
        Ruleset::resolve(&base, std::slice::from_ref(&needs_dora), &options),
        Err(RuleError::MissingCapability {
            addon: "dora".into(),
            needs: "dora".into()
        })
    );
    assert!(builtin::addons().iter().all(|a| a.supported_by(&base)));
}

#[test]
fn disabling_a_hand_removes_it_from_scoring() {
    let options = RoomOptions {
        disabled_hands: vec!["seven_pairs".into()],
        threshold_offset: -3,
        ..Default::default()
    };
    let r = Ruleset::resolve(
        &builtin::base("hk_fan").unwrap(),
        &builtin::addons(),
        &options,
    )
    .unwrap();
    // Seven pairs no longer scores; threshold 0 so an empty hand "wins" with 0.
    let result = score(&r, &ctx("1133m5588p2277s44z", vec![], false)).unwrap();
    assert!(result.matches.is_empty());
    assert_eq!(result.value, 0);
}
