// modules/settings.js -- Beallitasok (provider, model, API key, voice)
// Riboszoma modul: localStorage persist, provider lista, voice valaszto

import { store } from './core.js';

const LS_KEY = 'mermaid-settings-v1';

const PROVIDERS = [
  { id: 'claude', label: 'Claude (Anthropic)', models: ['claude-sonnet-4-5', 'claude-opus-4-1', 'claude-haiku-4'] },
  { id: 'ora-core', label: 'Ora Neural Core V4.2 (microscope-mcp-v3)', models: ['ora-auto', 'gemini-cli', 'groq', 'nvidia-nim'] },
  { id: 'pipeline-local', label: 'Pipeline Server (local, no API key)', models: ['pipeline-auto'] },
  { id: 'rongy-mcp', label: 'Rongyász MCP (stdio bridge)', models: ['rongy-auto', 'rongy-delegate'] },
  { id: 'openai', label: 'OpenAI', models: ['gpt-4o', 'gpt-4o-mini', 'o1', 'o1-mini'] },
  { id: 'ollama', label: 'Ollama (local)', models: ['qwen2.5:7b', 'llama3.2:3b', 'phi3:3.8b', 'mistral:7b'] },
  { id: 'gemini', label: 'Google Gemini', models: ['gemini-2.5-pro', 'gemini-2.5-flash', 'gemini-2.0-flash'] },
  { id: 'nvidia', label: 'NVIDIA NIM', models: ['meta/llama-3.3-70b', 'qwen2.5-7b', 'mistral-large'] }
];

function el(tag, opts, children) {
  const e = document.createElement(tag);
  if (opts) {
    if (opts.cls) e.className = opts.cls;
    if (opts.text !== undefined) e.textContent = opts.text;
    if (opts.attrs) for (const k in opts.attrs) e.setAttribute(k, opts.attrs[k]);
  }
  if (children) for (const c of children) e.appendChild(c);
  return e;
}

function loadSettings() {
  try {
    const saved = JSON.parse(localStorage.getItem(LS_KEY) || '{}');
    if (saved.server) store.set('server', saved.server);
    if (saved.provider) store.set('provider', saved.provider);
    if (saved.model) store.set('model', saved.model);
    if (saved.apiKey) store.set('apiKey', saved.apiKey);
    if (typeof saved.reasoning === 'boolean') store.set('reasoning', saved.reasoning);
    if (saved.voice) store.set('voice', saved.voice);
    if (typeof saved.narrate === 'boolean') store.set('narrate', saved.narrate);
  } catch (e) { console.warn('Settings load failed', e); }
}

function saveSettings() {
  const data = {
    server: store.get('server'),
    provider: store.get('provider'),
    model: store.get('model'),
    apiKey: store.get('apiKey'),
    reasoning: store.get('reasoning'),
    voice: store.get('voice'),
    narrate: store.get('narrate')
  };
  try { localStorage.setItem(LS_KEY, JSON.stringify(data)); } catch (e) {}
}

function row(label, input) {
  const r = el('div', { cls: 'settings-row' });
  r.appendChild(el('label', { text: label }));
  r.appendChild(input);
  return r;
}

export async function init() {
  loadSettings();

  const tab = document.getElementById('settings-tab');
  if (!tab) return;
  tab.innerHTML = '';

  const serverGroup = el('div', { cls: 'settings-group' });
  serverGroup.appendChild(el('h3', { text: 'Szerver' }));
  const serverInput = el('input', { attrs: { type: 'text', value: store.get('server') || '' } });
  serverInput.addEventListener('input', function (e) { store.set('server', e.target.value); });
  serverGroup.appendChild(row('WebSocket URL', serverInput));
  tab.appendChild(serverGroup);

  const providerGroup = el('div', { cls: 'settings-group' });
  providerGroup.appendChild(el('h3', { text: 'AI Provider' }));
  const providerSel = el('select');
  PROVIDERS.forEach(function (p) {
    const opt = el('option', { text: p.label, attrs: { value: p.id } });
    if (p.id === store.get('provider')) opt.setAttribute('selected', 'selected');
    providerSel.appendChild(opt);
  });
  let modelSel = el('select');
  providerSel.addEventListener('change', function (e) {
    store.set('provider', e.target.value);
    const p = PROVIDERS.find(function (x) { return x.id === e.target.value; });
    if (p && p.models.length) {
      modelSel.innerHTML = '';
      p.models.forEach(function (m) {
        const o = el('option', { text: m, attrs: { value: m } });
        modelSel.appendChild(o);
      });
      store.set('model', p.models[0]);
    }
  });
  providerGroup.appendChild(row('Provider', providerSel));

  const currentProv = PROVIDERS.find(function (p) { return p.id === store.get('provider'); }) || PROVIDERS[0];
  currentProv.models.forEach(function (m) {
    const o = el('option', { text: m, attrs: { value: m } });
    if (m === store.get('model')) o.setAttribute('selected', 'selected');
    modelSel.appendChild(o);
  });
  modelSel.addEventListener('change', function (e) { store.set('model', e.target.value); });
  providerGroup.appendChild(row('Model', modelSel));

  const apiKeyInput = el('input', { attrs: { type: 'password', value: store.get('apiKey') || '', placeholder: 'sk-ant-...' } });
  apiKeyInput.addEventListener('input', function (e) { store.set('apiKey', e.target.value); });
  providerGroup.appendChild(row('API Key', apiKeyInput));

  const reasoningChk = el('input', { attrs: { type: 'checkbox' } });
  if (store.get('reasoning')) reasoningChk.setAttribute('checked', 'checked');
  reasoningChk.addEventListener('change', function (e) { store.set('reasoning', e.target.checked); });
  providerGroup.appendChild(row('Reasoning', reasoningChk));

  tab.appendChild(providerGroup);

  const voiceGroup = el('div', { cls: 'settings-group' });
  voiceGroup.appendChild(el('h3', { text: 'Voice' }));
  const voiceInput = el('input', { attrs: { type: 'text', value: store.get('voice') || 'Noemi' } });
  voiceInput.addEventListener('input', function (e) { store.set('voice', e.target.value); });
  voiceGroup.appendChild(row('Voice neve', voiceInput));
  const narrateChk = el('input', { attrs: { type: 'checkbox' } });
  if (store.get('narrate')) narrateChk.setAttribute('checked', 'checked');
  narrateChk.addEventListener('change', function (e) { store.set('narrate', e.target.checked); });
  voiceGroup.appendChild(row('Automatikus narracio', narrateChk));
  tab.appendChild(voiceGroup);

  const saveBtn = el('button', { cls: 'settings-save', text: 'Mentes' });
  saveBtn.addEventListener('click', function () {
    saveSettings();
    saveBtn.textContent = 'Mentve!';
    setTimeout(function () { saveBtn.textContent = 'Mentes'; }, 1500);
  });
  tab.appendChild(saveBtn);
}

export const MODULE_INFO = { name: 'settings', version: '1.0.0', dependencies: ['core'], exports: ['init'] };