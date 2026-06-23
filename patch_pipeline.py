with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
old = "        // --- Auto-reflect: every N recalls, the system thinks about itself ---\n        if narrative.session_count > 0 && narrative.session_count % microscope_memory::self_reflect::AUTO_REFLECT_INTERVAL == 0 {\n            let reflection = microscope_memory::self_reflect::introspect(config, &reader, output_dir);\n            println!(\"{}\", microscope_memory::self_reflect::format_reflection(&reflection));\n        }"
new = """        // --- Auto-reflect: every N recalls, the system thinks about itself ---
        if narrative.session_count > 0 && (narrative.session_count as usize) % microscope_memory::self_reflect::AUTO_REFLECT_INTERVAL == 0 {
            let reflection = microscope_memory::self_reflect::introspect(config, &reader, output_dir);
            println!("{}", microscope_memory::self_reflect::format_reflection(&reflection));
        }

        // --- Auto self-model snapshot: every 10th recall ---
        if narrative.session_count > 0 && (narrative.session_count as usize) % 10 == 0 {
            let mut self_model = microscope_memory::self_model::SelfModel::load_or_init(output_dir);
            let snap = self_model.take_snapshot(config, &reader, output_dir);
            let change = self_model.describe_change();
            println!("{}", microscope_memory::self_model::format_self_model(&snap, &change));
        }

        // --- Auto curiosity: every 7th recall ---
        if narrative.session_count > 0 && (narrative.session_count as usize) % 7 == 0 {
            let mut curiosity = microscope_memory::curiosity::CuriosityState::load_or_init(output_dir);
            let queries = curiosity.generate_queries(config, &reader, output_dir);
            if !queries.is_empty() {
                println!("{}", microscope_memory::curiosity::format_curiosity(&queries));
            }
        }

        // --- Auto inner monologue: every 15th recall ---
        if narrative.session_count > 0 && (narrative.session_count as usize) % 15 == 0 {
            let mut monologue = microscope_memory::inner_monologue::MonologueState::load_or_init(output_dir);
            let entry = monologue.generate_monologue(config, &reader, output_dir);
            println!("{}", microscope_memory::inner_monologue::format_monologue(&entry));
        }"""
c = c.replace(old, new, 1)
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Auto pipeline wired")
