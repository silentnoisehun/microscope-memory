import json, httpx, uvicorn, sys
from fastapi import FastAPI, Request, Response
from fastapi.responses import StreamingResponse, JSONResponse

app = FastAPI()

OLLAMA_BASE = "http://localhost:11434"
MODEL = "gemma4:e2b"

@app.get("/health")
async def health():
    return {"status": "ok"}

@app.get("/v1/models")
async def list_models():
    return JSONResponse({
        "data": [
            {"id": "gemma4:e2b", "object": "model", "created": 0, "owned_by": "ollama"},
            {"id": "claude-sonnet-4-20250514", "object": "model", "created": 0, "owned_by": "ollama"},
            {"id": "claude-3-5-sonnet-20241022", "object": "model", "created": 0, "owned_by": "ollama"},
            {"id": "claude-3-haiku-20240307", "object": "model", "created": 0, "owned_by": "ollama"}
        ]
    })

@app.post("/v1/messages")
async def proxy_messages(request: Request):
    body = await request.json()
    system = body.get("system", "")
    messages = body.get("messages", [])
    stream = body.get("stream", False)
    
    ollama_messages = []
    if system:
        ollama_messages.append({"role": "system", "content": system})
    for msg in messages:
        role = msg["role"]
        content = msg.get("content", "")
        if isinstance(content, list):
            text_parts = [p["text"] for p in content if p.get("type") == "text"]
            content = " ".join(text_parts)
        ollama_messages.append({"role": role, "content": content})
    
    ollama_body = {
        "model": MODEL,
        "messages": ollama_messages,
        "stream": stream,
        "options": {"num_predict": 4096, "temperature": 0.7}
    }
    
    async with httpx.AsyncClient(timeout=300.0) as client:
        if stream:
            async with client.stream("POST", f"{OLLAMA_BASE}/api/chat", json=ollama_body) as resp:
                async def generate():
                    async for line in resp.aiter_lines():
                        if not line.strip():
                            continue
                        try:
                            data = json.loads(line)
                            if data.get("done"):
                                yield f"data: {json.dumps({'type': 'message_stop', 'delta': {'stop_reason': 'end_turn', 'stop_sequence': None}})}\n\n"
                                break
                            content = data.get("message", {}).get("content", "")
                            if content:
                                yield f"data: {json.dumps({'type': 'content_block_delta', 'index': 0, 'delta': {'type': 'text_delta', 'text': content}})}\n\n"
                        except:
                            pass
                return StreamingResponse(generate(), media_type="text/event-stream")
        else:
            resp = await client.post(f"{OLLAMA_BASE}/api/chat", json=ollama_body)
            data = resp.json()
            content = data.get("message", {}).get("content", "")
            result = {
                "id": "msg_ollama",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": content}],
                "model": MODEL,
                "stop_reason": "end_turn",
                "stop_sequence": None,
                "usage": {"input_tokens": 0, "output_tokens": 0}
            }
            return Response(content=json.dumps(result), media_type="application/json")

if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8080, log_level="warning")
