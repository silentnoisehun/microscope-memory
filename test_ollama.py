import httpx
resp = httpx.post("http://localhost:11434/api/chat", json={
    "model": "qwen3-cc:latest",
    "messages": [{"role": "user", "content": "Say hello in one word"}],
    "stream": False
})
print(resp.status_code)
print(resp.text[:200])
