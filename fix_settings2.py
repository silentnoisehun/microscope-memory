import json
s = {
    "env": {
        "ANTHROPIC_AUTH_TOKEN": "ollama",
        "ANTHROPIC_BASE_URL": "http://127.0.0.1:8080"
    },
    "model": "qwen3-cc:latest",
    "syntaxHighlightingDisabled": True,
    "skipDangerousModePermissionPrompt": True
}
with open("C:/Users/mater/.claude/settings.json", "w", encoding="utf-8") as f:
    json.dump(s, f, indent=2)
print("OK")
