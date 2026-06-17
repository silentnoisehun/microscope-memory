# Microscope Memory - AutoGPT Plugin
import requests
API = "http://localhost:6060/v1"

def recall(query, k=5):
    r = requests.get(f"{API}/recall", params={"q": query, "k": k})
    return r.json() if r.ok else []

def store(text, imp=5):
    r = requests.post(f"{API}/remember", json={"text": text, "importance": imp})
    return r.ok

# For AutoGPT: add to plugins/ folder
print("Microscope Memory AutoGPT plugin loaded.")
