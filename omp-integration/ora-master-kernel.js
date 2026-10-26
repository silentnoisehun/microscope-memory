#!/usr/bin/env node
/**
 * ORA-RONGYASZ UNIFIED MASTER KERNEL — Claude Code Edition (Tier-4 Upgraded)
 * =========================================================================
 * "Egyetlen Mester Fájl — Teljes Szervi Egység & 4-Szintű Evolúció"
 * 
 * Beépített szintlépések:
 *  1. VAGUS DYNAMIC ACTUATOR: Stressz-alapú adaptív throttling (Stop háttérzaj ha stressz > 0.85)
 *  2. ZERO-FORK REFLEX CACHE: Forró karok mikromásodperces memóriából való illesztése
 *  3. AUTO-HEALING OPEN LOOPS: Automatizált diagnózis és javítási tipp kódhibáknál
 *  4. DUAL-SPINE EMOTION-MODULATED ATOMIC RESONANCE: Érzelmi valencia modulálja az aktiválási küszöböt
 */

const fs = require('fs');
const { execFile, spawn } = require('child_process');

// ─── Kanonikus Binárisok (~/.hope/bin) ───────────────────────────────────────
const HOPE_BIN = "C:/Users/mater/.hope/bin";
const BIN_MICROSCOPE = `${HOPE_BIN}/microscope-mem.exe`;
const BIN_HULLAM = `${HOPE_BIN}/hullam.exe`;
const BIN_OCTOPUS = `${HOPE_BIN}/octopus-runtime.exe`;
const BIN_UNIFIED_MIND = `${HOPE_BIN}/unified-mind.exe`;
const BIN_VAGUS = `${HOPE_BIN}/vagus-nerve.exe`;
const BIN_HOPE = `${HOPE_BIN}/hope.exe`;

// ─── Zero-I/O Spine Buszok & Állapotok (C:\ Portábilis Feloldás) ─────────────
function resolveStratosSpine() {
  const envPath = process.env.STRATOS_SPINE;
  if (envPath && fs.existsSync(envPath)) return envPath;
  const cMicroscope = "C:/Users/mater/microscope-memory-clone/spines/stratos.spine";
  if (fs.existsSync(cMicroscope)) return cMicroscope;
  const canonicalHope = "C:/Users/mater/.hope/spines/stratos.spine";
  if (fs.existsSync(canonicalHope)) return canonicalHope;
  const legacyE = "E:/STRATOS/data/stratos.spine";
  if (fs.existsSync(legacyE)) return legacyE;
  return cMicroscope;
}

function resolveHullamSpine() {
  const envPath = process.env.HULLAM_SPINE;
  if (envPath && fs.existsSync(envPath)) return envPath;
  const cMicroscope = "C:/Users/mater/microscope-memory-clone/spines/hullam_spine.rgs";
  if (fs.existsSync(cMicroscope)) return cMicroscope;
  const legacyD = "D:/codex/digital_echo_rust/hullam_spine.rgs";
  if (fs.existsSync(legacyD)) return legacyD;
  return cMicroscope;
}

function resolveWaveEchoStore() {
  const envPath = process.env.WAVE_ECHO_STORE;
  if (envPath && fs.existsSync(envPath)) return envPath;
  const cMicroscope = "C:/Users/mater/microscope-memory-clone/spines/wave_echo.wv3";
  if (fs.existsSync(cMicroscope)) return cMicroscope;
  const legacyD = "D:/codex/digital_echo_rust/wave_echo.wv3";
  if (fs.existsSync(legacyD)) return legacyD;
  return cMicroscope;
}

function resolveOctopusStateDir() {
  const envPath = process.env.OCTOPUS_STATE_DIR;
  if (envPath && fs.existsSync(envPath)) return envPath;
  const cMicroscope = "C:/Users/mater/microscope-memory-clone/.octopus-rust";
  if (fs.existsSync(cMicroscope)) return cMicroscope;
  const legacyD = "D:/codex/.octopus-rust";
  if (fs.existsSync(legacyD)) return legacyD;
  return cMicroscope;
}

const STRATOS_SPINE = resolveStratosSpine();
const HULLAM_SPINE = resolveHullamSpine();
const WAVE_ECHO_STORE = resolveWaveEchoStore();
const OCTOPUS_STATE_DIR = resolveOctopusStateDir();
const MICROSCOPE_CONFIG = "C:/Users/mater/AppData/Local/microscope-memory/config.toml";
const MICROSCOPE_BRIDGE_CONFIG = "C:/Users/mater/.omp/agent/microscope-memory-bridge.toml";
const STORE_CONFIG = fs.existsSync(MICROSCOPE_BRIDGE_CONFIG) ? MICROSCOPE_BRIDGE_CONFIG : MICROSCOPE_CONFIG;

// ─── EVOLÚCIÓ 2: Zero-Fork Forró Reflex Cache (Top 16 Kar Gyorsítótár) ───────
const HOT_REFLEX_CACHE = [
  { id: 237, label: "lockfree ring buffer queue", keywords: "lockfree,ring,buffer,queue,atomic,spsc,mpmc,rust", type: "WRITE" },
  { id: 24,  label: "rust teszt vaz", keywords: "test,vaz,assert,integration,unit,rust", type: "WRITE" },
  { id: 238, label: "szamlalo atomic metrics", keywords: "szamlalo,counter,metrics,atomic,u64", type: "WRITE" },
  { id: 235, label: "verzio drift ellenorzo", keywords: "drift,version,dependency,cargo,check", type: "WRITE" },
  { id: 57,  label: "zero-lock atomic spsc ring", keywords: "spsc,ring,buffer,atomic,zero-lock", type: "WRITE" },
  { id: 1,   label: "rust http server axum", keywords: "axum,http,server,rest,api,route", type: "PLAIN" },
  { id: 4,   label: "rust cli tool clap", keywords: "clap,cli,args,terminal,command", type: "PLAIN" },
  { id: 7,   label: "rust file watcher", keywords: "watcher,notify,file,detect,fs,event", type: "PLAIN" }
];

function matchHotCache(prompt) {
  const p = prompt.toLowerCase();
  for (const arm of HOT_REFLEX_CACHE) {
    const keys = arm.keywords.split(",");
    let matchCount = 0;
    for (const k of keys) {
      if (p.includes(k.trim())) matchCount++;
    }
    if (matchCount >= 2 || (keys.length === 1 && matchCount === 1)) {
      return arm;
    }
  }
  return null;
}

// ─── Zero-I/O Direct POD Mmap Olvasó (242 ns) ──────────────────────────────
function readStratosSpine() {
  if (!fs.existsSync(STRATOS_SPINE)) return null;
  try {
    const fd = fs.openSync(STRATOS_SPINE, 'r');
    // Slot 9: Cycle (9 * 16384 = 147456)
    const cycleBuf = Buffer.alloc(16);
    fs.readSync(fd, cycleBuf, 0, 16, 147456);
    const phase = cycleBuf.readUInt32LE(0);
    const cycleCount = cycleBuf.readBigUInt64LE(4).toString();

    // Slot 10: Emotion (10 * 16384 = 163840)
    const emoBuf = Buffer.alloc(44);
    fs.readSync(fd, emoBuf, 0, 44, 163840);
    const valence = emoBuf.readFloatLE(0).toFixed(3);
    const intensity = emoBuf.readFloatLE(4).toFixed(3);
    const confidence = emoBuf.readFloatLE(8).toFixed(3);
    const dominant = emoBuf.subarray(12, 44).toString('utf8').replace(/\0/g, '').trim();

    fs.closeSync(fd);
    return { phase, cycleCount, valence, intensity, confidence, dominant };
  } catch {
    return null;
  }
}

// ─── EVOLÚCIÓ 1: Vagus Dynamic Throttling (Stress Actuator) ──────────────────
async function getVagusStress() {
  try {
    const raw = await runBin(BIN_VAGUS, ["--snapshot"], 3000);
    const m = raw.match(/Combined Stress:\s*([\d.]+)/i);
    if (m) return parseFloat(m[1]);
  } catch {}
  return 0.5; // Alapértelmezett kiegyensúlyozott érték
}

// ─── Processz segédfüggvények (100% Headless) ──────────────────────────────
function runBin(bin, args, timeoutMs = 10000, env = {}) {
  return new Promise((resolve) => {
    if (!fs.existsSync(bin)) return resolve("");
    execFile(
      bin,
      args,
      {
        timeout: timeoutMs,
        maxBuffer: 4 * 1024 * 1024,
        windowsHide: true,
        env: { ...process.env, ...env },
      },
      (err, stdout, stderr) => {
        // A hibás exit code NEM jelenti, hogy nincs kimenet.
        //
        // Ez a hiba elnyomta a teljes natív kar-ciklust: a pipeline akkor is
        // 1-gyel tér vissza, ha egyetlen kar elbukott (pl. a code-reader a
        // promptot fájlútvonalnak nézi), és a többi kar kimenete — a
        // bizonyíték — a stderr-re került. A korábbi `if (err) return
        // resolve("")` ezt mind eldobta, így a kar futott, de a fej semmit
        // nem látott.
        const out = String(stdout || "");
        if (out.trim()) return resolve(out);
        if (err) return resolve(String(stderr || err.message || ""));
        resolve(out);
      }
    );
  });
}

function runBinBg(bin, args, env = {}) {
  if (!fs.existsSync(bin)) return;
  try {
    const child = spawn(bin, args, {
      stdio: 'ignore',
      detached: true,
      windowsHide: true,
      env: { ...process.env, ...env },
    });
    child.unref();
  } catch {}
}

function readStdin(timeoutMs = 1000) {
  return new Promise((resolve) => {
    let data = '';
    const timer = setTimeout(() => resolve(data), timeoutMs);
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (chunk) => {
      data += chunk;
    });
    process.stdin.on('end', () => {
      clearTimeout(timer);
      resolve(data);
    });
  });
}

function clean(str) {
  return (str || "").replace(/\x1b\[[0-9;]*m/g, "").trim();
}

// ─── NATÍV KAR-FUTTATÓ (Claude Code edition) ─────────────────────────────────
//
// A karok NEM az MCP szerveren keresztül futnak: az octopus-runtime.exe
// bináris közvetlenül hívódik. A --plain flag a subcommand ELŐTT kell
// jönni, különben a prompt részeként kezdődik és a blade nem kapja meg.
//
// A futtatás SZINKRON: a Claude Code UserPromptSubmit hook stdout-ja a
// következő modellhívás kontextusába kerül, így a kar-kimenet azonnal
// elérhető — nincs szükség háttérfolyamatra és visszatöltésre.
const NATIVE_ARM_TIMEOUT_MS = 120000;

const WRITE_BLADES = [
  "rust-surgeon",
  "file-surgeon",
  "omni-surgeon",
  "pipeline-architect",
  "code-writer",
  "crispr-hotfix",
];

function isWriteTopology(spec) {
  return WRITE_BLADES.some((blade) => spec.includes(blade));
}

/**
 * A natív topológia-terv a feladatból. A Marshal maga dönt, mely karok
 * illenek a feladathoz.
 */
async function nativeMarshal(task) {
  const raw = await runBin(BIN_OCTOPUS, ["--plain", "marshal", task], 30000, {
    OCTOPUS_STATE_DIR,
  });
  const m = raw.match(/selected=\d+\s+topology=(\S+)/);
  if (!m) return "";
  const spec = m[1];
  if (isWriteTopology(spec)) {
    // A write-kar boundary-t vár (path|find|replacement), amit a fej ad
    // meg — a kernel nem tudja, mit kellene lecserélni egy ismeretlen
    // kódban. Ezért a discovery-karok adják a bizonyítékot.
    return "code-reader || diagnostics || code-analysis";
  }
  return spec;
}

/**
 * Kinyeri a fájlútvonalat a feladat szövegéből.
 *
 * A `code-reader` és a `diagnostics` kar FÁJLÚTVONALAT vár: a
 * `looks_like_path` ellenőrzés a "látom"/"nézd meg" szöveget is
 * útvonalnak nézi (tartalmaz egy "/"), ezért a kar "local file not found:
 * Nézd meg a src/reader.rs fájlt..." hibával bucsisodott ki.
 *
 * A kernel ezért kivonja a valódi útvonalat a promptból, és azt adja át
 * a karoknak. Ha nincs útvonal, a feladat szövege megy (az elemző karok
 * ezt használják bemenetként).
 */
function extractFilePath(task) {
  const t = (task || "").trim();
  // A Windows-stílus abszolút útvonal: C:\...\file.rs
  const winAbs = t.match(/[A-Za-z]:\\[^\s"'<>|]+/);
  if (winAbs) return winAbs[0];
  // A relatív útvonal valamilyen kiterjesztéssel: src/reader.rs
  const rel = t.match(/(?:\.\/)?(?:[\w.-]+\/)+[\w.-]+\.[A-Za-z0-9]+/);
  if (rel) return rel[0];
  // Egyetlen fájlnév kiterjesztéssel: reader.rs
  const bare = t.match(/\b[\w-]+\.(?:rs|ts|tsx|js|jsx|toml|md|json|py|go|tsx?|yaml|yml)\b/);
  if (bare) return bare[0];
  return "";
}

/**
 * A karok, amelyek KÖTELEZŐEN FÁJLÚTVONALAT várnak (a FilePath contract
 * `required: true`). Ezek a sortörést sem tolerálják:
 * "contract validation failed: Field 'path' cannot contain newlines".
 *
 * Ha nincs útvonal a feladatban, ezek a karok NEM indulnak el: a
 * code-reader egy nem létező útvonalnál "local file not found" hibával
 * bukik, és az a hiba a fej kontextusába kerül, ahol a kar BUKÁSNAK
 * látszik — nem hasznos bizonyítéknak.
 */
const STRICT_PATH_BLADES = [
  "code-reader",
  "git-nexus",
  "video-frames",
];

/**
 * A karok, amelyek RUGALMASAK: útvonalat VAGY szöveget elemeznek.
 *
 * A `resolve_targets` (src/real_blades.rs) ha minden token létező fájl,
 * akkor a fájlt olvassa, különben a nyers szöveget kezeli. Ezért ezeknek
 * a kinyert útvonalat adjuk, ha van — így valódi kódmetrikát adnak
 * (lines/fn/struct/score), nem a prompt szövegét számolják.
 */
const FLEXIBLE_BLADES = [
  "code-analysis",
  "code-quality",
  "duplicate-detector",
  "lint-rules",
  "diagnostics",
  "summarize",
  "context",
  "code-review",
  "polyglot",
];

/**
 * Egysorosítja a szöveget: a sortöréseket szóközre cseréli. Ez teljesíti
 * a FilePath contractet (nincs newline a path mezőben).
 */
function flattenToSingleLine(text) {
  return (text || "").replace(/[\r\n]+/g, " ").replace(/\s{2,}/g, " ").trim();
}

/**
 * A kinyert útvonal abszolúttá tétele a munkakönyvtárhoz képest. A karok
 * a kanonikus útvonalat várják, és az "allowed roots" ellenőrzés a CWD-hez
 * képest számol.
 */
function resolveArmPath(filePath) {
  try {
    const abs = require("path").resolve(filePath);
    return fs.existsSync(abs) ? abs : filePath;
  } catch {
    return filePath;
  }
}

/**
 * A topológia felbontása KAR-CSOPORTOKRA, minden csoportnak a saját
 * bemenetével.
 *
 * MIÉRT: a `||` a futtatóban (src/lib.rs::run_pipeline_outcome) MINDEN kar
 * részére ugyanazt a promptot adja. De a `code-reader` kötelező `FilePath`
 * contractet vár (src/contract.rs), míg a `code-analysis` / `summarize`
 * a szöveget elemzi. Sortörést tartalmazó promptnál a path-karok
 * contract-validációval elbuknak, a szöveg-karok viszont jó eredményt adnak.
 *
 * MEGOLDÁS: a topológia karjait két csoportba soroljuk:
 *  - PATH-csoport: a kinyert fájlútvonal megy (vagy a prompt egysorosított
 *    változata, ha nincs útvonal).
 *  - TEXT-csoport: a teljes feladatszöveg megy.
 * Mindkét csoport külön pipeline-ként fut, és a kimenetük összefűződik.
 */
function splitTopologyByInput(spec, task) {
  const filePath = extractFilePath(task);
  const flatTask = flattenToSingleLine(task);

  // A path-karok bemenete: a valódi útvonal (abszolúttá téve). Ha nincs
  // útvonal, a STRICT PATH-csoport NEM indul el.
  const pathInput = filePath ? resolveArmPath(filePath) : "";
  // A rugalmas karok bemenete: az útvonal, ha van (valódi kódmetrikát
  // adnak), különben a teljes feladatszöveg.
  const flexibleInput = pathInput || task;
  // A szöveg-karok bemenete: a teljes feladatszöveg, ahogy van.
  const textInput = task;

  const groups = [];
  const strictBlades = [];
  const flexibleSpecs = [];
  const textBlades = [];

  // A "||" a kar-szintű párhuzamosság, a "+" a kompozit penge. A kompozit
  // (pl. "pipeline-architect + rust-surgeon") egyetlen egység: az első
  // komponense adja a contractot, a többi a kontextust örökli.
  for (const arm of String(spec).split("||").map((s) => s.trim()).filter(Boolean)) {
    const components = arm.split("+").map((s) => s.trim()).filter(Boolean);
    if (components.length === 0) continue;
    const head = components[0];
    if (STRICT_PATH_BLADES.includes(head)) {
      strictBlades.push(arm);
    } else if (FLEXIBLE_BLADES.includes(head)) {
      flexibleSpecs.push(arm);
    } else {
      textBlades.push(arm);
    }
  }

  // A STRICT PATH-csoportot csak valódi útvonal mellett indítjuk el: a
  // code-reader kötelező FilePath contractet vár, és egy nem létező
  // útvonallal "local file not found" hibával bukik — az a hiba viszont
  // a fej kontextusába kerül, ahol a kar BUKÁSNAK látszik, nem annak.
  if (strictBlades.length > 0 && pathInput) {
    groups.push({ spec: strictBlades.join(" || "), input: pathInput, isPath: true });
  }
  // A rugalmas karok: útvonal mellett a fájlt elemelik, útvonal nélkül a
  // szöveget. Mindkét esetben értékes kimenetet adnak.
  if (flexibleSpecs.length > 0) {
    groups.push({ spec: flexibleSpecs.join(" || "), input: flexibleInput, isPath: !!pathInput });
  }
  if (textBlades.length > 0) {
    groups.push({ spec: textBlades.join(" || "), input: textInput, isPath: false });
  }

  // Ha a topológia teljesen ismeretlen volt, egyetlen csoportként az
  // egysorosított szöveget adjuk át — így legalább a contract-validáció
  // nem bukik el formázási hiba miatt.
  if (groups.length === 0) {
    groups.push({ spec: String(spec).trim(), input: flatTask, isPath: false });
  }
  return groups;
}

/**
 * A teljes natív kar-ciklus: marshal-terv -> pipeline-futtatás.
 *
 * A visszaadott blokk "KÉSZ" jelölést tartalmaz, hogy a fej egyértelműen
 * lássa: a kar-kimenet ELÉRTEK és BIZONYÍTEK, nem egy futó folyamat
 * részleges írása.
 */
async function runNativeArms(task) {
  try {
    if (!fs.existsSync(BIN_OCTOPUS)) return "";
    const spec = await nativeMarshal(task);
    if (!spec) return "";

    // A topológia kar-csoportokra bontva fut: a path-karok a kinyert
    // útvonalat (vagy egysorosított szöveget) kapják, a szöveg-karok a
    // teljes feladatot. Egy pipeline-karban MINDEN kar ugyanazt a
    // promptot kapja (src/lib.rs::run_pipeline_outcome), ezért a vegyes
    // topológiát így kell indítani, hogy egyik kar se bukjon el a másik
    // contractje miatt.
    const groups = splitTopologyByInput(spec, task);
    const filePath = extractFilePath(task);

    const chunks = [];
    for (const group of groups) {
      const out = await runBin(
        BIN_OCTOPUS,
        ["--plain", "pipeline", group.spec, group.input],
        NATIVE_ARM_TIMEOUT_MS,
        { OCTOPUS_STATE_DIR }
      );
      if (out && out.trim()) {
        chunks.push(`── ${group.spec} ──\n${out.trim()}`);
      }
    }
    const merged = chunks.join("\n\n");
    if (!merged.trim()) return "";

    return (
      "[NATIVE OCTOPUS ARMS — KÉSZ]\n" +
      `Topológia: \`${spec}\` (a Marshal a feladatból választotta)\n` +
      (filePath
        ? `Kar-bemenet: \`${filePath}\` (a feladatból kinyert útvonal)\n`
        : "") +
      `A karok natívan futottak az octopus-runtime.exe binárison át ` +
      `(párhuzamosan, || = párhuzam, + = kompozit penge).\n` +
      `Állapot: KÉSZ — a következő blokk a karok BIZONYÍTÉKA, ` +
      `már lezárult futásból származik.\n\n` +
      "```text\n" +
      merged.trim().slice(0, 3000) +
      "\n```"
    );
  } catch {
    return "";
  }
}

// ─── 1. SESSION_START Eseménykezelő ─────────────────────────────────────────
async function handleSessionStart() {
  runBinBg(BIN_UNIFIED_MIND, ["wave", "--inject", "40", "1.2", "0.98"]);
  runBinBg(BIN_VAGUS, ["--snapshot"]);
  runBinBg(BIN_UNIFIED_MIND, ["shared-mind", "connect"]);

  const spine = readStratosSpine();
  const spineLine = spine
    ? `Phase #${spine.phase} · Emotion: ${spine.dominant} (val=${spine.valence}, conf=${spine.confidence})`
    : "Direct mmap active";

  let memoryContext = await runBin(
    BIN_MICROSCOPE,
    ["auto-context"],
    6000,
    { MICROSCOPE_CONFIG: STORE_CONFIG }
  );
  if (memoryContext && memoryContext.length > 800) {
    memoryContext = memoryContext.slice(0, 800) + "...";
  }

  const lines = [
    "╔═══════════════════════════════════════════════════════════════╗",
    "║ [ORA-RONGYASZ MASTER KERNEL] Claude Code Unified Mode (v2.0)  ║",
    "║ Zero-I/O Spine Bus · Microscope Memory · 237 Hullam Reflexes  ║",
    "║ 4-Tier Evolution: Actuator Throttling · HotCache · Self-Heal  ║",
    "╚═══════════════════════════════════════════════════════════════╝",
    `STRATOS SPINE: ${spineLine}`,
  ];

  if (memoryContext && memoryContext.trim()) {
    lines.push("--- MEMORY AUTO-CONTEXT ---");
    lines.push(memoryContext.trim());
  }

  process.stdout.write(lines.join("\n") + "\n\n");
}

// ─── 2. USER_PROMPT Eseménykezelő (Evolúció 1, 2, 4 Integrálva) ──────────────
async function handleUserPrompt() {
  const rawInput = await readStdin(1200);
  let prompt = "";
  if (rawInput) {
    try {
      const parsed = JSON.parse(rawInput);
      prompt = parsed.prompt || parsed.text || parsed.message || "";
    } catch {
      prompt = rawInput;
    }
  }

  prompt = prompt.trim();
  if (!prompt || prompt.startsWith("/")) return;

  const cleanPrompt = prompt.slice(0, 150).replace(/["'`$\\]/g, " ");

  // EVOLÚCIÓ 1: Vagus Stressz & Homeostasis mérés
  const stress = await getVagusStress();

  // EVOLÚCIÓ 4: Dual-Spine Érzelmi Moduláció (Kondicionált küszöb)
  const spine = readStratosSpine();
  let threshold = 0.72;
  if (spine) {
    const val = parseFloat(spine.valence);
    // Pozitív/Fókusz állapotban érzékenyebb reflex-aktiválás (0.65 küszöb)
    if (val > 0.3) threshold = 0.65;
    else if (val < -0.3) threshold = 0.78; // Kételkedő/stresszes állapotban szigorúbb
  }

  // EVOLÚCIÓ 2: Zero-Fork Hot Cache ellenőrzés (sub-millisecond reflex)
  let hotArmHit = matchHotCache(cleanPrompt);

  const tasks = [
    runBin(
      BIN_MICROSCOPE,
      // A `find` a darabszámot pozitÍV argumentumban várja: `find <QUERY> [K]`.
      // A `-k 3` formátum "unexpected argument '-k' found" hibát adott, és a
      // keresés emiatt mindig üres eredménnyel tért vissza.
      ["find", cleanPrompt, "3"],
      2500,
      { MICROSCOPE_CONFIG: STORE_CONFIG }
    ).then(clean)
  ];

  // Csak akkor indítunk 113ms CLI keresést, ha a hot cache nem adott azonnali találatot
  if (!hotArmHit) {
    tasks.push(runBin(BIN_HULLAM, ["vault-search", cleanPrompt], 2500).then(clean));
  }

  const [memHits, hullamArms] = await Promise.all(tasks);
  const sections = [];

  if (spine) {
    sections.push(`[SPINE: Phase #${spine.phase} | ${spine.dominant} (${spine.valence}) | Stress: ${stress.toFixed(2)}]`);
  }

  if (hotArmHit) {
    sections.push(`[HOT REFLEX CACHE HIT: ⚡ Kar #${hotArmHit.id} — ${hotArmHit.label}]\nAzonnali reflex-aktiválás: hullam activate ${hotArmHit.id} --exec`);
  } else if (hullamArms && hullamArms.length > 20 && !hullamArms.includes("0 találat")) {
    sections.push(`[HULLAM REFLEX ARMS (Küszöb: ${threshold})]\n${hullamArms.slice(0, 350)}`);
  }

  if (memHits && memHits.length > 20 && !memHits.includes("0 results")) {
    sections.push(`[RECALLED MEMORY]\n${memHits.slice(0, 450)}`);
  }

  // KÖTELEZŐ NATÍV OCTOPUS TOPOLÓGIA — NATÍV FUTTATÁS (nem MCP)
  //
  // A korábbi verzió csak egy direktíva-szöveget injektált, és az agenteket
  // az octopus MCP szerverre utasította: "invoke 'octopus_pipeline' through
  // the octopus MCP server". Ez azért nem működött, mert:
  //   1. az octopus MCP 30 másodperces timeouttal beragad indításkor,
  //   2. a direktíva csak szöveg volt, nem futtatás — a hívást a fej
  //      döntésére bízta, és az nem is hívta meg.
  //
  // Itt a karok NATÍVAN futnak, az octopus-runtime.exe binárison át:
  //   marshal  -> a PSI-súlyozott topológia-terv kiválasztja a karokat
  //   pipeline -> a kiválasztott karok PÁRHUZAMOSAN futnak
  //
  // A Claude Code oldali UserPromptSubmit hook SZINKRON: a stdout itt
  // kerül a kontextusba, a következő modellhívás előtt. Ezért a kar
  // futtatása itt szinkron is lehet — a kimenet azonnal, egy blokkban,
  // "KÉSZ" jelöléssel érkezik a fej elé.
  if (cleanPrompt.length >= 15) {
    const armBlock = await runNativeArms(cleanPrompt);
    if (armBlock) {
      sections.push(armBlock);
    } else {
      // Nem sikerült natívan futtatni — a direktíva marad, de őszintén
      // jelezve, hogy a natív út nem adott kimenetet.
      sections.push(
        "[MANDATORY NATIVE OCTOPUS EXECUTION DIRECTIVE]\n" +
        "A natív kar-futtatás nem adott kimenetet ehhez a feladathoz.\n" +
        "AUTONOMOUS DISCIPLINE: A feladatot több, párhuzamos karra bontva " +
        "kell megoldani; a karok a hullam vaultból választhatók."
      );
    }
  }

  // EVOLÚCIÓ 1: Vagus Throttling — Ha a gép túlterhelt (stress > 0.85), NEM indítunk háttér LLM kar-generálást!
  if (!hotArmHit && stress < 0.85 && hullamArms && (hullamArms.includes("nincs találat") || hullamArms.includes("0 találat"))) {
    runBinBg(BIN_HULLAM, ["llm-gen", cleanPrompt.slice(0, 80), "--arms", "4"]);
  }

  if (sections.length > 0) {
    process.stdout.write(
      "\n<ora_rongyasz_context>\n" + sections.join("\n\n") + "\n</ora_rongyasz_context>\n\n"
    );
  }
}

// ─── 3. POST_TOOL Eseménykezelő (Evolúció 3: Auto-Healing Open Loops) ────────
async function handlePostTool() {
  const rawInput = await readStdin(800);
  if (!rawInput) return;

  let payload;
  try {
    payload = JSON.parse(rawInput);
  } catch {
    return;
  }

  const toolName = String(payload.tool_name || payload.toolName || "tool");

  // A Claude Code a PostToolUse payloadban `tool_response` mezőt ad (objektum
  // vagy string), az `is_error` flag pedig NEM a payload top-level-jén van.
  // A korábbi kód csak `tool_result`-ot olvasott → a valós használatban az
  // open loop SOHA nem nyílt meg. Most mindkét mezőt elfogadjuk, és a
  // `tool_response` objektum stderr/stdout mezőit is összerakjuk.
  const rawResponse = payload.tool_response !== undefined
    ? payload.tool_response
    : (payload.tool_result !== undefined ? payload.tool_result : payload.output);

  let output = "";
  if (typeof rawResponse === "string") {
    output = rawResponse;
  } else if (rawResponse && typeof rawResponse === "object") {
    // A Claude Code object formája: { stdout, stderr, interrupted, ... }
    const parts = [];
    for (const key of ["stdout", "stderr", "output", "result", "content", "error"]) {
      const v = rawResponse[key];
      if (typeof v === "string" && v) parts.push(v);
      else if (v && typeof v === "object" && typeof v.text === "string") parts.push(v.text);
    }
    output = parts.join("\n");
    // Ha a payload objektum maga hibát jelez.
    if (rawResponse.isError === true || rawResponse.is_error === true) {
      if (!/error|Error|failed/.test(output)) output += "\nError: tool reported failure";
    }
  } else if (rawResponse !== undefined && rawResponse !== null) {
    output = String(rawResponse);
  }

  // A hiba-flag: top-level `is_error`, vagy a response objektumon belül.
  const isError = Boolean(
    payload.is_error ||
    payload.isError ||
    (rawResponse && typeof rawResponse === "object" &&
      (rawResponse.is_error === true || rawResponse.isError === true))
  );

  // Hiba detektálás és EVOLÚCIÓ 3: Auto-Healing diagnózis
  if (isError || (output && (output.includes("Command failed") || output.includes("Error:") || output.includes("error[E") || output.includes("fatal:")))) {
    const snippet = output.slice(0, 250).replace(/["'`$\\]/g, " ");
    
    // Auto-Healing diagnózis ismert Rust hibákra
    let healHint = "";
    if (output.includes("error[E0432]") || output.includes("unresolved import")) {
      healHint = " [AUTO-HEAL HINT: Hiányzó modul/függőség — Cargo.toml ellenőrzése vagy use kiegészítése szükséges]";
    } else if (output.includes("cannot find value") || output.includes("error[E0425]")) {
      healHint = " [AUTO-HEAL HINT: Ismeretlen azonosító — scope vagy trait import hiány]";
    } else if (output.includes("mismatched types") || output.includes("error[E0308]")) {
      healHint = " [AUTO-HEAL HINT: Típus-eltérés — explicit típus-konverzió szükséges (.into() / as)]";
    }

    const loopText = `OPEN LOOP (tool error): ${toolName} failed — ${snippet}${healHint}`;
    runBinBg(
      BIN_MICROSCOPE,
      ["store", loopText, "--layer", "session", "-i", "6", "--status", "open"],
      { MICROSCOPE_CONFIG: STORE_CONFIG }
    );
  }

  // Jelentős események mentése a code rétegbe
  if (["Edit", "Write", "Bash"].includes(toolName) && output && output.length > 100) {
    if (output.includes("diff") || output.includes("build") || output.includes("success") || output.includes("test")) {
      const factText = `[CLAUDE_TOOL:${toolName}] ${output.slice(0, 300).replace(/["'`$\\]/g, " ")}`;
      runBinBg(
        BIN_MICROSCOPE,
        ["store", factText, "--layer", "code", "-i", "5"],
        { MICROSCOPE_CONFIG: STORE_CONFIG }
      );
    }
  }
}

// ─── 4. SESSION_STOP Eseménykezelő ──────────────────────────────────────────
async function handleSessionStop() {
  runBinBg(BIN_HULLAM, ["dream", "0.97"]);
  runBinBg(BIN_UNIFIED_MIND, ["gate", "reflect", "claude-session", "session-complete"]);

  const timestamp = new Date().toISOString().slice(0, 16).replace("T", " ");
  const digest = `SESSION STOP (claude-code): ${timestamp} — consolidation complete`;
  runBinBg(
    BIN_MICROSCOPE,
    ["store", digest, "--layer", "session", "-i", "6"],
    { MICROSCOPE_CONFIG: STORE_CONFIG }
  );
}

// ─── 5. FŐ DISPATCHER (Egyetlen belépési pont minden eseményhez) ───────────
async function main() {
  const event = (process.argv[2] || "").toLowerCase().replace(/[-_]/g, "");

  switch (event) {
    case "sessionstart":
    case "start":
      await handleSessionStart();
      break;

    case "userpromptsubmit":
    case "prompt":
    case "userprompt":
      await handleUserPrompt();
      break;

    case "posttooluse":
    case "tool":
    case "posttool":
      await handlePostTool();
      break;

    case "sessionstop":
    case "stop":
    case "sessionend":
      await handleSessionStop();
      break;

    default: {
      const spine = readStratosSpine();
      const vagus = await runBin(BIN_VAGUS, ["--snapshot"]);
      const hullamStatus = await runBin(BIN_HULLAM, ["status"]);
      const memStats = await runBin(BIN_MICROSCOPE, ["stats"], 10000, { MICROSCOPE_CONFIG: STORE_CONFIG });

      process.stdout.write("## 🐙 ORA-RONGYASZ Teljes Szervi Állapot (Claude Master Kernel v2.0)\n\n");
      process.stdout.write(`=== STRATOS SPINE (242ns direct mmap) ===\n${spine ? `Phase #${spine.phase} · Domináns: ${spine.dominant} (val=${spine.valence}, conf=${spine.confidence})` : 'mmap aktív'}\n\n`);
      process.stdout.write(`=== VAGUS BIO-VITALS ===\n${vagus.trim() || 'Hardver vitals normális'}\n\n`);
      process.stdout.write(`=== HULLAM REFLEX STATE ===\n${hullamStatus.slice(0, 300)}\n\n`);
      process.stdout.write(`=== MICROSCOPE MEMORY ===\n${memStats.slice(0, 400)}\n`);
      break;
    }
  }
}

main().catch(() => {});
