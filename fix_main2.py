content = open("src/main.rs", "r", encoding="utf-8").read()

# Fix the dream-log format string - use a more precise match
old = '                        "    {} \u2014 {}ms, replayed={}, strengthened={}, pruned={}+{}, patterns=+{}",'
new = '                        "    {} \u2014 {}ms, replayed={}, strengthened={}, pruned={}+{}, patterns=+{}, forgotten={}",'
content = content.replace(old, new)

# Fix the dream-log args
old = '                        cycle.consolidated_patterns\n                    );'
new = '                        cycle.consolidated_patterns,\n                        cycle.forgotten_blocks\n                    );'
content = content.replace(old, new)

open("src/main.rs", "w", encoding="utf-8").write(content)
print("OK")
