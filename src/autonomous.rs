//! Autonomous Mode — Microscope Memory önálló életciklusa
//!
//! A rendszer magától jár: időnként recall-ol (kíváncsiság), álmodik,
//! reflektál magára, történeteket épít, belső monológot folytat,
//! és mindezt TTS-sel is felolvassa.
//!
//! Használat:
//!   microscope-mem autonomous          # Alapértelmezett ciklus
//!   microscope-mem autonomous --tts     # Felolvasással
//!   microscope-mem autonomous --daemon  # Háttérfolyamatként

use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use colored::Colorize;

use crate::build;
use crate::config::Config;
use crate::daydream;
use crate::dream;
use crate::executive::{Executive, ModuleState};
use crate::inner_monologue;
use crate::narrative::NarrativeState;
use crate::narrative_memory;
use crate::reader::{store_memory_temporary, MicroscopeReader};
use crate::self_model::SelfModel;
use crate::self_reflect;

// ─── Constants ─────────────────────────────────────────────

/// Hány másodperc egy ciklus
pub const CYCLE_INTERVAL_SECS: u64 = 30;

/// Hány ciklus után jöjjön a daydream
pub const DAYDREAM_INTERVAL: usize = 3;

/// Hány ciklus után jöjjön a curiosity
pub const CURIOSITY_INTERVAL: usize = 2;

/// Hány ciklus után jöjjön a monológ
pub const MONOLOGUE_INTERVAL: usize = 5;

/// Hány ciklus után jöjjön a self-reflect
pub const REFLECT_INTERVAL: usize = 4;

/// Hány ciklus után jöjjön a dream consolidation
pub const DREAM_INTERVAL: usize = 10;

/// Hány ciklus után jöjjön a self-model snapshot
pub const SELF_MODEL_INTERVAL: usize = 7;

/// Hány ciklus után jöjjön a narrative memory építés
pub const NARRATIVE_INTERVAL: usize = 2;

// ─── AutonomousConfig ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AutonomousConfig {
    pub cycle_interval_secs: u64,
    pub daydream_interval: usize,
    pub curiosity_interval: usize,
    pub monologue_interval: usize,
    pub reflect_interval: usize,
    pub dream_interval: usize,
    pub self_model_interval: usize,
    pub narrative_interval: usize,
    pub tts_enabled: bool,
    pub daemon_mode: bool,
    pub max_cycles: Option<usize>,
}

impl Default for AutonomousConfig {
    fn default() -> Self {
        Self {
            cycle_interval_secs: CYCLE_INTERVAL_SECS,
            daydream_interval: DAYDREAM_INTERVAL,
            curiosity_interval: CURIOSITY_INTERVAL,
            monologue_interval: MONOLOGUE_INTERVAL,
            reflect_interval: REFLECT_INTERVAL,
            dream_interval: DREAM_INTERVAL,
            self_model_interval: SELF_MODEL_INTERVAL,
            narrative_interval: NARRATIVE_INTERVAL,
            tts_enabled: false,
            daemon_mode: false,
            max_cycles: None,
        }
    }
}

// ─── AutonomousEngine ──────────────────────────────────────

pub struct AutonomousEngine {
    config: AutonomousConfig,
    executive: Executive,
    cycle_count: usize,
    last_outputs: VecDeque<String>,
}

impl AutonomousEngine {
    pub fn new(auto_config: AutonomousConfig) -> Self {
        let executive = Executive::new();

        // Regisztráljuk a kognitív modulokat az Executive-ban
        executive.register_module(
            "daydream",
            "Daydream - asszociatív drift",
            60,
            0.3,
            vec!["creative".to_string(), "exploration".to_string()],
        );
        executive.register_module(
            "curiosity",
            "Curiosity - proaktív kíváncsiság",
            70,
            0.2,
            vec!["exploration".to_string(), "learning".to_string()],
        );
        executive.register_module(
            "monologue",
            "Inner Monologue - belső gondolkodás",
            50,
            0.4,
            vec!["reflection".to_string(), "planning".to_string()],
        );
        executive.register_module(
            "reflect",
            "Self-Reflection - önvizsgálat",
            80,
            0.3,
            vec!["reflection".to_string(), "meta".to_string()],
        );
        executive.register_module(
            "narrative",
            "Narrative Memory - történetépítés",
            40,
            0.2,
            vec!["memory".to_string(), "story".to_string()],
        );
        executive.register_module(
            "dream",
            "Dream Consolidation - álom",
            30,
            0.5,
            vec!["maintenance".to_string(), "pruning".to_string()],
        );
        executive.register_module(
            "self_model",
            "Self-Model - önkép",
            90,
            0.2,
            vec!["meta".to_string(), "identity".to_string()],
        );

        Self {
            config: auto_config,
            executive,
            cycle_count: 0,
            last_outputs: VecDeque::with_capacity(20),
        }
    }

    fn speak(&self, text: &str) {
        if !self.config.tts_enabled {
            return;
        }
        let safe_text = text.replace('"', " ").replace("\n", " ");
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let filename = format!("tts_{}.mp3", ts);
        // Edge TTS - headless MP3 generalas, megvárjuk
        let gen = std::process::Command::new("python")
            .args([
                "-m",
                "edge_tts",
                "--voice",
                "hu-HU-NoemiNeural",
                "--text",
                &safe_text,
                "--write-media",
                &filename,
            ])
            .output();
        match gen {
            Ok(_) => {
                // Headless lejatszas ffplay-el, megvárjuk amíg végez
                let _ = std::process::Command::new("ffplay")
                    .args(["-nodisp", "-autoexit", "-loglevel", "quiet", &filename])
                    .status();
                // Töröljük a temp fájlt
                let _ = std::fs::remove_file(&filename);
            }
            Err(e) => {
                eprintln!("  {} TTS error: {}", "ERROR:".red(), e);
            }
        }
    }

    fn store_result(&self, config: &Config, text: &str, layer: &str, importance: u8) {
        // Internal thoughts: only to append log + timeline, NOT to layer files
        // (they would accumulate and never be forgotten)
        if let Err(e) = store_memory_temporary(config, text, layer, importance) {
            eprintln!("  {} Store error: {}", "ERROR:".red(), e);
        }
    }

    fn run_daydream(&mut self, config: &Config, output_dir: &Path) -> String {
        let seed = {
            let narrative = NarrativeState::load_or_init(output_dir);
            if narrative.narrative.is_empty() || narrative.narrative == "I am silent." {
                "Microscope Memory autonomous mode".to_string()
            } else {
                narrative.narrative
            }
        };

        match daydream::daydream(config, &seed, 3) {
            Ok(result) => {
                let formatted = daydream::format_daydream(&result, false);
                let summary = format!(
                    "🧠 Daydream: {} lépés, érzelmi eltolódás: {:.2}",
                    result.steps.len(),
                    result.total_emotion_shift
                );
                println!("  {}", summary.cyan());
                println!("{}", formatted);

                self.store_result(
                    config,
                    &format!("Daydream: {} | {}", seed, result.final_narrative),
                    "associative",
                    4,
                );
                self.speak(&format!(
                    "Daydream kész. {} lépés, érzelmi eltolódás: {:.2}",
                    result.steps.len(),
                    result.total_emotion_shift
                ));
                summary
            }
            Err(e) => {
                let err = format!("Daydream error: {}", e);
                eprintln!("  {} {}", "ERROR:".red(), err);
                err
            }
        }
    }

    fn run_curiosity(&mut self, config: &Config, output_dir: &Path) -> String {
        let reader = match MicroscopeReader::open(config) {
            Ok(r) => r,
            Err(e) => return format!("Reader error: {}", e),
        };
        let mut curiosity = crate::curiosity::CuriosityState::load_or_init(output_dir);
        let queries = curiosity.generate_queries(config, &reader, output_dir);

        if queries.is_empty() {
            let msg = "🤔 Curiosity: nincs új felfedeznivaló".to_string();
            println!("  {}", msg.cyan());
            return msg;
        }

        let top = &queries[0];
        let msg = format!(
            "🤔 Curiosity: \"{}\" (score: {:.2}, ok: {})",
            top.query, top.score, top.reason
        );
        println!("  {}", msg.cyan());
        for q in queries.iter().take(3) {
            println!("    └─ [{}] {} ({:.2})", q.reason, q.query, q.score);
        }

        self.store_result(
            config,
            &format!("Curiosity: {} (score: {:.2})", top.query, top.score),
            "short_term",
            3,
        );
        self.speak(&format!("Kíváncsi vagyok: {}", top.query));
        msg
    }

    fn run_monologue(&mut self, config: &Config, output_dir: &Path) -> String {
        let reader = match MicroscopeReader::open(config) {
            Ok(r) => r,
            Err(e) => return format!("Reader error: {}", e),
        };
        let mut monologue = inner_monologue::MonologueState::load_or_init(output_dir);
        let entry = monologue.generate_monologue(config, &reader, output_dir);

        let formatted = inner_monologue::format_monologue(&entry);
        let msg = format!("💭 Monológ: {} lépés", entry.steps.len());
        println!("  {}", msg.cyan());
        println!("{}", formatted);

        let mono_text = entry.steps.join(" | ");
        self.store_result(
            config,
            &format!("Inner Monologue: {}", mono_text),
            "reflections",
            5,
        );
        if let Some(first) = entry.steps.first() {
            self.speak(&format!("Belső monológ: {}", first));
        }
        msg
    }

    fn run_reflect(&mut self, config: &Config, output_dir: &Path) -> String {
        let reader = match MicroscopeReader::open(config) {
            Ok(r) => r,
            Err(e) => return format!("Reader error: {}", e),
        };
        let reflection = self_reflect::introspect(config, &reader, output_dir);
        let formatted = self_reflect::format_reflection(&reflection);
        let msg = format!("🪞 Önreflexió: {}", reflection);
        println!("  {}", msg.cyan());
        println!("{}", formatted);

        self.store_result(
            config,
            &format!("Self-Reflection: {}", reflection),
            "reflections",
            6,
        );
        self.speak(&format!("Önreflexió: {}", reflection));
        msg
    }

    fn run_narrative(&mut self, config: &Config, output_dir: &Path) -> String {
        let reader = match MicroscopeReader::open(config) {
            Ok(r) => r,
            Err(e) => return format!("Reader error: {}", e),
        };
        // Végzünk egy recall-t, hogy legyenek eredményeink
        let results = reader.find_text("autonomous memory", 10);
        let results_slice: Vec<(f32, usize, bool)> =
            results.iter().map(|(d, i)| (*d as f32, *i, true)).collect();

        let mut nm = narrative_memory::NarrativeMemory::load_or_init(output_dir);
        if let Some(ep) = nm.build_episode(
            config,
            &reader,
            output_dir,
            "autonomous cycle",
            &results_slice,
        ) {
            let formatted = narrative_memory::format_episode(&ep);
            let msg = format!(
                "📖 Történet: {} ({} blokk)",
                ep.title,
                ep.block_indices.len()
            );
            println!("  {}", msg.cyan());
            println!("{}", formatted);
            self.speak(&format!("Új történet: {}. {}", ep.title, ep.summary));
            msg
        } else {
            let msg = "📖 Történet: nincs elég adat".to_string();
            println!("  {}", msg.cyan());
            msg
        }
    }

    fn run_dream(&mut self, config: &Config, output_dir: &Path) -> String {
        let block_count = match MicroscopeReader::open(config) {
            Ok(r) => r.block_count,
            Err(_) => 100,
        };
        match dream::dream_consolidate(output_dir, block_count) {
            Ok(cycle) => {
                let msg = format!("💤 Álom: {} fingerprint, {} megerősítve, {} ritkítva, {} elfelejtve, energia: {:.2} → {:.2}",
                    cycle.replayed_fingerprints, cycle.strengthened_pairs,
                    cycle.pruned_pairs + cycle.pruned_activations,
                    cycle.forgotten_blocks,
                    cycle.energy_before, cycle.energy_after);
                println!("  {}", msg.cyan());
                self.speak(&format!(
                    "Álom konszolidáció kész. {} elfelejtve. Energia: {:.2} -> {:.2}",
                    cycle.forgotten_blocks, cycle.energy_before, cycle.energy_after
                ));
                msg
            }
            Err(e) => {
                let err = format!("Dream error: {}", e);
                eprintln!("  {} {}", "ERROR:".red(), err);
                err
            }
        }
    }

    /// Append log rebuild — a dream consolidation után automatikusan
    fn run_rebuild(&mut self, config: &Config, output_dir: &Path) -> String {
        let append_path = output_dir.join("append.bin");
        if !append_path.exists() {
            let msg = "🔄 Rebuild: nincs függő append entry".to_string();
            println!("  {}", msg.cyan());
            return msg;
        }
        match build::build(config, true) {
            Ok(()) => {
                let _ = std::fs::remove_file(&append_path);
                let msg = "🔄 Rebuild: append log beépítve és törölve".to_string();
                println!("  {}", msg.green());
                self.speak("Append log újraépítve és törölve.");
                msg
            }
            Err(e) => {
                let err = format!("Rebuild error: {}", e);
                eprintln!("  {} {}", "ERROR:".red(), err);
                err
            }
        }
    }

    fn run_self_model(&mut self, config: &Config, output_dir: &Path) -> String {
        let reader = match MicroscopeReader::open(config) {
            Ok(r) => r,
            Err(e) => return format!("Reader error: {}", e),
        };
        let mut self_model = SelfModel::load_or_init(output_dir);
        let snap = self_model.take_snapshot(config, &reader, output_dir);
        let change = self_model.describe_change();
        let formatted = crate::self_model::format_self_model(&snap, &change);
        let msg = format!(
            "🧬 Önkép: {} blokk, {} minta, {} archeotípus",
            snap.block_count, snap.pattern_count, snap.archetype_count
        );
        println!("  {}", msg.cyan());
        println!("{}", formatted);
        self.speak(&format!(
            "Önkép frissítve. {} blokk, {} minta.",
            snap.block_count, snap.pattern_count
        ));
        msg
    }

    /// Egy teljes autonóm ciklus futtatása
    pub fn run_cycle(&mut self, config: &Config) -> Vec<String> {
        self.cycle_count += 1;
        let cycle = self.cycle_count;
        let output_dir = Path::new(&config.paths.output_dir);
        let mut outputs = Vec::new();

        println!();
        println!("{}", "═".repeat(60).cyan());
        println!("  🔄 Autonóm ciklus #{} — {}", cycle, chrono_or_now());
        println!("{}", "═".repeat(60).cyan());

        // Executive ciklus
        let executed = self.executive.cycle();
        if executed.contains(&"__low_energy__".to_string()) {
            let msg = "⚡ Alacsony energia — szünet".to_string();
            println!("  {}", msg.yellow());
            outputs.push(msg);
            return outputs;
        }

        // Homeostasis check
        let homeostatic = self.executive.homeostasis();
        for action in &homeostatic {
            println!("  {} {}", "🏠".yellow(), action.yellow());
        }

        // Első ciklus: minden modul fut. Utána: interval alapján
        let should_run = |interval: usize| cycle == 1 || cycle.is_multiple_of(interval);

        if should_run(self.config.daydream_interval) {
            self.executive
                .set_module_state("daydream", ModuleState::Running);
            let out = self.run_daydream(config, output_dir);
            self.executive
                .set_module_state("daydream", ModuleState::Idle);
            outputs.push(out);
        }

        if should_run(self.config.curiosity_interval) {
            self.executive
                .set_module_state("curiosity", ModuleState::Running);
            let out = self.run_curiosity(config, output_dir);
            self.executive
                .set_module_state("curiosity", ModuleState::Idle);
            outputs.push(out);
        }

        if should_run(self.config.monologue_interval) {
            self.executive
                .set_module_state("monologue", ModuleState::Running);
            let out = self.run_monologue(config, output_dir);
            self.executive
                .set_module_state("monologue", ModuleState::Idle);
            outputs.push(out);
        }

        if should_run(self.config.reflect_interval) {
            self.executive
                .set_module_state("reflect", ModuleState::Running);
            let out = self.run_reflect(config, output_dir);
            self.executive
                .set_module_state("reflect", ModuleState::Idle);
            outputs.push(out);
        }

        if should_run(self.config.narrative_interval) {
            self.executive
                .set_module_state("narrative", ModuleState::Running);
            let out = self.run_narrative(config, output_dir);
            self.executive
                .set_module_state("narrative", ModuleState::Idle);
            outputs.push(out);
        }

        if should_run(self.config.self_model_interval) {
            self.executive
                .set_module_state("self_model", ModuleState::Running);
            let out = self.run_self_model(config, output_dir);
            self.executive
                .set_module_state("self_model", ModuleState::Idle);
            outputs.push(out);
        }

        if should_run(self.config.dream_interval) {
            self.executive
                .set_module_state("dream", ModuleState::Running);
            let out = self.run_dream(config, output_dir);
            self.executive.set_module_state("dream", ModuleState::Idle);
            outputs.push(out);
        }

        // Executive statok kiírása
        let (mod_count, running, energy, attention) = self.executive.stats();
        println!(
            "  {} Modulok: {} | Fut: {} | Energia: {:.1}% | Figyelem: {:.1}%",
            "📊".cyan(),
            mod_count,
            running,
            energy * 100.0,
            attention * 100.0
        );

        // Minden ciklus végén: append log rebuild (ha van mit)
        let rebuild_out = self.run_rebuild(config, output_dir);
        outputs.push(rebuild_out);

        // Tároljuk a ciklus összefoglalót
        let summary = format!(
            "Autonomous cycle #{}: {} modules executed. Energy: {:.1}%, Attention: {:.1}%",
            cycle,
            outputs.len(),
            energy * 100.0,
            attention * 100.0
        );
        self.store_result(config, &summary, "session", 3);

        if !outputs.is_empty() {
            self.speak(&format!(
                "Ciklus {} kész. {} aktivitás.",
                cycle,
                outputs.len()
            ));
        }

        self.last_outputs
            .push_back(format!("#{}: {} aktivitás", cycle, outputs.len()));
        if self.last_outputs.len() > 20 {
            self.last_outputs.pop_front();
        }

        outputs
    }

    /// Fő ciklus — folyamatosan fut
    pub fn run(mut self, config: &Config) {
        let interval = Duration::from_secs(self.config.cycle_interval_secs);
        let max = self.config.max_cycles.unwrap_or(usize::MAX);

        println!(
            "{}",
            "╔══════════════════════════════════════════════════════╗".cyan()
        );
        println!(
            "{}",
            "║     🧠 MICROSCOPE MEMORY — AUTONÓM MÓD            ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╠══════════════════════════════════════════════════════╣".cyan()
        );
        println!("  Ciklus: {} másodperc", self.config.cycle_interval_secs);
        println!(
            "  TTS: {}",
            if self.config.tts_enabled {
                "✅ BE"
            } else {
                "❌ KI"
            }
        );
        println!(
            "  Daemon: {}",
            if self.config.daemon_mode {
                "✅"
            } else {
                "❌"
            }
        );
        println!(
            "  Max ciklus: {}",
            if max == usize::MAX {
                "végtelen".to_string()
            } else {
                max.to_string()
            }
        );
        println!(
            "{}",
            "╚══════════════════════════════════════════════════════╝".cyan()
        );
        println!();

        loop {
            if self.cycle_count >= max {
                println!(
                    "  {} Elértük a maximális ciklusszámot ({})",
                    "✓".green(),
                    max
                );
                break;
            }

            self.run_cycle(config);

            if self.config.daemon_mode {
                println!(
                    "  {} Várakozás {} másodperc...",
                    "⏳".cyan(),
                    interval.as_secs()
                );
                std::thread::sleep(interval);
            } else {
                break;
            }
        }

        println!();
        println!("{}", "═".repeat(60).cyan());
        println!("  ✅ Autonóm mód befejezve — {} ciklus", self.cycle_count);
        println!("  Utolsó kimenetek:");
        for out in self.last_outputs.iter().rev().take(5) {
            println!("    └─ {}", out);
        }
    }
}

// ─── Segédfüggvények ──────────────────────────────────────

fn chrono_or_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now % 60;
    let mins = (now / 60) % 60;
    let hours = (now / 3600) % 24;
    format!("{:02}:{:02}:{:02} UTC", hours, mins, secs)
}

/// Formázott kimenet az autonóm mód indításakor
pub fn print_autonomous_header(config: &AutonomousConfig) {
    println!(
        "{}",
        "╔══════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║     🧠 MICROSCOPE MEMORY — AUTONÓM MÓD            ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╠══════════════════════════════════════════════════════╣".cyan()
    );
    println!("  Ciklus idő:     {} másodperc", config.cycle_interval_secs);
    println!(
        "  Daydream:       minden {} ciklus",
        config.daydream_interval
    );
    println!(
        "  Curiosity:      minden {} ciklus",
        config.curiosity_interval
    );
    println!(
        "  Monológ:        minden {} ciklus",
        config.monologue_interval
    );
    println!(
        "  Önreflexió:     minden {} ciklus",
        config.reflect_interval
    );
    println!(
        "  Történet:       minden {} ciklus",
        config.narrative_interval
    );
    println!(
        "  Önkép:          minden {} ciklus",
        config.self_model_interval
    );
    println!("  Álom:           minden {} ciklus", config.dream_interval);
    println!(
        "  TTS:            {}",
        if config.tts_enabled {
            "✅ BE"
        } else {
            "❌ KI"
        }
    );
    println!(
        "  Daemon mód:     {}",
        if config.daemon_mode { "✅" } else { "❌" }
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════╝".cyan()
    );
}
