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
  
  // Instead of replacing everything, let's just fill the existing grid if possible
  // or clear and rebuild if it's easier. Let's rebuild to keep it clean.
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

  // Card 1: Memory
  const memCard = el('div', { cls: 'dash-card', attrs: { id: 'card-memory' } });
  memCard.appendChild(el('h3', { text: 'Microscope Memory' }));
  memCard.appendChild(el('div', { cls: 'metric-val', text: (stats.total_blocks || 0) + ' Blocks' }));
  memCard.appendChild(el('div', { cls: 'metric-sub', text: 'D0-D8 Active' }));
  scroll.appendChild(memCard);

  // Card 2: Alan
  const alanCard = el('div', { cls: 'dash-card', attrs: { id: 'card-alan' } });
  alanCard.appendChild(el('h3', { text: 'ALAN Audit' }));
  alanCard.appendChild(el('div', { cls: 'metric-val', text: stats.alan_running ? 'Running' : 'Idle' }));
  alanCard.appendChild(el('div', { cls: 'metric-sub', text: stats.alan_last_analysis || '0 Issues Found' }));
  scroll.appendChild(alanCard);

  // Card 3: Spine
  const spineCard = el('div', { cls: 'dash-card', attrs: { id: 'card-spine' } });
  spineCard.appendChild(el('h3', { text: 'Spine Bus' }));
  spineCard.appendChild(el('div', { cls: 'metric-val', text: stats.spine_connected ? 'Connected' : 'Disconnected' }));
  spineCard.appendChild(el('div', { cls: 'metric-sub', text: (stats.spine_latency_ns || 0) + ' ns/op' }));
  scroll.appendChild(spineCard);

  // Add more detailed cards if needed
  if (stats.depth_distribution) {
    scroll.appendChild(barCard('Depth Distribution (D0-D8)',
      Object.entries(stats.depth_distribution).map(function (e) { return { label: e[0], value: e[1] }; })
    ));
  }

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