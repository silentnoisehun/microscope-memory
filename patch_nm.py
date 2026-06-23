with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
old = "        // --- Auto inner monologue: every 15th recall ---\n        if narrative.session_count > 0 && (narrative.session_count as usize) % 15 == 0 {"
new = """        // --- Narrative Memory: build story episode from every recall ---
        if !activated.is_empty() {
            let mut nm = microscope_memory::narrative_memory::NarrativeMemory::load_or_init(output_dir);
            if let Some(ep) = nm.build_episode(config, &reader, output_dir, query, &all_results) {
                if nm.episodes.len() <= 3 || nm.episodes.len() % 5 == 0 {
                    println!("{}", microscope_memory::narrative_memory::format_episode(&ep));
                }
            }
        }

        // --- Auto inner monologue: every 15th recall ---
        if narrative.session_count > 0 && (narrative.session_count as usize) % 15 == 0 {"""
c = c.replace(old, new, 1)
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Narrative memory wired into recall pipeline")
