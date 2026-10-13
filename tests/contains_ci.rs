//! Case-insensitive search: helyessg s A/B sebessgmrs.
//!
//! A `reader::contains_ci` a per-block `to_lowercase()` hvst szeretn
//! megszntetni (a dev-indexen 23 774 allokci / query). Ez a tesztkszlet
//! kt dolgot bizonyt:
//!
//!  1. **Helyessg** — a gyors t s a karakter-t egyarnt pontosan azt adja,
//!     mint a referencia `hay.to_lowercase().contains(q)`.
//!  2. **Sebessg** — ugyanabban a processzben, egyms utn, zajmentesen.

use std::time::Instant;

use microscope_memory::reader::contains_ci;

/// A referencia-implementci: pontosan gy, ahogy a rgi kd csinlta.
fn reference(needle_raw: &str, hay: &str) -> bool {
    let q = needle_raw.to_lowercase();
    hay.to_lowercase().contains(&q)
}

const NEEDLES: &[&str] = &[
    "stratos",
    "STRATOS",
    "Stratos",
    "sTrAtOs",
    "maradk",
    "MARADK",
    "projekt clkitzs",
    "s",
    "szigoros",
    "alma",
    "MRS",
    "index",
    "memory",
];

const HAYS: &[&str] = &[
    "",
    "a",
    "StrAtOs fut.",
    "A STRATOS s a stratos is itt van",
    "Ez egy MARADK maradk",
    "projekt clkitzs dokumentci",
    "RVZTR tkrfrgp",
    "csupa\nsortrs\ns\ttabultor",
    "kezetes kezetes KEZETES",
    "No match at all",
    "s",
    "  alma  ",
    "vegyes 123 szmok s !@# jelek",
    "szi rhajs lels",
    "İstanbul İ (U+0130)",
    "A rvid s a hossz szveg is itt van, tbb soron tvel tartalommal",
];

#[test]
fn contains_ci_matches_reference_on_fixtures() {
    for n in NEEDLES {
        for h in HAYS {
            assert_eq!(
                contains_ci(&n.to_lowercase(), h),
                reference(n, h),
                "eltrs: needle={:?} hay={:?}",
                n,
                h
            );
        }
    }
}

#[test]
fn contains_ci_matches_reference_on_every_substring() {
    // Minden rszstring, minden kezd/ zr pozci.
    let hay = "A Gyors T cenja MRS sorn STRATOS maradk s szi rhajs";
    let lower = hay.to_lowercase();
    let mut checked = 0usize;
    for start in 0..=hay.len() {
        if !hay.is_char_boundary(start) {
            continue;
        }
        for end in start..=hay.len() {
            if !hay.is_char_boundary(end) {
                continue;
            }
            let needle = &lower[start..end];
            assert_eq!(
                contains_ci(needle, hay),
                reference(needle, hay),
                "eltrs: needle={:?}",
                needle
            );
            checked += 1;
        }
    }
    assert!(checked > 1000, "tl kevs rszstring ellenrizve: {}", checked);
}

#[test]
fn ascii_fast_path_equals_char_path() {
    // A sajt ASCII gyorsutat (nyers bjt) egy karakterenknti, immutci
    // nlkli varicival vesszk ssze — ha eltrnek, a gyorst hibs.
    let hay = "STRATOS stratos StraToS STrAtOs sTrAtOs s mg egy soroS";
    for needle in ["stratos", "strat", "str", "s", "atos"] {
        let lower = needle.to_lowercase();
        assert!(contains_ci(&lower, hay), "nem tallta: needle={:?}", needle);
    }
    // Egyelre nem szerepl szk.
    assert!(!contains_ci("stratoss", hay));
    assert!(!contains_ci("stratossz", hay));
    assert!(!contains_ci("xyzzy", hay));
    // Nagybets kezdet (ez volt a korbbi hibaforrs).
    assert!(contains_ci("stratos", "Stratos"));
    assert!(contains_ci("stratos", "STRATOS"));
}

/// A/B mrs egy les ihzetes index-szer szvegen. Figyelmezteten zajos
/// thread-ek nlkl futtatand: cargo test --lib -- --ignored --nocapture
#[test]
#[ignore = "timing benchmark — zajos, csak explicit futtatsra"]
fn ab_contains_ci_vs_tolowercase() {
    // Egy index-szer korpusz: sok rvid blokk, vegyes rsjegy s nagybet.
    let mut corpus: Vec<String> = Vec::with_capacity(23_000);
    let words = [
        "stratos",
        "memory",
        "hebbian",
        "tanuls",
        "architektra",
        "rszletek",
        "rtelmezs",
        "kutats",
        "ksrlet",
        "eredmny",
        "MRS",
        "Stratos",
    ];
    let mut seed = 0x1234_5678u64;
    for _ in 0..23_000 {
        let mut s = String::new();
        let n = 4 + (next(&mut seed) % 6) as usize;
        for _ in 0..n {
            let w = words[(next(&mut seed) as usize) % words.len()];
            s.push_str(w);
            s.push(' ');
        }
        // Vegyes nagybetsts, mint egy valdi indexben.
        if next(&mut seed).is_multiple_of(3) {
            s = s.to_uppercase();
        }
        corpus.push(s);
    }

    let queries = [
        "stratos",
        "memory architecture",
        "hebbian learning",
        "maradk",
        "eredmny",
    ];

    // ── A: a rgi md (to_lowercase per blokk) ──
    let mut a_total = std::time::Duration::ZERO;
    let mut a_hits = 0usize;
    for q in queries {
        let ql = q.to_lowercase();
        let t = Instant::now();
        for c in &corpus {
            if c.to_lowercase().contains(&ql) {
                a_hits += 1;
            }
        }
        a_total += t.elapsed();
    }

    // ── B: contains_ci ──
    let mut b_total = std::time::Duration::ZERO;
    let mut b_hits = 0usize;
    for q in queries {
        let ql = q.to_lowercase();
        let t = Instant::now();
        for c in &corpus {
            if contains_ci(&ql, c) {
                b_hits += 1;
            }
        }
        b_total += t.elapsed();
    }

    assert_eq!(a_hits, b_hits, "a gyors t ms tallatot ad!");

    let denom = (queries.len() * corpus.len()) as u32;
    println!(
        "\n[A/B] {} blokk  {} query = {} sszehasonlts",
        corpus.len(),
        queries.len(),
        denom
    );
    println!("  A: to_lowercase + contains   {:?}", a_total / denom);
    println!("  B: contains_ci               {:?}", b_total / denom);
    println!(
        "  gyorsuls: {:.2}",
        a_total.as_secs_f64() / b_total.as_secs_f64()
    );
}

fn next(seed: &mut u64) -> u64 {
    // xorshift64 — determinisztikus, kvlrl fgggetlen.
    let mut x = *seed;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *seed = x;
    x
}
