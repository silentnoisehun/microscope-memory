//! Uncontended Spine publish batching and validated header-copy benchmark.
//! Run: cargo run --release --example bench_spine_publish
//! No private data directory is required. These are API microbenchmarks, not
//! end-to-end consciousness-stream timings or durability measurements.

use microscope_memory::spine::Spine;
use std::hint::black_box;
use std::time::Instant;

const ITERATIONS: u64 = 20_000;

// The same eight aggregate updates used by the consciousness stream. Each
// invocation reads through, and writes through, the supplied handle/guard.
macro_rules! update_headers {
    ($handle:expr, $cycle:expr) => {{
        let mut h = $handle.read_hebbian();
        h.activation_count = $cycle as u32;
        $handle.write_hebbian(&h);
        let mut a = $handle.read_attention();
        a.total_recalls = $cycle as u32;
        $handle.write_attention(&a);
        let mut e = $handle.read_emotion();
        e.total_updates = $cycle;
        $handle.write_emotion(&e);
        let mut r = $handle.read_resonance();
        r.instance_id = $cycle;
        $handle.write_resonance(&r);
        let mut g = $handle.read_graph();
        g.node_count = $cycle as u32;
        $handle.write_graph(&g);
        let mut c = $handle.read_cache();
        c.total_predictions = $cycle as u32;
        $handle.write_cache(&c);
        let mut ar = $handle.read_archetype();
        ar.active_count = $cycle as u32;
        $handle.write_archetype(&ar);
        let mut mi = $handle.read_mirror();
        mi.total_echoes = $cycle;
        $handle.write_mirror(&mi);
    }};
}

fn measure(label: &str, mut operation: impl FnMut(u64)) {
    for cycle in 1..=1_000 {
        operation(black_box(cycle));
    }
    let start = Instant::now();
    for cycle in 1..=ITERATIONS {
        operation(black_box(cycle));
    }
    let ns = start.elapsed().as_nanos() as f64 / ITERATIONS as f64;
    println!("{label:42} {ns:10.1} ns/op ({ITERATIONS} iterations)");
}

fn main() -> std::io::Result<()> {
    let dir = std::env::temp_dir().join(format!(
        "m1c_spine_publish_bench_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let spine = Spine::open(&dir)?;
    println!(
        "Uncontended; advisory lock: {}",
        spine.publish_lock_is_advisory()
    );
    measure("tick (stream only)", |cycle| {
        spine.tick(cycle, 0.5, 0.25, !cycle, 1.0, cycle, cycle, cycle as f64);
    });
    measure("tick + 8 separate header updates", |cycle| {
        spine.tick(cycle, 0.5, 0.25, !cycle, 1.0, cycle, cycle, cycle as f64);
        update_headers!(spine, cycle);
    });
    measure("tick + 8 updates in one Publish batch", |cycle| {
        let mut p = spine.begin_publish();
        p.tick_stream(
            cycle,
            0.5,
            0.25,
            !cycle,
            1.0,
            cycle,
            cycle,
            cycle as f64,
            1234,
        );
        update_headers!(p, cycle);
        p.commit();
    });
    measure("11 separate header reads (not validated)", |_| {
        black_box((
            spine.read_stream(),
            spine.read_hebbian(),
            spine.read_attention(),
            spine.read_emotion(),
            spine.read_resonance(),
            spine.read_graph(),
            spine.read_cache(),
            spine.read_archetype(),
            spine.read_mirror(),
            spine.read_merkle(),
            spine.read_meta(),
        ));
    });
    measure("read_store_snapshot (validated)", |_| {
        black_box(spine.read_store_snapshot().expect("valid store"));
    });
    measure("read_snapshot (stream + record)", |_| {
        black_box(spine.read_snapshot().expect("valid stream"));
    });
    drop(spine);
    std::fs::remove_dir_all(dir)
}
