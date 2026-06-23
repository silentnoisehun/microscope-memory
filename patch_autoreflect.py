with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()

# Add auto-reflect after the narrative block, before the closing brace of the if !activated block
old = '        if narrative.session_count <= 3 || narrative.session_count % 10 == 0 {\n            println!("  {} {}", "NARRATIVE:".cyan(), narrative.narrative);\n        }\n    }'
new = '        if narrative.session_count <= 3 || narrative.session_count % 10 == 0 {\n            println!("  {} {}", "NARRATIVE:".cyan(), narrative.narrative);\n        }\n\n        // --- Auto-reflect: every N recalls, the system thinks about itself ---\n        if narrative.session_count > 0 && narrative.session_count % microscope_memory::self_reflect::AUTO_REFLECT_INTERVAL == 0 {\n            let reflection = microscope_memory::self_reflect::introspect(config, &reader, output_dir);\n            println!("{}", microscope_memory::self_reflect::format_reflection(&reflection));\n        }\n    }'

c = c.replace(old, new, 1)
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Auto-reflect added to recall pipeline")
