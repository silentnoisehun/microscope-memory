"""
Edge TTS HTTP Server for Microscope Memory
Port 8880, endpoints: /api/tts, /v1/audio/speech, /health

Default voice: hu-HU-NoemiNeural
Dual voice: stage directions in parentheses use male voice (Tamas)

Install: pip install edge-tts
"""
import asyncio
import io
import json
import re
import urllib.parse
from http.server import HTTPServer, BaseHTTPRequestHandler
import edge_tts

PORT = 8880
DEFAULT_VOICE = "hu-HU-NoemiNeural"
MALE_VOICE = "hu-HU-TamasNeural"

_PAREN_RE = re.compile(r'(\([^)]+\))')


def tts_sync(text: str, voice: str = DEFAULT_VOICE, rate: str = "+0%") -> bytes:
    """Sync wrapper for edge_tts async API — returns MP3 bytes."""
    async def _generate():
        communicate = edge_tts.Communicate(text, voice, rate=rate)
        buf = io.BytesIO()
        async for chunk in communicate.stream():
            if chunk["type"] == "audio":
                buf.write(chunk["data"])
        return buf.getvalue()

    loop = asyncio.new_event_loop()
    try:
        return loop.run_until_complete(_generate())
    finally:
        loop.close()


def tts_dual_voice(text: str, rate: str = "+0%") -> bytes:
    """Dual voice TTS: parenthesized text = male voice, rest = female voice.
    Concatenates MP3 segments."""
    segments = _PAREN_RE.split(text)
    audio_parts = []
    for seg in segments:
        seg = seg.strip()
        if not seg:
            continue
        if seg.startswith("(") and seg.endswith(")"):
            inner = seg[1:-1].strip()
            if inner:
                audio_parts.append(tts_sync(inner, MALE_VOICE, rate))
        else:
            if seg:
                audio_parts.append(tts_sync(seg, DEFAULT_VOICE, rate))
    if not audio_parts:
        return tts_sync(text, DEFAULT_VOICE, rate)
    return b"".join(audio_parts)


class TTSHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)

        if parsed.path == "/health":
            self._json_response(200, {"status": "ok", "engine": "edge-tts", "voice": DEFAULT_VOICE})
            return

        if parsed.path == "/api/tts":
            params = urllib.parse.parse_qs(parsed.query)
            text = params.get("text", [""])[0]
            voice = params.get("voice", [DEFAULT_VOICE])[0]
            speed = params.get("speed", ["1.0"])[0]

            if not text:
                self._json_response(400, {"error": "Missing 'text' parameter"})
                return

            try:
                spd = float(speed)
                pct = int((spd - 1.0) * 100)
                rate = f"{pct:+d}%"
            except ValueError:
                rate = "+0%"

            if "-" not in voice or "Neural" not in voice:
                voice = DEFAULT_VOICE

            try:
                if _PAREN_RE.search(text):
                    audio = tts_dual_voice(text, rate)
                else:
                    audio = tts_sync(text, voice, rate)
                self.send_response(200)
                self.send_header("Content-Type", "audio/mpeg")
                self.send_header("Content-Length", str(len(audio)))
                self.end_headers()
                self.wfile.write(audio)
            except Exception as e:
                self._json_response(500, {"error": str(e)})
            return

        self.send_response(404)
        self.end_headers()

    def do_POST(self):
        """POST /v1/audio/speech — OpenAI-compatible TTS endpoint"""
        parsed = urllib.parse.urlparse(self.path)

        if parsed.path in ("/v1/audio/speech", "/audio/speech"):
            content_len = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_len) if content_len > 0 else b"{}"
            try:
                req = json.loads(body)
            except json.JSONDecodeError:
                self._json_response(400, {"error": "Invalid JSON"})
                return

            text = req.get("input", "")
            voice = req.get("voice", "nova")
            speed = req.get("speed", 1.0)

            if not text:
                self._json_response(400, {"error": "Missing 'input'"})
                return

            voice_map = {
                "nova": "hu-HU-NoemiNeural",
                "alloy": "hu-HU-NoemiNeural",
                "echo": "hu-HU-TamasNeural",
                "fable": "hu-HU-NoemiNeural",
                "onyx": "hu-HU-TamasNeural",
                "shimmer": "hu-HU-NoemiNeural",
            }
            edge_voice = voice_map.get(voice, DEFAULT_VOICE)

            try:
                spd = float(speed)
                pct = int((spd - 1.0) * 100)
                rate = f"{pct:+d}%"
            except (ValueError, TypeError):
                rate = "+0%"

            try:
                if _PAREN_RE.search(text):
                    audio = tts_dual_voice(text, rate)
                else:
                    audio = tts_sync(text, edge_voice, rate)
                self.send_response(200)
                self.send_header("Content-Type", "audio/mpeg")
                self.send_header("Content-Length", str(len(audio)))
                self.end_headers()
                self.wfile.write(audio)
            except Exception as e:
                self._json_response(500, {"error": str(e)})
            return

        self.send_response(404)
        self.end_headers()

    def _json_response(self, code: int, data: dict):
        body = json.dumps(data).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, fmt, *args):
        print(f"[Edge TTS] {args[0]}")


def main():
    print(f"Edge TTS HTTP Server starting on port {PORT}")
    print(f"Voice: {DEFAULT_VOICE}")
    print(f"Endpoints:")
    print(f"  GET  /api/tts?text=...&voice=...&speed=1.0")
    print(f"  POST /v1/audio/speech (OpenAI-compatible)")
    print(f"  GET  /health")
    server = HTTPServer(("0.0.0.0", PORT), TTSHandler)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopped.")
        server.server_close()


if __name__ == "__main__":
    main()
