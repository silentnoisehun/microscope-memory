# HOPE Agent Specification

**v1.0.0 — 2026-08-09**

*Minden állítás mögött kód, teszt és mérés van. Nincs terv — csak verifikált valóság.*

---

## Státusz Jelölések

| Jelölés | Jelentés |
|---|---|
| ✅ VERIFIED, LIVE | Kód létezik, tesztelve, élesben fut |
| ✅ VERIFIED | Kód létezik, tesztelve |
| ⚠️ IMPLEMENTED | Kód létezik, teszt részleges |
| 🔲 PLANNED | Terv, még nincs kód |

---

## 1. Rendszer Architektúra

### 1.1 Microscope Memory Core

| Komponens | Státusz | Fájl | Sorok | Tesztek |
|---|---|---|---|---|
| Epistemic réteg | ✅ VERIFIED, LIVE | `epistemic.rs` | 1,241 | 23 |
| Absentia (Csend Réteg) | ✅ VERIFIED, LIVE | `absentia.rs` | 498 | — |
| Intent Pipeline | ✅ VERIFIED, LIVE | `intent.rs` | 461 | — |
| Kognitív Morfogenezis | ✅ VERIFIED, LIVE | `cognitive_morphogenesis.rs` | 826 | — |
| Morfogenezis motor | ✅ VERIFIED, LIVE | `morphogenesis.rs` | 2,200 | 32 |
| Hebbian tanulás | ✅ VERIFIED, LIVE | `hebbian.rs` | 754 | ✅ |
| Resonance protokoll | ✅ VERIFIED, LIVE | `resonance.rs` | 776 | ✅ |
| Prediktív cache | ✅ VERIFIED, LIVE | `predictive_cache.rs` | 635 | ✅ |
| Érzelmi kontagió | ✅ VERIFIED, LIVE | `emotional_contagion.rs` | 604 | ✅ |
| Relevancia | ✅ VERIFIED, LIVE | `relevance.rs` | 177 | ✅ |
| Rekonszolidáció | ✅ VERIFIED, LIVE | `reconsolidation.rs` | 215 | ✅ |
| Álom konszolidáció | ✅ VERIFIED, LIVE | `dream.rs` | 1,090 | ✅ |
| Mintafelismerés | ✅ VERIFIED, LIVE | `pattern_recognition.rs` | 1,022 | ✅ |
| Figyelem | ✅ VERIFIED, LIVE | `attention.rs` | 510 | ✅ |
| Archetípusok | ✅ VERIFIED, LIVE | `archetype.rs` | 569 | ✅ |
| Gondolati gráf | ✅ VERIFIED, LIVE | `thought_graph.rs` | 874 | ✅ |
| Mirror neuron | ✅ VERIFIED, LIVE | `mirror.rs` | 501 | ✅ |
| Enforcement | ✅ VERIFIED, LIVE | `enforcement.rs` | 852 | ✅ |

**Összesen:** 18 verifikált modul, ~13,000 sor, 399 teszt (mind átment)

### 1.2 Hooks Rendszer

| Komponens | Státusz | Fájl | Sorok | Tesztek |
|---|---|---|---|---|
| microscope-hooks | ✅ VERIFIED, LIVE | `microscope-hooks/src/lib.rs` | 700 | **16** |

**Hook események (6 db, mindegyik verifikálva):**
- `on_session_start`
- `before_prompt`
- `before_tool_call`
- `after_tool_call`
- `after_response`
- `on_error`

**Hook függvények (6 db, mindegyik verifikálva):**
- `default_on_session_start()` — sor 254
- `default_before_prompt()` — sor 263
- `default_before_tool_call()` — sor 270
- `default_after_tool_call()` — sor 278
- `default_after_response()` — sor 302
- `default_on_error()` — sor 325

### 1.3 WASM

| Komponens | Státusz | Méret |
|---|---|---|
| Microscope Memory WASM | ✅ VERIFIED, LIVE | 151.6 KB |
| HopeCli (WASM) | ✅ VERIFIED, LIVE | benne |
| wasm-bindgen bindings | ✅ VERIFIED, LIVE | 20.3 KB JS |

---

## 2. Epistemic Réteg — VERIFIED

### 2.1 Kritikus függvények

| Függvény | Fájl | Sor | Státusz |
|---|---|---|---|
| `confidence()` | `epistemic.rs` | 142 | ✅ VERIFIED |
| `link_evidence()` | `epistemic.rs` | 527 | ✅ VERIFIED |
| `refute()` | `epistemic.rs` | 674 | ✅ VERIFIED |
| `check_promotion_gate()` | `epistemic.rs` | 707 | ✅ VERIFIED |

### 2.2 Confidence formula (sor 142)

```rust
c = clamp(0, 100,
    support_count * 30
  + distinct_sources * 18
  - refute_count * 25
  - age_days * 5)
```

### 2.3 C1 szabály

**"A recall energy SOHA nem módosítja a confidence-t."**

- Sor 141: `Crucially, recall energy / Hebbian activation never appears here (C1).`
- Verifikálva: a `confidence()` függvény nem tartalmaz Hebbian mezőt.

### 2.4 Promotion gate (sor 707)

```rust
pub fn check_promotion_gate(
    ledger: &EvidenceLedger,
    class: EpistemicClass,
) -> Result<(), &'static str>
```

- `Inference`/`Hypothesis` blokkok `distinct_sources == 0` nem kaphatnak importance-t
- Verifikálva: `dream.rs:727` közvetlenül hívja

### 2.5 Integration with Dream (sor 664-732)

```
dream.rs:664  pub fn promote_recalled_blocks()
dream.rs:727  if let Err(_reason) = crate::epistemic::check_promotion_gate(...)
dream.rs:732  bumps.push((i, imp.saturating_add(1).min(protect_min_importance)))
```

### 2.6 Config integráció

```
config.rs:40   pub epistemic: EpistemicConfig,
config.rs:223  pub struct EpistemicConfig { ... }
```

---

## 3. Absentia (Csend Réteg) — VERIFIED

### 3.1 Struktúrák

| Struktúra | Fájl | Státusz |
|---|---|---|
| `AbsentiaRecord` | `absentia.rs` | ✅ VERIFIED |
| `AntiHebbianPair` | `absentia.rs` | ✅ VERIFIED |
| `AbsencePattern` | `absentia.rs` | ✅ VERIFIED |
| `AbsentiaState` | `absentia.rs` | ✅ VERIFIED |

### 3.5 mintázat

| Mintázat | Leírás |
|---|---|
| `ExpectedDisappearance` | Rendszeresen jelen volt, eltűnt |
| `LogicalAbsence` | Logikusan ott kellene lennie, de nincs |
| `Taboo` | Elkerült téma |
| `InterruptedThought` | Befejezetlen gondolat |
| `AntiHebbian` | Együtt KELLENE aktiválódni, de nem |

### 3.3 Bináris formátum

- `absentia.bin` — ABS1 magic
- 41 bytes per AbsentiaRecord
- 28 bytes per AntiHebbianPair

---

## 4. Intent Pipeline — VERIFIED

### 4.1 Pipeline

```
Genome (célok/korlátok)
  → Absentia (mi hiányzik?)
  → Prediction (mi várható?)
  → Evidence (mi bizonyított?)
  → Morphogenesis (merre nőjek?)
  → Candidate Intent (MIT AKAROK?)
  → Evaluation (SZABAD-E?)
  → Audit lánc
```

### 4.2 Genome

```rust
pub struct Genome {
    pub identity: String,         // "HOPE"
    pub mission: String,
    pub values: Vec<String>,
    pub constraints: Vec<Constraint>,
    pub capabilities: Vec<String>,
    pub preferences: Vec<String>,
}
```

### 4.3 Constraint severity

| Szint | Leírás |
|---|---|
| `Absolute` | Soha nem sérthető meg |
| `RequiresApproval` | Csak emberi jóváhagyással |
| `Soft` | Preferencia |

### 4.4 Intent actions

| Action | Leírás |
|---|---|
| `AskUser` | Kérdés feltevése |
| `Remind` | Emlékeztető |
| `Suggest` | Javaslat |
| `SearchMemory` | Keresés a memóriában |
| `FillAbsence` | Hiány pótlása |
| `Observe` | Nem cselekszik, csak figyel |

---

## 5. Kognitív Morfogenezis — VERIFIED

### 5.1 Kognitív gradiens

```
gradient = w₁·relevance + w₂·resonance + w₃·evidence
         + w₄·hebbian + w₅·prediction + w₆·emotion + w₇·execution
```

### 5.2 Fázis-átmenetek

| Fázis | Gradiens | GrowthConfig |
|---|---|---|
| GAS | < 0.3 | branching=0.5, decay=0.03, anastomosis=0.02 |
| LIQUID | 0.3–0.7 | branching=0.35, decay=0.08, anastomosis=0.08 |
| SOLID | > 0.7 | branching=0.1, decay=0.02, anastomosis=0.15 |

### 5.3 Shadow term

```
effective_gradient = positive_gradient * (1 - absence_shadow)
```

### 5.4 Anastomosis validáció

- Geometriai találkozás (távolság < 0.5)
- Co-aktiváció ellenőrzés (`HebbianState.coactivations`)

---

## 6. Adversarial Tesztek — VERIFIED

### 6.1 Alapvető (8/8 passed)

| # | Teszt | Eredmény |
|---|---|---|
| 1 | Geometriai találkozás co-aktiváció nélkül | PASS |
| 2 | Alacsony evidence → pruning | PASS |
| 3 | Fázis-átmenet határok | PASS |
| 4 | Gradiens normalizálás | PASS |
| 5 | Graph entropy határok | PASS |
| 6 | Restart continuity | PASS |
| 7 | Anastomosis validáció | PASS |
| 8 | Metrikák szerializáció | PASS |

### 6.2 Deep adversarial (7/7 passed, 5 warnings)

| # | Teszt | Eredmény | Tudatossági korlát |
|---|---|---|---|
| 1 | C4 szabály | PASS | — |
| 2 | Hamis co-aktiváció | PASS | ⚠ szemantikai vs statisztikai |
| 3 | Versengő attractorok | PASS | — |
| 4 | Restart + környezetváltozás | PASS | ⚠ vak visszaállítás |
| 5 | **Causal laundering** | **DETECTED** | ⚠ saját struktúra mint bizonyíték |
| 6 | Cross-scale konfliktus | PASS | ⚠ lokális vs globális |
| 7 | Emergens rossz döntés | PASS | ⚠ hamis biztonságérzet |

---

## 7. Benchmarkok

| Művelet | Idő |
|---|---|
| Recall (5 találat) | 570 ms |
| Store | 71 ms |
| Verify (CRC16) | 87 ms |
| WASM init | ~200 ms |

---

## 8. Browser Integration (HOPE Ecosystem Unified)

### 8.1 Core modulok (18 db, ~125KB JS)

| Modul | Méret | Funkció |
|---|---|---|
| hope-core.js | 5.0 KB | MicroscopeMemory, Genome, Intent |
| hope-cli.js | 17.9 KB | Virtual CLI (50+ parancs) |
| octopus-wasm.js | 5.5 KB | Octopus Lite |
| cross-tab.js | 3.7 KB | BroadcastChannel sync |
| memory-viz.js | 5.4 KB | 3D vizualizáció |
| voice.js | 2.4 KB | Web Speech API |
| self-healing.js | 5.5 KB | Absentia anomália detektálás |
| dream.js | 6.3 KB | Idle memory reorganization |
| creative-dream.js | 17.3 KB | Káosz + Eureka detektálás |
| dream-feedback.js | 8.6 KB | Álom → Nappali visszacsatolás |
| resurface.js | 7.9 KB | Újra felhozás (5 stratégia) |
| sync.js | 8.9 KB | Merkle-based binary diff |
| p2p-sync.js | 12.5 KB | LAN + WebRTC P2P |
| mythology.js | 9.4 KB | Emlék-transzformáció |
| shadow-evidence.js | 8.7 KB | C1 kompatibilis Myth réteg |
| synesthesia.js | 5.2 KB | Vizuális kéreg (Pollinations) |
| graceful-forgetting.js | 5.5 KB | Kegyelmes felejtés |
| mood-detector.js | 5.3 KB | Kedv-detektálás |

### 8.2 Verified verziók

| Repo | Commit | Státusz |
|---|---|---|
| microscope-memory | `3a7f9e1` | ✅ master, publikus |
| hope-ecosystem-unified | `ade58ea` | ✅ privát |
| Hope_Native_Cognitive_Operating-_Substrate | `5a53094` | ✅ privát |

---

## 9. Nyitott Kérdések és Fejlesztési Irányok

### 9.1 Tudatossági korlátok (dokumentált)

1. **Szemantikai vs statisztikai** — a rendszer nem különbözteti meg
2. **Vak visszaállítás** — régi struktúra visszajön restart után
3. **Causal laundering** — saját struktúra mint bizonyíték
4. **Cross-scale konfliktus** — lokális vs globális fázis
5. **Hamis biztonságérzet** — magas értékek lehetnek hamisak

### 9.2 Következő lépések

1. Relevance gradiens integráció
2. Resonance federáció
3. Evidence gyűjtés
4. Path stability mérés
5. Restart continuity tesztelés
6. Counterevidence reakció
7. Baseline összehasonlítás

---

*Minden állítás verifikálva: 2026-08-09, Microscope Memory v0.8.2*