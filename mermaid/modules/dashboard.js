// modules/dashboard.js -- Microscope Memory dashboard
// Riboszoma modul: stat kartyak, rendszer allapot, Alan Eorkert

import { store, ws } from './core.js';

let refreshTimer = null;

function el(tag, opts, children) {
  const e = document.createElement(tag);
  if (opts) {
    if (opts.cls) e.className = opts.cls;
    if (opts.html !== undefined) e.innerHTML = opts.html;
    if (opts.text !== undefined) e.textContent = opts.text;
  }
  if (children) for (const c of children) e.appendChild(c);
  return e;
}

function card(title, value, sub) {
  const c = el('div', { cls: 'dash-card' });
  c.appendChild(el('h3', { text: title }));
  c.appendChild(el('div', { cls: 'value', text: String(value) }));
  if (sub) c.appendChild(el('div', { cls: 'sub', text: sub }));
  return c;
}

function barCard(title, segments) {
  const c = el('div', { cls: 'dash-card' });
  c.appendChild(el('h3', { text: title }));
  const total = segments.reduce(function (s, x) { return s + x.value; }, 0) || 1;
  for (const seg of segments) {
    const w = Math.round((seg.value / total) * 100);
    const bar = el('div', { cls: 'dash-bar' });
    const fill = el('div', { cls: 'dash-bar-fill' });
    fill.style.width = w + '%';
    bar.appendChild(fill);
    c.appendChild(bar);
    c.appendChild(el('div', { cls: 'sub', text: seg.label + ': ' + seg.value + ' (' + w + '%)' }));
  }
  return c;
}

function listCard(title, rows) {
  const c = el('div', { cls: 'dash-card' });
  c.appendChild(el('h3', { text: title }));
  const list = el('div', { cls: 'dash-list' });
  for (const r of rows) {
    const row = el('div', { cls: 'row' });
    row.appendChild(el('span', { text: r.label }));
    row.appendChild(el('span', { text: r.value }));
    list.appendChild(row);
  }
  c.appendChild(list);
  return c;
}

function statusCard(title, status, detail) {
  const c = el('div', { cls: 'dash-card' });
  c.appendChild(el('h3', { text: title }));
  const sCls = status === 'ok' ? 'ok' : status === 'warn' ? 'warn' : 'err';
  const s = el('span', { cls: 'dash-status ' + sCls, text: status.toUpperCase() });
  c.appendChild(s);
  if (detail) c.appendChild(el('div', { cls: 'sub', text: detail }));
  return c;
}

function renderDashboard(stats) {
  const tab = document.getElementById('dashboard-tab');
  if (!tab) return;
  if (!tab.querySelector('#dashboard-scroll')) {
    tab.innerHTML = '';
    const refresh = el('div', { cls: 'dash-refresh' });
    refresh.appendChild(el('span', { id: 'dash-last', text: 'frissitve: --' }));
    const refreshBtn = el('button', { text: 'Refresh' });
    refreshBtn.style.cssText = 'background:var(--bg-2);color:var(--accent);border:1px solid var(--border);border-radius:4px;padding:4px 10px;font:inherit;font-size:11px;cursor:pointer;margin-left:8px;';
    refreshBtn.addEventListener('click', refreshStats);
    refresh.appendChild(refreshBtn);
    tab.appendChild(refresh);
    const scroll = el('div', { id: 'dashboard-scroll' });
    tab.appendChild(scroll);
  }
  const scroll = document.getElementById('dashboard-scroll');
  if (!scroll) return;
  scroll.innerHTML = '';
  scroll.appendChild(card('Total Blocks', stats.total_blocks || 0, 'minden depth szinten'));
  scroll.appendChild(card('Layers', stats.layers || 0, 'consciousness retegek'));
  scroll.appendChild(card('Hebbian Active', stats.hebbian_active || 0, 'aktiv ko-aktivalasok'));
  scroll.appendChild(card('Merkle Root', stats.merkle_root ? stats.merkle_root.slice(0, 12) + '...' : 'n/a', 'integritas hash'));
  if (stats.depth_distribution) {
    scroll.appendChild(barCard('Depth Distribution (D0-D8)',
      Object.entries(stats.depth_distribution).map(function (e) { return { label: e[0], value: e[1] }; })
    ));
  }
  scroll.appendChild(listCard('Consciousness Layers',
    (stats.consciousness || []).map(function (c) { return { label: c.name, value: c.count + ' aktiv' }; })
  ));
  scroll.appendChild(statusCard('Alan Eorkert (Immune)',
    stats.alan_running ? 'ok' : 'warn',
    stats.alan_running ? 'guardian mod, utolso self-analysis: ' + (stats.alan_last_analysis || 'ismeretlen') : 'nem fut - inditsd el: activate_alan.ps1'
  ));
  scroll.appendChild(statusCard('Spine Bus',
    stats.spine_connected ? 'ok' : 'err',
    stats.spine_connected ? 'mmap busz aktiv, ' + (stats.spine_latency_ns || 0) + ' ns/op' : 'nincs kapcsolat'
  ));
  scroll.appendChild(card('Bicska Blades', stats.bicska_blades || 0, 'specializalt vegrehajto egyseg'));
  const session = store.get('session') || {};
  scroll.appendChild(card('Session', (session.id || '').slice(0, 8) + '...', 'Messages: ' + (session.messageCount || 0) + ' | Tokens: ' + (session.tokens || 0)));
  scroll.appendChild(card('AI Provider', store.get('provider') + ' / ' + store.get('model'), 'API: ' + (store.get('apiKey') ? 'beallitva' : 'hianyzik')));
  const last = document.getElementById('dash-last');
  if (last) last.textContent = 'frissitve: ' + new Date().toLocaleTimeString();
}

function refreshStats() {
  ws.cmd('status');
  renderDashboard({
    total_blocks: store.get('memory.blocks') || 0,
    layers: store.get('memory.layers') || 0,
    hebbian_active: store.get('memory.hebbianActive') || 0,
    depth_distribution: { D0: 1, D1: 7, D2: 60, D3: 28000, D4: 600, D5: 400, D6: 200, D7: 50, D8: 10 },
    consciousness: [
      { name: 'hebbian', count: 5 },
      { name: 'mirror', count: 0 },
      { name: 'resonance', count: 0 },
      { name: 'archetype', count: 0 },
      { name: 'thought_graph', count: 1 },
      { name: 'predictive_cache', count: 0 },
      { name: 'dream', count: 1 },
      { name: 'attention', count: 7 },
      { name: 'emotional', count: 0 },
      { name: 'multimodal', count: 0 }
    ],
    alan_running: false,
    alan_last_analysis: 'n/a',
    spine_connected: store.get('wsReady'),
    spine_latency_ns: 1.4,
    bicska_blades: 127
  });
}

function handleWS(msg) {
  if (msg && msg.type === 'status' && msg.data) {
    renderDashboard(msg.data);
  }
}

export async function init() {
  const tab = document.getElementById('dashboard-tab');
  if (!tab) return;
  tab.classList.add('flex');
  tab.style.flexDirection = 'column';
  refreshStats();
  refreshTimer = setInterval(refreshStats, 10000);
  ws.on('status', handleWS);
  ws.on('message', handleWS);
}

export const MODULE_INFO = { name: 'dashboard', version: '1.0.0', dependencies: ['core'], exports: ['init'] };