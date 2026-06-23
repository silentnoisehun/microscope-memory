// modules/voice.js — Web Speech API (STT + TTS) + VU meter
// Riboszoma modul: hang input/output a Rongyász agent számára

import { store, ws } from './core.js';

let recognition = null;
let synth = window.speechSynthesis;
let audioCtx = null;
let analyser = null;
let stream = null;
let rafId = null;
let isRecording = false;
let isTTSEnabled = true;
let voices = [];

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

async function ensureRecognition() {
  if (recognition) return recognition;
  const SR = window.SpeechRecognition || window.webkitSpeechRecognition;
  if (!SR) {
    addStatus('A böngésző nem támogatja a Speech Recognition API-t.');
    return null;
  }
  recognition = new SR();
  recognition.lang = 'hu-HU';
  recognition.continuous = false;
  recognition.interimResults = true;

  recognition.onresult = (e) => {
    let txt = '';
    for (let i = e.resultIndex; i < e.results.length; i++) {
      txt += e.results[i][0].transcript;
    }
    const input = document.getElementById('chat-input');
    if (input) input.value = txt;
  };
  recognition.onerror = (e) => {
    addStatus('STT hiba: ' + e.error);
    stopMic();
  };
  recognition.onend = () => {
    stopMic();
  };
  return recognition;
}

async function startMic() {
  const rec = await ensureRecognition();
  if (!rec) return;
  try {
    rec.start();
    isRecording = true;
    updateMicBtn();
    addStatus('Mikrofon aktív — beszélj...');
    await startVU();
  } catch (e) {
    addStatus('Mic indítás sikertelen: ' + e.message);
  }
}

function stopMic() {
  if (recognition && isRecording) {
    try { recognition.stop(); } catch (e) {}
  }
  isRecording = false;
  updateMicBtn();
  stopVU();
  addStatus('Mikrofon leállítva');
}

export async function toggleMic() {
  if (isRecording) stopMic();
  else await startMic();
}

function updateMicBtn() {
  const btn = document.getElementById('chat-mic');
  if (btn) {
    btn.classList.toggle('recording', isRecording);
    btn.textContent = isRecording ? '⏹ Stop' : '🎤 Mic';
  }
}

async function startVU() {
  try {
    stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    audioCtx = new (window.AudioContext || window.webkitAudioContext)();
    const src = audioCtx.createMediaStreamSource(stream);
    analyser = audioCtx.createAnalyser();
    analyser.fftSize = 256;
    src.connect(analyser);
    const data = new Uint8Array(analyser.frequencyBinCount);
    const loop = () => {
      if (!isRecording) return;
      analyser.getByteFrequencyData(data);
      let sum = 0;
      for (let i = 0; i < data.length; i++) sum += data[i];
      const rms = sum / data.length / 255;
      // Frissítjük a voice-panel VU-t, ha van
      const bars = document.querySelectorAll('#vu-container .vu-bar');
      bars.forEach((b, i) => {
        const intensity = Math.max(0, Math.min(1, rms + (Math.random() - 0.5) * 0.2));
        b.style.height = (intensity * 100) + '%';
        if (intensity > 0.7) b.classList.add('peak');
        else b.classList.remove('peak');
      });
      rafId = requestAnimationFrame(loop);
    };
    loop();
  } catch (e) {
    addStatus('VU hiba: ' + e.message);
  }
}

function stopVU() {
  if (rafId) cancelAnimationFrame(rafId);
  if (stream) stream.getTracks().forEach((t) => t.stop());
  if (audioCtx) audioCtx.close();
  audioCtx = null;
  analyser = null;
  stream = null;
}

export function speak(text) {
  if (!isTTSEnabled || !synth) return;
  const utter = new SpeechSynthesisUtterance(text);
  utter.lang = 'hu-HU';
  utter.rate = 1.0;
  utter.pitch = 1.0;
  // Voice választás (ha elérhető)
  const voiceName = store.get('voice');
  if (voiceName && voices.length) {
    const v = voices.find((v) => v.name.indexOf(voiceName) >= 0 || v.lang.indexOf('hu') >= 0);
    if (v) utter.voice = v;
  }
  synth.cancel();
  synth.speak(utter);
}

function addStatus(text) {
  const el = document.getElementById('voice-status');
  if (el) el.textContent = text;
}

function loadVoices() {
  voices = synth.getVoices();
  const sel = document.getElementById('voice-select');
  if (sel) {
    sel.innerHTML = '';
    voices.forEach((v) => {
      const o = document.createElement('option');
      o.value = v.name;
      o.textContent = v.name + ' (' + v.lang + ')';
      sel.appendChild(o);
    });
  }
}

export async function init() {
  // Voice panel megjelenítés a voice tab tartalmában
  const tab = document.getElementById('voice-tab');
  if (!tab) return;
  tab.innerHTML = '';
  tab.classList.add('flex');
  tab.style.flexDirection = 'column';

  const panel = el('div', { cls: 'flex-1' });
  panel.style.padding = '16px';
  panel.style.overflowY = 'auto';

  // VU szekció
  const vuSection = el('div', { cls: 'voice-section' });
  vuSection.appendChild(el('h3', { text: '🎙 Mikrofon VU Meter' }));
  const vuContainer = el('div', { id: 'vu-container' });
  for (let i = 0; i < 32; i++) {
    vuContainer.appendChild(el('div', { cls: 'vu-bar' }));
  }
  vuSection.appendChild(vuContainer);
  vuSection.appendChild(el('div', { id: 'voice-status', cls: 'voice-status', text: 'Készen áll' }));
  panel.appendChild(vuSection);

  // TTS szekció
  const ttsSection = el('div', { cls: 'voice-section' });
  ttsSection.appendChild(el('h3', { text: '🔊 Text-to-Speech' }));
  const ttsRow = el('div', { cls: 'voice-row' });
  ttsRow.appendChild(el('label', { text: 'Hang:' }));
  const voiceSel = el('select', { id: 'voice-select' });
  ttsRow.appendChild(voiceSel);
  ttsSection.appendChild(ttsRow);
  const ttsRow2 = el('div', { cls: 'voice-row' });
  const narrateBtn = el('button', { cls: 'voice-btn active', text: '🔊 Narrate ON' });
  narrateBtn.addEventListener('click', () => {
    isTTSEnabled = !isTTSEnabled;
    narrateBtn.textContent = isTTSEnabled ? '🔊 Narrate ON' : '🔇 Narrate OFF';
    narrateBtn.classList.toggle('active', isTTSEnabled);
  });
  ttsRow2.appendChild(narrateBtn);
  const testBtn = el('button', { cls: 'voice-btn', text: '▶ Teszt' });
  testBtn.addEventListener('click', () => speak('Helló, Rongyász vagyok. Rezonancia kész.'));
  ttsRow2.appendChild(testBtn);
  ttsSection.appendChild(ttsRow2);
  panel.appendChild(ttsSection);

  // STT szekció
  const sttSection = el('div', { cls: 'voice-section' });
  sttSection.appendChild(el('h3', { text: '🎤 Speech-to-Text' }));
  const sttRow = el('div', { cls: 'voice-row' });
  const sttBtn = el('button', { cls: 'voice-btn', text: '🎤 Indít' });
  sttBtn.id = 'voice-stt-btn';
  sttBtn.addEventListener('click', () => toggleMic());
  sttRow.appendChild(sttBtn);
  sttSection.appendChild(sttRow);
  sttSection.appendChild(el('div', { cls: 'voice-status', text: 'Web Speech API (magyar) — böngésző natív' }));
  panel.appendChild(sttSection);

  tab.appendChild(panel);

  // Voices betöltése
  if (synth) {
    loadVoices();
    synth.onvoiceschanged = loadVoices;
  }
}

export const MODULE_INFO = {
  name: 'voice',
  version: '1.0.0',
  dependencies: ['core'],
  exports: ['init', 'speak', 'toggleMic']
};
