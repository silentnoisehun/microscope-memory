# Microscope Memory — forensic feladat: dream.rs meta.bin író + kernel

## Kontextus

Repo: `C:\Users\mater\microscope-memory-clone` (Rust 2021).
A kanonikus memória a **C: meghajtón** van:
`C:\Users\mater\AppData\Local\microscope-memory\output\`
(D: meghajtó egy másik példány, csak kontrollreferencia.)

## Megerősített tünetek (parancssorból reprodukálva)

1. `meta.bin` **148 bájt**, a várt formátum `META_HEADER_SIZE(16) + 9*DEPTH_ENTRY_SIZE(8) + 32 + 32 = 152`.
   A fájl tehát **12 bájtos fejléccel** íródott (magic+version+block_count, `depth_count` nélkül).
   A `reader.rs` 16 bájtos fejlécet vár → **4 bájtos off-by-4 elcsúszás** a depth-táblán.

2. `meta.bin` depth-táblája a fájban (16 bájtos olvasással):
   D0: start=12359 count=12359, D1..D7: start=0 count=12359, D8: start=0 **count=3595294311**.
   Valós depth-eloszlás a `microscope.bin`-ből számolva:
   D0:1 D1:14 D2:880 D3:4373 D4:12344 D5:76773 D6:189542 D7:540707 D8:568527 (összesen 1393161).

3. **`microscope-mem.exe look 0.5 0.5 0.5 8 3` kilép**:
   `memory allocation of 57524709136 bytes failed` (a D8 count-ból jön).
   Emiatt a `memory_look` MCP tool és a `memory_recall` (melyik a `tool_recall`-ban a
   `depth_ranges`-re épül) is összeomlik → ez a `mcp__microscope_memory_recall failed`
   open loopok gyökere.

4. `doctor` a C: indexen: **`[FAIL] 1345920 block(s) have CRC mismatches!`** (1 393 161 blokkból 1 345 920 hibás, ~96,6%).
   Ugyanez a kód a **D:** indexen: `[OK] All block CRCs verified.` (1 314 995 blokk).
   → A CRC-ellenőrzés kódja jó; a C: index *tartalma* sérült.

5. `data.bin` (6 687 836 B) és a header-ek `data_offset`/`data_len` értékei konzisztensek:
   a blokkok szinte teljesen folytonosak (1 gap, 0 overlap), `sum(data_len)` == data.bin méret,
   `max(data_offset+len)` == 6 687 864.
   A `data.bin` olvasható, értelmes szöveget tartalmaz.
   → A `data_offset`-ok JÓK. A **crc16 mező** a headerben rossz.

## A gyanúsított gyökérok: `src/dream.rs`

A `src/dream.rs` `forget_old_thoughts()` függvénye (kb. 425–560. sor) **második implementációja**
a `meta.bin` / `microscope.bin` / `data.bin` írására, és a `src/build.rs` mellett fut.

A `dream.rs` gyanúsított hibái (ellenőrizd, javítsd):

- **A) Fejléc-eltolás.** kb. 519–526. sor:
  ```rust
  if meta.len() >= 8 { new_meta.extend_from_slice(&meta[..8]); } else { new_meta.extend_from_slice(b"MSC4"); }
  new_meta.extend_from_slice(&(n as u32).to_le_bytes());
  ```
  Ez 12 bájtos fejlécet ír (magic+version+block_count), **kihagyja a `depth_count` u32 mezőt**.
  A `build.rs` 16 bájtos fejlécet ír (magic+version+block_count+depth_count).
  A `reader.rs`, `wasm.rs`, `snapshot.rs`, `commands/verify.rs` mind `META_HEADER_SIZE=16`-ot várnak.

- **B) Depth bájt pozíció.** kb. 532. sor: `let depth = headers[off + 14]; // depth is at byte 14`
  A `BlockHeader` `#[repr(C, packed)]`: x,y,z,zoom = 4× f32 = 16 bájt, **utána** `depth: u8` a 16. bájton,
  `layer_id` a 17-en, `data_offset` 18..22, `data_len` 22..24, `crc16` 30..32.
  A `dream.rs` saját doc-kommentje is ezt írja (`after depth (16) and layer_id (17)`),
  de a kód 14-et használ → a 14. bájt a `zoom` f32 alsó bájtja.

- **C) Depth range `start` számítás.** kb. 538–543. sor:
  ```rust
  let mut running_start = 0u32;
  for &count in &depth_counts[..9] {
      new_meta.extend_from_slice(&running_start.to_le_bytes());
      new_meta.extend_from_slice(&count.to_le_bytes());
      running_start += count;
  }
  ```
  Ez a D0-tól haladó összeget írja ki, **nem** a valós (start,count) pozíciókat.
  A `build.rs` a tényleges blokkpozíciókból számol (`range_start`).
  Ha a kompakció után a blokkok nem mélység szerint rendezettek, a tábla hibás lesz.
  → A helyes megoldás: töröld a blokkokat **mélység szerint rendezett sorrendben** megtartva,
     vagy számítsd a valós start-ot a tényleges kimeneti sorrendből.

- **D) `data_offset` nem íródik át.** A `dream.rs` új `data.bin`-t épít `new_data`-ba, de a
  header-ek `data_offset` mezőjét **változatlanul másolja** (`new_headers.extend_from_slice(&headers[...])`).
  Ha a kompakció törölte a blokkokat, a megmaradó header-ek `data_offset`-je a régi
  `data.bin`-ra mutat, nem az újra. → a CRC-ellenőrzés 96,6%-ban elhasal, mert a header
  `crc16` mezője a *régi* helyzetű adatra vonatkozik, a reader pedig az *új* `data.bin`-ből olvas.
  A `crc16`-ot az új adat újra számolva kell írni, és a `data_offset`-et az új pozícióra állítani.

- **E) Hiányzik a `depth_count` és a merkle/layers hash.** A `build.rs` a depth-tábla után
  32 bájtes merkle rootot és 32 bájtes layers hasht ír. A `dream.rs` a `meta_tail_start`
  = `META_HEADER_SIZE + 9*DEPTH_ENTRY_SIZE` = 88 offszetről másolja a maradékot — ha a
  forrás meta 148 bájtos volt, ez a 88-as offszet **a merkle root közepébe** mutat.
  → a merkle root és a layers hash félre kerül / elcsúszik.

## Amit kérünk

1. **Erősítsd meg vagy cáfold** az A–E pontokat a `src/dream.rs` tényleges kódjából olvasva.
   Nézd meg a teljes `forget_old_thoughts` függvényt és a `src/build.rs` `build()` / `build_consolidating()`
   meta-íróját, és hasonlítsd össze őket. A `src/build.rs` a referencia (helyes) implementáció.
2. **Javítsd a `src/dream.rs`-t**, hogy azonos (MSC4, 16 bájtos fejléc) formátumot írjon,
   mint a `build.rs`:
   - a `depth_count` mező is bekerüljön,
   - a depth a 16. bájt legyen,
   - a depth-range tábla a valós kimeneti pozíciókat tükrözze,
   - a `data_offset` és `crc16` az új `data.bin`-hez igazodjon (a `data_len` megmarad),
   - a merkle root + layers hash a helyes helyre kerüljön (vagy a `build.rs`-hez hasonlóan
     újraszámolva).
3. **Tegyél védőhálót a `reader.rs`-be**: a `meta.bin` megnyitásakor ellenőrizd, hogy a
   hossz legalább `META_HEADER_SIZE + 9*DEPTH_ENTRY_SIZE + 64` (152), és hogy a depth-tábla
   nem haladja meg a `block_count`-ot. Sérült `meta.bin` esetén **ne** legyen 57 GB-os
   allokáció és ne legyen abort — hanem `Err(...)` a hívónak, emberi olvasható üzenettel.
   (Nézd meg a `tests/index_corruption.rs` fájlt: ott van egy minta a hasonló védelemre.)
4. **Írj tesztet** (`tests/` alá vagy a `dream.rs` `mod tests`-be), ami a hibás
   `meta.bin`-t építi fel, és bizonyítja, hogy:
   - a reader elutasítja (nem crashel, nem allokál száz GB-ot),
   - a `dream.rs` újraírt meta-ja `META_HEADER_SIZE` + 9*8 + 64 = 152 bájt,
   - a depth-tábla visszaolvasható és összhangban van a blokkok tényleges mélységével.
5. Futtasd: `cargo test`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`.

## Fontos megkötések

- **NE futtass `microscope-mem rebuild`-et vagy bármilyen író parancsot a C: indexen!**
  Az index sérült; a javítás kód-szintű (a `dream.rs` javítása), és utána egy
  `build`/`rebuild` újraépíti az indexet a layer fájlokból.
  A layer fájlok (`C:\Users\mater\AppData\Local\microscope-memory\layers\*.txt`) épek.
- A `src/dream.rs` környezetében a `crate::HEADER_SIZE` (50), `crate::META_HEADER_SIZE` (16),
  `crate::DEPTH_ENTRY_SIZE` (8) konstansok használatosak.
- Ne nyúlj a `src/build.rs`-hez, kivéve ha tényleg hibás (a kontroll index provingozza, hogy jó).
- Rust 2021, idiomatikus stílus, `thiserror` a hibákra, a projekt AGENTS.md szerint.

## A második (kisebb) feladat: ORA kernel finomhangolás

Fájl: `omp-integration/ora-master-kernel.ts`

Problémák, amiket a diagnózis kimutatott:

1. **`detectSemanticProvider()` az ESM-ben `require()`-t hív** (357. sor):
   ```ts
   const text = readSync ? require("node:fs").readFileSync(cfg, "utf8") : "";
   ```
   Az `ora-master-kernel.ts` ESM modul (`import ... from "node:fs"` a tetején), a `require`
   nincs definiálva → `ReferenceError` a függvény hívásakor, ami az **egész `session_start`-ot**
   eldobja (a `try/catch` a `before_agent_start`-ban nem fogja, mert a `session_start`
   hook hibája nincs ugyanott elnyelve). A fájl tetején már importálva van a `readFileSync`?
   Nézd meg a 22–24. sort: `import { appendFileSync, existsSync, openSync, readSync, closeSync } from "node:fs";`
   → **A `readFileSync` nincs importálva.** Importáld, és töröld a `require`-os trükköt.

2. **A `TRIVIAL_CHAT` regex nem fogja a "szia"-t a `isActionableTask`-ban.**
   A `TRIVIAL_CHAT` regex: `/^(szia|hello|hi|...)$/i` — pontos illesztés, tehát a
   "szia !" (felkiáltójellel) átmegy rajta → a kernel "szia"-ra is ráindítja a teljes
   Octopus direktívát és a memória-visszakeresést. A `before_agent_start`-ban a
   `userMsg.length < 3` check alatt van, de a "szia !" 5 karakter.
   → Vezess be egy `isTrivialChatter(text)` függvényt, ami whitespace- és
   kiírásjel-mentesítés után, a "szia"/"hello"/"ok"/stb. **szavakra** néz (nem
   pontos egyezésre), és használd mindenhol, ahol ma `TRIVIAL_CHAT.test()` van.

3. **A `message_end` hook `lastUser`-t használ, de `lastUser` csak a `before_agent_start`-ban
   frissül.** Ha egy sessionben az első üzenet volt a parancs (`/ora`), a `lastUser` üres marad
   → a `gate reflect` üres task-ot kap. Minor, de érdemes `message_end`-ben is
   frissíteni/őrizni a valódi felhasználói üzenetet.

4. **A `tool_result` hook minden hibát open looppá nyit** — még akkor is, ha a hiba
   maga a loop-létrehozás hibája (recursion). A kernel logban ezt látjuk:
   `open loop auto-healing: tool error (mcp__microscope_memory_store)` — vagyis a
   **memória store** hibája is újabb open loopot nyit, és így önmagát felerősíti.
   → Adj egy **deduplikációs/korlátot**: ha az utolsó N másodpercben már nyíltunk
   loopot ugyanarra a hibára, ne nyissunk újat; és ha a hiba maga egy `memory_store`
   vagy `memory_recall` hiba, **ne** nyisson loopot (az a hiba az index sérültségét
   jelzi, nem új munkát).

5. **A `runBinBg` a `setTimeout`-os killt a Node event loopon tartja** — 60-120 másodperces
   timeoutekkel. A `child.unref()` a process-t elengedi, de a `setTimeout` nem. Sok
   ilyen hívás esetén a Node process nem tud kilépni. → `timer.unref()`.

## Amit vissza kell adni

- A gyökérok-elemzés (melyik pontban miért).
- A javított fájlok (a diff vagy a teljes új `dream.rs` / `ora-master-kernel.ts` rész).
- A tesztek eredménye (`cargo test` kimenet).
- A `reader.rs` védőháló megoldása.
- A kernel-javítások.
