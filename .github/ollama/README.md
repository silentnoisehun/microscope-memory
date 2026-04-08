# Ollama + Microscope-Memory Quickstart

## 1) Build the model
```bash
ollama create microscope-llama -f .github/ollama/Modelfile
```

## 2) Run Microscope Memory MCP server
```bash
microscope-mem --mcp-mode
```

## 3) Run your Ollama sidecar/bridge
```bash
python .github/ollama/ollama_sidecar.py --host 127.0.0.1 --port 7071
```

Then call:
- `GET /health`
- `GET /tools`
- `POST /tool` with body:
```json
{
  "name": "memory_recall",
  "arguments": {
    "query": "what did we discuss about architecture?",
    "k": 5
  }
}
```

## 4) Chat with the model
```bash
ollama run microscope-llama
```
