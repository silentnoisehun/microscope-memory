with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
old = """        Cmd::Monologue => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut monologue = microscope_memory::inner_monologue::MonologueState::load_or_init(output_dir);
            let entry = monologue.generate_monologue(&config, &reader, output_dir);
            println!("{}", microscope_memory::inner_monologue::format_monologue(&entry));
        }"""
new = """        Cmd::Monologue => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut monologue = microscope_memory::inner_monologue::MonologueState::load_or_init(output_dir);
            let entry = monologue.generate_monologue(&config, &reader, output_dir);
            println!("{}", microscope_memory::inner_monologue::format_monologue(&entry));
        }
        Cmd::Stories { k } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let nm = microscope_memory::narrative_memory::NarrativeMemory::load_or_init(output_dir);
            let episodes = nm.recent_episodes(k);
            if episodes.is_empty() {
                println!("  {} No narrative episodes yet - recall to build stories", "STORIES:".cyan());
            } else {
                println!("  {} {} recent episodes:", "STORIES:".cyan().bold(), episodes.len());
                for ep in episodes {
                    println!("{}", microscope_memory::narrative_memory::format_episode(ep));
                }
            }
        }
        Cmd::Daydream { seed, steps } => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let seed_text = if seed.is_empty() {
                let narrative = microscope_memory::narrative::NarrativeState::load_or_init(output_dir);
                if narrative.narrative.is_empty() || narrative.narrative == "I am silent." {
                    "Microscope Memory".to_string()
                } else {
                    narrative.narrative
                }
            } else {
                seed
            };
            match microscope_memory::daydream::daydream(&config, &reader, output_dir, &seed_text, steps) {
                Ok(result) => println!("{}", microscope_memory::daydream::format_daydream(&result, true)),
                Err(e) => eprintln!("  {} Daydream error: {}", "ERROR:".red(), e),
            }
        }
        Cmd::Hyperfocus { target, focus_type } => {
            let output_dir = Path::new(&config.paths.output_dir);
            let mut hf = microscope_memory::hyperfocus::Hyperfocus::new();
            let intensity = hf.enter_hyperfocus(&target, &focus_type);
            let _ = hf.save_state(output_dir);
            println!("  {} Entering hyperfocus on '{}' ({})", "FOCUS:".green().bold(), target, focus_type);
            println!("  {} Attention multiplier: {}x, Resource concentration: {:.0}%", "FOCUS:".green(), intensity, hf.resource_concentration * 100.0);
            // Run a focused recall
            let reader = open_reader(&config);
            let results = reader.find_text(&target, 10);
            if !results.is_empty() {
                println!("  {} Found {} relevant blocks", "FOCUS:".green(), results.len());
                for (depth, idx) in results.iter().take(5) {
                    reader.print_result(*idx, *depth as f32);
                }
            }
        }"""
c = c.replace(old, new, 1)
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Handlers added")
