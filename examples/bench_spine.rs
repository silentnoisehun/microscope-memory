//! Spine benchmark — a rétegek önálló fájljaikra töltése vs. egyetlen mmap.
//!
//! Két forgatókönyvet mér:
//!   A. COLD — a réteges `load_or_init()` lánc éles adatkönyvtárral (másolat)
//!   B. SPINE — ugyanaz az állapot egy `spine.bin` mmap-ból
//! plusz a hot path olvasás/írás késleltetése rétegenként.
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

    // ── C. Hot path: mind a 11 réteg fejrészének olvasása ──
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
        "C. 11 fejrész olvasása:              {:?} /all-11  ({} iters, {:?})",
        all_read / n_ops,
        n_ops,
        all_read
    );

    // ── D. Hot path: tick írás (amit a stream ciklus csinál 10 Hz-en) ──
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
        "D. tick() (teljes StreamPage írás):  {:?} /tick   ({} ticks, {:?})",
        tick_total / n_ticks,
        n_ticks,
        tick_total
    );

    // ── E. Rétegenkénti fejrész-írás (ez az, ami most fájl-írás) ──
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
        t.elapsed() / n3
            .try_into()
            .unwrap()
    );

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
    println!("   cycle={} hebbian_activations={}", stream.cycle, hebb.activation_count);
    println!("   emo_updates={} cache_hits={}", emo.total_updates, cache.total_hits);

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
        "11 fejrész olvasása: {:>9?}   teljes tick írás: {:>9?}",
        all_read / n_ops,
        tick_total / n_ticks
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
