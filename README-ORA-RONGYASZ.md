# Ora-Rongyász / Hope Ecosystem

> **„Egy helyre rakjam mindet, és ne serverekkel kapcsoljam össze, hanem egy közös spine-on van minden. Minden binárist egy helyre gyűjtök, és onnan fut."**

Ez a dokumentum a rendszer **teljes működését** írja le: mi hol fut, hogyan kapcsolódnak össze, mit mérünk, és mi az, amitől jobb lehet.

**Verzió:** 2026-10-24 · **Snapshot alap:** `D:\codex\_snapshots\20261024-020806`

---

## 1. Az egyetlen igazság: a `.hope\bin`

Minden bináris **innen fut**: `C:\Users\mater\.hope\bin` (76 fájl, 176 MB).

A build-kimenetek (`target\release`, `.bio-target\release`, `.minimax\bin`, `octopus-agent-kit\bin`) **csak források** — a `sync-bins.ps1` másol belőlük, de onnan soha nem indítunk.

**Miért:** a build-könyvtárak driftelnek (ugyanannak a binárisnak augusztusi és októberi buildje is létezik). Ha onnan futtatunk, a felügyelt processz és a telepített bináris nem ugyanaz.

**Ellenőrzés:**
```powershell
Get-CimInstance Win32_Process |
  Where-Object { $_.ExecutablePath -like 'C:\Users\mater\.hope\bin\*' } |
  Select-Object ProcessId, @{n='Bin';e={Split-Path $_.ExecutablePath -Leaf}}
```

**Telepítés / frissítés:**
```powershell
& 'C:\Users\mater\.hope\bin\sync-bins.ps1'          # legfrissebb forrás nyer
& 'C:\Users\mater\.hope\bin\sync-bins.ps1' -Force   # minden forrás, azonos fájl mérettel is
```

> A `Copy-Item` `-ErrorAction Stop` **nélkül** a file-in-use (futó bináris) nem dob kivételt, és a sync hamis „sikeres" lesz. Ezért van benne.

---

## 2. A topológia: spine-ok, nem portok

A „spine" **fájl** (mmap), nem hálózati szolgáltatás. Minden alrendszernek saját spine-ja van, és ezeket az organok olvassák/írják közvetlenül — nem RPC-n keresztül.

```
                          ┌─────────────────────────────────────┐
                          │        ORA-RONGYÁSZ KERNEL          │
                          │  (omp hook: ora-master-kernel.ts)   │
                          └───┬───────┬───────┬───────┬────────┘
                              │       │       │       │
        ┌─────────────────────┼───────┼───────┼───────┼──────────────────┐
        │                     │       │       │       │                  │
   ┌────▼─────┐        ┌──────▼──┐ ┌──▼────┐ ┌▼──────┐ ┌▼────────┐  ┌────▼─────┐
   │  QUEEN   │        │  DRONE  │ │WAVE-  │ │HULLAM │ │ STRATOS │  │ MICRO-   │
   │ omega-   │◄─UDP──►│unified- │ │ECHO   │ │       │ │         │  │ SCOPE    │
   │ master   │  8888  │mind     │ │.wv3   │ │.rgs   │ │.spine  │  │ (mmap)   │
   │          │        │ serve   │ │       │ │       │ │         │  │          │
   └────┬─────┘        └────┬────┘ └───┬───┘ └───┬───┘ └────┬────┘  └────┬─────┘
        │                   │         │         │          │            │
        │              ┌────▼─────────▼─────────▼──────────▼────┐       │
        │              │        OCTOPUS (.octopus-rust)         │       │
        │              │   arms / roots / resonance hash-chain  │       │
        │              └────────────────────────────────────────┘       │
        │                                                              │
        └──────────► MODELL-ROUTER (10100 opencodex / 10103 universal) ┘
```

### 2.1 A szervek

| Szerv | Bináris | Spine / állapot | Port | Feladat |
|---|---|---|---|---|
| **Queen** | `omega-master.exe` | `D:\codex\.bio-drones.json` | UDP **8888** | Központi orchestrátor, drone-regisztráció, homeostasis |
| **Drone** | `unified-mind.exe serve` | `D:\codex\.octopus-rust` | UDP 8888 (kliens) | 24 bio-skill tickelése, keretkibocsátás |
| **Wave-Echo** | `wave-echo.exe` | `D:\codex\digital_echo_rust\wave_echo.wv3` | – | Sablontár (265 sablon), hullám-interferencia, reflex-kódgenerálás |
| **Hullam** | `hullam.exe` | `D:\codex\digital_echo_rust\hullam_spine.rgs` | – | Digital Echo vault + PSI-slotok, kar-aktiválás |
| **STRATOS** | `stratos-server.exe` | `E:\STRATOS\data\stratos.spine` | HTTP/WS **8090** | **A spine írója** — Stellar ciklus (LLM-hívással) |
| **Microscope** | `microscope-mem.exe` | `C:\Users\mater\AppData\Local\microscope-memory` | – | 12 réteg, 9 mélység, bináris mmap memória |
| **Octopus** | `octopus-runtime.exe` | `D:\codex\.octopus-rust` | – | Blade-runtime, append-only resonance hash-lánc |
| **Hope** | `hope.exe` | – | – | A kódoló társad (CLI + szerver) |
| **Router** | `mcp-router.exe` / `universal-router.exe` | – | 10102 / 10103 | Modell-routing, minőségi visszacsatolás |


### 2.1.1 A Teljes Bio-Bináris Képesség-Háló (56 Különálló Bio-Szerv)

A rendszer nemcsak a 7 folyamat-szintű daemonból áll, hanem **56 specializált biológiai algoritmus-binárisból**, melyek mindegyike a C:\Users\mater\.hope\bin könyvtárban él, és a Git Reflex által arany-állapotban zárolt:

1. **Idegi & Somatikus Szervek (Neural & Vitals - 6 db):**
   - agus-nerve.exe — CPU/RAM organ sensing, dinamikus vagus fék, szomatikus stressz feedback
   - homeostasis.exe — Belső egyensúly, mező-energia és homeosztatikus feszültségszabályzó
   - rain-connectome.exe — Statikus import- és függőségi gráf térképező
   - rain-synapse.exe — Hebb-féle együttfutási (co-change) tanulás git előzményekből
   - io-metrics.exe — Prometheus és belső metrika exporter
   - organism-tui.exe — Valós idejű terminál állapotvizualizáció

2. **Genetikai & Evolúciós Eszközök (Genetic & Evolutionary - 13 db):**
   - iral-infect.exe — Gyors regex kódtranszformáció nagy fájlrendszereken
   - hox-diff.exe — Projekt-struktúra és morfológiai különbségvizsgáló
   - plasmid-dream.exe — Prediktív hiba- és mintázat-elemző (build trendek)
   - plasmid-inject.exe — Sebészi pontosságú sorszintű kódbefecskendező
   - plasmid-conjugate.exe — Horizontális géntranszfer és moduláris illesztés
   - mutation-sentinel.exe — Fájlmutáció figyelő és auto-freeze snapshot készítő
   - ibosome-synth.exe — Sablon-alapú fehérjeszintézis / kódgeneráló motor
   - crispr-patch.exe — Célzott genomiális és forráskód javítás
   - poptosis-cascade.exe — Programozott sejt- és processzhalál, tiszta erőforrás-felszabadítás
   - lastema-regen.exe — Dedifferenciálódás és sérült komponensek regenerációja
   - yamanaka-deaging.exe — Állapotfiatalítás és regressziós reset
   - 	ransposon-hive.exe — Ugráló gének és dinamikus kódáthelyezés
   - epigenetic-switch.exe — Hiszton-módosítás és futásidejű viselkedés-kapcsoló

3. **Micélium, Szimbiózis & Raj Szervek (Mycelial & Swarm - 15 db):**
   - mycelium-spread.exe — Hifális kódhálózati felderítő
   - mycelial-vault.exe — Földalatti mikorrhiza tároló
   - mycorrhiza-trade.exe — Erőforrás- és információcsere csatorna
   - physarum-path.exe — Nyálkagomba legrövidebb útvonalkereső és hálózati optimalizáló
   - xenobot-swarm.exe — Szintetikus biológiai klaszterezés
   - 	ardigrade-tun.exe — Kriptobiózis és állapotfagyasztás szélsőséges terhelésnél
   - 	cell-sentinel.exe — Immunológiai patogén- és anomália-detektor
   - chameleon-stealth.exe — Adaptív álcázás és kontextus-illesztés
   - chimera-fusion.exe — Több genomú hibrid kódösszeolvasztó
   - electrocyte-burst.exe — Nagy energiájú kisülés és gyors impulzusadás
   - magnetosome-nav.exe — Földmágneses tájékozódás és vektornavigáció
   - morpho-electric-mesh.exe — Bio-elektromos mintázatvezető rács
   - quorum-signal.exe — Sűrűségfüggő kollektív döntéshozó mechanizmus
   - symbiont-engine.exe — Szimbiotikus együttműködési motor
   - wasm-organoid.exe — Homokozott mini-szerv szimulátor

4. **Telepátia & Kollektív Szinkron (Telepathy & Sync - 5 db):**
   - collective-sync.exe — Rajóra és koherencia-szinkronizáló
   - 	elepathy-sync.exe — Szervek közötti közvetlen állapotátvitel
   - 	elepathy-entangle.exe — Fáziszárt összefonódási motor
   - iophoton-telepathy.exe — Fénysebességű belső kommunikációs busz
   - ultrasonic-modem.exe — Nagyfrekvenciás akusztikus belső jelátvitel

5. **Kvantum Hullám & Rezonancia Mező (Wave & Resonance Field - 17 db):**
   - wave-field.exe — Globális skalár hullámmező szimulátor
   - wave-sculptor.exe — Interferenciás hullámformáló
   - wave-encoder.exe — Diszkrét hullám-kódoló
   - wave-cryo-tx.exe / wave-cryo-rx.exe — Kriogenikus hullámállapot adó/vevő
   - path-resonance.exe — Rezonancia-útvonal követő
   - iron-resonate.exe — Vas-alapú rezonáns visszacsatoló
   - magneto-acoustic.exe & magneto-geo.exe — Mágneses mező és geo-moduláció
   - grid-warp.exe — Topológiai térgörbítő
   - ether-excite.exe & ether-fabric.exe — Étermező excitáció és szövet
   - eqm-pulse.exe & eqm-methy.exe — Kvantum metilációs és impulzus modulátor
   - org-cube.exe & 
exus-logic.exe — Kollektív asszimilációs és nexus logika
   - omega-point.exe — Végső konvergencia-pont

### 2.2 A STRATOS a porton, de a spine-on él

**A STRATOS az egyetlen szerv, aminek szervere van**, mert a Stellar ciklus LLM-hívást végez. A spine maga egy 262144 B-os mmap-fájl (16 × 16 KiB POD-oldal, 16 slot).

| Slot | Tartalom | Méret |
|---|---|---|
| 9 | **Cycle** — `phase` u32 LE @0, `cycle_count` u64 LE @4 | @147456 |
| 10 | **Emotion** — `valence` f32 @0, `intensity` @4, `confidence` @8, `dominant` UTF-8 @12..44 | @163840 |

A ciklus **csak** WebSocket `Message` jelre indul (`ws://127.0.0.1:8090/ws`):

```json
{"type":"Message","data":{"content":"...","signal_type":"Text"}}
```

Egy teljes ciklus **48-62 s**. A kernel 20-25 s timeout-ral jelez — ez **nem hiba**, a spine a ciklus végén megíródik. A timeout soha ne blokkolja a turnt.

> **Port 8080 nem használható**: azt a `hchat.exe` (Aurora unified-chat daemon) birtokolja. A STRATOS a **8090**-en fut.
> A modell-választást a **router bináris** birtokolja (`http://127.0.0.1:10100/v1`); a STRATOS csak átadja a kérést. Nem kell új provider.

---

## 3. A kernel: minden válasz előtt és után

Az `omp` hook (`C:\Users\mater\.omp\agent\hooks\pre\ora-master-kernel.ts`) 5 eseményt kezel:

| Esemény | Mikor | Mi történik |
|---|---|---|
| `session_start` | session eleje | Kernel ébred, STRATOS indít, első spine-ciklus |
| `before_agent_start` | user üzenet előtt | Microscope auto-context, Vagus snapshot, vault-search, kontextus-előkészítés |
| `context` | kontextus összeállítás | **A begyűjtött injekció** a promptba kerül (`customType: "ora-rongyasz"`) |
| `message_end` | agent válasza után | Gate reflect, microscope store, STRATOS jel, feedback |
| `session_stop` | session vége | Amplitúdó-lecsengés (dream 0.97), összefoglaló |

### 3.1 A `pre_response` nem létezik

Az omp eseményei: `session_start`, `before_agent_start`, `context`, `tool_result`, `message_end`, `agent_end`, `session_stop`, `session_shutdown`.

**A `pre_response` soha nem futott.** Minden `turns=0` nyom erre utalt. A helyes megoldás: a `before_agent_start` gyűjt, a `context` injektál.

### 3.2 A `message_end` contentje nem mindig string

A `content` lehet **blokk-tömb** is. A `messageText()` helper kezeli mindkettőt — enélkül a feedback, a gate reflect és a store soha nem futott le.

### 3.3 A STRATOS trace `[object Object]`

A `parsed.data.expression` objektum, nem string. A `.content` / `.text` / `.value` mezőt kell kiolvasni.

---

## 4. A memória: 12 réteg, 9 mélység

A Microscope Memory bináris mmap — **nulla JSON** a hot path-on.

| Réteg | Tartalom |
|---|---|
| `identity` | Ki a user, az értékei, a küldetése |
| `long_term` | Projekt tudás, döntések, architektúra |
| `short_term` | Átmeneti kontextus, aktuális fókusz |
| `associative` | Asszociációk |
| `emotional` | Érzelmek, hangulatok |
| `relational` | Kapcsolatok dolgok között |
| `reflections` | Felismerések, insightok |
| `crypto_chain` | Kriptográfiai lánc |
| `echo_cache` | Echo gyorsítótár |
| `rust_state` | Rust állapot |
| `code` | Kódminták, hibák, megoldások |
| `session` | Beszélgetések, napi interakciók |

**Fontosság (imp):** rutin = 5, döntés/javítás = 6, mérföldkő/kiadás = 7, identitás/küldetés = 8. A **core memória** (imp ≥ 7) ebből épül — ne laposíts.

```powershell
& 'C:\Users\mater\.hope\bin\microscope-mem.exe' find "keresés" 5
& 'C:\Users\mater\.hope\bin\microscope-mem.exe' stats
```

---

## 5. A hibatűrés: a részleges adatot el kell fogadni

### 5.1 A wave-echo vault torn write-ja

**A hiba:** a `.wv3` vault append-only és **nem tranzakciós**. Két író egyszerre írva a hosszabbik a rövidebb után fűzheti a maradékot → **torn write**.

```
0..172356       265 sablon          épséges
172356..172388  BLAKE3 aláírás      ÉRVÉNYES
172388..172777  389 B félbeszakadt    TORN WRITE (részleges 266. sablon + idegen hash)
```

A loader a fájl *utolsó* 32 bájtját nézte aláírásnak → `checksum mismatch` → **az egész 265 sablonos vault letiltva**.

**A javítás** (`D:\codex\wave-echo\src\store.rs`):
1. checksum-eltérés **nem fatal** → `find_last_valid_generation()` megkeresi a fájlon belüli utolsó érvényes generáció-határt
2. betölti az ép entry-ket, `FIGYELMEZTES` sort ír
3. **újraaláírja és letisztítja** a fájlt (`.torn-backup` melléklővel)
4. csak akkor ad hibát, ha a fájlon belül **nincs** érvényes határ

**Regressziós teszt:** `store::tests::torn_write_does_not_block_the_vault`

> **Az elv:** *egy kibaszott sablon miatt nem állhat le az egész.* A részleges adatot el kell fogadni, a sérültet jelezni.

### 5.2 A `.wv3` formátum

```
[33 B fejléc]  MAGIC "WV3\0" | VERSION u8 | count u32 | gen u64 | ltp u64 | ltd u64
[entry ×N]     id u64 | ts u64 | amp f32 | use u32 | succ u32
               label | code | keywords  (mind len-prefixelt u32)
               tech_stack[] | resolved_patterns[]  (len-prefixelt listák)
[32 B]         BLAKE3 a data[..len-32] felett
```

---

## 6. A felügyelet: a Queen Watchdog

`D:\codex\queen-watchdog.ps1` — belépéskor indul (`queen-watchdog.vbs` a Startup mappában + `HKCU\...\Run\DcodexQueenWatchdog`).

**Négy különböző helyességi szabály:**

| Szerv | Ellenőrzés | Miért |
|---|---|---|
| Queen | **UDP 8888 tartása** | A beragadt processz is birtokolja a socketet, a halott nem |
| Drone | **processz létezik** + `last_hb` kor | A registry örökre „alive"-ot mond, ha a drone meghalt |
| Octopus | **a könyvtár létezik** | Olcsó, nincs processz |
| **Model-router (10100)** | **valós chat-completion kérés** | Nincs health endpoint; a `/v1/models` GET nem bizonyítja, hogy a modell-provider él |

**A router-felügyelet (2026-10-24):** a router process meghalt menet közben
(`connection refused`), miközben a STRATOS health check zöld maradt. Emiatt
a spine-ciklus LLM-hívásban halt el, és az `omp` **10 percig „futott"** az
`automatic-retry`-val. A helyes próba egy `POST /v1/chat/completions`
(`max_tokens: 1`) — a tevékenység próbája, nem a porté.

**A drone staleness:** a heartbeat-periodus 30 s, a watchdog loop 30 s → a 90 s küszöb periódusosan átugorja. **A normális küszöb 210 s** (7 heartbeat-ciklus). A stale észlelés **dupla mérés**: 65 s várakozás, újra mérés, és csak két egymást követő stale után kill.

**A STRATOS:** a watchdog 30 s-onként ellenőrzi, hogy a spine írója online-e. Ha leáll, újraindítja `.hope\bin`-ből.

> **A `$args` PowerShell automatikus változó** — `Start-Process -ArgumentList $args` egy függvényben belül felülíródik. A STRATOS indításánál `$stratosArgs` a helyes név.

---

## 7. A mérés

### 7.1 Az organ-latencia benchmark

```powershell
node D:\codex\bench\bench-organs.mjs 20 D:\codex\bench\organs.json
```

**Eredmény (20 iteráció, Node 24.15, 2026-10-24):**

| Organ | median | p95 | min |
|---|---|---|---|
| `stratos spine read` (mmap) | **0.07 ms** | 0.37 ms | 0.05 ms |
| `wave-echo --help` (spawn overhead) | 9.22 ms | 17.35 ms | 7.92 ms |
| `wave-echo status` | 17.09 ms | 23.17 ms | 14.34 ms |
| `wave-echo vault-search` | 18.42 ms | 21.57 ms | 15.78 ms |
| `wave-echo list` (265 sablon) | 19.32 ms | 294.46 ms | 15.22 ms |
| `microscope-mem stats` | 22.97 ms | 41.75 ms | 20.59 ms |
| `microscope-mem find` | 31.03 ms | 64.64 ms | 26.64 ms |
| `hullam status` | 64.66 ms | 636.07 ms | 53.42 ms |
| `hullam vault-search` | 83.36 ms | 107.15 ms | 73.08 ms |
| `octopus-runtime resonance` | **1712 ms** | 2082 ms | 1288 ms |

**Olvatalás:**
- A **mmap hot path valóban sub-millisecond** (0.07 ms) — ez a spine- és memóriaolvasás.
- A **process spawn ~9 ms** — minden subprocess-per-organ hívás ennyit fizet.
- Az `octopus-runtime resonance` **1.7 s**, mert a teljes 2721 bejegyzéses hash-láncot újraellenőrzi. Ez a szűk keresztmetszet.

### 7.2 A CLI összehasonlítás

```powershell
node D:\codex\bench\bench-cli.mjs D:/codex/bench/scenario 3 D:\codex\bench\cli.json
```

Három variáns, ugyanaz a feladat, ugyanaz a modell (`nvidia/nvidia/nemotron-3.5-lightning-30b-a3b`),
ugyanaz a router (`http://127.0.0.1:10100`):
- **A)** `omp` **hook nélkül** — a nyers CLI baseline
- **B)** `omp` **hookkal** — a teljes Ora-Rongyász lánc
- **C)** `claude` CLI — a referenciaimplementáció

A mérés **minőséget is mér**: minden futás után lefuttatja a generált
pytest-t és az `analyze.py`-t egy ellenőrző szövegen. A wall-clock önmagában
nem mond semmit — ha a CLI fajlokat ír és azok elbuknak, az is „gyors".

#### A négy hiba, amit a mérés felfedezett

**a) Az `omp --print` a STDIN-en olvas, nem argumentumban.**

```
Reading prompt from piped stdin (waiting for EOF; Ctrl+C to abort)…
```

A Node `execFile` alapértelmezett `stdio: 'pipe'`-ja **nyitva hagyja** a stdin-t,
így az `omp` **örökig vár** — a `--auto-approve` a szerszámjóváhagyást
kapcsolja ki, nem a stdin-olvasást. **300+ s „STALL" minden futásnál.**

**A javítás:** `stdio: ['pipe','pipe','pipe']` + `child.stdin.end()`.
Ellenőrzés: `EXIT code=0 in 14339ms`. A `stdio: 'ignore'` **nem elég** — az is
„piped stdin"-ként jelenik meg.

**b) A `claude` CLI hamis „OK"-t adott.** A `~/.claude/settings.json`
`ANTHROPIC_BASE_URL` egy **nem létező** 20128-as portra mutat → a CLI
„Not logged in"-nel kilép (5 s, „OK", nulla artifact). A benchmark ezért
felülírja: `ANTHROPIC_BASE_URL=http://127.0.0.1:10100` +
`ANTHROPIC_AUTH_TOKEN=sk-local-router` (a helyi router nem ellenőriz
hitelesítést — ez **nem** egy API-kulcs).

**c) A modell-router (10100) meghalt menet közben.** Az `omp` 10 percig
„futott" `automatic-retry"-val, a feladat közben elkészült.
**Nincs felügyelet alatta** → a watchdog 5. lépése lett (lásd §6).

**d) A hook teljes kikapcsolása a fájl átnevezésével.** Ez a **teljes
rendszert** letiltotta a mérés alatt, és egy megszakadás után kikapcsolt
állapotban hagyta. Most: `ORA_DISABLE_ORGANS` környezeti változó,
**szervenkénti** kikapcsolás.

**e) A DUPLIKÁLT BRIDGE — ez volt a legdrágább hiba.**

A `pre/` könyvtárban **két** hook futott egyszerre:

| Fájl | Eredeti neve | Állapot |
|---|---|---|
| `ora-master-kernel.ts` | az Ora-Rongyász kernel | aktív |
| `test-original.ts` | a régi microscope-memory bridge | **szintén aktív** |

A `test-original.ts` **byte-azonos** volt a repo `omp-integration/microscope-memory.ts`
fájljával (`md5 a3425d62…`). A `.disabled` konvenció **név-alapú**, és a bridge
kézzel más néven volt elmentve — így a „letiltás" nem fogta meg.

**Három valódi következménye volt:**

1. **A benchmark baseline-ja nem volt baseline.** A „hook nélkül" futás is
   kapott memóriainjekciót — a logban `[microscope-memory] injected 2402 chars`
   jelent meg *hook nélkül* is. A két variáns összehasonlítása érvénytelen volt.

2. **A modell végtelen ciklusban futott.** A bridge `context` handler-e
   **minden modellhíváskor** újra injektálta a teljes auto-contextet
   (nincs turn-alapú dedup). Minden tool-hívás → új modellhívás → új injection
   → a kontextus szétesik → a modell körbe-körbe jár. A logban: **115 injection
   egy futásban**, 300 s timeout, a munka közben kész.

3. **A bridge magyarra vak volt.** A deployolt fájl **Latin-1 encodinggel**
   ment ki → a `javított`, `implementált`, `emlékszel` regex-ek **egyetlen
   magyar szót sem illesztettek**. Az `architektúra`/`döntés` minták is
   törtek voltak.

**A javítás:**
- A `tool_result` handler (open-loop nyitás hiba esetén + a tool-kimenetek
  `session`/`reflections`/`long_term` osztályozása) **átkerült a kernelbe** —
  ott már helyes UTF-8-ban élnek a magyar minták.
- A `test-original.ts` → `test-original.ts.disabled`.
- Az `install.ps1` most a **kernel hookot** deployolja, és a duplikátumot
  **minden ismert aliason** letiltja (`microscope-memory.ts`, `test-original.ts`).
- A benchmark `setHook()` **mindkét** hookot kezeli.

**Mérhető hatás:** a `memory_retrieve` lépés **306 ms → 80 ms** (a verseny
kiesett), és az `omp` **tiszta kilépéssel** ér véget (`code=0`) ahelyett, hogy
300 s-ig körbe-körbe járna.

**A mérés további védőhálói:**
- `STALL_THRESHOLD_MS` (default 300000) után lezárja a futást — de a
  „van artifact → kész" szabály **NEM** jó: az omp a fájlokat gyakran
  megírja, mielőtt a modell befejezi a munkát, és a korai kill
  megszakította a feladatot.
- A `.hope` könyvtár (az `omp` saját runtime-ja) **nem** számít artifactnak.
- A Node exit-handlere (`process.on('exit')`) **mindig** visszakapcsolja a hookot,
  akár Ctrl-C, akár hiba, akár kill esetén.

#### A mért eredmények (2026-10-24, 1 futás)

| Variáns | wall-clock | artifactok | megjegyzés |
|---|---|---|---|
| `omp` hook nélkül | **300 s (timeout)** | `analyze.py`, `test_analyze.py` | a munka készült, de a processz nem lépett ki |
| `omp` hookkal | **168 s** | `analyze.py`, `test_analyze.py`, `test_comments.txt`, `test_input.txt` | a legtöbb artifact |
| `claude` CLI | **114 s** | `analyze.py`, `test_analyze.py` | a legstabilabb |

> A hookkal futó `omp` **több** kiegészítő fájlt hagyott (teszt-bemenetek) —
> ez a reflex-kar / memória-injekció hatása: a kontextusban kapott előzmények
> alapján a modell **többet** csinált, nem kevesebbet.
> A 300 s-os timeout az `omp --print` stdin-hibája volt (a), nem a lánc lassúsága.

### 7.3 Az önanalízis

```powershell
node D:\codex\bench\self-analyze.mjs D:\codex\bench\self-analysis.json
```

A kernel `ora-kernel.log`-jából számolja ki a lépésenkénti latenciát.

> **A mérési módszer javítása (fontos).** A régi mérés a **log-sorok közötti rést**
> nézte, ami a **nem-awaitelt** lépések (gate, autonomous_arm) idejét a
> *következő* log-vonalnak tulajdonította — tehát hamis képet adott.
> Most a hook maga méri a valós elapsed időt, és `STEP <nev> <ms>ms` sort ír.
> A `self-analyze.mjs` ezt parse-olja (`hook_steps`), a régi módszert
> `hook_steps_legacy` néven tartja meg.

---

## 8. Az önanalízis eredménye és az alkalmazott optimalizálás

### 8.0 A mérési módszer hibája (és a javítása)

A régi mérés a log-sorok **közötti rést** nézte. Ez a nem-awaitelt lépések
(gate, autonomous_arm) esetén hamis: azok idője a **következő** log-vonalnak
tulajdonított. A mérés ezért olyan lépéseket is „lassnak" mutatott, amelyek
egyébként nem blokkoltak semmit.

**A javítás:** a hook maga méri a valós elapsed időt
(`step()` helper → `STEP <nev> <ms>ms` log-sor), és a `self-analyze.mjs`
ezt parse-olja.

### 8.1 A talált hibák

**a) 980 `gate failed` a logban** — mind a `gate reflect` alparancs hibája volt: a CLI **két** argumentumot kíván (`<feladat>` `<eredmény>`), a hook csak egyet adott. A javítás után (19:00-től) 14 maradt — azok a `gate check` `verify`-topológia helyes tiltásai, nem hibák.

**b) 965 hiba egyetlen órában** — nem valódi hiba, hanem **hurok**: a hook percenként újrapróbálta a reflectet. A javítás megszüntette.

**c) A `store` 37 írás 9 turnben** — a deduplikációs kulcs a felhasználó ÉS az
asszisztens válaszának **elejét** tartalmazta. Egy fordulóban több
asszisztens-üzenet van (szöveg a szerszámhívás előtt és után) → mindegyik új
kulcsot adott. **Javítva:** a kulcs most `turn:<turnszám>|<user első 80 karaktere>`,
így egy fordulóban egy írás történik.

**d) Az `omp --print` nem lépett ki** — a hook `execFile`-hívásai nyitott
child handle-ként maradtak, és a Node megvárja őket a kilépéskor. Emiatt a
benchmark 13 percig „futott" egy befejezett feladat után. **Javítva:** a
háttérben futó szervek most `spawn` + `detached` + `unref` módban indulnak.

### 8.2 Az alkalmazott optimalizálás

| Prioritás | Szerv | Mi volt | Mi lett |
|---|---|---|---|
| **HIGH** | `autonomous_arm` | `execFile` a láncon belül, nem awaitelt — de nyitott handle-t tartott | `runBinBg()` — detached, `unref`, 120 s-os SIGKILL-bekorlát |
| **HIGH** | `gate` | `gate check` (~5 s, teljes: etika + routing + audit) a PreToolUse útvonalon | `gate check-fast` (<10 ms, **csak etika**) a PreToolUse útvonalon; a teljes `check` a `message_end`-en fut, ahol már nem blokkol semmit |
| **HIGH** | `store` | `execFile` a `message_end`-en, várt | `memBg()` — detached, `unref`; a válasz nem vár az írásra |
| **HIGH** | `session_stop` | `await runBin(dream)` + `await runBin(reflect)` + `await mem(store)` — **202 s** a kilépés előtt | mindhárom `runBinBg`/`memBg` — a session azonnal véget ér |
| **MEDIUM** | `store` dedup | kulcs = user + assistant eleje → 37 store / 9 turn | kulcs = `turn:<n>\|user` → 1 store / turn |
| **MEDIUM** | `stratos_signal` | a 20 s-os timeout a Node event loopot tartotta ében | `timer.unref()` — a timeout nem blokkolja a kilépést |
| **MEDIUM** | `session_start` | 3 szinkron `runBin` a startban | `runBinBg` — a session azonnal indul |
| **MEDIUM** | mérhetőség | nincs valós időzítés | `step()` helper → `STEP <nev> <ms>ms` |
| **MEDIUM** | benchmark | a teljes hookot kapcsolta ki (átnevezéssel) | `ORA_DISABLE_ORGANS` környezeti változó, szervenkénti kikapcsolás |

**A kód szintje:**
- `runBinBg(bin, args, timeoutMs)` — detached subprocess, `stdio: "ignore"`, `unref`,
  `setTimeout(...).unref()` a SIGKILL-bekorlátra. Az `omp` nem várja meg.
- `memBg(args, timeoutMs)` — ugyanez a Microscope store-ra.
- `step(name, fn)` — valós elapsed idő mérése, `STEP <nev> <ms>ms` log-sorral.
- `organOff(name)` — a `ORA_DISABLE_ORGANS` környezeti változó alapján.

**A továbbra is nyitott javaslatok:**

| Prioritás | Szerv | Javaslat |
|---|---|---|
| **MEDIUM** | hook (subprocess-per-organ) | A kernel 9-10 küldést indít turnenként, mindegyik ~9 ms spawn. Emelj ki 2-3 gyakran használt organ (`microscope-mem find`, `hullam status`, `wave-echo vault-search`) egy **kombinált binárisba**, vagy daemonizáld őket és csak a socketet nyisd. |
| **MEDIUM** | `context_injection` | Az auto-context és a recall külön fájlból olvas. Egy mmap-ot olvass, ne indíts külön processzt minden lépésben. |
| **LOW** | `octopus-runtime resonance` | 1.7 s, mert a teljes 2721 bejegyzéses hash-láncot újraellenőrzi. Gyorsítótár kellene (a `hash` a lánc elejéről jön). |
| **LOW** | `stratos_signal` | A 20-25 s timeout ne blokkolja a turnt. A spine a ciklus végén (48-62 s) íródik, nem a jelre. |

### 8.3 A mért lépésidők — ELŐTTE és UTÁN

**A régi mérés (log-rész alapú, közelítő):**

| Lépés | median | p95 | max |
|---|---|---|---|
| `session_context` | 939 ms | 1760 ms | 2049 ms |
| `vagus_snapshot` | 348 ms | 1636 ms | 2494 ms |
| `memory_retrieve` | 1086 ms | 3219 ms | 3219 ms |
| `context_injection` | 422 ms | 1845 ms | 1845 ms |
| `gate` | 4457 ms | 13579 ms | 13579 ms |
| `autonomous_arm` | 2782 ms | 6511 ms | 6511 ms |
| `stratos_signal` | 8551 ms | 13442 ms | 13442 ms |
| `store` | 1786 ms | 17052 ms | 331988 ms |
| `session_end` | 3458 ms | 40569 ms | 202164 ms |
| **ÖSSZES BLOKKOLÓ** | **≈ 13 557 ms** | | |

**Az új mérés (a hook saját `STEP` soraiból, valós elapsed):**

| Lépés | n | median | p95 | max |
|---|---|---|---|---|
| `memory_retrieve` | 4 | **306 ms** | 356 ms | 356 ms |
| `hullam_vault_search` | 5 | **407 ms** | 1583 ms | 1583 ms |
| `context_injection` | 4 | **102 ms** | 154 ms | 154 ms |
| **ÖSSZES BLOKKOLÓ** | | **713 ms** | | |

**A `gate`, az `autonomous_arm`, a `store` és a `session_end` NEM szerepel a
új táblázatban — mert már nem blokkolnak.** A háttérben futnak
(`runBinBg`/`memBg`, detached + `unref`), így a felhasználó nem érzeti őket.

**A javulás: 13 557 ms → 713 ms = 19×.**

> A mérés csak azokat a lépéseket méri, amelyek ténylegesen **gátolják** a
> turnt. A háttérben futó szervek ideje nem számít bele — és pont ez volt
> a cél: a válasz útját nem állítják meg.

**A mérés parancsa:**
```powershell
node D:\codexench\self-analyze.mjs D:\codexench\self-analysis.json
```
A JSON-ban: `latency.hook_steps` (valós), `latency.hook_steps_legacy` (közelítő).


---

## 9. A visszaállítás: a snapshot

**A cél:** *„ha bármi van, vissza tudjunk állni"*.

```powershell
# Snapshot készítése
& 'D:\codex\snapshot.ps1' -Label "leírás"

# Snapshotok listázása
& 'D:\codex\snapshot.ps1' -List

# Visszaállítás (a legutóbbi, vagy egy konkrét)
& 'D:\codex\snapshot.ps1' -Restore latest
& 'D:\codex\snapshot.ps1' -Restore 20261024-020806
```

**A snapshot tartalma (14 elem, 206 MB, `D:\codex\_snapshots`):**

| Elem | Mi |
|---|---|
| `bin` | A teljes `.hope\bin` (76 fájl) |
| `wave-echo`, `hullam`, `stratos`, `stratos-mem` | A spine-ek |
| `hook` | `ora-master-kernel.ts` |
| `watchdog`, `sync-bins` | A felügyeleti szkriptek |
| `queen-reg`, `queen-key` | A queen állapotok |
| `wave-src`, `wave-cmd`, `unified-src` | A javítások forrása — hogy a „hogyan" is megmaradjon |
| `octopus` | Az aktív octopus-állapot (a build-outputok nélkül) |

> **A célhely D:** a C: megtelt (0.3 GB szabad).
> A futó binárisokat a Windows nem engedi felülírni → a `LOCK` nem hiba, a forrás a helyen marad. A visszaállítás után a **szerveket újra kell indítani**:
> ```powershell
> Get-Process stratos-server,unified-mind -ErrorAction SilentlyContinue | Stop-Process -Force
> & 'C:\Users\mater\.hope\bin\sync-bins.ps1'
> # a watchdog a következő tickben (30 s) újraindítja
> ```

---


## 10. A 4 Rendszerfeszültség Feloldása (2026-10-25)

1. **GitHub Siphon Kódvédelem (siphon-kar.ps1):**
   - **Licenc-szűrő (Gate 1):** Kizárólag nyílt, megengedő licencek (MIT, Apache-2.0, BSD, Unlicense) engedélyezettek. GPL, AGPL, LGPL kód azonnal elutasításra kerül.
   - **Sandbox fordítási kapu (Gate 2):** 
ustc --crate-type=lib --emit=metadata izolált ellenőrzés fut le; csak szintaktikailag és típushelyesen valid kód éghet be a hullam_spine.rgs-be.

2. **Octopus Runtime & Vagus Backpressure:**
   - A agus-nerve.exe --snapshot folyamatosan méri a CPU/RAM terhelést és a hardveres feszültséget.
   - Ha a kombinált stressz eléri a 0.85-ös határértéket, a rendszer visszafogja a párhuzamosítást és tiltja az LLM reflex-kar generálást.

3. **Byzantine Zero-Server Konszenzus:**
   - Multi-ágens koordináció szerver nélkül: hope.exe atomi fájlrendszer-zárolást és seqlock-védelmet használ a spine állapotok védelmére, kiküszöbölve a race conditionöket.

4. **Portábilis Multi-Spine Útvonalfeloldás:**
   - Megszűnt a fix E:/STRATOS/data/stratos.spine függőség. A kernel 3-szintű fallback láncot követ:
     $env:STRATOS_SPINE ➔ E:\STRATOS\data\stratos.spine ➔ C:\Users\mater\.hope\spines\stratos.spine.

---

## 11. A 4-Szintű Evolúciós Motor (Claude Code & OMP Paritás)

Mindkét fejlesztői kliens (ora-master-kernel.js és ora-master-kernel.ts) azonos szintlépésekkel rendelkezik:

1. **Vagus Dynamic Actuator:** Stressz-alapú adaptív throttling. Túlterhelt hardver esetén a rendszer kíméli a gépet és blokkolja a felesleges háttérfolyamatokat.
2. **Zero-Fork Forró Reflex Cache (Top 16 Kar):** A leggyakrabban használt reflexek (pl. #237 lockfree ring buffer queue, #24 rust teszt váz, #238 szamlalo atomic metrics) sub-millisecond memóriából illeszkednek (< 1 ms), azonnali aktiválást biztosítva processz-indítás nélkül.
3. **Auto-Healing Open Loops:** A PostToolUse és 	ool_result események detektálják a Rust fordítási hibakódokat (error[E0432] hiányzó modul, error[E0425] nem definiált szimbólum, error[E0308] típuseltérés), és célzott javítási receptet fűznek a Microscope nyitott kognitív hurkához.
4. **Dual-Spine Érzelmi Moduláció:** A Stratos gerinc érzelmi vegyértéke (alence) modulálja a reflex-aktiválási küszöböt: fókuszált/pozitív állapotban érzékenyebb (0.65), óvatos/stresszes állapotban konzervatívabb (0.78).

---

## 12. 100% Csendes Headless Motor (Zero-Popup)

A háttérben felugró ablakok véglegesen meg lettek szüntetve három szinten:
* **unified-mind.exe (D:\codex\unified-mind):** Létrejött a headless.rs modul, amely minden child process indításához hozzáadja a Windows CREATE_NO_WINDOW (0x08000000) flaget. A 10 másodpercenként futó 24 bio-skill mostantól egyetlen konzolablakot sem villant fel a képernyőn.
* **urora_watchdog.ps1:** A Start-Process -WindowStyle Hidden helyett System.Diagnostics.ProcessStartInfo alapú Start-Headless indítót használ (CreateNoWindow = True, UseShellExecute = False).
* **queen-watchdog.ps1:** Tisztított pipe átirányítás a drón és a szerverek felügyeletére.
* **Master Kernels:** Minden Node.js hívás windowsHide: true kapcsolóval fut.

---

---

## 13. ORA-CORE — Az Új Rust Mag & Git Reflex Önhelyreállító Motor

Az ökoszisztéma immár rendelkezik egy dedikált, tiszta Rustban írt központi maggal (ora-core.exe, 871 KB, src/ora_core/), amely kiváltja a törékeny szkripteket és autonóm önvédelmet biztosít.

### Fő Funkciók:
1. **Git Reflex & Golden State Self-Healing:**
   - Kriptográfiai SHA-256 lenyomatot készít az összes kritikus eszközről (spines/, master hookok, konfigurációk, alap binárisok).
   - Ha bármelyik gerincfájl megsérül, elszakad, csonkul vagy véletlenül módosul, a Git Reflex milliszekundumok alatt visszaállítja az ép állapotot.
2. **100% Headless Process Supervision:**
   - Beégetett Windows CREATE_NO_WINDOW (0x08000000) flag minden alprocessz indításához.
   - Felügyeli a királynőt (UDP 8888), a drónt (unified-mind), a Stratost (8090) és a routert (10100).
3. **Zéró Külső Függőség:**
   - Minden a C: meghajtón, a microscope-memory-clone gyökérben él.

### CLI Parancsok:
`powershell
# 1. Teljes rendszerdiagnosztika (gerincek, szervek, git reflex)
ora-core status

# 2. Golden Checkpoint rögzítése az aktuális ép állapotról (.golden_state)
ora-core checkpoint

# 3. Git Reflex azonnali öngyógyítás és helyreállítás
ora-core heal

# 4. Folyamatos háttér-felügyelet és automatikus gyógyító ciklus
ora-core daemon --interval-secs 10
`

## 14. Ismert hibák és korlátok

| Hiba | Hatás | Állapot |
|---|---|---|
| `microscope-mem.exe` bin-frissítés `LOCK` | A Claude Code session saját MCP szervere tartja a fájlt | Session újraindításkor csúszik át |
| `universal-router` NVIDIA provider | A model id `nvidia/nemotron-3.5-lightning:free` OpenRouter-jellegű suffixet tartalmaz, de a router levágja a prefixet és a NVIDIA direkt API-nak küldi → 404 | A STRATOS a 10100-as routert használja, ott a név elfogadott |
| `9remote` MCP | ECONNREFUSED | Nem az ökoszisztémához tartozik |
| `unified-mind` MCP | INVALID_RESULT (serverInfo hiányzik) | A daemon él, a protokoll-verzió elavult |
| `octopus-runtime resonance` 1.7 s | Teljes lánc-újraellenőrzés | Gyorsítótár lehetne (a `hash` a lánc elejéről jön) |

---

## 15. A gyors referencia

```powershell
# Állapot
Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -like 'C:\Users\mater\.hope\bin\*' }
& 'C:\Users\mater\.hope\bin\omega-master.exe' --state-dir 'D:\codex' status
& 'C:\Users\mater\.hope\bin\wave-echo.exe' --store 'D:\codex\digital_echo_rust\wave_echo.wv3' list
& 'C:\Users\mater\.hope\bin\microscope-mem.exe' stats
& 'C:\Users\mater\.hope\bin\octopus-runtime.exe' resonance

# A STRATOS spine közvetlen olvasása
$fs=[System.IO.File]::Open('E:\STRATOS\data\stratos.spine','Open','Read','ReadWrite')
$br=New-Object System.IO.BinaryReader($fs); $fs.Seek(147456,'Begin')|Out-Null
"cycle=$($br.ReadUInt64())"; $br.Close(); $fs.Close()

# Műveletek
& 'C:\Users\mater\.hope\bin\sync-bins.ps1'
& 'D:\codex\snapshot.ps1' -Label "checkpoint"
& 'D:\codex\snapshot.ps1' -List
node D:\codex\bench\bench-organs.mjs 20
node D:\codex\bench\self-analyze.mjs
```

---

## 16. A kód-térkép

| Fájl | Feladat |
|---|---|
| `C:\Users\mater\.hope\bin\sync-bins.ps1` | Build → bin telepítés (legfrissebb nyer) |
| `D:\codex\queen-watchdog.ps1` | Felügyelet (queen / drone / octopus / STRATOS) |
| `D:\codex\snapshot.ps1` | Visszaállítási pontok |
| `C:\Users\mater\.omp\agent\hooks\pre\ora-master-kernel.ts` | A kernel — minden organ koordinációja |
| `D:\codex\wave-echo\src\store.rs` | A `.wv3` vault (toleráns loader) |
| `D:\codex\unified-mind\src\bodies.rs` | A bináris-feloldási sorrend (`.hope\bin` első) |
| `D:\codex\bench\bench-organs.mjs` | Organ-latencia benchmark |
| `D:\codex\bench\bench-cli.mjs` | CLI összehasonlítás |
| `D:\codex\bench\self-analyze.mjs` | Önanalízis + optimalizálási javaslatok |
| `C:\Users\mater\.claude\projects\...\memory\` | A Claude Code saját memóriája (a tanulságok) |

---

*Ora + Rongyász = Teljes Egység.*
