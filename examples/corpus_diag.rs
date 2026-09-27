//! Read-only offline diagnostics over an already-built index.
//!
//! Reads `eval_output/{microscope.bin,data.bin,meta.bin,embeddings.bin}` and
//! reports, without re-embedding anything:
//!   * how many embedded blocks sit at each depth;
//!   * neighbour structure around sampled D5 blocks (D5->D5 vs D5->D3/D4);
//!   * for high-cosine pairs, whether the *text* actually agrees -- a high
//!     cosine is not the same fact, so names, numbers and negation are reported
//!     separately from the raw score.
//!
//! Usage: cargo run --release --features native,embeddings --example corpus_diag
//!        (expects MICROSCOPE_CONFIG to point at eval_config.toml)

use microscope_memory::config::Config;
use microscope_memory::embedding_index::EmbeddingIndex;
use microscope_memory::reader::MicroscopeReader;
use std::path::Path;

fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

/// Jaccard overlap of the token sets, plus the tokens unique to `a`.
fn token_report(a: &str, b: &str) -> (f32, Vec<String>) {
    let ta: std::collections::HashSet<String> = tokenize(a).into_iter().collect();
    let tb: std::collections::HashSet<String> = tokenize(b).into_iter().collect();
    let inter = ta.intersection(&tb).count() as f32;
    let union = ta.union(&tb).count().max(1) as f32;
    let only_a: Vec<String> = ta.difference(&tb).cloned().collect();
    (inter / union, only_a)
}

/// Digits (numbers) that appear in `a` but not in `b`.
fn number_conflicts(a: &str, b: &str) -> Vec<String> {
    let tb = tokenize(b);
    tokenize(a)
        .into_iter()
        .filter(|t| t.chars().any(|c| c.is_ascii_digit()) && !tb.contains(t))
        .collect()
}

fn has_negation(s: &str) -> bool {
    tokenize(s).iter().any(|w| {
        matches!(w.as_str(), "not" | "no" | "never" | "none" | "nor" | "without")
    })
}

fn truncate(s: &str) -> String {
    if s.chars().count() > 90 {
        s.chars().take(90).collect::<String>() + "..."
    } else {
        s.to_string()
    }
}

fn main() {
    let cfg_path = std::env::var("MICROSCOPE_CONFIG").unwrap_or_else(|_| "eval_config.toml".into());
    let cfg = Config::load(&cfg_path).expect("load MICROSCOPE_CONFIG");
    let out = Path::new(&cfg.paths.output_dir);
    let reader = MicroscopeReader::open(&cfg).expect("open reader");
    let eidx = EmbeddingIndex::open(&out.join("embeddings.bin")).expect("open embeddings");

    println!("== corpus ==");
    println!(
        "blocks={} embedded={} dim={} max_depth={}",
        reader.block_count,
        eidx.block_count(),
        eidx.dim(),
        eidx.max_depth()
    );

    // 1. embedded blocks per depth
    let mut by_depth = [0usize; 9];
    let mut by_depth_all = [0usize; 9];
    for i in 0..reader.block_count {
        let d = reader.header(i).depth as usize;
        if d < 9 {
            by_depth_all[d] += 1;
        }
    }
    for id in eidx.all_block_ids() {
        let i = *id as usize;
        if i < reader.block_count {
            let d = reader.header(i).depth as usize;
            if d < 9 {
                by_depth[d] += 1;
            }
        }
    }
    println!("\n== embedded blocks by depth (all blocks in parens) ==");
    for d in 0..9 {
        if by_depth_all[d] > 0 {
            println!(
                "  D{}: {:7} embedded  ({:7} total)",
                d, by_depth[d], by_depth_all[d]
            );
        }
    }
    report_neighbours(&reader, &eidx);
    report_degenerate(&reader, &eidx);
}

/// How much of the index is degenerate text, and how much of a top-1024 does
/// it occupy? The neighbour report showed the highest-cosine pairs are the
/// literal string "LLM" -- 3-character D5 fragments. If those crowd the result
/// list, excluding them is the corpus-level fix; if they do not, the ablation
/// is pointless. This bounds the potential gain before any ranking change.
///
/// Note the caution the text report already raised: of 40,000 sampled
/// high-cosine pairs, 1,666 differ in numbers and 245 differ in negation, so a
/// filter keyed on cosine similarity alone would merge contradictory facts.
/// A length filter is different -- it removes fragments that are not facts at
/// all -- but the contradiction count is why the filter must be length-based.
fn report_degenerate(reader: &MicroscopeReader, eidx: &EmbeddingIndex) {
    println!("\n== degenerate-text census (embedded blocks only) ==");
    let mut short_total = 0usize;
    let mut short_d5 = 0usize;
    let mut per_len: std::collections::BTreeMap<usize, usize> = Default::default();
    let mut examples: std::collections::BTreeMap<usize, Vec<String>> = Default::default();
    for id in eidx.all_block_ids() {
        let i = *id as usize;
        if i >= reader.block_count {
            continue;
        }
        let t = reader.text(i);
        let n = t.chars().count();
        if n <= 16 {
            short_total += 1;
            if reader.header(i).depth == 5 {
                short_d5 += 1;
            }
            *per_len.entry(n).or_default() += 1;
            let e = examples.entry(n).or_default();
            if e.len() < 3 {
                e.push(truncate(t));
            }
        }
    }
    println!("  blocks with <=16 chars: {} (D5: {})", short_total, short_d5);
    println!("\n  len : count : examples");
    for (len, count) in per_len.iter() {
        let ex = examples
            .get(len)
            .map(|v| v.join(" | "))
            .unwrap_or_default();
        println!("  {:3} : {:5} : {}", len, count, ex);
    }
}

/// 2 + 3: neighbour structure around sampled D5 blocks, and whether
/// high-cosine pairs actually denote the same fact.
///
/// Neighbour search only: each probe takes its own top-N by cosine, which is
/// O(N) per probe rather than the 46k x 46k full pairwise scan.
fn report_neighbours(reader: &MicroscopeReader, eidx: &EmbeddingIndex) {
    let step = (eidx.block_count() / 400).max(1);
    let mut d5_probes = 0usize;
    let mut nbr_depth = [0usize; 9];
    let mut sim_by_pair: std::collections::HashMap<(u8, u8), (f64, usize)> =
        std::collections::HashMap::new();
    let mut samples: Vec<(f32, usize, usize)> = Vec::new();

    for pos in (0..eidx.block_count()).step_by(step) {
        let qid = eidx.all_block_ids()[pos] as usize;
        if qid >= reader.block_count {
            continue;
        }
        let qd = reader.header(qid).depth;
        if qd != 5 {
            continue;
        }
        d5_probes += 1;
        let Some(qe) = eidx.embedding(qid) else { continue };
        for (sim, nid) in eidx.search(qe, 256) {
            if nid >= reader.block_count || nid == qid {
                continue;
            }
            let nd = reader.header(nid).depth as usize;
            if nd < 9 {
                nbr_depth[nd] += 1;
            }
            let e = sim_by_pair.entry((qd, nd as u8)).or_insert((0.0, 0));
            e.0 += sim as f64;
            e.1 += 1;
            if samples.len() < 40000 && sim >= 0.95 {
                samples.push((sim, qid, nid));
            }
        }
    }

    println!("\n== neighbour depth profile of sampled D5 probes (n={}) ==", d5_probes);
    for d in 0..9 {
        if nbr_depth[d] > 0 {
            println!("  D{}: {:8}", d, nbr_depth[d]);
        }
    }
    println!("\n== mean cosine by (probe depth, neighbour depth) ==");
    let mut keys: Vec<_> = sim_by_pair.keys().cloned().collect();
    keys.sort_unstable();
    for k in keys {
        let (sum, n) = sim_by_pair[&k];
        println!("  D{} -> D{}: mean {:.4}  (n={})", k.0, k.1, sum / n as f64, n);
    }

    println!("\n== high-cosine pairs (>=0.95): text agreement ==");
    let (mut b098, mut b95) = (0usize, 0usize);
    let (mut same_text, mut num_conflicts, mut neg_conflicts) = (0usize, 0usize, 0usize);
    let mut shown = 0usize;
    for (sim, a, b) in &samples {
        if *sim >= 0.98 {
            b098 += 1;
        } else {
            b95 += 1;
        }
        let (ja, only_a) = token_report(reader.text(*a), reader.text(*b));
        if reader.text(*a) == reader.text(*b) {
            same_text += 1;
        }
        let nc = number_conflicts(reader.text(*a), reader.text(*b));
        if !nc.is_empty() {
            num_conflicts += 1;
        }
        if has_negation(reader.text(*a)) != has_negation(reader.text(*b)) {
            neg_conflicts += 1;
        }
        if *sim >= 0.98 && shown < 12 {
            println!("  sim={:.4} jaccard={:.2}", sim, ja);
            println!("     A[{}]: {}", reader.header(*a).depth, truncate(reader.text(*a)));
            println!("     B[{}]: {}", reader.header(*b).depth, truncate(reader.text(*b)));
            if !only_a.is_empty() {
                println!("     only-in-A: {:?}", &only_a[..only_a.len().min(8)]);
            }
            if !nc.is_empty() {
                println!("     NUMBER CONFLICT: {:?}", nc);
            }
            if has_negation(reader.text(*a)) != has_negation(reader.text(*b)) {
                println!("     NEGATION CONFLICT");
            }
            shown += 1;
        }
    }
    println!("\n  sampled pairs >=0.98    : {}", b098);
    println!("  sampled pairs 0.95-0.98 : {}", b95);
    println!("  identical text          : {}", same_text);
    println!("  differing numbers       : {}", num_conflicts);
    println!("  differing negation      : {}", neg_conflicts);
    println!("\n(total sampled high-cosine pairs: {})", samples.len());
}
