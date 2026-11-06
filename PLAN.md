# Mahjongi: Project Plan

A web-based multiplayer mahjong game with a configurable ruleset. Players pick a simple base ruleset and layer optional add-ons on top. Written from scratch (borrowing ideas only, no code, from existing projects).

## Requirements

- Support rulesets for all major **13-tile** hand variants of mahjong.
- Basic web UI to play.
- Multiplayer (4 human players, no bots yet).
- Turn rulesets and individual hands on or off. A ruleset is a set of hands allowed for a win.
- Raise or lower the point threshold needed to win.
- In-game point tracking.
- Post-game conversion of points into account-level points (virtual currency) and Elo.

Out of scope for now: 16-tile variants (Taiwanese), American mahjong (joker card), bots, real-money play (virtual currency only, no cash-out).

## Architecture

Rust workspace plus a TypeScript web client.

| Component | Role |
|---|---|
| `engine` | Tiles, wall, hand decomposition, pattern primitives, scoring, game state machine. Pure and deterministic, no I/O. |
| `server` | axum + WebSockets. Authoritative game state, rooms, per-player views (hidden hands never leave the server), persistence. |
| `web` | Basic play UI with our own SVG tile art. |
| `rulesets/*.json` | Bases, add-ons and presets, defined as data. |

Rules are loaded as data at runtime, so changing rules never needs a recompile. Only a new kind of pattern primitive needs an engine change.

## Rules as data: base + add-ons

### Base
One scoring model with a default set of hands (first: **HK Fan**). The only thing a room must choose.

### Add-ons
Optional, toggleable layers applied on top of the base in a fixed order. An add-on does one small thing:
- adds hands (e.g. Thirteen Orphans, All Honors)
- changes a value (e.g. Seven Pairs 3 -> 4 fan)
- adds a bonus rule (e.g. dora adds +1 fan to payout only; riichi declaration +1 fan)
- shifts the win threshold (min fan +1 / -1)

Each add-on declares:
- `requires`: context fields it needs (e.g. dora tiles); unsupported add-ons are hidden or greyed out for a base
- `conflicts`: add-ons it cannot be combined with (auto-disabled on enable)
- `order`: application order

A **preset** is a saved base plus add-on list (e.g. "Casual HK").

### Hands
A hand is a matcher (built from engine primitives) plus a value per scoring model, and may differ when open vs closed:

```json
{
  "id": "all_pungs",
  "name": "All Pungs",
  "pattern": { "sets": { "pung": 4 }, "pair": 1 },
  "values": { "fan": 3 },
  "open_values": { "fan": 3 }
}
```

Built-in win shapes: 4 sets + pair, seven pairs, thirteen orphans.

### Threshold
Each value is flagged as `qualifying` (counts toward the threshold) or `payout_only` (e.g. dora). A win requires qualifying value >= the room's threshold. Payout uses qualifying + bonus value. In HK both are the same number; in Riichi-style rules they differ (dora cannot satisfy the minimum).

### Context object
Every hand evaluation receives: open/closed, how won (self-draw/discard), wait type, dealer, round/seat wind, dora tiles, flowers. A scoring model reads only the fields it uses.

## Scoring models

1. **Fan** (HK, Cantonese, Sichuan, Classical): fans add, payout doubles per fan up to a limit, minimum fan to win (default 3). *First base.*
2. **Han + fu** (Riichi): yaku required, fu from hand structure, limit tiers. *Later base.*
3. **Flat points** (MCR): fixed pattern points with exclusion rules, 8 point minimum. *Later base.*

One scoring model per room. Rulesets using different models are not mixed.

## Multiplayer

- Room codes, 4 human players, reconnect handling, turn timers.
- Claim windows with correct priority: win > pung/kong > chow.
- Event-sourced game log (replays, auditing, debugging).
- Seeded wall for reproducible tests.

## Points, Elo, virtual currency

- **In-game points:** payment rules per base (discarder pays vs all pay, dealer bonus), tracked in an event ledger.
- **Account points (virtual currency):** score change / ruleset limit x per-room stake multiplier, with a cap or diminishing returns so one huge hand cannot dominate. Each ruleset defines a normalizing limit so results are comparable across rulesets. No cash-out.
- **Elo:** computed from final placings (pairwise multiplayer Elo), tracked separately from account points: level vs skill.

## Milestones

1. **Setup:** workspace, CI, JSON schema for bases, add-ons, hands.
2. **Engine core:** tiles, wall, hand decomposition, win detection, tests against known hands.
3. **Pattern primitives + HK Fan base:** hands, scoring, threshold, add-on loader (order, requires, conflicts).
4. **Game state machine:** turns, claims, kongs, flowers, draws; random-game simulation tests.
5. **Server:** rooms, WebSockets, per-player views, CLI test client.
6. **Web UI:** lobby, table, rules/add-on picker.
7. **Accounts and ledger:** in-game tracking, account points, Elo.
8. **More rulesets:** Classical, Sichuan, Zung Jung, Singapore/Malaysia styles, then MCR, then Riichi (fu, furiten, riichi declaration) as a second base.

## Open questions

- Web framework for the client (React vs Svelte).
- Persistence (SQLite to start vs Postgres).
- Auth approach for accounts.
- Whether the engine also compiles to WASM for client-side hand hints.
