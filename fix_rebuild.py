content = open('src/autonomous.rs', 'r', encoding='utf-8').read()

# 1. Add build import after dream import
content = content.replace(
    'use crate::dream;',
    'use crate::dream;\nuse crate::build;'
)

# 2. Add run_rebuild method after run_dream
old_run_dream = '''    fn run_dream(&mut self, config: &Config, output_dir: &Path) -> String {
        let block_count = match MicroscopeReader::open(config) {
            Ok(r) => r.block_count,
            Err(_) => 100,
        };
        match dream::dream_consolidate(output_dir, block_count) {
            Ok(cycle) => {
                let msg = format!("\U0001f4a4 \u00c1lom: {} fingerprint, {} meger\u0151s\u00edtve, {} ritk\u00edtva, energia: {:.2} \u2192 {:.2}",
                    cycle.replayed_fingerprints, cycle.strengthened_pairs,
                    cycle.pruned_pairs + cycle.pruned_activations,
                    cycle.energy_before, cycle.energy_after);
                println!("  {}", msg.cyan());
                self.speak(&format!("Dream consolidation complete. Energy changed from {:.2} to {:.2}",
                    cycle.energy_before, cycle.energy_after));
                msg
            }
            Err(e) => {
                let err = format!("Dream error: {}", e);
                eprintln!("  {} {}", "ERROR:".red(), err);
                err
            }
        }
    }'''

new_run_dream = '''    fn run_dream(&mut self, config: &Config, output_dir: &Path) -> String {
        let block_count = match MicroscopeReader::open(config) {
            Ok(r) => r.block_count,
            Err(_) => 100,
        };
        match dream::dream_consolidate(output_dir, block_count) {
            Ok(cycle) => {
                let msg = format!("\U0001f4a4 \u00c1lom: {} fingerprint, {} meger\u0151s\u00edtve, {} ritk\u00edtva, energia: {:.2} \u2192 {:.2}",
                    cycle.replayed_fingerprints, cycle.strengthened_pairs,
                    cycle.pruned_pairs + cycle.pruned_activations,
                    cycle.energy_before, cycle.energy_after);
                println!("  {}", msg.cyan());
                self.speak(&format!("Dream consolidation complete. Energy changed from {:.2} to {:.2}",
                    cycle.energy_before, cycle.energy_after));
                msg
            }
            Err(e) => {
                let err = format!("Dream error: {}", e);
                eprintln!("  {} {}", "ERROR:".red(), err);
                err
            }
        }
    }

    /// Append log rebuild \u2014 a dream consolidation ut\u00e1n automatikusan
    fn run_rebuild(&mut self, config: &Config, output_dir: &Path) -> String {
        let append_path = output_dir.join("append.bin");
        if !append_path.exists() {
            let msg = "\U0001f504 Rebuild: nincs f\u00fcgg\u0151 append entry".to_string();
            println!("  {}", msg.cyan());
            return msg;
        }
        match build::build(config, true) {
            Ok(()) => {
                let _ = std::fs::remove_file(&append_path);
                let msg = "\U0001f504 Rebuild: append log be\u00e9p\u00edtve \u00e9s t\u00f6r\u00f6lve".to_string();
                println!("  {}", msg.green());
                self.speak(&"Append log rebuilt and cleared.".to_string());
                msg
            }
            Err(e) => {
                let err = format!("Rebuild error: {}", e);
                eprintln!("  {} {}", "ERROR:".red(), err);
                err
            }
        }
    }'''

content = content.replace(old_run_dream, new_run_dream)

# 3. Add rebuild call after dream in run_cycle
old_cycle = '''        if should_run(self.config.dream_interval) {
            self.executive.set_module_state("dream", ModuleState::Running);
            let out = self.run_dream(config, output_dir);
            self.executive.set_module_state("dream", ModuleState::Idle);
            outputs.push(out);
        }'''

new_cycle = '''        if should_run(self.config.dream_interval) {
            self.executive.set_module_state("dream", ModuleState::Running);
            let out = self.run_dream(config, output_dir);
            self.executive.set_module_state("dream", ModuleState::Idle);
            outputs.push(out);

            // Dream ut\u00e1n automatikus append log rebuild
            self.executive.set_module_state("dream", ModuleState::Running);
            let rebuild_out = self.run_rebuild(config, output_dir);
            self.executive.set_module_state("dream", ModuleState::Idle);
            outputs.push(rebuild_out);
        }'''

content = content.replace(old_cycle, new_cycle)

open('src/autonomous.rs', 'w', encoding='utf-8').write(content)
print('OK')
