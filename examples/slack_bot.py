# Microscope Memory - Slack Bot
import requests, os
from slack_sdk import WebClient
from slack_sdk.rtm import RTMClient

API = "http://localhost:6060/v1"
SLACK_TOKEN = os.environ.get("SLACK_BOT_TOKEN", "xoxb-your-token")
client = WebClient(token=SLACK_TOKEN)

@RTMClient.run_on(event="message")
def handle(msg):
    if "bot_id" in msg: return
    text = msg.get("text", "")
    # Store memory
    requests.post(f"{API}/remember", json={"text": f"[Slack] {text}", "importance": 3})
    # Recall on !memory
    if text.startswith("!memory"):
        q = text[8:]
        r = requests.get(f"{API}/recall", params={"q": q, "k": 5})
        data = r.json() if r.ok else []
        response = "
".join([m.get("text","") for m in (data or [])]) or "No memories."
        client.chat_postMessage(channel=msg["channel"], text=response)

# RTMClient(token=SLACK_TOKEN).start()
print("Slack bot example loaded.")
