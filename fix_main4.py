content = open("src/main.rs", "r", encoding="utf-8").read()
content = content.replace('cycle.consolidated_patterns,`n                        cycle.forgotten_blocks);', 'cycle.consolidated_patterns);')
open("src/main.rs", "w", encoding="utf-8").write(content)
print("OK")
