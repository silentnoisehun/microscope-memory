// modules/core.js — Mermaid Core Shell
class StateStore extends EventTarget {
  constructor() {
    super();
    this.state = {
      ws: null,
      wsReady: false,
      server: 'ws://localhost:8080/ws',
      provider: 'claude',
      model: 'claude-sonnet-4-5',
      apiKey: '',
      reasoning: false,
      voice: 'Noemi',
      narrate: true,
      session: { id: crypto.randomUUID(), startedAt: Date.now(), messageCount: 0, tokens: 0 },
      memory: { blocks: 0, layers: 0, hebbianActive: 0 },
      pipeline: { active: false, currentStep: null }
    };
    this.modules = new Map();
  }
  get(path) { return path.split('.').reduce((o, k) => (o ? o[k] : undefined), this.state); }
  set(path, value) {
    const keys = path.split('.');
    const last = keys.pop();
    const target = keys.reduce((o, k) => { o[k] = o[k] || {}; return o[k]; }, this.state);
    const old = target[last];
    target[last] = value;
    this.dispatchEvent(new CustomEvent('change', { detail: { path, value, old } }));
  }
  subscribe(path, callback) {
    const handler = (e) => {
      if (e.detail.path === path || e.detail.path.indexOf(path + '.') === 0) {
        callback(e.detail.value, e.detail.old);
      }
    };
    this.addEventListener('change', handler);
    return () => this.removeEventListener('change', handler);
  }
  async loadModule(name) {
    if (this.modules.has(name)) return this.modules.get(name);
    try {
      const mod = await import('./' + name + '.js');
      this.modules.set(name, mod);
      return mod;
    } catch (e) { console.error('Failed to load module', name, e); return null; }
  }
  async reloadModule(name) {
    this.modules.delete(name);
    return await this.loadModule(name);
  }
}

export const store = new StateStore();

class WSClient {
  constructor() {
    this.ws = null;
    this.reconnectDelay = 1000;
    this.maxReconnectDelay = 30000;
    this.handlers = new Map();
    this.connect();
  }
  connect() {
    const url = store.get('server');
    try {
      this.ws = new WebSocket(url);
      this.ws.onopen = () => { this.reconnectDelay = 1000; store.set('wsReady', true); this.emit('open'); };
      this.ws.onmessage = (e) => {
        try { const msg = JSON.parse(e.data); this.emit(msg.type || 'message', msg); }
        catch (err) { this.emit('raw', e.data); }
      };
      this.ws.onerror = () => this.emit('error');
      this.ws.onclose = () => {
        store.set('wsReady', false);
        this.emit('close');
        setTimeout(() => this.connect(), this.reconnectDelay);
        this.reconnectDelay = Math.min(this.reconnectDelay * 2, this.maxReconnectDelay);
      };
      store.set('ws', this.ws);
    } catch (e) {
      console.error('WS connect failed', e);
      setTimeout(() => this.connect(), this.reconnectDelay);
      this.reconnectDelay = Math.min(this.reconnectDelay * 2, this.maxReconnectDelay);
    }
  }
  on(type, handler) {
    if (!this.handlers.has(type)) this.handlers.set(type, []);
    this.handlers.get(type).push(handler);
    return () => {
      const arr = this.handlers.get(type) || [];
      const i = arr.indexOf(handler);
      if (i >= 0) arr.splice(i, 1);
    };
  }
  emit(type, data) {
    const arr = this.handlers.get(type) || [];
    arr.forEach((h) => { try { h(data); } catch (e) { console.error(e); } });
  }
  send(obj) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(obj));
      return true;
    }
    return false;
  }
  ask(text, options) {
    options = options || {};
    return this.send({
      type: 'ask', text,
      provider: store.get('provider'),
      model: store.get('model'),
      api_key: store.get('apiKey'),
      reasoning: store.get('reasoning'),
      session: store.get('session').id
    });
  }
  cmd(cmd, args) {
    args = args || {};
    return this.send({ type: 'cmd', cmd, ...args });
  }
}

export const ws = new WSClient();

function startMiniVU() {
  const container = document.getElementById('vu-mini');
  if (!container) return;
  for (let i = 0; i < 20; i++) {
    const b = document.createElement('div');
    b.className = 'vu-mini-bar';
    container.appendChild(b);
  }
  let phase = 0;
  setInterval(() => {
    const bars = container.children;
    for (let i = 0; i < bars.length; i++) {
      const noise = Math.sin(phase * 0.3 + i * 0.7) * 0.5 + 0.5;
      const h = Math.max(2, noise * 14);
      bars[i].style.height = h + 'px';
      const intensity = noise;
      if (intensity > 0.7) bars[i].style.background = 'var(--warn)';
      else if (intensity > 0.4) bars[i].style.background = 'var(--accent)';
      else bars[i].style.background = 'var(--border)';
    }
    phase++;
  }, 80);
}

function setupConnectionIndicator() {
  const el = document.getElementById('connection-state');
  if (!el) return;
  ws.on('open', () => el.classList.add('online'));
  ws.on('close', () => el.classList.remove('online'));
  ws.on('error', () => el.classList.remove('online'));
}

export function activateTab(name) {
  document.querySelectorAll('.tab-btn').forEach((b) => {
    b.classList.toggle('active', b.dataset.tab === name);
  });
  document.querySelectorAll('.tab-content').forEach((c) => {
    c.classList.toggle('active', c.id === name + '-tab');
  });
  store.set('currentTab', name);
  store.dispatchEvent(new CustomEvent('tabActivated', { detail: { tab: name } }));
}

async function hotSwapModule(name) {
  const mod = await store.reloadModule(name);
  store.dispatchEvent(new CustomEvent('moduleSwapped', { detail: { name, mod } }));
  console.log('[hot-swap]', name, 'reloaded');
  return mod;
}

function updateStatusText() {
  const el = document.getElementById('status-text');
  if (!el) return;
  const ready = store.get('wsReady');
  el.textContent = ready ? 'online' : 'offline';
  el.style.color = ready ? 'var(--ok)' : 'var(--err)';
}

export async function initMermaid() {
  document.querySelectorAll('.tab-btn').forEach((b) => {
    b.addEventListener('click', () => activateTab(b.dataset.tab));
  });

  const moduleMap = { chat: 'chat', voice: 'voice', diff: 'diff', dashboard: 'dashboard', settings: 'settings' };
  const loadedModules = new Set();

  store.addEventListener('tabActivated', async (e) => {
    const tab = e.detail.tab;
    const modName = moduleMap[tab];
    if (modName && !loadedModules.has(modName)) {
      const mod = await store.loadModule(modName);
      if (mod && typeof mod.init === 'function') {
        await mod.init();
        loadedModules.add(modName);
      }
    }
  });

  if ('serviceWorker' in navigator) {
    try { await navigator.serviceWorker.register('./sw.js'); console.log('[sw] registered'); }
    catch (e) { console.warn('[sw] registration failed', e); }
  }

  setupConnectionIndicator();
  store.subscribe('wsReady', updateStatusText);
  updateStatusText();
  startMiniVU();
  activateTab('chat');

  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.addEventListener('message', (e) => {
      if (e.data && e.data.type === 'module-updated' && e.data.module) {
        hotSwapModule(e.data.module);
      }
    });
  }
}

export const MODULE_INFO = { name: 'core', version: '1.0.0', dependencies: [], exports: ['store', 'ws', 'activateTab', 'initMermaid'] };
