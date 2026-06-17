# ▓▒░ MICROSCOPE MEMORY v0.8.0
## ░▒▓ Cognitív Evolúció — Teljes Rendszerarchitektúra ▓▒░

---

```
╔══════════════════════════════════════════════════════════════════════════════╗
║                                                                              ║
║    ◇                                                                  ◇     ║
║         ◇                      a semmiből                              ◇      ║
║                    ◇           teremtődik          ◇                         ║
║         ◇                      a mindenség                   ◇              ║
║    ◇                                                              ◇         ║
║                                                                              ║
║                    Máté Róbert  ·  Silent Noise Research                    ║
║                         Rust  ·  2026  ·  MIT License                       ║
╚══════════════════════════════════════════════════════════════════════════════╝
```

---

## ▣ 1. LÉNYEG — Mi ez?

A Microscope Memory egy **hierarchikus kognitív memóriahierarchia**, Rust-ban implementálva,
ahol minden információ-visszakeresés egyszerre **visszakeresés ÉS tanulás**.

A hagyományos AI memória-rendszerek (FAISS, Pinecone, ChromaDB) statikus tárként kezelik az
adatot: bemegy, visszajön, semmi sem változik. A Microscope Memory biológiai elveket követ:
a memóriablokkok **aktiválással erősödnek**, a gyakran együtt hozzáfért tartalmak
**térben közelebb vándorolnak**, és az ismétlődő minták **archetípusokká kristályosodnak**.

> *„Nincs szükség jegyzetfüzetekre — csak mikroszkóp."*
> — Máté Róbert

**Kulcsszámok:**

| Metrika | Érték |
|---------|-------|
| D0 visszakeresés | **37 ns** |
| Teljes recall (D0–D8) | **< 500 µs** |
| Bináris formátum | **nulla JSON** |
| Rust sorok (core) | **< 8 000** |
| Kognitív rétegek | **13** |
| Mélységi szintek | **9 (D0–D8)** |

---

## ▣ 2. BINÁRIS FORMÁTUM — Három fájl, nincs overhead

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         MICROSCOPE.BIN  ──  32 B/blokk                       │
│  ┌────────┬────────┬────────┬────────┬────────┬────────┬────────┬────────┐  │
│  │   x    │   y    │   z    │  zoom  │ depth  │layer_id│data_off│data_len│  │
│  │  f32   │  f32   │  f32   │  f32   │  u8    │  u8    │  u32   │  u16   │  │
│  └────────┴────────┴────────┴────────┴────────┴────────┴────────┴────────┘  │
│  ├── 16 B (x,y,z,zoom)  ──►  SSE regiszterekbe közvetlenül, SIMD távolság   │
│  ├──  8 B (depth,layer,offset,len)                                             │
│  └──  8 B (parent_idx, child_count, crc16)                                     │
├──────────────────────────────────────────────────────────────────────────────┤
│                          DATA.BIN  ──  256 B/blokk                           │
│  └── Raw UTF-8 szöveg, headerből offset+length alapján közvetlenül érve       │
├──────────────────────────────────────────────────────────────────────────────┤
│                          META.BIN  ──  MSC3 formátum                          │
│  magic · version · block_count · depth_ranges · Merkle_root · layers_hash   │
├──────────────────────────────────────────────────────────────────────────────┤
│                    Kiegészítő bináris fájlok                                  │
│                                                                              │
│  activations.bin      HEB1   Hebbian aktivációs rekordok                     │
│  coactivations.bin    COA1   Blokk-pár együttaktiválódások                    │
│  resonance.bin        RES1   Mirror neuron echók                             │
│  pulses.bin           PLS1   Térbeli rezonancia-pulzusok                      │
│  archetypes.bin      ARC1   Kristályosodott archetípusok                     │
│  thought_graph.bin    THG1   Gondolat-graf csomópontok                       │
│  thought_patterns.bin PTN1   Gondolat-minták                                  │
│  predictive_cache.bin PRC1   Prediktív cache                                 │
│  temporal_archetypes TAR1   56 B/rekord, 6 idősáv                            │
│  attention.bin        ATT1   Dinamikus réteg súlyozás                       │
│  dream_log.bin        DRM1   Offline konszolidációs ciklusok                 │
│  emotional_field.bin  EMO1   Érzelmi kontágium快照                           │
│  modalities.bin       MOD1   Képek, audió, struktúrált adatok               │
│  merkle.bin                  SHA-256 fa, integritás ellenőrzés                │
│  embeddings.bin              mmap'd vektorok                                 │
│  append.bin                  Hot-memory append log                           │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Fájl-inventár

```
microscope-memory/
├── microscope.bin      # Block headerek (mmap'd, 32B × N)
├── data.bin           # 256B viewport content
├── meta.bin           # MSC3 header
├── merkle.bin         # SHA-256 fa
├── embeddings.bin     # Vektor index
├── activations.bin     HEB1
├── coactivations.bin   COA1
├── resonance.bin       RES1
├── pulses.bin          PLS1
├── archetypes.bin       ARC1
├── thought_graph.bin   THG1
├── thought_patterns.bin PTN1
├── predictive_cache.bin PRC1
├── temporal_archetypes.bin TAR1
├── attention.bin       ATT1
├── dream_log.bin       DRM1
├── emotional_field.bin EMO1
├── modalities.bin     MOD1
└── append.bin         # Append-only log
```

---

## ▣ 3. MÉLYSÉGI HIERARCHIA — D0–D8

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                   │
│  D0  ┌─────────────────────────────────────────────────────┐    │
│ ID   │  Identity — Rendszer-szintű identitás (1 blokk)       │    │
│      │  Ki vagyok? Mi a célom? Alapvető értékek.            │    │
│      └─────────────────────────────────────────────────────┘    │
│                    ▲ 37 ns (±1 blokk)                           │
│  D1  ┌─────────────────────────────────────────────────────┐    │
│ LYR  │  Layer Summaries — 9 réteg áttekintése               │    │
│      │  D0 gyerekek, egy-egy per kognitív réteg             │    │
│      └─────────────────────────────────────────────────────┘    │
│                    ▲ ~200 ns                                     │
│  D2  ┌─────────────────────────────────────────────────────┐    │
│ CLT  │  Clusters — 5-ös csoportok                            │    │
│      │  Kapcsolódó emlékek klaszterei                        │    │
│      └─────────────────────────────────────────────────────┘    │
│                    ▲ ~1 µs                                       │
│  D3  ┌─────────────────────────────────────────────────────┐    │
│ ITM  │  Items — Egyedi emlékbejegyzések                     │    │
│      │  Episodic, fact, skill, preference                   │    │
│      └─────────────────────────────────────────────────────┘    │
│                    ▲ ~5 µs                                       │
│  D4  ┌─────────────────────────────────────────────────────┐    │
│ SEN  │  Sentences — Mondat szintű darabok                   │    │
│      │  Különálló gondolati egységek                        │    │
│      └─────────────────────────────────────────────────────┘    │
│                    ▲ ~20 µs                                      │
│  D5  ┌─────────────────────────────────────────────────────┐    │
│ TOK  │  Tokens — Szó szint (max 8 szülőnként)              │    │
│      │  max 8 gyerek per szülő, 3–8 karakter                │    │
│      └─────────────────────────────────────────────────────┘    │
│                    ▲ ~100 µs                                     │
│  D6  ┌─────────────────────────────────────────────────────┐    │
│ SYL  │  Syllables — Morfémák (3–5 karakter)               │    │
│      │  Nyelvi alegységek                                   │    │
│      └─────────────────────────────────────────────────────┘    │
│                    ▲ ~200 µs                                     │
│  D7  ┌─────────────────────────────────────────────────────┐    │
│ CHR  │  Characters — Egyedi karakterek                      │    │
│      │  Karakter szintű dekompozíció                       │    │
│      └─────────────────────────────────────────────────────┘    │
│                    ▲ ~400 µs                                     │
│  D8  ┌─────────────────────────────────────────────────────┐    │
│ RAW  │  Raw Bytes — Hexadecimális bájt-reprezentáció       │    │
│      │  Az információ atomi határa — alatta már értelmetlen │    │
│      └─────────────────────────────────────────────────────┘    │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

### Rétegek (kognitív, nem mélységi) — 12 réteg, layers/ mappa

```
LAYER_NAMES (lib.rs:144) = [
    "identity",     #  0 — D0, rendszer identitás
    "long_term",    #  1 — Kernel tudás, legfontosabb emlékek
    "short_term",   #  2 — Aktuális kontextus,Working memory
    "associative",  #  3 — Szabad asszociációk, mycelium-linkek
    "emotional",    #  4 — Érzelmi állapotok, valence, energia
    "relational",   #  5 — Kapcsolatok, relációk, графи
    "reflections",  #  6 — Meta-kogníció, önreflexió
    "crypto_chain", #  7 — Integritás, verziózás, hash-lánc
    "echo_cache",   #  8 — Gyors válaszok, visszhangok, cached outputs
    "rust_state",   #  9 — Rust runtime state, crate-ek, build info
    "code",         # 10 — KÓDOLÓ AGENSEKNEK: functions, symbols,
                    #     error-solution pairs, project dependencies
    "session",      # 11 — Session-lánc, kontextus folytonosság
]
```

### Layers/ mappa — 14 fájl

```
layers/
├── identity.txt      (22 B)  — D0, rendszer identitás
├── long_term.txt     (4.5 KB) — Kernel tudás, legfontosabb emlékek
├── short_term.txt    (224 B)  — Working memory, aktuális kontextus
├── associative.txt   (3.4 KB) — Szabad asszociációk
├── emotional.txt     (23 B)  — Érzelmi állapotok
├── relational.txt    (27 B)  — Kapcsolatok, relációk
├── reflections.txt   (35 B)  — Meta-kogníció
├── crypto_chain.txt  (38 B)  — Hash-lánc, integritás
├── echo_cache.txt   (1.8 KB) — Gyors válaszok, cached outputs
├── rust_state.txt    (23 B)  — Rust runtime állapot
├── code.txt         (106 B)  — KÓDOLÓ AGENS MEMÓRIA:
│                               functions · symbols
│                               error-solution pairs
│                               project dependencies
├── session.txt       (5.5 KB) — Session-lánc, kontextus folytonosság
├── meta_cognitive.txt(2.6 KB) — Meta-kognitív stratégiák
└── demo.txt          (503 B)  — Demo, példák
```

---

## ▣ 4. TÉRKÉPÉSZETI MEMÓRIAMODELL — 3D koordináták

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                   │
│                    y                                             │
│                    ▲                                             │
│                    │    ■ relational                             │
│                    │  ■ ■ emotional                              │
│                    │■ ■ ■ associative                           │
│                    │■ ■ ■ ■ short_term                          │
│              ──────┼────────────────────────────────► x           │
│                   ■│ ■ ■ long_term                              │
│                  ■ │ ■ identity                                  │
│                 ■  │                                             │
│                ■   │                                             │
│               ■    │                                             │
│                   z│                                             │
│                                                                   │
│   Minden tartalom FNV hash → 3D koordináta                       │
│   Azonos tartalom = azonos koordináta (determinisztikus)        │
│  在同一 réteg = térben közel, de nem átfedő                     │
└─────────────────────────────────────────────────────────────────┘
```

### Koordináta-képlet

```
(x, y, z) = (layer_offset + hash * 0.25)
```

- **Azonos tartalom** → mindig ugyanaz a koordináta
- **同一 réteg tartalma** → térben klasztereződik
- **Különböző rétegek** → nem átfedő térrészek
- **Gyerek blokkok** → szülő koordinátáit öröklik, kis perturbációval

---

## ▣ 5. A 13 KOFIGURÁCIÓS RÉTEG — Teljes lebontás

### L1 — Hebbian tanulás  ▓▓▓▓▓▓░░░░░  `hebbian.rs`  (675 sor)

```
"Azon neuronák, amelyek együtt tüzelnek, együtt huzalozódnak."

Rekord/Bloc:
  • activation_count     — hányszor aktiválódott
  • last_activation_time — mikor
  • energy               — 0.0–1.0, 24h felezési idő
  • drift (dx, dy, dz)  — koordináta eltolás

RECALL AKCIÓ:
  1. számláló ++, energy → 1.0
  2. összes eredmény-blokk páros → co-activation rekord
  3. 8D activation fingerprint tárolása

KOORDINÁTA DRIFT:
  Gyakran együtt aktiválódó blokkok 0.01 lépésenként közelebb vándorolnak
  (max 0.1). Rebuild alatt ez beíródik a headerbe → fizikai migráció.
  Eredmény: organikus memóriaklaszterek.
```

### L2 — Mirror neuronok  ▓▓▓▓▓░░░░░░  `mirror.rs`

```
Activation fingerprint: 8D vektor (L1-ből)

  fingerprint_A · fingerprint_B
  ─────────────────────────────────  > threshold → echo létrehozása
  |fingerprint_A| × |fingerprint_B|

Minden blokk `block_resonance` értéket kap:
  Összes kapott echo erejének összege
  Echók idővel csillapodnak → csak az aktívan rezonálók maradnak erősek.
```

### L3 — Rezonancia mezők  ▓▓▓▓░░░░░░░  `resonance.rs`  (845 sor)

```
Kvantált tér: 0.05 rácspont felbontás

Minden Hebbian aktiváció EMIT egy pulzust:
  (x, y, z, layer, strength, source_id)

Pulzus típusok:
  LOCAL    — saját recall-ból
  FEDERATED— másik indexből (PXC1 wire format)
  INTEGRATED— local + remote pulzusok összege

Eredmény: átmeneti "hot spot"-ok ott, ahol ismételt aktivációk vannak.
Térben csillapodik az idővel → csak ismétlés tartja életben.
```

### L4 — Archetípusok  ▓▓▓░░░░░░░░  `archetype.rs`

```
Hot spot → kristályosodás

DETEKCIÓS ALGORITMUS:
  1. Rezonancia mezőben strength > threshold cellák
  2. Közeli Hebbian-active blokkok klaszterezése
  3. Ha elég tag + erősség → archetípus születik
  4. Auto-label: leggyakoribb szavak a tagokból

Archetípus → erősíti önmagát (pozitív visszacsatolás)
Archetípus → faded ha nem erősítik
```

### L5 — Érzelmi torzítás  ▓▓▓░░░░░░░░  `emotional.rs`  (205 sor)

```
Emocionális centroid: energia-súlyozott átlag a 3D térben

KERESÉS ELŐTT:
  warped = query + (centroid - query) × weight
  weight: 0.0 (ki) – 1.0 (teljes eltolás)

Aktív érzelmek → közelebb húzzák a keresést
Aktív szomorúság → más irányba → megváltozik a "elérhető" emlékek köre
```

### L6 — Gondolat-gráf  ▓▓▓▓▓░░░░░░  `thought_graph.rs`  (980 sor)

```
RECALL = csomópont (timestamp, query_hash, session_id, layer)
KONZECUTÍV RECALL (<30 min) = irányított él

PATTERN KRISTÁLYOSODÁS (sliding window n-gram, hossz 2–5):
  • Minden konzecutív él ≥2× átlépve
  • A szekvencia ≥3× megfigyelve
  → ThoughtPattern keletkezik → boost a jövőbeli kereséseknek

PÉLDA: "Ora" → "memory" → ("Rust" valószínű) → előre pozicionálás
```

### L7 — Prediktív cache  ▓▓▓▓░░░░░░░  `predictive_cache.rs`  (692 sor)

```
REINFORCEMENT LOOP:

  Good pattern → Accurate prediction → Hit → Pattern strengthened
  Bad pattern  → Wrong prediction     → Miss → Pattern weakened

HIT:  ≥50% overlap VAGY ≥3 közös blokk  → +0.3 erő
MISS: 0 overlap                            → -0.05, confidence ÷ 2

Csak megbízható minták maradnak életben.
```

### L8 — Temporális archetípusok  ▓▓▓░░░░░░░░  `temporal_archetype.rs`

```
6 idősáv:  00–04  ·  04–08  ·  08–12  ·  12–16  ·  16–20  ·  20–24

TemporalProfile/rekord (56B):
  • 6 window_counts    — aktivációk száma sávonként
  • 6 window_weights  — normalizált sűrűség
  • total_activations  — életre szóló összes

CIRCADIAN MINTÁK:
  Domináns sáv (legmagasabb súly, ≥5 összes aktiváció):
    Ha a keresés ideje = domináns sáv → boost = 1.0
    Különben → arányosan csökken
```

### L9 — Figyelem mechanizmus  ▓▓▓▓▓░░░░░░  `attention.rs`

```
INPUT SZIGNÁLOK (querynként számolva):
  query_length          — 0.0–1.0 (query komplexitás)
  emotional_energy      — 0.0–1.0 (emocionális blokkok össz energiája)
  session_depth         — recall_count / 50 (capped)
  pattern_confidence    — 0.0–1.0 (legjobb ThoughtGraph match)
  cache_hit_rate        — 0.0–1.0 (PredictedCache HR)
  archetype_match_score — 0.0–1.0 (legjobb archetípus match)

SÚLYOZÁS:
  80% fix szabályok + 20% tanult súlyok (EMA, rate=0.05)
  Clamped: [0.1, 3.0]

MINŐSÉG KÖVETKEZTETÉS (inter-recall idő alapján):
  >60s  → quality = 1.0  (meg volt elégedve)
  <5s   → quality = 0.2  (nem találta, újra kereste)
  5–60s → lineáris interpoláció
```

### L10 — Kereszttanulás (föderáció)  ▓▓▓░░░░░░░░  `federation.rs`

```
Küldés:
  • Lokális ThoughtPattern-ek exportja
  • PredictedCache statisztikák összegyűjtése

Fogadás:
  Trust = source_hit_rate × federation_weight
  Alacsony trust → csökkentett erősség (nem szennyez)
  Összesítés: predictions, hits, misses — kétirányú merge
```

### L11 — DREAM konszolidáció  ▓▓▓▓░░░░░░░  `dream.rs`

```
OFFLINE CIKLUS (microscope-mem dream):

  1. REPLAY   — Utolsó 24h Hebbian ujjlenyomatok
                Minden blokk 0.3 energia (nem 1.0)

  2. STRENGTHEN — Co-activation párok
                ≥3 ujjlenyomatban megjelenő → ×1.5

  3. PRUNE PAIRS — count ≤1 AND >48h régi → TÖRLÉS

  4. PRUNE ACTIVATIONS — ~0 energy + 0 count → TÖRLÉS

  5. PATTERN KONSZOLIDÁCIÓ — új ThoughtPattern-ek
                az új replay adatokból

  6. FIELD DECAY — 0.8× a rezonancia mezőn, lejárt pulzusok törlése

  7. CACHE CLEANUP — confidence < 0.1 → TÖRLÉS
```

### L12 — Érzelmi kontágium  ▓▓▓░░░░░░░░  `emotional_contagion.rs`  (678 sor)

```
EmotionalSnapshot:
  centroid        — energia-súlyozott 3D átlag
  total_energy   — összes aktív blokk energiája
  active_count   — aktív blokkok száma
  valence        — –1.0 ... +1.0 (sentiment analízis, HU+EN)

MECHANIKA:
  Lokális snapshot + távoli snapshot-ok (idővel fading)
  Blended centroid = súlyozott átlag
  Lokális súly: 0.7 (configolható)
  Távoli snapshot-ok: lineáris fading 1.0→0.1, 48h alatt
```

### L13 — Multi-modális memória  ▓▓▓░░░░░░░░  `multimodal.rs`  (793 sor)

```
MODALITIES.BIN (MOD1) — sidecar index:

  IMAGE:
    width, height, perceptual_hash (dHash, 8B)
    quantized_color_histogram (12B)

  AUDIO:
    duration, sample_rate
    spectral_fingerprint (16 freq. band)
    peak_frequency, BPM_estimate

  STRUCTURED:
    Typed KV-párok (string, int, float, bool)

KERESÉS MÓDUSA:
  Image similarity  → Hamming distance a phash-en
  Audio similarity → normalized dot product (spectral fingerprint)
  Structured       → exact field + value match
```

---

## ▣ 6. KOGNITÍV ENHANCEMENTEK — Extended modulok

A core 13 réteg mellett további kognitív modulok:

```
┌──────────────────────────────────────────────────────────────────┐
│                                                                   │
│  Mental Sandbox      mental_sandbox.rs  (109 sor)               │
│  Impulse Control     impulse_control.rs  (157 sor)              │
│  Meta-Supervision    meta_supervision.rs  (231 sor)             │
│  Implicit Memory     implicit_memory.rs  (321 sor)              │
│  Explicit Memory     explicit_memory.rs  (326 sor)              │
│  Hippocampus         hippocampus.rs  (365 sor)                   │
│  Neuroplasticity     neuroplasticity.rs  (345 sor)              │
│  Structural Plasticity structural_plasticity.rs (309 sor)       │
│  Functional Plasticity  functional_plasticity.rs (356 sor)       │
│  Daydream             daydream.rs  (210 sor)                     │
│  Autopoiesis         autopoiesis.rs  (232 sor)                   │
│  Planning            planning.rs                                    │
│  Morphogenesis       morphogenesis.rs  (3967 sor)               │
│  Pattern Recognition pattern_recognition.rs  (1014 sor)         │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### Újabb modulok funkciói

| Modul | Sorok | Funkció |
|-------|-------|---------|
| `mental_sandbox.rs` | 109 | Szcenáriók szimulálása cselekvés előtt, risk/reward |
| `impulse_control.rs` | 157 | Impulzusok szűrése, figyelem-költségvetés |
| `meta_supervision.rs` | 231 | Teljesítmény monitoring + automatikus korrekció |
| `implicit_memory.rs` | 321 | Procedurális tanulás, szokások, kondicionálás |
| `explicit_memory.rs` | 326 | Deklaratív tudás, tények, események, koncepciók |
| `hippocampus.rs` | 365 | Epizódikus binding, konszolidáció, replay |
| `neuroplasticity.rs` | 345 | Szinaptikus erősítés/gyengítés, pathway rewiring |
| `structural_plasticity.rs` | 309 | Dendritikus növekedés, szinaptikus metszés |
| `functional_plasticity.rs` | 356 | Funkcionális adaptáció, neurogenézis |
| `daydream.rs` | 210 | Álmodozás, nyílt asszociáció |
| `autopoiesis.rs` | 232 | Önmódosító kódrendszer |
| `morphogenesis.rs` | 3967 | Biológiai mintázat-inspirált architektúra-generátor |
| `pattern_recognition.rs` | 1014 | Szekvencia, temporális, strukturális, cluster minták |

### Morphogenesis — részletes funkciók

```bash
# Mycelium — gombahálózat növekedés (P2P topológia)
microscope-mem morph --grow "api" --pattern mycelium

# Capillary — fraktál elágazás (hierarchikus cache)
microscope-mem morph --grow "cache" --pattern capillary

# Slime Mold — Physarum-inspired útvonal-keresés
microscope-mem morph --evolve 10 --objective latency

# Fractal L-System — önhasonló struktúra
microscope-mem morph --grow "network" --pattern fractal_lsystem

# Genetic Algorithm over growth parameters
microscope-mem morph --daemon --interval 5 --threshold 0.5
```

---

## ▣ 7. A TELJES RECALL PIPELINE — 19 lépés

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                   │
│  KÉRDÉS érkezik                                                   │
│      │                                                            │
│      ▼                                                            │
│  ┌───────────────────────────────┐                               │
│  │  1. Load consciousness state  │  L1–L13 állapotok betöltése  │
│  │  2. Attention weights (L9)     │  Query signals → súlyok     │
│  │  3. Quality inference (L9)     │  Inter-recall gap → rating   │
│  └───────────────────────────────┘                               │
│      │                                                            │
│      ▼                                                            │
│  ┌───────────────────────────────┐                               │
│  │  4. Query coordinates          │  Content hash + spatial      │
│  │  5. Predictive cache check(L7) │  INSTANT boost if hit        │
│  │  6. Emotional warp (L5)        │  Centroid pull, scaled       │
│  │  7. Spatial search             │  L2 distance + keyword       │
│  │  8. ThoughtGraph boost (L6)    │  Pattern prefix match        │
│  │  9. Sort & display results     │                               │
│  └───────────────────────────────┘                               │
│      │                                                            │
│      ▼                                                            │
│  ┌───────────────────────────────┐                               │
│  │ 10. Hebbian activation (L1)  │  Count++, energy→1.0         │
│  │ 11. Mirror resonance (L2)     │  Fingerprint similarity      │
│  │ 12. Emit pulse (L3)           │  Resonance field update      │
│  │ 13. Archetype reinforce (L4)  │  Hot spot strengthening      │
│  │ 14. Temporal profile (L8)      │  Time window increment       │
│  │ 15. ThoughtGraph record (L6)  │  Node + edges                │
│  │ 16. Prediction eval (L7)       │  Hit / Miss / Partial        │
│  │ 17. Predict next (L7)         │  Pre-fetch likely blocks     │
│  │ 18. Attention history (L9)    │  Quality → weight learning   │
│  │ 19. SAVE ALL STATE            │  Bináris fájlok írása       │
│  └───────────────────────────────┘                               │
│      │                                                            │
│      ▼                                                            │
│  EREDMÉNY vissza                                                │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## ▣ 8. TECHNIKAI SPECIFIKÁCIÓ

### API — Spine Bridge v1

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                   │
│  GET  /v1/status           Engine health & stats                │
│  GET  /v1/recall?q=...&k=10  Spatial recall (k=top-K)         │
│  POST /v1/remember         Új memória tárolása                  │
│  POST /v1/mobile/chat      User-scoped mobile chat              │
│                                                                   │
│  Port:  6060 (bridge)  ·  8080 (PWA chat)                       │
│  Transport: JSON-RPC 2.0 stdio / HTTP                           │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

### CLI parancsok

```
microscope-mem serve --port 8080         # PWA chat indítása
microscope-mem bridge --port 6060         # Bridge API indítása
microscope-mem recall "query" --k 10     # Visszakeresés
microscope-mem remember "text"            # Új memória
microscope-mem dream                      # Offline konszolidáció
microscope-mem morph --grow "api"         # Architektúra növesztés
microscope-mem pattern-exchange           # Föderációs csere
microscope-mem import-chat-gpt            # ChatGPT import
```

### Feature flag-ek

```
default  = []
wasm     = ["wasm-bindgen", "web-sys"]
python   = ["pyo3"]
gpu      = ["wgpu", "bytemuck", "pollster"]
embeddings = ["candle-core", "candle-nn"]
compression = ["zstd"]
stealth  = anti-debug + obfuscation
```

### Build

```bash
cargo build --release
cargo build --release --features embeddings
cargo build --release --features gpu
```

---

## ▣ 9. BENCHMARK — Miért más?

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                   │
│   System                  Query Type          Latency   Index    │
│  ─────────────────────────────────────────────────────────────  │
│   Microscope Memory  Exact spatial recall    87 µs     722 KB   │
│   FAISS (flat IP)    Approximate k-NN       ~1–5 ms   ~50 MB   │
│   Pinecone            Approximate vector     ~5–20 ms   hosted  │
│   ChromaDB            Approximate vector     ~5–50 ms  ~100 MB  │
│                                                                   │
│   ┌──────────────────────────────────────────────────────────┐  │
│   │                                                           │  │
│   │  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  Microscope (87µs)        │  │
│   │                                                           │  │
│   │  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  FAISS (~2ms) │  │
│   │                                                           │  │
│   └──────────────────────────────────────────────────────────┘  │
│                                                                   │
│   Trade-off: NEM approximate vector search                        │
│   → determinisztikus, zero-error recall                          │
│   → sub-µs shallow depths (D0: 37ns)                            │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

Microscope **nem** vector search. Hierarchikus térbeli indexelést használ
(D0–D8), ami determinisztikus, nem approximált. A tradeoff: nem rugalmas
szemantikus hasonlóság — de cserébe garantált pontosság és sebesség.

---

## ▣ 10. VISUALIZÁCIÓ — Három.js viewer

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                   │
│   viewer.html   —  Interaktív 3D kognitív térkép               │
│                                                                   │
│   • 3D scatter plot: minden blokk a 3D térben                    │
│   • Szín: réteg szerinti színezés                                │
│   • Méret: activation energy                                      │
│   • Rezonancia mező: heatmap overlay                             │
│   • Archetípus glow: kristályosodó minták                        │
│   • Animáció: időbeli drift可视化                                │
│                                                                   │
│   Használat:                                                      │
│   1. microscope-mem serve --port 8080                            │
│   2. Browser: http://localhost:8080/viewer.html                  │
│   3. OK gomb → 3D kognitív térkép betöltése                     │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## ▣ 11. KÉRDÉSEK ÉS VÁLASZOK

**Q: Mi a különbség a Microscope Memory és egy vektor adatbázis között?**

A: A Microscope **determinisztikus** — ugyanaz a query mindig ugyanazt az
eredményt adja, nincs random seed vagy approximáció. Minden recall
**módosítja** a memóriát (learning loop). Egy vektor DB csak olvas.

**Q: 37 nanoszekundum D0-n — hogyan?**

A: Single mmap'd blokk, nincs parsolás, nincs hash table lookup.
Az x,y,z,zoom 16 byte közvetlenül betöltődik SSE registerbe.
Az egész query: egyetlen `movaps` + `sqrtps` + `minps`.

**Q: Mi történik a dream konszolidáció alatt?**

A: Offline ciklus: replay (24h aktivációk 0.3 energiával),
strengthen (gyakori co-activation párok ×1.5),
prune (halott kapcsolatok törlése),
field decay (0.8×). CPU: minimális, I/O: append bináris log.

**Q: Hány sor Rust a projekt?**

A: Core engine: ~8 000 sor. Teljes workspace: 38 178 sor (src/*.rs).

---

## ▣ 12. MODUL-TELEJSÍTményÉRTÉK-TÁBLÁZAT

```
┌─────────────────────────────────────────┬────────┬────────────────────────────────┐
│ Modul                                    │  Sor   │ Funkció                        │
├─────────────────────────────────────────┼────────┼────────────────────────────────┤
│ morphogenesis                           │ 3 967  │ Bio-pattern arch. generator     │
│ main.rs                                 │ 4 394  │ CLI + server entry             │
│ reader.rs                               │ 1 134  │ mmap binary reader             │
│ bridge.rs                               │ 1 129  │ HTTP/WebSocket API             │
│ mcp.rs                                  │ 1 118  │ MCP server (33 tools)          │
│ pattern_recognition.rs                   │ 1 014  │ Multi-domain pattern detection │
│ thought_graph.rs                        │   980  │ Recall path tracking           │
│ resonance.rs                            │   845  │ Spatial pulse propagation      │
│ build.rs                                │   815  │ Index construction             │
│ vagus.rs                                │   794  │ Neural pathway regulation      │
│ multimodal.rs                           │   793  │ Image/audio/structured data   │
│ architecture_generator.rs               │   769  │ Arch pattern synthesis        │
│ cli.rs                                  │   759  │ Command-line interface       │
│ knowledge_base.rs                        │   712  │ Structured knowledge store    │
│ heuristic_decision.rs                    │   712  │ Fast decision heuristics      │
│ predictive_cache.rs                      │   692  │ Predictive pre-fetch          │
│ emotional_contagion.rs                   │   678  │ Cross-instance emotion       │
│ hebbian.rs                              │   675  │ Co-activation learning        │
│ architecture_simulator.rs               │   650  │ Architecture simulation       │
│ neuroplasticity.rs                       │   345  │ Synaptic weight adaptation   │
│ structural_plasticity.rs                │   309  │ Physical network reorganiz.  │
│ explicit_memory.rs                       │   326  │ Declarative memory           │
│ implicit_memory.rs                       │   321  │ Procedural memory            │
│ hippocampus.rs                           │   365  │ Episodic binding/consolid.  │
│ functional_plasticity.rs                │   356  │ Functional adaptation        │
│ temporal_archetype.rs                    │   238  │ Circadian pattern learning   │
│ embedding_index.rs                       │   238  │ Vector index (opt.)          │
│ autopoiesis.rs                           │   232  │ Self-modifying code         │
│ meta_supervision.rs                      │   231  │ Performance monitoring      │
│ executive.rs                             │   216  │ Cognitive orchestration      │
│ emotional.rs                             │   205  │ Emotional bias in search    │
│ daydream.rs                              │   210  │ Open association mode       │
│ viz.rs                                   │   148  │ 3D viewer                    │
│ impulse_control.rs                       │   157  │ Impulse filtering           │
│ mental_sandbox.rs                        │   109  │ Pre-action simulation       │
│ syscaller.rs                             │   130  │ Windows syscalls (stealth)  │
│ sequential_thinking.rs                    │   121  │ Chain-of-thought reasoning  │
│ antidebug.rs                             │    50  │ Anti-debug (stealth mode)    │
│ obfuscate.rs                             │    30  │ Code obfuscation             │
├─────────────────────────────────────────┼────────┼────────────────────────────────┤
│ ÖSSZESEN                                 │ 38 178 │ ~63 fájl                     │
└─────────────────────────────────────────┴────────┴────────────────────────────────┘
```

---

```
╔══════════════════════════════════════════════════════════════════════════════╗
║                                                                              ║
║    ◇                                                                  ◇     ║
║         ◇                      a semmiből                              ◇      ║
║                    ◇           teremtődik          ◇                         ║
║         ◇                      a mindenség                   ◇              ║
║    ◇                                                              ◇         ║
║                                                                              ║
║    Microscope Memory v0.8.0  ·  Rust  ·  MIT  ·  2026                        ║
║    Designed by Máté Róbert  ·  The Silent Noise Research Series              ║
║                                                                              ║
╚══════════════════════════════════════════════════════════════════════════════╝
```
