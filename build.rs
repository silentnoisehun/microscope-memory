//! build.rs — Polymorphic constant generator.
//! Generates unique XOR keys and jitter values per build.
//! (Red Audit - Phase 3)

use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("const_gen.rs");

    // Simple pseudo-random seed from SystemTime for polymorphism
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let seed = since_the_epoch.as_nanos();

    let xor_key = (seed % 255) as u8;
    let jitter = 5 + (seed % 10) as u64; // 5-15ms range

    let content = format!(
        "pub const POLY_XOR_KEY: u8 = {};\npub const POLY_JITTER: u64 = {};\n",
        xor_key, jitter
    );

    fs::write(&dest_path, content).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
