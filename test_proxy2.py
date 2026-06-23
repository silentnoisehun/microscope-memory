import httpx, json

resp = httpx.post("http://127.0.0.1:8080/v1/messages", json={
    "model": "qwen3-cc:latest",
    "messages": [{"role": "user", "content": "hi"}],
    "max_tokens": 50,
    "stream": False
}, timeout=30)
print(f"Test 1: {resp.status_code}")
data = resp.json()
print(f"  Content: {data['content'][0]['text'][:50]}")
