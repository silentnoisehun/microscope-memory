# Kliens beállítási útmutató

## 1. Cline (VS Code Extension)

### Lépések:
1. VS Code → Cline extension tab (pipac ikon)
2. ⚙️ Settings → API Provider dropdown
3. Válaszd: **OpenAI Compatible**
4. Kitöltés:
   - **Base URL**: `http://localhost:4000/v1`
   - **API Key**: `sk-<virtual key a pipeline proxy-ból>`
   - **Model**: `gyors` (vagy `eros` / `ultra`)
5. Kattints: Save / Verify

### Konfig fájl helye:
```
~/.cline/data/settings/providers.json
```

### Ha a proxyt nem localhost-on futtatod:
- Cseréld a `localhost:4000`-et a proxy tényleges IP-címére
- Pl.: `http://192.168.1.100:4000/v1`

---

## 2. Kilo Code (VS Code / JetBrains / CLI)

### Lépések (UI):
1. Kilo Code ⚙️ → Settings → Providers tab
2. Kattints: **Add Custom Provider** (vagy válaszd az OpenAI Compatible-t)
3. Kitöltés:
   - **Provider ID**: `sajat-proxy`
   - **Provider API**: `OpenAI Compatible`
   - **Base URL**: `http://localhost:4000/v1`
   - **API Key**: `sk-<virtual key>`
4. Submit

### Konfig fájl (`kilo.jsonc`):

```jsonc
{
  "$schema": "https://app.kilo.ai/config.json",
  "model": "openai-compatible/gyors",
  "provider": {
    "openai-compatible": {
      "options": {
        "apiKey": "sk-<virtual key>",
        "baseURL": "http://localhost:4000/v1"
      },
      "models": {
        "gyors": {
          "name": "Gyors modell",
          "tool_call": false,
          "limit": { "context": 128000, "output": 4096 }
        },
        "eros": {
          "name": "Erős modell",
          "tool_call": true,
          "limit": { "context": 128000, "output": 8192 }
        },
        "ultra": {
          "name": "Ultra modell",
          "tool_call": true,
          "limit": { "context": 200000, "output": 16384 }
        }
      }
    }
  }
}
```

### Task difficulty váltás:
- Egyszerű feladat kilio Code-ban: válaszd a `gyors` model-t
- Komplex: válts `eros`-ra vagy `ultra`-ra

---

## 3. OpenCode (CLI)

### Lépések:
1. Hozd létre / szerkeszd a `opencode.json` fájlt:

```json
{
  "provider": {
    "sajat-proxy": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Sajat Proxy",
      "options": {
        "baseURL": "http://localhost:4000/v1",
        "apiKey": "sk-<virtual key>"
      },
      "models": {
        "gyors": { "name": "Gyors modell" },
        "eros": { "name": "Eros modell" },
        "ultra": { "name": "Ultra modell" }
      }
    }
  }
}
```

2. Nyisd meg az OpenCode CLI-t
3. `/connect` → válaszd ki: `sajat-proxy`
4. `/model` → válaszd: `gyors`, `eros` vagy `ultra`

### Config fájl helye:
- Projekt szint: `./opencode.json`
- Globális: `~/.config/opencode/opencode.json`

---

## 4. OpenClaw (Custom Agent)

### Használat

```bash
#!/bin/bash
# pipeline-api.sh — OpenClaw API hívás a proxy-n keresztül

PROXY_URL="${PIPELINE_PROXY_URL:-http://localhost:4000/v1}"
PROXY_KEY="${PIPELINE_API_KEY:-sk-openclaw-default}"

pipeline_chat() {
  local model="$1"     # gyors | eros | ultra
  local message="$2"

  curl -s "$PROXY_URL/chat/completions" \
    -H "Authorization: Bearer $PROXY_KEY" \
    -H "Content-Type: application/json" \
    -d "{
      \"model\": \"$model\",
      \"messages\": [{\"role\": \"user\", \"content\": \"$message\"}]
    }"
}

# Példa használat:
# pipeline_chat "gyors" "Hello, write a simple function"
# pipeline_chat "ultra" "Design the architecture for a distributed system"
```

### Environment változók OpenClaw-ban:

```bash
export PIPELINE_PROXY_URL="http://localhost:4000/v1"
export PIPELINE_API_KEY="sk-<virtual key openclaw-nek>"
```

---

## 5. API Teszt (minden klienshez)

Ellenőrizd, hogy a proxy működik:

```bash
# Egyszerű chat hívás
curl http://localhost:4000/v1/chat/completions \
  -H "Authorization: Bearer sk-<virtual key>" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gyors",
    "messages": [{"role": "user", "content": "Say hello!"}]
  }'

# Model lista lekérése
curl http://localhost:4000/v1/models \
  -H "Authorization: Bearer sk-<virtual key>"
```

## Hibaelhárítás

| Probléma | Valószínű ok | Megoldás |
|---|---|---|
| "Invalid API Key" | Rossz virtual key | Generálj újat a master kulccsal |
| "Model not found" | Nincs ilyen alias | Ellenőrizd a litellm_config.yaml-t |
| Connection refused | Proxy nem fut | `bash tools/pipeline.sh start` |
| Timeout | Provider nem válaszol | Ellenőrizd a provider API kulcsot |
| Magas költség | Rossz model van kiválasztva | Válts `gyors`-ra egyszerű feladatoknál |
