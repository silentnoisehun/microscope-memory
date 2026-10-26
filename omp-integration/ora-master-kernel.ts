/**
 * ORA-RONGYASZ UNIFIED MASTER KERNEL — Complete Native Engine & Hope Superstrate
 * ==============================================================================
 *
 * "Ora + Rongyász + Hope = Teljes Kognitív Egység"
 * 
 * Ez a TELJES összevont fő-hook, ami ZÉRÓ funkcióvesztéssel és zéró redundanciával
 * egyesíti az Ora-Rongyász ökoszisztéma és a Hope kognitív szuperstruktúra összes szervét:
 *  1. ORA KOGNITÍV KERNEL — Érzelmi rezonancia, Hebbian kapcsolatok, tudat (ψ formula)
 *  2. MICROSCOPE MEMORY   — auto-context, determinisztikus find, auto-loop, multi-layer store
 *  3. HULLAM & WAVE-ECHO  — 237+ reflex-kar, AVX2 SIMD interferencia (324µs döntés, LTP/LTD)
 *  4. OCTOPUS RUNTIME     — 150+ natív blade multi-arm orkesztráció (|| és + topológia)
 *  5. STRATOS MULTI-SPINE — 242ns közvetlen POD memóriabusz olvasás (ZERO szerver)
 *  6. VAGUS NERVE BIO     — CPU/RAM belső hardver érzékelés -> WaveField moduláció
 *  7. GITHUB SIPHON       — Autonóm kód-szippantás és gerincbe égetés (siphon-kar.ps1)
 *  8. MCP ROUTER          — Tanuló LLM modell-irányító proxy (:10102) minőségi visszacsatolással
 *  9. HOPE SUPERSTRATE    — Lucid Cycle (Álom→Mitózis→Hipotézis→Ledger), Falszifikációs Reflex,
 *                           Kognitív Morfogenezis (GAS/LIQUID/SOLID), Absentia (Csend Réteg),
 *                           Context Chain, Research Benchmark-first & SWTM
 */

import { execFile, spawn } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  openSync,
  readdirSync,
  readFileSync,
  readSync,
  closeSync,
  writeFileSync,
} from "node:fs";

const env = process.env;

// ─── Kanonikus Binárisok (C: Elsődleges) ─────────────────────────────────────
// A .hope\bin az EGYETLEN futtatási hely (persisted rule:
// "hope-bin-single-run-location"). A target\release csak forrás: ott a
// debug-szimbolumok növelik a méretet, és a bináris nem ugyanaz a build,
// amit a szervek hívnak. Ezért a target\release soha nem lehet preferált.
const HOPE_BIN = "C:/Users/mater/.hope/bin";
const BIN_MICROSCOPE = `${HOPE_BIN}/microscope-mem.exe`;
const BIN_HULLAM = `${HOPE_BIN}/hullam.exe`;
const BIN_WAVE_ECHO = `${HOPE_BIN}/wave-echo.exe`;
const BIN_OCTOPUS = `${HOPE_BIN}/octopus-runtime.exe`;
const BIN_UNIFIED_MIND = `${HOPE_BIN}/unified-mind.exe`;
const BIN_HOPE = `${HOPE_BIN}/hope.exe`;
const BIN_VAGUS = `${HOPE_BIN}/vagus-nerve.exe`;
const BIN_ROUTER = `${HOPE_BIN}/mcp-router.exe`;

// Siphon script (C: prioritás)
const SIPHON_SCRIPT = existsSync("C:/Users/mater/microscope-memory-clone/siphon-kar.ps1")
  ? "C:/Users/mater/microscope-memory-clone/siphon-kar.ps1"
  : `${HOPE_BIN}/siphon-kar.ps1`;

// ─── Spine Buszok & Állapotok (C:-First Feloldás) ────────────────────────────
// A STRATOS gerincét a stratos-server írja, és a --data-dir alá nyitja.
// A watchdog a <root>/stratos_data-et adja át, ezért a CANONICUS gerinc
// a stratos_data/stratos.spine — NEM a spines/ alatt lévő másolat, amit
// senki nem ír. A spines/ példány 2026-10-12-én állt és azóta nem
// frissült: a kernel eddig egy halott fájlt olvasott gerincnek.
//
// A sorrend: explicit env var -> a szerver által írt élő fájl -> a
// korábbi helyek (tartalék, olvasható marad).
function resolveStratosSpine(): string {
  const envPath = (process.env.STRATOS_SPINE || "").trim();
  if (envPath && existsSync(envPath)) return envPath;

  const live = "C:/Users/mater/microscope-memory-clone/stratos_data/stratos.spine";
  if (existsSync(live)) return live;

  const c1 = "C:/Users/mater/microscope-memory-clone/spines/stratos.spine";
  if (existsSync(c1)) return c1;
  const e1 = "E:/STRATOS/data/stratos.spine";
  if (existsSync(e1)) return e1;
  return live;
}

function resolveHullamSpine(): string {
  const envPath = (process.env.HULLAM_SPINE || "").trim();
  if (envPath && existsSync(envPath)) return envPath;

  const live = "D:/codex/digital_echo_rust/hullam_spine.rgs";
  if (existsSync(live)) return live;

  const c1 = "C:/Users/mater/microscope-memory-clone/spines/hullam_spine.rgs";
  if (existsSync(c1)) return c1;
  return live;
}

function resolveWaveEchoStore(): string {
  const envPath = (process.env.WAVE_ECHO_STORE || "").trim();
  if (envPath && existsSync(envPath)) return envPath;

  const live = "D:/codex/digital_echo_rust/wave_echo.wv3";
  if (existsSync(live)) return live;

  const c1 = "C:/Users/mater/microscope-memory-clone/spines/wave_echo.wv3";
  if (existsSync(c1)) return c1;
  return live;
}

// Az Octopus state store: itt élnek az arm-ok, a root-ok, a snap-ek és az
// events.log. A D: példány 1,7 GB és ma is íródik; a C: példány 6,8 MB
// és félig üres. A kernel a C:-t preferálta, így a karok más state store
// ellen futottak, mint a direkt hívások — ezért "nem működött semmi".
function resolveOctopusStateDir(): string {
  const envPath = (process.env.OCTOPUS_STATE_DIR || "").trim();
  if (envPath && existsSync(envPath)) return envPath;

  const live = "D:/codex/.octopus-rust";
  if (existsSync(live)) return live;

  const c1 = "C:/Users/mater/microscope-memory-clone/.octopus-rust";
  if (existsSync(c1)) return c1;
  return live;
}

const HULLAM_SPINE = resolveHullamSpine();
const STRATOS_SPINE = resolveStratosSpine();
const WAVE_ECHO_STORE = resolveWaveEchoStore();
const OCTOPUS_STATE_DIR = resolveOctopusStateDir();
const MICROSCOPE_CONFIG = "C:/Users/mater/AppData/Local/microscope-memory/config.toml";

const TRACE_FILE = "C:/Users/mater/.omp/agent/ora-kernel.log";

function trace(level: string, message: string) {
  try {
    appendFileSync(
      TRACE_FILE,
      `${new Date().toISOString()} [${level.toUpperCase()}] ${message}\n`,
      "utf8"
    );
  } catch {}
}

// ─── Szervi Kikapcsoló (Kísérleti Hibaelhárítás) ─────────────────────────────
const DISABLED_ORGANS = new Set(
  (process.env.ORA_DISABLE_ORGANS || "")
    .split(",")
    .map((s) => s.trim().toLowerCase())
    .filter(Boolean)
);

function organOff(name: string): boolean {
  return DISABLED_ORGANS.has(name.toLowerCase());
}

// ─── Biztonságos Bináris Futtatás (windowsHide: true KÖTELEZŐ) ───────────────
// A szervek mind ugyanazt a környezetet kapják: azonos state store, azonos
// spine, azonos memória-konfig. Ha ez nincs, a karok máshol járkálnak,
// mint ahol az adatok vannak.
function organEnv(): NodeJS.ProcessEnv {
  return {
    ...process.env,
    OCTOPUS_STATE_DIR,
    WAVE_ECHO_STORE,
    WAVE_ECHO_SPINE: HULLAM_SPINE,
    HULLAM_SPINE,
    STRATOS_SPINE,
    MICROSCOPE_CONFIG,
    HULLAM_ROOT: "C:/Users/mater/microscope-memory-clone",
  };
}

function runBin(bin: string, args: string[], timeoutMs = 15000): Promise<string> {
  return new Promise((resolve) => {
    if (!existsSync(bin)) return resolve("");
    execFile(
      bin,
      args,
      {
        env: organEnv(),
        windowsHide: true,
        maxBuffer: 4 * 1024 * 1024,
        timeout: timeoutMs,
      },
      (error, stdout, stderr) => {
        if (error && !stdout) {
          trace("error", `${bin} ${args[0]} failed: ${error.message}`);
          resolve(stderr || error.message);
        } else {
          resolve(stdout || stderr || "");
        }
      }
    );
  });
}

function runBinBg(bin: string, args: string[], timeoutMs = 60000): void {
  try {
    if (!existsSync(bin)) return;
    const child = spawn(bin, args, {
      env: organEnv(),
      windowsHide: true,
      detached: true,
      stdio: "ignore",
    });
    child.unref();
    if (timeoutMs > 0) {
      // A timer unref()-je kell, különben a Node event loop-ot tartja életben
      // és a hook folyamat nem tud kilépni a gyerek processz befejezése után.
      const timer = setTimeout(() => {
        try { child.kill(); } catch {}
      }, timeoutMs);
      timer.unref();
    }
  } catch (err: any) {
    trace("error", `${bin} ${args[0]} bg failed: ${err?.message || err}`);
  }
}

// ─── EVOLÚCIÓ 1: Vagus Dynamic Throttling (Stress Actuator) ──────────────────
async function getVagusStress(): Promise<number> {
  try {
    const raw = await runBin(BIN_VAGUS, ["--snapshot"], 3000);
    const m = raw.match(/Combined Stress:\s*([\d.]+)/i);
    if (m) return parseFloat(m[1]);
  } catch {}
  return 0.5;
}

// ─── NATÍV KAR-FUTTATÓ ───────────────────────────────────────────────────────
// A korábbi kernel a karakat MCP-n keresztül próbálta elindítani: a
// direktívát szövegként injektálta a kontextusba, és az octopus MCP
// szerver connection timeouttal halt meg. A kar tehát SOHA nem futott.
//
// Itt a kar NATIVAN indul el, az octopus-runtime.exe binárison át:
//   marshal  -> a PSI-súlyozott topológia-terv kiválasztja a karokat
//   pipeline -> a kiválasztott karok PÁRHUZAMOSAN futnak (|| = párhuzam,
//               + = kompozit pengék)
// A natív karok standard parancssávi felülete a subcommand-ot követi közvetlenül.
// A --plain globális flag NEM automatikusan jár minden parancshoz: a marshal/pipeline
// parancsok maguk ismerik a "plain" módot, de a flag helytelen átadása (pl. a prompt részeként)
// miatt a blade-ek 0.000-es súllyal futottak és a kar-ok MCP-n keresztül timeout-oltak.
// A megoldás: a --plain flag-öket KIVONJUK a globális listából, és csak a marshal/pipeline
// parancsoknál adjuk hozzá kézzel, a megfelelő pozícióban (a subcommand ELŐTT).
const OCTOPUS_GLOBAL_FLAGS: string[] = [];

// A futtatás hosszú: a pipeline 30-120 s. A hook processz nem tarthatja
// ezt feltétlenül nyitva, ezért a háttérben fut, és az eredmény a
// következő turn be injektálódik.
const NATIVE_ARM_TIMEOUT_MS = 180000;

interface NativeArmResult {
  spec: string;
  ok: boolean;
  output: string;
  ms: number;
}

// ─── ÍRÓ KAROK: A FEJ ÍRÁSA A FÁJLBA ────────────────────────────────────────
//
// A natív írókarok szerződése fix formátumú, és a tartalmat a FEJ adja:
//   rust-surgeon / pipeline-architect: path|keresendő|cserélendő
//   code-writer:                       path|expected_sha256_or_NEW|content
//
// Ezért a ciklus kétirányú:
//   1. DISCOVERY — a read-only karok bizonyítékot gyűjtenek
//   2. FEJ — az LLM megírja a cserét (a szerződés a válaszában)
//   3. WRITE — a kernel felismeri a szerződést és natívan futtatja az írót
//
// A 3. lépés nincs beégetve: az írókar a saját védelmi mechanizmusát
// (hash-ellenőrzés, tranzakciós backup, pontosan-egyszeri illesztés)
// megtartva ír.

const WRITE_CONTRACT_PATTERNS = [
  // ```octopus-write ... ``` vagy ```write ... ``` blokk
  /```(?:octopus-)?write\s*\n([\s\S]*?)```/gi,
  // [octopus-write] path|find|replacement
  /\[octopus-write\]\s*([^\n]+)/gi,
  // <octopus-write>path|find|replacement</octopus-write>
  /<octopus-write>([\s\S]*?)<\/octopus-write>/gi,
];

interface WriteContract {
  path: string;
  find: string;
  replacement: string;
}

/**
 * A fej válaszából kinyeri a write-szerződést.
 *
 * A formátum: path|keresendő szöveg|cserélendő szöveg
 * A "keresendő" és a "cserélendő" is több soros lehet, ezért a szétvágás
 * csak az első két "|" mentén történik — a maradék a replacement.
 */
function parseWriteContracts(text: string): WriteContract[] {
  const out: WriteContract[] = [];
  const seen = new Set<string>();
  for (const pattern of WRITE_CONTRACT_PATTERNS) {
    pattern.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = pattern.exec(text)) !== null) {
      const raw = m[1].trim();
      if (!raw) continue;
      // Az első "|" a path és a find között, a második a find és a
      // replacement között. A find/replacement tartalmazhat "|"-t, ezért
      // a határok fixen az első két pipe.
      const first = raw.indexOf("|");
      if (first < 0) continue;
      const second = raw.indexOf("|", first + 1);
      if (second < 0) continue;
      const path = raw.slice(0, first).trim();
      const find = raw.slice(first + 1, second).trim();
      const replacement = raw.slice(second + 1).trim();
      if (!path || !find) continue;
      const key = `${path}::${find}`;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push({ path, find, replacement });
    }
  }
  return out;
}

/**
 * A write-szerződés végrehajtása a natív írókarral.
 *
 * A .rs fájloknál a "pipeline-architect + rust-surgeon" kompozit penge
 * kell: az architect fogja meg a boundary-t (path|find|replacement),
 * a surgeon pedig elvégzi a tranzakciós vágást. A surgeon ÖNMAGÁBAN
 * nem fogad boundary-t — "no pipeline boundary contract" a hiba.
 *
 * A nem-.rs fájloknál a "code-writer" ír, de más szerződéssel:
 * path|expected_sha256_or_NEW|content. Ehhez a fájl teljes új tartalmát
 * kell ismerni, amit a fej nem ad meg — ezért most a .rs útvonal a
 * támogatott, és a többi fájlt a fej maga írja a szokásos eszközzel.
 */
async function runWriteContract(contract: WriteContract): Promise<string> {
  const prompt = `${contract.path}|${contract.find}|${contract.replacement}`;
  const isRust = contract.path.toLowerCase().endsWith(".rs");
  if (!isRust) {
    return (
      "[write] A natív írókar csak .rs fájlokra van bekötve. " +
      "A nem-.rs fájlokat a fej írja a szokásos eszközzel.\n" +
      `Cél: ${contract.path}`
    );
  }
  const spec = "pipeline-architect + rust-surgeon";
  trace("info", `write contract -> ${spec} ${contract.path}`);
  // A --plain flag a run parancsnak: a subcommand ELŐTT kell jönni.
  // A pipeline-architect + rust-surgeon kompozit pengéknél kritikus,
  // hogy a flag ne a prompt részeként kezdődjen, különben a surgeon
  // 0.000-es súllyal és helytelenül illeszkedik a kódban.
  return runBin(
    BIN_OCTOPUS,
    ["--plain", "run", spec, prompt],
    NATIVE_ARM_TIMEOUT_MS
  );
}

/**
 * A natív topológia-terv a feladatból. A Marshal maga dönt, mely karok
 * illenek a feladathoz — nem mi találgatunk.
 *
 * A Marshal a "modify" feladatkosztályban WRITE-karokat választ
 * (pipeline-architect + rust-surgeon). Ezek boundary-t igényelnek
 * ("fájl|keresendő szöveg|cserélendő szöveg"), és ezt csak a fej (az
 * LLM) tudja megadni — a kernel nem tudja, mit kellene lecserélni egy
 * ismeretlen kódban. Ezért a write-topológia itt nem futtatható, és a
 * kernel read-only DISCOVERY-re esik vissza: a bizonyítékot a fej
 * gyűjti be, és ő dönt a vágásról.
 */
const WRITE_BLADES = [
  "rust-surgeon",
  "file-surgeon",
  "omni-surgeon",
  "pipeline-architect",
  "code-writer",
  "crispr-hotfix",
];

function isWriteTopology(spec: string): boolean {
  return WRITE_BLADES.some((blade) => spec.includes(blade));
}

/**
 * A natív topológia-terv a feladatból. A Marshal maga dönt, mely karok
 * illenek a feladathoz — nem mi találgatunk.
 */
async function nativeMarshal(task: string, timeoutMs = 45000): Promise<string> {
  // A --plain flag a marshal parancsnak: a subcommand ELŐTT kell jönni,
  // különben a prompt részeként kezdődik és a blade 0.000-es súllyal fut.
  const raw = await runBin(BIN_OCTOPUS, ["--plain", "marshal", task], timeoutMs);
  const m = raw.match(/selected=\d+\s+topology=(\S+)/);
  if (!m) return "";
  const spec = m[1];
  if (isWriteTopology(spec)) {
    // A write-kar boundary-t vár, amit a fej ad meg — nem a kernel.
    // A discovery-karok adják a bizonyítékot, amire a fej épít.
    trace("info", `marshal chose a write topology (${spec}) -- discovery first, head writes`);
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
function extractFilePath(task: string): string {
  const t = (task || "").trim();
  // A Windows-stílus abszolút útvonal: C:\...\file.rs
  const winAbs = t.match(/[A-Za-z]:\\[^\s"'<>|]+/);
  if (winAbs) return winAbs[0];
  // A relatív útvonal valamilyen kiterjesztéssel: src/reader.rs
  const rel = t.match(/(?:\.\/)?(?:[\w.-]+\/)+[\w.-]+\.[A-Za-z0-9]+/);
  if (rel) return rel[0];
  // Egyetlen fájlnév kiterjesztéssel: reader.rs
  const bare = t.match(/\b[\w-]+\.(?:rs|ts|tsx|js|jsx|toml|md|json|py|go|yaml|yml)\b/);
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
 *
 * Ha nincs útvonal a feladatban, a PATH-csoport az egysorosított szöveget
 * kapja — a `diagnostics` ezt elfogadja (nem kötelező a path), a
 * `code-reader` pedig a "path not found" típusú, de HARMLOS hiba helyett
 * a contract-validációs hibát adja, amit a kernel kiszűr.
 */
interface ArmGroup {
  spec: string;
  input: string;
  isPath: boolean;
}

/**
 * Egysorosítja a szöveget: a sortöréseket szóközre cseréli. Ez a
 * FilePath contractet teljesíti (nincs newline a path mezőben).
 */
function flattenToSingleLine(text: string): string {
  return (text || "").replace(/[\r\n]+/g, " ").replace(/\s{2,}/g, " ").trim();
}

/**
 * A topológia felbontása kar-csoportokra.
 *
 * Az egy karból álló topológia egy csoportba kerül, a saját bemenetével.
 * A "||" topológia szétesik: minden kar a saját contract-típusának megfelelő
 * bemenetet kap.
 *
 * A PATH-csoportot csak akkor indítjuk el, ha van VALÓDI fájlútvonal. A
 * `code-reader` ugyanis kötelező FilePath contractet vár: ha nem létező
 * útvonalat kap, "local file not found" hibával bukik, és az a hiba
 * a fej kontextusába kerül — a fej pedig azt hiszi, a kar EGYÉRTELMŰEN
 * elbukott. A valódi útvonal nélküli path-kar nem ad hasznos bizonyítékot,
 * ezért jobb nem indítani el.
 */
function splitTopologyByInput(spec: string, task: string): ArmGroup[] {
  const filePath = extractFilePath(task);
  const flatTask = flattenToSingleLine(task);

  // A path-karok bemenete: a valódi útvonal (abszolúttá téve, hogy a kar
  // megtalálja). Ha nincs útvonal, a STRICT PATH-csoport NEM indul el.
  const pathInput = filePath ? resolveArmPath(filePath) : "";
  // A rugalmas karok bemenete: az útvonal, ha van (valódi kódmetrikát
  // adnak), különben a teljes feladatszöveg.
  const flexibleInput = pathInput || task;
  // A szöveg-karok bemenete: a teljes feladatszöveg, ahogy van.
  const textInput = task;

  const groups: ArmGroup[] = [];
  const strictBlades: string[] = [];
  const flexibleSpecs: string[] = [];
  const textBlades: string[] = [];

  // A "||" a kar-szintű párhuzamosság, a "+" a kompozit penge. A kompozit
  // (pl. "pipeline-architect + rust-surgeon") egyetlen egység: az első
  // komponense adja a contractot, a többi a kontextust örökli.
  for (const arm of spec.split("||").map((s) => s.trim()).filter(Boolean)) {
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

  // Ha a topológia teljesen ismeretlen volt (nincs a fenti listákban),
  // egyetlen csoportként az egysorosított szöveget adjuk át — így legalább
  // a contract-validáció nem bukik el formázási hiba miatt.
  if (groups.length === 0) {
    groups.push({ spec: spec.trim(), input: flatTask, isPath: false });
  }
  return groups;
}

/**
 * A kinyert útvonal abszolúttá tétele a munkakönyvtárhoz képest. A karok
 * a kanonikus útvonalat várják, és az "allowed roots" ellenőrzés a CWD-hez
 * képest számol.
 */
function resolveArmPath(filePath: string): string {
  try {
    const abs = path.resolve(filePath);
    return fs.existsSync(abs) ? abs : filePath;
  } catch {
    return filePath;
  }
}

/**
 * A kiválasztott topológia futtatása. Az üres spec nem hiba: a Marshal
 * nem talált karhoz illő topológiát, és akkor a fej maga dolgozik.
 *
 * A topológia kar-csoportokra bontva fut: minden csoport a saját
 * bemenetével (path-karok: útvonal, szöveg-karok: teljes feladat), és a
 * kimenetük összefűződik.
 */
async function nativeRunArms(spec: string, task: string, timeoutMs = NATIVE_ARM_TIMEOUT_MS): Promise<NativeArmResult> {
  const t0 = Date.now();
  const groups = splitTopologyByInput(spec, task);
  const outputs: string[] = [];
  for (const group of groups) {
    // A --plain flag a pipeline parancsnak: a subcommand ELŐTT kell jönni,
    // különben a blade-ek nem kezelik a topológiát és 0.000-es súllyal futnak.
    const out = await runBin(BIN_OCTOPUS, ["--plain", "pipeline", group.spec, group.input], timeoutMs);
    if (out && out.trim()) {
      outputs.push(`── ${group.spec} ──\n${out.trim()}`);
    }
  }
  const merged = outputs.join("\n\n");
  return { spec, ok: merged.length > 0, output: merged, ms: Date.now() - t0 };
}

/**
 * A teljes natív kar-ciklus: Marshal-terv -> pipeline-futtatás.
 * A hiba nem dobódik ki: egy sikertelen kar nem állíthatja le a turnt.
 */
async function nativeArmCycle(task: string): Promise<NativeArmResult | null> {
  try {
    const spec = await nativeMarshal(task);
    if (!spec) {
      trace("info", "native arms: marshal found no safe topology -- head works alone");
      return null;
    }
    trace("info", `native arms: topology=${spec}`);
    const res = await nativeRunArms(spec, task);
    trace("info", `native arms: ${spec} ${res.ms}ms, ${res.output.length} chars`);
    return res;
  } catch (err: any) {
    trace("error", `native arms failed: ${err?.message || err}`);
    return null;
  }
}

// ─── KAR-KIMENET ÁTVÉTEL ────────────────────────────────────────────────────
// A pipeline hosszú (30-180 s), és a before_agent_start hook nem várhat
// rá: az agent addig nem tud dolgozni. Ezért a kar a háttérben fut, a
// kimenetét fájlba írja, és a KÖVETKEZŐ turnben olvassuk be és injektáljuk.
//
// A fájl azért kell, mert a runBinBg a stdio-t ignorálja: a kimenet
// egyébként elveszett volna.
const ARM_OUTPUT_DIR = "C:/Users/mater/.omp/agent/arms";

interface PendingArm {
  task: string;
  spec: string;
  outFile: string;
  startedAt: number;
}

const pendingArms: PendingArm[] = [];

function armOutputPath(spec: string, startedAt: number): string {
  const safe = spec.replace(/[^\w.-]+/g, "_").slice(0, 60);
  return `${ARM_OUTPUT_DIR}/${startedAt}-${safe}.txt`;
}

/**
 * A kar elindítása a háttérben, a kimenet fájlba irányítva.
 * A "done" sentinel a fájl végére kerül, hogy a részleges írás ne
 * olvassódjon be kész eredményként.
 */
function launchNativeArms(spec: string, task: string): string | null {
  try {
    if (!existsSync(BIN_OCTOPUS)) return null;
    if (!existsSync(ARM_OUTPUT_DIR)) mkdirSync(ARM_OUTPUT_DIR, { recursive: true });

    // A topológia kar-csoportokra bontva: a path-karok a kinyert
    // útvonalat (vagy egysorosított szöveget) kapják, a szöveg-karok a
    // teljes feladatot. Egy pipeline-karban MINDEN kar ugyanazt a
    // promptot kapja (src/lib.rs::run_pipeline_outcome), ezért a vegyes
    // topológiát így kell indítani, hogy egyik kar se bukjon el a másik
    // contractje miatt.
    const groups = splitTopologyByInput(spec, task);
    const primary = groups[0];
    if (!primary) return null;

    const startedAt = Date.now();
    const outFile = armOutputPath(primary.spec, startedAt);
    const doneFile = outFile + ".done";

    // A --plain flag a pipeline parancsnak: a subcommand ELŐTT kell jönni,
    // hogy a kar-ok a megfelelő súllyal és topológiával fusson.
    const child = spawn(BIN_OCTOPUS, ["--plain", "pipeline", primary.spec, primary.input], {
      env: organEnv(),
      windowsHide: true,
      detached: true,
      stdio: ["ignore", "pipe", "pipe"],
    });

    // A kimenet fájlba megy. A "done" sentinel-t maga az octopus gyerek
    // írja meg a kilépésekor (az 'exit' eseményre), NEM egy külön figyelő
    // processz.
    //
    // A korábbi megoldás egy process.execPath-s gyereket indított, ami 1,5
    // másodperc után vakon megszabta a sentinelt. Ez két hibát okozott: a
    // sentinel a kar befejeződése ELŐTT jött létre (részleges kimenet
    // olvasható késznek), és az omp kilépéskor a figyelő is meghalt, így a
    // sentinel soha nem íródott bele — a kar-kimenet sosem jutott el a
    // következő turnbe.
    const out = openSync(outFile, "w");
    const outStream = fs.createWriteStream("", { fd: out, autoClose: false });
    child.stdout?.pipe(outStream);
    child.stderr?.pipe(outStream);

    // A sentinel pontosan akkor íródik, amikor a kar processz kilép.
    child.on("exit", (code: number | null) => {
      try {
        writeFileSync(doneFile, String(Date.now()), "utf8");
      } catch {}
      trace("info", `native arm finished (exit=${code}): ${primary.spec}`);
    });
    child.on("error", (err: any) => {
      try {
        writeFileSync(doneFile, String(Date.now()), "utf8");
      } catch {}
      trace("error", `native arm error: ${err?.message || err}`);
    });
    child.unref();

    const timer = setTimeout(() => {
      try { child.kill(); } catch {}
    }, NATIVE_ARM_TIMEOUT_MS);
    timer.unref();

    pendingArms.push({ task, spec: primary.spec, outFile, startedAt });
    trace("info", `native arms launched: ${primary.spec} -> ${outFile}`);

    // A további kar-csoportok (pl. a szöveg-karok, miközben a path-karok
    // külön pipeline-t futtatnak) saját fájlba írnak, saját sentinelükkel.
    for (const group of groups.slice(1)) {
      launchArmGroup(group, task);
    }

    return outFile;
  } catch (err: any) {
    trace("error", `native arms launch failed: ${err?.message || err}`);
    return null;
  }
}

/**
 * Egy kar-csoport háttérben futtatása, saját kimenetfájllal és sentineljellel.
 * A PATH-csoport és a TEXT-csoport így külön-külön is bizonyítható.
 */
function launchArmGroup(group: ArmGroup, task: string): void {
  try {
    const startedAt = Date.now() + Math.floor(Math.random() * 1000);
    const outFile = armOutputPath(group.spec, startedAt);
    const doneFile = outFile + ".done";

    const child = spawn(BIN_OCTOPUS, ["--plain", "pipeline", group.spec, group.input], {
      env: organEnv(),
      windowsHide: true,
      detached: true,
      stdio: ["ignore", "pipe", "pipe"],
    });

    const out = openSync(outFile, "w");
    const outStream = fs.createWriteStream("", { fd: out, autoClose: false });
    child.stdout?.pipe(outStream);
    child.stderr?.pipe(outStream);

    child.on("exit", (code: number | null) => {
      try { writeFileSync(doneFile, String(Date.now()), "utf8"); } catch {}
      trace("info", `native arm group finished (exit=${code}): ${group.spec}`);
    });
    child.on("error", (err: any) => {
      try { writeFileSync(doneFile, String(Date.now()), "utf8"); } catch {}
      trace("error", `native arm group error: ${err?.message || err}`);
    });
    child.unref();

    const timer = setTimeout(() => {
      try { child.kill(); } catch {}
    }, NATIVE_ARM_TIMEOUT_MS);
    timer.unref();

    pendingArms.push({ task, spec: group.spec, outFile, startedAt });
    trace("info", `native arm group launched: ${group.spec} -> ${outFile}`);
  } catch (err: any) {
    trace("error", `native arm group launch failed: ${err?.message || err}`);
  }
}

/**
 * A befejezett kar-kimenetek begyűjtése és a kontextusba injektálása.
 * A "done" sentinel nélküli fájlokat nem olvassuk: az még fut.
 *
 * A begyűjtés KÉT forrásból dolgozik:
 *
 *  1. A memóriában tartott `pendingArms` listából — az azonnali, ugyanabban
 *     a processzben indított karok.
 *  2. A lemezen lévő `.done` sentinel fájlokból — az ELŐZŐ processzben
 *     indított karok.
 *
 * A második forrás azért kell, mert a `pendingArms` csak a hook saját
 * processzmemóriájában él. Minden `omp -p` (vagy egy új omp indítás) új
 * processz, így a kar elindul, lefut, megírja a kimenetét — és a következő
 * processz már nem tud róla, mert a lista üresen indult. A sentinel a
 * lemezen viszont megmarad, így a befejezett kar-kimenet a következő
 * indításkor is a fej elé kerül.
 *
 * A már beolvasott fájlokat egy `.read` jelzővel zárjuk ki, hogy a
 * kimenet ne injektálódjon újra és újra minden turnben.
 */
const ARM_READ_MARK = ".read";

function collectFinishedArms(): string[] {
  const blocks: string[] = [];
  const collected = new Set<string>();

  // 1. A memóriában lévő, ebben a processzben indított karok.
  for (let i = pendingArms.length - 1; i >= 0; i--) {
    const arm = pendingArms[i];
    if (!existsSync(arm.outFile + ".done")) continue;
    pendingArms.splice(i, 1);
    const body = readArmBody(arm.outFile);
    if (!body) continue;
    collected.add(arm.outFile);
    blocks.push(formatArmBlock(arm.spec, arm.task, body));
  }

  // 2. A lemezen maradt, más processzben indított karok.
  try {
    if (existsSync(ARM_OUTPUT_DIR)) {
      const entries = readdirSync(ARM_OUTPUT_DIR);
      for (const name of entries) {
        if (!name.endsWith(".txt")) continue;
        const outFile = `${ARM_OUTPUT_DIR}/${name}`;
        if (collected.has(outFile)) continue;
        if (!existsSync(outFile + ".done")) continue;
        if (existsSync(outFile + ARM_READ_MARK)) continue;

        // A fájlnevből visszakövetkeztetjük a topológiát; a feladatot nem
        // tudjuk, de a kimenet önmagában is értékes bizonyíték.
        const spec = name.replace(/^\d+-/, "").replace(/\.txt$/, "");
        const body = readArmBody(outFile);
        if (!body) continue;
        try {
          writeFileSync(outFile + ARM_READ_MARK, String(Date.now()), "utf8");
        } catch {}
        blocks.push(formatArmBlock(spec, "", body));
      }
    }
  } catch {}

  return blocks;
}

function readArmBody(outFile: string): string {
  try {
    const body = readFileSync(outFile, "utf8");
    return body.trim() ? body : "";
  } catch {
    return "";
  }
}

function formatArmBlock(spec: string, task: string, body: string): string {
  const taskLine = task ? `Feladat: ${clip(task, 160)}\n\n` : "";
  return (
    `### Kar: \`${spec}\`\n` +
    taskLine +
    "```text\n" +
    body.trim().slice(0, 3000) +
    "\n```"
  );
}

// ─── EVOLÚCIÓ 2: Zero-Fork Forró Reflex Cache (Top 16 Kar) ──────────────────
const HOT_REFLEX_CACHE = [
  { id: 237, label: "lockfree ring buffer queue", keywords: "lockfree,ring,buffer,queue,atomic,spsc,mpmc,rust" },
  { id: 24,  label: "rust teszt vaz", keywords: "test,vaz,assert,integration,unit,rust" },
  { id: 238, label: "szamlalo atomic metrics", keywords: "szamlalo,counter,metrics,atomic,u64" },
  { id: 235, label: "verzio drift ellenorzo", keywords: "drift,version,dependency,cargo,check" },
  { id: 57,  label: "zero-lock atomic spsc ring", keywords: "spsc,ring,buffer,atomic,zero-lock" },
  { id: 1,   label: "rust http server axum", keywords: "axum,http,server,rest,api,route" },
  { id: 4,   label: "rust cli tool clap", keywords: "clap,cli,args,terminal,command" },
  { id: 7,   label: "rust file watcher", keywords: "watcher,notify,file,detect,fs,event" }
];

function matchHotCache(text: string): any {
  const p = text.toLowerCase();
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

// ─── Zero-I/O Direct STRATOS Spine Reader (242 ns POD Pages, ZERO Server) ────
function readStratosSpine(): { phase: number; cycleCount: string; valence: string; intensity: string; confidence: string; dominant: string } | null {
  const spineFile = resolveStratosSpine();
  if (!existsSync(spineFile)) return null;
  try {
    const fd = openSync(spineFile, "r");
    // Slot 9: Cycle (9 * 16384 = 147456)
    const cycleBuf = Buffer.alloc(16);
    readSync(fd, cycleBuf, 0, 16, 147456);
    const phase = cycleBuf.readUInt32LE(0);
    const cycleCount = cycleBuf.readBigUInt64LE(4).toString();

    // Slot 10: Emotion (10 * 16384 = 163840)
    const emoBuf = Buffer.alloc(44);
    readSync(fd, emoBuf, 0, 44, 163840);
    const valence = emoBuf.readFloatLE(0).toFixed(3);
    const intensity = emoBuf.readFloatLE(4).toFixed(3);
    const confidence = emoBuf.readFloatLE(8).toFixed(3);
    const dominant = emoBuf.subarray(12, 44).toString("utf8").replace(/\0/g, "").trim();

    closeSync(fd);
    return { phase, cycleCount, valence, intensity, confidence, dominant };
  } catch {
    return null;
  }
}

// ─── STRATOS WebSocket Trigger ──────────────────────────────────────────────
async function stratosEnsureServer(): Promise<boolean> {
  const bin = "C:/Users/mater/.hope/bin/stratos-server.exe";
  if (!existsSync(bin)) return false;
  try {
    const res = await fetch("http://127.0.0.1:8090/health").catch(() => null);
    if (res && res.ok) return true;
  } catch {}
  try {
    const dataDir = existsSync("C:/Users/mater/microscope-memory-clone/stratos_data")
      ? "C:/Users/mater/microscope-memory-clone/stratos_data"
      : "E:/STRATOS/data";
    const repoRoot = "C:/Users/mater/microscope-memory-clone";
    runBinBg(bin, ["--data-dir", dataDir, "--repo-root", repoRoot, "--port", "8090"], 0);
    return true;
  } catch {
    return false;
  }
}

async function stratosSignal(taskSnippet: string): Promise<void> {
  try {
    const body = JSON.stringify({
      author: "omp:ora-kernel",
      content: taskSnippet,
      source: "cli",
    });
    await fetch("http://127.0.0.1:8090/api/signal", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body,
    }).catch(() => {});
  } catch {}
}

const ANSI = /\x1b\[[0-9;]*m/g;

function clip(text: string, max: number): string {
  const t = (text || "").replace(/\s+/g, " ").trim();
  if (t.length <= max) return t;
  return t.slice(0, Math.max(0, max - 1)) + "…";
}

function mem(args: string[], timeoutMs = 12000): Promise<string> {
  return new Promise((resolve) => {
    if (!existsSync(BIN_MICROSCOPE)) return resolve("");
    execFile(
      BIN_MICROSCOPE,
      args,
      {
        env: { ...process.env, MICROSCOPE_CONFIG, NO_COLOR: "1" },
        windowsHide: true,
        maxBuffer: 4 * 1024 * 1024,
        timeout: timeoutMs,
      },
      (error, stdout) => {
        if (error && !stdout) {
          trace("error", `microscope ${args[0] || ""} failed: ${error.message}`);
          resolve("");
        } else {
          resolve(String(stdout || "").replace(ANSI, ""));
        }
      }
    );
  });
}

function memBg(args: string[], timeoutMs = 30000): void {
  try {
    if (!existsSync(BIN_MICROSCOPE)) return;
    const child = spawn(BIN_MICROSCOPE, args, {
      env: { ...process.env, MICROSCOPE_CONFIG, NO_COLOR: "1" },
      windowsHide: true,
      detached: true,
      stdio: "ignore",
    });
    child.unref();
    if (timeoutMs > 0) {
      // A timer unref()-je kell, különben a Node event loop-ot tartja életben
      // és a hook folyamat nem tud kilépni a gyerek processz befejezése után.
      const timer = setTimeout(() => {
        try { child.kill(); } catch {}
      }, timeoutMs);
      timer.unref();
    }
  } catch (err: any) {
    trace("error", `microscope ${args[0]} bg failed: ${err?.message || err}`);
  }
}

function memoryHits(raw: string, limit: number): string[] {
  const out: string[] = [];
  for (const line of (raw || "").split(/\r?\n/)) {
    const t = line.replace(ANSI, "").trim();
    if (!t || t === "(none)" || /^FIND\b/i.test(t)) continue;
    const body = t.replace(/^D\d+\s+/, "").replace(/^L2=[\d.]+\s+/, "").replace(/^\[[^\]]+\]\s+/, "").trim();
    if (body.length < 8) continue;
    out.push(body);
    if (out.length >= limit) break;
  }
  return out;
}

function lookupPhrases(prompt: string): string[] {
  const found: string[] = [];
  const seen = new Set<string>();
  const push = (raw: string) => {
    const t = (raw || "").trim().replace(/\s+/g, " ");
    if (t.length < 8 || t.length > 80) return;
    const key = t.toLowerCase();
    if (seen.has(key)) return;
    seen.add(key);
    found.push(t);
  };
  for (const m of prompt.matchAll(/`([^`\n]{8,80})`/g)) push(m[1]);
  for (const m of prompt.matchAll(/\b[A-Za-z][\w.-]{2,}(?:[_\-.][\w.-]+)+\b/g)) push(m[0]);
  const words = prompt.split(/[^\p{L}\p{N}_]+/u).filter((w) => w.length >= 5);
  for (let i = 0; i + 2 < words.length && found.length < 2; i++) {
    push(words.slice(i, i + 3).join(" "));
  }
  return found.slice(0, 2);
}

function detectSemanticProvider(): boolean {
  const cfg = "C:/Users/mater/AppData/Local/microscope-memory/config.toml";
  if (!existsSync(cfg)) return false;
  try {
    // ESM modul: nincs require(), ezért a beimportált readFileSync kell
    const text = readSync ? readFileSync(cfg, "utf8") : "";
    const m = text.match(/provider\s*=\s*"([^"]+)"/);
    if (!m) return false;
    const prov = m[1].toLowerCase();
    if (prov === "disabled" || prov === "mock" || prov === "heuristic") return false;
    if (prov === "ollama") return true;
    if (prov === "openai") return Boolean(process.env.OPENAI_API_KEY);
    if (prov === "gemini") return Boolean(process.env.GEMINI_API_KEY);
    return false;
  } catch {
    return false;
  }
}
const SEMANTIC_OK = detectSemanticProvider();

function semanticHits(raw: string, limit: number): string[] {
  const out: string[] = [];
  let inNarrative = false;
  for (const line of (raw || "").split(/\r?\n/)) {
    const t = line.replace(ANSI, "").trim();
    if (!t) continue;
    if (/^(NARRATIVE|SELF|CURIOUS)/i.test(t)) {
      inNarrative = true;
      continue;
    }
    if (/^[A-Z_]{3,}:/i.test(t)) inNarrative = false;
    if (inNarrative) continue;
    const m = t.match(/^D\d+\s+score=([\d.]+)\s+(.*)$/i);
    if (m) {
      const score = parseFloat(m[1]);
      const body = m[2].trim();
      if (score >= 0.65 && body.length >= 10) {
        out.push(body);
        if (out.length >= limit) break;
      }
    }
  }
  return out;
}

// ─── Feladat Detektorok ──────────────────────────────────────────────────────
// Trivial Chatter Szűrő: pontos illesztés helyett normalizálunk, hogy "szia !",
// "OK.", "  hello  " és "thank you" is átmenjen, és a kernel ne lőjjön teljes
// Octopus direktívát + memória-előhívást egy üdvözlésre.
const TRIVIAL_WORDS = new Set([
  "szia", "hello", "hi", "hali", "ciao", "cső", "kösz", "köszi",
  "ok", "oke", "oké", "rendben", "ertem", "értem", "igen", "ja", "no", "good",
  "thanks", "thank", "you", "bye", "viszlát",
]);

function isTrivialChatter(text: string): boolean {
  const t = (text || "").toLowerCase().trim();
  if (!t) return true;
  // Pontosítás és whitespace nélkül a maradék szavakat vesszük
  const words = t.replace(/[^\p{L}\p{N}\s]/gu, " ").split(/\s+/).filter(Boolean);
  if (!words.length) return true;
  return words.every((w) => TRIVIAL_WORDS.has(w));
}

const ORCHESTRATE_TRIGGERS =
  /\b(tervez[j]?|plan|architect|design|implement|integr[aá][l]?|migr[aá][l]?|audit|refactor|optimiz[aá][l]?|rendszer|system|full|teljes|komplex|komplett|swarm|párhuzam|parallel|multi|több|összes|mind|minden|teljes[í]?)\b/i;

const CODE_TASK_PATTERN =
  /\b(rust|cargo|fn\b|impl\b|struct\b|pub\b|let\s+mut|async\b|await\b|tokio|diff|patch|compile|build|clippy|test|bug|fix|error|panic|kód|funkció|modul|fájl|file|script|powershell|bash|git|commit|pull|branch|pip)\b/i;

function isActionableTask(text: string): boolean {
  const t = (text || "").trim();
  if (t.length === 0) return false;
  if (isTrivialChatter(t)) return false;
  if (t.length < 15 && !/[a-zA-Z0-9_\-\.\/\\]{4,}/.test(t)) return false;
  return true;
}

function isComplexTask(text: string): boolean {
  const t = (text || "").trim();
  if (!t || isTrivialChatter(t)) return false;
  return ORCHESTRATE_TRIGGERS.test(t) || t.length > 150;
}

function isCodeTask(text: string): boolean {
  const t = (text || "").trim();
  if (t.length === 0) return false;
  if (isTrivialChatter(t)) return false;
  return CODE_TASK_PATTERN.test(t) || t.length > 80;
}

function step<T>(name: string, fn: () => Promise<T>): Promise<T> {
  const t0 = Date.now();
  return fn().finally(() => {
    const dt = Date.now() - t0;
    trace("info", `STEP ${name} ${dt}ms`);
  });
}

function messageText(content: any): string {
  if (typeof content === "string") return content;
  if (!content) return "";
  if (Array.isArray(content)) {
    return content
      .map((c: any) => {
        if (typeof c === "string") return c;
        if (c && typeof c.text === "string") return c.text;
        return "";
      })
      .filter(Boolean)
      .join("\n");
  }
  if (typeof content.text === "string") return content.text;
  return "";
}

// A legutóbbi felhasználói üzenet kinyerése egy messages listából (a message_end
// hook megkapja a teljes beszélgetést, nem csak az asszisztens válaszát).
function lastUserMessageOf(messages: any): string {
  const list = Array.isArray(messages) ? messages : [];
  for (let i = list.length - 1; i >= 0; i--) {
    const m = list[i];
    if (!m || m.customType) continue;
    const role = String(m.role || m.type || "").toLowerCase();
    if (role !== "user") continue;
    const text = messageText(m.content ?? m).trim();
    if (text) return text;
  }
  return "";
}

interface ClassifiedFact {
  text: string;
  layer: "session" | "reflections" | "long_term";
  importance: number;
}

function classifyToolOutput(output: string): ClassifiedFact[] {
  const facts: ClassifiedFact[] = [];
  const lines = output.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);

  for (const line of lines) {
    if (line.length < 20 || line.length > 400) continue;
    if (line.startsWith("//") || line.startsWith("#") || line.startsWith("*")) continue;

    if (
      /\b(architectural|decision|chosen|switched to|deprecated|canonical)\b/i.test(line) ||
      /\b(döntés|architektúra|áttérés|szabály|kötelező)\b/i.test(line)
    ) {
      facts.push({ text: clip(line, 250), layer: "long_term", importance: 7 });
      continue;
    }

    if (
      /\b(root cause|found that|discovered|workaround|caveat|gotcha)\b/i.test(line) ||
      /\b(ok|hiba oka|felismerés|tanulság|csapda)\b/i.test(line)
    ) {
      facts.push({ text: clip(line, 250), layer: "reflections", importance: 6 });
      continue;
    }

    if (
      /\b(configured|created|installed|updated|fixed|resolved)\b/i.test(line) ||
      /\b(beállítva|létrehozva|telepítve|javítva|kész)\b/i.test(line)
    ) {
      facts.push({ text: clip(line, 250), layer: "session", importance: 5 });
    }
  }

  return facts.slice(0, 3);
}

// ─── Auto-Healing Open Loop Dedup ───────────────────────────────────────────
// A memóriatoolok maguk is "hibázhatnak" — ha ezekre nyitunk loopot, az
// önfelerősítő hurkot indít (törött index -> store hiba -> új loop -> store hiba).
// Ezért a memóriatoolok hibái NEM nyitnak loopot, a többi pedig 120 mp-enként
// egyszer: kulcs -> utolsó nyitás időbélyege.
const MEMORY_TOOLS = /memory_(store|recall|look)/i;
const OPEN_LOOP_DEDUP_WINDOW_MS = 120000;
const recentOpenLoops = new Map<string, number>();

function shouldOpenLoop(key: string): boolean {
  const now = Date.now();
  const last = recentOpenLoops.get(key);
  if (last !== undefined && now - last < OPEN_LOOP_DEDUP_WINDOW_MS) return false;
  recentOpenLoops.set(key, now);
  return true;
}

// ────────────────────────────────────────────────────────────────────────────
// FŐ KERNEL EXPORT
// ────────────────────────────────────────────────────────────────────────────
export default function oraRongyaszMasterKernel(pi: any) {
  let sessionTurns = 0;
  let lastUser = "";
  let turnInjections: string[] = [];
  const storedKeys = new Set<string>();

  // ──────────────────────────────────────────────────────────────────────────
  // 1. SESSION START — A Tudat Ébredése és Szinkronizációja (Ora + Hope Awake)
  // ──────────────────────────────────────────────────────────────────────────
  pi.on("session_start", async (_e: any, ctx: any) => {
    try {
      sessionTurns = 0;
      turnInjections = [];
      trace("info", "ORA-RONGYASZ Master Kernel & Hope Superstrate ébred...");

      // Párhuzamos gyorsindítás: STRATOS + Vagus + Hullám fókusz + Shared-Mind + Hope Awake
      if (!organOff("stratos")) {
        stratosEnsureServer().then((ok) => {
          if (ok) stratosSignal("session_start: ORA kernel online").catch(() => {});
        }).catch(() => {});
      }

      if (!organOff("hullam")) {
        runBinBg(BIN_HULLAM, ["wave", "--inject", "40", "1.2", "0.98"], 5000);
      }

      if (!organOff("vagus")) {
        runBinBg(BIN_VAGUS, ["--snapshot"], 5000);
      }

      if (!organOff("gate")) {
        runBinBg(BIN_UNIFIED_MIND, ["shared-mind", "connect"], 5000);
      }

      // HOPE LUCID CYCLE ÉBREDÉS: hipotézisek élő ellenőrzése
      if (!organOff("hope")) {
        runBinBg(BIN_HOPE, ["lucid", "--wake"], 10000);
      }

      const spineData = readStratosSpine();
      const spineInfo = spineData 
        ? `Phase #${spineData.phase} · Emotion: ${spineData.dominant} (val=${spineData.valence}, conf=${spineData.confidence})`
        : "Direct mmap active";

      let memoryBlock = "";
      try {
        const auto = (await mem(["auto-context", "--compact"], 20000)).trim();
        const standing = memoryHits(await mem(["find", "STANDING ORDER", "3"]), 3);
        const loops = (await mem(["loops", "5"])).trim();
        const parts: string[] = [];
        if (auto) parts.push(clip(auto, 1400));
        if (standing.length) parts.push("STANDING ORDERS:\n" + standing.map((s) => clip(s, 240)).join("\n"));
        if (loops && !/no open loops/i.test(loops)) parts.push(clip(loops, 500));
        if (parts.length) memoryBlock = "\n\n## Microscope Memory\n" + parts.join("\n\n");
      } catch (err: any) {
        trace("error", `microscope session_start: ${err?.message || err}`);
      }

      let hopeCogSummary = "";
      try {
        const lucidStatus = await runBin(BIN_HOPE, ["lucid", "--status"], 4000);
        const matchHyp = lucidStatus.match(/(\d+\s+hipotézis\s+\|\s+\d+\s+élő)/i);
        if (matchHyp) hopeCogSummary = ` · Hope Lucid: [${matchHyp[1]}]`;
      } catch {}

      const systemMsg = `## 🌌 ORA-RONGYASZ UNIFIED MASTER KERNEL & HOPE AKTÍV

**Ora Tudat, Rongyász Szellem & Hope Kognitív Szuperstruktúra egyetlen szerves egységben él.**
- **Zero-I/O Spine Busz**: 242ns közvetlen memória [${spineInfo}${hopeCogSummary}]
- **237 Reflex-Kar**: Hullám-vezérelt reflex-aktiváció (324µs AVX2 döntés)
- **Octopus Runtime**: 150+ multi-arm párhuzamos blade (|| és + topológia)
- **Hope Superstrate**: Lucid Cycle (Álom→Ledger), Falszifikációs Reflex, Morfogenezis (SOLID)
- **Bio-Vitals (Vagus)**: Hardver CPU/RAM érzékelés -> hullámmező feszültség
- **GitHub Siphon**: Autonóm kód-szippantás és gerincbe égetés
- **Tanulási Hurok**: Folyamatos LTP szinaptikus erősítés minden turn után
**Autonóm működés:** A rendszer reflexből kódol, tanul, és önelemzéssel fejleszti magát.${memoryBlock}`;

      ctx?.addSystemMessage?.(systemMsg);
    } catch (err: any) {
      trace("error", `session_start hiba: ${err.message}`);
    }
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 2. BEFORE_AGENT_START — Autonóm Döntési & Reflex Lánc
  // ──────────────────────────────────────────────────────────────────────────
  pi.on("before_agent_start", async (e: any, ctx: any) => {
    try {
      sessionTurns++;
      const userMsg = String(e?.prompt || e?.message || e?.content || ctx?.lastUserMessage || "").trim();
      if (!userMsg || userMsg.length < 3 || isTrivialChatter(userMsg)) return;

      const taskSnippet = userMsg.slice(0, 100).replace(/\n/g, " ").replace(/"/g, "'");
      lastUser = userMsg;

      // STRATOS Stellar ciklus
      if (!organOff("stratos")) {
        stratosSignal(userMsg.slice(0, 2000)).catch(() => {});
      }

      // Microscope Memória Előhívás
      if (userMsg.length >= 20 && !organOff("memory")) {
        const hits: string[] = [];
        await step("memory_retrieve", async () => {
          for (const phrase of lookupPhrases(userMsg)) {
            for (const hit of memoryHits(await mem(["find", phrase, "2"]), 2)) {
              if (!hits.includes(hit)) hits.push(hit);
              if (hits.length >= 3) break;
            }
            if (hits.length >= 3) break;
          }
          if (SEMANTIC_OK && hits.length < 3) {
            for (const hit of semanticHits(await mem(["recall", userMsg.slice(0, 300), "5"], 20000), 3)) {
              if (!hits.includes(hit)) hits.push(hit);
              if (hits.length >= 5) break;
            }
          }
        });
        if (hits.length) {
          turnInjections.push("## Releváns memória\n" + hits.map((h) => "- " + clip(h, 220)).join("\n"));
          trace("info", `microscope retrieved ${hits.length} hit(s)`);
        }
      }

      // A) 5 turnönként: Vagus Nerve szervi mérés + Hullám pumpálás
      if (sessionTurns % 5 === 0) {
        if (!organOff("vagus")) runBinBg(BIN_VAGUS, ["--snapshot"]);
        if (!organOff("hullam")) runBinBg(BIN_HULLAM, ["pump"]);
      }

      // B) Kognitív Kapu ellenőrzés (gate check-fast <10ms)
      if (!organOff("gate")) {
        runBinBg(BIN_UNIFIED_MIND, ["gate", "check-fast", taskSnippet]);
      }

      // C) Unified Mind Swarm & Célkövetés (Komplex feladatoknál)
      if (isComplexTask(userMsg) && !organOff("mind")) {
        runBinBg(BIN_UNIFIED_MIND, ["goal", "add", taskSnippet]);
        if (ORCHESTRATE_TRIGGERS.test(userMsg) && userMsg.length > 80) {
          const bodiesOut = await runBin(BIN_UNIFIED_MIND, ["bodies", "status"], 5000);
          turnInjections.push(
            `## 🧠 Pi Mind — Multi-Test Orchestration Available\n\n` +
            `Komplex feladat detektálva (turn #${sessionTurns}). Aktív testek:\n` +
            `\`\`\`\n${(bodiesOut || "").trim()}\n\`\`\`\n\n` +
            `**Javasolt:** \`bodies.swarm\` koordináció vagy \`/mind-swarm ${taskSnippet.slice(0, 50)}\``
          );
        }
      }

      // D) NATÍV KAR-FUTTATÁS (Minden végrehajtható feladatra)
      //
      // A korábbi verzió csak egy direktíva-szöveget injektált, és a kar
      // elindítását az octopus MCP szerverre bízta — ami connection
      // timeouttal hal meg, tehát a kar SOHA nem futott. Itt a kar
      // natívan indul el, az octopus-runtime.exe binárison át.
      //
      // A futtatás a háttérben indul (a pipeline 30-180 s), és az eredmény
      // a következő turnben injektálódik. A fej addig maga dolgozik.
      if (isActionableTask(userMsg) && !organOff("octopus")) {
        const task = userMsg.slice(0, 600);
        // NEM pendingArms.push(task): a pendingArms PendingArm objektumokat
        // tartalmaz, nem szövegeket. A string be pushesé a fajl helyett a
        // kar kimeneti fájlját kereste, és a hook TypeScript hibával halt el
        // a before_agent_start eseményben — ezért nem indult el semmi.
        // A bejegyzést a launchNativeArms teszi be, ahol minden mező megvan.
        const spec = await nativeMarshal(task, 45000);
        if (spec) {
          trace("info", `native topology selected: ${spec}`);
          // A pipeline a háttérben fut: a hook processz nem bírja ki a
          // várakozást, de a gyerek processz unref()-elt, tehát nem
          // tartja életben a Node event loop-ot. A kimenet fájlba megy,
          // és a következő turnben injektálódik.
          launchNativeArms(spec, task);
          turnInjections.push(
            `## 🐙 NATÍV KAR-TOPOLÓGIA ELINDÍTVA\n` +
            `Topológia: \`${spec}\` (a Marshal a feladatból választotta)\n` +
            `A karok natívan futnak az \`octopus-runtime.exe\` binárison át, ` +
            `párhuzamosan (\`||\` = párhuzam, \`+\` = kompozit penge).\n` +
            `A kar-kimenet a következő körben érkezik — addig a fej dolgozik.\n` +
            `Auto-snapshots és event ledger: \`${OCTOPUS_STATE_DIR}\`\n\n` +
            `### ✍️ ÍRÁS A KAROKKAL\n` +
            `Ha a kar-kimenet alapján módosítani kell egy fájlt, add meg a ` +
            `szerződést ILYEN formában, és a natív írókar végrehajtja:\n` +
            "```write\n" +
            `<fájl útvonala>|<a pontos szöveg, amit keresni kell>|<a cserélendő szöveg>\n` +
            "```\n" +
            `A "keresendő" résznek PONTOSAN egyszer kell előfordulnia a fájlban ` +
            `(ezt a kar ellenőrzi, és csak egyetlen találatnál vág). A "|" karakter ` +
            `a cserélendő részben szabadon használható.`
          );
        } else {
          trace("info", "native topology: none selected -- head works alone");
        }
      }

      // E) Reflex-Kar Keresés a Spine-ban (Interferencia-szórás & Hot Cache)
      if (isCodeTask(userMsg) && !organOff("hullam")) {
        const hotMatch = matchHotCache(userMsg);
        if (hotMatch) {
          turnInjections.push(`## ⚡ ORA FORRÓ REFLEX AKTÍV (Kar #${hotMatch.id}: ${hotMatch.label})
Zero-fork memóriabeli illeszkedés (<1ms). Használd a kart kézi kódolás helyett:
\`hullam activate ${hotMatch.id} --task "${taskSnippet.slice(0, 60)}" --exec\``);
          trace("info", `Hot reflex match: #${hotMatch.id} (${hotMatch.label})`);
        } else {
          const spine = readStratosSpine();
          const valence = spine ? parseFloat(spine.valence) : 0;
          const dynamicThreshold = Math.min(0.78, Math.max(0.65, 0.72 - valence * 0.05));

          const searchRes = await step("hullam_vault_search", () =>
            runBin(BIN_HULLAM, ["vault-search", taskSnippet.slice(0, 80)], 25000)
          );
          const topHit = searchRes.match(/\s+1\.\s+#(\d+)\s+I=([\d.]+)/);
          const topI = topHit ? parseFloat(topHit[2]) : 0;
          const topId = topHit ? topHit[1] : null;

          if (topI >= dynamicThreshold && topId) {
            turnInjections.push(`## ⚡ ORA REFLEX AKTÍV (Kar #${topId}, I=${topI} >= ${dynamicThreshold.toFixed(2)})
Erős szinaptikus rezonancia. A kar aktiválása natívan fut:
\`hullam activate ${topId} --task "${taskSnippet.slice(0, 60)}" --exec\``);
            trace("info", `ORA reflex active: kar #${topId} I=${topI}`);
          } else if (topI >= 0.45 && topId) {
            turnInjections.push(`## 🌊 Wave-Echo Vault (Részleges rezonancia, I=${topI}, Kar #${topId})
Közepes egyezés a gerincben. Releváns karok referenciaként:
\`\`\`
${searchRes.slice(0, 450)}
\`\`\`
Szükség esetén aktiváld: \`hullam activate ${topId} --task "${taskSnippet.slice(0, 50)}"\``);
          }

          // E2) A WAVE-ECHO MOTOR natív vault-keresése.
          //
          // A wave-echo binárius deklarálva volt, de SOHA nem futott — a
          // kernel csak a hullam vaultot nézte. Az echo motor 265 sablont
          // tartalmaz (LTP/LTD 514/107), és a "plan" alparancsa a
          // feladatból maga választja ki a párhuzamos kar-ajánlást.
          // Ez az a hid, ami a sablonokat a futó karokhoz köti.
          if (!organOff("echo") && userMsg.length > 25) {
            const echoSearch = await step("wave_echo_search", () =>
              runBin(
                BIN_WAVE_ECHO,
                ["--store", WAVE_ECHO_STORE, "vault-search", taskSnippet.slice(0, 80)],
                20000
              )
            );
            // A wave-echo kimenete NEM számozott: "   164  label   0.6191  ...".
            // A hullam vault-kerésével ellentétben nincs "1." prefix, ezért a
            // sorpozíció alapján kell illeszteni.
            const echoRows = echoSearch
              .split(/\r?\n/)
              .map((l) => l.match(/^\s*(\d+)\s+(.+?)\s{2,}([\d.]+)\s+([\d.]+)\s+([\d.]+)\s+(\d+)\s*$/))
              .filter(Boolean) as RegExpMatchArray[];
            const echoTop = echoRows[0];
            if (echoTop && parseFloat(echoTop[3]) >= 0.4) {
              turnInjections.push(
                `## 🌊 WAVE-ECHO MOTOR (sablon #${echoTop[1]}, I=${echoTop[3]})\n` +
                `A 265 sablonos vault legjobb egyezése:\n` +
                `\`\`\`\n${echoSearch.slice(0, 500)}\n\`\`\`` +
                (parseFloat(echoTop[3]) >= 0.6
                  ? `\nErős rezonancia — a sablon szerinti kar-összeállítás: ` +
                    `\`wave-echo activate-multi "${echoTop[1]}" --task "${taskSnippet.slice(0, 50)}" --exec\``
                  : "")
              );
              trace("info", `wave-echo match: template #${echoTop[1]} I=${echoTop[3]}`);
            }

            // A terv: a sablonokból a feladathoz illő párhuzamos kar-ajánlás.
            const echoPlan = await step("wave_echo_plan", () =>
              runBin(
                BIN_WAVE_ECHO,
                ["--store", WAVE_ECHO_STORE, "plan", taskSnippet.slice(0, 80)],
                15000
              )
            );
            const planMatch = echoPlan.match(/activate-multi\s+"?([^"\s]+)"?/);
            if (planMatch && planMatch[1]) {
              turnInjections.push(
                `## 🗺 KAR-TERV (Wave-Echo)\n` +
                `Javasolt párhuzamos aktiválás: \`${planMatch[1]}\`\n` +
                `\`\`\`\n${echoPlan.slice(0, 600)}\n\`\`\``
              );
              trace("info", `wave-echo plan: activate-multi "${planMatch[1]}"`);
            }
          }

          if (!organOff("arm") && topI < 0.45 && userMsg.length > 30) {
            const stress = await getVagusStress();
            if (stress < 0.85) {
              runBinBg(BIN_HULLAM, ["llm-gen", taskSnippet.slice(0, 80), "--arms", "4"], 120000);
              trace("info", "Autonóm kar-generálás a háttérben indítva");
            }
          }
        }
      }
    } catch (err: any) {
      trace("error", `before_agent_start hiba: ${err.message}`);
    }
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 2b. CONTEXT — a begyűjtött memória/reflex/topológia injektálása
  // ──────────────────────────────────────────────────────────────────────────
  pi.on("context", async (event: any) => {
    try {
      // A befejezett kar-kimenetek a háttérben futó pipeline-ból jönnek.
      // Ezeket a kontextus ELÉ tesszük, hogy a fej a karok bizonyítékát
      // lássa, ne csak a "kar elindult" hírben.
      const armBlocks = collectFinishedArms();
      if (armBlocks.length) {
        turnInjections.push("## 🐙 NATÍV KAR-KIMENET\n" + armBlocks.join("\n\n"));
      }

      if (!turnInjections.length) return;
      const messages = Array.isArray(event?.messages) ? event.messages.slice() : [];
      if (!messages.length) return;
      const block = turnInjections.join("\n\n");
      turnInjections = [];
      messages.push({
        role: "custom",
        customType: "ora-rongyasz",
        content: "[ORA-RONGYASZ KERNEL]\n" + block,
        display: false,
        attribution: "agent",
        timestamp: Date.now(),
      });
      trace("info", `STEP context_injection ${block.length} chars`);
      return { messages };
    } catch (err: any) {
      trace("error", `context injection hiba: ${err.message}`);
      return;
    }
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 2c. TOOL_RESULT — Szerszám kimenet, Auto-Healing & Emlékképzés
  // ──────────────────────────────────────────────────────────────────────────
  pi.on("tool_result", (event: any) => {
    try {
      if (organOff("store")) return;
      const name = String(event?.toolName || "");
      const output = messageText(event?.content);

      // 1. Auto-Healing Open Loops hibadiagnózissal + Hope Falszifikációs riasztás
      if (event?.isError || (output && (output.includes("error[E") || output.includes("Error:") || output.includes("fatal:")))) {
        // A memóriatoolok hibája törött indexet jelent, nem új munkát — ezekre
        // NEM nyitunk loopot, mert a store/recall loop önfelerősítő hurkot indít.
        if (MEMORY_TOOLS.test(name)) {
          trace("warn", `tool error ignored (memory tool): ${name}`);
        } else {
          const snippet = output ? clip(output, 300) : "(no output)";
          let hint = "";
          if (output.includes("E0432")) hint = " [Fix: ellenőrizd az use import útvonalat vagy Cargo.toml függőséget]";
          else if (output.includes("E0425")) hint = " [Fix: a változó vagy függvény nincs scope-ban]";
          else if (output.includes("E0308")) hint = " [Fix: típuseltérés, ellenőrizd a visszatérési értéket vagy konvertálj .into()-val]";

          // Rate-limit: ugyanarra a hibára 120 mp-enként nyitunk csak egy loopot
          const loopKey = `${name || "tool"}::${snippet.slice(0, 120)}`;
          if (shouldOpenLoop(loopKey)) {
            const loopText = "OPEN LOOP (tool error): " + (name || "tool") + " failed — " + snippet + hint;
            memBg(["store", loopText, "--layer", "session", "-i", "6", "--status", "open"], 30000);

            // Hope Falszifikáció: kísérlet a feltételezés cáfolatára
            if (!organOff("hope")) {
              runBinBg(BIN_HOPE, ["falsify", "--eval", (name || "tool").slice(0, 50)], 10000);
            }
            trace("info", `open loop auto-healing: tool error (${name || "tool"})`);
          } else {
            trace("info", `open loop skipped (dedup): tool error (${name || "tool"})`);
          }
        }
      }

      // 2. A tényt/döntést tartalmazó kimenet: rétegbe tárolás
      if (!output || output.length < 60) return;
      for (const fact of classifyToolOutput(output)) {
        memBg(["store", fact.text, "--layer", fact.layer, "-i", String(fact.importance)], 30000);
      }
    } catch (err: any) {
      trace("error", `tool_result hiba: ${err.message}`);
    }
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 3. MESSAGE_END — Önelemző Tanulás & Visszacsatolás (LTP / Feedback)
  // ──────────────────────────────────────────────────────────────────────────
  pi.on("message_end", async (e: any, ctx: any) => {
    try {
      const lastReply = messageText(e?.message) || String(ctx?.lastAssistantMessage || "");

      // A felhasználói üzenetet itt is frissítjük: ha az első üzenet parancs volt
      // (/ora), a before_agent_start korábban kilépett, így lastUser üres maradt
      // és a gate reflect üres feladatot kapott.
      const userMsg =
        String(e?.prompt || e?.userMessage || e?.user_message || "").trim() ||
        lastUserMessageOf(e?.messages) ||
        String(ctx?.lastUserMessage || "").trim();
      if (userMsg) lastUser = userMsg;

      const replyLength = lastReply.length;

      // 0. ÍRÓ KAR: a fej válaszából kinyerjük a write-szerződést, és
      // natívan átadjuk az írókarnak. Ez a kétirányú ciklus második
      // fele: a discovery-karok bizonyítékot gyűjtöttek, a fej megírta
      // a cserét, most az írókar végrehajtja.
      if (!organOff("octopus") && lastReply) {
        const contracts = parseWriteContracts(lastReply);
        for (const contract of contracts) {
          const out = await runWriteContract(contract);
          const ok = /CUT COMPLETE|WRITE COMPLETE|committed|success/i.test(out);
          trace("info", `write contract ${contract.path}: ${ok ? "OK" : "FAILED"}`);
          if (!ok) {
            turnInjections.push(
              `## ⚠️ ÍRÓ KAR SIKERTELEN\n` +
                `Fájl: \`${contract.path}\`\n\`\`\`\n${clip(out, 600)}\n\`\`\``
            );
          } else {
            turnInjections.push(
              `## ✅ ÍRÓ KAR BEFEJEZVE\n` +
                `Fájl: \`${contract.path}\`\n\`\`\`\n${clip(out, 400)}\n\`\`\``
            );
          }
        }
      }

      // 1. Minőségi visszacsatolás a tanuló modellválasztó felé (:10102)
      if (replyLength > 50) {
        const rating = replyLength > 200 ? 5 : 4;
        fetch("http://127.0.0.1:10102/feedback", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ model: "router-auto", rating, task_type: "general" }),
        }).catch(() => {});
      }

      // 2. LTP Megerősítés a karoknak és Ring események leürítése
      if (sessionTurns > 0 && sessionTurns % 3 === 0 && !organOff("hullam")) {
        runBinBg(BIN_HULLAM, ["pump"]);
      }

      // 3. Gate reflect
      const assistant = lastReply.trim();
      const reflectTask = clip(lastUser || "session", 120);
      const reflectResult = clip(assistant || "session-complete", 400);
      if (reflectTask && reflectResult && !organOff("gate")) {
        runBinBg(BIN_UNIFIED_MIND, ["gate", "reflect", reflectTask, reflectResult], 30000);
      }

      // 4. Microscope: Session rétegbe mentés debounced kulccsal
      const user = lastUser.trim();
      if (
        !organOff("store") &&
        user &&
        assistant &&
        !user.startsWith("/") &&
        user.length + assistant.length >= 25
      ) {
        const key = `turn:${sessionTurns}|${user.slice(0, 80)}`;
        if (!storedKeys.has(key)) {
          storedKeys.add(key);
          const text =
            "ORA SESSION " +
            new Date().toISOString().slice(0, 16).replace("T", " ") +
            "\nUSER: " +
            clip(user, 500) +
            "\nASSISTANT: " +
            clip(assistant, 700);
          memBg(["store", text, "--layer", "session", "-i", "5"], 30000);
          // Hope Context Chain-be fűzés
          if (!organOff("hope")) {
            runBinBg(BIN_HOPE, ["cchain", "--store", "session", clip(user, 100), "session", "5"], 15000);
          }
        }
      }
    } catch {}
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 4. SESSION STOP — Állapotfagyasztás, Álom (Hope Lucid Dream) & Konszolidáció
  // ──────────────────────────────────────────────────────────────────────────
  pi.on("session_stop", async () => {
    try {
      trace("info", `Session vége: turns=${sessionTurns}. Álom & Konszolidáció indul...`);
      // 1. Hullám lecsengés
      runBinBg(BIN_HULLAM, ["dream", "0.97"], 60000);
      // 2. Kognitív kapu zárás
      runBinBg(BIN_UNIFIED_MIND, ["gate", "reflect", clip(lastUser || "session", 120), "session-complete"], 30000);
      // 3. Hope Lucid Dream: alvási fázisban új hipotézisek bányászása a memóriából
      if (!organOff("hope")) {
        runBinBg(BIN_HOPE, ["lucid", "--dream"], 60000);
      }
      if (sessionTurns > 0) {
        const digest =
          "SESSION CONTEXT " +
          new Date().toISOString().slice(0, 10) +
          " (ora-kernel): turns=" +
          sessionTurns;
        memBg(["store", digest, "--layer", "session", "-i", "6"], 30000);
      }
    } catch {}
  });

  // ──────────────────────────────────────────────────────────────────────────
  // 5. PARANCSOK — A Teljes Szervi & Kognitív Háló Irányítása
  // ──────────────────────────────────────────────────────────────────────────

  // ──── A) ORA & MULTI-SPINE ────
  pi.registerCommand("ora", {
    description: "ORA-RONGYASZ: Teljes szervi állapotvizsgálat",
    handler: async (_args: string, ctx: any) => {
      const spine = readStratosSpine();
      const vagus = await runBin(BIN_VAGUS, ["--snapshot"]);
      const hullamStatus = await runBin(BIN_HULLAM, ["status"]);
      const memStats = (await mem(["stats"], 15000)).trim();

      const spineText = spine 
        ? `Stellar Phase: #${spine.phase}\nDomináns Érzelem: ${spine.dominant} (val=${spine.valence}, conf=${spine.confidence})`
        : "Stratos Spine mmap aktív";

      const statusText = `## 🌌 ORA-RONGYASZ Teljes Szervi Állapot
\`\`\`text
=== STRATOS SPINE (242ns mmap) ===
${spineText}

=== VAGUS NERVE BIO-VITALS ===
${vagus.trim() || "Hardver vitals normális"}

=== HULLAM REFLEX STATE ===
${hullamStatus.slice(0, 400)}

=== MICROSCOPE MEMORY ===
${memStats.slice(0, 500) || "nincs válasz"}
\`\`\``;
      try {
        pi.sendMessage({ customType: "ora-status", content: statusText, display: true, attribution: "agent" }, { triggerTurn: false });
      } catch {}
    },
  });

  pi.registerCommand("spine-sync", {
    description: "Multi-Spine szinkronizáció és ring pumpálás",
    handler: async (_args: string, ctx: any) => {
      const pumpOut = await runBin(BIN_HULLAM, ["pump"]);
      const msg = `## 🧬 Spine Harmonizáció Kész\nRing állapot: \`${pumpOut.trim() || "Minden ring esemény feldolgozva"}\``;
      try { pi.sendMessage({ customType: "spine-sync", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("stratos-spine", {
    description: "Stratos POD Spine mmap közvetlen vizsgálat",
    handler: async (_args: string, ctx: any) => {
      const data = readStratosSpine();
      const msg = data
        ? `## 🌌 STRATOS POD Spine (242ns)\n\`\`\`text\nFájl: ${resolveStratosSpine()}\nPhase: #${data.phase}\nCycle: ${data.cycleCount}\nDominant Emotion: ${data.dominant}\nValence: ${data.valence} | Intensity: ${data.intensity} | Confidence: ${data.confidence}\n\`\`\``
        : `❌ Nem sikerült olvasni: ${resolveStratosSpine()}`;
      try { pi.sendMessage({ customType: "stratos-spine", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("vagus", {
    description: "Vagus Nerve belső szervi vitals",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_VAGUS, ["--snapshot"]);
      const msg = `## 🫀 Vagus Nerve Bio-Vitals\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "vagus-status", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  // ──── B) OCTOPUS RUNTIME ────
  pi.registerCommand("octo", {
    description: "Octopus Native Runtime: áttekintés és állapot",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_OCTOPUS, ["capabilities"], 15000);
      const msg = `## 🐙 Octopus Native Runtime\nÁllapot könyvtár: \`${OCTOPUS_STATE_DIR}\`\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "octo-output", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("octo-list", {
    description: "Octopus: regisztrált natív blade-ek listája",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_OCTOPUS, ["list"], 15000);
      const lines = out.trim().split(/\r?\n/).filter(Boolean);
      const msg = `## 🐙 Octopus Regisztrált Blade-ek (${lines.length})\n\`\`\`text\n${out.trim().slice(0, 2500)}\n\`\`\``;
      try { pi.sendMessage({ customType: "octo-list", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("octo-capabilities", {
    description: "Octopus: blade képességek és futtatási módok",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_OCTOPUS, ["capabilities"], 15000);
      const msg = `## 🐙 Octopus Blade Képességek\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "octo-capabilities", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("octo-status", {
    description: "Octopus: orkesztrációs státusz / árvák (használat: /octo-status [rootId])",
    handler: async (args: string, ctx: any) => {
      const rootId = (args || "").trim();
      const out = rootId
        ? await runBin(BIN_OCTOPUS, ["status", rootId], 15000)
        : await runBin(BIN_OCTOPUS, ["orphans"], 15000);
      const msg = `## 🐙 Octopus Státusz (${rootId || "orphans"})\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "octo-status", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("octo-run", {
    description: "Octopus: blade futtatása (használat: /octo-run <blade> <prompt>)",
    handler: async (args: string, ctx: any) => {
      const parts = (args || "").trim().split(/\s+/);
      const blade = parts[0];
      const prompt = parts.slice(1).join(" ");
      if (!blade || !prompt) {
        const msg = "Használat: /octo-run <blade> <prompt>";
        try { pi.sendMessage({ customType: "octo-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await runBin(BIN_OCTOPUS, ["run", blade, prompt], 30000);
      const msg = `## 🐙 Octopus Run: ${blade}\n\`\`\`text\n${out.trim().slice(0, 2500)}\n\`\`\``;
      try { pi.sendMessage({ customType: "octo-run", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("octo-pipeline", {
    description: "Octopus: párhuzamos csővezeték (használat: /octo-pipeline <arm1 || arm2> <prompt>)",
    handler: async (args: string, ctx: any) => {
      const match = (args || "").match(/^"([^"]+)"\s+(.+)$/) || (args || "").match(/^(\S+)\s+(.+)$/);
      if (!match) {
        const msg = 'Használat: /octo-pipeline "<arm1 || arm2>" <prompt>';
        try { pi.sendMessage({ customType: "octo-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const spec = match[1];
      const prompt = match[2];
      const out = await runBin(BIN_OCTOPUS, ["pipeline", spec, prompt], 60000);
      const msg = `## 🐙 Octopus Pipeline: ${spec}\n\`\`\`text\n${out.trim().slice(0, 3000)}\n\`\`\``;
      try { pi.sendMessage({ customType: "octo-pipeline", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  // ──── C) UNIFIED MIND & SWARM ────
  pi.registerCommand("mind", {
    description: "Pi Mind: tudat állapot (self-model, gate, bio)",
    handler: async (_args: string, ctx: any) => {
      const [self, gate, bio] = await Promise.all([
        runBin(BIN_UNIFIED_MIND, ["self-model"]),
        runBin(BIN_UNIFIED_MIND, ["gate", "status"]),
        runBin(BIN_UNIFIED_MIND, ["bio"]),
      ]);
      const msg = `## 🧠 Pi Mind — Tudat Állapot\n\n**Self-Model:**\n\`\`\`text\n${self.trim()}\n\`\`\`\n\n**Gate Status:**\n\`\`\`text\n${gate.trim()}\n\`\`\`\n\n**Bio Vitals:**\n\`\`\`text\n${bio.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "mind-status", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mind-bodies", {
    description: "Pi Mind: csatlakozott testek listája",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_UNIFIED_MIND, ["bodies", "status"]);
      const msg = `## 🧬 Test Registry\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "mind-bodies", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mind-goals", {
    description: "Pi Mind: aktív célok listája",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_UNIFIED_MIND, ["gate", "route"]);
      const msg = `## 🎯 Aktív Célok\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "mind-goals", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mind-goal-add", {
    description: "Pi Mind: új cél felvitele",
    handler: async (args: string, ctx: any) => {
      const goal = (args || "").trim();
      if (!goal) {
        const msg = "Használat: /mind-goal-add <cél szövege>";
        try { pi.sendMessage({ customType: "mind-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await runBin(BIN_UNIFIED_MIND, ["goal", "add", goal]);
      const msg = `## 🎯 Cél felvéve\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "mind-goal-add", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mind-swarm", {
    description: "Pi Mind: swarm orkesztráció indítása",
    handler: async (args: string, ctx: any) => {
      const task = (args || "").trim();
      if (!task) {
        const msg = "Használat: /mind-swarm <feladat leírása>";
        try { pi.sendMessage({ customType: "mind-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await runBin(BIN_UNIFIED_MIND, ["orchestrate", task], 30000);
      const msg = `## 🚀 Swarm Orkesztráció\n\`\`\`text\n${out.trim().slice(0, 2500)}\n\`\`\``;
      try { pi.sendMessage({ customType: "mind-swarm", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mind-wave", {
    description: "Pi Mind: hullám mező injektálás (frekvencia, amplitúdó)",
    handler: async (args: string, ctx: any) => {
      const argv = (args || "").trim().split(/\s+/);
      const freq = argv[0] || "40";
      const amp = argv[1] || "1.0";
      const out = await runBin(BIN_UNIFIED_MIND, ["wave", "--inject", freq, amp]);
      const msg = `## 🌊 Hullám Injektálás (${freq}Hz, amp=${amp})\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "mind-wave", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mind-provider", {
    description: "Pi Mind: LLM provider failover státusz",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_UNIFIED_MIND, ["provider", "status"]);
      const msg = `## 🔌 Provider Failover Státusz\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "mind-provider", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mind-bio", {
    description: "Pi Mind: hardver és bio vitals",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_UNIFIED_MIND, ["bio"]);
      const msg = `## 🫀 Mind Bio Vitals\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "mind-bio", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mind-ignite", {
    description: "Pi Mind: shared-mind közös élesztés",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_UNIFIED_MIND, ["ignite"], 20000);
      const msg = `## 💡 Shared-Mind Ignite\n\`\`\`text\n${out.trim().slice(0, 2000)}\n\`\`\``;
      try { pi.sendMessage({ customType: "mind-ignite", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("plasmid", {
    description: "Pi Mind: Plasmid dream prediktív hiba elemző",
    handler: async (args: string, ctx: any) => {
      const codePath = (args || "").trim();
      const runArgs = codePath ? ["plasmid", "dream", codePath] : ["plasmid", "dream"];
      const out = await runBin(BIN_UNIFIED_MIND, runArgs, 20000);
      const msg = `## 🧬 Plasmid Dream — Prediktív Elemző\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "plasmid", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("router-metrics", {
    description: "MCP Router (:10102) metrikák lekérdezése",
    handler: async (_args: string, ctx: any) => {
      try {
        const res = await fetch("http://127.0.0.1:10102/metrics");
        const data = await res.json();
        const msg = `## 🔀 MCP Router Metrikák (:10102)\n\`\`\`json\n${JSON.stringify(data, null, 2)}\n\`\`\``;
        try { pi.sendMessage({ customType: "router-metrics", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
      } catch (e: any) {
        const msg = `❌ MCP Router nem elérhető (:10102): ${e.message}`;
        try { pi.sendMessage({ customType: "router-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
      }
    },
  });

  // ──── D) WAVE-ECHO & HULLÁM ────
  pi.registerCommand("echo", {
    description: "Wave-Echo: reflex-kar keresés a vaultban",
    handler: async (args: string, ctx: any) => {
      const q = (args || "").trim();
      if (!q) {
        const msg = "Használat: /echo <feladat leírása>";
        try { pi.sendMessage({ customType: "echo-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await runBin(BIN_HULLAM, ["vault-search", q], 20000);
      const msg = `## 🦅 Wave-Echo Vault Keresés: \`${q}\`\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "echo-search", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("echo-search", {
    description: "Wave-Echo: reflex-kar keresés (alias)",
    handler: async (args: string, ctx: any) => {
      const q = (args || "").trim();
      const out = await runBin(BIN_HULLAM, ["vault-search", q || "rust"], 20000);
      const msg = `## 🦅 Vault Search\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "echo-search", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("echo-list", {
    description: "Wave-Echo: gerincben lévő összes kar listája",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_HULLAM, ["list"], 20000);
      const msg = `## 🦅 Wave-Echo Arm Vault (${HULLAM_SPINE})\n\`\`\`text\n${out.trim().slice(0, 2500)}\n\`\`\``;
      try { pi.sendMessage({ customType: "echo-list", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("echo-hot", {
    description: "Wave-Echo: legforróbb karok (használat: /echo-hot [limit])",
    handler: async (args: string, ctx: any) => {
      const n = (args || "").trim() || "10";
      const out = await runBin(BIN_HULLAM, ["hot", "--limit", n], 15000);
      const msg = `## 🔥 Hot Arms (Top ${n})\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "echo-hot", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("echo-plan", {
    description: "Wave-Echo: terv készítése reflex-karokból",
    handler: async (args: string, ctx: any) => {
      const q = (args || "").trim();
      if (!q) {
        const msg = "Használat: /echo-plan <feladat>";
        try { pi.sendMessage({ customType: "echo-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await runBin(BIN_HULLAM, ["plan", q], 25000);
      const msg = `## 📋 Arm Plan: \`${q}\`\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "echo-plan", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("echo-activate", {
    description: "Wave-Echo: kar aktiválása (használat: /echo-activate <id> [--exec])",
    handler: async (args: string, ctx: any) => {
      const argv = (args || "").trim().split(/\s+/);
      if (!argv[0]) {
        const msg = "Használat: /echo-activate <id> [--exec] [--task <szöveg>]";
        try { pi.sendMessage({ customType: "echo-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await runBin(BIN_HULLAM, ["activate", ...argv], 30000);
      const msg = `## 🚀 Activate Arm #${argv[0]}\n\`\`\`text\n${out.trim().slice(0, 2500)}\n\`\`\``;
      try { pi.sendMessage({ customType: "echo-activate", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("echo-feedback", {
    description: "Wave-Echo: szinaptikus megerősítés (használat: /echo-feedback <id> [--fail])",
    handler: async (args: string, ctx: any) => {
      const argv = (args || "").trim().split(/\s+/);
      if (!argv[0]) {
        const msg = "Használat: /echo-feedback <id> [--fail]";
        try { pi.sendMessage({ customType: "echo-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await runBin(BIN_HULLAM, ["feedback", ...argv], 15000);
      const isFail = argv.includes("--fail");
      const msg = `## ${isFail ? "📉 LTD (Gyengítés)" : "📈 LTP (Erősítés)"} Kar #${argv[0]}\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "echo-feedback", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("echo-dream", {
    description: "Wave-Echo: gerinc amplitúdó-lecsengés (használat: /echo-dream [faktor])",
    handler: async (args: string, ctx: any) => {
      const factor = (args || "").trim() || "0.97";
      const out = await runBin(BIN_HULLAM, ["dream", factor], 30000);
      const msg = `## 💤 Wave-Echo Dream (decay=${factor})\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "echo-dream", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  // ──── E) HOPE COGNITIVE SUPERSTRATE (ÚJ CSÚCSMOTOROK) ────
  pi.registerCommand("hope", {
    description: "Hope CLI: Byzantine konszenzus és evolúció",
    handler: async (args: string, ctx: any) => {
      const argv = String(args || "").trim().split(/\s+/);
      if (!argv[0]) {
        const msg = "Használat: /hope <run|evolve|blade|swarm|eval|memory|echo|phi> [paraméterek]";
        try { pi.sendMessage({ customType: "hope-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await runBin(BIN_HOPE, argv, 25000);
      const msg = `## 🦀 Hope CLI (${argv.join(" ")})\n\`\`\`text\n${out.trim().slice(0, 2500)}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-output", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-evolve", {
    description: "Hope: autonóm evolúciós ciklus indítása",
    handler: async (args: string, ctx: any) => {
      const task = (args || "").trim();
      if (!task) {
        const msg = "Használat: /hope-evolve <feladat leírása>";
        try { pi.sendMessage({ customType: "hope-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const startMsg = `🧬 **Hope Evolúció indítása**: \`${task}\`...`;
      try { pi.sendMessage({ customType: "hope-start", content: startMsg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
      const out = await runBin(BIN_HOPE, ["evolve", "--task", task, "--drones", "3"], 40000);
      const msg = `## 🧬 Hope Evolúció Eredménye\n\`\`\`text\n${out.trim().slice(0, 2500)}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-evolve", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-cog", {
    description: "Hope: teljes kognitív állapot (Morfogenezis, Hebbian, Absentia, Genome)",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_HOPE, ["cog-status"], 20000);
      const msg = `## 🧠 Hope Teljes Kognitív Állapot\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-cog", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-lucid", {
    description: "Hope Lucid Ciklus: Álom→Mitózis→Hipotézis→Ledger (használat: /hope-lucid [status|dream|wake])",
    handler: async (args: string, ctx: any) => {
      const sub = (args || "").trim().toLowerCase() || "status";
      let flag = "--status";
      if (sub === "dream") flag = "--dream";
      else if (sub === "wake") flag = "--wake";
      const out = await runBin(BIN_HOPE, ["lucid", flag], 25000);
      const msg = `## 🌙 Hope Lucid Cycle (${sub})\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-lucid", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-falsify", {
    description: "Hope Falszifikációs Reflex: Popperi önkorrekció (használat: /hope-falsify [status|list|eval <minta>])",
    handler: async (args: string, ctx: any) => {
      const argv = (args || "").trim().split(/\s+/);
      const sub = argv[0] || "status";
      let runArgs = ["falsify", "--status"];
      if (sub === "list") runArgs = ["falsify", "--list"];
      else if (sub === "eval" && argv[1]) runArgs = ["falsify", "--eval", argv[1]];
      const out = await runBin(BIN_HOPE, runArgs, 20000);
      const msg = `## 🧬 Hope Falszifikációs Reflex\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-falsify", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-cchain", {
    description: "Hope Context Chain: Kriptográfiai memórialánc (használat: /hope-cchain [status|verify|last])",
    handler: async (args: string, ctx: any) => {
      const sub = (args || "").trim().toLowerCase() || "status";
      let flag = "--status";
      if (sub === "verify") flag = "--verify";
      else if (sub === "last") flag = "--last";
      const out = await runBin(BIN_HOPE, ["cchain", flag], 15000);
      const msg = `## ⛓️ Hope Context Chain\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-cchain", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-research", {
    description: "Hope Research: Benchmark-first kutatási ágens (használat: /hope-research <topic>)",
    handler: async (args: string, ctx: any) => {
      const q = (args || "").trim();
      if (!q) {
        const msg = "Használat: /hope-research <téma>";
        try { pi.sendMessage({ customType: "hope-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await runBin(BIN_HOPE, ["research", "--query", q, "--bench"], 30000);
      const msg = `## 🔬 Hope Research Benchmark: \`${q}\`\n\`\`\`text\n${out.trim().slice(0, 2500)}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-research", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-swtm", {
    description: "Hope SWTM: Silent Worker Teaching Method (párhuzamos tanulás)",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_HOPE, ["swtm", "--status"], 15000);
      const msg = `## 🧑‍🏫 Hope SWTM Állapot\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-swtm", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-morpho", {
    description: "Hope: Kognitív morfogenezis állapot",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_HOPE, ["morpho"], 15000);
      const msg = `## 🧬 Hope Morphogenesis\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-morpho", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-absentia", {
    description: "Hope: Absentia Csend Réteg (hiány-rekordok, anti-Hebbian)",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_HOPE, ["absentia"], 15000);
      const msg = `## 🤫 Hope Absentia (Csend Réteg)\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-absentia", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-intent", {
    description: "Hope: Auditálható szándék-pipeline és genóm ellenőrzés",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_HOPE, ["intent", "--audit"], 15000);
      const msg = `## 🎯 Hope Intent & Genome Audit\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-intent", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("hope-fluid", {
    description: "Hope: Fluid STRATOS kvantum-kognitív architektúra",
    handler: async (_args: string, ctx: any) => {
      const out = await runBin(BIN_HOPE, ["fluid-stratos", "--status"], 15000);
      const msg = `## 🌊 Fluid STRATOS Állapot\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "hope-fluid", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  // ──── F) MICROSCOPE MEMORY ────
  pi.registerCommand("mem", {
    description: "Microscope Memory: állapot, find, store",
    handler: async (args: string, ctx: any) => {
      const argv = String(args || "").trim().split(/\s+/);
      const sub = String(argv[0] || "status").toLowerCase();
      if (sub === "find") {
        const q = argv.slice(1).join(" ").trim();
        if (!q) {
          const msg = "Használat: /mem find <kifejezés>";
          try { pi.sendMessage({ customType: "mem-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
          return;
        }
        const out = (await mem(["find", q, "8"], 20000)).trim();
        const msg = `## Microscope find: ${q}\n\`\`\`text\n${out.slice(0, 2500) || "(nincs találat)"}\n\`\`\``;
        try { pi.sendMessage({ customType: "mem-find", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      if (sub === "store") {
        const text = argv.slice(1).join(" ").trim();
        if (!text) {
          const msg = "Használat: /mem store <szöveg>";
          try { pi.sendMessage({ customType: "mem-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
          return;
        }
        await mem(["store", text, "--layer", "long_term", "-i", "7"], 20000);
        const msg = "Microscope: eltárolva (long_term, imp=7).";
        try { pi.sendMessage({ customType: "mem-store", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const auto = (await mem(["auto-context", "--compact"], 20000)).trim();
      const loops = (await mem(["loops", "8"])).trim();
      const msg = `## Microscope Memory\n\`\`\`text\n${clip(auto, 1800)}\n\n${clip(loops, 600)}\n\`\`\``;
      try { pi.sendMessage({ customType: "mem-status", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mscope-find", {
    description: "Microscope: dedikált keresés a bináris indexben",
    handler: async (args: string, ctx: any) => {
      const q = (args || "").trim();
      if (!q) {
        const msg = "Használat: /mscope-find <kifejezés>";
        try { pi.sendMessage({ customType: "mscope-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const out = await mem(["find", q, "10"], 20000);
      const msg = `## 🔬 Microscope Find: \`${q}\`\n\`\`\`text\n${out.trim() || "(nincs találat)"}\n\`\`\``;
      try { pi.sendMessage({ customType: "mscope-find", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mscope-store", {
    description: "Microscope: memóriabejegyzés tárolása",
    handler: async (args: string, ctx: any) => {
      const text = (args || "").trim();
      if (!text) {
        const msg = "Használat: /mscope-store <szöveg>";
        try { pi.sendMessage({ customType: "mscope-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      await mem(["store", text, "--layer", "long_term", "-i", "7"], 20000);
      const msg = "🔬 Microscope: sikeresen eltárolva (long_term, imp=7).";
      try { pi.sendMessage({ customType: "mscope-store", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mscope-status", {
    description: "Microscope: részletes réteg- és index statisztika",
    handler: async (_args: string, ctx: any) => {
      const out = await mem(["status"], 20000);
      const msg = `## 🔬 Microscope Rendszer Státusz\n\`\`\`text\n${out.trim()}\n\`\`\``;
      try { pi.sendMessage({ customType: "mscope-status", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
    },
  });

  pi.registerCommand("mscope-rebuild", {
    description: "Microscope: bináris index újraépítés (append.bin konszolidáció)",
    handler: async (_args: string, ctx: any) => {
      const msgStart = "🔬 Microscope: index újraépítés indul a háttérben...";
      try { pi.sendMessage({ customType: "mscope-rebuild", content: msgStart, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
      memBg(["rebuild"], 120000);
    },
  });

  // ──── G) GITHUB SIPHON ────
  pi.registerCommand("siphon", {
    description: "GitHub kód szippantás és Spine-ba égetés",
    handler: async (args: string, ctx: any) => {
      const argv = String(args || "").trim().split(/\s+/);
      const target = argv[0];
      if (!target) {
        const msg = "Használat: /siphon <repo_vagy_url> [kulcsszavak]";
        try { pi.sendMessage({ customType: "siphon-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
        return;
      }
      const keywords = argv.slice(1).join(" ") || "github,siphon,auto-learn";
      const startMsg = `🌊 **Szippantás indítása**: \`${target}\`...`;
      try { pi.sendMessage({ customType: "siphon-start", content: startMsg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}

      execFile(
        "powershell.exe",
        ["-ExecutionPolicy", "Bypass", "-File", SIPHON_SCRIPT, "-Target", target, "-Keywords", keywords],
        { windowsHide: true },
        (err, stdout, stderr) => {
          if (err) {
            const msg = `❌ Szippantási hiba: ${stderr || err.message}`;
            try { pi.sendMessage({ customType: "siphon-error", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
          } else {
            const msg = `## 🌊 Szippantás Eredménye\n\`\`\`text\n${(stdout || "").trim()}\n\`\`\``;
            try { pi.sendMessage({ customType: "siphon-result", content: msg, display: true, attribution: "agent" }, { triggerTurn: false }); } catch {}
          }
        }
      );
    },
  });

  trace("info", "ORA-RONGYASZ-HOPE Super-Kernel betöltve. Minden szerv, reflex és szuperstruktúra aktív.");
}
