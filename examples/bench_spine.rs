//! Spine benchmark — a rétegek önálló fájljaikra töltése vs. egyetlen mmap.
//!
//! Méri:
//!   A. COLD — a réteges `load_or_init()` lánc éles adatkönyvtárral (másolat)
//!   B. SPINE — ugyanaz az állapot egy `spine.bin` mmap-ból
//!   C–F. a hot path: fejrész-olvasás, `tick()`írás, rétegenkénti r+w — **egyszálú,
//!      versenytárs nélkül**
//!   I. **kontendált**: n író × m olvasó, throughput + szakadási ráta
//!
//! Az A/A2/A3 sorok csak a `data/` könyvtárral jelentenek valamit; az nincs a
//! repóban, ezért egy tiszta clone-ból futtatva az "előtte" oldal üres
//! alapállapotokat tölt be. Ezt a bench ki is írja, ne kelljen a számból
//! kitalálni.
//!
//! Futtatás (release): cargo run --release --example bench_spine

use std::path::{Path, PathBuf};
use std::time::Instant;

use microscope_memory::spine::{CacheHeader, EmotionHeader, Spine, PAGE_SIZE};

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("m1c_spine_bench_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// A nyolc stream-réteg betöltése a saját, jelenlegi API-ján.
fn load_layers_legacy(output_dir: &Path) {
    use microscope_memory::archetype::ArchetypeState;
    use microscope_memory::attention::AttentionState;
    use microscope_memory::emotional_state::EmotionalStateRing;
    use microscope_memory::mirror::MirrorState;
    use microscope_memory::predictive_cache::PredictiveCache;
    use microscope_memory::resonance::ResonanceState;
    use microscope_memory::thought_graph::ThoughtGraphState;

    // A blokkszám a meta.bin 8..12 bájtjában van (MSC4/MSCM fejléc),
    // vagy az aktivációs fájl méretéből — 32 B/rekord.
    let block_count = std::fs::read(output_dir.join("meta.bin"))
        .ok()
        .filter(|m| m.len() >= 12)
        .map(|m| u32::from_le_bytes([m[8], m[9], m[10], m[11]]) as usize)
        .unwrap_or_else(|| {
            std::fs::metadata(output_dir.join("activations.bin"))
                .map(|md| (md.len() / 32) as usize)
                .unwrap_or(100)
        });
    let _hebbian = microscope_memory::hebbian::HebbianState::load_or_init(output_dir, block_count);
    let _attention = AttentionState::load_or_init(output_dir);
    let _emotion = EmotionalStateRing::load_or_init(output_dir);
    let _resonance = ResonanceState::load_or_init(output_dir);
    let _graph = ThoughtGraphState::load_or_init(output_dir);
    let _cache = PredictiveCache::load_or_init(output_dir);
    let _archetypes = ArchetypeState::load_or_init(output_dir);
    let _mirror = MirrorState::load_or_init(output_dir);
}

/// n író × m olvasó egy közös `spine.bin`-en. Az írók `tick()`-et írnak, az
/// olvasók `read_snapshot()`-t vesznek, és a page-belső invariánst ellenőrzik.
fn contend(npub: usize, nread: usize, per: u64) -> (f64, f64, u64, u64, u64) {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;

    let dir = temp_dir(&format!("c{}x{}", npub, nread));
    let mut pubs = Vec::new();
    for _ in 0..npub {
        pubs.push(Arc::new(Spine::open(&dir).expect("open publisher")));
    }
    let readers: Vec<_> = (0..nread)
        .map(|_| Arc::new(Spine::open_ro(&dir).expect("open reader")))
        .collect();

    let stop = Arc::new(AtomicBool::new(false));
    let ticks = Arc::new(AtomicU64::new(0));
    let snaps = Arc::new(AtomicU64::new(0));
    let torn = Arc::new(AtomicU64::new(0));
    let fresh = Arc::new(AtomicU64::new(0));

    let t0 = Instant::now();
    let mut writers = Vec::new();
    for (i, sp) in pubs.into_iter().enumerate() {
        let (stop, ticks) = (stop.clone(), ticks.clone());
        let parity = i as u64;
        writers.push(std::thread::spawn(move || {
            let mut seq = 0u64;
            while !stop.load(Ordering::Relaxed) {
                // A cycle tartja az író paritásáról, így egy teljes page
                // bármelyik írótól kielégíti az olvasó oracle-át.
                let cycle = 1_000 + seq * npub as u64 + parity;
                sp.tick(cycle, 0.1, 0.2, !cycle, 0.5, cycle, cycle, cycle as f64);
                seq += 1;
                ticks.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }
    let mut readers_h = Vec::new();
    for rd in readers {
        let (stop, torn, fresh, snaps) = (stop.clone(), torn.clone(), fresh.clone(), snaps.clone());
        readers_h.push(std::thread::spawn(move || {
            let mut done = 0u64;
            while done < per && !stop.load(Ordering::Relaxed) {
                match rd.read_snapshot() {
                    Some(s) => {
                        if s.stream.cycle == 0 && s.stream.predicted_query_hash == 0 {
                            fresh.fetch_add(1, Ordering::Relaxed);
                        } else if s.stream.predicted_query_hash != !s.stream.cycle {
                            torn.fetch_add(1, Ordering::Relaxed);
                        }
                        snaps.fetch_add(1, Ordering::Relaxed);
                        done += 1;
                    }
                    // Csillagzás: inkább szólaljunk meg, mint nézzenek ki
                    // nullának a szakadási számlálók.
                    None => {
                        torn.fetch_add(1, Ordering::Relaxed);
                        stop.store(true, Ordering::Relaxed);
                    }
                }
            }
        }));
    }
    for h in readers_h {
        h.join().expect("reader thread");
    }
    stop.store(true, Ordering::Relaxed);
    for h in writers {
        h.join().expect("writer thread");
    }

    let secs = t0.elapsed().as_secs_f64().max(1e-9);
    let out = (
        snaps.load(Ordering::Relaxed) as f64 / secs,
        ticks.load(Ordering::Relaxed) as f64 / secs,
        snaps.load(Ordering::Relaxed),
        torn.load(Ordering::Relaxed),
        fresh.load(Ordering::Relaxed),
    );
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn main() {
    println!("═══════════════════════════════════════════════════════");
    println!("  MICROSCOPE SPINE — mmap vs per-layer file load");
    println!("═══════════════════════════════════════════════════════\n");

    // Éles adatkészlet másolata a benchmark-hez.
    let real_data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
    let data_copy = temp_dir("data");
    let copy_note = match copy_dir(&real_data, &data_copy) {
        Ok(_) => {
            let n = std::fs::read_dir(&data_copy).unwrap().count();
            format!("{} files, {} KB", n, dir_size(&data_copy) / 1024)
        }
        Err(_) => "(no bundled data dir; using empty)".to_string(),
    };
    println!("Adatkészlet: {}\n", copy_note);
    if !real_data.is_dir() {
        println!(
            "FIGYELEM: nincs {} — az A/A2/A3 „előtte” sorok üres könyvtárat töltenek,\n             tehát a gyorsulási szám ebből a futásból NEM értelmezhető. A C–I sorok\n             ettől függetlenek (csak spine API-t használnak).\n",
            real_data.display()
        );
    }

    // ── A. Legacy cold ──
    let n_cold = 20;
    let mut legacy_cold = std::time::Duration::ZERO;
    for _ in 0..n_cold {
        // Minden iterációban új könyvtár, hogy az OS cache ne segítsen.
        let d = temp_dir("cold");
        copy_dir(&data_copy, &d).unwrap();
        let t = Instant::now();
        load_layers_legacy(&d);
        legacy_cold += t.elapsed();
        let _ = std::fs::remove_dir_all(&d);
    }
    println!(
        "A. Legacy 8× load_or_init (cold):   {:?} /start   ({} runs)",
        legacy_cold / n_cold,
        n_cold
    );

    // ── A2. Legacy warm (egy könyvtár újra és újra) ──
    let t = Instant::now();
    load_layers_legacy(&data_copy);
    let legacy_warm_first = t.elapsed();
    println!("A2. Legacy first load (warm):      {:?}", legacy_warm_first);

    let n_warm = 100;
    let t = Instant::now();
    for _ in 0..n_warm {
        load_layers_legacy(&data_copy);
    }
    let legacy_reload_first = t.elapsed() / n_warm;
    println!(
        "A3. Legacy re-load:                 {:?} /start\n",
        legacy_reload_first
    );

    // ── B. Spine cold ──
    let spine_dir = temp_dir("spine");
    let t = Instant::now();
    let spine = Spine::open(&spine_dir).unwrap();
    let spine_cold = t.elapsed();
    println!("B. Spine::open (cold, fresh file): {:?}", spine_cold);

    let t = Instant::now();
    let _sb = spine.read_superblock().unwrap();
    println!("   superblock read:                 {:?}", t.elapsed());

    drop(spine);

    // ── B2. Spine warm ──
    let n_spine = 100;
    let t = Instant::now();
    for _ in 0..n_spine {
        let s = Spine::open(&spine_dir).unwrap();
        let _ = s.read_stream();
    }
    let spine_open_warm = t.elapsed() / n_spine;
    println!(
        "B2. Spine::open (warm):             {:?} /open\n",
        spine_open_warm
    );

    // ── C. Hot path: mind a 11 réteg fejrészének olvasása — EGYSZÁLÚ, író nélkül.
    //        Egy versenytárs sincs, tehát a lock-ok vitelenek; az I. szekció méri
    //        azt, amit a seqlock és a publisher lock ténylegesen befolyásol. ──
    let s = Spine::open(&spine_dir).unwrap();

    // Először egy teljes tick írása, hogy a fejlécek ne legyenek mind nulla.
    s.tick(1, 0.1, 0.2, 12345, 0.9, 67890, 111, 42.0);

    let n_ops = 200_000;
    let t = Instant::now();
    for i in 0..n_ops {
        let _ = s.read_stream();
        let _ = s.read_hebbian();
        let _ = s.read_attention();
        let _ = s.read_emotion();
        let _ = s.read_resonance();
        let _ = s.read_graph();
        let _ = s.read_cache();
        let _ = s.read_archetype();
        let _ = s.read_mirror();
        let _ = s.read_merkle();
        let _ = s.read_meta();
        if i % 1000 == 0 {
            // Nyomás nélkül egyszerűen pörgetjük a lock-ot.
            let _ = s.size_bytes();
        }
    }
    let all_read = t.elapsed();
    println!(
        "C. 11 fejrész olvasása (1 szál):     {:?} /all-11  ({} iters, {:?})",
        all_read / n_ops,
        n_ops,
        all_read
    );

    // ── D. Hot path: tick írás — egyszálú, tehát a publish lock nem vár senkire,
    //        de a két flock syscall így is benne van; az I. szekció a versenyt. ──
    let n_ticks = 200_000;
    let t = Instant::now();
    for i in 0..n_ticks {
        s.tick(
            i as u64,
            (i % 7) as f32 / 7.0,
            (i % 13) as f32 / 13.0,
            i as u64,
            ((i % 5) as f32) / 5.0,
            (i as u64).wrapping_mul(31),
            1750000000000 + i as u64,
            (i as f64) * 0.5,
        );
    }
    let tick_total = t.elapsed();
    println!(
        "D. tick() (1 szál, teljes page írás): {:?} /tick   ({} ticks, {:?})",
        tick_total / n_ticks,
        n_ticks,
        tick_total
    );

    // ── E. Az olvasási minta ára: a régi két külön olvasás vs. a bracketelt
    //        snapshot. Ugyazt a párt hasonlítjuk össze, amire a fogyasztónak
    //        szüksége van (stream fej + szuperblokk), nem tetszőleges műveletet. ──
    let t = Instant::now();
    for _ in 0..n_ops {
        let _ = s.read_stream();
        let _ = s.read_superblock();
    }
    let pair_read = t.elapsed();
    let t = Instant::now();
    for _ in 0..n_ops {
        let _ = s.read_snapshot();
    }
    let snap_read = t.elapsed();
    println!(
        "E1. read_stream + read_superblock:  {:?} /pair",
        pair_read / n_ops
    );
    println!(
        "E2. read_snapshot (bracketelt):     {:?} /pair   ({:.2}× az E1-hez képest)\n",
        snap_read / n_ops,
        snap_read.as_secs_f64() / pair_read.as_secs_f64()
    );

    // ── E'. Rétegenkénti fejrész-írás (ez az, ami most fájl-írás) ──
    let n_w = 100_000;
    let t = Instant::now();
    for i in 0..n_w {
        let mut h = s.read_hebbian();
        h.activation_count = i;
        h.delta_tail_offset = i as u64 * 96;
        s.write_hebbian(&h);
    }
    println!(
        "E. Hebbian fejrész r+w:             {:?} /op\n",
        t.elapsed() / n_w
    );

    // ── F. Emoció/attention/cache együttes (3 réteg r+w) ──
    let n3 = 100_000;
    let t = Instant::now();
    for i in 0..n3 {
        let mut e = s.read_emotion();
        e.current[i % 21] = (i % 11) as f32 / 11.0;
        e.total_updates = i as u64;
        s.write_emotion(&e);

        let mut a = s.read_attention();
        a.learned_weights[i % 7] = (i % 9) as f32 / 9.0;
        a.total_recalls = i as u32;
        s.write_attention(&a);

        let mut c = s.read_cache();
        c.total_hits = i as u32;
        c.total_misses = (i / 10) as u32;
        s.write_cache(&c);
    }
    println!(
        "F. Emotion+Attention+Cache r+w:     {:?} /3-réteg\n",
        t.elapsed() / n3.try_into().unwrap()
    );

    // ── I. Kontendált: n író × m olvasó, throughput + szakadási ráta ──
    // Az olvasó oracle-ai: a page *belsejéből* ellenőrzött páros
    // (`predicted_query_hash == !cycle`), tehát nem kell hozzá baseline. Az
    // initált de még publikálatlan page (minden nulla) NEM szakadás — ezért a
    // `fresh` számláló külön fut; nélküle a szálindítási latencia az első
    // konfigurációban 16–490 olvasót null-page-en találna, és szakadásnak
    // látszana. Ez a tanulság bekerült a tesztbe is.
    let npub = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .min(4);
    for (p, r) in [(1usize, 1usize), (2, 2), (npub, npub), (1, npub), (npub, 1)] {
        let (read_s, tick_s, snaps, torn, fresh) = contend(p, r, 200_000);
        println!(
            "I.  {} író × {} olvasó: {:>9.0} snapshot/s, {:>9.0} tick/s |              szakadás={}/{} nem-publikált={}",
            p, r, read_s, tick_s, torn, snaps, fresh
        );
    }
    println!();

    // ── G. sync (durability pont) ──
    let t = Instant::now();
    s.sync().unwrap();
    println!("G. sync (flush → disk):             {:?}", t.elapsed());

    // ── H. Reopen: minden állapot visszajön-e ──
    drop(s);
    let reopened = Spine::open(&spine_dir).unwrap();
    let stream = reopened.read_stream();
    let hebb = reopened.read_hebbian();
    let emo: EmotionHeader = reopened.read_emotion();
    let cache: CacheHeader = reopened.read_cache();
    println!("\nH. Reopen:");
    println!(
        "   cycle={} hebbian_activations={}",
        stream.cycle, hebb.activation_count
    );
    println!(
        "   emo_updates={} cache_hits={}",
        emo.total_updates, cache.total_hits
    );

    // ── Összefoglaló ──
    let legacy_reload = legacy_reload_first;
    let spine_reload = spine_open_warm;
    println!("\n───────────────────────────────────────────────────────");
    println!("ÖSSZEFOGLALÓ");
    println!("───────────────────────────────────────────────────────");
    println!(
        "Indulás (cold):    legacy {:>10.3?}  →  spine {:>10.3?}   ({:.1}×)",
        legacy_cold / n_cold,
        spine_cold,
        (legacy_cold / n_cold).as_secs_f64() / spine_cold.as_secs_f64()
    );
    println!(
        "Újranyitás (warm): legacy {:>10.3?}  →  spine {:>10.3?}   ({:.1}×)",
        legacy_reload,
        spine_reload,
        legacy_reload.as_secs_f64() / spine_reload.as_secs_f64()
    );
    println!(
        "11 fejrész olvasása: {:>9?}   tick írás (1 szál): {:>9?}",
        all_read / n_ops,
        tick_total / n_ticks
    );
    println!(
        "olvasási pár: régi {:>9?}  →  bracketelt {:>9?}   ({:.2}×)",
        pair_read / n_ops,
        snap_read / n_ops,
        pair_read.as_secs_f64() / snap_read.as_secs_f64()
    );
    println!(
        "Megjegyzés: a tick() két flock syscallot (spine.publish LOCK_EX + unlock) \
         és egy seqlock-abroncsot tartalmaz; a D. sor ezt tartalmazza. A lock \
         nélküli érték a BENCHMARKS.md Spine publish protocol szekciójában van, \
         mert azt a példából nem lehet váltani."
    );
    println!(
        "Spine fájl: {} oldal ({} KB @ {} B/oldal)",
        reopened.size_bytes() / PAGE_SIZE,
        reopened.size_bytes() / 1024,
        PAGE_SIZE
    );

    let _ = std::fs::remove_dir_all(&data_copy);
    let _ = std::fs::remove_dir_all(&spine_dir);
}

fn dir_size(p: &Path) -> u64 {
    let mut total = 0;
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            if let Ok(md) = e.metadata() {
                total += md.len();
            }
        }
    }
    total
}
