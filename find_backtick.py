import re
content = open("src/main.rs", "r", encoding="utf-8").read()
matches = [(m.start(), m.group()) for m in re.finditer(r"`n", content)]
for pos, m in matches:
    line = content[:pos].count("\n") + 1
    print(f"Line {line}: found literal backtick-n at position {pos}")
    start = max(0, pos-50)
    end = min(len(content), pos+50)
    print(f"  Context: {repr(content[start:end])}")
if not matches:
    print("No literal backtick-n found")
