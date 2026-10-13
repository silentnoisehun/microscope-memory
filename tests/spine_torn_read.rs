//! Torn-read regression for the Spine publish path.
//!
//! Three setups, the three ways a reader can end up with a stream header that
//! does not belong to the superblock payload it read next to it:
//!
//!   * [`torn_read_two_handles`] — writer and reader as separate `Spine`
//!     handles in one process. Each handle owns its own `Mutex<MmapMut>`, so
//!     the two locks never exclude each other and no happens-before relation
//!     exists between them. This is the case a `Release`-on-drop story is
//!     *not* enough for.
//!   * [`torn_read_two_writers`] — two publishers competing, so a reader can
//!     also catch the middle of a 16 KiB page copy.
//!   * [`torn_read_two_processes`] — the writer is another process, which is
//!     what `spine.bin` is mapped shared for in the first place.
//!
//! The whole-store tests additionally stamp all eleven headers in one batch,
//! including the Emotion fields beyond byte 64. They exercise shared-handle
//! threads, independent handles, and a child process; publish progress and
//! distinct observed sequences prevent a quiescent snapshot from passing.
//!
//! Every test asserts a hard zero on violations, and every test also asserts
//! that it actually raced a live writer — without that second half, "0
//! violations" is just a reader that nobody competed with. Two tests do it
//! with an unbracketed control read (the pre-fix read path, which must violate
//! on the same file at the same moment); the two-publisher test asserts publish
//! progress instead, because its control turned out to be scheduling-sensitive
//! — see the comment in [`torn_read_two_writers`]. Publishers keep running
//! until the readers are done for the same reason.
//!
//! What the four counters mean, since one is easy to over-read:
//!
//!   * `mixed` and `torn_page` are properties of the **file**: a snapshot whose
//!     parts came from two publishes, or a page copied while it was being
//!     written.
//!   * `starved` is a property of the **reader**: `read_snapshot` must never
//!     give up while a writer is alive.
//!   * `open_window` is a property of the **read path**, not of the writer:
//!     `read_snapshot` refuses an odd counter, so it can never hand one out,
//!     and the counter exists so that dropping that check later cannot be
//!     silent. Whether a *writer* can leave a window open behind it is a
//!     different question, answered by the unit tests in `src/spine.rs`
//!     (`test_stalled_publish_window_is_recoverable`), not by this file.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use microscope_memory::spine::Spine;

/// These tests each spin up a publisher and a reader that never sleep. Run
/// concurrently on a small machine they oversubscribe it badly enough that the
/// spawned writer barely gets scheduled, so they take turns instead. A load
/// test that starves its own subject is measuring the scheduler.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Writes per publisher. Large enough that the unbracketed control read is
/// bound to land inside a publish window on any machine that can run the
/// writer loop at all.
const WRITES: u64 = 60_000;
/// Cycle values start here so that `0` (never published) can never be
/// confused with a real publish.
const CYCLE_BASE: u64 = 1_000;

struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "m1c_spine_torn_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Every publisher takes the *next* cycle value from the file rather than from
/// its own loop counter. That is what makes the invariant in
/// [`check_bracketed`] hold across publishers that do not know about each
/// other: one publish advances `cycle` and the publish counter by exactly one
/// each, so their difference is the same for every publish by every process.
fn next_cycle(spine: &Spine) -> u64 {
    spine
        .read_snapshot()
        .map(|s| s.stream.cycle.wrapping_add(1))
        .unwrap_or(CYCLE_BASE)
}

/// Publish such that a reader can tell a complete snapshot from a mix: the
/// cycle value and its complement live in the same page, and the cycle value
/// advances exactly one per publish, as does the publish counter.
fn publish_at(spine: &Spine, cycle: u64) {
    spine.tick(
        cycle,
        0.5,
        0.25,
        !cycle,
        1.0,
        cycle,
        1_700_000_000_000 + cycle,
        cycle as f64,
    );
}

/// Take the next cycle from the file and publish it.
fn publish(spine: &Spine) {
    publish_at(spine, next_cycle(spine));
}

/// What a snapshot has to satisfy to be trustworthy.
#[derive(Default)]
struct Violations {
    /// Stream header and superblock payload from different publishes.
    mixed: AtomicU64,
    /// Half-copied page: the cycle value and its own complement disagree.
    torn_page: AtomicU64,
    /// A publish counter caught odd, i.e. read inside a window.
    open_window: AtomicU64,
    /// `read_snapshot` gave up instead of retrying.
    starved: AtomicU64,
}

impl Violations {
    fn total(&self) -> u64 {
        self.mixed.load(Ordering::Relaxed)
            + self.torn_page.load(Ordering::Relaxed)
            + self.open_window.load(Ordering::Relaxed)
            + self.starved.load(Ordering::Relaxed)
    }
}

/// `cycle - tick_seq` must be the same constant for every publish, because
/// both advance by exactly one per publish. A mixed read breaks it — no
/// timing luck involved, which is what makes this a regression test rather
/// than a race lottery.
fn check_bracketed(reader: &Spine, baseline_delta: u64, out: &Violations, reads: u64) {
    let mut seen_seq = 0u64;
    for _ in 0..reads {
        let Some(snap) = reader.read_snapshot() else {
            out.starved.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let delta = snap.stream.cycle.wrapping_sub(snap.tick_seq);
        // cycle and !cycle in one page: any complete read satisfies this.
        if snap.stream.predicted_query_hash != !snap.stream.cycle {
            out.torn_page.fetch_add(1, Ordering::Relaxed);
        }
        if snap.sequence & 1 == 1 {
            out.open_window.fetch_add(1, Ordering::Relaxed);
        }
        if delta != baseline_delta {
            out.mixed.fetch_add(1, Ordering::Relaxed);
        }
        // The counter is a global publish ordinal: it never goes backwards,
        // and neither does a single writer's cycle.
        if snap.tick_seq < seen_seq {
            out.mixed.fetch_add(1, Ordering::Relaxed);
        }
        seen_seq = snap.tick_seq.max(seen_seq);
    }
}

/// The pre-fix read path, kept on purpose: two separate locked reads with
/// nothing bracketing them. `cycle` can come from one publish and `tick_seq`
/// from the next, which is the mixing the seqlock exists to remove.
fn check_unbracketed(reader: &Spine, baseline_delta: u64, out: &Violations, reads: u64) {
    for _ in 0..reads {
        let stream = reader.read_stream();
        let Some(sb) = reader.read_superblock() else {
            continue;
        };
        if stream
            .cycle
            .wrapping_sub(sb.tick_seq)
            .wrapping_sub(baseline_delta)
            != 0
        {
            out.mixed.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn report(tag: &str, v: &Violations, reads: u64) -> String {
    format!(
        "{tag}: reads={reads} mixed={} torn_page={} open_window={} starved={}",
        v.mixed.load(Ordering::Relaxed),
        v.torn_page.load(Ordering::Relaxed),
        v.open_window.load(Ordering::Relaxed),
        v.starved.load(Ordering::Relaxed),
    )
}

/// Child-process entry point: publish until told to stop. Returns `true` when
/// this process *is* the child, so the caller returns from its test body.
fn maybe_run_as_writer() -> bool {
    let Ok(dir) = std::env::var("MICROSCOPE_SPINE_TORN_DIR") else {
        return false;
    };
    let stop = std::env::var("MICROSCOPE_SPINE_TORN_STOPFILE").ok();
    let spine = Spine::open(Path::new(&dir)).expect("child: open spine");
    // Bounded so a child whose parent died mid-test cannot spin forever.
    for _ in 0..(WRITES * 20) {
        if let Some(path) = &stop {
            if Path::new(path).exists() {
                break;
            }
        }
        let cycle = next_cycle(&spine);
        if std::env::var_os("MICROSCOPE_SPINE_TORN_WHOLE_STORE").is_some() {
            publish_all_headers(&spine, cycle);
        } else {
            publish_at(&spine, cycle);
        }
    }
    true
}

#[test]
fn torn_read_two_handles() {
    let _alone = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = TmpDir::new("handles");
    let writer = Arc::new(Spine::open(dir.path()).expect("open writer"));
    let reader = Arc::new(Spine::open_ro(dir.path()).expect("open reader"));

    // One quiescent publish to fix the constant that every later read must
    // reproduce.
    publish(&writer);
    let baseline = reader.read_snapshot().expect("baseline snapshot");
    let delta = baseline.stream.cycle.wrapping_sub(baseline.tick_seq);
    assert_eq!(
        reader
            .read_stream()
            .cycle
            .wrapping_sub(reader.read_superblock().unwrap().tick_seq),
        delta,
        "the unbracketed read must agree while the file is idle"
    );

    let stop = Arc::new(AtomicBool::new(false));
    let violations = Arc::new(Violations::default());
    let control = Arc::new(Violations::default());

    let writer_thread = {
        let (writer, stop) = (writer.clone(), stop.clone());
        std::thread::spawn(move || {
            // Until the readers are done, not a fixed count: the control read
            // has to have a live publish to trip over on every attempt, or a
            // writer that finishes first turns the test green and empty.
            while !stop.load(Ordering::Relaxed) {
                publish(&writer);
            }
        })
    };
    let reader_thread = {
        let (violations, reader, stop) = (violations.clone(), reader.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut done = 0;
            while done < WRITES {
                check_bracketed(&reader, delta, &violations, 500);
                done += 500;
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            done
        })
    };
    let control_thread = {
        let (control, reader, stop) = (control.clone(), reader.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut done = 0;
            while done < WRITES {
                check_unbracketed(&reader, delta, &control, 500);
                done += 500;
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            done
        })
    };

    drop(writer); // the writer thread owns the handle from here on
    let reads = reader_thread.join().expect("reader thread");
    stop.store(true, Ordering::Relaxed);
    writer_thread.join().expect("writer thread");
    control_thread.join().expect("control thread");

    let summary = report("two handles", &violations, reads);
    assert_eq!(
        violations.total(),
        0,
        "torn reads survived the seqlock: {summary}"
    );
    // Sensitivity check: on the same file, at the same moment, the read path
    // from before the publish window *does* mix. If this ever comes back 0,
    // the test above is not measuring anything.
    assert!(
        control.mixed.load(Ordering::Relaxed) > 0,
        "control read found no mixing, so this run proves nothing: {}",
        report("control", &control, reads)
    );
}

#[test]
fn torn_read_two_writers() {
    let _alone = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = TmpDir::new("writers");
    let spine_a = Arc::new(Spine::open(dir.path()).expect("open writer a"));
    let spine_b = Arc::new(Spine::open(dir.path()).expect("open writer b"));
    let reader = Arc::new(Spine::open_ro(dir.path()).expect("open reader"));

    publish(&spine_a);
    let violations = Arc::new(Violations::default());
    let stop = Arc::new(AtomicBool::new(false));

    let writers: Vec<_> = [(spine_a.clone(), 0u64), (spine_b.clone(), 1u64)]
        .into_iter()
        .map(|(sp, parity)| {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut seq = 0u64;
                // Bounded, but only ever ended by `stop` in practice: see the
                // note in `torn_read_two_handles` about a quiet file.
                while !stop.load(Ordering::Relaxed) && seq < WRITES {
                    // Two publishers, one counter: `cycle` keeps the parity of
                    // its writer and increments by 2, so a complete page always
                    // satisfies `cycle % 2 == parity` and `hash == !cycle`.
                    let cycle = CYCLE_BASE + seq * 2 + parity;
                    sp.tick(cycle, 0.5, 0.25, !cycle, 1.0, seq, 0, cycle as f64);
                    seq += 1;
                }
                seq
            })
        })
        .collect();

    let seq_before = reader
        .read_snapshot()
        .expect("snapshot before the writers start")
        .tick_seq;

    let reader_thread = {
        let (violations, reader, stop) = (violations.clone(), reader.clone(), stop.clone());
        std::thread::spawn(move || {
            let mut reads = 0u64;
            let mut max_seq = 0u64;
            while reads < WRITES * 2 {
                // No baseline delta here: two publishers make that constant
                // meaningless. What must still hold for every publish, from
                // either of them, is that the page is whole and that the
                // payload the reader decoded belongs to a closed window.
                let Some(snap) = reader.read_snapshot() else {
                    violations.starved.fetch_add(1, Ordering::Relaxed);
                    continue;
                };
                if snap.stream.predicted_query_hash != !snap.stream.cycle {
                    violations.torn_page.fetch_add(1, Ordering::Relaxed);
                }
                if snap.sequence & 1 == 1 {
                    violations.open_window.fetch_add(1, Ordering::Relaxed);
                }
                if snap.tick_seq * 2 != snap.sequence {
                    // The payload names the window it was published under.
                    violations.mixed.fetch_add(1, Ordering::Relaxed);
                }
                reads += 1;
                max_seq = snap.tick_seq.max(max_seq);
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            (reads, max_seq)
        })
    };

    let (reads, max_seq) = reader_thread.join().expect("reader thread");
    stop.store(true, Ordering::Relaxed);
    for w in writers {
        w.join().expect("writer thread");
    }

    let summary = report("two writers", &violations, reads);
    assert_eq!(
        violations.total(),
        0,
        "torn reads survived the seqlock: {summary}"
    );
    // Sensitivity check. The unbracketed half-page oracle the other two tests
    // use is scheduling-sensitive with two publishers and two readers sharing
    // one handle: it measured 1 258-1 362 torn pages per 60 000 reads with a
    // single control thread and 0 per 120 000 once the bracketed reader shares
    // the handle, so an assert on it would be a flake on a busy machine. What
    // cannot be vacuous is progress: the counter is global, so a reader that
    // raced a live writer must have watched it advance by thousands.
    assert!(
        max_seq > seq_before + 1_000,
        "the publish counter only moved from {seq_before} to {max_seq}, so the \
         readers never raced a writer and this run proves nothing: {}",
        report("two writers", &violations, reads)
    );
}

#[test]
fn torn_read_two_processes() {
    let _alone = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if maybe_run_as_writer() {
        return; // this process was spawned to publish, not to assert
    }
    let dir = TmpDir::new("procs");
    let writer = Spine::open(dir.path()).expect("open writer");
    publish(&writer);
    let reader = Spine::open_ro(dir.path()).expect("open reader");
    let baseline = reader.read_snapshot().expect("baseline snapshot");
    let delta = baseline.stream.cycle.wrapping_sub(baseline.tick_seq);

    let stopfile = dir.path().join("stop");
    let _ = std::fs::remove_file(&stopfile);
    let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"))
        // A substring filter, not `--exact`: the child has to match this test
        // whether it lives at the test target's root (here) or inside a module
        // (any harness that inlines it), and it exits through the env-var
        // branch below before running anything else.
        .arg("torn_read_two_processes")
        .arg("--nocapture")
        .env("MICROSCOPE_SPINE_TORN_DIR", dir.path())
        .env("MICROSCOPE_SPINE_TORN_STOPFILE", &stopfile)
        // libtest's single-threaded setting keeps the child out of the
        // harness's business; it only runs the loop above and exits.
        .env("RUST_TEST_THREADS", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn writer child");

    let violations = Violations::default();
    let control = Violations::default();
    let mut reads = 0u64;
    let mut control_reads = 0u64;
    let mut control_rounds = 0u64;
    let started = std::time::Instant::now();
    while reads < WRITES && started.elapsed() < std::time::Duration::from_secs(30) {
        check_bracketed(&reader, delta, &violations, 500);
        reads += 500;
        // The control needs the writer's own handle here: a reader in another
        // process has nothing to serialise against, which is the point.
        check_unbracketed(&writer, delta, &control, 500);
        control_reads += 500;
        control_rounds += 1;
    }
    let _ = std::fs::File::create(&stopfile);
    let _ = child.wait();

    // The child publishes until told to stop, so the file must have moved
    // well past the parent's own single publish. Without this, a child that
    // never started would make the assertions below vacuously true.
    let last = reader.read_snapshot().expect("final snapshot");
    assert!(
        last.stream.cycle > CYCLE_BASE + 100,
        "the writer child only reached cycle {}, so this run proves nothing",
        last.stream.cycle
    );
    assert!(
        reads >= 1_000,
        "the child writer never got going, so this run proves nothing ({reads} reads, \
         {control_rounds} control rounds)"
    );
    let summary = report("two processes", &violations, reads);
    assert_eq!(
        violations.total(),
        0,
        "torn reads survived the seqlock: {summary}"
    );
    assert!(
        control.mixed.load(Ordering::Relaxed) > 0,
        "cross-process control found no mixing, so this run proves nothing: {}",
        report("control", &control, control_reads)
    );
}

/// Stamp every encoded header with one cycle; the last Emotion fields test
/// the part of the prefix beyond byte 64 as well as cross-page agreement.
fn publish_all_headers(spine: &Spine, cycle: u64) {
    let timestamp = 1_700_000_000_000 + cycle;
    let mut p = spine.begin_publish();
    p.tick_stream(
        cycle,
        0.5,
        0.25,
        !cycle,
        1.0,
        cycle,
        timestamp,
        cycle as f64,
        timestamp as i64,
    );
    // Widen the partial-update interval deliberately. Readers must reject it.
    std::thread::yield_now();
    let mut h = p.read_hebbian();
    h.activation_count = cycle as u32;
    p.write_hebbian(&h);
    let mut a = p.read_attention();
    a.total_recalls = cycle as u32;
    p.write_attention(&a);
    let mut e = p.read_emotion();
    e.current[20] = cycle as f32;
    e.total_updates = cycle;
    e.last_update = timestamp;
    p.write_emotion(&e);
    let mut r = p.read_resonance();
    r.instance_id = cycle;
    p.write_resonance(&r);
    let mut g = p.read_graph();
    g.node_count = cycle as u32;
    p.write_graph(&g);
    let mut c = p.read_cache();
    c.total_predictions = cycle as u32;
    p.write_cache(&c);
    let mut ar = p.read_archetype();
    ar.active_count = cycle as u32;
    p.write_archetype(&ar);
    let mut mi = p.read_mirror();
    mi.total_echoes = cycle;
    p.write_mirror(&mi);
    let mut me = p.read_merkle();
    me.root_hash = !cycle;
    p.write_merkle(&me);
    let mut meta = p.read_meta();
    meta.promotion_count = cycle;
    p.write_meta(&meta);
    p.commit();
}

fn store_is_consistent(s: &microscope_memory::spine::StoreSnapshot) -> bool {
    let cycle = s.stream.cycle;
    s.sequence & 1 == 0
        && s.tick_seq == s.sequence / 2
        && s.stream.predicted_query_hash == !cycle
        && s.stream.updated_ms == s.epoch_ms as u64
        && s.hebbian.activation_count == cycle as u32
        && s.attention.total_recalls == cycle as u32
        && s.emotion.current[20] == cycle as f32
        && s.emotion.total_updates == cycle
        && s.emotion.last_update == s.stream.updated_ms
        && s.resonance.instance_id == cycle
        && s.graph.node_count == cycle as u32
        && s.cache.total_predictions == cycle as u32
        && s.archetype.active_count == cycle as u32
        && s.mirror.total_echoes == cycle
        && s.merkle.root_hash == !cycle
        && s.meta.promotion_count == cycle
}

#[test]
fn torn_read_whole_store_shared_and_independent_handles() {
    let _alone = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = TmpDir::new("whole_store");
    let shared = Arc::new(Spine::open(dir.path()).expect("shared writer"));
    let independent = Arc::new(Spine::open(dir.path()).expect("independent writer"));
    if !shared.publish_lock_is_advisory() {
        eprintln!("skipping competing handles: no advisory publisher lock");
        return;
    }
    publish_all_headers(&shared, CYCLE_BASE);
    let reader = Spine::open_ro(dir.path()).expect("reader");
    let before = reader.read_store_snapshot().unwrap().sequence;
    let stop = Arc::new(AtomicBool::new(false));
    let barrier = Arc::new(std::sync::Barrier::new(4));
    let mut writers = Vec::new();
    // Two threads share the same flock descriptor; the third has its own.
    // This catches taking flock before the same-handle mapping mutex.
    for (id, writer) in [shared.clone(), shared, independent]
        .into_iter()
        .enumerate()
    {
        let stop = stop.clone();
        let barrier = barrier.clone();
        writers.push(std::thread::spawn(move || {
            barrier.wait();
            for n in 1..=WRITES * 20 {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                publish_all_headers(&writer, CYCLE_BASE + n * 3 + id as u64);
            }
        }));
    }
    barrier.wait();
    let mut failures = 0;
    let mut max_sequence = before;
    let mut observed = std::collections::BTreeSet::new();
    for _ in 0..10_000 {
        match reader.read_store_snapshot() {
            Some(s) => {
                failures += u64::from(!store_is_consistent(&s));
                max_sequence = max_sequence.max(s.sequence);
                observed.insert(s.sequence);
            }
            None => failures += 1,
        }
    }
    stop.store(true, Ordering::Release);
    for writer in writers {
        writer.join().expect("writer thread");
    }
    assert!(
        max_sequence > before + 100,
        "publishers did not make progress"
    );
    assert!(
        observed.len() > 10,
        "reader did not observe live publishing"
    );
    assert_eq!(failures, 0, "mixed, odd or unavailable store snapshots");
}

#[test]
fn torn_read_whole_store_two_processes() {
    let _alone = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    if maybe_run_as_writer() {
        return;
    }
    let dir = TmpDir::new("whole_store_procs");
    let writer = Spine::open(dir.path()).expect("writer");
    if !writer.publish_lock_is_advisory() {
        eprintln!("skipping competing processes: no advisory publisher lock");
        return;
    }
    publish_all_headers(&writer, CYCLE_BASE);
    let reader = Spine::open_ro(dir.path()).expect("reader");
    let before = reader.read_store_snapshot().unwrap().sequence;
    let stopfile = dir.path().join("stop");
    let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .arg("torn_read_whole_store_two_processes")
        .arg("--nocapture")
        .env("MICROSCOPE_SPINE_TORN_DIR", dir.path())
        .env("MICROSCOPE_SPINE_TORN_STOPFILE", &stopfile)
        .env("MICROSCOPE_SPINE_TORN_WHOLE_STORE", "1")
        .env("RUST_TEST_THREADS", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn writer child");
    let mut failures = 0;
    let mut max_sequence = before;
    let mut observed = std::collections::BTreeSet::new();
    let started = std::time::Instant::now();
    let mut reads = 0;
    // Keep reading until the child demonstrably published during the reads.
    while (reads < 10_000 || observed.len() < 100)
        && started.elapsed() < std::time::Duration::from_secs(30)
    {
        match reader.read_store_snapshot() {
            Some(s) => {
                failures += u64::from(!store_is_consistent(&s));
                max_sequence = max_sequence.max(s.sequence);
                observed.insert(s.sequence);
            }
            None => failures += 1,
        }
        reads += 1;
    }
    std::fs::File::create(&stopfile).expect("stop writer child");
    let status = child.wait().expect("wait writer child");
    assert!(status.success(), "child publisher failed: {status}");
    assert!(max_sequence > before + 100, "child did not publish");
    assert!(
        observed.len() >= 100,
        "reader did not observe live publishing"
    );
    assert!(reads >= 10_000, "too few store snapshots");
    assert_eq!(failures, 0, "mixed, odd or unavailable store snapshots");
}
