with open("src/main.rs","r",encoding="utf-8") as f:
    c = f.read()
# Add debug print before narrative memory
old = "        // --- Narrative Memory: build story episode from every recall ---"
new = '        eprintln!("  DEBUG: activated.len()={}, all_results main={}", activated.len(), all_results.iter().filter(|(_,_,m)| *m).count());\n        // --- Narrative Memory: build story episode from every recall ---'
c = c.replace(old, new, 1)
with open("src/main.rs","w",encoding="utf-8") as f:
    f.write(c)
print("Debug added")
