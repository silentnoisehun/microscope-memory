# Public Demo Release Checklist — v0.1.0-public-demo

> **Memory is not a static vault.**  
> **Memory is a living structure.**

```
ARCHITECTURAL MEMORY · PUBLIC DEMO

T0      Fresh safe index
T100    Clusters forming
T1000   Context fields crystallize
T5000   Public demo locked
```

---

## 1. Run Full Test Suite

```powershell
cd D:\codex\microscope-memory
cargo test --workspace --release
```

**Expected: 285+ tests passing**
- microscope-memory: 269 tests
- microscope-hooks: 16 tests
- Total: 285 tests

## 2. Verify Public Demo Configuration

```powershell
type examples\config.public-demo.toml
```

**Expected:**
```toml
[hooks]
enabled = true
read_only = true
write_enabled = false
min_importance = 3
```

## 3. Verify Safety Defaults

- [ ] `read_only = true` — no memory writes from hooks
- [ ] `write_enabled = false` — after_response hook disabled
- [ ] `min_importance = 3` — low-importance memories filtered
- [ ] Secret filtering active (passwords, tokens, API keys)
- [ ] stderr-only logging (never stdout)

## 4. Verify No Private Dataset Path

```powershell
Select-String -Path config.toml -Pattern "layers_dir|output_dir"
```

**Expected:** No paths pointing to private data. Demo dataset only.

## 5. Verify MCP Tools Respond

```powershell
echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}' | .\target\release\microscope-mem.exe mcp
```

**Expected:** Returns list of MCP tools including:
- memory_status
- memory_recall
- memory_find
- memory_look
- memory_auto_context

## 6. Verify Write Tools Are Not Exposed

Check that no write/delete/dangerous tools are exposed in public mode:
- [ ] No memory_store in tools/list
- [ ] No memory_build in tools/list
- [ ] No memory_rebuild in tools/list
- [ ] No memory_consolidate in tools/list
- [ ] No memory_dream in tools/list

## 7. Verify Secret Filtering

```powershell
echo '{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"memory_recall\",\"arguments\":{\"query\":\"password is secret\"}}}' | .\target\release\microscope-mem.exe mcp
```

**Expected:** Query is processed but no secret is stored in memory.

## 8. Build Release Binary

```powershell
cargo build --release
```

**Expected:** Binary at `target/release/microscope-mem.exe`

## 9. Version Label

```powershell
git tag v0.1.0-public-demo
git push origin v0.1.0-public-demo
```

## 10. Final Verification

- [ ] All 285+ tests pass
- [ ] Release binary builds cleanly
- [ ] Public demo config is active
- [ ] No private data paths
- [ ] MCP tools respond correctly
- [ ] Write tools are hidden
- [ ] Secret filtering works
- [ ] Version tag is set

---

```
ARCHITECTURAL MEMORY
TEMPORAL EVOLUTION

T0      Fresh safe index
T100    Clusters forming
T1000   Context fields crystallize
T5000   Public demo locked
```
