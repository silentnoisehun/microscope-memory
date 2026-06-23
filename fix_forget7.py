content = open("src/dream.rs", "r", encoding="utf-8").read()

# Remove the stray duplicate lines 151-154
old = """                .sum(),
                .cycles
                .iter()
                .map(|c| c.forgotten_blocks as u64)
                .sum(),
            total_forgotten_blocks: self"""

new = """                .sum(),
            total_forgotten_blocks: self"""

content = content.replace(old, new)

open("src/dream.rs", "w", encoding="utf-8").write(content)
print("OK")
