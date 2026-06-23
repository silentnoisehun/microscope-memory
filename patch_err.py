with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()

# Change the narrative.update error handling
old = "        let _ = narrative.update(\n            output_dir,\n            Some(&esr),\n            Some(&wm_items),\n            due_count,\n            thought_count,\n            Some(query),\n        );"

new = """        if let Err(e) = narrative.update(
            output_dir,
            Some(&esr),
            Some(&wm_items),
            due_count,
            thought_count,
            Some(query),
        ) {
            eprintln!("  {} narrative update failed: {}", "ERROR:".red(), e);
        }"""

if old in c:
    c = c.replace(old, new, 1)
    with open("src/main.rs","w",encoding="utf-8") as f:
        f.write(c)
    print("Error handling added")
else:
    print("ERROR: Could not find narrative.update call")
    # Debug
    idx = c.find("narrative.update(")
    if idx >= 0:
        print(f"Found at position {idx}")
        print(repr(c[idx:idx+300]))
