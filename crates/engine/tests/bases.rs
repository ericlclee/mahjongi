use mahjongi_engine::builtin;
use mahjongi_engine::*;

fn ctx(concealed: &str, melds: Vec<Set>, self_draw: bool) -> WinContext {
    WinContext {
        concealed: Counts::parse(concealed).unwrap(),
        melds,
        winning_tile: Tile::new(0).unwrap(),
        self_draw,
        seat_wind: Tile::new(27).unwrap(),
        round_wind: Tile::new(27).unwrap(),
    }
}

fn rules(base: &str, offset: i32) -> Ruleset {
    let options = RoomOptions {
        threshold_offset: offset,
        ..Default::default()
    };
    Ruleset::resolve(&builtin::base(base).unwrap(), &builtin::addons(), &options).unwrap()
}

fn ids(result: &ScoreResult) -> Vec<&str> {
    result.matches.iter().map(|m| m.id.as_str()).collect()
}

#[test]
fn every_base_loads_and_has_unique_hand_ids() {
    for base in builtin::bases() {
        let mut seen = std::collections::HashSet::new();
        for h in &base.hands {
            assert!(seen.insert(h.id.clone()), "{} repeats {}", base.id, h.id);
            assert!(
                h.values.contains_key(&base.model),
                "{}: {} has no {} value",
                base.id,
                h.id,
                base.model
            );
        }
        assert!(Ruleset::resolve(&base, &builtin::addons(), &RoomOptions::default()).is_ok());
    }
}

#[test]
fn classic_has_a_lower_limit() {
    let r = rules("hk_classic", 0);
    assert_eq!(r.limit, 8);
    // Thirteen orphans is 13 fan in the base list, but clamps to the limit.
    let result = score(&r, &ctx("19m19p19s12345677z", vec![], false)).unwrap();
    assert_eq!(result.value, 8);
}

#[test]
fn classic_all_green_wins() {
    let r = rules("hk_classic", 0);
    let result = score(&r, &ctx("234s234s666s888s66z", vec![], false)).unwrap();
    assert!(ids(&result).contains(&"all_green"), "{:?}", ids(&result));
    assert_eq!(result.value, 8);
}

#[test]
fn classic_outside_hand_needs_a_terminal_or_honor_in_every_set() {
    let r = rules("hk_classic", -3);
    let outside = score(&r, &ctx("123m789m123p999p11z", vec![], false)).unwrap();
    assert!(ids(&outside).contains(&"outside_hand"));
    let inside = score(&r, &ctx("234m789m123p999p11z", vec![], false)).unwrap();
    assert!(!ids(&inside).contains(&"outside_hand"));
}

#[test]
fn tai_uses_its_own_scale() {
    let r = rules("sg_tai", 0);
    assert_eq!((r.threshold, r.limit), (1, 5));
    // A plain all-chows hand is worth 4 tai (plus 1 for being concealed), versus 1 fan in Hong Kong.
    let hand = ctx("123m456m789m123p55p", vec![], false);
    let result = score(&r, &hand).unwrap();
    assert_eq!(ids(&result), vec!["concealed", "common_hand"]);
    assert_eq!(result.value, 5);
    // A concealed hand with nothing else still earns its 1 tai, so it wins at a 1 tai minimum
    // but not at 2.
    let mixed = ctx("123m456p789s111m22p", vec![], false);
    assert_eq!(score(&r, &mixed).unwrap().value, 1);
    assert!(matches!(
        score(&rules("sg_tai", 1), &mixed),
        Err(ScoreError::BelowThreshold { have: 1, need: 2 })
    ));
}

#[test]
fn fan_add_ons_are_not_offered_for_tai() {
    let base = builtin::base("sg_tai").unwrap();
    assert!(builtin::addons().iter().all(|a| !a.supported_by(&base)));
    let options = RoomOptions {
        addons: vec!["all_simples".to_string()],
        ..Default::default()
    };
    assert!(Ruleset::resolve(&base, &builtin::addons(), &options).is_err());
}

#[test]
fn full_games_play_out_under_every_base() {
    for base in builtin::bases() {
        for seed in 0..6u64 {
            let options = RoomOptions {
                threshold_offset: -10,
                ..Default::default()
            };
            let rules = Ruleset::resolve(&base, &builtin::addons(), &options).unwrap();
            let mut game = Game::new(GameConfig::new(rules, seed));
            for _ in 0..50_000 {
                if game.is_over() {
                    break;
                }
                if game.is_between_hands() {
                    game.next_hand().unwrap();
                    continue;
                }
                for seat in game.waiting_seats() {
                    if let Some(a) = simple_bot::choose(&game, seat) {
                        game.apply(seat, a).unwrap();
                        break;
                    }
                }
            }
            assert!(game.is_over(), "{} seed {} did not finish", base.id, seed);
            assert_eq!(game.scores().iter().sum::<i32>(), 0);
        }
    }
}

#[test]
fn every_hand_has_an_example_that_scores_it() {
    let examples = builtin::examples();
    let mut hands: Vec<(String, Ruleset)> = Vec::new();
    for base in builtin::bases() {
        let options = RoomOptions {
            threshold_offset: -10,
            addons: builtin::addons()
                .iter()
                .filter(|a| a.supported_by(&base) && a.id != "strict_threshold")
                .map(|a| a.id.clone())
                .collect(),
            ..Default::default()
        };
        let rules = Ruleset::resolve(&base, &builtin::addons(), &options).unwrap();
        for h in &rules.hands {
            hands.push((h.id.clone(), rules.clone()));
        }
    }
    for (id, rules) in hands {
        let example = examples
            .get(&id)
            .unwrap_or_else(|| panic!("no example for {id}"));
        let (concealed, melds) = example.split();
        assert_eq!(
            concealed.total() + melds.len() * 3,
            14,
            "{id}: example is not a full hand"
        );
        let scored = |self_draw| {
            let ctx = WinContext {
                concealed: concealed.clone(),
                melds: melds.clone(),
                winning_tile: Tile::new(0).unwrap(),
                self_draw,
                seat_wind: Tile::new(27).unwrap(),
                round_wind: Tile::new(27).unwrap(),
            };
            score(&rules, &ctx)
                .map(|r| r.matches.iter().any(|m| m.id == id))
                .unwrap_or(false)
        };
        assert!(scored(true), "{id}: example does not score as {id}");
    }
}
