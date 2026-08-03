# Evidence Layer (Epistemic Layer) - Implementation Plan v0.1

> A cél nem egy „okosabb memória”, hanem egy auditálható episztemikus réteg, amely a bizonyíték-súlyt (amire tényalap van) teljesen elválasztja a recall-salienciától („amit gyakran előhívok”). A bizonyíték-súly csak új, független megfigyeléssel nőhet — soha a visszahívással.

## 1. Probléma (verifikálva a kódban)

A `importance` byte ma két összeférhetetlen feladatot lát el:

1. Retenciós/evikciós prioritás. A `protect_min_importance` fölé került blokk soha nem evikciálódik (`evict_over_capacity`, `src/dream.rs:520`).
2. Salience jelzés. A `promote_recalled_blocks` (`src/dream.rs:664`) a Hebbian-energiából (`rec.energy` és `activation_count`) növeli az `importance`-t, és átírja a `(imp=N)`-markert (`bump_entry_by_text_hash`, `src/dream.rs:758`).

Ez az önerősítő hurok: a gyakran előhívott emlék a salience révén fontossá lesz, ezzel evikciós védelmet kap, miközben a bizonyíték (tény) soha nem számít. Egy következtetés variációkat újratárolni elég ahhoz, hogy magas `importance` + védelmet kapjon, akkor is, ha soha nem volt mögötte megfigyelés. Emiatt az LLM a kövi konklúziót „adatként” idézi, pedig hipotézis volt.

## 2. Alapelv: két független dimenzió

| Dimenzió | Meghajtó | Jelentés | Ami sosem |
|---|---|---|---|
| Salience (meglévő `importance`) | Hebbian, Resonance, Thought Graph, Dream | „mi jár a fej GSén” | nem = bizonyíték |
| Confidence (új) | független Observation/Evidence blokkok száma + forrásgyökök | „mire van tényalap” | nem származhat recall-ból |

A salience-dimenziót nem tiltjuk el: a Hebbian, Resonance, Thought Graph és a Dream változatlanul fut. Csak a `confidence`-et választjuk le a salience-től, és egyetlen új gate épül a `promote_recalled_blocks`-ba (lásd 6.).

## 3. Új modul: `src/epistemic.rs`

- a `store_*` család des kiegészítése: `store_memory_with_epistemic(...)` fogadja a class-t és a supports-listát;
- a `build` a rebuildkor a `BlockHeader.flags` byte osztály-bitjeit a ledgerből besztámploza (a byte ma létezik, `0`-ra van merevítve, lásd `build.rs:635`);
- `dream::promote_recalled_blocks` lesz az egyetlen új enforcement pont (gate);
- a `reader` recall-eredménye `confidence` és `class` mezővel kiegészül.

## 4. Új bináris állapotok

### 4.1 `evidence.bin` (magic `EVD1`) - indexelt ledger

Kulcs: `u64` content-hash = FNV-1a a marker-strippolt szövegen (azonos a rendszer meglévő mintájával). Ezért a kulcs a rebuild/reindex alatt determinisztikus marad. Rekord ~40 byte:

```
struct EvidenceRecord {
    content_hash: u64,      // kulcs
    class: u8,              // 1=Observation 2=Evidence 3=Inference 4=Hypothesis
    source_id: u64,         // forrásgyök (instancia / aktor)
    support_count: u32,     // független Evidence/Observation blokkok
    refute_count: u32,      // cáfolatok
    distinct_sources: u32,  // nem saját-echo forrásgyökök
    first_seen_ms: u64, last_support_ms: u64, last_refute_ms: u64,
    confidence: u8,         // 0..100 (build-kor előre kiszámolva)
    flags: u8,
}
```

### 4.2 `evidence_log.bin` (tag `EVL1`) - append-only, SHA-256 láncolt audit

Az `enforcement.rs` `AuditChunk` mintájára: `hash = sha256(prev_hash | record)`. Minden esemény (STORE, LINK, REFUTE, PROMO_GATE, RECLASS) egyetlen láncba, utólag ellenőrizhető és verifikálható.

```
struct EvidenceAuditChunk {
    prev_hash: [u8; 32], ts_ms: u64,
    event: u8, content_hash: u64, source_id: u64,
    status: u8, delta: i32, note_len: u16, note: [u8; 64],
    hash: [u8; 32],
}
```

## 5. A négy osztály elkülönítése

| Osztály | biting | Jelentése | Példa |
|---|---|---|---|
| Observation | 001 | nyers, közledeni észlelt/adat | „a gyór reggel 6-kor kezdődik” |
| Evidence | 010 | Observation, explicit `supports`-kötéssel | az előző, egy állazításhoz zavarva |
| Inference | 011 | megfigyelésekkel levezett konklúzió | „alter → ez = zor” |
| Hypothesis | 100 | sz kell ellenőrizni / hipotetikus | „a Microscope 2027-re tudatos” |

**Szabály:** csak Observation és Evidence lehet `supports`; Inference/Hypothesis soha (sem önmagát, sem másik Inference nem támaszthat). Ez gyökerénél töri meg az önvisszacsatolást.

## 6. Anti-loop (echo és parafrázis)

A leggyakoribb támadás: az LLM egy következtés parafrázisát újra tárolja és „Observation”-ként adja be a cél-jához. A `link` a hívs:

- az új Evidence szövegét embeddeljük, és cosine-ozzuk a cél-kijelentéssel, illetve a korábbi echo-jáival;
- ha a hasonlóság >= `sim_threshold` (0.85) ÉS azonos `source_id`: saját-echo, a `distinct_sources` nem nő;
- ha a `source_id` új ÉS nem near-duplicate, `distinct_sources` +1;
- a `confidence` képlete nem használja a recall/energy értéket, csak support/refute/source-számlatot.

Így a „vorder Hot” (magas salience) soha nem növeli a stabilitást; a stabilitás csak több, független megfigyeléssel nő.

## 6. A `promote_recalled_blocks` gate (az egyetlen enforcement pont)

```
for (i, rec) in energy_loop {
    if imp >= protect_min_importance { continue; }
    if rec.energy < promote_energy || rec.activation_count == 0 { continue; }
    let class = class_of_block(i);
    if matches!(class, Inference | Hypothesis) {
        let ev = ledger.by_hash(hash_of_block(i));
        if ev.distinct_sources == 0 {
            audit.log(PROMO_GATE, i, "promotion_blocked:no_evidence");
            continue;   // fontosság-növelés blokkolva; a salience fut
        }
    }
    bumps.push((i, imp.saturating_add(1)));
}
```

A Dream többi része (replay, strengthen, prune), a ThoughtGraph és a Resonance változatlanul fut a salience-sávon; csak a fontosság-promótál gátolják be a bizonyítatlan következtésekkel. Opcionálisan `eviction_evidence_bias` (>0) bekapcsolásával az `evict_over_capacity` a confidence-et is evidenciálja a score-ba; alapérték 0, így az eviction eddigi viselkedése nem változik.

## 7. CLI / MCP / Rust API

### Rust API (re-export)

```
pub mod epistemic;
pub use epistemic::{EpistemicClass::{Observation,Evidence,Inference,Hypothesis},
                    EvidenceRecord, EvidenceLedger, confidence,
                    link_evidence, refute, audit_chain};
pub fn store_memory_with_epistemic(config: &Config, text: &str, layer: &str,
                            importance: u8, class: EpistemicClass,
                            supports: Option<&[&str]>) -> Result<(), String>;
```

### CLI (`src/cli.rs`)

- `store --class {observation|evidence|inference|hypothesis} --supports h1,h2`
- új `evidence` alparanec: `show`, `link`, `refute`, `audit`, `gate-stats`
- `recall` / `mql` szűrők: `--class` és `--confidence-min`; kiírás `confidence` + `class`

### MCP (`src/mcp.rs`)

- `memory_store` séma: `epistemic_class` és `supports`
- új toolok: `memory_evidence_show`, `memory_evidence_link`, `memory_evidence_refute`, `memory_evidence_audit`, `memory_evidence_gates`
- `memory_recall` és `memory_auto_context` eredménye `confidence` + `class`

### REST (`src/bridge.rs`)

- `/v1/store`: body `class` + `supports`, válaszban `confidence`
- `/v1/recall`: soronként `class` + `confidence`
- `/v1/evidence/:hash` GET: rekord + audit-kimenet

## 8. Konfiguráció (`[epistemic]` új szekció)

```
gate_promotion = 1
min_independent_sources = 1
sim_threshold = 0.85
search_k = 8
confidence = { observation_w = 30, source_w = 18, refute_w = 25, age_penalty = 5 }
eviction_evidence_bias = 0.0
```

## 9. Teszt-mátrix

### Unit
1. `confidence_does_not_rise_on_recall_alone` - a fő invaria: 100 energia-replay nem növeli a confidence-et, amíg független Evidence-link fel.
2. `self_echo_and_paraphrase_do_not_add_sources` - ön-refuxole és hasonló/szik azonos source soha nem növel; jegyző, független verzió, annak.
3. `refutation_lowers_confidence` - a refute csökkent, azután új kopdat obs visszamenőleg állítják.
4. `promotion_gate_blocks_unsupported_obs` - a gate nem lep meg, «distinct_sources==0», és a PROMO_GATE az audit-láncba kerül.
5. `cannot_link_inference_as_support`.
6. `audit_chain_tamper_detection` - a férfi bájt-módosítás → verifik csont-l.
7. `rebuild_is_idempotent` - build → evidence → build: confidence és class egyezik.
8. `flags_stamp_survives_rebuild`.

### Integration
9. három Hebbian-replay egy Hypothesis-en: importance nem emelkedik, de a Dream fut, a ThoughtGraph/Resonance él.
10. `eviction_evidence_bias=0` aloss: az eviction viselkedése változatlan (regresszió őr).
11. auto-context: a „hot” és a „confirmed” külön sor kapja; az alap-contention is elérhető.

### E2E
12. `/v1/store {class: hypothesis}` → 100x `/v1/recall` → `confidence==0`, `support==0`; aztán `/v1/store {class:observation,supports:[claim]}` → `confidence>0`, az audit `PROMO_GATE`-tal.
13. MCP `memory_evidence_audit` verifikál; `memory_evidence_gates` a blokkolások számát kapja.
14. multi-project: GLOBAL megfigyelés nem válik forrása egy másik projectnek (por-izoláció).

## 10. Önkritika

- **Forrás-hitelesség**: egyfelhasználós, lokális rendszerben a „független forrás” heurisztika; a szükséges színlős (több forrás) torzíthat. Ez nem igazság-orákulum, hanem bizonyíték-súly, amit az audit-lánc meghatározhatóvá tesz.
- **Confidence képlet**: konstans súlyok; a tuning nagyon is dönthet a konklúzió felett, ezért alapmenetként a súlyokat a fi-valishöte diver-en kell kalibrafni.
- **False negative**: a near-dup szűrő a nagyon heteli módszer *jó* megfigyel elsejét is „echo”nak és a konkrétst is; a rossz osztályozás (hipotézisnek jelölve) a jól-megfigyelt emeleteket is átfogragja, meg fogja marade. Kezelés: explicit `--class` és `evidence reclass`.
- **False positive**: téves „külön source” vést-jelöléssel a confidence tévesül akárő meredek lehet; a PRÓs az a verifiable audit-chain és a discrete upstream.
- **Karbantartás**: a réteg a meglévő, törékeny append/rebuild úton fut; újrarolás, de szemléűdő, komplexit a javítás idejét is növeli.

## 11. Teljesítmény-költség

- store/link: egy FNV hash + (eseti) bounded top-8 koszinusz-eles; a confidence elöformált, nem élő számítás.
- near-dup csak a `link`-kénysben; a recall hot-path-et nem érinti; O(k) `search_k=8`, soha O(n).
- a dream-promotiális loop egy HashMap-féle O(1) bekúrt.
- új lemez ~44 B/emlék a `embeddings.bin` (2.3 GB) mögé fixarákat léptet.

## 12. Implementáció állapota (2026-08-03)

### Kész

- `src/epistemic.rs`: EpistemicClass, EvidenceRecord, EvidenceLedger, AuditChain,
  confidence(), link_evidence(), refute(), check_promotion_gate()
- 21 unit teszt (C1-C5 invariánsok)
- Header offset javítás: importance byte 13 → 48, layer_id 12 → 17 (`dream.rs`)
- `[epistemic]` konfig szekció (`config.rs`)
- Promotion gate beépítve a `promote_recalled_blocks`-ba (`dream.rs`)
- 334/334 teszt zöld (313 meglévő + 21 új)
- Clippy tiszta

### Nyitott (következő lépés)

- `build.rs`: flags byte stampolás a ledgerből rebuildkor
- `reader.rs`: recall eredmény confidence + class mezővel
- CLI: `evidence` alparancs (show/link/refute/audit/gate-stats)
- MCP: `memory_evidence_*` toolok
- REST: `/v1/evidence/:hash` endpoint

## 13. Nyitott döntések

| # | Döntés | Javaslat |
|---|---|---|
| 1 | A gate be legyen-e alapértelmezve? | Igen |
| 2 | `eviction_evidence_bias` alapértéke | 0 (nem változtat a meglévő evictionön) |
| 3 | hot vs. confirmed az auto-contextben | két sor / címke |
| 4 | `sim_threshold` | 0.85 (kalibrálható, `search_k` korlátozza) |
