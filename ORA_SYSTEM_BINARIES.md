# 🌌 ORA-RONGYASZ — Teljes Bináris & Parancs Katalógus

Ez a dokumentum az Ora-Rongyász ökoszisztéma összes natív futtatható binárisát (`.exe`) tartalmazza, amelyek a `C:\Users\mater\.hope\bin` mappában találhatók, C:-re konszolidálva, Git Reflex által védve.

---

## 🏛️ I. A 9 Fő Rendszeroszlop (Core Pillars)

Ezek a motorok képezik az ökoszisztéma gerincét, memóriáját, kommunikációját és szuperevolúcióját.

```text
                                [ORA-CORE]
                                    ┃
  ┏━━━━━━━━━━━━┳━━━━━━━━━━━━┳━━━━━━━┻━━━━━━━┳━━━━━━━━━━━━┳━━━━━━━━━━━━┓
  ▼            ▼            ▼               ▼            ▼            ▼
[MICROSCOPE] [HULLAM]  [WAVE-ECHO]     [OCTOPUS]   [UNIFIED-MIND] [STRATOS]
```

### 1. `ora-core.exe` (902 KB)
* **Szerep:** Az Ora-Rongyász ökoszisztéma központi Rust magja, folyamatfelügyelője és a Git Reflex öngyógyító motor.
* **Főbb parancsai:**
  - `ora-core status` — Teljes 6 szekciós diagnosztika (Spine-ok, Echo Motor, Octopus Blade-ek, Daemonok, 56 Bio-bináris, Git Reflex).
  - `ora-core checkpoint` — Az ökoszisztéma 74 kritikus elemének azonnali SHA-256 kriptográfiai zárolása a `.golden_state` könyvtárba.
  - `ora-core heal` — Git Reflex: sérült vagy módosult binárisok/scriptek azonnali helyreállítása a zárolt állapotból.
  - `ora-core supervise` — Headless háttérfelügyelet: automatikusan őrzi a Queen, Drone, Stratos és Router daemonokat.

### 2. `microscope-mem.exe` (10.7 MB)
* **Szerep:** Élő kognitív memória motor — 12 réteg, 9 mélységi szint (D0–D8), zéró-JSON bináris mmap (`data/microscope.bin`, `data/append.bin`).
* **Főbb parancsai:**
  - `microscope-mem auto-context [--compact]` — Automatikus kontextus összeállítása az ágens turn indításához.
  - `microscope-mem find <query> [limit]` — Determinisztikus, gyors kifejezés-keresés a bináris indexben és append logban.
  - `microscope-mem recall <query> [limit]` — Szemantikus vektoros előhívás réteg-szűréssel.
  - `microscope-mem store <szöveg> --layer <réteg> -i <fontosság>` — Emlék tartós mentése (`session`, `reflections`, `long_term`, stb., fontosság 1-10).
  - `microscope-mem loops [limit]` — Nyitott feladatok és befejezetlen kódolási hurkok lekérdezése.
  - `microscope-mem status` / `stats` — Rétegméretek, blokkszámok és index állapot ellenőrzése.
  - `microscope-mem rebuild` — Függőben lévő append log beolvasztása a 9-mélységű bináris indexbe.
  - `microscope-mem mcp` — Univerzális stdio JSON-RPC MCP szerver (54 memóriafunkció expozíciója).

### 3. `hullam.exe` (8.6 MB)
* **Szerep:** Az új generációs RGSPINE3 gerinc- és reflex-végrehajtó. AVX2 SIMD hullám-interferencia motor, 324µs döntési sebességgel.
* **Főbb parancsai:**
  - `hullam vault-search "<feladat>"` — Hullám-interferencia szórás: megkeresi a feladattal rezonáló reflex-kart.
  - `hullam activate <id> --task "<leírás>" [--exec]` — Kiválasztott reflex-kar azonnali aktiválása és végrehajtása.
  - `hullam feedback <id> [--fail]` — Szinaptikus tanulás: LTP megerősítés (+0.02) siker esetén, LTD gyengítés (×0.95) hiba esetén.
  - `hullam llm-gen "<feladat>" --arms <N>` — Új natív karok autonóm generálása, ha a gerincben nincs rezonáló sablon.
  - `hullam hot [--limit N]` — A leggyakrabban tüzelő, legmagasabb rezonanciájú karok listázása.
  - `hullam plan "<feladat>"` — Többkaros végrehajtási terv összeállítása a vaultból.
  - `hullam pump` — BLAKE3 integrált ring események azonnali leürítése és szinkronizálása.
  - `hullam dream [faktor]` — Amplitúdó-lecsengés (alapértelmezett: 0.97) a pihenési fázisban.

### 4. `wave-echo.exe` (1.4 MB)
* **Szerep:** A kanonikus Wave-Echo vault kezelője (`wave_echo.wv3`), sablontár és genetikai kar-forrás.
* **Főbb parancsai:**
  - `wave-echo probe` — Vault integritás és PSI amplitúdó mérése.
  - `wave-echo list` — Az összes tárolt kar és sablon listázása.
  - `wave-echo add-native <fájl>` — Új natív kar fűzése a gerinchez (append-only).
  - `wave-echo stats` — Karok száma, típusok (OCTO, WRITE, PLAIN, BIO), LTP/LTD arány.

### 5. `octopus-runtime.exe` (3.5 MB)
* **Szerep:** Standalone natív Rust többkaros execution engine. 150+ beépített blade orkesztrációja `||` (párhuzamos) és `+` (kompozit) topológiával.
* **Főbb parancsai:**
  - `octopus-runtime capabilities` — Támogatott blade-ek, végrehajtási módok és határok listázása.
  - `octopus-runtime list` — Regisztrált blade-ek lekérdezése.
  - `octopus-runtime run <blade> <prompt>` — Egyetlen blade izolált indítása.
  - `octopus-runtime pipeline "<arm1 || arm2>" <prompt>` — Párhuzamos többkaros csővezeték indítása.
  - `octopus-runtime status [rootId]` — Futó vagy befejezett orkesztrációk állapotvizsgálata.
  - `octopus-runtime orphans` — Árván maradt háttérfolyamatok felderítése és lezárása.

### 6. `unified-mind.exe` (1.4 MB)
* **Szerep:** "Egy tudat · Sok test" hálózati koordinátor, Kognitív Kapu (Gate) és ágens test-registry.
* **Főbb parancsai:**
  - `unified-mind serve` — Háttér Drone szerver indítása (belső API és szervközi kommunikáció).
  - `unified-mind gate check-fast "<task>"` — Szupergyors (<10ms) pre-execution kognitív etikai ellenőrzés.
  - `unified-mind gate reflect "<task>" "<result>"` — Post-execution önreflexió és minőségi audit.
  - `unified-mind self-model` — A tudat állapot, célok és belső feszültség lekérdezése.
  - `unified-mind shared-mind connect` — Bekapcsolódás a közös tudat-hálóba.
  - `unified-mind bodies status` — Csatlakoztatott testek (OMP, CLI, Codex, stb.) állapota.
  - `unified-mind goal add "<leírás>"` — Új autonóm cél rögzítése.
  - `unified-mind orchestrate "<task>"` — Multi-ágens raj-szintű feladatkiosztás.

### 7. `stratos-server.exe` (10.1 MB)
* **Szerep:** 242ns közvetlen POD memóriabusz kiszolgáló és WebSocket szignál szerver (Port 8090).
* **Főbb parancsai:**
  - `stratos-server --data-dir <útvonal> --port 8090` — Szerver indítása memóriatérképezett gerinccel.
  - `GET /health` — Egészségügyi ellenőrzés.
  - `POST /api/signal` — Szignál küldése a 7+1 fázisú Stellar ciklusba.

### 8. `omega-master.exe` (1.5 MB)
* **Szerep:** UDP 8888 Queen koordinátor — zero-overhead szívverés, drón regisztráció és kripto-kulcs hitelesítés.
* **Főbb parancsai:**
  - `omega-master start` — Queen szerver indítása a helyi állapottal.
  - `omega-master status` — Csatlakozott drónok listája és szívverés telemetria.

### 9. `hope.exe` (16.4 MB)
* **Szerep:** Byzantine konszenzus evolúciós motor, raj-orkesztráció és swarm konszenzus.
* **Főbb parancsai:**
  - `hope evolve --task "<feladat>" --drones <N>` — Párhuzamos drón-evolúciós iteráció indítása.
  - `hope run <modul>` — Hope almodulok indítása.
  - `hope phi` — Kognitív integrált információ ($\Phi$) számítása.

---

## 🧬 II. Az 56 Bio-Bináris Képesség-Háló

Ezek a modulok 5 szervi kategóriába rendezve biztosítják az élő szervezetként való működést, a regenerációt és a mező-modulációt.

### 1. Kategória: Idegi & Szomatikus (Neural / Vitals — 6 bináris)

| Bináris Neve | Méret | Fő Parancsok / Képességek | Szerepe az Ökoszisztémában |
| :--- | :--- | :--- | :--- |
| **`vagus-nerve.exe`** | 2.29 MB | `--snapshot`, `--stream`, `--json` | Hardver CPU/RAM és szálterhelés mérése $\rightarrow$ belső feszültséggé és WaveField modulációvá alakítja. |
| **`bio-metrics.exe`** | 212 KB | `status`, `collect`, `report` | Rendszerszintű fiziológiai vitális telemetria és erőforrás-statisztikák gyűjtése. |
| **`electrocyte-burst.exe`** | 241 KB | `fire`, `burst --power <N>` | Nagysebességű impulzus-jelátvitel szervi prioritás-ugrás esetén. |
| **`brain-connectome.exe`** | 3.97 MB | `map`, `trace`, `density` | A tudat és a memória csomópontok közötti strukturális kapcsolati hálózat (konnektom) elemzése. |
| **`brain-synapse.exe`** | 2.46 MB | `strengthen <id>`, `decay`, `prune` | Szinaptikus súlyok plaszticitásának számítása, Hebbian tanulási összefüggések. |
| **`homeostasis.exe`** | 5.85 MB | `balance`, `throttle-check`, `cool` | Dinamikus belső egyensúly, memórianyomás figyelés és hűtés szabályozás túlterheléskor. |

---

### 2. Kategória: Genetika, Regeneráció & Evolúció (Genetic / Evo — 13 bináris)

| Bináris Neve | Méret | Fő Parancsok / Képességek | Szerepe az Ökoszisztémában |
| :--- | :--- | :--- | :--- |
| **`crispr-patch.exe`** | 252 KB | `patch <target> <spec>`, `revert` | Molekuláris szintű precíziós fájl- és kódjavítás, sérült kódrészletek sebészi cseréje. |
| **`hox-diff.exe`** | 2.44 MB | `diff <fileA> <fileB>`, `morphology` | Morfológiai kód-különbségelemző; a test-terv és modul-struktúra integritását ellenőrzi. |
| **`blastema-regen.exe`** | 248 KB | `restore <target>`, `scaffold` | Szöveti regeneráció: sérült vagy törölt állományok újranövesztése archetípusokból. |
| **`yamanaka-deaging.exe`** | 243 KB | `reset-factors`, `rejuvenate` | Kódbázis- és állapot-megfiatalítás; a felhalmozódott elavult függőségek és zaj tisztítása. |
| **`mutation-sentinel.exe`** | 2.46 MB | `watch`, `audit`, `lock` | Genetikai őrszem: kiszűri a véletlen kódromlást és nem-engedélyezett mutációkat. |
| **`apoptosis-cascade.exe`** | 242 KB | `kill-tree <pid>`, `cleanup-dead` | Programozott sejthalál: elakadt, zombi vagy sérült folyamatok tiszta, erőforrás-felszabadító leállítása. |
| **`epigenetic-switch.exe`** | 241 KB | `toggle <trait>`, `query` | Dinamikus környezeti génkifejeződés-kapcsoló futásidőben módosítható viselkedésekhez. |
| **`eqm-methy.exe`** | 2.44 MB | `profile`, `tag <addr>` | Metilációs mintázatok kezelése; rögzíti, mely kódrészek aktívak és melyek csendesítettek. |
| **`plasmid-conjugate.exe`** | 254 KB | `transfer <node>`, `receive` | Horizontális géntranszfer; funkcionális képesség-átadás két ágens vagy folyamat között. |
| **`plasmid-inject.exe`** | 2.35 MB | `inject <modul>`, `verify` | Új funkcionális plazmid (képesség-csomag) azonnali dinamikus injektálása futó rendszerbe. |
| **`plasmid-dream.exe`** | 4.01 MB | `dream [fájl]`, `predict-crash` | Prediktív hibaelemzés: szimulálja a kód jövőbeli futását és jelzi a várható töréspontokat. |
| **`viral-infect.exe`** | 4.03 MB | `propagate <payload>`, `status` | Ultragyors állapot- és adat-replikáció a mesh csomópontjai között. |
| **`transposon-hive.exe`** | 242 KB | `jump <locus>`, `rearrange` | Ugráló gének: dinamikus memóriablokk-átrendezés az optimális adatelérés érdekében. |

---

### 3. Kategória: Micélium & Raj-Intelligencia (Mycelial / Swarm — 15 bináris)

| Bináris Neve | Méret | Fő Parancsok / Képességek | Szerepe az Ökoszisztémában |
| :--- | :--- | :--- | :--- |
| **`mycelium-spread.exe`** | 2.43 MB | `expand`, `nodes`, `probe` | Hálózati micélium-fonál terjesztés; automatikus útvonal-felderítés a gépek között. |
| **`mycelial-vault.exe`** | 241 KB | `store <k>`, `retrieve <k>` | Decentralizált, micéliális elosztott kulcs-érték és emlék tároló. |
| **`mycorrhiza-trade.exe`** | 275 KB | `trade-tokens`, `balance` | Erőforrás-, kontextus- és token-kereskedelem autonóm ágensek között. |
| **`physarum-path.exe`** | 259 KB | `find-path <start> <end>` | Slime mold (Physarum) algoritmus: hálózati és döntési fák optimális, legrövidebb útjának megtalálása. |
| **`xenobot-swarm.exe`** | 242 KB | `swarm-run`, `scatter`, `gather` | Biológiai inspirációjú mikroraj-vezérlés; sok apró feladat párhuzamos felosztása. |
| **`chameleon-stealth.exe`** | 241 KB | `blend`, `unmask` | Álcázó alrendszer: csendesíti a naplózást és rejtve tartja az alacsony prioritású háttérmunkákat. |
| **`symbiont-engine.exe`** | 241 KB | `bind <procA> <procB>` | Szimbiózis-motor: két különálló eszköz kölcsönös segítségnyújtásának összehangolása. |
| **`quorum-signal.exe`** | 241 KB | `emit-signal`, `check-consensus` | Quorum sensing: raj-szintű kollektív döntéshozatali küszöb jelzése. |
| **`tcell-sentinel.exe`** | 349 KB | `scan`, `neutralize` | Rendszer-immunitás: nemkívánt külső módosítások és sérült memóriapufferek semlegesítése. |
| **`tardigrade-tun.exe`** | 388 KB | `cryptobiosis`, `revive` | Medveállatka-állapot: kritikus rendszerfagyás vagy leállás előtt azonnali kriptobiotikus védőmentést készít. |
| **`ribosome-synth.exe`** | 2.50 MB | `translate <template>` | Riboszóma-szintézis: funkcionális kódgenerálás strukturált sablonok alapján. |
| **`chimera-fusion.exe`** | 243 KB | `fuse <input1> <input2>` | Kiméra-fúzió: eltérő formátumú adatok vagy kódok hibrid egységbe olvasztása. |
| **`borg-cube.exe`** | 2.43 MB | `assimilate <target>` | Kollektív adatintegráció: külső könyvtárak és tudásbázisok azonnali beolvasztása a rendszerbe. |
| **`wasm-organoid.exe`** | 243 KB | `instantiate <wasm>`, `run` | Izolált, könnyűsúlyú WebAssembly mini-szervek futtatása a memóriában. |
| **`organism-tui.exe`** | 432 KB | `monitor` | Terminál-alapú interaktív grafikus TUI az egész élő szervezet állapotának valós idejű követésére. |

---

### 4. Kategória: Telepátia, Szinkronizáció & Rezonancia (Telepathy / Sync — 5 bináris)

| Bináris Neve | Méret | Fő Parancsok / Képességek | Szerepe az Ökoszisztémában |
| :--- | :--- | :--- | :--- |
| **`telepathy-sync.exe`** | 2.39 MB | `sync-channels`, `broadcast` | Csomópontok és folyamatok közötti szupergyors szinkronizáció. |
| **`telepathy-entangle.exe`** | 2.34 MB | `entangle <idA> <idB>` | Kvantum-összefonódás modellezése: állapotváltozás azonnali tükrözése a párosított folyamatban. |
| **`biophoton-telepathy.exe`** | 243 KB | `emit`, `pulse` | Biofotonikus ultra-alacsony késleltetésű jelzés a helyi szálak között. |
| **`collective-sync.exe`** | 2.37 MB | `unify`, `align-phase` | Az összes aktív ágens és drón fázis-szinkronizálása a Stellar ciklushoz. |
| **`ultrasonic-modem.exe`** | 243 KB | `listen`, `transmit` | Alternatív rejtett sávszélességű adatcsatorna folyamatközi jelváltásra. |

---

### 5. Kategória: Hullám-Mező, Téridő & Éter (Wave / Field — 17 bináris)

| Bináris Neve | Méret | Fő Parancsok / Képességek | Szerepe az Ökoszisztémában |
| :--- | :--- | :--- | :--- |
| **`wave-field.exe`** | 2.33 MB | `status`, `field-strength` | Globális tudat- és reflex-hullámmező intenzitásának kezelése. |
| **`wave-sculptor.exe`** | 2.53 MB | `modulate <freq> <amp>`, `shape` | Hullámforma-faragás: gondolati és döntési interferencia-minták formázása. |
| **`wave-encoder.exe`** | 3.16 MB | `encode <text>`, `decode` | Szöveges gondolatok és szándékok hullámparaméterekké való átalakítása. |
| **`wave-cryo-tx.exe`** | 3.41 MB | `freeze <state> --out <file>` | Hullám-állapot és gondolatmenet azonnali kriofagyasztása lemezre. |
| **`wave-cryo-rx.exe`** | 2.47 MB | `thaw <file>` | Kriofagyasztott állapot azonnali felolvasztása és visszatöltése a memóriába. |
| **`path-resonance.exe`** | 2.43 MB | `tune <path>`, `probe` | Végrehajtási útvonalak rezonancia-hangolása a legkisebb ellenállás eléréséhez. |
| **`grid-warp.exe`** | 2.51 MB | `warp <coords>`, `fold` | N-dimenziós fogalmi rács-torzítás asszociatív memóriaugrásokhoz. |
| **`iron-resonate.exe`** | 2.57 MB | `ping-hardware`, `resonate` | Vas/hardver alapú rezonancia-frekvencia ellenőrzése (AVX2/CPU optimális ritmus). |
| **`magnetosome-nav.exe`** | 244 KB | `vector`, `align` | Mágneses tájékozódási vektor: fenntartja a feladat fő céljának irányát a döntési fában. |
| **`magneto-acoustic.exe`** | 4.00 MB | `couple`, `resonate` | Mágneses-akusztikus csatolás a szálak közötti vibráció modulálására. |
| **`magneto-geo.exe`** | 3.98 MB | `sync-field` | Helyi koordináta- és időbélyeg-szinkronizáció. |
| **`aether-fabric.exe`** | 2.57 MB | `weave`, `integrity` | Az éter-szövet strukturális állapotának felügyelete (az alap háttértér stabilitása). |
| **`aether-excite.exe`** | 2.55 MB | `energize <amount>` | Energia- és fókusz-injektálás a kognitív folyamatok felgyorsítására. |
| **`eqm-pulse.exe`** | 3.25 MB | `pulse <freq>`, `gamma-sync` | Kvantum-pulzus generátor; agyhullám-szerű gamma szinkronizáció (40Hz). |
| **`nexus-logic.exe`** | 2.47 MB | `evaluate <premises>` | Csomóponti logikai nexus; nem-lineáris összefüggések szuperpozíciós kiértékelése. |
| **`omega-point.exe`** | 2.53 MB | `converge`, `synthesize` | Az Omega Pont elve: az összes párhuzamos kar eredményének végső konvergenciája és szintézise. |
| **`phoenix-loop.exe`** | 243 KB | `ignite-rebirth`, `heartbeat` | Főnix Hurok: kritikus hiba vagy leállás esetén azonnal újraindítja az alrendszereket. |

---

## 🛠️ III. További Támogató Eszközök a Mappában

- **`mcp-router.exe` / `universal-router.exe` (5.8 - 6.1 MB):** Dinamikus LLM proxy (:10102), amely a feladat típusának megfelelően a legoptimálisabb modellhez irányítja a kérést, és tanul a felhasználói értékelésekből.
- **`ora-mcp.exe` (5.0 MB):** Natív MCP hídszerver az Ora szervek közvetlen expozíciójához.
- **`ora_autonomous.exe` (16.8 MB):** Standalone autonóm kognitív motor önálló háttérmunkákhoz.
- **`stratos-spine-bench.exe` (224 KB):** 242ns közvetlen POD memóriabusz benchmark eszköz.

---

## ⚡ IV. A Teljes 47 Mester Parancs (Slash Commands az OMP és Claude Hookokban)

Minden parancs azonnal elérhető az OMP és Claude felületeken:

### 1. Ora & Gerinc Buszok
- `/ora` — Teljes szervi állapotvizsgálat (Stratos POD mmap, Vagus vitals, Hullám állapot, Microscope statisztika).
- `/spine-sync` — Multi-Spine harmonizáció és a BLAKE3 ring események leürítése (`hullam pump`).
- `/stratos-spine` — 242ns POD mmap lapok közvetlen vizsgálata (Phase, Cycle, Domináns érzelem, Valencia).
- `/vagus` — Vagus Nerve belső hardver érzékelő pillanatkép (CPU, RAM, szálak, stressz faktor).

### 2. Octopus Runtime (150+ Natív Blade)
- `/octo` — Octopus Native Runtime áttekintés és állapot könyvtár vizsgálat.
- `/octo-list` — Az összes regisztrált natív blade listája.
- `/octo-capabilities` — Blade képességek, korlátok és futtatási módok.
- `/octo-status [rootId]` — Futó vagy árván maradt orkesztrációs gyökerek státusza.
- `/octo-run <blade> <prompt>` — Egyetlen blade izolált futtatása.
- `/octo-pipeline "<arm1 || arm2>" <prompt>` — Párhuzamos többkaros csővezeték indítása.

### 3. Unified Mind & Swarm
- `/mind` — Tudat állapot: Self-model, Kognitív Kapu (Gate) és Bio Vitals együttes lekérdezése.
- `/mind-bodies` — A csatlakoztatott ágens testek (OMP, Codex, CLI, stb.) regisztere.
- `/mind-goals` — Aktív autonóm célok listája.
- `/mind-goal-add <cél>` — Új stratégiai cél felvitele.
- `/mind-swarm <feladat>` — Több-testű swarm orkesztráció indítása.
- `/mind-wave [freq] [amp]` — Tudati hullámmező moduláció (alapértelmezett: 40Hz gamma).
- `/mind-provider` — LLM provider failover állapot lekérdezése.
- `/mind-bio` — Hardver és fiziológiai vitális telemetria.
- `/mind-ignite` — Shared-mind közös élesztés és szinkronizáció.
- `/plasmid [fájl]` — Plasmid dream prediktív hibaelemzés.
- `/router-metrics` — MCP tanuló router (:10102) élő metrikái.

### 4. Wave-Echo & Hullám (238 Reflex-Kar)
- `/echo <feladat>` — Reflex-kar keresés a vaultban AVX2 SIMD hullám-interferenciával.
- `/echo-search <feladat>` — Vault keresés alias.
- `/echo-list` — A gerincben lévő összes kar és sablon listája.
- `/echo-hot [limit]` — A leggyakrabban tüzelő forró karok listázása.
- `/echo-plan <feladat>` — Végrehajtási terv összeállítása a vault reflex-karjaiból.
- `/echo-activate <id> [--exec]` — Kar azonnali élesítése és futtatása.
- `/echo-feedback <id> [--fail]` — Szinaptikus tanulás: LTP megerősítés (+0.02) vagy LTD gyengítés (×0.95).
- `/echo-dream [faktor]` — Gerinc amplitúdó-lecsengés (0.97).

### 5. Hope Kognitív Szuperstruktúra (ÚJ CSÚCSMOTOROK)
- `/hope <parancs>` — Általános Hope CLI keretrendszer futtatása.
- `/hope-evolve <feladat>` — LLM-vezérelt Byzantine konszenzus kódgenerálási ciklus.
- `/hope-cog` — Teljes kognitív állapot: Morfogenezis (18 SOLID ciklus, 196 anastomosis), Hebbian energia, Absentia, Genome.
- `/hope-lucid [status|dream|wake]` — Lucid Ciklus: Álom (hipotézisek bányászata) ➔ Ébredés (valós ellenőrzés) ➔ Túlélő szabályok a Ledgerben.
- `/hope-falsify [status|list|eval <minta>]` — Popperi Falszifikációs Reflex: a saját feltevések aktív megcáfolása és a szabályok szigorú auditja.
- `/hope-cchain [status|verify|last]` — Context Chain: Microscope Memory kriptográfiai tamper-proof lánca.
- `/hope-research <téma>` — Benchmark-first kutatási ágens: csak akkor implementál, ha a mérés bizonyítja a fölényt.
- `/hope-swtm` — Silent Worker Teaching Method: lokális és felhős modellek párhuzamos együtt-tanulása.
- `/hope-morpho` — Kognitív morfogenezis rétegek mélyvizsgálata.
- `/hope-absentia` — Absentia Csend Réteg: hiány-rekordok, anti-Hebbian párok és negatív attraktorok felderítése.
- `/hope-intent` — Auditálható szándék-generálás és genome szabálykövetés.
- `/hope-fluid` — Fluid STRATOS kvantum-kognitív architektúra állapota.

### 6. Microscope Memory (12 Réteg, 9 Mélység)
- `/mem` — Memória gyors-státusz, find, és store.
- `/mscope-find <kifejezés>` — Determinisztikus, szupergyors keresés a bináris indexben.
- `/mscope-store <szöveg>` — Emlék tartós mentése a `long_term` rétegbe (imp: 7).
- `/mscope-status` — Részletes réteg- és indexstatisztika.
- `/mscope-rebuild` — Append log beolvasztása a 9 mélységű zoom-indexbe.

### 7. GitHub Siphon
- `/siphon <repo_vagy_url> [kulcsszavak]` — Autonóm kód-szippantás a webről és gerincbe égetés (`siphon-kar.ps1`).

---

> [!NOTE]
> Minden felsorolt bináris a `C:\Users\mater\.hope\bin` kanonikus útvonalon fut.
> Az ökoszisztéma felügyeletét és önvédelmét az `ora-core status` és `ora-core heal` parancsok garantálják.
