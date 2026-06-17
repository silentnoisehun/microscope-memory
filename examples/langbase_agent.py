# Microscope Memory - Langbase Agent Integration
import requests

API = "http://localhost:6060/v1"
LANGBASE_API = "https://api.langbase.com/v1"
LANGBASE_KEY = os.environ.get("LANGBASE_API_KEY", "lb_xxx")

def get_tools():
    """Return Microscope tools for Langbase agent definition."""
    return [
        {
            "type": "function",
            "function": {
                "name": "microscope_recall",
                "description": "Recall from persistent memory",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Search query"},
                        "k": {"type": "integer", "default": 5}
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "microscope_store",
                "description": "Store in persistent memory",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "text": {"type": "string", "description": "Text to remember"},
                        "importance": {"type": "integer", "default": 5}
                    },
                    "required": ["text"]
                }
            }
        }
    ]

def recall(query, k=5):
    r = requests.get(f"{API}/recall", params={"q": query, "k": k})
    return r.json() if r.ok else []

def store(text, imp=5):
    r = requests.post(f"{API}/remember", json={"text": text, "importance": imp})
    return r.ok

print("Langbase tools defined. Use get_tools() in your Langbase pipe.")
