# Microscope Memory — Teljes Rendszerelemzes

## 1. Alapkoncepcio

**Mikroszop-metafora**: A memoria egyforma meretu blokkokban (256 char) letezik minden melysegben. A query zoom szintje hatarozza meg, mit latsz — mint egy valodi mikroszop. Nem az adatmeret valtozik, hanem a **felbontas**.

**CPU cache analogia**: L1/L2/L3 cache hierarchia, de kognitiv memoriara alkalmazva. A fizikai memoria-menedzsment mintait hasznalja emberi/AI memoria tarolasara.

---

## 2. Architektura — Tiszta Rust

| | **Rust** (`lib.rs` + `main.rs`, ~1770 sor) |
|---|---|
| **Szerep** | Hierarchia-epito + produkcios lekerd. motor |
| **Adat** | JSON bemenet -> pure binary (mmap, zero-copy) |
| **Sebesseg** | 37-102 ns/query (mmap, tiered grid) |
| **Melysegek** | D0-D8 (9 szint) |
| **Crypto** | SHA-256 hash chain + Merkle fa |

Egyetlen Rust binarisban: build, query, store, recall, verify.

---

## 3. A 9 melysegi szint (Rust)

```
D0: Identity          1 blokk     — Az egesz memoria egy mondatban
D1: Layer summaries   9 blokk     — Retegenkent 1 osszefoglalo
D2: Topic clusters   112 blokk    — 5 elem/cluster csoportositas
D3: Individual items  540 blokk   — Egyeni memoria elemek
D4: Sentences       1,363 blokk   — Mondat szintu felbontas
D5: Tokens          6,138 blokk   — Szavak (max 8/mondat)
D6: Syllables      26,353 blokk   — Szotagok (3-5 char chunk)
D7: Characters     96,714 blokk   — Egyeni karakterek
D8: Raw bytes      97,031 blokk   — Hex byte reprezentacio
                  ──────────
                  228,261 total
```

A Rust D0-D8-ig generalja a teljes hierarchiat (reteg osszefoglalo -> mondat -> szotag -> karakter -> byte), ami a teljes "optikai zoom" metaforat valosit meg.

---

## 4. Binaris formatum

### BlockHeader — 32 byte, `#[repr(C, packed)]`
```
x: f32          — 3D pozicio X (0.0-1.0)
y: f32          — 3D pozicio Y
z: f32          — 3D pozicio Z
zoom: f32       — normalizalt melyseg (depth/8.0)
depth: u8       — 0-8
layer_id: u8    — 0-8 (melyik memoria reteg)
data_offset: u32 — offset a data.bin-ben
data_len: u16   — adat hossz (max 256)
parent_idx: u32  — szulo blokk index
child_count: u16 — gyerekek szama
_pad: [u8; 2]   — alignment padding
```

### Fajlok
| Fajl | Meret | Tartalom |
|------|-------|----------|
| `microscope.bin` | 7,133 KB | Header tomb (228K x 32 byte) |
| `data.bin` | 891 KB | Szoveges adat blokkok |
| `meta.bin` | 88 byte | Magic + block count + depth ranges |
| `chain.bin` | 17,833 KB | SHA-256 hash chain (228K link) |
| `merkle.bin` | 7,133 KB | Merkle fa (228K node x 32 byte hash) |
| `append.bin` | valtozo | Store append log (rebuild-ig) |

**Osszesen: ~33 MB** binaris, amibol az aktiv query-hez csak 8 MB kell (headers + data).

---

## 5. 3D Terbeli Pozicionalas

**Determinisztikus**: Ugyanaz a tartalom mindig ugyanazokra a koordinatakra kerul.

```
content_coords(text, layer):
    hash = FNV-1a varians (3 x u64 state, elso 128 byte)
    base_x = hash[0] / max -> 0.0-0.25 tartomany
    base_y = hash[1] / max -> 0.0-0.25 tartomany
    base_z = hash[2] / max -> 0.0-0.25 tartomany
    return base + layer_offset
```

**Reteg offsetek** — minden reteg sajat terreszet kap:
```
long_term:    (0.00, 0.00, 0.00)  — origo kozeleben
short_term:   (0.15, 0.15, 0.15)  — kozepen
associative:  (0.30, 0.00, 0.00)  — X tengely menten
emotional:    (0.00, 0.30, 0.00)  — Y tengely menten
relational:   (0.30, 0.30, 0.00)  — XY sik
reflections:  (0.00, 0.00, 0.30)  — Z tengely menten
crypto_chain: (0.30, 0.00, 0.30)  — XZ sik
echo_cache:   (0.00, 0.30, 0.30)  — YZ sik
rust_state:   (0.15, 0.00, 0.15)  — XZ atlo
```

Melyebb szintek (D4->D8) a szulo poziciojabol **egyre kisebb offset**-tel ternek el:
- D4: +/-1/25,500
- D5: +/-1/255,000
- D6: +/-1/2,550,000
- D7: +/-1/25,500,000
- D8: +/-1/255,000,000

Ez biztositja, hogy egy szo karakterei terben is kozel vannak a szohoz.

---

## 6. Lekerdezesi strategiak

### AoS (Array-of-Structs) — baseline
Brute-force linearis scan az adott depth range-ben. Minden blokkra L2 tavolsagot szamol.

### Tiered Spatial Grid — optimalizalt
```
D0-D2: Raw mmap scan     (1-112 blokk, L1 cache-ben elfer)
D3-D8: SpatialGrid       (adaptiv felbontas)
```

**SpatialGrid** mukodese:
1. A ter `res^3` cellara van osztva (4^3 -> 32^3, blokk szam alapjan)
2. Minden blokk a koordinatainak megfelelo cellaba kerul
3. Query: a celcella + 26 szomszed (3x3x3 kocka) atvizsgalasa
4. `select_nth_unstable` (O(n) partial sort) a top-k kinyeresere

**Adaptiv felbontas**:
| Blokkok | Grid meret | Cellak |
|---------|-----------|--------|
| <200 | 4^3 | 64 |
| <1,000 | 8^3 | 512 |
| <5,000 | 12^3 | 1,728 |
| <20,000 | 16^3 | 4,096 |
| <50,000 | 24^3 | 13,824 |
| 50,000+ | 32^3 | 32,768 |

### 4D Soft Zoom
Zoom mint negyedik dimenzio: `distance = spatial_L2 + (zoom_diff x weight)^2`. Cross-depth keresesnel hasznos.

### Recall (termeszetes nyelvi)
1. Auto-zoom: query szavak szama -> idealis depth tartomany
2. Spatial L2 kereses a tartomanyban
3. Keyword boost: ha a szoveg tartalmazza a query kulcsszavait, -0.1/hit a tavolsagbol
4. Append log kereses (meg nem rebuild-elt memoriak)
5. Deduplikacio + rendezes

---

## 7. Crypto reteg

### Hash Chain (chain.bin)
```
ChainLink (80 byte):
    content_hash: [u8; 32]   — SHA-256(blokk szoveg)
    prev_hash:    [u8; 32]   — SHA-256(elozo ChainLink bajtok)
    timestamp_us: u64        — UNIX microsec
    block_index:  u32        — blokk index
    layer_id:     u8
    depth:        u8
    _pad:         [u8; 2]
```

- **Szekvencialis**: minden link az elozo link hash-ere mutat
- **Tamper detection**: barmelyik link modositasa megtori a lancot
- **Append**: `store` parancs valos idoben boviti (nem kell rebuild)
- **Verify**: 228K link ellenorzese 25 ms

### Merkle Tree (merkle.bin)
```
MerkleNode (32 byte):
    hash: [u8; 32]   — SHA-256(content_hash + children_hashes)
```

- **Fa struktura**: a blokkok szulo-gyerek kapcsolatait koveti (D0->D8)
- **Level**: hash = SHA-256(szoveg)
- **Belso node**: hash = SHA-256(sajat_content + concat(gyerek_hash-ek))
- **Root**: `cafe8887d0a5d4fe` — az egesz memoria egyetlen hash-ben
- **Branch verify**: barmelyik blokk -> root utvonal O(depth) lepesben ellenorizheto
- **Verify**: 228K node 50 ms

---

## 8. Memoria retegek (9 reteg)

| Reteg | Tartalom | Elemek |
|-------|----------|--------|
| `long_term` | Kodbazis elemzes, reflexiok, tartos tudas | 396 |
| `short_term` | Ora bot uzenetek, session tortenet | 19 |
| `associative` | Hullam, rezonancia, emotimem, Rust, Telegram | 42 |
| `emotional` | Erzelmi allapotok | 2 |
| `relational` | Szemelyek, kapcsolatok | 6 |
| `reflections` | Onreflexiok | 4 |
| `crypto_chain` | Session log-ok | 22 |
| `echo_cache` | Rovid valasz cache | 8 |
| `rust_state` | Rust tudas snapshot | 38 |

Forras: `layers/*.json` (projekt konyvtarban)

---

## 9. Store -> Rebuild -> Recall folyamat

```
store "text" --layer X --importance N
    |-- content_coords(text, layer) -> (x, y, z)
    |-- append.bin-be ir (18 byte header + text)
    +-- chain.bin-t boviti (append_chain_link)

rebuild
    |-- layers/*.json beolvasas
    |-- append.bin beolvasztas D3-ba  <-- [JAVITOTT BUG]
    |-- D0->D8 hierarchia felepites
    |-- microscope.bin + data.bin + meta.bin iras
    |-- chain.bin ujraepites
    |-- merkle.bin ujraepites
    +-- append.bin torles

recall "query"
    |-- auto_zoom(query) -> center_zoom, radius
    |-- content_coords(query, "query") -> (qx, qy, qz)
    |-- L2 spatial kereses (zoom_lo..zoom_hi)
    |-- Keyword boost kereses
    |-- Append log kereses
    +-- Deduplikacio + top-k rendezes
```

---

## 10. Performance osszefoglalo

| Muvelet | Ido |
|---------|-----|
| **Look (D0, 1 blokk)** | ~37-100 ns |
| **Look (D3, 540 blokk, Grid)** | ~5 us |
| **Look (D7, 97K blokk, Grid 32^3)** | ~32 us |
| **Look (D8, 97K blokk, Grid 32^3)** | ~35 us |
| **Store** | ~6 ms |
| **Recall** | ~700 us |
| **Find (text search)** | <1 ms |
| **Build** | ~110 ms (228K blokk) |
| **Verify chain** | 25 ms |
| **Verify merkle** | 50 ms |
| **Tiered vs AoS** | **15.9x speedup** |

---

## 11. Vizualizacio (opcionalis, `--features viz`)

| Fajl | Sor | Szerep |
|------|-----|--------|
| `src/bin/viz.rs` | 288 | Fo belepesi pont, wgpu init |
| `src/viz/renderer.rs` | ~411 | wgpu render pipeline |
| `src/viz/camera.rs` | 80 | 3D kamera vezerles |
| `src/viz/scene.rs` | 75 | Blokkok -> renderelheto pontok |
| `src/viz/ui.rs` | 146 | egui UI (zoom slider, layer filter) |
| `src/viz/picking.rs` | 53 | Blokk kivalasztas kattintassal |
| `src/viz/edges.rs` | 55 | Szulo-gyerek elek rajzolasa |
| `src/viz/shaders/` | ~92 | WGSL shaderek (point + edge) |

---

## 12. Fajlmeret osszesites

| Komponens | Sorok |
|-----------|-------|
| `src/lib.rs` | ~1,635 |
| `src/main.rs` | 137 |
| `src/viz/*` | ~1,100 |
| **Osszesen** | **~2,872** |

---

## 13. Azonositott jellemzok es trade-off-ok

**Erossegek:**
- Tiszta Rust — egyetlen binaris, nincs Python fuggeseg
- Zero-copy mmap: a kernel kezeli a memoria-lapozast, nincs desszeerializacio
- Determinisztikus pozicionalas: nincs kulso index, a tartalom IS a cim
- Crypto integritas: chain + Merkle egyutt, branch verify O(depth) lepes
- Tiered grid: 16x speedup minimalis memoria overhead-del
- Relativ utvonalak — hordozhato projekt

**Jelenlegi korlatok:**
- `layer_texts` `&str` lifetime kotes -> az append merge `find()` + `push()` keringos
- `verify_branch()` az egesz Merkle fat ujraszamolja egyetlen branch-hez is (O(n) a O(depth) helyett)
- Auto-zoom kizarolag szoszamon alapul, szemantikai elemzes nelkul
- D6-D8 (szotag/char/byte) a blokkok 96%-a, de ritkan lekerdezett

---

*Generalta: Claude Code, 2026-03-20*
*Projekt: microscope-memory v0.1.0 — Tiszta Rust*
