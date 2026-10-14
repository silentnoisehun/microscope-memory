"""Quick TTS — just save WAV, no playback."""
import os, sys, win32com.client

BASE = r"C:\Users\mater\.codync\bots\69ea5c5b-5783-44fd-9c74-505d948621ce\workspace\microscope-memory"
text_file = os.path.join(BASE, "speak_tanulas.txt")
wav_file  = os.path.join(BASE, "tanulas_noemi.wav")

with open(text_file, "r", encoding="utf-8") as f:
    text = f.read()

speaker = win32com.client.Dispatch("SAPI.SpVoice")

# Find Noémi voice
voices = speaker.GetVoices()
selected = None
for i in range(voices.Count):
    v = voices.Item(i)
    desc = v.GetDescription()
    if "Noémi" in desc or "Noemi" in desc:
        selected = v
        break
if not selected:
    for i in range(voices.Count):
        v = voices.Item(i)
        if "Magyar" in v.GetDescription():
            selected = v
            break

if selected:
    speaker.Voice = selected
    print(f"Voice: {selected.GetDescription()}")
else:
    print("Using default voice")

speaker.Rate = -2
speaker.Volume = 100

# Save to WAV only
stream = win32com.client.Dispatch("SAPI.SpFileStream")
stream.Open(wav_file, 3, False)
speaker.AudioOutputStream = stream
speaker.Speak(text)
stream.Close()

print(f"WAV saved: {wav_file}")
print(f"File size: {os.path.getsize(wav_file)} bytes")
print("Done — no playback, just file.")
