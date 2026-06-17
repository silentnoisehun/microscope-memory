"""Microscope Memory — MCP Server for AI Coding Agents (v0.8.0)"""
import json, sys, os, subprocess
from pathlib import Path

MICROSCOPE_BIN = os.environ.get("MICROSCOPE_BIN", "C:/Users/mater/Desktop/microscope-memory/target/release/microscope-mem.exe")

def ms(text, layer="long_term", imp=5):
    try: return subprocess.run([MICROSCOPE_BIN, "store", "-l", layer, "-i", str(imp), text], capture_output=True, text=True, timeout=10).returncode == 0
    except: return False

def mr(query, k=10):
    try:
        r = subprocess.run([MICROSCOPE_BIN, "recall", query, str(k)], capture_output=True, text=True, timeout=10)
        if r.returncode == 0:
            out = []
            for line in r.stdout.strip().split("\n"):
                if "] [" in line and len(line.split(" ", 3)) >= 4:
                    out.append({"text": line.split(" ", 3)[3]})
            return out
    except: pass
    return []

def mt(query, steps=5):
    try:
        r = subprocess.run([MICROSCOPE_BIN, "think", query, str(steps)], capture_output=True, text=True, timeout=30)
        return [l for l in r.stdout.strip().split("\n") if l] if r.returncode == 0 else []
    except: return []

TOOLS = {
    "recall": {"description": "Recall from Microscope Memory", "input_schema": {"type": "object", "properties": {"query": {"type": "string"}, "k": {"type": "integer", "default": 10}}, "required": ["query"]}},
    "store": {"description": "Store in Microscope Memory", "input_schema": {"type": "object", "properties": {"text": {"type": "string"}, "importance": {"type": "integer", "default": 5}}, "required": ["text"]}},
    "think": {"description": "Chain-of-thought thinking", "input_schema": {"type": "object", "properties": {"query": {"type": "string"}, "max_steps": {"type": "integer", "default": 5}}, "required": ["query"]}},
}

def handle(msg):
    t = msg.get("type", "")
    i = msg.get("id", "")
    if t == "initialize": return {"type": "initialized", "id": i, "serverInfo": {"name": "microscope-memory", "version": "0.8.0"}}
    if t == "list_tools": return {"type": "tool_list", "id": i, "tools": list(TOOLS.values())}
    if t == "call_tool":
        name, args = msg.get("tool", ""), msg.get("arguments", {})
        if name == "recall": return {"type": "tool_result", "id": i, "result": {"data": mr(args.get("query",""), args.get("k",10))}}
        if name == "store": ok = ms(args.get("text",""), imp=args.get("importance",5)); return {"type": "tool_result", "id": i, "result": {"success": ok}}
        if name == "think": return {"type": "tool_result", "id": i, "result": {"steps": mt(args.get("query",""), args.get("max_steps",5))}}
    return {"type": "error", "id": i, "error": f"Unknown: {t}"}

def main():
    if "--config" in sys.argv:
        cfg = {"mcpServers": {"microscope": {"command": "python", "args": [__file__, "--stdio"], "env": {"MICROSCOPE_BIN": MICROSCOPE_BIN}}}}
        print("\n=== Claude Code / Cursor / Continue MCP Config ===")
        print(json.dumps(cfg, indent=2))
        print("\nAdd to ~/.claude/settings.json or .cursor/mcp.json or ~/.continue/config.json")
        return
    while True:
        try:
            line = sys.stdin.readline()
            if not line: break
            resp = handle(json.loads(line))
            sys.stdout.write(json.dumps(resp) + "\n")
            sys.stdout.flush()
        except: continue

if __name__ == "__main__":
    if "--config" in sys.argv: main()
    elif "--stdio" in sys.argv: main()
    else:
        print("Microscope Memory MCP Server")
        print("  --stdio   Run MCP server over stdio")
        print("  --config  Print agent config")
