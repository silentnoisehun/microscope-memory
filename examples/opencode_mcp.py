# Microscope Memory - OpenCode MCP Integration
import json, subprocess, sys
from pathlib import Path

MICROSCOPE = "C:/Users/mater/Desktop/microscope-memory/target/release/microscope-mem.exe"

# Generate OpenCode MCP config
config = {
    "mcpServers": {
        "microscope": {
            "type": "stdio",
            "command": "python",
            "args": [__file__, "--server"],
            "env": {"MICROSCOPE_BIN": MICROSCOPE}
        }
    }
}

print("# OpenCode MCP Configuration")
print("# Place in .opencode.json or OpenCode settings:")
print(json.dumps(config, indent=2))

if "--server" in sys.argv:
    # Run as MCP server for OpenCode
    while True:
        try:
            line = sys.stdin.readline()
            if not line: break
            msg = json.loads(line)
            resp = {"id": msg.get("id", ""), "type": "result"}
            if msg.get("method") == "list_tools":
                resp["result"] = {
                    "tools": [
                        {"name": "recall", "description": "Recall memories", "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}}},
                        {"name": "store", "description": "Store memory", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}, "importance": {"type": "integer"}}}}
                    ]
                }
            sys.stdout.write(json.dumps(resp) + "
")
            sys.stdout.flush()
        except: break
