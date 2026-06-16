# Mobile Memory Gateway (Android + iOS)

Use the unified endpoint:
- `POST /v1/mobile/chat`

It performs:
1. User-scoped memory recall
2. Provider call (`ollama`, `openai`, `gemini`)
3. Persistent memory write-back

## Example: Ollama
```bash
curl -X POST http://localhost:6060/v1/mobile/chat \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "user-123",
    "message": "Mi volt a tegnapi terv?",
    "provider": "ollama",
    "model": "llama3",
    "api_base": "http://127.0.0.1:11434"
  }'
```

## Example: OpenAI
```bash
curl -X POST http://localhost:6060/v1/mobile/chat \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "user-123",
    "message": "Summarize my prior preferences.",
    "provider": "openai",
    "model": "gpt-4o-mini",
    "api_key": "YOUR_OPENAI_KEY",
    "api_base": "https://api.openai.com"
  }'
```

## Example: Gemini
```bash
curl -X POST http://localhost:6060/v1/mobile/chat \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "user-123",
    "message": "What did I ask before?",
    "provider": "gemini",
    "model": "gemini-1.5-flash",
    "api_key": "YOUR_GEMINI_KEY"
  }'
```

## Android/iOS notes
- Keep `user_id` stable (device account ID / app user ID).
- Set `remember_user=true` and `remember_assistant=true` (defaults) for persistence.
- For offline-friendly behavior, queue failed requests locally and replay when online.
- For production, put API keys on server side only (do not ship secrets in mobile app binaries).