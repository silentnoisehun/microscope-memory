# Provider referenciák

## NVIDIA NIM (build.nvidia.com)

| Tulajdonság | Érték |
|---|---|
| **Modellek** | Llama 3.1 70B, Llama 3.1 405B, Mistral Large |
| **API endpoint** | `https://integrate.api.nvidia.com/v1` |
| **Kulcs szerzés** | [build.nvidia.com](https://build.nvidia.com/) → Sign up → API kulcs |
| **Ingyen** | Igen — kezdeti credit, napi limit |
| **Rate limit** | ~100-300 RPM modelltől függően |
| **Tool calling** | Igen (Llama 3.1) |
| **LiteLLM prefix** | `nvidia_nim/` |

### Config:
```yaml
litellm_params:
  model: nvidia_nim/meta/llama-3.1-405b-instruct
  api_key: os.environ/NVIDIA_API_KEY
```

---

## OpenRouter

| Tulajdonság | Érték |
|---|---|
| **Modellek** | 200+ (GPT, Claude, Gemini, Llama, Mistral, stb.) |
| **API endpoint** | `https://openrouter.ai/api/v1` |
| **Kulcs szerzés** | [openrouter.ai/keys](https://openrouter.ai/keys) |
| **Ingyen** | Ingyen modellek + $1 kezdő credit |
| **Rate limit** | Modelltől függ |
| **Tool calling** | Igen |
| **LiteLLM prefix** | `openrouter/` |

### Config:
```yaml
litellm_params:
  model: openrouter/anthropic/claude-sonnet-4-20250514
  api_key: os.environ/OPENROUTER_API_KEY
```

### Jó modellek OpenRouter-ön:
- `openai/gpt-4o-mini` — gyors, olcsó
- `anthropic/claude-sonnet-4-20250514` — erős kódolás
- `anthropic/claude-opus-4-20250514` — komplex reasoning
- `meta-llama/llama-3.1-405b-instruct` — nagy kontextus

---

## Google Gemini

| Tulajdonság | Érték |
|---|---|
| **Modellek** | Gemini 2.0 Flash, Gemini 2.0 Pro, Gemini 1.5 Pro |
| **API endpoint** | `https://generativelanguage.googleapis.com/v1beta/openai/` |
| **Kulcs szerzés** | [aistudio.google.com](https://aistudio.google.com/apikey) |
| **Ingyen** | **Igen!** Gemini 2.0 Flash ingyen réteg |
| **Rate limit** | 1500 RPM ingyen |
| **Tool calling** | Igen |
| **LiteLLM prefix** | `gemini/` |

### Config:
```yaml
litellm_params:
  model: gemini/gemini-2.0-flash-001
  api_key: os.environ/GEMINI_API_KEY
```

---

## SambaNova

| Tulajdonság | Érték |
|---|---|
| **Modellek** | Llama 3.1 8B, Llama 3.1 70B, Llama 3.1 405B |
| **API endpoint** | `https://api.sambanova.ai/v1` |
| **Kulcs szerzés** | [cloud.sambanova.ai](https://cloud.sambanova.ai/) |
| **Ingyen** | **Igen!** Ingyen API kulcs regisztrációval |
| **Rate limit** | Bőkezű ingyen réteg |
| **Tool calling** | Igen (Llama 3.1) |
| **LiteLLM prefix** | `sambanova/` |

### Config:
```yaml
litellm_params:
  model: sambanova/Meta-Llama-3.1-8B-Instruct
  api_key: os.environ/SAMBANOVA_API_KEY
```

---

## Provider összehasonlítás

| Provider | Ingyen? | RPM | Tool calling | Legjobb modell |
|---|---|---|---|---|
| **NVIDIA NIM** | ✅ Kezdeti credit | 100-300 | ✅ | Llama 3.1 405B |
| **OpenRouter** | ✅ Ingyen modellek + $1 | Változó | ✅ | Claude Sonnet 4 |
| **Gemini** | ✅ **Flash ingyen** | 1500 | ✅ | Gemini 2.0 Flash |
| **SambaNova** | ✅ **Ingyen regisztráció** | Magas | ✅ | Llama 3.1 405B |

---

## Ajánlott routing mátrix

| Kliens | Egyszerű feladat | Közepes feladat | Komplex feladat |
|---|---|---|---|
| Cline | gyors → OpenRouter Mini | eros → Gemini Flash | ultra → Claude Opus |
| Kilo Code | gyors → SambaNova 8B | eros → NVIDIA 70B | ultra → NVIDIA 405B |
| OpenCode | gyors → RR mindkettő | eros → RR mindhárom | ultra → Claude Sonnet |
| OpenClaw | gyors → leggyorsabb | eros → legjobb ár/érték | ultra → legerősebb |
