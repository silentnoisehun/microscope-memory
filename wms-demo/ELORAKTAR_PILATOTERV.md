# ÉLŐ RAKTÁR — Rendszerbevezetési Terv

**Címzett:** Gábor (Területi vezető, Rudolf Logisztika — Tatabánya, Lotte részleg)
**Készítette:** Máté (Logisztikai adminisztrátor)
**Dátum:** 2026. augusztus
**Verzió:** 1.0 — Pilot terv

---

## 1. Vezetői összefoglaló

Az ÉLŐ RAKTÁR egy olyan raktárkezelő rendszer, amely **0 Ft hardver befektetéssel** és a **meglévő Motorola/Zebra terminálokon** működik. A rendszer célja, hogy a jelenlegi káoszt — ahol senki nem tudja, mi hol van, és a betárolás emberi hibákkal van tele — egy **hibamentes, automatikus, folyamatos leltárral rendelkező** működésre cserélje.

**A kulcs számok:**

| Mutató | Jelenleg | ÉLŐ RAKTÁR rendszerrel |
|--------|----------|----------------------|
| Ember / váltás | 20 | 5 |
| Betárolási hiba | Gyakori | 0% |
| Leltározás | Fizikai, heti | Folyamatos, automatikus |
| Pozicionálás | Emberi emlékezet | Rádiófrekvenciás (RF) követés |
| Hardver költség | — | 0 Ft |
| Keresési idő raklaponként | 5-15 perc | 0 (a rendszer mondja meg) |

---

## 2. A jelenlegi helyzet

### 2.1 Mi a probléma?

A tatabányai külső raktár Lotte részlegén a betárolás jelenleg **káoszos** és **emberi hibákra van kiélezve**:

- **Nincs nyilvántartás:** A targoncás nem tudja, hova tette a zsákot. Nincs perszisztens leltár.
- **Kognitív túlterheltség:** A sofőrnek kódokat kell keresnie, helyeket megjegyeznie, SAP-ban rögzítenie — közben 750 kg-os zsákokat mozgat.
- **Nincs hálózat:** Az objektumban nincs kiépített Wi-Fi. A rendszer rádiófrekvenciás (RF) követést használ — a plafonra helyezett érzékelőkön keresztül, mint egy beltéri GPS.
- **Megbízói inkorrektség:** A Lotte-tól hibás szállítólevelekkel, sérült áruval, ömlesztve érkezik minden, de tőlünk minden apróságot követelnek (pecsét, dátum, teljes rendszám).
- **Mozgóbér elvonás:** Egy kis hiba miatt hetekre vagy egész évre elveszik a mozgóbér.
- **Emberi hiba:** Mivel a targoncásnak döntéseket kell hoznia (hol van hely, hova tegyem, hogyan rögzítsem), a hiba gyakori és elkerülhetetlen.

### 2.2 Miért nem működik a jelenlegi módszer?

A jelenlegi folyamat azt feltételezi, hogy a targoncás **emlékezik**, **dönt**, és **rögzít** — miközben 750 kg-os zsákokat mozgat -20°C-ban. Ez emberileg lehetetlen followingnak bizonyul.

A probléma **nem az emberekben van** — hanem a **rendszerben**, ami túl sokat kér tőlük.

---

## 3. Az ÉLŐ RAKTÁR rendszer

### 3.1 Alapelv

> **A targoncás csak vezet. Minden mást a rendszer intéz.**

Nem kell gondolkodnia, nem kell döntést hoznia, nem kell kódokat keresnie. Egyetlen feladata van: követni a nyilat a képernyőn.

### 3.2 Hogyan működik?

A rendszer **14 lépésből** áll, amelyből **csak 1 lépés** igényel emberi döntést (az admin bekönyvelése):

| Lépés | Ki csinálja? | Emberi döntés? |
|-------|-------------|---------------|
| 1. Kamion beáll a rámpára | Sofőr | Nem |
| 2. Sofőr átadja szállítólevelet | Sofőr | Nem |
| 3. **Admin bekönyveli (1 kattintás)** | **Admin** | **Igen — ez az egyetlen** |
| 4. Rendszer generál 10 virtuális zsák címkét | Rendszer | Nem |
| 5. RFID követő aktiválása | Rendszer | Nem |
| 6. Targoncás felveszi a zsákot | Targoncás | Nem (csak felemeli) |
| 7. Útvonal képe megjelenik a terminálon | Rendszer | Nem |
| 8. Átvált nagy nyílra — indulás | Rendszer | Nem |
| 9. Követi a nyilat a célig | Targoncás | Nem (csak követ) |
| 10. Lerakja a zsákot — pozíció auto. ment | Rendszer | Nem |
| 11. Zöld visszajelzés | Rendszer | Nem |
| 12. Következő zsák — ismétlés | Rendszer | Nem |
| 13. Ha rossz helyre rakna — PIROS RIASZTÁS | Rendszer | Nem (tiltja) |
| 14. Mind 10 zsák kész — fuvar lezárva | Rendszer | Nem |

**14 lépésből 1-ben kell embernek gondolkodni.** A többi automatikus.

### 3.3 Technikai megoldás

| Komponens | Megoldás | Költség |
|-----------|----------|---------|
| **Pozicionálás** | Rádiófrekvenciás (RF) követés a plafon érzékelőin keresztül | 0 Ft (meglévő terminál + plafonszenzorok) |
| **Pozicionálás** | Rádiófrekvenciás követés (RF) a plafonon | 0 Ft (szenzorok a meglévő terminálon) |
| **Navigáció** | Egyetlen nagy nyíl a Motorola terminálon | 0 Ft (meglévő eszköz) |
| **Adatrögzítés** | Rádiófrekvenciás pozíció-rögzítés + helyi SQLite | 0 Ft (szoftver) |
| **Szinkronizáció** | Batch szinkron dokkolóállomáson | 0 Ft |
| **Szkennelés** | Meglévő Motorola/Zebra szkenner | 0 Ft |
| **Címkék** | Virtuális, auto-generált QR kódok | 0 Ft (szoftver) |

**Hardver befektetés: 0 Ft.** Csak egy app kell a meglévő terminálra.

---

## 4. Hibamentesség — Miért 0% a betárolási hiba?

### 4.1 A hiba forrása megszüntetve

A jelenlegi rendszerben a hiba abból fakad, hogy a **targoncás dönt** — és az emberi döntés hibázik. Az ÉLŐ RAKTÁR rendszerben:

1. **A targoncás nem dönt.** A rendszer mondja meg, hova tegye. A sofőr csak követi a nyilat.
2. **A rendszer tiltja a rossz helyet.** Ha a targoncás nem a megadott pozícióra teszi le a zsákot:
   - A terminál **azonnal hangosan sípol** és **pirosan villog**
   - A sofőrnek **nincs lehetősége csendben félrerakni** — a rendszer mindenkit figyelmeztet
3. **A téves pozíció is rögzítve lesz.** Ha a sofőr mégis ott hagyja:
   - A tényleges (rossz) pozíció **elmentésre kerül** — a zsák nem vész el
   - A műszakvezető **azonnal értesítést kap** a dashboardon
   - Az áru **megtalálható marad** — csak más helyen van, mint kellett volna
4. **3. réteg tiltva.** A rendszer nem enged 3 zsákot egymásra rakni — a 2 magas szabály kikényszerítve.
5. **Cluster optimalizáció.** Egy konténer 10 zsákja **egymás mellé** kerül — nincs szétszórva a raktárban.

### 4.2 Hibamátrix

| Hibalehetőség | Jelenlegi rendszer | ÉLŐ RAKTÁR |
|---------------|-------------------|-----------|
| Rossz helyre tesz | Gyakori, észrevétlen | Azonnali riasztás + pozíció mentve |
| 3. réteg (biztonsági kockázat) | Előfordul | Rendszer tiltja |
| Elfelejti rögzíteni | Gyakori | Automatikus (nem kell emlékezni) |
| Nem találja vissza a zsákot | Gyakori, leltározás kell | 0 (GPS-szerű pozíciókövetés) |
| Szállítólevél hiányos | Mozgóbér elvonás | Rendszer ellenőrzi, riaszt ha hiányos |
| Összekeveri anyagokat | Előfordul | Színkódok + anyagkód minden zsákon |

---

## 5. Állandó leltár — Mindig kész

### 5.1 Mi változik?

A jelenlegi rendszerben a **leltározás fizikai folyamat** — emberek járkálnak, számolnak, keresnek, javítanak. Heti szinten.

Az ÉLŐ RAKTÁR rendszerben a **leltár soha nem készül el — mert mindig kész**:

- Minden zsák lerakásakor a **pozíció automatikusan rögzítve** lesz (X, Y koordináta + szint L1/L2)
- A leltár **másodpercről másodpercre** naprakész
- Bármikor, **egy gombnyomásra** lekérdezhető: mi, hol, mennyi
- A műszakvezető dashboardja **élőben** mutatja az egész raktárt
- **Nincs fizikai leltározás** — megszűnik a heti órákig tartó folyamat

### 5.2 Leltár érték láthatóság

A vezetői dashboard valós időben mutatja:

- **Zsákok száma** csarnokonként
- **Leltárérték forintban** (zsákok × 750 kg × anyagár/kg)
- **Anyag eloszlás** (PP, PE, ABS, PET, PVC) százalékosan
- **Raktár kihasználás** (%) — foglalt vs. szabad pozíciók
- **2 magas arány** — hány pozíción van 2 zsák
- **Napi forgalom** — beérkezett és kitárolt zsákok

---

## 6. Pilot terv — 1 folyosó, 1 targonca, 2 hét

### 6.1 Miért pilot?

Nem kell az egész raktárt átalakítani. **Egyetlen folyosóban, egyetlen targoncával** teszteljük a rendszert. Ha működik — kiterjesztjük. Ha nem — 0 Ft veszteség.

### 6.2 Pilot idővonal

| Nap | Tevékenység | Időigény |
|-----|------------|----------|
| 1-2 | Raktártérképezés: a tesztszekció méreteinek digitalizálása, koordináta-háló létrehozása | 2 óra |
| 3 | App telepítése egy Motorola terminálra, kioszk mód, szenzorkalibrálás | 1 óra |
| 4-8 | Éles teszt: a kijelölt targoncás a tesztszekcióban dolgozik a rendszerrel | 5 munkanap |
| 9-10 | Eredmények mérése, jelentés Gábornak, döntés a kiterjesztésről | 2 nap |

### 6.3 Mit mérünk a pilot után?

| Metrika | Mérés |
|---------|-------|
| Hibás lerakások száma | Before/after összehasonlítás |
| Átlagos betárolási idő zsákonként | Másodpercben |
| Leltár pontosság | A rendszer koordinátái vs. fizikai valóság |
| Targoncás visszajelzés | Mennyivel könnyebb-e a munka |
| 2 magas rakás biztonság | A rendszer tiltja-e a 3. réteget |

### 6.4 Pilot költsége

| Tétel | Költség |
|-------|---------|
| Hardver | 0 Ft (meglévő terminál + RF szenzorok) |
| Szoftver | Saját fejlesztés |
| Raktártérképezés | 2 óra |
| Targonca tartókonzol | ~20.000 Ft (ha kell) |
| **Összesen** | **~0 Ft** |

---

## 7. Bevezetés utáni gyorsulás

### 7.1 Időmegtakarítás

| Tevékenység | Jelenleg | ÉLŐ RAKTÁR-val | Megtakarítás |
|-------------|----------|----------------|-------------|
| Tárhely keresése | 5-15 min/zsák | 0 (a rendszer mondja) | 100% |
| SAP rögzítés | 2-3 min/zsák | 0 (automatikus) | 100% |
| Leltározás (heti) | 4-8 óra/hét | 0 (folyamatos) | 100% |
| Elveszett zsák keresése | 30-60 min | 0 (pozíció ismert) | 100% |
| Betárolás raklaponként | 10-20 min | 3-5 min | 60-75% |

### 7.2 Munkaerő megtakarítás

A jelenlegi 20 fős csapat 5 fősre csökkenhet, mert:

- **Nincs manuális rögzítés** — a rendszer automatikusan rögzít
- **Nincs keresés** — a rendszer tudja, hol van minden
- **Nincs leltározás** — a leltár mindig kész
- **Nincs adminisztrációs hiba** — a rendszer nem hibázik
- A maradék 5 ember: 2 targoncás + 1 admin + 1 műszakvezető + 1 tartalék

### 7.3 Költséghatékonyság

| Költségtétel | Jelenleg (éves) | ÉLŐ RAKTÁR-val | Megtakarítás |
|-------------|-----------------|----------------|-------------|
| Munkaerő (15 felesleges ember) | ~45.000.000 Ft/év | 0 Ft | 45M Ft/év |
| Mozgóbér elvonás (hibák miatt) | ~2.000.000 Ft/év | 0 Ft (nincs hiba) | 2M Ft/év |
| Fizikai leltározás (órák) | ~3.000.000 Ft/év | 0 Ft | 3M Ft/év |
| Elveszett áru | ~5.000.000 Ft/év | 0 Ft | 5M Ft/év |
| **Összes potenciális megtakarítás** | | | **~55M Ft/év** |

Hardver befektetés: **~0 Ft** (RF szenzorok a plafonon — minimális költség, a meglévő terminálokon fut a szoftver).
Szoftverfejlesztés: saját munka, projektdíj vagy saját fejlesztés.

---

## 8. Miért nem lesz kaotikus?

### 8.1 A káosz forrásai megszüntetve

| Káosz forrás | Jelenleg | ÉLŐ RAKTÁR megoldás |
|-------------|----------|---------------------|
| Mindenki a maga feje után megy | "Majd lesz valami" | A rendszer tervezi az útvonalat |
| Nincs leltár | "Hol van az a zsák?" | Élő pozíciókövetés — GPS a raktárban |
| Emberi hiba a rögzítésben | Elfelejti, elírja | Automatikus — nincs mit elfelejteni |
| Keresgélés | Órákig tart | 0 másodperc — a rendszer megmondja |
| Raktártérkép nincs | Senki nem tudja a teljes képet | Admin dashboard — élő, valós idejű |
| Megbízói hibák nincsenek dokumentálva | Szóbeli panaszkodás | Audit log — minden mozgás rögzítve |
| Nem lehet előre tervezni | "Mikor jön a következő kamion?" | Vezetői dashboard — napi tervezés |

### 8.2 Rendszerfegyelem — automatikusan

A rendszer nem kéri a dolgozóktól, hogy **emlékezzenek** vagy **figyeljenek**. Ehelyett:

- **Kikényszeríti** a szabályokat (3. réteg tiltás, pozíció-ellenőrzés)
- **Automatikusan** rögzíti a pozíciót (nem kell emlékezni)
- **Visszajelzést** ad (zöld = jó, piros = rossz)
- **Audit trail** — minden mozgás nyomon követhető

**Nincs szabály, amit be kell tartani — mert a rendszer betartja helyetted.**

---

## 9. Megbízói kapcsolat (Lotte)

### 9.1 A jelenlegi probléma

A Lotte-tól hibás szállítólevelekkel, sérült áruval, ömlesztve érkezik minden. De tőlünk minden apróságot követelnek (pecsét, dátum, teljes rendszám). Ha bármi hiányzik — mozgóbér elvonás.

### 9.2 Az ÉLŐ RAKTÁR megoldás

- **Beérkezéskor a rendszer ellenőrzi** a szállítólevelet — hiányos adatok esetén azonnal riaszt
- **Minden érkezés dokumentálva** van — fénykép, szkenner, pozíció
- **Audit log** — ha a Lotte vitatja, a rendszer mutatja, mikor, mit, hogyan érkezett
- **Nincs mozgóbér elvonás** — mert nincs emberi hiba a rögzítésben
- **A rendszer szabályozottan, dokumentálva** dolgozik — védelem a megbízó inkorrektsége ellen

---

## 10. Kockázatok és mitigálás

| Kockázat | Valószínűség | Megoldás |
|---------|-------------|---------|
| A dolgozók ellenállnak | Közepes | Pilot csak 1 folyosóban, önkéntes targoncás |
| Inerciális drift (pontoság romlik) | Alacsonk | Kalibrációs pontok a raktárban, dokkolóállomás szinkron |
| A terminál lemerül | Alacsonk | Tartókonzol töltéssel, tartalék terminál |
| A rendszer nem működik | Nagyon alacsonk | Pilot 0 Ft — ha nem jó, nincs veszteség |
| A felsővezetés nem támogatja | Közepes | 0 Ft hardver — nem kell beruházási engedély |

---

## 11. Következő lépések

1. **Bemutató Gábornak** — a 3D demó + ez a dokumentum
2. **Pilot indulás** — 1 folyosó, 1 targonca, 2 hét
3. **Mérés** — hibaszám, idő, leltár pontosság
4. **Döntés** — kiterjesztés vagy sem
5. **Ha sikeres** — fokozatos kiterjesztés a többi folyosóra

### Miért pont most?

- A káosz napról napra nő
- A mozgóbér elvonások demoralizálják a csapatot
- A megbízói követelmények egyre szigorúbbak
- A 0 Ft hardver azt jelenti: **nincs kockázat**
- A pilot 2 hetet vesz igénybe — nem hónapokat

---

## 12. Összegzés

Az ÉLŐ RAKTÁR nem egy új szoftver — **egy új működési modell**. Ahol:

- A **targoncás csak vezet** — nincs gondolkodás, nincs hiba
- A **leltár mindig kész** — nincs fizikai leltározás
- A **rendszer intéz mindent** — pozíció, útvonal, riasztás, leltár
- A **költség minimális** — meglévő terminálok + RF szenzorok a plafonon
- A **pilot 2 hetes** — utána mérhető az eredmény

**Ha a pilot nem működik — semmit nem veszítettünk.**
**Ha működik — 55M Ft/év megtakarítás, 0% hiba, folyamatos leltár.**

---

*Szerkesztette: Máté — Logisztikai adminisztrátor*
*Készült: 2026. augusztus*
*Rudolf Logisztika — Tatabánya — Lotte részleg*