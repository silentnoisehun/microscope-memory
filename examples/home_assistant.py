# Microscope Memory - Home Assistant Sensor
import requests
API = "http://localhost:6060/v1"

def setup_platform(hass, config, add_entities, discovery=None):
    add_entities([MicroscopeSensor()])

class MicroscopeSensor:
    def __init__(self):
        self._name = "Microscope Memory"
        self._state = None

    def update(self):
        try:
            r = requests.get(f"{API}/status", timeout=2)
            if r.ok: self._state = r.json().get("blocks", 0)
        except: self._state = None

    @property
    def name(self): return self._name
    @property
    def state(self): return self._state

# Place in custom_components/microscope_memory/sensor.py
