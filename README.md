# Mahjongi

Four-player mahjong in the browser with rules you can switch on and off. Pick a base ruleset
(Hong Kong fan scoring to start), layer add-ons on top, turn individual hands off, and raise or
lower the points needed to win. Nothing is remembered after a game ends.

Built from scratch in Rust (engine and server) with a plain HTML/JS client. It borrows ideas, not
code, from existing open-source mahjong projects.

## Run it

```sh
cargo run -p mahjongi-server          # serves the game on http://localhost:3000
PORT=8080 WEB_DIR=web cargo run -p mahjongi-server
```

Open the page on four devices (or four tabs), create a room on one, and join with the room code on
the others. The host picks the rules and starts the game. If you drop out, reopening the page
rejoins your seat.

## How the rules work

- **Base:** `rulesets/bases/*.json`. A scoring model, a win threshold, a limit, a payout table and a
  list of hands.
- **Hands:** each is a pattern built from a few engine primitives (all chows, all pungs, one suit,
  pung of a dragon, and so on) plus a value per scoring model, with optional open-hand values.
- **Add-ons:** `rulesets/addons.json`. Small layers that add hands, change a value, or shift the
  threshold. They can declare `requires`, `conflicts` and `order`.
- **Room options:** switch hands off and shift the threshold from the lobby.

Changing any of this is a JSON edit. The engine only needs new code when a hand needs a new kind
of pattern.

## Layout

| Path | What |
|---|---|
| `crates/engine` | Tiles, wall, win detection, patterns, scoring, the game state machine. Pure and deterministic. |
| `crates/server` | Rooms, lobby options, WebSocket play, static hosting. In-memory only. |
| `web/` | The browser client (no build step). |
| `rulesets/` | Bases and add-ons as data. |
| `scripts/ui-smoke` | Plays a full game through the real UI in jsdom; `screenshots.mjs` takes phone screenshots. |

## Test

```sh
cargo test --workspace
cd scripts/ui-smoke && npm install && npm test
```

## Scoring defaults

The fan values and payout table in `hk_fan.json` are reasonable Hong Kong style defaults, not an
official rulebook. Payments: a self-drawn win is paid by each opponent, a win off a discard is paid
in full by the discarder, and anything paid to or by the dealer is doubled.

See `PLAN.md` for the roadmap.
