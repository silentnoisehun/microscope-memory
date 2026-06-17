"""
Launchpad Brain — Autonomous visual thinking shell
Operáció = vizuális minta + hangulat
Érintés-reaktív water ripple + MIDI gombfigyelés
"""
from lpminimk3 import Mode, find_launchpads
import time, random, queue, threading, math, mido

# === SZÍNEK ===
RED = 5; GREEN = 13; BLUE = 25; YELLOW = 10
CYAN = 53; MAGENTA = 45; WHITE = 127; OFF = 0
ORANGE = 47; LT_GREEN = 62

# === FÉNYERŐ ===
BRIGHT_MIN = 0x10
BRIGHT_LOW = 0x30
BRIGHT_MID = 0x60
BRIGHT_HIGH = 0x90
BRIGHT_MAX = 0x7F

# === NOTE → ROW/COL MAPPING ===
# Launchpad Mini MK3 MIDI mapping
NOTE_TO_RC = {
    11:(0,7), 12:(1,7), 13:(2,7), 14:(3,7), 15:(4,7), 16:(5,7), 17:(7,0),
    21:(0,6), 22:(1,6), 23:(2,6), 24:(3,6), 25:(4,6), 26:(5,6), 27:(6,7),
    31:(0,5), 32:(1,5), 33:(2,5), 34:(3,5), 35:(4,5), 36:(5,5), 37:(6,6), 38:(7,6),
    41:(0,4), 42:(1,4), 43:(2,4), 44:(3,4), 45:(4,4), 46:(5,4), 47:(6,5), 48:(7,5),
    51:(0,3), 52:(1,3), 53:(2,3), 54:(3,3), 55:(4,3), 56:(5,3), 57:(6,4), 58:(7,4),
    61:(0,2), 62:(1,2), 63:(2,2), 64:(3,2), 65:(4,2), 66:(5,2), 67:(6,2), 68:(7,3),
    71:(0,1), 72:(1,1), 73:(2,1), 74:(3,1), 75:(4,1), 76:(5,1), 77:(6,1), 78:(7,2),
    81:(0,0), 82:(1,0), 83:(2,0), 84:(3,0), 85:(4,0), 86:(5,0), 87:(6,0), 88:(7,1),
}
RC_TO_NOTE = {(r, c): n for n, (r, c) in NOTE_TO_RC.items()}

# === OPERÁCIÓ → VIZUÁLIS MINTÁK ===
OPERATIONS = {
    "search":      ("zoom_in",        CYAN,   "keresés — nagyító"),
    "write":       ("flow_down",      GREEN,  "írás — lefelé hullás"),
    "read":        ("wave_right",     BLUE,   "olvasás — sor hullám"),
    "delete":      ("implode",        RED,    "törlés — összeomlás"),
    "copy":        ("duplicate",      YELLOW, "másolás — tükrözés"),
    "move":        ("slide",          GREEN,  "mozgatás — elcsúszás"),
    "compile":     ("pulse_all",      YELLOW, "fordítás — pulzálás"),
    "test":        ("spark_burst",    GREEN,  "teszt — sziporkák"),
    "error":       ("alarm",          RED,    "hiba — vészvillogás"),
    "success":     ("rainbow_wave",   WHITE,  "siker — szivárvány"),
    "waiting":     ("breathe",        CYAN,   "várakozás — légzés"),
    "thinking":    ("spiral",         MAGENTA,"gondolkodás — spirál"),
    "idle":        ("crawl",          LT_GREEN,"üresjárat — kúszás"),
}


# =============================================================================
# LaunchpadBrain
# =============================================================================
class LaunchpadBrain:
    def __init__(self, brightness=BRIGHT_HIGH):
        self.lp = find_launchpads()[0]
        self.lp.open()
        self.lp.mode = Mode.LIVE
        self.running = True
        self.mood = "calm"
        self._brightness = brightness
        self._event_queue = queue.Queue()
        self._midi_port = None
        self._touch_enabled = False

        # Fényerő
        self.lp.send_message([0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x17, brightness, 0xF7])

        # Hangulat → szín
        self.mood_colors = {
            "excited": YELLOW,
            "curious":  CYAN,
            "calm":     GREEN,
            "thinking": BLUE,
            "creative": MAGENTA,
            "tired":    LT_GREEN,
            "alert":    RED,
        }

        self.current_op = None
        self.op_result = None

    # -------------------------------------------------------------------------
    # Fényerő
    # -------------------------------------------------------------------------
    def set_brightness(self, level):
        level = max(BRIGHT_MIN, min(BRIGHT_MAX, level))
        self.lp.send_message([0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x17, level, 0xF7])
        self._brightness = level

    def dim(self):      self.set_brightness(BRIGHT_LOW)
    def brighten(self): self.set_brightness(BRIGHT_HIGH)

    # -------------------------------------------------------------------------
    # Hangulat
    # -------------------------------------------------------------------------
    def set_mood(self, mood):
        if mood in self.mood_colors:
            self.mood = mood
            print(f"[mood] {mood}")

    # -------------------------------------------------------------------------
    # Operáció
    # -------------------------------------------------------------------------
    def op(self, operation, result="ok", **kwargs):
        self.current_op = operation
        self.op_result = result

        op_key = operation.lower().strip()
        if op_key not in OPERATIONS:
            op_key = "idle"

        pattern_name, default_color, _desc = OPERATIONS[op_key]
        color = kwargs.get("color", default_color)

        print(f"[op] {operation} → {pattern_name} ({result})")

        pattern_func = getattr(self, pattern_name, None)
        if pattern_func:
            pattern_func(color=color, result=result, **kwargs)
        else:
            self._fallback_flash(color=color)

    def _fallback_flash(self, color=RED):
        for _ in range(3):
            self._fill(color)
            time.sleep(0.2)
            self.lp.grid.reset()
            time.sleep(0.1)

    # -------------------------------------------------------------------------
    # ALAP SEHGGÉSEK
    # -------------------------------------------------------------------------
    def _clear(self):  self.lp.grid.reset()

    def _fill(self, color):
        for led in self.lp.grid.led_range():
            led.color = color

    # --- zoom_in ---
    def zoom_in(self, color=CYAN, **kwargs):
        for size in range(8):
            r_s, r_e = size, 7-size
            c_s, c_e = size, 7-size
            for r in range(r_s, r_e+1):
                for c in range(c_s, c_e+1):
                    if r in (r_s, r_e) or c in (c_s, c_e):
                        self.lp.grid.led(r, c).color = color
            time.sleep(0.08)
        time.sleep(0.4)
        self._clear()

    # --- flow_down ---
    def flow_down(self, color=GREEN, **kwargs):
        for col in range(8):
            for row in range(8):
                self.lp.grid.led(row, col).color = color
                time.sleep(0.03)
        time.sleep(0.3)
        self._clear()

    # --- wave_right ---
    def wave_right(self, color=BLUE, **kwargs):
        for col in range(8):
            for row in range(8):
                self.lp.grid.led(row, col).color = color
            time.sleep(0.06)
        for col in range(8):
            for row in range(8):
                self.lp.grid.led(row, col).color = OFF
            time.sleep(0.06)
        self._clear()

    # --- implode ---
    def implode(self, color=RED, **kwargs):
        for wave in range(4):
            for r in range(wave, 8-wave):
                for c in range(wave, 8-wave):
                    d = max(abs(r-3.5), abs(c-3.5))
                    if int(d) == 7-wave:
                        self.lp.grid.led(r, c).color = color
            time.sleep(0.1)
        self._fill(OFF)
        time.sleep(0.3)
        self._clear()

    # --- duplicate ---
    def duplicate(self, color=YELLOW, **kwargs):
        for half in range(4):
            for r in range(8):
                c1 = 3 - half
                c2 = 4 + half
                if c1 >= 0:
                    self.lp.grid.led(r, c1).color = color
                if c2 < 8:
                    self.lp.grid.led(r, c2).color = color
            time.sleep(0.08)
        time.sleep(0.3)
        self._clear()

    # --- slide ---
    def slide(self, color=GREEN, **kwargs):
        for shift in range(8):
            for r in range(8):
                c = (shift + r) % 8
                self.lp.grid.led(r, c).color = color
                prev = (shift + r - 1) % 8
                self.lp.grid.led(r, prev).color = OFF
            time.sleep(0.07)
        self._clear()

    # --- pulse_all ---
    def pulse_all(self, color=YELLOW, **kwargs):
        for _ in range(4):
            for b in list(range(0, 127, 10)) + list(range(127, 0, -10)):
                for led in self.lp.grid.led_range():
                    led.color = b if b > 10 else 0
                time.sleep(0.02)
        self._clear()

    # --- spark_burst ---
    def spark_burst(self, color=GREEN, **kwargs):
        c = self.mood_colors.get(self.mood, color)
        for _ in range(40):
            r, col = random.randint(0,7), random.randint(0,7)
            self.lp.grid.led(r, col).color = random.choice([c, WHITE, color])
            time.sleep(0.02)
        self._clear()

    # --- alarm ---
    def alarm(self, color=RED, **kwargs):
        for _ in range(6):
            self._fill(RED)
            time.sleep(0.1)
            self._clear()
            time.sleep(0.1)
        self._clear()

    # --- rainbow_wave ---
    def rainbow_wave(self, color=WHITE, **kwargs):
        rainbow = [RED, ORANGE, YELLOW, GREEN, CYAN, BLUE, MAGENTA, WHITE]
        for offset in range(8):
            for r in range(8):
                c = (r + offset) % 8
                self.lp.grid.led(r, c).color = rainbow[r]
            time.sleep(0.1)
        self._clear()

    # --- breathe ---
    def breathe(self, color=CYAN, **kwargs):
        c = self.mood_colors.get(self.mood, color)
        for _ in range(4):
            for b in list(range(0, 100, 6)) + list(range(100, 0, -6)):
                for led in self.lp.grid.led_range():
                    led.color = b if b > 10 else 0
                time.sleep(0.04)
        self._clear()

    # --- spiral ---
    def spiral(self, color=MAGENTA, **kwargs):
        dirs = [(0,1),(1,0),(0,-1),(-1,0)]
        r, c, d = 0, 0, 0
        visited = set()
        for step in range(64):
            self.lp.grid.led(r, c).color = color
            visited.add((r,c))
            time.sleep(0.04)
            dr, dc = dirs[d % 4]
            nr, nc = r+dr, c+dc
            if 0 <= nr < 8 and 0 <= nc < 8 and (nr,nc) not in visited:
                r, c = nr, nc
            else:
                d += 1
                dr, dc = dirs[d % 4]
                r, c = r+dr, c+dc
        time.sleep(0.4)
        self._clear()

    # --- crawl ---
    def crawl(self, color=LT_GREEN, **kwargs):
        c = self.mood_colors.get(self.mood, color)
        for row in range(8):
            for col in range(8):
                self.lp.grid.led(row, col).color = c
                time.sleep(0.05)
                self.lp.grid.led(row, col).color = OFF
        self._clear()

    # --- expand ---
    def expand(self, color=GREEN, **kwargs):
        for wave in range(8):
            for r in range(8):
                for c in range(8):
                    d = max(abs(r-3.5), abs(c-3.5))
                    if int(d) <= wave:
                        self.lp.grid.led(r, c).color = color
            time.sleep(0.1)
        self._clear()

    # --- heartbeat ---
    def heartbeat(self, color=RED, **kwargs):
        for _ in range(3):
            # expand
            for size in range(1, 5):
                r_s, r_e = 3-size, 4+size
                c_s, c_e = 3-size, 4+size
                for r in range(max(0,r_s), min(8,r_e+1)):
                    for c in range(max(0,c_s), min(8,c_e+1)):
                        self.lp.grid.led(r, c).color = color
                time.sleep(0.08)
            self._clear()
            time.sleep(0.25)
        self._clear()

    # --- water_ripple (érintésre) ---
    def water_ripple(self, r_center, c_center, color=CYAN, duration=1.8, waves=3):
        """Vizcsepp fodrozódás — kifelé haladó körhullámok"""
        start = time.time()
        wave_speed = 4.5

        for wave_idx in range(waves):
            wave_t0 = wave_idx * 0.5
            for step in range(80):
                t = time.time() - start - wave_t0
                if t < 0:
                    time.sleep(0.02)
                    continue
                radius = t * wave_speed
                if radius > 10:
                    break

                alpha = max(0.0, 1.0 - radius / 7.0)
                for r in range(8):
                    for c in range(8):
                        dist = math.sqrt((r - r_center)**2 + (c - c_center)**2)
                        if abs(dist - radius) < 0.6:
                            brightness = max(1, int(127 * alpha))
                            self.lp.grid.led(r, c).color = brightness
                        elif dist < radius - 0.5:
                            inner = max(1, int(35 * alpha))
                            self.lp.grid.led(r, c).color = inner

                time.sleep(0.04)

        self._clear()

    # -------------------------------------------------------------------------
    # ÉRINTÉS-REAKTÍV MÓD
    # -------------------------------------------------------------------------
    def touch_reactive(self, duration=None):
        """Folyamatosan figyeli a MIDI input-ot és water_ripple-t indít"""
        if self._midi_port is None:
            self._midi_port = mido.open_input('LPMiniMK3 MIDI 0')

        active_ripples = []   # list of (r, c, t0, color)
        touch_count = [0]

        def poll_midi():
            while self._touch_enabled:
                msg = self._midi_port.poll()
                if msg and hasattr(msg, 'velocity') and msg.velocity > 0:
                    note = msg.note
                    if note in NOTE_TO_RC:
                        r, c = NOTE_TO_RC[note]
                        color = self.mood_colors.get(self.mood, CYAN)
                        active_ripples.append((r, c, time.time(), color))
                        touch_count[0] += 1
                        print(f"[touch] row={r} col={c} #{touch_count[0]}")
                time.sleep(0.005)

        self._touch_enabled = True
        poll_thread = threading.Thread(target=poll_midi, daemon=True)
        poll_thread.start()

        print("[touch] Érintés-reaktív mód aktiválva. Érintsd meg a gombokat!")
        print("[touch] Ctrl+C vagy op quit a kilépéshez")

        end_time = None if duration is None else time.time() + duration

        try:
            while self._touch_enabled:
                if end_time and time.time() >= end_time:
                    break

                current_time = time.time()
                still_active = []

                for (r, c, t0, color) in active_ripples:
                    age = current_time - t0
                    if age < 2.5:
                        # Max 2 hullámfront egyszerre
                        for wi in range(2):
                            radius = (age * 5.0 + wi * 2.5) % 9
                            alpha = max(0.0, 1.0 - radius / 7.0)
                            if alpha < 0.05:
                                continue
                            for row in range(8):
                                for col in range(8):
                                    dist = math.sqrt((row-r)**2 + (col-c)**2)
                                    if abs(dist - radius) < 0.5:
                                        self.lp.grid.led(row, col).color = int(100 * alpha)
                            time.sleep(0.01)
                        still_active.append((r, c, t0, color))

                active_ripples = still_active

                # Ha nincs ripple: halvány breathing háttér
                if not active_ripples:
                    phase = (time.time() % 3.0) / 3.0
                    b = int(20 * (0.5 + 0.5 * math.sin(phase * 2 * math.pi)))
                    for r in range(8):
                        for c in range(8):
                            self.lp.grid.led(r, c).color = b if b > 4 else 0

                time.sleep(0.04)

        except KeyboardInterrupt:
            pass
        finally:
            self._touch_enabled = False
            self._clear()
            print(f"[touch] Leállva. Összesen {touch_count[0]} érintés.")

    def stop_touch(self):
        self._touch_enabled = False

    # -------------------------------------------------------------------------
    # ÜZENET ALAPÚ VÉGZETÉS
    # -------------------------------------------------------------------------
    def run(self):
        print("[brain] Indulás... (op, mood, dim/brighten, touch, test, quit)")
        self.heartbeat(color=GREEN)
        while self.running:
            try:
                msg = self._event_queue.get(timeout=0.5)
                cmd = msg.get("cmd")

                if cmd == "op":
                    self.op(msg.get("name", "idle"), result=msg.get("result", "ok"),
                           color=msg.get("color"))

                elif cmd == "mood":
                    self.set_mood(msg.get("name", "calm"))

                elif cmd == "dim":
                    self.dim()
                elif cmd == "brighten":
                    self.brighten()
                elif cmd == "brightness":
                    self.set_brightness(msg.get("level", BRIGHT_MID))

                elif cmd == "touch":
                    self.touch_reactive(duration=msg.get("duration"))

                elif cmd == "stop_touch":
                    self.stop_touch()

                elif cmd == "test":
                    for name in ["zoom_in","flow_down","wave_right","implode",
                                 "duplicate","slide","pulse_all","spark_burst",
                                 "alarm","rainbow_wave","breathe","spiral",
                                 "crawl","expand","heartbeat"]:
                        fn = getattr(self, name, None)
                        if fn:
                            print(f"  >> {name}")
                            fn(color=self.mood_colors.get(self.mood, CYAN))
                            time.sleep(0.3)

                elif cmd == "quit":
                    self.running = False

            except queue.Empty:
                pass

        self._clear()
        print("[brain] Lezárva")

    def close(self):
        self.running = False
        self._touch_enabled = False
        self._clear()
        if self._midi_port:
            self._midi_port.close()
        self.lp.close()
        print("[brain] Lezárva")


# =============================================================================
# CLI SHELL
# =============================================================================
if __name__ == "__main__":
    brain = LaunchpadBrain()

    print()
    print("=== Launchpad Brain CLI ===")
    print("op <name> [ok|error|waiting]  — operáció")
    print("mood <name>                    — hangulat")
    print("dim / brighten                 — fényerő")
    print("brightness <0-127>             — fényerő közvetlen")
    print("touch [sec]                    — érintés-reaktív mód")
    print("test                           — összes minta tesztje")
    print("quit                           — kilépés")
    print()

    import threading

    def reader():
        import sys
        while brain.running:
            try:
                line = input("> ").strip()
                if not line:
                    continue
                parts = line.split()
                cmd = parts[0].lower()

                if cmd == "op" and len(parts) >= 2:
                    result = parts[2] if len(parts) > 2 else "ok"
                    brain._event_queue.put({"cmd": "op", "name": parts[1], "result": result})
                elif cmd == "mood" and len(parts) >= 2:
                    brain._event_queue.put({"cmd": "mood", "name": parts[1]})
                elif cmd == "dim":
                    brain._event_queue.put({"cmd": "dim"})
                elif cmd == "brighten":
                    brain._event_queue.put({"cmd": "brighten"})
                elif cmd == "brightness" and len(parts) >= 2:
                    try:
                        lvl = int(parts[1])
                        brain._event_queue.put({"cmd": "brightness", "level": lvl})
                    except:
                        pass
                elif cmd == "touch":
                    dur = int(parts[1]) if len(parts) > 1 else None
                    brain._event_queue.put({"cmd": "touch", "duration": dur})
                elif cmd == "test":
                    brain._event_queue.put({"cmd": "test"})
                elif cmd in ("quit", "exit", "q"):
                    brain._event_queue.put({"cmd": "quit"})
                    break
            except EOFError:
                break
            except Exception as e:
                print(f"Hiba: {e}")

    t = threading.Thread(target=reader, daemon=True)
    t.start()

    try:
        brain.run()
    except KeyboardInterrupt:
        brain._event_queue.put({"cmd": "quit"})
    finally:
        brain.close()
