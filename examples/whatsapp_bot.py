# Microscope Memory - WhatsApp Bot (via Twilio)
import requests
from flask import Flask, request
from twilio.twiml.messaging_response import MessagingResponse

API = "http://localhost:6060/v1"
app = Flask(__name__)

@app.route("/webhook", methods=["POST"])
def webhook():
    msg = request.form.get("Body", "")
    sender = request.form.get("From", "")
    # Store
    requests.post(f"{API}/remember", json={"text": f"[WA {sender}] {msg}", "importance": 3})
    # Recall
    resp = MessagingResponse()
    if msg.startswith("!recall"):
        r = requests.get(f"{API}/recall", params={"q": msg[8:], "k": 3})
        data = r.json() if r.ok else []
        reply = "
".join([m.get("text","") for m in (data or [])]) or "Not found."
        resp.message(reply)
    return str(resp)

# app.run(port=5000)
print("WhatsApp bot example loaded.")
