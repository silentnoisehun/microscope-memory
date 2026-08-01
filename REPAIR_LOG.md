# Microscope Memory Repair Log

Baseline backup: `D:\codex\microscope-memory-backups\20260701-150656`

This log tracks the July 2026 stabilization pass. The local memory corpus is test data, so incompatible cognitive state may be reset when preserving it would attach learning to the wrong memories.

| ID | Severity | Finding | Repair | Verification | Status |
| --- | --- | --- | --- | --- | --- |
| MM-001 | Critical | Rebuilds reorder numeric block indexes while Hebbian state persists those indexes. Learning can move to unrelated memories. | Add stable SHA-256 block IDs, persist the ID generation used by cognitive state, and remap state by ID. Reset legacy state that has no trustworthy ID generation. | Reorder regression test plus state-size/generation checks. | In progress |
| MM-002 | Critical | Durable layer files retain only 50 entries and rebuild then deletes `append.bin`, causing silent loss. | Remove durable rolling truncation. Keep the append log after rebuild until a transactional WAL checkpoint exists. | Store more than 50 entries, rebuild, and verify all remain. | In progress |
| MM-003 | High | UTF-8 text is truncated at arbitrary bytes in blocks and append records. | Truncate only at Unicode scalar boundaries. | Multibyte boundary unit and integration tests. | In progress |
| MM-004 | High | Merkle leaves cover data only; header coordinates and metadata can be modified undetected. | Hash the exact 32-byte header together with block data and bump the index format. | Header-tamper verification test. | In progress |
| MM-005 | High | Runtime Hebbian drift is ignored by live recall, then baked into rebuilt headers. | Keep the base index immutable and apply effective coordinates during live ranking. | Ranking regression test; repeated rebuild remains deterministic. | Pending |
| MM-006 | High | CLI and MCP recall differ; MCP injects recent session words and writes recall output back as durable memories. | Share query normalization/scoring, honor `keyword_boost`, remove session contamination, and make generated recall traces temporary. | CLI/MCP parity and no-feedback-loop tests. | Pending |
| MM-007 | High | Energy decay ignores stored energy and uses unsigned subtraction for clock skew; dream pruning over-counts defaults. | Use saturating elapsed time, multiply by stored energy, and count only real state transitions. | Focused energy and repeated-dream tests. | Pending |
| MM-008 | Medium | `src/main.rs` contains a second layer of mojibake while carrying a useful `MICROSCOPE_CONFIG` change. | Restore clean tracked text and retain environment-based config selection. | UTF-8 scan, format, compile, and CLI smoke test. | Pending |
| MM-009 | Medium | Personal memory layer files remain tracked despite `.gitignore`. | Remove them from the Git index while retaining local files and sanitized fixtures only. | `git ls-files layers` contains no personal layer data. | Pending |
| MM-010 | Medium | Automatic paths have inconsistent secret filtering. | Centralize redaction for automatic capture while keeping explicit manual storage behavior visible. | Secret-pattern tests across hook and MCP paths. | Pending |

## Acceptance gates

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `cargo test --test integration`
- clean rebuild, CRC verification, Merkle verification, and recall smoke test
- before/after release benchmark recorded here
