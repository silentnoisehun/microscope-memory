#!/usr/bin/env python3
"""Minimal Ollama -> Microscope Memory MCP sidecar.

POST /tool body:
{
  "name": "memory_recall",
  "arguments": {"query": "...", "k": 5}
}
"""

from __future__ import annotations

import argparse
import json
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import Any


class McpClient:
    def __init__(self, cmd: list[str]) -> None:
        self.proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=False,
            bufsize=0,
        )
        self._lock = threading.Lock()
        self._next_id = 1
        self._initialize()

    def _send(self, payload: dict[str, Any]) -> dict[str, Any]:
        if self.proc.stdin is None or self.proc.stdout is None:
            raise RuntimeError("MCP process stdio is not available")

        body = json.dumps(payload).encode("utf-8")
        msg = f"Content-Length: {len(body)}\r\n\r\n".encode("ascii") + body
        self.proc.stdin.write(msg)
        self.proc.stdin.flush()

        headers = {}
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("MCP process closed stdout")
            if line in (b"\r\n", b"\n"):
                break
            key, value = line.decode("ascii", errors="ignore").split(":", 1)
            headers[key.strip().lower()] = value.strip()

        content_length = int(headers.get("content-length", "0"))
        if content_length <= 0:
            raise RuntimeError("Invalid MCP response: missing content-length")

        data = self.proc.stdout.read(content_length)
        if not data:
            raise RuntimeError("Empty MCP response body")
        return json.loads(data.decode("utf-8"))

    def _request(self, method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
        req_id = self._next_id
        self._next_id += 1
        payload = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params or {},
        }
        response = self._send(payload)
        if "error" in response:
            raise RuntimeError(str(response["error"]))
        return response

    def _initialize(self) -> None:
        with self._lock:
            self._request(
                "initialize",
                {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "ollama-sidecar", "version": "0.1.0"},
                },
            )
            self._send(
                {
                    "jsonrpc": "2.0",
                    "method": "initialized",
                    "params": {},
                }
            )

    def list_tools(self) -> list[dict[str, Any]]:
        with self._lock:
            response = self._request("tools/list")
            return response.get("result", {}).get("tools", [])

    def call_tool(self, name: str, arguments: dict[str, Any]) -> str:
        with self._lock:
            response = self._request(
                "tools/call", {"name": name, "arguments": arguments}
            )
        result = response.get("result", {})
        content = result.get("content", [])
        if not content:
            return ""
        first = content[0]
        return str(first.get("text", ""))


class SidecarHandler(BaseHTTPRequestHandler):
    client: McpClient

    def _send_json(self, code: int, payload: dict[str, Any]) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        if self.path == "/health":
            self._send_json(200, {"ok": True})
            return
        if self.path == "/tools":
            try:
                tools = self.client.list_tools()
                self._send_json(200, {"tools": tools})
            except Exception as exc:  # pylint: disable=broad-except
                self._send_json(500, {"error": str(exc)})
            return
        self._send_json(404, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/tool":
            self._send_json(404, {"error": "not found"})
            return

        length = int(self.headers.get("Content-Length", "0"))
        data = self.rfile.read(length)
        try:
            payload = json.loads(data.decode("utf-8"))
            name = payload["name"]
            arguments = payload.get("arguments", {})
        except Exception:  # pylint: disable=broad-except
            self._send_json(400, {"error": "invalid request body"})
            return

        try:
            text = self.client.call_tool(name, arguments)
            self._send_json(200, {"ok": True, "text": text})
        except Exception as exc:  # pylint: disable=broad-except
            self._send_json(500, {"ok": False, "error": str(exc)})


def main() -> None:
    parser = argparse.ArgumentParser(description="Ollama sidecar for Microscope MCP")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=7071)
    parser.add_argument("--microscope-bin", default="microscope-mem")
    args = parser.parse_args()

    client = McpClient([args.microscope_bin, "--mcp-mode"])
    SidecarHandler.client = client

    server = HTTPServer((args.host, args.port), SidecarHandler)
    print(f"Sidecar listening on http://{args.host}:{args.port}")
    server.serve_forever()


if __name__ == "__main__":
    main()