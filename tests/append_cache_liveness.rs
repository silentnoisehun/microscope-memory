//! A hosszú életű reader append-csepők tesztje.
//!
//! A `find_text_all_ranked` az `append.bin`-t cache-eli. Ha a cache nem
//! validálja magát, egy olyan reader, amelyet egy store előtt nyitottak meg,
//! **elavult** találatokat ad vissza — a store által hozzáadott emlék
//! láthatatlan marad. Ez a teszt ezt az invariánst kényszeríti ki.

use microscope_memory::config::Config;
use microscope_memory::reader::{store_memory, MicroscopeReader};
use tempfile::TempDir;

fn cfg_for(d: &TempDir) -> Config {
    let mut cfg = Config::default();
    cfg.paths.output_dir = d.path().to_string_lossy().to_string();
    cfg.paths.layers_dir = d.path().to_string_lossy().to_string();
    cfg.paths.temp_dir = d.path().to_string_lossy().to_string();
    cfg
}

#[test]
fn long_lived_reader_sees_entries_appended_after_open() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_for(&dir);

    // First build a minimal index: one block containing "elemer".
    // The build helpers write the full index; using the public store path
    // needs an index, so build through the library's own entry point.
    microscope_memory::build::build(&cfg, false, false).expect("build");

    // Open the reader, then append through the same reader's view of the log.
    let mut reader = MicroscopeReader::open(&cfg).expect("open reader");

    // The unique token is what we assert on: it cannot be in the index.
    let appended_text = "oneziglyphunique appended after reader open xylophone";
    store_memory(&cfg, appended_text, "session", 8).expect("store");

    let hits = reader.find_text_all_ranked(&cfg, "xylophone", 10);
    assert!(
        hits.iter().any(|(_, _, is_main, _)| !*is_main),
        "a hosszú életű reader nem látja a nyitás UTÁN append-elt blokkot: {:?}",
        hits
    );
}

#[test]
fn ranked_matches_fresh_reader_after_append() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = cfg_for(&dir);
    microscope_memory::build::build(&cfg, false, false).expect("build");

    let mut stale = MicroscopeReader::open(&cfg).expect("open reader");
    let before = stale.find_text_all_ranked(&cfg, "zebra", 10);

    store_memory(&cfg, "zebra zebra appended after open", "session", 9).expect("store");

    let after = stale.find_text_all_ranked(&cfg, "zebra", 10);

    let mut fresh = MicroscopeReader::open(&cfg).expect("open fresh reader");
    let baseline = fresh.find_text_all_ranked(&cfg, "zebra", 10);

    assert_eq!(
        before.len(),
        0,
        "a 'zebra' token nem szerepelhet az indexben a store előtt"
    );
    assert_eq!(
        after, baseline,
        "a hosszú életű reader eltér a friss readertől az append után"
    );
    assert!(
        !after.is_empty(),
        "az újrafelvett bejegyzés nem található meg"
    );
}
