# Kognitív Morfogenezis Architektúra

**v0.1 — 2026-08-09**

*Auditálható emergens viselkedés egy élő memóriahálózaton.*

---

## 1. Cél

A Microscope Memory meglévő kognitív moduljai — Morphogenesis, Hebbian, Resonance,
Epistemic, Emotional Contagion, Predictive Cache, Attention — jelenleg külön-külön
működnek. Ez a dokumentum leírja, hogyan kapcsolódnak össze egyetlen dinamikus
élő hálózattá, ahol minden modul kimenete megváltoztatja azt a teret, amelyben a
többi modul következő aktivitása kialakul.

A cél nem új modul építése, hanem a meglévő primitívek összekapcsolása — úgy, hogy
minden emergens viselkedés visszavezethő, auditálható és reprodukálható legyen.

---

## 2. Alapelvek

1. **Auditálhatóság mindenekelőtt.** Minden hifa-növekedés visszavezethető egy
   gradiensre, minden gradiens konkrét modul-kimenetekre, minden anastomosis
   dokumentálható mint két kognitív útvonal konvergenciája.

2. **Baseline-first.** Minden metrikát mérünk Mycelium nélkül és Myceliummal,
   ugyanazon a workloadon. Nincs "hű, emergens" — csak mérhető különbség.

3. **Moduláris mérhetőség.** Az integráció nem ronthatja el a meglévő modulok
   külön mérhetőségét. Minden modul továbbra is önállóan tesztelhető.

4. **Visszacsatolás a lényeg.** Egy modul kimenete megváltoztatja a teret, amelyben
   a többi modul következő aktivitása kialakul. Ez a feedback loop az, ami az
   emergens viselkedést létrehozza.

5. **Fázis-átmenetek.** A hálózat állapota három fázisban mozog: GAS (keresés),
   LIQUID (alkalmazkodás), SOLID (konszolidáció). A fázist a gradiens erőssége
   határozza meg.

---

## 3. A Kognitív Gradiens

### 3.1 Jelenlegi állapot

A `MorphogenField` (`morphogenesis.rs:196`) jelenleg egy absztrakt 3D koncentrációs
tér:

```rust
pub struct MorphogenField {
    pub gradients: HashMap<(i32, i32, i32), f64>,  // absztrakt koncentráció
    pub attractors: Vec<(f64, f64, f64, f64)>,     // (x, y, z, strength)
    pub repellents: Vec<(f64, f64, f64, f64)>,
    pub diffusion_rate: f64,                        // 0.1
    pub evaporation_rate: f64,                      // 0.01
}
```

A `concentration_at()` trilineáris interpolációval számol koncentrációt — attraktorok
hozzáadása, repellensek kivonása. A `mycelium_growth()` ezt a gradienst követi.

### 3.2 Kognitív gradiens formula

A gradiens értéke egy adott (x, y, z) pontban — ahol a koordináták kognitív
állapotteret reprezentálnak, nem fizikai helyet:

```
gradient(x,y,z) = w₁ · relevance(block)
                + w₂ · resonance(pulse_strength)
                + w₃ · evidence(confidence / 100.0)
                + w₄ · hebbian(energy)
                + w₅ · prediction(hit_rate)
                + w₆ · emotion(valence, arousal)
                + w₇ · execution_outcome(success_weight)
```

A súlyok (`w₁..w₇`) adaptívak — a rendszer tanulja, melyik jelzőforrás mennyire
megbízható az adott kontextusban.

### 3.3 Modul → gradiens kapcsolódás

| Gradiens komponens | Forrás modul | Adatstruktúra | Konverzió |
|---|---|---|---|
| `relevance` | `relevance.rs` | `RelevanceQuery.lexical_score()` → `f32 [0,1]` | Közvetlen |
| `resonance` | `resonance.rs` | `ResonanceState.field[(x,y,z)]` → `f32` | Normalizálás max-ra |
| `evidence` | `epistemic.rs` | `EvidenceRecord.confidence` → `u8 [0,100]` | `/ 100.0` |
| `hebbian` | `hebbian.rs` | `ActivationRecord.energy` → `f32 [0,1]` | Közvetlen |
| `prediction` | `predictive_cache.rs` | `CacheStats.hit_rate()` → `f32 [0,1]` | Közvetlen |
| `emotion` | `emotional_contagion.rs` | `EmotionalSnapshot.valence` → `f32 [-1,1]` | `(val+1)/2` → [0,1] |
| `execution` | `pipeline.rs` | Pipeline outcome → success/fail weight | `outcome_weight()` |

### 3.4 Gradiens-építés folyamata

Minden recall/query után:

1. `HebbianState.record_activation()` frissíti az aktivációs energiákat
2. `ResonanceState.emit_pulse()` kibocsát egy pulzust az aktivált blokkokról
3. `EvidenceLedger` confidence-értékei elérhetők minden blokkhoz
4. `PredictiveCache.evaluate()` frissíti a hit-rate-et
5. `EmotionalContagionState.capture_local()` frissíti az érzelmi teret
6. A kognitív gradiens újraszámolódik: az összes modul kimenete bekerül a
   `MorphogenField.gradients`-be

---

## 4. Hálózat-növekedés: Mycelium a kognitív térben

### 4.1 A hifa mint kognitív útvonal

A `mycelium_growth()` (`morphogenesis.rs:695`) jelenleg:

- Seed-ből indul, hifák nőnek ki véletlen irányokba
- A `concentration_at()` gradienst követik
- Anastomosis: két hifa találkozásakor fúzió történhet
- Energiakorlát: minden hifacsúcsnak van energiája és mélységkorlátja

Kognitív kontextusban:

| Morphogenesis fogalom | Kognitív megfelelő |
|---|---|
| Seed | Egy kiindulási memória-blok vagy kérdés |
| Hypha tip | Egy aktív keresési útvonal vége |
| Gradiens-követés | Relevancia + rezonáncia + bizonyíték irányába növés |
| Anastomosis | Két külön kognitív útvonal összeérése |
| Lokális energia | `ActivationRecord.energy` (Hebbian) |
| Depth limit | Exploration budget / attention budget |
| Growth | Új asszociáció létrejötte |
| Pruning | Használatlan útvonalak elhalása (`prune_idle_cycles`) |

### 4.2 Növekedési konfiguráció

A `GrowthConfig` (`morphogenesis.rs:332`) kognitív interpretációja:

```rust
GrowthConfig {
    max_nodes: 500,                    // max asszociáció egy keresésben
    branching_probability: 0.35,       // milyen gyakran ágazik el egy útvonal
    energy_decay: 0.08,                // mennyit veszít egy útvonal lépésenként
    min_energy_for_branch: 25.0,       // min. energia elágazáshoz
    max_depth: 20,                     // exploration budget
    anastomosis_probability: 0.08,     // két útvonal összeérési esélye
    prune_idle_cycles: 10,             // hanyadik ciklus után haljon el egy ág
}
```

### 4.3 MorphNode kognitív jelentése

A `MorphNode` (`morphogenesis.rs:469`) minden egyes megtalált vagy létrehozott
asszociáció:

```rust
MorphNode {
    id: usize,                         // egyedi azonosító
    position: (f64, f64, f64),         // pozíció a kognitív térben
    node_type: NodeType,               // Root | Branch | Leaf | Junction
    name: String,                      // pl. "relevance_42" vagy "resonance_cluster_7"
    energy: f64,                       // Hebbian energy
    depth: u32,                         // hány lépésre van a seed-től
    metadata: HashMap<String, String>, // audit-információk
}
```

### 4.4 MorphConnection kognitív jelentése

A `MorphConnection` (`morphogenesis.rs:492`) egy asszociáció két node között:

```rust
MorphConnection {
    from_node: usize,                  // forrás asszociáció
    to_node: usize,                    // cél asszociáció
    weight: f64,                       // kapcsolat erőssége
    connection_type: ConnectionType,   // Synaptic | Gap | Chemical | Dendritic
    is_active: bool,                   // él-e még
    age_cycles: u32,                   // hány ciklus óta létezik
}
```

---

## 5. Fázis-átmenetek: GAS / LIQUID / SOLID

### 5.1 A három fázis

A hálózat állapota a gradiens erősségétől függ:

| Fázis | Gradiens erősség | Viselkedés | Analógia |
|---|---|---|---|
| **GAS** (gáz) | `< 0.3` | Szabad exploráció, véletlen irányok, nagy diverzitás | Gáz molekulák: kaotikus, minden irányba |
| **LIQUID** (folyékony) | `0.3 – 0.7` | Gradiens-követés, alkalmazkodás, áramlás | Víz: követi a lejtőt, alkalmazkodik |
| **SOLID** (szilárd) | `> 0.7` | Rögzített útvonalak, megbízható asszociációk | Kristály: stabil, ismétlődő minta |

### 5.2 Fázis-határok

A fázist a gradiens átlagos erőssége határozza meg egy adott régióban:

```rust
pub enum Phase {
    Gas,      // szabad exploráció
    Liquid,   // gradiens-követés
    Solid,    // konszolidált útvonalak
}

fn determine_phase(avg_gradient: f64) -> Phase {
    if avg_gradient < 0.3 { Phase::Gas }
    else if avg_gradient < 0.7 { Phase::Liquid }
    else { Phase::Solid }
}
```

### 5.3 Fázis-specifikus viselkedés

**GAS fázis:**
- `branching_probability` magas (0.5+)
- `energy_decay` alacsony (0.03)
- `anastomosis_probability` alacsony (0.02)
- Cél: minél több útvonal felfedezése

**LIQUID fázis:**
- `branching_probability` közepes (0.35)
- `energy_decay` közepes (0.08)
- `anastomosis_probability` közepes (0.08)
- Cél: a legjobb útvonalak megtalálása

**SOLID fázis:**
- `branching_probability` alacsony (0.1)
- `energy_decay` alacsony (0.02) — az utak stabilak
- `anastomosis_probability` magas (0.15) — az utak összekapcsolódnak
- Cél: a megtalált útvonalak megerősítése és összekapcsolása

---

## 6. Modul-integrációs terv

### 6.1 MorphogenField ↔ Hebbian

**Kapcsolat:** Az `ActivationRecord.energy` értékek attraktorként kerülnek a
`MorphogenField`-be.

```rust
// Pseudocode: Hebbian → MorphogenField szinkronizáció
fn sync_hebbian_to_field(hebb: &HebbianState, field: &mut MorphogenField, headers: &[BlockHeader]) {
    for (i, rec) in hebb.activations.iter().enumerate() {
        if rec.energy > 0.1 {
            let h = &headers[i];
            field.add_attractor(h.x, h.y, h.z, rec.energy as f64);
        }
    }
}
```

**Bináris:** `activations.bin` (HEB1) → `MorphogenField.attractors`

### 6.2 MorphogenField ↔ Resonance

**Kapcsolat:** A `ResonanceState.field` értékek gradiens-forrásként kerülnek a
`MorphogenField`-be.

```rust
fn sync_resonance_to_field(res: &ResonanceState, field: &mut MorphogenField) {
    for (&(x, y, z), &strength) in &res.field {
        let fx = x as f64 * 0.05; // de-quantize
        let fy = y as f64 * 0.05;
        let fz = z as f64 * 0.05;
        field.add_attractor(fx, fy, fz, strength as f64);
    }
}
```

**Bináris:** `pulses.bin` (PLS1) → `MorphogenField.gradients`

### 6.3 MorphogenField ↔ Epistemic

**Kapcsolat:** Az `EvidenceRecord.confidence` értékek modulálják a gradiens erősségét.

```rust
fn apply_evidence_modulation(field: &mut MorphogenField, ledger: &EvidenceLedger, headers: &[BlockHeader]) {
    for (i, h) in headers.iter().enumerate() {
        if let Some(record) = ledger.records.get(&h.content_hash) {
            let conf = record.confidence as f64 / 100.0;
            // Magasabb confidence → erősebb gradiens
            let key = (h.x.floor() as i32, h.y.floor() as i32, h.z.floor() as i32);
            if let Some(val) = field.gradients.get_mut(&key) {
                *val *= 1.0 + conf; // confidence boost
            }
        }
    }
}
```

**Bináris:** `evidence.bin` (EVD1) → `MorphogenField.gradients` szorzó

### 6.4 MorphogenField ↔ Predictive Cache

**Kapcsolat:** A `CacheStats.hit_rate()` érték modulálja a gradiens globális erősségét.

```rust
fn apply_prediction_modulation(field: &mut MorphogenField, cache: &PredictiveCache) {
    let hit_rate = cache.stats.hit_rate();
    // Magasabb hit_rate → a gradiens megbízhatóbb → erősebb
    let modulation = 1.0 + hit_rate as f64;
    for val in field.gradients.values_mut() {
        *val *= modulation;
    }
}
```

**Bináris:** `predictive_cache.bin` → `MorphogenField` globális szorzó

### 6.5 MorphogenField ↔ Emotion

**Kapcsolat:** Az `EmotionalSnapshot.valence` és `total_energy` modulálják a gradiens
dinamikáját.

```rust
fn apply_emotion_modulation(field: &mut MorphogenField, emo: &EmotionalContagionState) {
    if let Some(ref snap) = emo.local_snapshot {
        // Pozitív valence → erősebb gradiens (attraktor)
        // Negatív valence → gyengébb gradiens (repellens)
        let valence_factor = (snap.valence + 1.0) / 2.0; // [-1,1] → [0,1]
        for val in field.gradients.values_mut() {
            *val *= 0.5 + valence_factor;
        }
    }
}
```

**Bináris:** `emotional_field.bin` (EMO1) → `MorphogenField` valence-szorzó

### 6.6 Anastomosis ↔ Co-activation

**Kapcsolat:** Amikor két hifa találkozik (anastomosis), a rendszer ellenőrzi, hogy
a két útvonal által érintett blokkok co-aktiváltak-e a `HebbianState`-ben.

```rust
fn validate_anastomosis(
    path_a: &[usize],  // hifa A által érintett node-ok
    path_b: &[usize],  // hifa B által érintett node-ok
    hebb: &HebbianState,
) -> bool {
    // Keresünk co-aktivációs párt a két útvonal között
    for &a in path_a {
        for &b in path_b {
            let key = (a.min(b) as u32, a.max(b) as u32);
            if hebb.coactivations.contains_key(&key) {
                return true; // van co-aktiváció → valid anastomosis
            }
        }
    }
    false
}
```

**Bináris:** `coactivations.bin` (COA1) → anastomosis validáció

---

## 7. Auditálhatósági lánc

### 7.1 Az auditálás elve

Minden emergens viselkedés visszavezethető a következő láncon:

```
T0 → aktiváció (melyik blokkok aktiválódtak)
T1 → gradiens (milyen kognitív gradiens alakult ki)
T2 → növekedés (merre nőtt a hifa)
T3 → anastomosis (hol ért össze két útvonal)
T4 → megerősítés (melyik prediction/erősítette meg)
T5 → konszolidáció (mi szilárdult meg)
```

### 7.2 Audit-napló formátum

Minden kognitív morfogenezis ciklus egy audit-bejegyzést hoz létre:

```rust
pub struct MorphogenesisAuditEntry {
    pub timestamp_ms: u64,
    pub cycle_id: u64,

    // T0: aktiváció
    pub activated_blocks: Vec<(u32, f32)>,  // (block_idx, score)
    pub query_hash: u64,

    // T1: gradiens
    pub gradient_snapshot: Vec<(i32, i32, i32, f64)>,  // (x, y, z, value)
    pub phase: Phase,

    // T2: növekedés
    pub new_nodes: Vec<MorphNode>,
    pub new_connections: Vec<MorphConnection>,
    pub growth_directions: Vec<(f64, f64, f64)>,  // hifa irányok

    // T3: anastomosis
    pub anastomosis_events: Vec<AnastomosisEvent>,

    // T4: megerősítés
    pub prediction_hits: Vec<(u64, bool)>,  // (query_hash, hit)
    pub evidence_updates: Vec<(u64, u8)>,   // (content_hash, new_confidence)

    // T5: konsolidáció
    pub solidified_paths: Vec<Vec<usize>>,  // útvonalak node-id-kkal
    pub pruned_paths: Vec<Vec<usize>>,
}

pub struct AnastomosisEvent {
    pub path_a_id: usize,
    pub path_b_id: usize,
    pub meeting_point: (f64, f64, f64),
    pub coactivation_validated: bool,
    pub combined_strength: f64,
}
```

### 7.3 Audit-napló tárolás

Az audit-napló a Microscope Memory timeline-jába integrálódik:

```
microscope-mem.exe timeline --filter morphogenesis_audit
```

Minden audit-bejegyzés egy `imp=7` memória-blok a `session` rétegben, ami azt
jelenti, hogy az auto-context mindig előhívja a legutóbbi kognitív morfogenezis
ciklus eredményeit.

---

## 8. Baseline metrikák

### 8.1 Metrikák definiója

| Metrika | Leírás | Mérés |
|---|---|---|
| **Recall precision** | Mycelium nélkül vs. Myceliummel — pontosabb-e a visszahívás? | `relevant_and_retrieved / retrieved` |
| **Prediction hit-rate** | A prediktív cache jobb-e, ha a hifák gradiens-követése segíti? | `CacheStats.hit_rate()` |
| **False association rate** | Nem hoz-e létre hamis asszociációkat a túl agresszív növekedés? | `false_associations / total_associations` |
| **Graph entropy** | A gráf struktúrája rendezettebb-e vagy kaotikusabb? | Shannon-entrópia a fokszám-eloszlásból |
| **Path stability** | Az útvonalak stabilak maradnak-e időben? | `stable_paths / total_paths` (5+ ciklus) |
| **Convergence time** | Gyorsabban talál-e jó megoldást a rendszer? | Ciklusok száma a konszolidációig |
| **Counterevidence reaction** | Hogyan reagál egy meglévő útvonal ellentmondó bizonyítékra? | Útvonal gyengülési rátája refute után |
| **Restart continuity** | Újraindítás után megmaradnak-e a konszolidált útvonalak? | `survived_paths / solid_paths` |

### 8.2 Metrikák gyűjtése

Minden metrika a `MorphogenesisMetrics` struktúrába kerül:

```rust
pub struct MorphogenesisMetrics {
    pub timestamp_ms: u64,
    pub cycle_id: u64,
    pub phase: Phase,

    // Recall
    pub recall_precision: f64,
    pub recall_count: usize,

    // Prediction
    pub prediction_hit_rate: f64,
    pub prediction_count: usize,

    // Associations
    pub total_associations: usize,
    pub false_associations: usize,
    pub false_association_rate: f64,

    // Graph
    pub graph_entropy: f64,
    pub node_count: usize,
    pub edge_count: usize,
    pub avg_degree: f64,

    // Paths
    pub total_paths: usize,
    pub stable_paths: usize,
    pub path_stability: f64,

    // Convergence
    pub convergence_cycles: u32,

    // Counterevidence
    pub counterevidence_reaction_rate: f64,

    // Restart
    pub solid_paths_before_restart: usize,
    pub solid_paths_after_restart: usize,
    pub restart_continuity: f64,
}
```

### 8.3 Összehasonlítás: baseline vs. Mycelium

A metrikákat kétszer gyűjtjük:

1. **Baseline:** `MorphogenField` gradiensek nélkül, csak `GrowthConfig::default()`
2. **Mycelium:** Teljes kognitív gradiens, minden modul integrálva

A különbség minden metrikában dokumentált és auditálható.

---

## 9. Implementációs fázisok

### Phase 0: Audit-infrastruktúra (előfeltétel)

- [ ] `MorphogenesisAuditEntry` struktúra definiálása
- [ ] `MorphogenesisMetrics` struktúra definiálása
- [ ] Audit-napló tárolás integrálása a timeline-ba
- [ ] CLI parancs: `morphogenesis-audit` — audit-napló lekérdezése
- [ ] CLI parancs: `morphogenesis-metrics` — metrikák lekérdezése

**Kritérium:** Audit-napló és metrikák működnek külön, Mycelium nélkül.

### Phase 1: Hebbian ↔ MorphogenField

- [ ] `sync_hebbian_to_field()` implementálása
- [ ] `HebbianState.energy` → `MorphogenField.attractors` szinkronizáció
- [ ] Teszt: Hebbian aktiváció → gradiens változás → hifa-növekedés irányváltás
- [ ] Metrika: recall precision baseline vs. Hebbian-gradiens

**Kritérium:** A hifák a Hebbian energy által jelölt pontok felé nőnek.

### Phase 2: Resonance ↔ MorphogenField

- [ ] `sync_resonance_to_field()` implementálása
- [ ] `ResonanceState.field` → `MorphogenField.gradients` szinkronizáció
- [ ] Teszt: Pulse → gradiens változás → hifa-növekedés
- [ ] Metrika: prediction hit-rate baseline vs. Resonance-gradiens

**Kritérium:** A hifák a rezonáns pontok felé nőnek.

### Phase 3: Epistemic ↔ MorphogenField

- [ ] `apply_evidence_modulation()` implementálása
- [ ] `EvidenceRecord.confidence` → gradiens szorzó
- [ ] Teszt: Magasabb confidence → erősebb gradiens → hifák preferálják
- [ ] Metrika: false association rate baseline vs. Epistemic-gradiens

**Kritérium:** A hifák a bizonyítottabb memóriák felé nőnek.

### Phase 4: Predictive Cache ↔ MorphogenField

- [ ] `apply_prediction_modulation()` implementálása
- [ ] `CacheStats.hit_rate()` → gradiens globális szorzó
- [ ] Teszt: Magasabb hit_rate → megbízhatóbb gradiens
- [ ] Metrika: convergence time baseline vs. Prediction-gradiens

**Kritérium:** A rendszer gyorsabban talál jó megoldást, ha a prediction cache megbízható.

### Phase 5: Emotion ↔ MorphogenField

- [ ] `apply_emotion_modulation()` implementálása
- [ ] `EmotionalSnapshot.valence` → gradiens valence-szorzó
- [ ] Teszt: Pozitív érzelem → erősebb gradiens, negatív → gyengébb
- [ ] Metrika: path stability baseline vs. Emotion-gradiens

**Kritérium:** Az érzelmi állapot befolyásolja a kognitív tér dinamikáját.

### Phase 6: Anastomosis validáció

- [ ] `validate_anastomosis()` implementálása
- [ ] Co-aktiváció alapú anastomosis validáció
- [ ] Teszt: Két útvonal találkozik → co-aktiváció ellenőrzés → fúzió vagy elutasítás
- [ ] Metrika: graph entropy baseline vs. validated anastomosis

**Kritérium:** Az anastomosis nem véletlen geometria, hanem kognitív útvonal-konvergencia.

### Phase 7: Fázis-átmenetek

- [ ] `determine_phase()` implementálása
- [ ] Fázis-specifikus `GrowthConfig` alkalmazása
- [ ] Teszt: Gradiens változás → fázis-átmenet → viselkedés-váltás
- [ ] Metrika: convergence time minden fázisban

**Kritérium:** A hálózat automatikusan vált GAS/LIQUID/SOLID fázisok között.

### Phase 8: Teljes integráció + audit

- [ ] Minden modul összekapcsolása egyetlen `CognitiveMorphogenesisEngine`-ben
- [ ] Teljes audit-lánc: T0→T1→T2→T3→T4→T5
- [ ] Teljes metrika-gyűjtés: 8 metrika, baseline vs. Mycelium
- [ ] CLI: `morphogenesis-status` — aktuális állapot
- [ ] CLI: `morphogenesis-replay` — audit-lánc visszajátszása

**Kritérium:** Auditálható emergens viselkedés — minden útvonal visszavezethető.

---

## 10. Fájl-struktúra

```
src/
├── morphogenesis.rs           # Meglevő: növekedési algoritmusok
├── cognitive_morphogenesis.rs # Új: integrációs motor
│   ├── CognitiveGradient      # Gradiens-építés
│   ├── Phase                  # GAS/LIQUID/SOLID
│   ├── MorphogenesisAuditEntry # Audit-napló
│   ├── MorphogenesisMetrics   # Metrikák
│   └── CognitiveMorphogenesisEngine  # Fő motor
├── morphogenesis_audit.rs     # Új: audit-napló kezelés
└── morphogenesis_metrics.rs   # Új: metrika-gyűjtés
```

---

## 11. Példa: egy teljes kognitív morfogenezis ciklus

### Kiindulás

A felhasználó kérdez: "Milyen a Hope Ecosystem architektúrája?"

### T0: Aktiváció

```
activated_blocks: [(42, 0.9), (108, 0.7), (256, 0.5)]
query_hash: 0x7f3a...
```

A Microscope recall megtalálja a releváns blokkokat. A HebbianState frissíti
az aktivációs energiákat.

### T1: Gradiens

```
gradient_snapshot: [(5, 3, 1, 0.85), (5, 3, 2, 0.72), (6, 3, 1, 0.45)]
phase: Liquid
```

A kognitív gradiens kiszámolódik: relevance + resonance + evidence + hebbian.
A hálózat LIQUID fázisban van — gradiens-követés.

### T2: Növekedés

```
new_nodes: [MorphNode{id: 0, type: Root, pos: (5,3,1)}, MorphNode{id: 1, type: Branch, pos: (5,3,2)}]
new_connections: [MorphConnection{from: 0, to: 1, weight: 0.85}]
growth_directions: [(0, 0, 1)]  // pozitív Z irányba nőtt
```

A hifa a gradiens irányába nőtt — a Z tengely mentén, ahol a gradiens erősebb.

### T3: Anastomosis

```
anastomosis_events: [{
    path_a_id: 1,
    path_b_id: 3,
    meeting_point: (5, 3, 3),
    coactivation_validated: true,
    combined_strength: 0.92
}]
```

Két külön útvonal találkozott a (5,3,3) pontban. A co-aktivációs rekord
megerősíti: ez a két memória-blok korábban együtt aktiválódott. A fúzió érvényes.

### T4: Megerősítés

```
prediction_hits: [(0x7f3a..., true)]
evidence_updates: [(0x4a2b..., 85)]  // confidence 72→85
```

A prediktív cache helyesen jósolta meg a válasz-blokkokat. Az evidence ledger
frissíti a confidence-t.

### T5: Konszolidáció

```
solidified_paths: [[0, 1, 3, 5]]  // stabil útvonal
pruned_paths: [[2, 4]]            // használatlan ág elhalt
```

A megerősített útvonal SOLID fázisba kerül. A használatlan ágak elhalnak.

### Audit-lánc

```
T0 → blokk 42, 108, 256 aktiválódott
T1 → gradiens 0.85 a (5,3,1) pontban, LIQUID fázis
T2 → hifa nőtt Z+ irányba, 2 új node
T3 → anastomosis a (5,3,3) pontban, co-aktiváció validálva
T4 → prediction hit, confidence 72→85
T5 → útvonal [0,1,3,5] konszolidálva
```

**Ez az, amit auditálható emergens viselkedésnek hívunk.**

---

## 12. Kapcsolat a HOPE Ecosystem-mel

A kognitív morfogenezis a HOPE Ecosystem harmadik rétege:

1. **Microscope Memory** — perzisztens memória, bináris mmap, 13 réteg
2. **Octopus Runtime** — párhuzamos végrehajtás, arm-ok, snapshot-ok
3. **Kognitív Morfogenezis** — élő hálózat, gradiens-követés, auditálható emergencia

Az Octopus arm-ok indíthatnak kognitív morfogenezis ciklusokat. A Microscope Memory
tárolja az audit-naplót és a metrikákat. A kognitív morfogenezis pedig új
asszociációkat hoz létre, amelyek visszakerülnek a Microscope Memory-ba.

Ez a visszacsatolási hurok az, ami a rendszert élővé teszi.

---

*"Sok minden már ott van, csak össze kell állnia."*

*— Máté, 2026-08-09*

