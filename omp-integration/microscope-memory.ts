/**
 * Microscope Memory — omp (oh-my-pi) bridge
 * =========================================
 *
 * Wires the Microscope Memory engine into an omp session following the
 * engine's own prescribed pattern for CLI agents
 * (docs/CODEX_HOOKS.md):
 *
 *     before each response:  microscope-mem auto-context
 *     after each interaction: microscope-mem store
 *     optional MCP server:    microscope-mem mcp   (memory_* tools)
 *
 * Transport: the engine's own MCP server ("the universal transport"), spoken
 * over stdio JSON-RPC. The CLI's stdout was rejected as a transport because it
 * mixes results with ANSI colouring and stderr diagnostics; the MCP tools
 * return clean, structured text.
 *
 * Measured engine facts this bridge is built on:
 *
 *  - `memory_find` is the reliable lookup. It searches the binary index AND
 *    the append log, and ranks by relevance. `memory_ping` returns noise
 *    (measured: every hit scored 0.000-0.001 and the entries were unrelated),
 *    so it is not used here.
 *  - `memory_recall` is semantic but its ranking is weak and its output
 *    carries NARRATIVE/SELF/CURIOUS sections; it is exposed to the model
 *    through the MCP server, not used for automatic injection.
 *  - A stored memory is written to the append log immediately and to
 *    `layers/<name>.txt` best-effort; the binary index only catches up at the
 *    next rebuild. `find`/`recall` search the append log too, so a stored
 *    memory is retrievable right away.
 *  - `auto-context` assembles: state, the last session timeline, open loops
 *    and the core memories (importance >= 7) - and the core memories come
 *    from the append log, so they are the freshest high-importance items.
 *  - The engine's own hook manager runs read-only by default
 *    (`read_only=true, write_enabled=false`); writes happen because the
 *    bridge calls `memory_store` explicitly.
 *
 * The bridge is fail-silent: a broken engine, a missing binary or a locked
 * index must never break the agent. Every call is bounded by a timeout, every
 * failure is traced to its own log file, and no handler ever throws.
 *
 * Environment overrides:
 *   MICROSCOPE_MEMORY_ENABLED=0   disable the whole bridge
 *   MICROSCOPE_AUTO_STORE=0       disable automatic storing
 *   MICROSCOPE_RETRIEVAL=0        disable per-prompt retrieval
 *   MICROSCOPE_BIN                path to microscope-mem.exe
 *   MICROSCOPE_CONFIG             path to config.toml (data + behaviour)
 */

import { spawn } from "node:child_process";
import { appendFileSync, existsSync, readFileSync, renameSync, writeFileSync } from "node:fs";

const env = process.env;

const ENABLED = env.MICROSCOPE_MEMORY_ENABLED !== "0";
const AUTO_STORE = env.MICROSCOPE_AUTO_STORE !== "0";
const RETRIEVAL = env.MICROSCOPE_RETRIEVAL !== "0";

// The bridge prefers the binary built from the current source (it has the
// relevance ranking). The August build in D:\codex is deliberately NOT in
// the list: it is the previous engine generation.
const BIN =
  [
    env.MICROSCOPE_BRIDGE_BIN,
    "C:/Users/mater/microscope-memory-clone/target/release/microscope-mem.exe",
    "C:/Users/mater/AppData/Local/microscope-memory/microscope-mem.exe",
  ].find((p) => !!p && existsSync(p as string)) || "";

// The bridge owns the memory of THIS repo (the clone instance). The machine
// wide D:\codex instance is the previous generation and is shared by other
// tools, so the bridge must not write its session traffic into it. Override
// with MICROSCOPE_BRIDGE_CONFIG if that is ever wanted.
const CONFIG =
  [
    env.MICROSCOPE_BRIDGE_CONFIG,
    "C:/Users/mater/.omp/agent/microscope-memory-bridge.toml",
    "C:/Users/mater/AppData/Local/microscope-memory/config.toml",
  ].find((p) => !!p && existsSync(p as string)) || "";

const MCP_ENV = CONFIG ? { ...env, MICROSCOPE_CONFIG: CONFIG } : { ...env };

// The bridge config is the canonical config with one difference: auto_rebuild
// is off, so a store can never start a multi-second index rebuild inside a
// hook handler. It is regenerated from the canonical config at every session
// start, so a change to the canonical config (a different embedding provider,
// for example) can never go stale in the bridge.
//
// The source is a fixed path, never the ambient MICROSCOPE_CONFIG: a process
// can carry a stale environment (the old D:\codex instance), and following it
// would silently point the bridge at the wrong memory.
const CANONICAL_CONFIG =
  env.MICROSCOPE_BRIDGE_BASE || "C:/Users/mater/AppData/Local/microscope-memory/config.toml";
const BRIDGE_CONFIG = "C:/Users/mater/.omp/agent/microscope-memory-bridge.toml";

function syncBridgeConfig(): void {
  try {
    if (!existsSync(CANONICAL_CONFIG)) return;
    const raw = readFileSync(CANONICAL_CONFIG, "utf8");
    const patched = raw.replace(
      /^([ \t]*)auto_rebuild[ \t]*=[ \t]*true/m,
      "$1auto_rebuild = false",
    );
    const current = existsSync(BRIDGE_CONFIG)
      ? readFileSync(BRIDGE_CONFIG, "utf8")
      : "";
    if (current === patched) return;
    const tmp = BRIDGE_CONFIG + ".tmp";
    writeFileSync(tmp, patched, "utf8");
    // Rename over the old file so a reader never sees a half-written config.
    try {
      renameSync(tmp, BRIDGE_CONFIG);
    } catch {
      writeFileSync(BRIDGE_CONFIG, patched, "utf8");
    }
  } catch {
    /* a stale bridge config must never break the session */
  }
}

const TRACE_FILE = "C:/Users/mater/.omp/agent/microscope-memory.log";
const MAX_PHRASES = Number(env.MICROSCOPE_MAX_PHRASES || "4");
const MAX_HITS = Number(env.MICROSCOPE_MAX_HITS || "4");
const CONTEXT_MAX_CHARS = Number(env.MICROSCOPE_CONTEXT_MAX_CHARS || "2400");

// ---------------------------------------------------------------------------
// Tracing
// ---------------------------------------------------------------------------

let logger: any = null;
let piRef: any = null;

function trace(level: string, message: string) {
  try {
    appendFileSync(TRACE_FILE, new Date().toISOString() + " [" + level + "] " + message + "\n", "utf8");
  } catch {
    /* tracing must never break the agent */
  }
}

function log(level: string, message: string) {
  trace(level, message);
  try {
    const target = logger || piRef?.logger;
    if (!target) return;
    const fn = target[level] || target.log || target.info;
    if (typeof fn === "function") fn.call(target, message);
  } catch {
    /* logging must never break the agent */
  }
}

// ---------------------------------------------------------------------------
// MCP client — the engine's universal transport
// ---------------------------------------------------------------------------

type Pending = { resolve: (v: any) => void; timer: any };

class McpClient {
  private child: any = null;
  private pending = new Map<number, Pending>();
  private nextId = 1;
  private buffer = "";
  private starting: Promise<boolean> | null = null;

  get alive(): boolean {
    return !!this.child && !this.child.killed;
  }

  async start(): Promise<boolean> {
    if (this.alive) return true;
    if (this.starting) return this.starting;
    this.starting = this.doStart().finally(() => {
      this.starting = null;
    });
    return this.starting;
  }

  private async doStart(): Promise<boolean> {
    if (!BIN) {
      log("warn", "[microscope-memory] microscope-mem.exe not found - bridge idle");
      return false;
    }
    try {
      const child = spawn(BIN, ["mcp"], {
        env: MCP_ENV,
        stdio: ["pipe", "pipe", "pipe"],
        windowsHide: true,
      });
      this.child = child;

      child.stdout.setEncoding("utf8");
      child.stdout.on("data", (chunk: string) => this.onData(chunk));
      child.stderr.setEncoding("utf8");
      child.stderr.on("data", (d: string) => {
        const t = (d || "").trim();
        if (t) trace("debug", "[microscope-memory] mcp stderr: " + t.slice(0, 300));
      });
      child.on("error", (e: any) => log("warn", "[microscope-memory] mcp spawn error: " + (e?.message || e)));
      child.on("exit", (code: number) => {
        log("info", "[microscope-memory] mcp server exited (code " + code + ")");
        this.child = null;
        for (const [, p] of this.pending) {
          clearTimeout(p.timer);
          p.resolve(null);
        }
        this.pending.clear();
      });

      const init = await this.rpc(
        "initialize",
        {
          protocolVersion: "2024-11-05",
          capabilities: {},
          clientInfo: { name: "omp-microscope-bridge", version: "1.0.0" },
        },
        15000,
      );
      if (!init) {
        log("warn", "[microscope-memory] MCP initialize failed");
        this.stop();
        return false;
      }
      this.notify("notifications/initialized");
      log("info", "[microscope-memory] MCP engine connected: " + (init?.serverInfo?.name || "microscope"));
      return true;
    } catch (e: any) {
      log("warn", "[microscope-memory] MCP start failed: " + (e?.message || e));
      this.child = null;
      return false;
    }
  }

  private onData(chunk: string) {
    this.buffer += chunk;
    let idx: number;
    while ((idx = this.buffer.indexOf("\n")) >= 0) {
      const line = this.buffer.slice(0, idx).trim();
      this.buffer = this.buffer.slice(idx + 1);
      if (!line) continue;
      let msg: any;
      try {
        msg = JSON.parse(line);
      } catch {
        continue;
      }
      if (msg.id && this.pending.has(msg.id)) {
        const p = this.pending.get(msg.id)!;
        clearTimeout(p.timer);
        this.pending.delete(msg.id);
        p.resolve(msg);
      }
    }
  }

  private notify(method: string) {
    try {
      this.child?.stdin?.write(JSON.stringify({ jsonrpc: "2.0", method }) + "\n");
    } catch {}
  }

  private rpc(method: string, params: any, timeoutMs: number): Promise<any> {
    return new Promise((resolve) => {
      const id = this.nextId++;
      const timer = setTimeout(() => {
        this.pending.delete(id);
        log("warn", "[microscope-memory] MCP timeout: " + method);
        resolve(null);
      }, timeoutMs);
      this.pending.set(id, { resolve, timer });
      try {
        this.child?.stdin?.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
      } catch (e: any) {
        clearTimeout(timer);
        this.pending.delete(id);
        resolve(null);
      }
    });
  }

  /** Call a memory tool and return its text output. */
  async tool(name: string, args: any, timeoutMs = 20000): Promise<string> {
    if (!(await this.start())) return "";
    const res = await this.rpc("tools/call", { name, arguments: args }, timeoutMs);
    if (!res) return "";
    if (res.error) {
      log("warn", "[microscope-memory] MCP error on " + name + ": " + JSON.stringify(res.error).slice(0, 300));
      return "";
    }
    const content = res.result?.content;
    if (Array.isArray(content)) {
      return content
        .map((c: any) => (typeof c?.text === "string" ? c.text : ""))
        .filter(Boolean)
        .join("\n");
    }
    return "";
  }

  stop() {
    try {
      if (this.child) {
        this.child.kill();
        this.child = null;
      }
    } catch {}
    for (const [, p] of this.pending) {
      clearTimeout(p.timer);
      p.resolve(null);
    }
    this.pending.clear();
  }
}

// ---------------------------------------------------------------------------
// Phrase extraction — what to look up from the user's prompt
// ---------------------------------------------------------------------------

const STOPWORDS = new Set(
  (
    "a az egy és de hogy is nem van meg el the and for with that this from you your not but has have " +
    "was were are will can should would into when then than also all any how what why who where which " +
    "about just like make made need want use used using run runs running it its there their them they we " +
    "our these those been being does did done get got give given take taken here more most other some " +
    "such only own same too very do if because until while against between through during before after " +
    "above below up down out off again further once each few nor s " +
    "itt ezt azt mert vagy mint akkor után előtt közben szerint miatt nélkül által egyéb más másik " +
    "sok kevés nagyon most már még csak minden semmi valami akármi meg kell lesz lehet " +
    "hogyha amikor ami amely amiért tehát vagyis"
  ).split(/\s+/),
);

/**
 * Pick the phrases worth looking up.
 *
 * The engine's `find` is a phrase match, so the whole prompt never matches;
 * a multi-word phrase does. Single words are avoided on purpose: measured on
 * the real index, a one-word query returns the bare token blocks ("Octopus")
 * instead of the entry that carries the information.
 */
function extractPhrases(prompt: string): string[] {
  const strong: string[] = [];
  const weak: string[] = [];
  const seen = new Set<string>();

  const push = (raw: string, bucket: string[]) => {
    const t = (raw || "")
      .trim()
      .replace(/\s+/g, " ")
      .replace(/^[`"']+|[`"'\.,;:!?()\[\]]+$/g, "");
    if (t.length < 8 || t.length > 90) return;
    const key = t.toLowerCase();
    if (seen.has(key)) return;
    seen.add(key);
    bucket.push(t);
  };

  for (const m of prompt.matchAll(/`([^`\n]{8,90})`/g)) push(m[1], strong);
  for (const m of prompt.matchAll(/"([^"\n]{8,90})"/g)) push(m[1], strong);
  for (const m of prompt.matchAll(/[A-Za-z]:\\[^\s"'<>|]{4,120}/g)) push(m[0], strong);
  for (const m of prompt.matchAll(/\b[A-Za-z][A-Za-z0-9]*(?:[_\-.][A-Za-z0-9]+)+\b/g)) push(m[0], strong);
  // Title Case sequences ("Standing Order", "Open Loop") - the Hungarian
  // suffix would otherwise make the 3-gram miss the stored phrase.
  for (const m of prompt.matchAll(/\b[A-Z][a-z]+(?:\s+[A-Z][a-z]+)+\b/g)) push(m[0], strong);

  const words = prompt.split(/[^\p{L}\p{N}_]+/u).filter(Boolean);
  for (let i = 0; i + 2 < words.length; i++) {
    const w = words.slice(i, i + 3);
    if (w.every((x) => STOPWORDS.has(x.toLowerCase()))) continue;
    if (!w.some((x) => x.length >= 6 || /\d/.test(x))) continue;
    push(w.join(" "), weak);
  }

  return strong.concat(weak).slice(0, MAX_PHRASES);
}

/** Parse the clean output of `memory_find`: "[D2 long_term] text". */
function parseFind(text: string): string[] {
  const out: string[] = [];
  for (const line of (text || "").split(/\r?\n/)) {
    const m = /^\[D\d+\s+[^\]]+\]\s*(.+)$/.exec(line.trim());
    if (m) {
      const t = m[1].trim();
      if (t) out.push(t);
    }
  }
  return out;
}

/**
 * The engine's own post-tool-use semantics: a tool output that contains a
 * fact, an error or a decision is stored with an escalating layer and
 * importance (ai-memory-skill/hooks/post-tool-use.ps1).
 */
const FACT_PATTERNS: { re: RegExp; layer: string; importance: number; prefix: string }[] = [
  {
    re: /(fixed|javított|solved|megoldott|implemented|implementált|added|hozzáadva|created|létrehozva|discovered|found|talált|learned|tanult|understood|értett)/i,
    layer: "session",
    importance: 5,
    prefix: "",
  },
  {
    re: /(error|hiba|exception|failed|sikertelen|crash|assert|panic)/i,
    layer: "reflections",
    importance: 7,
    prefix: "ERROR CONTEXT: ",
  },
  {
    re: /(architecture|architektúra|protocol|protokoll|design|tervezés|decision|döntés|choice|választás)/i,
    layer: "long_term",
    importance: 8,
    prefix: "DESIGN DECISION: ",
  },
];

function classifyToolOutput(output: string): { text: string; layer: string; importance: number }[] {
  const out: { text: string; layer: string; importance: number }[] = [];
  for (const p of FACT_PATTERNS) {
    if (!p.re.test(output)) continue;
    const tail = output.slice(Math.max(0, output.length - 500)).trim();
    if (tail.length < 50) continue;
    out.push({ text: p.prefix + tail, layer: p.layer, importance: p.importance });
  }
  return out;
}

/** Prompts that explicitly ask about what is remembered. */
const MEMORY_QUESTION =
  /(emlékszel|emlékezik|remember|mit tudsz|mit tudsz rólam|what do you know|do you know|tudsz-e|előző|korábban|earlier|previous|álljunk vissza|MUNKAALLAPOT|STANDING ORDER|memóri)/i;

// ---------------------------------------------------------------------------
// Message helpers
// ---------------------------------------------------------------------------

function textOf(payload: any): string {
  if (!payload) return "";
  if (typeof payload === "string") return payload;
  const content = payload.content;
  if (typeof content === "string") return content;
  if (Array.isArray(content)) {
    return content
      .map((block: any) => {
        if (typeof block === "string") return block;
        if (!block) return "";
        if (typeof block.text === "string") return block.text;
        if (typeof block.content === "string") return block.content;
        if (block.type === "text" && typeof block.value === "string") return block.value;
        return "";
      })
      .filter(Boolean)
      .join("\n");
  }
  return "";
}

function roleOf(message: any): string {
  if (!message) return "";
  return String(message.role || message.type || "").toLowerCase();
}

/** Last user prompt and last assistant answer out of a message list. */
function extractExchange(messages: any): { user: string; assistant: string } {
  let user = "";
  let assistant = "";
  const list = Array.isArray(messages) ? messages : [];
  for (let i = list.length - 1; i >= 0; i--) {
    const m = list[i];
    if (!m || m.customType) continue;
    const role = roleOf(m);
    if (!assistant && (role === "assistant" || role === "model")) assistant = textOf(m);
    if (!user && role === "user") user = textOf(m);
    if (user && assistant) break;
  }
  return { user, assistant };
}

function clip(text: string, max: number): string {
  const t = (text || "").replace(/\s+/g, " ").trim();
  if (t.length <= max) return t;
  return t.slice(0, Math.max(0, max - 1)) + "\u2026";
}

// ---------------------------------------------------------------------------
// Extension entry point
// ---------------------------------------------------------------------------

export default function microscopeMemory(pi: any) {
  piRef = pi;
  logger = pi.logger || null;

  if (!ENABLED) {
    log("info", "[microscope-memory] disabled via MICROSCOPE_MEMORY_ENABLED=0");
    return;
  }

  const mcp = new McpClient();

  let autoContext = "";
  let retrieved = "";
  let semanticOk = false;
  let cwd = "";
  let exchanges = 0;
  let storedHashes = new Set<string>();
  let sessionId = "";
  let touchedFiles = new Set<string>();
  let commands: string[] = [];
  let toolErrors = 0;

  // The main session id, captured at the first session_start. Subagent loops
  // run in their own session, and their exchanges must not be stored as if
  // they were the user's session.
  const sessionIdOf = (ctx: any): string => {
    try {
      const sm = ctx?.sessionManager;
      if (!sm) return "";
      if (typeof sm.getSessionId === "function") return String(sm.getSessionId() || "");
      if (sm.sessionId) return String(sm.sessionId);
      return "";
    } catch {
      return "";
    }
  };

  let mainSessionId = "";

  const isMainSession = (ctx: any): boolean => {
    if (!mainSessionId) return true;
    const id = sessionIdOf(ctx);
    if (!id) return true;
    return id === mainSessionId;
  };

  // --- session lifecycle --------------------------------------------------
  pi.on("session_start", async (_event: any, ctx: any) => {
    try {
      cwd = ctx?.cwd || cwd || process.cwd();
      log("info", "[microscope-memory] session start (cwd=" + cwd + ")");
      if (!mainSessionId) mainSessionId = sessionIdOf(ctx);
      // Follow the canonical config (a changed embedding provider must not
      // stay invisible to the bridge).
      syncBridgeConfig();
      if (!(await mcp.start())) return;
      autoContext = (await mcp.tool("memory_auto_context", {}, 20000)).trim();

      // The engine's own session-start lookup (ai-memory-skill/hooks/
      // session-start.ps1): the standing orders and the recent session
      // summaries are found by their fixed markers, not by a free-text query.
      // Measured: a prompt-derived phrase ("Standing Order bejegyzesekrol")
      // does not match the stored text, so these two lookups are fixed.
      const standing = parseFind(await mcp.tool("memory_find", { query: "STANDING ORDER", k: 5 }, 15000));
      const summaries = parseFind(await mcp.tool("memory_find", { query: "SESSION CONTEXT", k: 5 }, 15000));
      const fixed: string[] = [];
      if (standing.length) fixed.push("[STANDING ORDERS]\n" + standing.join("\n"));
      if (summaries.length) fixed.push("[RECENT SESSION SUMMARIES]\n" + summaries.join("\n"));

      // The open loops are the engine's "unfinished work" layer and are not
      // part of the auto-context snapshot, so they are fetched separately.
      const loops = (await mcp.tool("memory_loops", { k: 5 }, 15000)).trim();
      if (loops && !/\bno open loops\b|\b0 open loops\b/i.test(loops)) {
        fixed.push("[OPEN LOOPS - unfinished work]\n" + loops);
      }
      if (fixed.length) autoContext += "\n\n" + fixed.join("\n\n");
      // The semantic path only carries meaning with a real embedding provider.
      // With the mock provider the recall results are scored "unknown" and
      // measured as noise, so the semantic lookup stays off until the engine
      // is built with real embeddings.
      const status = await mcp.tool("memory_status", {}, 15000);
      const provider = /Embedding provider:\s*([\w-]+)/i.exec(status || "");
      semanticOk = !!provider && !/^(mock|none)$/i.test(provider[1]);
      log(
        "info",
        "[microscope-memory] session context loaded (" +
          autoContext.length +
          " chars, embedding provider=" +
          (provider ? provider[1] : "?") +
          ", semantic recall=" +
          (semanticOk ? "on" : "off") +
          ")",
      );
    } catch (e: any) {
      log("warn", "[microscope-memory] session_start failed: " + (e?.message || e));
    }
  });

  // --- per prompt: look up what the prompt is about -----------------------
  pi.on("before_agent_start", async (event: any) => {
    try {
      const prompt = typeof event?.prompt === "string" ? event.prompt : "";
      // Remembered for the store: the assistant's answer arrives in a separate
      // event (message_end).
      if (prompt) pendingPrompt = prompt;
      if (!RETRIEVAL || !BIN || prompt.length < 20) return;
      const isQuestion = MEMORY_QUESTION.test(prompt);
      const phrases = extractPhrases(prompt);
      if (phrases.length === 0 && !isQuestion) return;

      const lines: string[] = [];
      const seen = new Set<string>();
      const add = (hit: string) => {
        const key = hit.slice(0, 80).toLowerCase();
        if (seen.has(key)) return;
        seen.add(key);
        lines.push(hit);
      };

      // Deterministic phrase lookup - the reliable primitive.
      //
      // Sequential on purpose: the engine maps the index file, and on Windows
      // several concurrent readers of the same mapping collide (the engine's
      // own tests document "creating microscope.bin fails while a user-mapped
      // section is still open"). A find takes 4-16 ms, so running them one
      // after the other costs less than a tenth of a second.
      if (phrases.length) {
        for (const phrase of phrases) {
          const text = await mcp.tool("memory_find", { query: phrase, k: 3 }, 15000);
          for (const hit of parseFind(text)) {
            add(hit);
            if (lines.length >= MAX_HITS) break;
          }
          if (lines.length >= MAX_HITS) break;
        }
      }

      // Semantic recall for the whole prompt. Only when the engine has real
      // embeddings: with the mock provider the ranking is meaningless and
      // the output carries extra sections.
      if (isQuestion && semanticOk) {
        const rec = await mcp.tool("memory_recall", { query: prompt.slice(0, 300), k: 5 }, 20000);
        for (const hit of parseFind(rec)) {
          add(hit);
          if (lines.length >= MAX_HITS + 2) break;
        }
      }

      retrieved = lines.join("\n");
      if (retrieved) {
        log(
          "info",
          "[microscope-memory] retrieved " +
            lines.length +
            " hit(s) for prompt" +
            (isQuestion && semanticOk ? " (semantic recall included)" : ""),
        );
      } else {
        log("debug", "[microscope-memory] no stored memory for prompt");
      }
    } catch (e: any) {
      log("warn", "[microscope-memory] retrieval failed: " + (e?.message || e));
      retrieved = "";
    }
  });

  // --- inject into the model call (not persisted in the session) ---------
  pi.on("context", async (event: any) => {
    try {
      if (!ENABLED) return;
      const messages = Array.isArray(event?.messages) ? event.messages.slice() : [];
      if (messages.length === 0) return;

      const parts: string[] = [];
      if (autoContext) parts.push(autoContext);
      if (retrieved) parts.push("[RELEVANT STORED MEMORY]\n" + retrieved);
      if (parts.length === 0) return;

      let content = parts.join("\n\n");
      if (content.length > CONTEXT_MAX_CHARS) {
        content = content.slice(0, CONTEXT_MAX_CHARS) + "\n\u2026";
      }

      messages.push({
        role: "custom",
        customType: "microscope-memory",
        content:
          "[MICROSCOPE MEMORY - retrieved context for this turn]\n" + content,
        display: false,
        attribution: "agent",
        timestamp: Date.now(),
      });
      log(
        "info",
        "[microscope-memory] injected " +
          content.length +
          " chars into the model call (auto-context " +
          autoContext.length +
          ", retrieved " +
          retrieved.length +
          ")",
      );
      return { messages };
    } catch (e: any) {
      log("warn", "[microscope-memory] context injection failed: " + (e?.message || e));
      return;
    }
  });

  // --- observe tool activity (notification event, never blocks) -----------
  pi.on("tool_result", (event: any) => {
    try {
      if (event?.isError) toolErrors++;
      const name = String(event?.toolName || "");
      const input = event?.input || {};
      const path = input.path || input.file_path || input.filePath || input.filename;
      if (path) touchedFiles.add(String(path));
      if (name === "bash" && input.command) commands.push(clip(String(input.command), 90));

      // The engine's post-tool-use semantics: a tool output that carries a
      // fact, an error or a decision is stored with an escalating layer and
      // importance. The store is fire-and-forget so the tool call is never
      // delayed.
      if (!AUTO_STORE) return;
      const output = textOf(event?.content);
      // A failed tool call is an objective "unfinished work" signal: open a
      // loop so the next session (and the model) see it. The model decides
      // when to resolve it (memory_resolve_loop) - this is not a keyword
      // heuristic, just a reaction to an actual error.
      if (event?.isError) {
        const snippet = output ? clip(output, 300) : "(no output)";
        const loopText = "OPEN LOOP (tool error): " + (name || "tool") + " failed — " + snippet;
        mcp
          .tool("memory_store", { text: loopText, layer: "session", importance: 6, status: "open" }, 20000)
          .catch(() => {});
      }
      if (!output || output.length < 60) return;
      const facts = classifyToolOutput(output);
      for (const fact of facts) {
        mcp.tool(
          "memory_store",
          { text: fact.text, layer: fact.layer, importance: fact.importance },
          20000,
        )
          .then((res) => {
            if (res) {
              log(
                "info",
                "[microscope-memory] stored tool fact (" + fact.layer + "/imp=" + fact.importance + ")",
              );
            }
          })
          .catch(() => {});
      }
    } catch {}
  });

  // --- auto-store the exchange -------------------------------------------
  const storeExchange = async (user: string, assistant: string, trigger: string): Promise<boolean> => {
    const u = (user || "").trim();
    const a = (assistant || "").trim();
    if (!u || !a) {
      log("debug", "[microscope-memory] " + trigger + ": nothing to store");
      return false;
    }
    if (u.startsWith("/")) {
      log("debug", "[microscope-memory] " + trigger + ": slash command, skipped");
      return false;
    }
    if (u.length + a.length < 25) {
      log("debug", "[microscope-memory] " + trigger + ": exchange too short, skipped");
      return false;
    }
    const key = u.slice(0, 120) + "|" + a.slice(0, 120);
    if (storedHashes.has(key)) {
      log("debug", "[microscope-memory] " + trigger + ": duplicate, skipped");
      return false;
    }
    storedHashes.add(key);

    const text =
      "OMP SESSION " +
      new Date().toISOString().slice(0, 16).replace("T", " ") +
      " [" +
      (cwd || process.cwd()) +
      "]\nUSER: " +
      clip(u, 700) +
      "\nASSISTANT: " +
      clip(a, 900);

    const out = await mcp.tool(
      "memory_store",
      { text, layer: "session", importance: 5 },
      30000,
    );
    exchanges++;
    if (out) {
      log("info", "[microscope-memory] stored exchange (" + trigger + "): " + out.replace(/\s+/g, " ").slice(0, 160));
    } else {
      log("warn", "[microscope-memory] store produced no output (" + trigger + ")");
    }
    return true;
  };

  // --- periodic session level consolidation --------------------------------
  //
  // The counters live in a state file, because omp is restarted often: a
  // per-process counter would never reach the threshold in short sessions.
  // The digest is cheap and compact, the consolidation summarises the whole
  // session layer, so the two have different thresholds.
  const STATE_FILE = "C:/Users/mater/.omp/agent/microscope-memory-state.json";
  const DIGEST_EVERY = 5;
  const CONSOLIDATE_EVERY = 20;

  const readState = (): { turns?: number; consolidations?: number } => {
    try {
      const raw = readFileSync(STATE_FILE, "utf8");
      const parsed = JSON.parse(raw);
      return {
        turns: typeof parsed?.turns === "number" ? parsed.turns : 0,
        consolidations: typeof parsed?.consolidations === "number" ? parsed.consolidations : 0,
      };
    } catch {
      return { turns: 0, consolidations: 0 };
    }
  };

  const writeState = (turns: number, consolidations: number) => {
    try {
      writeFileSync(
        STATE_FILE,
        JSON.stringify({ turns, consolidations, at: new Date().toISOString() }),
        "utf8",
      );
    } catch {}
  };

  const state = readState();
  let turnsSinceConsolidation = state.turns || 0;
  let turnsSinceDigest = state.consolidations || 0;

  const writeSessionDigest = async (reason: string, consolidate: boolean): Promise<void> => {
    try {
      const date = new Date().toISOString().slice(0, 10);
      const parts: string[] = [
        "SESSION CONTEXT " + date + " (omp): repo=" + (cwd || process.cwd()),
        "turns=" + exchanges,
      ];
      if (touchedFiles.size) parts.push("files: " + Array.from(touchedFiles).slice(0, 6).join(", "));
      if (commands.length) parts.push("cmds: " + commands.slice(0, 4).join(" | "));
      if (toolErrors) parts.push("tool-errors=" + toolErrors);
      const context = clip(parts.join(" | "), 600);
      const out = await mcp.tool(
        "memory_session_context",
        {
          context,
          summary:
            date +
            " (omp): repo=" +
            (cwd || process.cwd()) +
            ", turns=" +
            exchanges +
            (touchedFiles.size ? ", files=" + touchedFiles.size : "") +
            (toolErrors ? ", tool-errors=" + toolErrors : ""),
        },
        1500,
      );
      log(
        "info",
        "[microscope-memory] session digest stored (" + reason + "): " + out.replace(/\s+/g, " ").slice(0, 140),
      );

      if (!consolidate) return;

      // Consolidate the session into long-term memory. The engine groups the
      // session entries by session id and writes a summary of each group, so
      // what the session learned stays findable in the long_term layer and not
      // only in the append log. It is additive - nothing is removed.
      const cons = await mcp.tool("memory_consolidate", {}, 1500);
      if (cons) {
        log(
          "info",
          "[microscope-memory] session consolidated (" + reason + "): " + cons.replace(/\s+/g, " ").slice(0, 140),
        );
      }
    } catch (e: any) {
      log("warn", "[microscope-memory] digest failed: " + (e?.message || e));
    }
  };

  /** Called after an exchange was actually stored (not a duplicate). */
  const afterStore = async (): Promise<void> => {
    turnsSinceConsolidation++;
    turnsSinceDigest++;
    const dueDigest = turnsSinceDigest >= DIGEST_EVERY;
    const dueConsolidation = turnsSinceConsolidation >= CONSOLIDATE_EVERY;
    if (dueDigest) turnsSinceDigest = 0;
    if (dueConsolidation) turnsSinceConsolidation = 0;
    writeState(turnsSinceConsolidation, turnsSinceDigest);
    if (dueDigest || dueConsolidation) {
      await writeSessionDigest(
        dueConsolidation ? "periodic+consolidate" : "periodic",
        dueConsolidation,
      );
    }
  };

  // --- the assistant's finished answer: the reliable store point -----------
  //
  // Measured: in a session that is interrupted (the usual way people quit),
  // only session_start, before_agent_start, context and message_end fire.
  // agent_end, session_stop, turn_end and session_shutdown do NOT. So the
  // exchange is stored when the assistant message completes; the end-of-turn
  // events stay as a fallback and the content hash keeps them from storing
  // twice.
  let pendingPrompt = "";

  const hasToolCalls = (message: any): boolean => {
    const content = message?.content;
    if (!Array.isArray(content)) return false;
    return content.some(
      (b: any) =>
        b &&
        (b.type === "toolCall" ||
          b.type === "tool_use" ||
          b.type === "tool-call" ||
          typeof b.name === "string"),
    );
  };

  const isFinalAnswer = (message: any): boolean => {
    const stop = message?.stopReason;
    if (stop && stop !== "stop" && stop !== "end_turn" && stop !== "length") return false;
    return !hasToolCalls(message);
  };

  pi.on("message_end", async (event: any) => {
    try {
      if (!AUTO_STORE) return;
      const message = event?.message;
      if (roleOf(message) !== "assistant" && roleOf(message) !== "model") return;
      if (!isFinalAnswer(message)) return;
      const assistant = textOf(message);
      if (!assistant) return;
      const stored = await storeExchange(pendingPrompt, assistant, "message_end");
      pendingPrompt = "";
      if (stored) await afterStore();
    } catch (e: any) {
      log("warn", "[microscope-memory] message_end failed: " + (e?.message || e));
    }
  });

  pi.on("agent_end", async (event: any) => {
    try {
      if (!AUTO_STORE) return;
      const { user, assistant } = extractExchange(event?.messages);
      const stored = await storeExchange(user, assistant, "agent_end");
      if (stored) await afterStore();
    } catch (e: any) {
      log("warn", "[microscope-memory] agent_end failed: " + (e?.message || e));
    }
  });

  pi.on("session_stop", async (event: any, ctx: any) => {
    try {
      if (!AUTO_STORE) return;
      if (!isMainSession(ctx)) return;
      if (!sessionId) sessionId = String(event?.session_id || "");
      const fromEvent = extractExchange(event?.messages);
      const user = fromEvent.user;
      const assistant = fromEvent.assistant || textOf(event?.last_assistant_message);
      const stored = await storeExchange(user, assistant, "session_stop");
      if (stored) await afterStore();
    } catch (e: any) {
      log("warn", "[microscope-memory] session_stop failed: " + (e?.message || e));
    }
  });

  // --- best effort final flush (may never fire: people just quit) ---------
  pi.on("session_shutdown", async () => {
    try {
      if (AUTO_STORE && exchanges > 0) {
        await writeSessionDigest("shutdown", false);
      }
    } catch {}
    mcp.stop();
  });

  // --- commands ------------------------------------------------------------
  pi.registerCommand("mscope", {
    description: "Microscope Memory: show the current auto-context snapshot",
    handler: async (_args: string, ctx: any) => {
      const text = (await mcp.tool("memory_auto_context", {}, 20000)).trim() || "(no context available)";
      try {
        pi.sendMessage(
          {
            customType: "microscope-context",
            content: "[MICROSCOPE MEMORY - auto-context snapshot]\n" + text,
            display: true,
            attribution: "agent",
          },
          { triggerTurn: false },
        );
      } catch {}
      if (ctx?.hasUI) ctx.ui?.notify?.("Microscope Memory snapshot loaded", "info");
    },
  });

  pi.registerCommand("mscope-find", {
    description: "Microscope Memory: phrase search (usage: /mscope-find <phrase>)",
    handler: async (args: string, ctx: any) => {
      const query = String(args || "").trim();
      if (!query) {
        if (ctx?.hasUI) ctx.ui?.notify?.("usage: /mscope-find <phrase>", "warn");
        return;
      }
      const text = (await mcp.tool("memory_find", { query, k: 8 }, 20000)).trim() || "(none)";
      try {
        pi.sendMessage(
          {
            customType: "microscope-memory",
            content: "[MICROSCOPE MEMORY - find: " + query + "]\n" + text,
            display: true,
            attribution: "agent",
          },
          { triggerTurn: false },
        );
      } catch {}
    },
  });

  pi.registerCommand("mscope-store", {
    description: "Microscope Memory: store a memory (usage: /mscope-store <text>)",
    handler: async (args: string, ctx: any) => {
      const text = String(args || "").trim();
      if (!text) {
        if (ctx?.hasUI) ctx.ui?.notify?.("usage: /mscope-store <text>", "warn");
        return;
      }
      const out = await mcp.tool("memory_store", { text, layer: "long_term", importance: 7 }, 20000);
      if (ctx?.hasUI) ctx.ui?.notify?.("Microscope Memory: stored", "info");
      try {
        pi.sendMessage(
          {
            customType: "microscope-memory",
            content: "[MICROSCOPE MEMORY - stored]\n" + (out.trim() || "ok"),
            display: true,
            attribution: "agent",
          },
          { triggerTurn: false },
        );
      } catch {}
    },
  });

  pi.registerCommand("mscope-status", {
    description: "Microscope Memory: bridge and engine diagnostics",
    handler: async (_args: string, ctx: any) => {
      const status = await mcp.tool("memory_status", {}, 20000);
      const lines = [
        "bridge: enabled",
        "binary: " + (BIN || "NOT FOUND"),
        "config: " + (CONFIG || "(engine default)"),
        "mcp connected: " + mcp.alive,
        "auto-store: " + AUTO_STORE,
        "retrieval: " + RETRIEVAL,
        "exchanges stored: " + exchanges,
        "files touched: " + touchedFiles.size,
        "tool errors: " + toolErrors,
        "",
        status,
      ];
      try {
        pi.sendMessage(
          {
            customType: "microscope-memory",
            content: "[MICROSCOPE MEMORY - status]\n" + lines.join("\n"),
            display: true,
            attribution: "agent",
          },
          { triggerTurn: false },
        );
      } catch {}
      if (ctx?.hasUI) ctx.ui?.notify?.("Microscope Memory status sent", "info");
    },
  });

  // Consolidation is deliberately explicit: the bridge never triggers an
  // index rebuild on its own, because a rebuild of the 1.7M-block index takes
  // ~20 s and must not happen inside a hook handler. The engine's own
  // auto_rebuild (from other tools) consolidates the append log as well.
  pi.registerCommand("mscope-rebuild", {
    description: "Microscope Memory: consolidate the append log into the binary index (slow)",
    handler: async (_args: string, ctx: any) => {
      if (ctx?.hasUI) ctx.ui?.notify?.("Microscope Memory: rebuilding the index, this takes a while", "info");
      const started = Date.now();
      const out = await mcp.tool("memory_rebuild", {}, 300000);
      const text = out.trim() || "(no output)";
      try {
        pi.sendMessage(
          {
            customType: "microscope-memory",
            content:
              "[MICROSCOPE MEMORY - rebuild finished in " +
              Math.round((Date.now() - started) / 1000) +
              " s]\n" +
              text,
            display: true,
            attribution: "agent",
          },
          { triggerTurn: false },
        );
      } catch {}
      log("info", "[microscope-memory] rebuild finished in " + Math.round((Date.now() - started) / 1000) + " s");
    },
  });
}
