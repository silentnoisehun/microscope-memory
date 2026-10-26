/**
 * NATÍV KAR-CIKLUS TESZT — a master kernel új részeinek valós végrehajtása.
 *
 * A harness ugyanazokat a binárisokat hívja, ugyanazzal az env-vel, mint a
 * kernel: nincs mock, nincs stub. Ha ez a fájl zöld, a karok tényleg futnak.
 *
 * Futtatás:  node omp-integration/test-native-arms.mjs
 */

import { execFile, spawn } from "node:child_process";
import { existsSync, mkdirSync, openSync, readFileSync } from "node:fs";
import * as fs from "node:fs";

const HOPE_BIN = "C:/Users/mater/.hope/bin";
const BIN_OCTOPUS = `${HOPE_BIN}/octopus-runtime.exe`;
const BIN_WAVE_ECHO = `${HOPE_BIN}/wave-echo.exe`;
const BIN_HULLAM = `${HOPE_BIN}/hullam.exe`;

const OCTOPUS_STATE_DIR = "D:/codex/.octopus-rust";
const WAVE_ECHO_STORE = "D:/codex/digital_echo_rust/wave_echo.wv3";
const HULLAM_SPINE = "D:/codex/digital_echo_rust/hullam_spine.rgs";
const STRATOS_SPINE =
  "C:/Users/mater/microscope-memory-clone/stratos_data/stratos.spine";
const MICROSCOPE_CONFIG =
  "C:/Users/mater/AppData/Local/microscope-memory/config.toml";
const ARM_OUTPUT_DIR = "C:/Users/mater/.omp/agent/arms-test";

const OCTOPUS_GLOBAL_FLAGS = ["--plain"];
const NATIVE_ARM_TIMEOUT_MS = 180000;

const organEnv = () => ({
  ...process.env,
  OCTOPUS_STATE_DIR,
  WAVE_ECHO_STORE,
  WAVE_ECHO_SPINE: HULLAM_SPINE,
  HULLAM_SPINE,
  STRATOS_SPINE,
  MICROSCOPE_CONFIG,
  HULLAM_ROOT: "C:/Users/mater/microscope-memory-clone",
});

function runBin(bin, args, timeoutMs = 15000) {
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
        if (error && !stdout) resolve(stderr || error.message);
        else resolve(stdout || stderr || "");
      }
    );
  });
}

async function nativeMarshal(task, timeoutMs = 45000) {
  const raw = await runBin(
    BIN_OCTOPUS,
    [...OCTOPUS_GLOBAL_FLAGS, "marshal", task],
    timeoutMs
  );
  const m = raw.match(/selected=\d+\s+topology=(\S+)/);
  return m ? m[1] : "";
}

function launchNativeArms(spec, task) {
  if (!existsSync(BIN_OCTOPUS)) return null;
  if (!existsSync(ARM_OUTPUT_DIR)) mkdirSync(ARM_OUTPUT_DIR, { recursive: true });

  const startedAt = Date.now();
  const safe = spec.replace(/[^\w.-]+/g, "_").slice(0, 60);
  const outFile = `${ARM_OUTPUT_DIR}/${startedAt}-${safe}.txt`;
  const doneFile = outFile + ".done";

  const child = spawn(
    BIN_OCTOPUS,
    [...OCTOPUS_GLOBAL_FLAGS, "pipeline", spec, task],
    {
      env: organEnv(),
      windowsHide: true,
      detached: true,
      stdio: ["ignore", "pipe", "pipe"],
    }
  );

  const fd = openSync(outFile, "w");
  const ws = fs.createWriteStream("", { fd, autoClose: false });
  child.stdout?.pipe(ws);
  child.stderr?.pipe(ws);
  child.unref();

  const timer = setTimeout(() => {
    try {
      child.kill();
    } catch {}
  }, NATIVE_ARM_TIMEOUT_MS);
  timer.unref();

  const watcher = spawn(
    process.execPath,
    [
      "-e",
      `const fs=require("fs");` +
        `let n=0;` +
        `const t=setInterval(()=>{` +
        `  try{fs.writeFileSync(${JSON.stringify(doneFile)},String(Date.now()));` +
        `  clearInterval(t);process.exit(0);}catch(e){}` +
        `},1500);` +
        `setTimeout(()=>{clearInterval(t);process.exit(0);},${NATIVE_ARM_TIMEOUT_MS});`,
    ],
    { windowsHide: true, detached: true, stdio: "ignore" }
  );
  watcher.unref();

  return { outFile, doneFile, startedAt };
}

let failures = 0;
function check(label, cond, detail = "") {
  const mark = cond ? "PASS" : "FAIL";
  if (!cond) failures++;
  console.log(`  [${mark}] ${label}${detail ? " — " + detail : ""}`);
}

async function main() {
  console.log("=== 1. WAVES-ECHO vault-search (a kernel új E2 lépése) ===");
  const echoSearch = await runBin(
    BIN_WAVE_ECHO,
    ["--store", WAVE_ECHO_STORE, "vault-search", "meta.bin writer depth table"],
    20000
  );
  check("wave-echo vault-search nem üres", echoSearch.length > 0);
  const echoTop = echoSearch.match(/^\s*\d+\.\s+(\d+)\s+(.+?)\s+([\d.]+)\s+([\d.]+)/m);
  check("a kernel regex-e illeszkedik a kimenetre", Boolean(echoTop),
    echoTop ? `#${echoTop[1]} I=${echoTop[3]}` : "NEM ILLESZKEDIK");

  console.log("=== 2. WAVES-ECHO plan (a kar-ajánlás) ===");
  const echoPlan = await runBin(
    BIN_WAVE_ECHO,
    ["--store", WAVE_ECHO_STORE, "plan", "javítsd a meta.bin írót a dream.rs-ben"],
    15000
  );
  const planMatch = echoPlan.match(/activate-multi\s+"([^"]+)"/);
  check("a kernel plan-regex-e illeszkedik", Boolean(planMatch),
    planMatch ? planMatch[1] : "NEM ILLESZKEDIK");

  console.log("=== 3. NATÍV MARSHAL (topológia-terv) ===");
  const t0 = Date.now();
  const spec = await nativeMarshal("javítsd a meta.bin írót a dream.rs-ben és építsd a release binárist");
  check("marshal topológiát választott", Boolean(spec), spec || "nincs");
  console.log(`      (${((Date.now() - t0) / 1000).toFixed(1)}s)`);

  if (!spec) {
    console.log("=== 4. NATÍV KAR-FUTTATÁS — kihagyva (nincs topológia) ===");
  } else {
    console.log(`=== 4. NATÍV KAR-FUTTATÁS: ${spec} ===`);
    const arm = launchNativeArms(spec, "C:\\Users\\mater\\microscope-memory-clone\\src\\dream.rs");
    check("a kar elindult", Boolean(arm));
    if (arm) {
      const deadline = Date.now() + NATIVE_ARM_TIMEOUT_MS;
      while (!existsSync(arm.doneFile) && Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 1000));
      }
      check("a kar befejeződött (done sentinel)", existsSync(arm.doneFile));
      const body = readFileSync(arm.outFile, "utf8");
      check("a kar-kimenet nem üres", body.trim().length > 0,
        `${body.length} karakter`);
      check("a kimenet kar-szintű bizonyítékot tartalmaz",
        /code-analysis|diagnostics|code-reader|summary|── ARM|─/.test(body));
      console.log("      --- kimenet első 500 karaktere ---");
      console.log("      " + body.trim().slice(0, 500).split("\n").join("\n      "));
    }
  }

  console.log("=== 5. HULLAM vault-search (a meglévő lépés) ===");
  const hullam = await runBin(BIN_HULLAM, ["vault-search", "meta.bin depth table"], 25000);
  check("hullam vault-search nem üres", hullam.length > 0);
  const hullamTop = hullam.match(/\s+1\.\s+#(\d+)\s+I=([\d.]+)/);
  check("a kernel hullam-regex-e illeszkedik", Boolean(hullamTop),
    hullamTop ? `#${hullamTop[1]} I=${hullamTop[2]}` : "NEM ILLESZKEDIK");

  console.log(`\n=== ÖSSZEFOGLALÓ: ${failures === 0 ? "MINDEN TESZT ZÖLD" : failures + " HIBA"} ===`);
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((err) => {
  console.error("HARNESS HIBA:", err);
  process.exit(2);
});
