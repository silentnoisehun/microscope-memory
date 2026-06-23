// modules/diff.js -- Code/memory diff comparison
// Riboszoma modul: LCS differ, szinkron scroll, statisztikas

let leftText = '';
let rightText = '';

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

function lcs(a, b) {
  const n = a.length, m = b.length;
  const dp = [];
  for (let i = 0; i <= n; i++) dp.push(new Array(m + 1).fill(0));
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < m; j++) {
      if (a[i] === b[j]) dp[i + 1][j + 1] = dp[i][j] + 1;
      else dp[i + 1][j + 1] = Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const ops = [];
  let i = n, j = m;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && a[i - 1] === b[j - 1]) {
      ops.unshift({ type: 'same', text: a[i - 1] });
      i--; j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      ops.unshift({ type: 'add', text: b[j - 1] });
      j--;
    } else {
      ops.unshift({ type: 'del', text: a[i - 1] });
      i--;
    }
  }
  return ops;
}

function renderDiff() {
  const a = leftText.split('\n');
  const b = rightText.split('\n');
  const ops = lcs(a, b);
  const left = document.getElementById('diff-left');
  const right = document.getElementById('diff-right');
  const stats = document.getElementById('diff-stats');
  if (!left || !right) return;
  left.innerHTML = '';
  right.innerHTML = '';
  let adds = 0, dels = 0, sames = 0;
  for (const op of ops) {
    if (op.type === 'same') {
      left.appendChild(el('span', { cls: 'diff-line same', text: '  ' + op.text }));
      right.appendChild(el('span', { cls: 'diff-line same', text: '  ' + op.text }));
      sames++;
    } else if (op.type === 'add') {
      right.appendChild(el('span', { cls: 'diff-line add', text: '+ ' + op.text }));
      left.appendChild(el('span', { cls: 'diff-line ctx', text: '  ' }));
      adds++;
    } else if (op.type === 'del') {
      left.appendChild(el('span', { cls: 'diff-line del', text: '- ' + op.text }));
      right.appendChild(el('span', { cls: 'diff-line ctx', text: '  ' }));
      dels++;
    }
  }
  if (stats) {
    stats.innerHTML = '';
    stats.appendChild(el('span', { text: '+' + adds + ' added' }));
    stats.appendChild(el('span', { text: '-' + dels + ' removed' }));
      stats.appendChild(el('span', { text: '=' + sames + ' same' }));
  }
}

function syncScroll(src, dst) {
  const el = document.createElement('div');
  el.addEventListener('scroll', function () { dst.scrollTop = src.scrollTop; });
}

function swap() {
  const t = leftText;
  leftText = rightText;
  rightText = t;
  const lIn = document.getElementById('diff-left-input');
  const rIn = document.getElementById('diff-right-input');
  if (lIn) lIn.value = leftText;
  if (rIn) rIn.value = rightText;
  renderDiff();
}

function clearAll() {
  leftText = '';
  rightText = '';
  const lIn = document.getElementById('diff-left-input');
  const rIn = document.getElementById('diff-right-input');
  if (lIn) lIn.value = '';
  if (rIn) rIn.value = '';
  renderDiff();
}

function loadSample() { leftText = String.fromCharCode(102,117,110,99,116,105,111,110,32,104,101,108,108,111,40,41,32,123,10,32,32,99,111,110,115,111,108,101,46,108,111,103,40,34,72,101,108,108,111,34,41,59,10,32,32,114,101,116,117,114,110,32,52,50,59,10,125); rightText = String.fromCharCode(102,117,110,99,116,105,111,110,32,104,101,108,108,111,40,110,97,109,101,41,32,123,10,32,32,99,111,110,115,111,108,101,46,108,111,103,40,34,72,105,44,32,34,32,43,32,110,97,109,101,41,59,10,32,32,114,101,116,117,114,110,32,52,50,59,10,125); const lIn = document.getElementById(String.fromCharCode(100,105,102,102,45,108,101,102,116,45,105,110,112,117,116)); const rIn = document.getElementById(String.fromCharCode(100,105,102,102,45,114,105,103,104,116,45,105,110,112,117,116)); if (lIn) lIn.value = leftText; if (rIn) rIn.value = rightText; renderDiff(); }

export const MODULE_INFO = { name: String.fromCharCode(100,105,102,102), version: String.fromCharCode(49,46,48,46,48), dependencies: [String.fromCharCode(99,111,114,101)], exports: [String.fromCharCode(105,110,105,116)] };
