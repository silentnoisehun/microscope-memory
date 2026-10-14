"""TTS via Windows SAPI — reads speak_tanulas.txt with the Noémi voice."""
import os
import sys
import win32com.client

BASE = r"C:\Users\mater\.codync\bots\69ea5c5b-5783-44fd-9c74-505d948621ce\workspace\microscope-memory"
text_file = os.path.join(BASE, "speak_tanulas.txt")
wav_file  = os.path.join(BASE, "tanulas_noemi.wav")

with open(text_file, "r", encoding="utf-8") as f:
    text = f.read()

speaker = win32com.client.Dispatch("SAPI.SpVoice")

# --- List available voices ---
print("Available voices:")
voices = speaker.GetVoices()
for i in range(voices.Count):
    v = voices.Item(i)
    print(f"  [{i}] {v.GetDescription()}")

# --- Select Noémi, or fallback to any Hungarian voice ---
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
        desc = v.GetDescription()
        if "Magyar" in desc or "hu-HU" in desc:
            selected = v
            break

if selected:
    speaker.Voice = selected
    print(f"\nSelected voice: {selected.GetDescription()}")
else:
    print("\nNo Hungarian voice found — using default")

speaker.Rate = 0
speaker.Volume = 100

# --- Save to WAV ---
stream = win32com.client.Dispatch("SAPI.SpFileStream")
stream.Open(wav_file, 3, False)  # SPSFM_CreateAlways
speaker.AudioOutputStream = stream
speaker.Speak(text)
stream.Close()
print(f"\nSaved WAV: {wav_file}")

# --- Speak through default audio device ---
speaker.AudioOutputStream = None
speaker.Rate = -2   # slightly slower for intelligibility
speaker.Speak(text)
print("Speaking complete.")
