content = open("src/main.rs", "r", encoding="utf-8").read()

# Add forgotten blocks to dream output
old = '                    println!("  Pruned blocks: {}", cycle.pruned_activations);'
new = '                    println!("  Pruned blocks: {}", cycle.pruned_activations);\n                    println!("  Forgotten:      {} blocks", cycle.forgotten_blocks);'
content = content.replace(old, new)

# Add forgotten to dream-log output
old = '                        cycle.consolidated_patterns\n                    );'
new = '                        cycle.consolidated_patterns,\n                        cycle.forgotten_blocks\n                    );'
content = content.replace(old, new)

# Update the format string for dream-log
old = '"    {} \u2014 {}ms, replayed={}, strengthened={}, pruned={}+{}, patterns=+{}"'
new = '"    {} \u2014 {}ms, replayed={}, strengthened={}, pruned={}+{}, patterns=+{}, forgotten={}"'
content = content.replace(old, new)

# Add forgotten to dream-log stats
old = '            println!("  Total replayed: {} fingerprints", stats.total_replayed);'
new = '            println!("  Total replayed: {} fingerprints", stats.total_replayed);\n            println!("  Total forgotten: {} blocks", stats.total_forgotten_blocks);'
content = content.replace(old, new)

open("src/main.rs", "w", encoding="utf-8").write(content)
print("OK")
