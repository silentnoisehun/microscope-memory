"""Microscope Memory + Ollama local RAG"""
import requests

BRIDGE = "http://localhost:6060/v1"
OLLAMA = "http://localhost:11434/api"

def recall(query, k=5):
    r = requests.get(f"{BRIDGE}/recall", params={"q": query, "k": k})
    return r.json() if r.ok else []

def store(text, imp=3):
    requests.post(f"{BRIDGE}/remember", json={"text": text, "importance": imp})

def chat(user_input):
    mems = recall(user_input, 5)
    ctx = "
".join([f"- {m.get(\"text\",\"\")}" for m in (mems or [])])
    prompt = f"Context:
{ctx}

User: {user_input}
Assistant:"
    r = requests.post(f"{OLLAMA}/generate", json={"model": "llama3.2", "prompt": prompt, "stream": False})
    reply = r.json().get("response", "")
    store(f"Q: {user_input} A: {reply[:200]}")
    return reply

print(chat("What do we know about Rust?"))
