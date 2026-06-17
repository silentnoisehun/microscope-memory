"""Microscope Memory - OpenAI Assistants API"""
import requests, json, time

BRIDGE = "http://localhost:6060/v1"

def recall(query, k=5):
    r = requests.get(f"{BRIDGE}/recall", params={"q": query, "k": k})
    return r.json() if r.ok else []

def store(text, imp=5):
    r = requests.post(f"{BRIDGE}/remember", json={"text": text, "importance": imp})
    return r.ok

# Usage:
store("User prefers Python", 7)
mems = recall("user preference")
print("Memories:", mems)
