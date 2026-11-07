// Fills a room with server bots through the lobby buttons and plays one hand against them
// through the real UI, with the normal pause after discards and bot delay switched on.
import { JSDOM, VirtualConsole } from 'jsdom';
import { spawn, execSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const PORT = Number(process.env.PORT || 3056);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function waitFor(fn, what, timeout = 20000) {
  const start = Date.now();
  for (;;) {
    const v = fn();
    if (v) return v;
    if (Date.now() - start > timeout) throw new Error(`timed out waiting for ${what}`);
    await sleep(20);
  }
}

execSync('cargo build -q -p mahjongi-server', { cwd: root, stdio: 'inherit' });
const server = spawn(path.join(root, 'target/debug/mahjongi-server'), [], {
  env: { ...process.env, PORT: String(PORT), WEB_DIR: path.join(root, 'web'),
    CLAIM_WINDOW_SECS: '0.3', CLAIM_HOLD_SECS: '0.6', BOT_DELAY_SECS: '0.05' },
  stdio: 'inherit',
});
let exitCode = 0;
try {
  for (let i = 0; i < 100; i++) {
    try { if ((await fetch(`http://127.0.0.1:${PORT}/`)).ok) break; } catch (e) { /* not up */ }
    await sleep(100);
  }
  const errors = [];
  const vc = new VirtualConsole();
  vc.on('jsdomError', (e) => errors.push(e.message));
  const { window } = await JSDOM.fromURL(`http://127.0.0.1:${PORT}/`, {
    runScripts: 'dangerously', resources: 'usable', pretendToBeVisual: true, virtualConsole: vc,
  });
  const doc = window.document;
  const $ = (s) => doc.querySelector(s);
  const $$ = (s) => [...doc.querySelectorAll(s)];
  const button = (text) => $$('button').find((b) => b.textContent.includes(text) && !b.disabled);
  const click = (el) => el.dispatchEvent(new window.MouseEvent('click', { bubbles: true }));
  const S = () => window.__mj;

  const name = await waitFor(() => $('input[placeholder="Your name"]'), 'home');
  name.value = 'Eric';
  name.dispatchEvent(new window.Event('input', { bubbles: true }));
  click(await waitFor(() => button('Create room'), 'create'));

  for (let n = 1; n <= 3; n++) {
    click(await waitFor(() => button('Add a bot'), 'add bot button'));
    await waitFor(() => S().lobby && S().lobby.players.filter(Boolean).length === n + 1, `bot ${n}`);
  }
  if (button('Add a bot')) throw new Error('no add button on a full table');
  if (!$('.players-list').textContent.includes('🤖 Bot 3')) throw new Error('bots should be listed');
  click(button('Remove'));
  await waitFor(() => button('Add a bot'), 'seat freed');
  click(button('Add a bot'));
  click(await waitFor(() => button('Start game'), 'start'));
  await waitFor(() => S().state, 'game');

  let myDiscards = 0;
  let botDiscards = 0;
  let lastEvent = '';
  const t0 = Date.now();
  while (!$('.overlay')) {
    if (Date.now() - t0 > 90000) throw new Error('hand did not finish');
    const st = S().state;
    const ev = JSON.stringify(st.view.last_event);
    if (ev !== lastEvent) {
      lastEvent = ev;
      if (st.view.last_event && st.view.last_event.type === 'discard' && st.view.last_event.seat !== st.seat) botDiscards++;
    }
    const actions = $$('.actions button');
    const act = actions.find((b) => b.textContent.startsWith('🎉')) || actions.find((b) => b.textContent.startsWith('Pass'));
    if (act) click(act);
    else if (st.view.phase === 'discard' && st.view.turn === st.seat && st.view.legal.length) {
      const tiles = $$('.tiles.hand .tile');
      click(tiles[tiles.length - 1]);
      click(tiles[tiles.length - 1]);
      myDiscards++;
      await waitFor(() => S().state.view.turn !== st.seat || S().state.view.phase !== 'discard', 'my discard');
    }
    await sleep(20);
  }
  if (!S().state.names.slice(1).every((n) => n.startsWith('🤖'))) throw new Error('bots should be marked on the table');
  if (errors.length) throw new Error('page errors: ' + errors.join('; '));
  if (botDiscards < 3) throw new Error(`bots barely played (${botDiscards} discards)`);
  console.log(`bots test passed: hand finished, ${myDiscards} of my discards, ${botDiscards} bot discards`);
  window.close();
} catch (err) {
  console.error('bots test FAILED:', err);
  exitCode = 1;
} finally {
  server.kill();
  process.exit(exitCode);
}
