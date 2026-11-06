// Plays a whole game through the real web UI (rendered in jsdom) against the real server,
// with three scripted bots sitting at the other seats.
import { JSDOM, VirtualConsole } from 'jsdom';
import WebSocket from 'ws';
import { spawn, execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const PORT = Number(process.env.PORT || 3055);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function waitFor(fn, what, timeout = 15000) {
  const start = Date.now();
  for (;;) {
    const v = fn();
    if (v) return v;
    if (Date.now() - start > timeout) throw new Error(`timed out waiting for ${what}`);
    await sleep(15);
  }
}

// ---- heuristic player shared by the bots and the UI driver ----
function usefulness(counts, tile) {
  let u = counts[tile] * 4;
  if (tile < 27) {
    for (const d of [-2, -1, 1, 2]) {
      const o = tile + d;
      if (o >= 0 && o < 27 && Math.floor(o / 9) === Math.floor(tile / 9)) u += counts[o] * (Math.abs(d) === 1 ? 2 : 1);
    }
  }
  return u;
}

function chooseDiscard(view) {
  const counts = new Array(34).fill(0);
  for (const t of view.hand) counts[t]++;
  if (view.drawn != null) counts[view.drawn]++;
  const options = view.legal.filter((a) => a.type === 'discard').map((a) => a.tile);
  options.sort((a, b) => usefulness(counts, a) - usefulness(counts, b) || a - b);
  return options[0];
}

function decide(state) {
  const view = state.view;
  if (!view.legal.length) {
    return view.phase === 'hand_over' && !state.acks[state.seat] ? { type: 'next_hand' } : null;
  }
  const has = (t) => view.legal.find((a) => a.type === t);
  const action = has('win') || has('pung') || has('kong') || has('pass') || { type: 'discard', tile: chooseDiscard(view) };
  return { type: 'act', action };
}

function startBot(code, name) {
  const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
  ws.on('open', () => ws.send(JSON.stringify({ type: 'join', code, name })));
  ws.on('message', (data) => {
    const msg = JSON.parse(data);
    if (msg.type !== 'state' || msg.view.phase === 'game_over') return;
    const reply = decide(msg);
    if (reply) ws.send(JSON.stringify(reply));
  });
  return ws;
}

// ---- boot the server ----
console.log('building server…');
execSync('cargo build -p mahjongi-server', { cwd: root, stdio: 'inherit' });
const server = spawn(path.join(root, 'target/debug/mahjongi-server'), [], {
  env: { ...process.env, PORT: String(PORT), WEB_DIR: path.join(root, 'web') },
  stdio: 'inherit',
});
const bots = [];
let exitCode = 0;

try {
  await waitFor(async () => false, 'noop', 1).catch(() => {});
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${PORT}/`)).ok) break; } catch (e) { /* not up yet */ }
    await sleep(100);
  }

  const errors = [];
  const vc = new VirtualConsole();
  vc.on('jsdomError', (e) => errors.push(e.message));
  vc.on('error', (e) => errors.push(String(e)));
  const dom = await JSDOM.fromURL(`http://127.0.0.1:${PORT}/`, {
    runScripts: 'dangerously', resources: 'usable', pretendToBeVisual: true, virtualConsole: vc,
  });
  const { window } = dom;
  const doc = window.document;
  const $ = (sel) => doc.querySelector(sel);
  const $$ = (sel) => [...doc.querySelectorAll(sel)];
  const button = (text) => $$('button').find((b) => b.textContent.includes(text) && !b.disabled);
  const click = (el) => el.dispatchEvent(new window.MouseEvent('click', { bubbles: true }));
  const S = () => window.__mj;

  // home screen
  const nameInput = await waitFor(() => $('input[placeholder="Your name"]'), 'home screen');
  nameInput.value = 'Eric';
  nameInput.dispatchEvent(new window.Event('input', { bubbles: true }));
  click(await waitFor(() => button('Create room'), 'create button'));

  // lobby
  const code = (await waitFor(() => $('.code'), 'room code')).textContent;
  console.log('room', code);
  for (const n of ['Bot1', 'Bot2', 'Bot3']) bots.push(startBot(code, n));
  await waitFor(() => button('Start game'), 'four players');

  // pick another ruleset through the dropdown, check it counts in tai, then come back
  const setSelect = (select, value) => {
    select.value = value;
    select.dispatchEvent(new window.Event('change', { bubbles: true }));
  };
  const rulesetSelect = () => $$('select').find((x) => [...x.options].some((o) => o.value === 'sg_tai'));
  setSelect(await waitFor(rulesetSelect, 'ruleset picker'), 'sg_tai');
  await waitFor(() => S().lobby.rules.unit === 'tai', 'switched to tai');
  if (!$('.stepper').textContent.includes('tai')) throw new Error('lobby should show tai');
  setSelect(rulesetSelect(), 'hk_fan');
  await waitFor(() => S().lobby.rules.unit === 'fan', 'back to fan');

  // turn timer
  const timerSelect = $$('select').find((x) => [...x.options].some((o) => o.value === '30'));
  setSelect(timerSelect, '60');
  await waitFor(() => S().lobby.settings.turn_secs === 60, 'timer set');

  // change the threshold from 3 down to 0
  for (let want = 2; want >= 0; want--) {
    click(button('−'));
    await waitFor(() => $('.stepper b').textContent === String(want), `threshold ${want}`);
  }
  // add-on: All Simples
  const addonBox = await waitFor(() => $$('label').find((l) => l.textContent.includes('All Simples') && l.querySelector('input')), 'add-on');
  const check = addonBox.querySelector('input');
  check.checked = true;
  check.dispatchEvent(new window.Event('change', { bubbles: true }));
  await waitFor(() => S().lobby.settings.addons.includes('all_simples'), 'add-on enabled');
  // hand off: Seven Pairs
  const sp = $$('label').find((l) => [...l.querySelectorAll('span')].some((x) => x.textContent === 'Seven Pairs'));
  sp.querySelector('input').checked = false;
  sp.querySelector('input').dispatchEvent(new window.Event('change', { bubbles: true }));
  await waitFor(() => S().lobby.settings.disabled_hands.includes('seven_pairs'), 'hand switched off');
  if (S().lobby.rules.hands.some((x) => x.id === 'seven_pairs')) throw new Error('seven pairs should be off');
  console.log('lobby ok: threshold', S().lobby.rules.threshold, 'hands', S().lobby.rules.hands.length);

  click(button('Start game'));
  await waitFor(() => S().state, 'game start');
  if (S().state.turn_secs !== 60) throw new Error('timer should carry into the game');
  await waitFor(() => $('#turn-timer') && /⏱ \d+s/.test($('#turn-timer').textContent), 'countdown shown');

  // play through the UI
  let wins = 0;
  let overlaysSeen = 0;
  let lastOverlayKey = '';
  const started = Date.now();
  while (!$$('h3').some((h) => h.textContent === 'Final standings')) {
    if (Date.now() - started > 150000) throw new Error('game did not finish');
    const overlay = $('.overlay');
    if (overlay) {
      const key = S().state.view.hand_no + ':' + S().state.view.phase;
      if (key !== lastOverlayKey) {
        lastOverlayKey = key;
        overlaysSeen++;
        if (overlay.textContent.includes('wins!')) wins++;
      }
      const next = button('Next hand');
      if (next) click(next);
      await sleep(15);
      continue;
    }
    const view = S().state.view;
    const actions = $$('.actions button');
    const pick = (t) => actions.find((b) => b.textContent.startsWith(t));
    const act = pick('🎉') || pick('Pung') || pick('Kong') || pick('Pass');
    if (act) {
      click(act);
    } else if (view.phase === 'discard' && view.turn === view.seat && view.legal.length) {
      const tile = chooseDiscard(view);
      const tiles = $$('.tiles.hand .tile');
      let idx = view.hand.indexOf(tile);
      if (idx < 0) idx = tiles.length - 1; // the drawn tile is last
      click(tiles[idx]);
      click(tiles[idx]);
    }
    await sleep(10);
  }

  // the last hand's panel is shown together with the standings, so count it here
  if ($('.overlay') && $('.overlay').textContent.includes('wins!')) wins++;
  const handsPlayed = S().state.view.hand_no;
  const standings = $$('ol li').map((li) => li.textContent);
  const scores = S().state.view.scores;
  console.log('standings:', standings.join(' | '));
  if (standings.length !== 4) throw new Error('expected four standings');
  if (scores.reduce((a, b) => a + b, 0) !== 0) throw new Error('scores must sum to zero');
  if (handsPlayed < 4) throw new Error(`expected at least 4 hands, played ${handsPlayed}`);
  if (wins < 1) throw new Error('expected at least one win to be shown');
  if (errors.length) throw new Error('page errors: ' + errors.join('; '));
  console.log(`UI smoke test passed: ${handsPlayed} hands, ${wins} wins shown, no page errors`);
  window.close();
} catch (err) {
  console.error('UI smoke test FAILED:', err);
  exitCode = 1;
} finally {
  bots.forEach((b) => b.close());
  server.kill();
  process.exit(exitCode);
}
