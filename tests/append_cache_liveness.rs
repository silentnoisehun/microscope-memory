//! A hosszĂş Ă©letĹ± reader append-csepĹ‘k tesztje.
//!
//! A `find_text_all_ranked` az `append.bin`-t cache-eli. Ha a cache nem
//! validĂˇlja magĂˇt, egy olyan reader, amelyet egy store elĹ‘tt nyitottak meg,
//! **elavult** talĂˇlatokat ad vissza â€” a store Ăˇltal hozzĂˇadott emlĂ©k
//! lĂˇthatatlan marad. Ez a teszt ezt az invariĂˇnst kĂ©nyszerĂ­ti ki.

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
        "a hosszĂş Ă©letĹ± reader nem lĂˇtja a nyitĂˇs UTĂN append-elt blokkot: {:?}",
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
        "a 'zebra' token nem szerepelhet az indexben a store elĹ‘tt"
    );
    assert_eq!(
        after, baseline,
        "a hosszĂş Ă©letĹ± reader eltĂ©r a friss readertĹ‘l az append utĂˇn"
    );
    assert!(!after.is_empty(), "az Ăşjrafelvett bejegyzĂ©s nem talĂˇlhatĂł meg");
}
