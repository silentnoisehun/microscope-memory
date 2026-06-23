import json
with open("E:/microscope-local/.claude/settings.local.json", "r", encoding="utf-8") as f:
    s = json.load(f)
s["enableAllProjectMcpServers"] = False
s["enabledMcpjsonServers"] = []
s["mcpServers"] = {}
with open("E:/microscope-local/.claude/settings.local.json", "w", encoding="utf-8") as f:
    json.dump(s, f, indent=2)
print("OK")
