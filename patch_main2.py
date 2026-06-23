with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
old = "        Cmd::Introspect => {\n            let reader = open_reader(&config);\n            let output_dir = Path::new(&config.paths.output_dir);\n            let reflection = microscope_memory::self_reflect::introspect(&config, &reader, output_dir);\n            println!(\"{}\", microscope_memory::self_reflect::format_reflection(&reflection));\n        }"
new = """        Cmd::Introspect => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let reflection = microscope_memory::self_reflect::introspect(&config, &reader, output_dir);
            println!("{}", microscope_memory::self_reflect::format_reflection(&reflection));
        }
        Cmd::SelfModel => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut self_model = microscope_memory::self_model::SelfModel::load_or_init(output_dir);
            let snap = self_model.take_snapshot(&config, &reader, output_dir);
            let change = self_model.describe_change();
            println!("{}", microscope_memory::self_model::format_self_model(&snap, &change));
        }
        Cmd::Curiosity => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut curiosity = microscope_memory::curiosity::CuriosityState::load_or_init(output_dir);
            let queries = curiosity.generate_queries(&config, &reader, output_dir);
            println!("{}", microscope_memory::curiosity::format_curiosity(&queries));
        }
        Cmd::Monologue => {
            let reader = open_reader(&config);
            let output_dir = Path::new(&config.paths.output_dir);
            let mut monologue = microscope_memory::inner_monologue::MonologueState::load_or_init(output_dir);
            let entry = monologue.generate_monologue(&config, &reader, output_dir);
            println!("{}", microscope_memory::inner_monologue::format_monologue(&entry));
        }"""
c = c.replace(old, new, 1)
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Handlers added to main.rs")
