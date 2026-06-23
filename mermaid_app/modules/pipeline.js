// modules/pipeline.js -- Pipeline Architect v7.0 integracio
// Riboszoma modul: PLAN -> EXECUTE -> WRITE kodsor vezerlo

import { store, ws } from './core.js';

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

let currentPlan = null;

function renderPlan(plan) {
  const tab = document.getElementById('pipeline-tab');
  if (!tab) return;
  const out = document.getElementById('pipeline-output');
  if (!out) return;
  out.innerHTML = '';
  if (!plan) {
    out.appendChild(el('div', { cls: 'voice-status', text: 'Nincs aktiv terv. Adj utasitast a chat-ben (pl. tervezd meg, hogy...)' }));
    return;
  }
  out.appendChild(el('h3', { text: 'Aktiv terv: ' + (plan.title || 'terv') }));
  if (plan.description) out.appendChild(el('div', { cls: 'voice-status', text: plan.description }));
  if (plan.steps) {
    plan.steps.forEach(function (step, i) {
      const stepEl = el('div', { cls: 'dash-card' });
      stepEl.style.marginBottom = '8px';
      const status = step.status || 'pending';
      stepEl.appendChild(el('h3', { text: 'Lepes ' + (i + 1) + ': ' + (step.tool || step.name || 'muvelet') }));
      const sCls = status === 'done' ? 'ok' : status === 'running' ? 'warn' : 'err';
      const s = el('span', { cls: 'dash-status ' + sCls, text: status.toUpperCase() });
      stepEl.appendChild(s);
      if (step.target) stepEl.appendChild(el('div', { cls: 'sub', text: 'target: ' + step.target }));
      if (step.description) stepEl.appendChild(el('div', { cls: 'sub', text: step.description }));
      if (step.code) {
        const pre = el('pre', { text: step.code });
        pre.style.cssText = 'background:var(--bg);border:1px solid var(--border);border-radius:4px;padding:8px;overflow-x:auto;margin-top:6px;font-size:11px;color:var(--accent-2);';
        stepEl.appendChild(pre);
      }
      out.appendChild(stepEl);
    });
  }
}

function handleWS(msg) {
  if (!msg) return;
  if (msg.type === 'pipeline' || msg.type === 'plan') {
    currentPlan = msg.data || msg.plan;
    renderPlan(currentPlan);
  }
  if (msg.type === 'pipeline_step' && currentPlan && currentPlan.steps) {
    const i = msg.step_index;
    if (currentPlan.steps[i]) {
      currentPlan.steps[i].status = msg.status || 'done';
      if (msg.result) currentPlan.steps[i].result = msg.result;
      renderPlan(currentPlan);
    }
  }
}

export async function planTask(description) {
  ws.cmd('pipeline_plan', { description: description, provider: store.get('provider'), model: store.get('model') });
  store.set('pipeline.active', true);
}

export async function executeStep(index) {
  ws.cmd('pipeline_execute', { step_index: index });
}

export async function init() {
  const tab = document.getElementById('pipeline-tab');
  if (!tab) return;
  tab.classList.add('flex');
  tab.style.flexDirection = 'column';
  tab.style.padding = '16px';
  tab.style.overflowY = 'auto';
  const header = el('div');
  header.style.marginBottom = '12px';
  header.appendChild(el('h3', { text: 'Pipeline Architect v7.0' }));
  header.appendChild(el('div', { cls: 'voice-status', text: 'PLAN -> EXECUTE -> WRITE - kodsor automatizmus Claude/OpenAI API-val' }));
  tab.appendChild(header);
  const out = el('div', { id: 'pipeline-output' });
  tab.appendChild(out);
  renderPlan(null);
  ws.on('message', handleWS);
  ws.on('pipeline', handleWS);
  ws.on('plan', handleWS);
  ws.on('pipeline_step', handleWS);
}

export const MODULE_INFO = { name: 'pipeline', version: '1.0.0', dependencies: ['core'], exports: ['init', 'planTask', 'executeStep'] };