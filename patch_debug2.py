with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
# Find the narrative memory section and add debug before it
old = "        // --- Narrative Memory: build story episode from every recall ---"
new = "        eprintln!(\"DEBUG: activated.len={}, all_results main={}\", activated.len(), all_results.iter().filter(|(_,_,m)| *m).count());\n        // --- Narrative Memory: build story episode from every recall ---"
if old in c:
    c = c.replace(old, new, 1)
    with open("src/main.rs","w",encoding="utf-8") as f:
        f.write(c)
    print("Debug added")
else:
    print("ERROR: Could not find insertion point")
    # Find similar text
    import re
    for m in re.finditer(r"Narrative Memory", c):
        print(f"Found at {m.start()}: {c[m.start()-50:m.start()+50]}")
