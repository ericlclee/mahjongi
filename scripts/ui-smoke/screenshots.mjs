// Takes phone-sized screenshots of the lobby and table using the installed Chrome.
import { chromium } from 'playwright-core';
import WebSocket from 'ws';
import { spawn, execSync } from 'node:child_process';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const PORT = 3056;
const OUT = process.env.OUT || '/tmp/shots';
fs.mkdirSync(OUT, { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function usefulness(c, t) {
  let u = c[t] * 4;
  if (t < 27) for (const d of [-2, -1, 1, 2]) { const o = t + d; if (o >= 0 && o < 27 && Math.floor(o / 9) === Math.floor(t / 9)) u += c[o] * (Math.abs(d) === 1 ? 2 : 1); }
  return u;
}
function decide(state) {
  const v = state.view;
  if (!v.legal.length) return v.phase === 'hand_over' && !state.acks[state.seat] ? { type: 'next_hand' } : null;
  const has = (t) => v.legal.find((a) => a.type === t);
  let a = has('win') || has('pung') || has('kong') || has('pass');
  if (!a) {
    const c = new Array(34).fill(0);
    v.hand.forEach((t) => c[t]++); if (v.drawn != null) c[v.drawn]++;
    const o = v.legal.filter((x) => x.type === 'discard').map((x) => x.tile).sort((x, y) => usefulness(c, x) - usefulness(c, y) || x - y);
    a = { type: 'discard', tile: o[0] };
  }
  return { type: 'act', action: a };
}

execSync('cargo build -p mahjongi-server', { cwd: root, stdio: 'inherit' });
const server = spawn(path.join(root, 'target/debug/mahjongi-server'), [], { env: { CLAIM_WINDOW_SECS: '0', ...process.env, PORT: String(PORT), WEB_DIR: path.join(root, 'web') }, stdio: 'inherit' });
const bots = [];
const browser = await chromium.launch({ executablePath: '/usr/bin/google-chrome', args: ['--no-sandbox'] });
try {
  await sleep(800);
  const page = await (await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 })).newPage();
  page.on('pageerror', (e) => console.log('PAGE ERROR', e.message));
  await page.goto(`http://127.0.0.1:${PORT}/`);
  await page.screenshot({ path: `${OUT}/1-home.png` });
  await page.fill('input[placeholder="Your name"]', 'Eric');
  await page.click('text=Create room');
  const code = await page.textContent('.code');
  ['Mei', 'Kenji', 'Aroha'].forEach((name) => {
    const ws = new WebSocket(`ws://127.0.0.1:${PORT}/ws`);
    ws.on('open', () => ws.send(JSON.stringify({ type: 'join', code, name })));
    ws.on('message', (d) => { const m = JSON.parse(d); if (m.type === 'state' && m.view.phase !== 'game_over') { const r = decide(m); if (r) ws.send(JSON.stringify(r)); } });
    bots.push(ws);
  });
  await page.waitForSelector('text=Start game');
  for (let i = 0; i < 3; i++) await page.click('button:has-text("−")');
  await sleep(300);
  await page.screenshot({ path: `${OUT}/2-lobby.png`, fullPage: true });
  await page.click('text=Start game');

  const snap = () => page.evaluate(() => JSON.stringify(window.__mj.state));
  let shots = { turn: false, claim: false, over: false };
  const t0 = Date.now();
  while (Date.now() - t0 < 90000 && !(shots.turn && shots.claim && shots.over)) {
    const st = JSON.parse(await snap());
    if (!st) { await sleep(50); continue; }
    const v = st.view;
    const reply = decide(st);
    const myHand = v.hand.length;
    if (!shots.turn && v.phase === 'discard' && v.turn === v.seat && v.players.some((p) => p.discards.length >= 6)) {
      await sleep(100); await page.screenshot({ path: `${OUT}/3-my-turn.png`, fullPage: true }); shots.turn = true;
    }
    if (!shots.claim && v.phase === 'claim' && v.legal.length && v.legal.some((a) => a.type !== 'pass')) {
      await sleep(100); await page.screenshot({ path: `${OUT}/4-claim.png`, fullPage: true }); shots.claim = true;
    }
    if (!shots.over && v.phase === 'hand_over' && v.outcome && v.outcome.win) {
      await sleep(100); await page.screenshot({ path: `${OUT}/5-hand-over.png`, fullPage: true }); shots.over = true;
    }
    if (reply) await page.evaluate((m) => window.__mj.ws.send(JSON.stringify(m)), reply);
    await sleep(25);
  }
  console.log('screenshots:', shots, fs.readdirSync(OUT).join(', '));
} catch (e) {
  console.error('FAILED', e);
} finally {
  bots.forEach((b) => b.close()); await browser.close(); server.kill(); process.exit(0);
}
