with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()

# Find the auto-reflect block and add more auto features after it
old = "            println!(\"{}\", microscope_memory::self_reflect::format_reflection(&reflection));\n        }\n    }"
new = """            println!("{}", microscope_memory::self_reflect::format_reflection(&reflection));
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

        // --- Narrative Memory: build story episode from every recall ---
        {
            let mut nm = microscope_memory::narrative_memory::NarrativeMemory::load_or_init(output_dir);
            if let Some(ep) = nm.build_episode(config, &reader, output_dir, query, &all_results) {
                if nm.episodes.len() <= 3 || nm.episodes.len() % 5 == 0 {
                    println!("{}", microscope_memory::narrative_memory::format_episode(&ep));
                }
            }
        }

        // --- Auto inner monologue: every 15th recall ---
        if narrative.session_count > 0 && (narrative.session_count as usize) % 15 == 0 {
            let mut monologue = microscope_memory::inner_monologue::MonologueState::load_or_init(output_dir);
            let entry = monologue.generate_monologue(config, &reader, output_dir);
            println!("{}", microscope_memory::inner_monologue::format_monologue(&entry));
        }
    }"""

if old in c:
    c = c.replace(old, new, 1)
    with open("src/main.rs","w",encoding="utf-8") as f:
        f.write(c)
    print("All auto features added to recall pipeline")
else:
    print("ERROR: Could not find insertion point")
    # Debug
    idx = c.find("format_reflection(&reflection)")
    if idx >= 0:
        print(f"Found at {idx}")
        print(repr(c[idx:idx+100]))
