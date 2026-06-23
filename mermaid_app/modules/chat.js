// modules/chat.js — Rongyász Agent Chat
// Riboszoma modul: WebSocket chat UI, üzenetek, tool megjelenítés

import { store, ws } from './core.js';

let messagesEl = null;
let inputEl = null;
let sendBtn = null;
let micBtn = null;
let history = [];

function el(tag, opts, children) {
  const e = document.createElement(tag);
  if (opts) {
    if (opts.cls) e.className = opts.cls;
    if (opts.html !== undefined) e.innerHTML = opts.html;
    if (opts.text !== undefined) e.textContent = opts.text;
    if (opts.attrs) for (const k in opts.attrs) e.setAttribute(k, opts.attrs[k]);
  }
  if (children) for (const c of children) e.appendChild(c);
  return e;
}

function escapeHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function renderMarkdown(text) {
  let h = escapeHtml(text);
  h = h.replace(/\\(\w*)\n([\s\S]*?)\\/g, '<pre><code></code></pre>');
  h = h.replace(/\([^\]+)\/g, '<code></code>');
  h = h.replace(/\*\*([^\*]+)\*\*/g, '<strong></strong>');
  h = h.replace(/\*([^\*]+)\*/g, '<em></em>');
  h = h.replace(/\n/g, '<br>');
  return h;
}

function ts() {
  const d = new Date();
  return d.getHours().toString().padStart(2, '0') + ':' +
         d.getMinutes().toString().padStart(2, '0') + ':' +
         d.getSeconds().toString().padStart(2, '0');
}

function addMessage(role, text, opts) {
  opts = opts || {};
  if (!messagesEl) return;
  const cls = 'msg ' + role;
  const msg = el('div', { cls });
  const roleLbl = el('div', { cls: 'role', text: role.toUpperCase() });
  msg.appendChild(roleLbl);
  const body = el('div');
  if (role === 'tool' || role === 'error') {
    body.style.whiteSpace = 'pre-wrap';
    body.textContent = text;
  } else {
    body.innerHTML = renderMarkdown(text);
  }
  msg.appendChild(body);
  if (!opts.hideTs) {
    const time = el('div', { cls: 'ts', text: ts() });
    msg.appendChild(time);
  }
  messagesEl.appendChild(msg);
  messagesEl.scrollTop = messagesEl.scrollHeight;
  return msg;
}

function addThinking() {
  if (!messagesEl) return null;
  const msg = el('div', { cls: 'msg thinking' });
  const spinner = el('span', { cls: 'spinner', text: '⠋' });
  const txt = el('span', { text: 'gondolkodom...' });
  msg.appendChild(spinner);
  msg.appendChild(txt);
  messagesEl.appendChild(msg);
  messagesEl.scrollTop = messagesEl.scrollHeight;
  return msg;
}

function removeThinking(node) {
  if (node && node.parentNode) node.parentNode.removeChild(node);
}

async function detectAndRun(text) {
  const rongyasz = await import('./rongyasz.js');
  const skills = await import('./skills.js');

  const trimmed = text.trim();

  // /command shortcut
  if (trimmed.startsWith('/')) {
    const parts = trimmed.split(/\s+/);
    const cmd = parts[0];
    const arg = parts.slice(1).join(' ');

    if (cmd === '/status') {
      addMessage('system', 'Session: ' + store.get('session').id.slice(0, 8) + '... | Messages: ' + store.get('session').messageCount + ' | Provider: ' + store.get('provider') + ' | Model: ' + store.get('model'));
      return;
    }
    if (cmd === '/recall' || cmd === '/find' || cmd === '/remember' || cmd === '/look') {
      ws.cmd('microscope', { command: cmd.slice(1), args: arg });
      addMessage('system', '> ' + cmd + ' ' + arg);
      return;
    }
    if (cmd === '/hebbian' || cmd === '/mirror' || cmd === '/archetypes' || cmd === '/patterns' || cmd === '/dream' || cmd === '/doctor') {
      ws.cmd('microscope', { command: cmd.slice(1) });
      addMessage('system', '> ' + cmd);
      return;
    }
  }

  // Skill kategória detektálás
  const cat = skills.detectCategory(trimmed);
  if (cat) {
    const info = skills.getCategoryInfo(cat);
    addMessage('tool', '🎯 Skill kategória észlelve: ' + info.label + ' (' + info.count + ' skill)');
  }

  // Agent válasz kérése
  ws.ask(trimmed, { history: history.slice(-10) });
  const thinking = addThinking();
  store.set('_chat.thinkingNode', thinking);
}

function send() {
  if (!inputEl) return;
  const text = inputEl.value.trim();
  if (!text) return;
  addMessage('user', text);
  history.push({ role: 'user', content: text });
  inputEl.value = '';
  store.set('session.messageCount', store.get('session').messageCount + 1);
  detectAndRun(text);
}

function handleWSEvent(msg) {
  if (!msg) return;
  const thinking = store.get('_chat.thinkingNode');

  if (msg.type === 'chunk' || msg.type === 'delta') {
    if (thinking) removeThinking(thinking);
    store.set('_chat.thinkingNode', null);
    if (msg.text) {
      // Egyszerű streaming: az utolsó agent üzenethez fűzzük, vagy újat nyitunk
      const last = messagesEl.lastElementChild;
      if (last && last.classList.contains('agent') && last.dataset.streaming === '1') {
        const body = last.children[1];
        body.innerHTML = renderMarkdown(msg.text);
      } else {
        const m = addMessage('agent', msg.text, { hideTs: true });
        if (m) m.dataset.streaming = '1';
      }
    }
    return;
  }

  if (msg.type === 'done' || msg.type === 'complete') {
    if (thinking) removeThinking(thinking);
    store.set('_chat.thinkingNode', null);
    const last = messagesEl.lastElementChild;
    if (last && last.dataset.streaming === '1') {
      delete last.dataset.streaming;
      const ts = el('div', { cls: 'ts', text: ts() });
      last.appendChild(ts);
    }
    if (msg.text) {
      history.push({ role: 'assistant', content: msg.text });
    }
    return;
  }

  if (msg.type === 'tool' || msg.type === 'tool_call') {
    if (thinking) removeThinking(thinking);
    store.set('_chat.thinkingNode', null);
    addMessage('tool', '🔧 ' + (msg.name || 'tool') + (msg.args ? '\n' + JSON.stringify(msg.args, null, 2) : '') + (msg.result ? '\n→ ' + (typeof msg.result === 'string' ? msg.result : JSON.stringify(msg.result)) : ''));
    return;
  }

  if (msg.type === 'error') {
    if (thinking) removeThinking(thinking);
    store.set('_chat.thinkingNode', null);
    addMessage('error', msg.message || msg.text || 'Ismeretlen hiba');
    return;
  }

  if (msg.type === 'message' && msg.text) {
    if (thinking) removeThinking(thinking);
    store.set('_chat.thinkingNode', null);
    addMessage('agent', msg.text);
    history.push({ role: 'assistant', content: msg.text });
  }
}

export async function init() {
  messagesEl = document.getElementById('chat-messages');
  inputEl = document.getElementById('user-input');
  sendBtn = document.getElementById('send-btn');
  micBtn = document.getElementById('voice-trigger');

  if (sendBtn) sendBtn.addEventListener('click', send);
  if (inputEl) {
    inputEl.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        send();
      }
    });
  }
  if (micBtn) {
    micBtn.addEventListener('click', async () => {
      const voice = await import('./voice.js');
      await voice.toggleMic();
    });
  }

  // Üdvözlés
  const rongyasz = await import('./rongyasz.js');
  setTimeout(() => {
    addMessage('system', 'Rongyász v2.0 — Matrjoska Fraktál aktív');
    addMessage('agent', rongyasz.getRongyaszGreeting());
  }, 100);

  // WS események
  ws.on('chunk', handleWSEvent);
  ws.on('done', handleWSEvent);
  ws.on('tool', handleWSEvent);
  ws.on('error', handleWSEvent);
  ws.on('message', handleWSEvent);
}

export const MODULE_INFO = {
  name: 'chat',
  version: '1.0.0',
  dependencies: ['core', 'rongyasz', 'skills', 'voice'],
  exports: ['init']
};
