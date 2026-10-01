# Claims

Every claim this project publishes, where it comes from, and whether it holds.
Written for the external audit that asked for it; it is meant to be argued with.

A claim is **verified** only if it was checked against this tree during the work
that produced this file. **Historical** means it was true of an earlier commit
and is kept for continuity. **Unverified** means the document says it and
nothing here checked it. Nothing is marked verified on the strength of the
document that asserts it.

Scope of the checking: commit `fd345d5` onward, on Windows, Rust 1.98.1, release
profile. The Linux and wasm32 builds were not exercised by hand; CI covers Linux.

## The REST bridge had no entry point, and 31 other commands are still gone

Found on 2026-10-01, while chasing the auth and CORS behaviour the audit's
item 13 asks for. Both halves of this were absent from the audit.

`bridge::run` in src/bridge.rs had **no call site anywhere in src/**. None of the
118 CLI commands started it, and the command documented at
docs/ARCHITECTURE.md:617 to start it did not exist:

```
> microscope-mem bridge --port 6060
error: unrecognized subcommand 'bridge'
```

openapi.json therefore described a service no shipped binary served. The
existing REST contract test could not catch this: it compares the spec against
the route list in the source, so it confirms the document matches the code while
saying nothing about whether the code is reachable. That is a class of test
worth naming -- a contract test between a document and an unreachable
implementation passes forever.

**The cause was not an oversight.** `a962ad1`, titled "fix: resolve 33 build
errors", removed 32 command variants from src/cli.rs and left this comment at
main.rs:

```rust
// Cmd::Bridge removed — replaced by napi-rs native addon
// See native/src/lib.rs for the #[napi] equivalent
```

That replacement was never built. `native/` contains only the auto-generated
`index.js` loader and `index.d.ts`; there is no `native/src/lib.rs`, no `.node`
binary, and no `napi` entry in Cargo.toml or Cargo.lock. The comment points at a
file that does not exist, and the shim that was committed would throw on
`require()`.

**`Cmd::Bridge` is restored**, because the implementation in bridge.rs was never
removed -- that commit changed it by 11 lines. `microscope-mem bridge
[--host] [--port]` now starts it, defaulting to loopback, and the guard in
`bridge.rs:run` that refuses a non-loopback bind without an api_key now has
something to guard. `tests/rest_behaviour.rs` starts the real binary and covers
the spec endpoint, the /v1 routes, 400/404/405, the CORS default, and that guard.

**The other 31 commands are now restored.** They were bound again from
`a962ad1^` rather than rewritten, so the flag names, defaults and bodies are the
original ones. a962ad1 touched none of the modules -- `git show --stat` on
morphogenesis.rs, mental_sandbox.rs, impulse_control.rs, meta_supervision.rs,
code_memory.rs and chatgpt.rs returns nothing -- so the module APIs the old arms
were written against are the APIs that are there now.

Seven were documented as runnable when this was found, and six work again:

| Command | Where documented | Now |
|---|---|---|
| `morph` (6 mentions) | docs/ARCHITECTURE.md | restored, 18 flags |
| `import-chat-gpt` | docs/ARCHITECTURE.md | restored, 4 flags |
| `sandbox` | examples/cognitive_enhancement.md | restored, 4 flags |
| `impulse` | examples/cognitive_enhancement.md | restored, 6 flags |
| `meta` | examples/cognitive_enhancement.md | restored, 5 flags |
| `code` | COGNITIVE_ENHANCEMENTS.md | restored, 9 flags |
| `remember` | docs/ARCHITECTURE.md | never existed under that name; docs point at `store` |

The full set is 26 top-level commands plus the four `WmAction` sub-commands
(`wm show`, `wm push`, `wm decay`, `wm consolidate`), verified by running each
one: 26/26 answer `--help`. `Daydream` appeared in the removed list but is alive
in HEAD with different fields, so it was left alone; re-adding the old
definition is what produced "the name `Daydream` is defined multiple times" and
"has no field named `verbose`".

Three things did not come back verbatim, each a difference between then and now
rather than a rewrite:

- `timestamp_to_str`, a date-formatting helper the ImportChatGpt arm calls, went
  with everything else. Pure arithmetic, no dependencies, so it came back as it
  was.
- `store_memory` lost a fifth argument; the signature is now
  `store_memory_pipeline(config, text, layer, importance)`. The old trailing
  `None` was dropped rather than replaced with a guess.
- The old file was itself mojibake -- Hungarian accents through cp1250. Restoring
  it verbatim brought 48 of them back and `check_mojibake.py` caught every one.
  The repair is mechanical: a mangled pair is a leading U+0102, U+0139 or U+0042
  plus the cp1250 rendering of the second UTF-8 byte, so re-encode and decode as
  UTF-8 to recover the character, iterated to a fixed point because the damage is
  two levels deep in places. Checked against the words themselves -- Növekedési,
  konfiguráció, evolúciós, listázása, természetes, küszöb, idővel,
  alapértelmezett. One glyph the rule cannot invert is a stray U+00B3 in the
  density banner, present since 2bcc01d where the line was written; the same
  format string uses ASCII `->` for the same purpose further along, so it became
  `->`.


The method is worth recording because the first attempt at it was wrong. A
regex over the documentation suggested 8 bad commands; running each one showed
7 real and 1 false positive (`cargo`, from a `cargo test` line that followed a
wrapped `microscope-mem` reference), while missing `remember` entirely and
flagging `dream`, `pattern-exchange`, `store` and `build`, which all exist. The
81 subcommands parsed out of `--help` against 118 enum variants is the same kind
of gap. A check that only reads documents is not a check.

Also found while writing the routing test: `/v1/status` answers **500** when the
configured data directory does not exist, so the first request against a fresh
clone that has not been built yet is a server error rather than "no data yet".
Pinned by `status_on_uninitialised_store_is_500`; whether to change it is a
product decision.

## Architecture

| Claim | Source | Status |
|---|---|---|
| 9 depth levels, D0 to D8 | `src/lib.rs`, depth ranges in `meta.bin` | verified |
| 12 memory layers | `src/lib.rs` `LAYER_NAMES` | verified |
| 7 attention layers | `src/attention.rs` `NUM_LAYERS` | verified |
| 13 reinforcement mechanisms | `WHITEPAPER.md` 3.1-3.13 | verified (a different axis from the twelve; see the note added at the head of that paper) |
| Binary format: `microscope.bin` headers, `data.bin` text, `meta.bin` index | `src/reader.rs` | verified |
| Header stride 50 bytes current, 32 legacy | `src/lib.rs` `HEADER_SIZE` / `LEGACY_HEADER_SIZE` | verified |
| 16 KiB block data limit | `src/lib.rs` `BLOCK_DATA_SIZE` | verified |
| 256-character viewport | `BENCHMARKS.md`, `config.example.toml` | verified |
| MCP JSON-RPC over stdio | `src/mcp.rs`, driven end to end by `tests/mcp_stdio.rs` | verified |
| REST/OpenAPI bridge | `src/bridge.rs`, contract checked by `tests/rest_contract.rs` | verified |
| WASM library builds | `cargo check --target wasm32-unknown-unknown --no-default-features --features wasm` | verified |
| WASM binary builds | it does not, and cannot: the `[[bin]]` needs the `native` feature | verified (a negative result) |

## Correctness and safety

| Claim | Source | Status |
|---|---|---|
| Library tests pass | `cargo test --lib`, 443 passed, 1 ignored | verified |
| Integration tests pass | `cargo test --test integration`, 35 passed | verified |
| 519 tests across 11 targets | `cargo test --all-targets` | verified |
| No compiler warnings | `cargo clippy --all-targets --locked -- -D warnings` | verified |
| rustfmt clean | `cargo fmt --all -- --check` | verified |
| 57 `unsafe` blocks in 11 files, 15 with an adjacent safety note | `UNSAFE.md`, derived mechanically | verified |
| A truncated header file is refused at open | `tests/index_corruption.rs` | verified, and this was a real defect until `e0cea99` |
| Files are free of codepage corruption | `scripts/check_mojibake.py`, 345 tracked files | verified |

## Measurements

Every number below is corpus-dependent and none of it is a property of the
architecture on its own. The corpora are not interchangeable and the docs now say
which is which.

| Claim | Value | Source | Status |
|---|---|---|---|
| Demo corpus blocks | 1,285,288 | `BENCHMARKS.md` | unverified (not rebuilt here) |
| Evaluation index blocks | 967,587 | `BENCHMARKS.md` | unverified |
| SciFact index blocks | 5,633,165 | `microscope-mem stats` on `scifact_output` | verified |
| SciFact abstracts, depth 3 | 5,183 | same | verified |
| SciFact embedded vectors | 6,230 | same, and `meta.bin` | verified |
| In-process spatial query | ~112 us average | `BENCHMARKS.md` | unverified |
| Full `find`, warm page cache | ~186 ms | `BENCHMARKS.md` | unverified |
| Full `find`, cold page cache | up to ~3.6 s | `BENCHMARKS.md` | unverified |
| SciFact recall | R@1 53.1, R@5 74.8, R@10 81.8 | `BENCHMARKS.md` | unverified |
| SciFact latency | p50 156.3 ms | `BENCHMARKS.md` | unverified |
| FAISS comparison row | p50 0.41 ms, R@1 48.3 | `BENCHMARKS.md` | unverified, and **not like for like**: FAISS searches precomputed query vectors, Microscope embeds the query per call |
| "sub-microsecond" | D0 only | `WHITEPAPER.md` | historical, and narrower than it sounds: it is the inner loop at the shallowest depth, not the command |

## Claims that are deliberately not made

- That the codebase is memory safe. Nothing here has run under Miri or a
  sanitizer; the Windows FFI paths have not been run on a live system.
- That the WASM artifact is browser-verified. It compiles and it binds; it has
  not been driven in a browser.
- That the benchmark numbers reproduce on other hardware. No corpus hash, host
  description or repetition count is recorded with them yet.

## Still open

- The dependency gate checks licences and package sources, not advisories.
  `scripts/check_dependencies.py` reads `cargo metadata` and refuses a licence
  that is not on its list or a package that does not come from crates.io. It
  cannot tell whether a pinned version has a known CVE, because there is no
  advisory database in that job; `cargo deny check advisories` is the missing
  piece and needs a tool that is not installed here.
- The declared `rust-version` of 1.91 is derived, not built on: it was found by
  raising the value until `cargo clippy -- -D warnings` stopped reporting
  `incompatible_msrv`, after a hand-derived 1.88 turned out to be wrong about
  `str::floor_char_boundary` in src/narrative.rs. CI tests 1.98.1, so the floor
  is the lowest value nothing has objected to, not one a build has confirmed.
- `Cargo.toml` says 0.8.2 while the tree is 115 commits past the v0.9.2 tag, so
  the version a binary was built from cannot be read off the manifest.
- Two block totals are published for the same demo corpus: 1,285,288 in
  `BENCHMARKS.md` and 1,253,006 in `docs/ECOSYSTEM_DESCRIPTION.md` and
  `docs/FULL_SYSTEM_DESCRIPTION.md`. Neither was rebuilt here, so which is right
  is not established; the two documents disagree by about 32,000 blocks and are
  now both marked as point-in-time.
- `layers/session.txt` holds codepage damage from a lossy round that cannot be
  reversed: at least one byte is gone. It is runtime memory data, not source, and
  the checker skips it deliberately.
- The integration tests mutate `tests/fixtures/layers/long_term.txt`. Not
  reproduced or fixed here; it was noticed only because it kept appearing in
  otherwise unrelated diffs.
- Benchmark query sets were never versioned, so no recall figure in
  BENCHMARKS.md can be re-derived. The metadata section added there says which
  of the audit's eleven fields are present and which are absent.
