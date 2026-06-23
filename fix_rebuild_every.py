content = open("src/autonomous.rs", "r", encoding="utf-8").read()

# Remove rebuild from inside dream block
old = '''        if should_run(self.config.dream_interval) {
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

new = '''        if should_run(self.config.dream_interval) {
            self.executive.set_module_state("dream", ModuleState::Running);
            let out = self.run_dream(config, output_dir);
            self.executive.set_module_state("dream", ModuleState::Idle);
            outputs.push(out);
        }'''

content = content.replace(old, new)

# Add rebuild at the end of run_cycle, before the summary
old2 = '''        // T\u00e1roljuk a ciklus \u00f6sszefoglal\u00f3t
        let summary = format!("Autonomous cycle #{}: {} modules executed. Energy: {:.1}%, Attention: {:.1}%",
            cycle, outputs.len(), energy * 100.0, attention * 100.0);
        self.store_result(config, &summary, "session", 3);'''

new2 = '''        // Minden ciklus v\u00e9g\u00e9n: append log rebuild (ha van mit)
        let rebuild_out = self.run_rebuild(config, output_dir);
        outputs.push(rebuild_out);

        // T\u00e1roljuk a ciklus \u00f6sszefoglal\u00f3t
        let summary = format!("Autonomous cycle #{}: {} modules executed. Energy: {:.1}%, Attention: {:.1}%",
            cycle, outputs.len(), energy * 100.0, attention * 100.0);
        self.store_result(config, &summary, "session", 3);'''

content = content.replace(old2, new2)

open("src/autonomous.rs", "w", encoding="utf-8").write(content)
print("OK - rebuild minden ciklusban")
