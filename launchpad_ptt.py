"""
Launchpad PTT Voice — Recording + VU meter on Launchpad
Nyomva: VU meter folyamatosan, elengedve: STT -> inject

A meglévő listener.py Groq STT + inject logikáját használja.
"""
import sys
sys.path.insert(0, 'E:/microscope-memory')
sys.path.insert(0, 'E:/MCP/Voice')

import os
import json
import threading
import time
import tempfile
import numpy as np
import sounddevice as sd
import soundfile as sf
from pathlib import Path

from lpminimk3 import Mode, find_launchpids, find_launchpads
import mido

# =============================================================================
# GROQ STT
# =============================================================================
GROQ_API_KEY = os.environ.get("GROQ_API_KEY", "") or "gsk_5N...2MZc"
GROQ_STT_URL = "https://api.groq.com/openai/v1/audio/transcriptions"
GROQ_MODEL   = "whisper-large-v3-turbo"

SAMPLE_RATE   = 16_000
CHANNELS      = 1
BLOCK_SIZE    = 512

def stt_groq(wav_path: str) -> str | None:
    try:
        with open(wav_path, "rb") as f:
            resp = requests.post(
                GROQ_STT_URL,
                headers={"Authorization": f"Bearer {GROQ_API_KEY}"},
                files={"file": f},
                data={"model": GROQ_MODEL, "language": "hu", "response_format": "json"},
                timeout=30
            )
        if resp.status_code == 200:
            return resp.json().get("text", "").strip() or None
    except Exception as e:
        print(f"[STT] Error: {e}")
    return None

def inject_text(text: str) -> None:
    text = text.strip()
    if not text:
        return
    try:
        import win32clipboard, win32con
        win32clipboard.OpenClipboard()
        win32clipboard.EmptyClipboard()
        win32clipboard.SetClipboardData(win32con.CF_UNICODETEXT, text)
        win32clipboard.CloseClipboard()
        time.sleep(0.05)
        subprocess.Popen(
            ["powershell", "-NoProfile", "-WindowStyle", "Hidden", "-Command",
             "Add-Type -AssemblyName System.Windows.Forms; "
             "Start-Sleep -Milliseconds 50; "
             "[System.Windows.Forms.SendKeys]::SendWait('^v')"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        time.sleep(0.15)
        subprocess.Popen(
            ["powershell", "-NoProfile", "-WindowStyle", "Hidden", "-Command",
             "Add-Type -AssemblyName System.Windows.Forms; "
             "Start-Sleep -Milliseconds 50; "
             "[System.Windows.Forms.SendKeys]::SendWait('~')"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
        )
        print(f"[Voice] -> Injektálva: {text[:60]}")
    except Exception as e:
        print(f"[Voice] Inject error: {e}")

# =============================================================================
# AUDIO STATE
# =============================================================================
class State:
    recording: bool    = False
    audio_buffer: list = []
    lock: threading.Lock = threading.Lock()

STATE = State()
_stream = None

def _audio_callback(indata, frames, time_info, status):
    with STATE.lock:
        if not STATE.recording:
            return
        STATE.audio_buffer.append(indata.copy())

def start_stream():
    global _stream
    if _stream is None:
        _stream = sd.InputStream(
            samplerate=SAMPLE_RATE, channels=CHANNELS,
            blocksize=BLOCK_SIZE, dtype="float32", callback=_audio_callback
        )
        _stream.start()

def stop_stream():
    global _stream
    if _stream:
        _stream.stop()
        _stream.close()
        _stream = None

def process_and_inject():
    with STATE.lock:
        if not STATE.audio_buffer:
            return
        buffer = list(STATE.audio_buffer)
        STATE.audio_buffer.clear()
        STATE.recording = False

    audio = np.concatenate(buffer, axis=0).flatten().astype(np.float32)
    if len(audio) / SAMPLE_RATE < 0.3:
        print("[Voice] Túl rövid.")
        return

    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tmp:
        sf.write(tmp.name, audio, SAMPLE_RATE)
        wav_path = tmp.name

    print(f"[Voice] STT feldolgozás...")
    text = stt_groq(wav_path)
    Path(wav_path).unlink(missing_ok=True)

    if text:
        print(f"[Voice STT] {text}")
        inject_text(text)
    else:
        print("[Voice] Nem értettem.")


# =============================================================================
# VU METER (Launchpad)
# =============================================================================
RED    = 5
YELLOW = 10
GREEN  = 13
ORANGE = 18
OFF    = 0

def show_vu(lp, level: float, ptt_active: bool = False):
    """VU meter: 3 oszlop (cols 4-6) + PTT gomb (col 7)"""
    # level: 0.0 - 1.0+
    vu_cols = [4, 5, 6]  # VU oszlopok
    max_height = 8

    normalized = max(0.0, min(1.5, level))

    for ci, col in enumerate(vu_cols):
        target_height = int(normalized * max_height)
        target_height = min(target_height, 8)

        for row in range(8):
            if row < target_height:
                # Szín: alacsony=zöld, közép=sárga, magas=piros
                if row < 3:
                    color = GREEN
                elif row < 6:
                    color = YELLOW
                else:
                    color = RED
                try:
                    lp.grid.led(row, col).color = color
                except:
                    pass
            else:
                try:
                    lp.grid.led(row, col).color = OFF
                except:
                    pass

    # PTT gomb: col 7, row 7
    ptt_color = ORANGE if ptt_active else OFF
    try:
        lp.grid.led(7, 7).color = ptt_color
    except:
        pass


# =============================================================================
# PTT BUTTON
# =============================================================================
# SESSION layout jobb-alsó 9. gomb
# A SESSION layout right-side column = col 8 (0-indexed: 8 → a 9. gomb)
# Bottom row = row 8

def note_to_session_rc(note: int) -> tuple[int, int] | None:
    """SESSION layout: note -> (row, col)
    Right-side buttons are col=8, bottom row is row=8"""
    # Right-side scene launch buttons: notes 89-96
    # row 0-7 = notes 89-96 (but we need to verify)
    # Bottom right corner (9th button) = note 104 or similar
    # Let's just use the raw approach: we know the pattern
    # Standard SESSION layout:
    # col 8 buttons: row 0-7 = notes 89-96 (0x59-0x60)
    # row 8 buttons: col 0-8 = notes 104-112?
    # We'll detect dynamically
    return None  # override below


# Actual discovered mapping for SESSION layout right column
# note 89 = row 0 col 8, note 90 = row 1 col 8, etc.
# note 104 = row 8 col 0 (bottom-left corner)
# note 112 = row 8 col 8 (bottom-right corner / scene 9)
SESSION_NOTE_TO_RC = {}
for row in range(8):
    SESSION_NOTE_TO_RC[89 + row] = (row, 8)   # right column
for col in range(8):
    SESSION_NOTE_TO_RC[104 + col] = (8, col)   # bottom row
SESSION_NOTE_TO_RC[112] = (8, 8)                # bottom-right corner

# A 8x8-as grid jobb szélső oszlopából a legalsó gomb (note 88) = PTT
# (A jobb oldali "scene launch" gombok a Mini MK3-n nem küldenek MIDI-t, csak LED-et)
# VU meter: a tőle balra lévő 3 oszlop (cols 4,5,6) 8 sorban
PTT_NOTE = 88


# =============================================================================
# MAIN PTT MODE
# =============================================================================
def run_ptt(duration: int | None = None) -> None:
    """Fő loop: PTT gomb figyelés + VU meter
    PTT: col 7, row 7 (8x8 grid jobb-alsó sarka)
    VU:   cols 4-6
    """
    import mido
    import requests as _req

    from lpminimk3 import Mode as LPMode, find_launchpads

    print("[PTT] Launchpad PTT indítása...")
    print("[PTT] PTT: col 7, row 7 (jobb-alsó sarok)")
    print("[PTT] VU:  cols 4-6 (balra a PTT-től)")
    print("[PTT] Kilépés: Ctrl+C")
    print()

    lp = find_launchpads()[0]
    lp.open()
    lp.mode = LPMode.LIVE

    midi_port = mido.open_input('LPMiniMK3 MIDI 0')

    # VU state
    active = [False]        # felvétel aktív
    peak_level = [0.0]
    peak_hold = [0.0]       # instant peak
    peak_decay_ts = [0.0]

    def audio_monitor():
        """Háttérszál: VU szint + PTT LED frissítés"""
        while True:
            with STATE.lock:
                if STATE.audio_buffer:
                    latest = STATE.audio_buffer[-1]
                    rms = float(np.sqrt(np.mean(latest ** 2)))
                    # Gain: adjust multiplier for your mic
                    level = min(1.0, rms * 12.0)
                    peak_level[0] = max(peak_level[0], level)
                    peak_hold[0] = level
                    peak_decay_ts[0] = time.time()

            # Peak decay
            if time.time() - peak_decay_ts[0] > 0.3:
                peak_level[0] = max(0, peak_level[0] - 0.03)

            # VU megjelenítés
            show_vu(lp, peak_level[0], ptt_active=(STATE.recording and peak_hold[0] > 0.01))

            time.sleep(0.04)

    monitor_thread = threading.Thread(target=audio_monitor, daemon=True)
    monitor_thread.start()

    start_stream()

    end_time = None if duration is None else time.time() + duration

    print("[PTT] Várakozás PTT gombra...")

    try:
        while True:
            if end_time and time.time() >= end_time:
                break

            msg = midi_port.poll()
            if msg and hasattr(msg, 'velocity'):
                note = msg.note
                vel = msg.velocity

                if note == PTT_NOTE:
                    if vel > 0 and not STATE.recording:
                        # LENYOMVA — indít
                        with STATE.lock:
                            STATE.recording = True
                            STATE.audio_buffer.clear()
                            peak_level[0] = 0.0
                            active[0] = True
                        print("[PTT] ● Felvétel indul...")

                    elif vel == 0 and STATE.recording:
                        # ELENGEDVE — feldolgozás
                        print("[PTT] ■ Feldolgozás...")
                        active[0] = False
                        with STATE.lock:
                            STATE.recording = False
                        stop_stream()

                        # STT + inject háttérben
                        threading.Thread(target=process_and_inject, daemon=True).start()

                        time.sleep(0.2)
                        start_stream()

            time.sleep(0.005)

    except KeyboardInterrupt:
        print("\n[PTT] Leállítás...")
    finally:
        active[0] = False
        with STATE.lock:
            STATE.recording = False
        stop_stream()
        midi_port.close()
        lp.grid.reset()
        lp.close()
        print("[PTT] Készen.")


# =============================================================================
# CLI
# =============================================================================
if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Launchpad PTT Voice")
    parser.add_argument("--duration", "-d", type=int, default=None, help="MP idõ")
    args = parser.parse_args()

    run_ptt(duration=args.duration)
