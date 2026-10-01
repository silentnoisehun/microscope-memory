//! No-learn mode: measure without changing what is being measured.
//!
//! A `recall` is not a read. It records activations, strengthens co-activations,
//! emits resonance pulses, refines attention weights, rewrites the narrative and
//! stores that narrative as a memory. That is the learning, and it is the point
//! of the system -- but it also means the act of measuring changes the state the
//! next measurement starts from.
//!
//! Measured: replaying one query returned its top hit at L2=0.45051, then
//! 0.25906, 0.20475, 0.17290 and 0.16108 across consecutive runs, while the
//! pre-fetch confidence on that query climbed 62% -> 81% -> 98%. A single
//! recall rewrote 17 files in the output directory. A benchmark built on that
//! reports how often the system has been asked rather than what it knows, which
//! is why the recorded recall figures were not reproducible: three runs of one
//! unmodified binary gave R@1 of 37, 34 and 34.
//!
//! Setting `MICROSCOPE_NO_LEARN=1` returns from the recall as soon as the
//! ranking is printed, before any learning state is written. The search is
//! untouched -- the same blocks are scored, boosted and ranked, and the same
//! numbers are printed -- so only the learning writes are skipped, and repeated
//! runs against one index stop feeding each other.
//!
//! This is about the memory system only, and is unrelated to `[hooks] read_only`
//! in the config, which governs the hook manager. Opt-in and off by default: a
//! server that forgets a query it just served is broken, so nothing changes
//! unless a measurement asks for it.

/// True when the process must not write learning state.
pub fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MICROSCOPE_NO_LEARN")
            .map(|v| parse(&v))
            .unwrap_or(false)
    })
}

fn parse(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_accepts_the_usual_spellings_and_nothing_else() {
        // The environment is read once per process, so the rule itself is what
        // is under test, not the ambient value of the test runner.
        for yes in ["1", "true", "TRUE", " yes ", "on"] {
            assert!(parse(yes), "{yes:?} should enable read-only");
        }
        for no in ["0", "false", "", "maybe", "2"] {
            assert!(!parse(no), "{no:?} should leave learning on");
        }
    }
}
