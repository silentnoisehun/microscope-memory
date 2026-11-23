#!/usr/bin/env node
/**
 * microscope-hook.js — Universal AI Agent Memory Hook (Native CLI, Zero-MCP).
 *
 * Designed for Claude Code, MiniMax Code, Codex CLI, Goose, Antigravity, OMP.
 * Directly communicates with the native `microscope-mem` binary.
 *
 * Core Features:
 *  - Bulletproof JSON parsing with Windows backslash sanitization (prevents JSON escape syntax errors)
 *  - Dynamic multi-platform binary & config resolution (no hardcoded user paths)
 *  - Sequential mmap-safe phrase retrieval (eliminates Windows lock collisions)
 *  - Auto-context injection on session start and prompt submission
 *  - Automatic memory capture on tool results with dynamic importance classification
 *  - Detached background index rebuild (never blocks agent execution)
 *  - Fail-silent contract (always exits 0 to never break agent turn)
 */

import { spawn } from "node:child_process";
import {
  appendFileSync,
  existsSync,
  readFileSync,
  writeFileSync,
  mkdirSync,
} from "node:fs";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ---------------------------------------------------------------------------
// Configuration & Path Resolution
// ---------------------------------------------------------------------------

const HOME = process.env.USERPROFILE || process.env.HOME || "";
const LOG_DIR = process.env.MICROSCOPE_HOOK_DATA ||
  (process.env.MINIMAX_HOOKS_DATA || join(HOME, ".microscope", "hook_data"));
const TRACE_FILE = join(LOG_DIR, "microscope-hook.log");
const STATE_FILE = join(LOG_DIR, "microscope-hook-state.json");
const BRIDGE_CONFIG = join(LOG_DIR, "bridge-config.toml");

function resolveMicroscopeBin() {
  const candidates = [
    process.env.MICROSCOPE_BIN,
    process.env.MICROSCOPE_BRIDGE_BIN,
    HOME ? join(HOME, ".hope", "bin", process.platform === "win32" ? "microscope-mem.exe" : "microscope-mem") : null,
    resolve(__dirname, "..", "..", "target", "release", process.platform === "win32" ? "microscope-mem.exe" : "microscope-mem"),
    resolve(__dirname, "..", "..", "target", "debug", process.platform === "win32" ? "microscope-mem.exe" : "microscope-mem"),
    process.platform === "win32" ? "microscope-mem.exe" : "microscope-mem",
  ].filter(Boolean);

  for (const c of candidates) {
    if (existsSync(c)) return c;
  }
  return process.platform === "win32" ? "microscope-mem.exe" : "microscope-mem";
}

const BIN = resolveMicroscopeBin();

function resolveCanonicalConfig() {
  if (process.env.MICROSCOPE_CONFIG && existsSync(process.env.MICROSCOPE_CONFIG)) {
    return process.env.MICROSCOPE_CONFIG;
  }
  const standardLocal = process.platform === "win32"
    ? join(HOME, "AppData", "Local", "microscope-memory", "config.toml")
    : join(HOME, ".config", "microscope-memory", "config.toml");
  if (existsSync(standardLocal)) return standardLocal;

  const repoConfig = resolve(__dirname, "..", "..", "config.toml");
  if (existsSync(repoConfig)) return repoConfig;

  return standardLocal;
}

const CANONICAL_CONFIG = resolveCanonicalConfig();

const ENABLED = process.env.MICROSCOPE_MEMORY_ENABLED !== "0";
const AUTO_STORE = process.env.MICROSCOPE_AUTO_STORE !== "0";
const RETRIEVAL = process.env.MICROSCOPE_RETRIEVAL !== "0";
const MAX_PHRASES = Number(process.env.MICROSCOPE_MAX_PHRASES || "4");
const MAX_HITS = Number(process.env.MICROSCOPE_MAX_HITS || "4");
const CONTEXT_MAX_CHARS = Number(process.env.MICROSCOPE_CONTEXT_MAX_CHARS || "2400");
const REBUILD_EVERY = Number(process.env.MICROSCOPE_REBUILD_EVERY || "20");
const CALL_TIMEOUT_MS = 8000;

// ---------------------------------------------------------------------------
// Logging & Diagnostics
// ---------------------------------------------------------------------------

function trace(level, message) {
  try {
    mkdirSync(LOG_DIR, { recursive: true });
    appendFileSync(
      TRACE_FILE,
      `${new Date().toISOString()} [${level.toUpperCase()}] ${message}\n`,
      "utf8",
    );
  } catch {
    /* fail-silent */
  }
}

// ---------------------------------------------------------------------------
// Resilient JSON Parser (Windows Backslash Sanitization)
// ---------------------------------------------------------------------------

function safeJsonParse(raw) {
  const text = String(raw || "").replace(/^\uFEFF+/, "").trim();
  if (!text) return { ok: true, payload: {} };

  // Step 1: Standard parse
  try {
    return { ok: true, payload: JSON.parse(text) };
  } catch (err1) {
    // Step 2: Sanitize invalid Windows unescaped backslashes
    // Fixes "Bad escaped character in JSON at position..."
    try {
      const sanitized = text.replace(/\\(?!["\\/bfnrt]|u[0-9a-fA-F]{4})/g, "\\\\");
      return { ok: true, payload: JSON.parse(sanitized) };
    } catch (err2) {
      // Step 3: Aggressive fallback for paths
      try {
        const aggressive = text.replace(/\\([^\\])/g, "\\\\$1");
        return { ok: true, payload: JSON.parse(aggressive) };
      } catch (err3) {
        trace("ERROR", `safeJsonParse failed: ${err1.message}`);
        return { ok: false, error: err1.message, payload: {} };
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Native Binary Invocation
// ---------------------------------------------------------------------------

function runBin(args, timeoutMs = CALL_TIMEOUT_MS) {
  return new Promise((resolve) => {
    let child;
    try {
      child = spawn(BIN, args, {
        env: { ...process.env, MICROSCOPE_CONFIG: BRIDGE_CONFIG },
        windowsHide: true,
        stdio: ["ignore", "pipe", "pipe"],
      });
    } catch (e) {
      trace("WARN", `spawn failed: ${e && e.message}`);
      return resolve("");
    }

    let out = "";
    let done = false;
    const finish = (value) => {
      if (done) return;
      done = true;
      try { child.kill(); } catch { /* ignore */ }
      resolve(value);
    };

    const timer = setTimeout(() => {
      trace("WARN", `timeout after ${timeoutMs}ms: ${args.join(" ")}`);
      finish("");
    }, timeoutMs);

    try {
      child.stdout.on("data", (d) => { out += d.toString(); });
      child.stderr.on("data", () => {});
      child.on("error", (e) => {
        clearTimeout(timer);
        trace("WARN", `child error: ${e && e.message}`);
        finish("");
      });
      child.on("close", (code) => {
        clearTimeout(timer);
        if (code !== 0) {
          trace("WARN", `child exit ${code}: ${args.join(" ")}`);
          finish("");
          return;
        }
        finish(out);
      });
    } catch (e) {
      clearTimeout(timer);
      trace("WARN", `stream error: ${e && e.message}`);
      finish("");
    }
  });
}

function syncBridgeConfig() {
  try {
    if (!existsSync(CANONICAL_CONFIG)) return;
    const raw = readFileSync(CANONICAL_CONFIG, "utf8");
    // Ensure auto_rebuild = false in bridge so hook never blocks on 15s rebuild
    const patched = raw.replace(
      /^(auto_rebuild\s*=\s*)true\s*$/m,
      "$1false",
    );
    mkdirSync(LOG_DIR, { recursive: true });
    writeFileSync(BRIDGE_CONFIG, patched, "utf8");
  } catch (e) {
    trace("WARN", `config sync failed: ${e && e.message}`);
  }
}

// ---------------------------------------------------------------------------
// State Management
// ---------------------------------------------------------------------------

function readState() {
  try {
    if (existsSync(STATE_FILE)) {
      const s = JSON.parse(readFileSync(STATE_FILE, "utf8"));
      return s && typeof s === "object" ? s : {};
    }
  } catch { /* ignore */ }
  return {};
}

function writeState(state) {
  try {
    mkdirSync(LOG_DIR, { recursive: true });
    writeFileSync(STATE_FILE, JSON.stringify(state, null, 2), "utf8");
  } catch (e) {
    trace("WARN", `state write failed: ${e && e.message}`);
  }
}

// ---------------------------------------------------------------------------
// Phrase Extraction & Search
// ---------------------------------------------------------------------------

const STOPWORDS = new Set([
  "a", "az", "egy", "és", "de", "hogy", "is", "nem", "van", "meg", "el", "the",
  "and", "for", "with", "that", "this", "from", "you", "your", "not", "but", "has",
  "have", "was", "were", "are", "will", "can", "should", "would", "into", "when",
  "then", "than", "also", "all", "any", "how", "what", "why", "who", "where",
  "which", "about", "just", "like", "make", "made", "need", "want", "use", "used",
  "using", "run", "runs", "running", "it", "its", "there", "their", "them", "they",
  "we", "our", "these", "those", "been", "being", "does", "did", "done", "get", "got",
]);

function extractPhrases(text) {
  if (!text || typeof text !== "string") return [];
  const clean = text
    .replace(/[^\p{L}\p{N}_\-\s]/gu, " ")
    .toLowerCase()
    .split(/\s+/)
    .filter((w) => w.length > 2 && !STOPWORDS.has(w));

  const phrases = [];
  for (let i = 0; i < clean.length - 1; i++) {
    phrases.push(`${clean[i]} ${clean[i + 1]}`);
    if (i < clean.length - 2) {
      phrases.push(`${clean[i]} ${clean[i + 1]} ${clean[i + 2]}`);
    }
  }
  return [...new Set(phrases)].slice(0, MAX_PHRASES);
}

function parseFindOutput(raw) {
  const hits = [];
  for (const line of String(raw || "").split(/\r?\n/)) {
    const m = /\[([^\]]+)\]\s*(.+)$/.exec(line.trim());
    if (m) {
      const text = m[2].trim();
      if (text) hits.push({ layer: m[1].trim(), text });
    }
  }
  return hits;
}

function clip(text, maxChars) {
  if (!text || text.length <= maxChars) return text;
  return text.slice(0, maxChars) + "\n...[clipped]";
}

// ---------------------------------------------------------------------------
// Hook Event Handlers
// ---------------------------------------------------------------------------

async function onSessionStart(payload) {
  if (!ENABLED) return 0;
  syncBridgeConfig();

  trace("INFO", `session_start: id=${payload.session_id || "none"}`);
  const out = await runBin(["auto-context", "--compact"]);
  if (!out || !out.trim()) return 0;

  const context =
    `[MICROSCOPE MEMORY - Auto-Loaded Context]\n` +
    clip(out.trim(), CONTEXT_MAX_CHARS) + "\n";

  process.stdout.write(context);
  trace("INFO", `session_start injected ${context.length} chars`);
  return 0;
}

async function onUserPromptSubmit(payload) {
  if (!ENABLED || !RETRIEVAL) return 0;
  const prompt = payload.prompt || "";
  if (!prompt || prompt.length < 5) return 0;

  const phrases = extractPhrases(prompt);
  if (phrases.length === 0) return 0;

  // IMPORTANT: Sequential execution to avoid Windows mmap lock collisions
  const hits = [];
  const seen = new Set();

  for (const phrase of phrases) {
    const raw = await runBin(["find", phrase, "--limit", "3"]);
    const items = parseFindOutput(raw);
    for (const item of items) {
      if (!seen.has(item.text)) {
        seen.add(item.text);
        hits.push(item);
        if (hits.length >= MAX_HITS) break;
      }
    }
    if (hits.length >= MAX_HITS) break;
  }

  if (hits.length === 0) return 0;

  const formatted = hits
    .map((h) => `• [${h.layer}] ${h.text}`)
    .join("\n");
  const content =
    `[MICROSCOPE MEMORY - Retrieved Turn Context]\n` +
    clip(formatted, CONTEXT_MAX_CHARS) + "\n";

  process.stdout.write(content);
  trace("INFO", `prompt injected ${hits.length} hits`);
  return 0;
}

async function onPostToolUse(payload) {
  if (!ENABLED || !AUTO_STORE) return 0;

  const toolName = payload.tool_name || "";
  const toolInput = typeof payload.tool_input === "string"
    ? payload.tool_input
    : JSON.stringify(payload.tool_input || "");
  const toolResult = typeof payload.tool_result === "string"
    ? payload.tool_result
    : JSON.stringify(payload.tool_result || "");
  const isError = Boolean(payload.is_error);

  // Skip noise
  if (!toolName || toolName === "read_file" || toolName === "list_dir") return 0;

  // Determine layer & importance
  let layer = "session";
  let importance = 5;

  if (isError) {
    layer = "code";
    importance = 6;
  } else if (/edit_file|write_file|patch|run_command/.test(toolName)) {
    layer = "code";
    importance = 6;
  }

  const summary = `Tool ${toolName}: ${isError ? "FAILED" : "SUCCESS"}. In: ${toolInput.slice(0, 100)} Out: ${toolResult.slice(0, 200)}`;
  const cleanSummary = summary.replace(/[\r\n]+/g, " ").trim();

  await runBin(["store", cleanSummary, "--layer", layer, "--importance", String(importance)]);
  trace("INFO", `stored: [${layer}:${importance}] ${cleanSummary.slice(0, 80)}`);

  // Check rebuild threshold
  const state = readState();
  state.store_count = (state.store_count || 0) + 1;
  writeState(state);

  if (state.store_count % REBUILD_EVERY === 0) {
    triggerDetachedRebuild();
  }

  return 0;
}

async function onStop(payload) {
  if (!ENABLED) return 0;
  trace("INFO", "session stop received");
  return 0;
}

function triggerDetachedRebuild() {
  try {
    trace("INFO", "triggering detached background rebuild");
    const child = spawn(
      process.execPath,
      [__filename, "rebuild"],
      {
        detached: true,
        stdio: "ignore",
        windowsHide: true,
      }
    );
    child.unref();
  } catch (e) {
    trace("WARN", `detached rebuild trigger failed: ${e && e.message}`);
  }
}

async function runRebuildWorker() {
  trace("INFO", "detached rebuild worker starting");
  const start = Date.now();
  await runBin(["build"], 30000);
  const dur = Date.now() - start;
  trace("INFO", `detached rebuild worker finished in ${dur}ms`);
  return 0;
}

// ---------------------------------------------------------------------------
// Stdin & Main Dispatch
// ---------------------------------------------------------------------------

function readStdin() {
  return new Promise((resolve) => {
    if (process.stdin.isTTY) return resolve("");
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => { data += chunk; });
    process.stdin.on("end", () => resolve(data));
    process.stdin.on("error", () => resolve(""));
    setTimeout(() => resolve(data), 2000);
  });
}

async function main() {
  const mode = (process.argv[2] || "").toLowerCase();
  const raw = await readStdin();
  const parsed = safeJsonParse(raw);
  const payload = parsed.payload || {};

  const nested = payload.payload && typeof payload.payload === "object" ? payload.payload : {};
  const get = (...names) => {
    for (const n of names) {
      if (payload[n] !== undefined && payload[n] !== null) return payload[n];
      if (nested[n] !== undefined && nested[n] !== null) return nested[n];
    }
    return undefined;
  };

  const view = {
    ...payload,
    prompt: get("prompt", "user_prompt") ?? "",
    tool_name: get("tool_name", "tool") ?? "",
    tool_input: get("tool_input", "input") ?? "",
    tool_result: get("tool_result", "tool_response", "tool_output", "result") ?? "",
    is_error: get("is_error", "tool_error", "error") ?? false,
    session_id: get("session_id") ?? "",
  };

  switch (mode) {
    case "session-start":
    case "sessionstart":
      return onSessionStart(view);
    case "prompt":
    case "userpromptsubmit":
      return onUserPromptSubmit(view);
    case "tool-result":
    case "posttooluse":
      return onPostToolUse(view);
    case "stop":
    case "sessionend":
      return onStop(view);
    case "rebuild":
      return runRebuildWorker();
    default:
      trace("WARN", `unknown hook mode: ${mode}`);
      return 0;
  }
}

main()
  .then((code) => process.exit(code || 0))
  .catch((e) => {
    trace("ERROR", `unhandled exception: ${e && e.stack ? e.stack : e}`);
    process.exit(0);
  });
