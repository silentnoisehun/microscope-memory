# Performance: 37 ns vs ~99 µs on D0

## Nyers eredmény

A `docs/ARCHITECTURE.md` két különböző számot ad:

- **37 ns** — a dokumentum szerint "warm L1 cache, 1 blokk" feltételekkel.
- **~99 µs** — az aktuális benchmarkkomparáción mért, cold cache esetén.

## Mi okozja a különbséget?

| Faktor | 37 ns | ~99 µs |
|---|---|---|
| **Cache state** | L1d hot, adat már a CPU-on | Cold start, mmap page faultokkal |
| **Mérési egység** | Egyetlen blokk, nulla overhead | Teljes `look()` hívás: komparáció + formázás + kiírás |
| **Hő** | Labor, nincs kernel overhead | Valós CLI, több `println!`, hozzáférés `append.bin`-hez is |
| **Hardver** | Specifikus CPU, turboram | Általános rendszer, alapterhelés |

## Szakmai magyarázat

A `src/commands/bench.rs` 10 000 iterációt futtat, és átlagot számol. Ott a fókusz a **zoom integrált keresési sebességen** van, nem egyetlen cache-hit eseten. A 37 ns az ideális, "best case scenario" — például akkor, ha az index L1-be fér, és a query pontosan egyetlen blokkot talál.

A ~99 µs a **valós, átlagos** eset: az index nagyobb, a keresés több depth-szintet érint, és a kiírási költség (stdout flush) beleszámít.

## Következtetés

Mindkét szám helyes; csak a feltételek különbözők. A fő különbség a **measurement methodology**, nem a kódban lévő hiba.
