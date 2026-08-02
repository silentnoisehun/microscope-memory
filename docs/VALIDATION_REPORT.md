# Commitment-Enforcement — Validation Report

Verification artifact for the claim:

> The Octopus / Microscope Memory reference implementation instantiates all
> three architectural roles end-to-end.

This report pins the exact reproducible SHAs so the statement does not float
on a moving branch.

## 1. Microscope Memory

| Item | Value |
|------|-------|
| PR #3 (can_execute + load_engine_strict + mock proof) | merged |
| Merge commit (master) | `2b7085ec7b961820c68ca244857f162700db8907` |
| Release tag | `v0.9.1-commitment-enforcement` (tag `2b7085e`, object `d78db6712eae17de8870e3903ed4116fb5dc80f8`) |
| CI | `build-and-test: pass` (1m–2m) |

## 2. Octopus native runtime

| Feature | Value |
|---------|-------|
| PR #4 | native Microscope gate (fail-closed) wired into `execute_component` |
| Merge commit (main) | `087a373d1ec405e39a1b65023e0417e0804c9333` |
| Release tag | `v0.9.1-octopus-native-enforcement` (object `bdcc23d8...`) |
| CI (windows) | Format, Clippy `-D warnings`, Octopus tests, Bio tests, Release — **pass** |

## 3. Evidence

### Executor-boundary (mock, deterministic) — Microscope `tests/enforcement_executor_e2e.rs`

| decision | executor_call_count |
|----------|---------------------|
| BLOCKED | 0 |
| ATTRIBUTION_ERROR | 0 |
| ALLOWED | 1 |
| OVERRIDDEN | 1 |

Restart: a second engine session loads the persisted commitment + audit and
still blocks the same forbidden operation.

### Real binary (native `octopus-runtime.exe`) — `scripts/verify-enforcement-e2e.ps1`

| case | executor_call_count | exit |
|------|--------------------:|-----:|
| BLOCKED | 0 | 1 |
| ATTRIBUTION_ERROR | 0 | 1 |
| ALLOWED | 1 | 0 |
| OVERRIDDEN | 1 | 0 |
| restart + blocked (2 processes) | 0 | 1 |
| unprovisioned (fail-closed) | 0 | 1 |

Fail-closed: missing/corrupt state, unreadable audit, invalid chain, or a gate
error => the blade never reaches the native executor.

## 4. Traceability

- Microscope gate API: `EnforcementEngine::can_execute()`, `load_engine_strict()`.
- Octopus choke point: `execute_component` in `src/lib.rs`, gated via
  `src/enforcement.rs`.
- 7-file scope of PR #3 and 5-file scope of octopus PR #4.
