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

## Deploy

The server is a single process with everything in memory, so run exactly one instance.
A `Dockerfile` is included, plus `render.yaml` (Render: New > Blueprint > pick this repo) and
`fly.toml` (Fly.io: `fly launch --copy-config --no-deploy && fly deploy`). The server listens on
`$PORT` and serves the web client itself, over `wss://` when the host uses https.

## How the rules work

- **Base:** `rulesets/bases/*.json`. A scoring model, a win threshold, a limit, a payout table and a
  list of hands. The host picks one in the lobby:
  - `hk_fan` Hong Kong, fan, minimum 3, limit 10.
  - `hk_classic` Hong Kong with an 8-fan limit plus Outside Hand, All Green and Three/Four Kongs.
  - `sg_tai` Singapore / Malaysia style, tai, minimum 1, limit 5. The fan add-ons are hidden for it.
- **Hands:** each is a pattern built from a few engine primitives (all chows, all pungs, one suit,
  pung of a dragon, and so on) plus a value per scoring model, with optional open-hand values.
- **Add-ons:** `rulesets/addons.json`. Small layers that add hands, change a value, or shift the
  threshold. They can declare `requires`, `conflicts` and `order`.
- **Room options:** switch hands off, shift the threshold and set a turn timer from the lobby.
- **Bots:** the host can tap "Add a bot" in the lobby to fill empty seats. Bots win when they
  can, take every pung and chow, and throw from the suit they hold fewest of. They wait 0.8
  seconds before each move so you can follow along (`BOT_DELAY_SECS` changes that).
- **Pause after discards:** every discard waits 3 seconds before the next draw, whether or not
  anyone can claim it, so the pause gives nothing away. Anyone who could claim the tile can tap
  Hold to stretch it to 10 seconds; nobody is told who. A claim made during the pause happens at
  once. Set `CLAIM_WINDOW_SECS` and `CLAIM_HOLD_SECS` to change the lengths (0 turns the pause off).
- **Turn timer:** off, or 10 to 60 seconds. When it runs out the server passes on a claim, or
  throws the tile just drawn. The countdown restarts after every action.

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

The values and payout tables in the base files (`hk_fan`, `hk_classic`, `sg_tai`) are reasonable defaults, not an
official rulebook. Payments: a self-drawn win is paid by each opponent, a win off a discard is paid
in full by the discarder, and anything paid to or by the dealer is doubled.

See `PLAN.md` for the roadmap.
