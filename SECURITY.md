# Security

## Reporting Vulnerabilities

Please open a GitHub issue tagged `security`. Do not include exploit details in public issues.

---

## Threat Model

Microscope Memory is a **single-user, on-device** memory engine. It runs as a
local CLI / MCP server / Node.js addon and only reads/writes inside its own
`output/` directory. It does not open network listeners by default and does not
phone home.

| Surface | Exposure |
|---------|----------|
| Filesystem | `output/` (configurable via `config.toml`) |
| Network    | Optional `microscope-mem serve` on `127.0.0.1:6060` (off by default) |
| Native addon | `native/index.win32-x64-msvc.node` exposes 8 typed functions to JS/TS |
| MCP server | stdio only, no TCP/UDP listening |

## Codebase Hygiene

- **No `unsafe` outside `consciousness_seqlock.rs`** — the seqlock needs raw
  pointer access to swap atomics; the rest of the codebase is safe Rust.
- **Merkle + CRC16 verification** on every block at load time; see
  `microscope-mem verify` and `microscope-mem verify-merkle`.
- **Atomic append log** (`append.bin`) — repairs on crash via
  `microscope-mem doctor --fix`.
- **No third-party network calls** during build, recall, or dream — all I/O is
  local file or mmap.

## Dependencies of Note

| Crate | Used for | Risk surface |
|-------|----------|--------------|
| `windows-sys` | `VirtualQuery` on Windows for mmap protection check | Read-only memory info |
| `memmap2`     | mmap the binary index | Standard, no parsing |
| `reqwest`     | `federation.rs` cross-instance recall (opt-in) | TLS, only to peers you configure |
| `axum`        | legacy HTTP bridge (not started by `spine` CLI) | Disabled by default |
| `pyo3`        | Python bindings (only with `--features python`) | Not compiled in default build |

## What's NOT in the Codebase

For full transparency, the following capabilities are **not** present and are
not planned:

- No anti-VM, anti-sandbox, or anti-debug detection
- No direct syscalls or IAT camouflage
- No code or string obfuscation
- No polymorphic build signatures
- No network beaconing or telemetry
- No process memory reading (`NtReadVirtualMemory` and similar)

If you find any of the above in the source tree, please open a security issue.
