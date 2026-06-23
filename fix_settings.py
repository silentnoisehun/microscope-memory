import json
with open("C:/Users/mater/.claude/settings.json", "r", encoding="utf-8-sig") as f:
    s = json.load(f)
s["enabledMcpjsonServers"] = []
s["hooks"] = {}
with open("C:/Users/mater/.claude/settings.json", "w", encoding="utf-8") as f:
    json.dump(s, f, indent=2)
print("OK")
