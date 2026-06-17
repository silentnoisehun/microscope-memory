# Microscope Memory - Obsidian Sync Plugin
# Place in .obsidian/plugins/microscope-sync/main.js
"""
Connects Obsidian vault to Microscope Memory.
Every note save stores an entry in memory for cross-reference recall.
"""
import requests, json, os
from pathlib import Path

API = "http://localhost:6060/v1"
VAULT = os.environ.get("OBSIDIAN_VAULT", ".")

def sync_note(filepath):
    with open(filepath, "r", encoding="utf-8") as f:
        content = f.read()[:512]  # First 512 chars
    name = Path(filepath).stem
    requests.post(f"{API}/remember", json={
        "text": f"[Obsidian] {name}: {content}",
        "layer": "associative",
        "importance": 4
    })
    print(f"Synced: {name}")

def search_notes(query):
    r = requests.get(f"{API}/recall", params={"q": query, "k": 10})
    if r.ok:
        for m in (r.json() or []):
            print(f"  - {m.get("text","")[:100]}")

# Usage:
# sync_note("/path/to/note.md")
# search_notes("project ideas")
