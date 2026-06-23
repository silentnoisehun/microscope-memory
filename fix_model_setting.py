import json
with open("C:/Users/mater/.claude/settings.json", "r", encoding="utf-8-sig") as f:
    s = json.load(f)
s["model"] = "gemma4:e2b"
with open("C:/Users/mater/.claude/settings.json", "w", encoding="utf-8") as f:
    json.dump(s, f, indent=2)
print("OK")
