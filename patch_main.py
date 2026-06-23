with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
old = '        Cmd::Mermaid { port } => {\n            if let Err(e) = microscope_memory::mermaid::run(config, port).await {\n                eprintln!("  {} Mermaid error: {}", "ERROR:".red(), e);\n            }\n        }\n    }'
new = '        Cmd::Mermaid { port } => {\n            if let Err(e) = microscope_memory::mermaid::run(config, port).await {\n                eprintln!("  {} Mermaid error: {}", "ERROR:".red(), e);\n            }\n        }\n        Cmd::Introspect => {\n            let reader = open_reader(&config);\n            let output_dir = Path::new(&config.paths.output_dir);\n            let reflection = microscope_memory::self_reflect::introspect(&config, &reader, output_dir);\n            println!("{}", microscope_memory::self_reflect::format_reflection(&reflection));\n        }\n    }'
c = c.replace(old, new, 1)
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Introspect handler added to main.rs")
