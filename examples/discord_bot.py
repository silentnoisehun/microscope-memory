# Microscope Memory - Discord Bot
import discord, requests
API = "http://localhost:6060/v1"
TOKEN = "YOUR_BOT_TOKEN"

class MemBot(discord.Client):
    async def on_message(self, msg):
        if msg.author == self.user: return
        # Store all messages
        requests.post(f"{API}/remember", json={"text": f"[#{msg.channel}] {msg.author}: {msg.content}", "importance": 3})
        # !recall command
        if msg.content.startswith("!recall"):
            r = requests.get(f"{API}/recall", params={"q": msg.content[8:], "k": 5})
            data = r.json() if r.ok else []
            await msg.channel.send("
".join([m.get("text","") for m in (data or [])]) or "Not found.")

# client = MemBot(intents=discord.Intents.default())
# client.run(TOKEN)
