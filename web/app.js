(() => {
  'use strict';

  const WIND_CJK = ['東', '南', '西', '北'];
  const WIND_EN = ['East', 'South', 'West', 'North'];
  const SUIT_CJK = ['萬', '筒', '索'];
  const SAVE_KEY = 'mahjongi.session';
  const NAME_KEY = 'mahjongi.name';

  const S = {
    ws: null,
    connected: false,
    name: safeGet(NAME_KEY) || '',
    joinCode: new URLSearchParams(location.search).get('room') || '',
    lobby: null,
    state: null,
    selected: null,
    showRules: false,
    rejoining: false,
  };
  window.__mj = S; // handy for debugging and UI tests

  // ---------- helpers ----------

  // The seat to rejoin is kept per tab (sessionStorage) so several tabs of one browser can
  // play as different people; the player's name is shared (localStorage).
  function safeGet(key, store) {
    try { return (store || localStorage).getItem(key); } catch (e) { return null; }
  }
  function safeSet(key, value, store) {
    try {
      const s = store || localStorage;
      if (value == null) s.removeItem(key); else s.setItem(key, value);
    } catch (e) { /* ignore */ }
  }
  function loadSession() {
    try { return JSON.parse(safeGet(SAVE_KEY, sessionStorage) || 'null'); } catch (e) { return null; }
  }
  function saveSession(value) {
    safeSet(SAVE_KEY, value, sessionStorage);
  }

  function h(tag, props, ...kids) {
    const el = document.createElement(tag);
    for (const [k, v] of Object.entries(props || {})) {
      if (v === false || v == null) continue;
      if (k === 'class') el.className = v;
      else if (k.startsWith('on')) el.addEventListener(k.slice(2).toLowerCase(), v);
      else if (k === 'value' || k === 'checked' || k === 'disabled' || k === 'open') el[k] = v;
      else el.setAttribute(k, v === true ? '' : v);
    }
    for (const kid of kids.flat(Infinity)) {
      if (kid == null || kid === false) continue;
      el.append(kid.nodeType ? kid : document.createTextNode(String(kid)));
    }
    return el;
  }

  function send(msg) {
    if (S.ws && S.ws.readyState === WebSocket.OPEN) S.ws.send(JSON.stringify(msg));
  }

  let toastTimer = null;
  function toast(text) {
    const el = document.getElementById('toast');
    el.textContent = text;
    el.hidden = false;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => { el.hidden = true; }, 3500);
  }

  function banner(text) {
    const el = document.getElementById('banner');
    el.textContent = text || '';
    el.hidden = !text;
  }

  // ---------- tiles ----------

  function tileFace(i) {
    if (i < 27) {
      const suit = Math.floor(i / 9);
      return { cls: 'mps'[suit], big: String((i % 9) + 1), suit: SUIT_CJK[suit] };
    }
    if (i < 31) return { cls: 'z', big: WIND_CJK[i - 27], suit: '' };
    return {
      cls: 'z ' + ['dragon-white', 'dragon-green', 'dragon-red'][i - 31],
      big: ['白', '發', '中'][i - 31],
      suit: '',
    };
  }

  function tileText(i) {
    const f = tileFace(i);
    return f.big + f.suit;
  }

  function tileEl(i, opts = {}) {
    const f = tileFace(i);
    const cls = ['tile', f.cls, opts.size, opts.selectable && 'selectable', opts.selected && 'selected',
      opts.dim && 'dim', opts.hot && 'hot', opts.win && 'win'].filter(Boolean).join(' ');
    return h('div', { class: cls, title: tileText(i), onclick: opts.onclick },
      h('span', { class: 'big' }, f.big), f.suit ? h('span', { class: 'suit' }, f.suit) : null);
  }

  function backEl(size) {
    return h('div', { class: ['tile', 'back', size].filter(Boolean).join(' ') });
  }

  function meldEl(m, size) {
    const t = m.tile;
    let tiles;
    if (m.kind === 'chow') tiles = [t, t + 1, t + 2].map((x) => tileEl(x, { size }));
    else if (m.kind === 'pung') tiles = [t, t, t].map((x) => tileEl(x, { size }));
    else if (m.open) tiles = [t, t, t, t].map((x) => tileEl(x, { size }));
    else tiles = [backEl(size), tileEl(t, { size }), tileEl(t, { size }), backEl(size)];
    return h('div', { class: 'meld', title: m.kind + (m.open ? '' : ' (concealed)') }, tiles);
  }

  // ---------- connection ----------

  function connect() {
    const proto = location.protocol === 'https:' ? 'wss://' : 'ws://';
    const ws = new WebSocket(proto + location.host + '/ws');
    S.ws = ws;
    ws.onopen = () => {
      S.connected = true;
      banner('');
      const saved = loadSession();
      if (saved) {
        S.rejoining = true;
        send({ type: 'join', code: saved.code, name: saved.name || S.name || 'Player', token: saved.token });
      }
      render();
    };
    ws.onmessage = (e) => {
      let msg;
      try { msg = JSON.parse(e.data); } catch (err) { return; }
      onMessage(msg);
    };
    ws.onclose = () => {
      S.connected = false;
      banner('Connection lost. Reconnecting…');
      setTimeout(connect, 1500);
    };
  }

  function onMessage(msg) {
    switch (msg.type) {
      case 'joined':
        S.rejoining = false;
        saveSession(JSON.stringify({ code: msg.code, token: msg.token, name: S.name }));
        break;
      case 'lobby':
        S.lobby = msg;
        S.state = null;
        break;
      case 'state':
        S.state = msg;
        S.lobby = null;
        S.selected = null;
        S.timerEnd = msg.turn_left_ms == null ? null : Date.now() + msg.turn_left_ms;
        break;
      case 'error':
        if (S.rejoining) {
          S.rejoining = false;
          saveSession(null);
          S.lobby = null;
          S.state = null;
        } else {
          toast(msg.message);
        }
        break;
      default:
        break;
    }
    render();
  }

  // Countdown shown on the table; updated in place so the hand is not redrawn every tick.
  setInterval(() => {
    const el = document.getElementById('turn-timer');
    if (!el) return;
    if (S.timerEnd == null) { el.textContent = ''; return; }
    const left = Math.max(0, Math.ceil((S.timerEnd - Date.now()) / 1000));
    el.textContent = `⏱ ${left}s`;
    el.className = 'timer' + (left <= 5 ? ' low' : '');
  }, 250);

  function leave() {
    send({ type: 'leave' });
    saveSession(null);
    S.lobby = null;
    S.state = null;
    render();
  }

  // ---------- screens ----------

  function render() {
    const app = document.getElementById('app');
    let view;
    if (S.state) view = renderTable(S.state);
    else if (S.lobby) view = renderLobby(S.lobby);
    else view = renderHome();
    app.replaceChildren(view);
  }

  function renderHome() {
    const nameInput = h('input', {
      type: 'text', placeholder: 'Your name', maxlength: '20', value: S.name,
      oninput: (e) => { S.name = e.target.value; safeSet(NAME_KEY, S.name); },
    });
    const codeInput = h('input', {
      type: 'text', placeholder: 'Room code', maxlength: '4', value: S.joinCode,
      style: 'text-transform:uppercase',
      oninput: (e) => { S.joinCode = e.target.value; },
    });
    return h('div', {},
      h('h1', {}, '🀄 Mahjongi'),
      h('p', { class: 'muted' }, 'Four-player mahjong with rules you can switch on and off. Nothing is saved after a game.'),
      h('div', { class: 'card' },
        h('h2', {}, 'Your name'), nameInput),
      h('div', { class: 'card' },
        h('h2', {}, 'Start a new room'),
        h('p', { class: 'muted small' }, 'You choose the rules, then share the room code with three friends.'),
        h('button', { disabled: !S.connected, onclick: () => send({ type: 'create', name: S.name }) }, 'Create room')),
      h('div', { class: 'card' },
        h('h2', {}, 'Join a room'),
        h('div', { class: 'row' },
          h('div', { class: 'grow' }, codeInput),
          h('button', { disabled: !S.connected, onclick: () => send({ type: 'join', code: S.joinCode, name: S.name }) }, 'Join'))));
  }

  function renderLobby(L) {
    const isHost = L.seat === L.host;
    const st = L.settings;
    const update = (patch) => send({ type: 'set_options', settings: { ...st, ...patch } });

    const toggleAddon = (id, on) => {
      const info = L.addons.find((a) => a.id === id);
      let list = st.addons.filter((x) => x !== id);
      if (on) {
        // enabling an add-on switches off anything it conflicts with
        list = list.filter((x) => !info.conflicts.includes(x));
        for (const other of L.addons) {
          if (other.conflicts.includes(id)) list = list.filter((x) => x !== other.id);
        }
        list.push(id);
      }
      update({ addons: list });
    };
    const toggleHand = (id, on) => {
      const list = st.disabled_hands.filter((x) => x !== id);
      if (!on) list.push(id);
      update({ disabled_hands: list });
    };

    const unit = L.rules.unit;
    const valueOf = (id) => L.rules.hands.find((x) => x.id === id);
    const players = h('ul', { class: 'players-list' }, L.players.map((p, i) =>
      h('li', {},
        h('span', {}, p ? p.name : 'Waiting for player…', i === L.seat ? ' (you)' : '', i === L.host ? ' ★' : ''),
        p ? h('span', { class: 'badge' + (p.connected ? '' : ' off') }, p.connected ? 'here' : 'away') : null)));

    const rulesetBox = h('div', { class: 'card' },
      h('h2', {}, 'Ruleset'),
      h('select', { disabled: !isHost,
        onchange: (e) => update({ base: e.target.value, addons: [], disabled_hands: [], threshold_offset: 0 }) },
        L.bases.map((b) => h('option', { value: b.id, selected: b.id === st.base }, b.name))));

    const addonBox = h('div', { class: 'card' },
      h('h2', {}, 'Add-ons'),
      L.addons.length ? null : h('p', { class: 'muted' }, 'No add-ons for this ruleset.'),
      L.addons.map((a) => h('label', {},
        h('input', { type: 'checkbox', checked: st.addons.includes(a.id), disabled: !isHost,
          onchange: (e) => toggleAddon(a.id, e.target.checked) }),
        h('span', {}, h('b', {}, a.name), h('div', { class: 'muted small' }, a.description)))));

    const handBox = h('div', { class: 'card handlist' },
      h('h2', {}, 'Hands that can win'),
      h('p', { class: 'muted small' }, `Switch hands off to stop them counting. Values are in ${unit} (open hands in brackets when lower).`),
      L.all_hands.map((hd) => {
        const on = !st.disabled_hands.includes(hd.id);
        const v = valueOf(hd.id);
        const value = v ? (v.open_value !== v.value ? `${v.value} (${v.open_value}) ${unit}` : `${v.value} ${unit}`) : 'off';
        return h('label', {},
          h('input', { type: 'checkbox', checked: on, disabled: !isHost, onchange: (e) => toggleHand(hd.id, e.target.checked) }),
          h('span', { class: 'grow' }, hd.name), h('span', { class: 'badge' + (on ? '' : ' off') }, value));
      }));

    const thresholdBox = h('div', { class: 'card' },
      h('h2', {}, 'Minimum to win'),
      h('div', { class: 'stepper' },
        h('button', { class: 'secondary', disabled: !isHost, onclick: () => update({ threshold_offset: st.threshold_offset - 1 }) }, '−'),
        h('b', {}, L.rules.threshold),
        h('button', { class: 'secondary', disabled: !isHost, onclick: () => update({ threshold_offset: st.threshold_offset + 1 }) }, '+'),
        h('span', { class: 'muted' }, unit + ' (hands worth less cannot win; limit ' + L.rules.limit + ')')));

    const gameBox = h('div', { class: 'card' },
      h('h2', {}, 'Game'),
      h('label', {},
        h('input', { type: 'checkbox', checked: st.flowers, disabled: !isHost, onchange: (e) => update({ flowers: e.target.checked }) }),
        h('span', {}, 'Use flower tiles (144 tiles instead of 136)')),
      h('div', { class: 'row' },
        h('span', {}, 'Wind rounds'),
        h('select', { disabled: !isHost, onchange: (e) => update({ rounds: Number(e.target.value) }) },
          [1, 2, 3, 4].map((n) => h('option', { value: n, selected: n === st.rounds }, n === 1 ? '1 (East only)' : String(n))))),
      h('div', { class: 'row' },
        h('span', {}, 'Turn timer'),
        h('select', { disabled: !isHost, onchange: (e) => update({ turn_secs: Number(e.target.value) }) },
          [0, 10, 15, 20, 30, 60].map((n) => h('option', { value: n, selected: n === st.turn_secs }, n === 0 ? 'Off' : `${n} seconds`)))),
      st.turn_secs ? h('p', { class: 'muted small' }, 'If you run out of time the game passes on a claim, or throws the tile you just drew.') : null);

    return h('div', {},
      h('div', { class: 'topbar' }, h('h1', {}, '🀄 Room'), h('button', { class: 'secondary', onclick: leave }, 'Leave')),
      h('div', { class: 'card' },
        h('div', { class: 'muted' }, 'Room code'),
        h('div', { class: 'code' }, L.code),
        h('div', { class: 'row' },
          h('button', { class: 'secondary', onclick: () => copyInvite(L.code) }, 'Copy invite link')),
        players),
      isHost ? null : h('p', { class: 'muted' }, 'The host (★) chooses the rules and starts the game.'),
      rulesetBox, thresholdBox, addonBox, handBox, gameBox,
      isHost ? h('button', { disabled: !L.can_start, onclick: () => send({ type: 'start' }) },
        L.can_start ? 'Start game' : 'Waiting for four players…') : null);
  }

  function copyInvite(code) {
    const url = `${location.origin}/?room=${code}`;
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(url).then(() => toast('Invite link copied'), () => window.prompt('Copy this link', url));
    } else {
      window.prompt('Copy this link', url);
    }
  }

  // ---------- table ----------

  function eventText(m) {
    const e = m.view.last_event;
    if (!e) return '';
    const n = (s) => m.names[s] || 'Someone';
    switch (e.type) {
      case 'hand_start': return `New hand — ${n(e.dealer)} deals`;
      case 'discard': return `${n(e.seat)} discarded ${tileText(e.tile)}`;
      case 'pung': return `${n(e.seat)} called pung on ${tileText(e.tile)}`;
      case 'chow': return `${n(e.seat)} called chow on ${tileText(e.tile)}`;
      case 'kong': return `${n(e.seat)} made a kong of ${tileText(e.tile)}`;
      case 'win': return e.from == null ? `${n(e.seat)} won by self-draw` : `${n(e.seat)} won off ${n(e.from)}`;
      case 'draw': return 'The wall ran out — drawn hand';
      default: return '';
    }
  }

  function panel(m, seat, cls) {
    const v = m.view;
    const p = v.players[seat];
    const isTurn = v.phase === 'discard' && v.turn === seat;
    const score = v.scores[seat];
    const lastDiscard = v.last_event && v.last_event.type === 'discard' && v.last_event.seat === seat
      && (v.phase === 'claim' || v.phase === 'discard');
    return h('section', { class: `panel ${cls}${isTurn ? ' turn' : ''}` },
      h('header', {},
        h('span', { class: 'badge' }, WIND_CJK[(seat - v.dealer + 4) % 4]),
        h('span', { class: 'name' }, m.names[seat] || `Seat ${seat + 1}`),
        m.connected[seat] ? null : h('span', { title: 'disconnected' }, '💤'),
        h('span', { class: 'muted small' }, seat === v.seat ? '' : `${p.hand_count} tiles`),
        h('span', { class: 'score ' + (score > 0 ? 'pos' : score < 0 ? 'neg' : '') }, (score > 0 ? '+' : '') + score)),
      p.melds.length || p.flowers.length ? h('div', { class: 'tiles' },
        p.melds.map((x) => meldEl(x, 'small')),
        p.flowers.map((f) => h('span', { class: 'flower' }, `花${f + 1}`))) : null,
      h('div', { class: 'tiles discards' }, p.discards.map((t, i) =>
        tileEl(t, { size: 'small', hot: lastDiscard && i === p.discards.length - 1 }))));
  }

  function actionButtons(m) {
    const v = m.view;
    const legal = v.legal;
    const buttons = [];
    const act = (action) => send({ type: 'act', action });
    if (legal.some((a) => a.type === 'win')) {
      buttons.push(h('button', { class: 'win', onclick: () => act({ type: 'win' }) }, '🎉 Win!'));
    }
    if (legal.some((a) => a.type === 'pung')) {
      buttons.push(h('button', { onclick: () => act({ type: 'pung' }) }, 'Pung'));
    }
    for (const a of legal.filter((x) => x.type === 'kong')) {
      buttons.push(h('button', { onclick: () => act(a) }, 'Kong ', tileEl(a.tile, { size: 'mini' })));
    }
    for (const a of legal.filter((x) => x.type === 'chow')) {
      buttons.push(h('button', { onclick: () => act(a) }, 'Chow ',
        [0, 1, 2].map((i) => tileEl(a.low + i, { size: 'mini', hot: v.claim && a.low + i === v.claim.discard }))));
    }
    if (legal.some((a) => a.type === 'pass')) {
      buttons.push(h('button', { class: 'secondary', onclick: () => act({ type: 'pass' }) }, 'Pass'));
    }
    return h('div', { class: 'actions' }, buttons);
  }

  function handRow(m) {
    const v = m.view;
    const discardable = new Set(v.legal.filter((a) => a.type === 'discard').map((a) => a.tile));
    const pick = (pos, tile) => {
      if (!discardable.has(tile)) return;
      if (S.selected === pos) {
        send({ type: 'act', action: { type: 'discard', tile } });
        S.selected = null;
      } else {
        S.selected = pos;
        render();
      }
    };
    const tiles = v.hand.map((t, i) => tileEl(t, {
      selectable: discardable.has(t), selected: S.selected === i, onclick: () => pick(i, t),
    }));
    if (v.drawn != null) {
      tiles.push(h('span', { class: 'gap' }));
      tiles.push(tileEl(v.drawn, {
        selectable: discardable.has(v.drawn), selected: S.selected === 'drawn', onclick: () => pick('drawn', v.drawn),
      }));
    }
    return h('div', { class: 'handwrap' }, h('div', { class: 'tiles hand' }, tiles));
  }

  function statusLine(m) {
    const v = m.view;
    const n = (s) => m.names[s] || 'Someone';
    const ev = eventText(m);
    let line = '';
    if (v.phase === 'discard') {
      line = v.turn === v.seat
        ? (v.legal.length ? 'Your turn — tap a tile twice to discard it' : 'Your turn')
        : `Waiting for ${n(v.turn)}…`;
    } else if (v.phase === 'claim') {
      line = v.legal.length
        ? `${n(v.claim.from)} discarded ${tileText(v.claim.discard)} — do you want it?`
        : `Waiting for ${v.claim.waiting} player${v.claim.waiting === 1 ? '' : 's'} to decide…`;
    }
    return h('div', { class: 'status' },
      h('div', { class: 'muted small' }, ev),
      h('div', { class: 'big' }, line),
      m.turn_secs ? h('div', { id: 'turn-timer', class: 'timer' }) : null);
  }

  function rulesPanel(m) {
    const r = m.rules;
    return h('div', { class: 'card' },
      h('h2', {}, 'Rules in this game'),
      h('p', { class: 'muted small' }, `You need ${r.threshold} ${r.unit} to win (limit ${r.limit} ${r.unit}).`),
      h('table', { class: 'matches' }, r.hands.map((x) => h('tr', {},
        h('td', {}, x.name), h('td', {}, x.open_value !== x.value ? `${x.value} (${x.open_value} open)` : String(x.value))))));
  }

  function renderTable(m) {
    const v = m.view;
    const me = v.seat;
    const over = v.phase === 'hand_over' || v.phase === 'game_over';
    return h('div', {},
      h('div', { class: 'topbar' },
        h('div', {},
          h('b', {}, `${WIND_EN[v.round_wind]} round · hand ${v.hand_no}`),
          h('div', { class: 'muted small' }, `Wall ${v.wall_remaining} · need ${v.threshold} ${m.rules.unit} · room ${m.code}`)),
        h('div', { class: 'row' },
          h('button', { class: 'secondary', onclick: () => { S.showRules = !S.showRules; render(); } }, 'Rules'),
          h('button', { class: 'secondary', onclick: () => { if (window.confirm('Leave the game? You can rejoin from this device.')) leave(); } }, 'Leave'))),
      S.showRules ? rulesPanel(m) : null,
      h('div', { class: 'table' },
        panel(m, (me + 2) % 4, 'top'), panel(m, (me + 3) % 4, 'left'), panel(m, (me + 1) % 4, 'right')),
      statusLine(m),
      over ? overlay(m) : null,
      over ? null : actionButtons(m),
      panel(m, me, 'me'),
      handRow(m));
  }

  function overlay(m) {
    const v = m.view;
    const o = v.outcome;
    if (!o) return null;
    const n = (s) => m.names[s] || 'Someone';
    const gameOver = v.phase === 'game_over';
    const box = h('div', { class: 'overlay' });

    if (o.win) {
      const w = o.win;
      const winTileAt = o.hands[w.winner].indexOf(w.winning_tile);
      box.append(
        h('h2', {}, `${n(w.winner)}${w.winner === v.seat ? ' (you)' : ''} wins!`),
        h('div', { class: 'muted' }, w.self_draw ? 'Won by self-draw' : `Won off ${n(w.from)}'s discard`),
        h('div', { class: 'hands-row tiles' },
          o.melds[w.winner].map((x) => meldEl(x, 'small')),
          o.hands[w.winner].map((t, i) => tileEl(t, { size: 'small', win: i === winTileAt }))),
        h('table', { class: 'matches' },
          w.matches.length ? w.matches.map((x) => h('tr', {},
            h('td', {}, x.name + (x.count > 1 ? ` ×${x.count}` : '') + (x.qualifying ? '' : ' (bonus)')),
            h('td', {}, `${x.value} ${m.rules.unit}`))) : h('tr', {}, h('td', {}, 'No scoring hands'), h('td', {}, `0 ${m.rules.unit}`)),
          h('tr', {}, h('td', {}, h('b', {}, 'Total')), h('td', {}, `${w.value} ${m.rules.unit} = ${w.points} pts`))));
    } else {
      box.append(h('h2', {}, 'Drawn hand'), h('p', { class: 'muted' }, 'The wall ran out. Nobody wins and no points change hands.'));
    }

    box.append(h('h3', {}, 'Points this hand'),
      h('div', { class: 'pay' }, [0, 1, 2, 3].map((s) => [
        h('span', {}, n(s)),
        h('span', { class: 'num score ' + (o.transfers[s] > 0 ? 'pos' : o.transfers[s] < 0 ? 'neg' : '') }, (o.transfers[s] > 0 ? '+' : '') + o.transfers[s]),
        h('span', { class: 'num' }, v.scores[s]),
      ])));

    box.append(h('details', {}, h('summary', {}, "Everyone's hands"),
      [0, 1, 2, 3].map((s) => h('div', { class: 'hands-row' },
        h('div', { class: 'muted small' }, n(s)),
        h('div', { class: 'tiles' }, o.melds[s].map((x) => meldEl(x, 'mini')), o.hands[s].map((t) => tileEl(t, { size: 'mini' })))))));

    if (gameOver) {
      const order = [0, 1, 2, 3].sort((a, b) => v.scores[b] - v.scores[a]);
      box.append(h('h3', {}, 'Final standings'),
        h('ol', {}, order.map((s) => h('li', {}, `${n(s)} — ${v.scores[s] > 0 ? '+' : ''}${v.scores[s]}`))),
        h('div', { class: 'row' },
          m.host === v.seat
            ? h('button', { onclick: () => send({ type: 'start' }) }, 'Play again')
            : h('span', { class: 'muted' }, 'Waiting for the host to start another game…'),
          h('button', { class: 'secondary', onclick: leave }, 'Leave')));
    } else {
      box.append(h('p', { class: 'muted small' }, `${n(v.dealer)} deals the next hand.`));
      const ready = m.acks[v.seat];
      const waiting = [0, 1, 2, 3].filter((s) => m.connected[s] && !m.acks[s]).map(n);
      box.append(h('div', { class: 'row' },
        h('button', { disabled: ready, onclick: () => send({ type: 'next_hand' }) }, ready ? 'Waiting for others…' : 'Next hand'),
        waiting.length ? h('span', { class: 'muted small' }, `Waiting for: ${waiting.join(', ')}`) : null));
    }
    return box;
  }

  // ---------- start ----------

  setInterval(() => send({ type: 'ping' }), 25000);
  render();
  connect();
})();
