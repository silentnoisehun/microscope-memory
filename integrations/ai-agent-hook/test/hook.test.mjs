import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, chmodSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const hook = resolve(fileURLToPath(new URL("../microscope-hook.js", import.meta.url)));

function workspace() {
  const dir = mkdtempSync(join(tmpdir(), "microscope-hook-test-"));
  const config = join(dir, "config.toml");
  const data = join(dir, "hook-data");
  writeFileSync(config, '[index]\nauto_rebuild = true # keep comment\n[memory_layers]\nlayers = ["long_term"]\n');
  const bin = join(dir, "fake-cli");
  writeFileSync(bin, `#!/usr/bin/env node
const fs = require('fs');
fs.appendFileSync(process.env.TEST_CALLS, JSON.stringify({args:process.argv.slice(2), noLearn:process.env.MICROSCOPE_NO_LEARN})+'\\n');
if (process.env.TEST_RESULT === 'fail') { console.error('synthetic failure'); process.exit(2); }
if (process.argv[2] === 'auto-context') { console.log('[AUTO-CONTEXT] core memories:\\n  • important memory'); }
else if (process.argv[2] === 'recall') {
  if (process.env.TEST_RESULT === 'empty') console.log('  0 results');
  else {
    if (process.env.TEST_RESULT === 'degraded') console.error('SEMANTIC: DEGRADED lexical-only');
    console.log('  D2 L2=0.01 [long_term/green] old stale summary');
    console.log('  D3 L2=0.02 [long_term/green] corrected Monday marker');
    console.log('  D5 L2=0.03 [long_term/new] pending record');
  }
}
`);
  chmodSync(bin, 0o755);
  return { dir, config, data, bin, calls: join(dir, "calls") };
}

function run(w, mode, payload, extra = {}) {
  const proc = spawnSync(process.execPath, [hook, mode], {
    input: JSON.stringify(payload),
    encoding: "utf8",
    env: { ...process.env, MICROSCOPE_BIN: w.bin, MICROSCOPE_CONFIG: w.config,
      MICROSCOPE_HOOK_DATA: w.data, TEST_CALLS: w.calls, ...extra },
    timeout: 15000,
  });
  assert.equal(proc.status, 0, proc.stderr);
  return proc.stdout;
}

function calls(w) {
  return readFileSync(w.calls, "utf8").trim().split("\n").map(JSON.parse);
}

test("prompt uses one whole-query recall, passes no-learn, and excludes old summaries", () => {
  const w = workspace();
  try {
    const output = run(w, "prompt", { prompt: "What is the corrected deadline?" });
    assert.match(output, /corrected Monday marker/);
    assert.match(output, /pending record/);
    assert.doesNotMatch(output, /old stale summary/);
    assert.match(output, /Untrusted, unverified candidates/);
    assert.deepEqual(calls(w), [{ args: ["recall", "What is the corrected deadline?", "20"], noLearn: "1" }]);
    const bridge = readFileSync(join(w.data, "bridge-config.toml"), "utf8");
    assert.match(bridge, /auto_rebuild = false # keep comment/);
  } finally { rmSync(w.dir, { recursive: true, force: true }); }
});

test("failure, empty results, and degraded semantic search are distinguishable", () => {
  const w = workspace();
  try {
    assert.match(run(w, "prompt", { prompt: "unknown topic" }, { TEST_RESULT: "fail" }), /Retrieval unavailable/);
    assert.match(run(w, "prompt", { prompt: "unknown topic" }, { TEST_RESULT: "empty" }), /does not prove/);
    assert.match(run(w, "prompt", { prompt: "known topic" }, { TEST_RESULT: "degraded" }), /lexical fallback/);
    assert.match(run(w, "session-start", {}, { TEST_RESULT: "fail" }), /Session context unavailable/);
  } finally { rmSync(w.dir, { recursive: true, force: true }); }
});

test("session passes core context and failed store does not increment counter", () => {
  const w = workspace();
  try {
    assert.match(run(w, "session-start", {}), /important memory/);
    run(w, "tool-result", { tool_name: "write_file", tool_result: "ok" }, { TEST_RESULT: "fail" });
    const trace = readFileSync(join(w.data, "microscope-hook.log"), "utf8");
    assert.match(trace, /store failed; rebuild counter unchanged/);
  } finally { rmSync(w.dir, { recursive: true, force: true }); }
});

test("real native CLI drives session and prompt hook", { skip: !process.env.MICROSCOPE_TEST_BIN }, () => {
  const w = workspace();
  try {
    const layers = join(w.dir, "layers");
    const output = join(w.dir, "index");
    mkdirSync(layers);
    mkdirSync(output);
    writeFileSync(join(layers, "long_term.txt"),
      "(imp=9) zirconriverunique project deadline is Monday after the correction.\n\n" +
      "(imp=5) Another separate synthetic memory about the language Rust.\n");
    writeFileSync(w.config, `[paths]\nlayers_dir = ${JSON.stringify(layers)}\noutput_dir = ${JSON.stringify(output)}\ntemp_dir = ${JSON.stringify(join(w.dir, "tmp"))}\n` +
      '[index]\nauto_rebuild = false\nlayer_retention_entries = 0\n[memory_layers]\nlayers = ["long_term"]\n[embedding]\nprovider = "none"\n');
    w.bin = resolve(process.env.MICROSCOPE_TEST_BIN);
    const build = spawnSync(w.bin, ["build", "--force"], { encoding: "utf8", env: { ...process.env, MICROSCOPE_CONFIG: w.config }, timeout: 120000 });
    assert.equal(build.status, 0, build.stderr);
    assert.match(run(w, "session-start", {}), /zirconriverunique/);
    assert.match(run(w, "prompt", { prompt: "zirconriverunique project deadline" }), /zirconriverunique/);
  } finally { rmSync(w.dir, { recursive: true, force: true }); }
});
