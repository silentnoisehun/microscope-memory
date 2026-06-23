content = open("src/dream.rs", "r", encoding="utf-8").read()

# Fix duplicate total_forgotten_blocks
# Find the first occurrence and remove the second
first = content.find("total_forgotten_blocks")
second = content.find("total_forgotten_blocks", first + 5)
if second > 0:
    # Find the line with the second occurrence and remove it
    line_start = content.rfind("\n", 0, second) + 1
    line_end = content.find("\n", second)
    content = content[:line_start] + content[line_end+1:]

# Fix the yellow() issue - use a simple string instead
content = content.replace(
    'println!("  {} {} belső gondolat elfelejtve ({} blokk maradt)", \n        "FORGET".yellow(), forgotten, n);',
    'println!("  [FORGET] {} belső gondolat elfelejtve ({} blokk maradt)", forgotten, n);'
)

open("src/dream.rs", "w", encoding="utf-8").write(content)
print("OK")
