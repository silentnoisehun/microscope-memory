import httpx, json
resp = httpx.post("http://127.0.0.1:8080/v1/messages", json={
    "model": "qwen3-cc:latest",
    "messages": [{"role": "user", "content": "Say hello in one word"}],
    "max_tokens": 50,
    "stream": True
}, timeout=60)
collected = ""
async for line in resp.aiter_lines():
    if line.startswith("data: "):
        data = json.loads(line[6:])
        if data.get("type") == "content_block_delta":
            collected += data["delta"]["text"]
        elif data.get("type") == "message_stop":
            break
print(f"Streaming test: [{collected}]")
