use mahjongi_engine::builtin;
use mahjongi_engine::*;

fn config(seed: u64, flowers: bool, offset: i32) -> GameConfig {
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
    let mut cfg = GameConfig::new(rules, seed);
    cfg.flowers = flowers;
    cfg
}

struct Played {
    game: Game,
    hands: u32,
    wins: u32,
}

/// Play a whole game with the heuristic bot, checking invariants after every step.
fn play(cfg: GameConfig) -> Played {
    let total = if cfg.flowers { 144 } else { 136 };
    let mut game = Game::new(cfg);
    let (mut hands, mut wins) = (0, 0);
    for _ in 0..50_000 {
        assert_eq!(game.total_pieces(), total, "pieces must be conserved");
        if game.is_over() || game.is_between_hands() {
            let outcome = game.outcome().expect("a finished hand has an outcome");
            assert_eq!(outcome.transfers.iter().sum::<i32>(), 0, "hand is zero-sum");
            hands += 1;
            wins += outcome.win.is_some() as u32;
            if game.is_over() {
                assert_eq!(game.scores().iter().sum::<i32>(), 0, "game is zero-sum");
                return Played { game, hands, wins };
            }
            game.next_hand().unwrap();
            continue;
        }
        let seat = *game
            .waiting_seats()
            .first()
            .expect("someone must be able to act");
        let action = simple_bot::choose(&game, seat).expect("bot has an action");
        game.apply(seat, action).unwrap();
    }
    panic!("game did not finish");
}

#[test]
fn many_games_finish_with_invariants_intact() {
    let mut wins = 0;
    let mut hands = 0;
    for seed in 0..60 {
        let p = play(config(seed, seed % 2 == 0, -3));
        assert!(p.hands >= 4, "a one-round game has at least four hands");
        wins += p.wins;
        hands += p.hands;
    }
    assert!(
        wins > 20,
        "bots should win some hands (won {wins} of {hands})"
    );
}

#[test]
fn default_threshold_games_also_finish() {
    for seed in 100..120 {
        play(config(seed, true, 0));
    }
}

#[test]
fn replaying_the_log_reproduces_the_game() {
    for seed in [3u64, 4, 5] {
        let cfg = config(seed, true, -3);
        let played = play(cfg.clone());
        let replayed = Game::replay(cfg, played.game.history()).unwrap();
        assert_eq!(replayed.scores(), played.game.scores());
        assert_eq!(replayed.history(), played.game.history());
        assert!(replayed.is_over());
    }
}

#[test]
fn views_hide_other_players_hands() {
    let game = Game::new(config(9, false, 0));
    let view = game.view(2);
    assert_eq!(view.hand.len(), 13);
    assert!(view.drawn.is_none());
    assert!(view.legal.is_empty());
    assert_eq!(view.players[0].hand_count, 14);
    let dealer = game.view(0);
    assert_eq!(dealer.hand.len(), 13);
    assert!(dealer.drawn.is_some());
    assert!(!dealer.legal.is_empty());
    let json = serde_json::to_string(&dealer).unwrap();
    assert!(json.contains("\"phase\":\"discard\""));
}
