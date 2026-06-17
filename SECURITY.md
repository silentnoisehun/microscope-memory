# Security

## Reporting Vulnerabilities

Please open a GitHub issue tagged `security`. Do not include exploit details in public issues.

---

## Optional `stealth` Feature

Two modules are compiled only when `--features stealth` is explicitly passed.
They are **not** included in the default build or on crates.io.

### `src/antidebug.rs` — Soft VM / Sandbox Detection

**Purpose:** Detects whether the process is running inside a virtual machine or
automated sandbox, so that benchmarks and latency-sensitive paths can log a
warning or refuse to run (avoiding misleading performance numbers in CI VMs).

**What it does:**
- Reads CPUID bit 31 (hypervisor present flag) — standard, read-only CPU instruction.
- Checks two well-known Windows registry keys for VirtualBox / VMware Tools.

**What it does NOT do:**
- It does not terminate, crash, or alter the host system.
- It does not phone home or exfiltrate data.
- Score threshold ≥ 2 required before any action — a cloud VM with hypervisor
  but no VBox registry (e.g. AWS EC2) scores 1 and passes through normally.

**Scope:** Windows only (`windows-sys` dependency). Dead code on Linux/macOS.

### `src/obfuscate.rs` — Compile-time XOR String Obfuscation

**Purpose:** Obfuscates internal constant strings (non-sensitive configuration
keys and build-time tokens) so that naive `strings` extraction on the binary
does not reveal internal symbol names.

**What it does:**
- `xor_str!` macro: XOR-encodes a string literal at compile time.
- `decrypt()`: reverses the XOR at runtime when the value is needed.

**What it does NOT do:**
- No secrets, API keys, or credentials are stored here.
- XOR with a fixed key is not cryptographic — it is obfuscation only.

### Relation to MIT License

Both modules are MIT-licensed along with the rest of the project. The `stealth`
feature is opt-in and intended for use cases where tamper-resistance or
benchmark integrity matters (e.g. hardware-locked deployments). There is no
obligation to use it.
