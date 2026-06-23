---
name: pipeline
description: >-
  AI Gateway Proxy pipeline — LiteLLM alapú multi-provider routing rendszer.
  Használd ha: (1) API proxy-t kell telepíteni/konfigurálni, (2) provider-eket
  kell bekötni (NVIDIA, OpenRouter, Gemini, SambaNova), (3) round-robin load
  balancing kell, (4) feladat nehézség alapján kell modellt választani,
  (5) klienseket kell bekötni (Cline, Kilo Code, OpenCode, OpenClaw),
  (6) virtuális API kulcsokat kell managelni, (7) cost tracking.
  Triggers: "proxy", "api gateway", "pipeline", "round-robin", "litellm",
  "provider routing", "model fallback", "task difficulty routing".
metadata:
  openclaw:
    requires:
      bins: ["docker", "curl", "bash"]
---

# Pipeline Skill — AI Gateway Proxy

## Áttekintés

Teljes API proxy pipeline LiteLLM-mel. Egy OpenAI-kompatibilis endpoint mögé
több provider-t teszel round-robin load balancinggel, task difficulty
routinggal, fallback lánccal.

```
Kliens → proxy:4000/v1 → LiteLLM Router → [NVIDIA, OpenRouter, Gemini, SambaNova]
```

## Telepítés

### 1. docker-compose.yml

Lásd: `references/docker-compose.yml`

### 2. Provider konfig

Lásd: `references/litellm_config.yaml`

### 3. .env fájl

```bash
# Kötelező
LITELLM_MASTER_KEY=sk-master-XXXXX
DATABASE_URL=sqlite:///litellm.db

# Provider API kulcsok
NVIDIA_API_KEY=nvapi-XXXXX
OPENROUTER_API_KEY=sk-or-v1-XXXXX
GEMINI_API_KEY=AIzaXXXXX
SAMBANOVA_API_KEY=sambanova-XXXXX
```

### 4. Indítás

A skill könyvtárából:
```bash
cd .github/openclaw/skills/pipeline
bash tools/pipeline.sh start
```

---

## Routing stratégia

| Modell alias | Feladat | Provider-ek | Stratégia |
|---|---|---|---|
| `gyors` | Autocomplete, chat, egyszerű kód | OpenRouter GPT-4o-mini, SambaNova Llama 8B | Round-robin |
| `eros` | Kódírás, refaktor, debug | Gemini Flash, NVIDIA Llama 70B, OpenRouter Claude | Round-robin |
| `ultra` | Architektúra, komplex reasoning | OpenRouter Claude Sonnet, NVIDIA Llama 405B | Latency-based |

### Fallback lánc

```
ultra ✗ → eros → gyors
eros ✗ → gyors
Minden hibás → utolsó talpon lévő provider
```

---

## Kliens beállítások

Részletes útmutató: `references/clients.md`

| Kliens | Config helye | Provider típus | Base URL |
|---|---|---|---|
| **Cline** | VS Code settings → OpenAI Compatible | `OpenAI Compatible` | `http://proxy:4000/v1` |
| **Kilo Code** | `kilo.jsonc` → `provider.openai-compatible` | `openai-compatible` | `http://proxy:4000/v1` |
| **OpenCode** | `opencode.json` → `provider` | `@ai-sdk/openai-compatible` | `http://proxy:4000/v1` |
| **OpenClaw** | `tools/pipeline-api.sh` | OpenAI-compat API call | `http://proxy:4000/v1` |

---

## Virtuális API kulcsok

Minden kliens kap egy saját virtual key-t a master kulccsal:

```bash
# Kulcs generálás
curl -X POST http://localhost:4000/key/generate \
  -H "Authorization: Bearer $LITELLM_MASTER_KEY" \
  -d '{
    "models": ["gyors", "eros", "ultra"],
    "metadata": {"user": "cline"},
    "max_budget": 10.0,
    "budget_duration": "30d"
  }'

# Válasz: {"key": "sk-XXXXX"}
```

---

## Parancsok

### Proxy indítás/leállítás

```bash
# Futtatás a skill könyvtárából:
# cd .github/openclaw/skills/pipeline

bash tools/pipeline.sh start    # docker compose up -d
bash tools/pipeline.sh stop     # docker compose down
bash tools/pipeline.sh logs     # naplók nézése
bash tools/pipeline.sh status   # státusz ellenőrzés
bash tools/pipeline.sh restart  # újraindítás
```

### Admin műveletek

```bash
# Kulcsok listázása
curl http://localhost:4000/key/list \
  -H "Authorization: Bearer $LITELLM_MASTER_KEY"

# Használati statisztika
curl http://localhost:4000/spend/keys \
  -H "Authorization: Bearer $LITELLM_MASTER_KEY"

# Proxy health check
curl http://localhost:4000/health
```

---

## Provider-ek

Részletes provider config: `references/providers.md`

| Provider | Modellek | Előny | API kulcs beszerzés |
|---|---|---|---|
| **OpenRouter** | 200+ modell | Legtöbb opció | openrouter.ai/keys |
| **Gemini** | Gemini 2.0 Flash/Pro | Ingyenes réteg | aistudio.google.com |
| **NVIDIA NIM** | Llama 3.1 70B/405B | Nagy modellek ingyen | build.nvidia.com |
| **SambaNova** | Llama 3.1 8B/405B | Gyors inference | cloud.sambanova.ai |

---

## Használati példák

### Cline beállítás

```json
// ~/.cline/data/settings/providers.json
{
  "provider": "openai-compatible",
  "baseUrl": "http://localhost:4000/v1",
  "apiKey": "sk-kulcs-cline-nek",
  "model": "gyors"
}
```

### OpenCode beállítás

```json
// opencode.json
{
  "provider": {
    "sajat-proxy": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Sajat Proxy",
      "options": {
        "baseURL": "http://localhost:4000/v1",
        "apiKey": "sk-kulcs-opencode-nek"
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

### OpenClaw használat

```bash
# API hívás a proxy-n keresztül
PROXY_URL="http://localhost:4000/v1"
PROXY_KEY="sk-kulcs-openclaw-nek"

curl "$PROXY_URL/chat/completions" \
  -H "Authorization: Bearer $PROXY_KEY" \
  -d '{
    "model": "gyors",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

---

## Karbantartás

### Naplózás

A LiteLLM automatikusan naplóz minden hívást az SQLite adatbázisba.
Használat lekérdezés:

```bash
curl http://localhost:4000/spend/keys \
  -H "Authorization: Bearer $LITELLM_MASTER_KEY"
```

### Frissítés

```bash
docker-compose pull
docker-compose up -d
```

### Hibakeresés

```bash
# Proxy naplók
docker-compose logs -f litellm

# Health check
curl -s http://localhost:4000/health | jq .

# Model lista
curl -s http://localhost:4000/models \
  -H "Authorization: Bearer $LITELLM_MASTER_KEY" | jq .
```

## Etikai irányelvek

- Minden kliens saját virtual key-t kap → auditálható használat
- Budget limit állítható kulcsonként → nem lehet elszállni
- Naplózás minden hívásról → visszakövethetőség
- Content policy fallback → tiltott tartalom esetén provider váltás
