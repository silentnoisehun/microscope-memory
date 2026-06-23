import json
with open("E:/microscope-local/.claude/settings.local.json", "r", encoding="utf-8") as f:
    s = json.load(f)
s["enableAllProjectMcpServers"] = True
s["enabledMcpjsonServers"] = ["microscope-memory", "voice-mcp"]
s["mcpServers"] = {
    "microscope": {
        "command": "E:\\microscope-local\\target\\release\\microscope-mem.exe",
        "args": ["mcp"]
    },
    "voice-mcp": {
        "command": "python",
        "args": ["E:\\microscope-local\\tools\\voice_mcp.py"]
    }
}
with open("E:/microscope-local/.claude/settings.local.json", "w", encoding="utf-8") as f:
    json.dump(s, f, indent=2)
print("OK")
