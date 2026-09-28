"""Measure recall latency through the MCP server, where one process serves many
queries, against the same questions the CLI harness uses.

The CLI path pays a process start and an embedding-provider construction per
query. A long-lived server should pay the model once. This script reports the
first call (which includes any one-time cost) separately from the warm ones, so
the amortisation is visible rather than assumed.

Usage:  python scripts/bench_mcp_recall.py [runs] [--config eval_config.toml]
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
from resonance_set import CASES  # noqa: E402


def percentile(xs, p):
    xs = sorted(xs)
    if not xs:
        return 0.0
    k = max(0, min(len(xs) - 1, int(round((p / 100) * (len(xs) - 1)))))
    return xs[k]


def main() -> int:
    runs = 20
    config = ROOT / "eval_config.toml"
    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--config" and i + 1 < len(args):
            config = ROOT / args[i + 1]
            i += 2
        else:
            runs = int(args[i])
            i += 1

    binary = ROOT / "target" / "release" / "microscope-mem.exe"
    if not binary.exists():
        binary = ROOT / "target" / "release" / "microscope-mem"
    if not binary.exists():
        print("release binary not found; run: cargo build --release --features native,embeddings")
        return 1

    # Read-only, so twenty identical questions do not warm each other up: an
    # MCP recall writes associative links, and without this the p50 would partly
    # measure what the previous calls left behind rather than the server. Note
    # what this then excludes -- the linking writes are part of the cost being
    # measured elsewhere, and with them suppressed this p50 is a floor, not the
    # number a real, learning server sees.
    env = dict(os.environ, MICROSCOPE_CONFIG=str(config), MICROSCOPE_NO_LEARN="1")
    proc = subprocess.Popen(
        [str(binary), "mcp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        encoding="utf-8",
        errors="replace",
        bufsize=1,
        cwd=str(ROOT),
        env=env,
    )

    def call(msg):
        proc.stdin.write(json.dumps(msg) + "\n")
        proc.stdin.flush()
        while True:
            line = proc.stdout.readline()
            if not line:
                raise RuntimeError("server closed stdout")
            try:
                resp = json.loads(line)
            except json.JSONDecodeError:
                # Startup banners and diagnostics can land on stdout; the
                # protocol response is the one carrying our id.
                continue
            if resp.get("id") == msg["id"]:
                return resp

    call({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "bench_mcp_recall", "version": "1"},
        },
    })

    cases = CASES[:runs]
    times = []
    first_text = ""
    for n, case in enumerate(cases):
        msg = {
            "jsonrpc": "2.0",
            "id": 100 + n,
            "method": "tools/call",
            "params": {
                "name": "memory_recall",
                "arguments": {"query": case.question, "k": 10},
            },
        }
        t0 = time.perf_counter()
        resp = call(msg)
        times.append((time.perf_counter() - t0) * 1000.0)
        if n == 0:
            content = resp.get("result", {}).get("content", [])
            first_text = json.dumps(content)[:160]

    try:
        proc.stdin.close()
        proc.wait(timeout=10)
    except Exception:
        proc.kill()

    warm = times[1:] or times
    print(f"=== MCP memory_recall, one process, n={len(times)} ===")
    print(f"  first call (one-time cost)   {times[0]:8.1f} ms")
    print(f"  warm  p50 {percentile(warm, 50):7.1f} ms   p95 {percentile(warm, 95):7.1f} ms")
    print(f"  all   p50 {percentile(times, 50):7.1f} ms   min {min(times):7.1f}  max {max(times):7.1f}")
    print(f"  first result: {first_text}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
