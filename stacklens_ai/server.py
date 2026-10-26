"""Dependency-free HTTP demo for StackLens AI.

Run from repository root: python3 -m stacklens_ai.server
Set NEBIUS_API_KEY on the server to enable real NVIDIA Nemotron inference.
"""

from __future__ import annotations

import json
import os
from collections import defaultdict, deque
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Lock
from time import monotonic
from typing import Any
from urllib.parse import urlsplit

from .engine import SnapshotError, evaluate
from .nebius import NebiusError, available, explain

PACKAGE_DIR = Path(__file__).resolve().parent
STATIC = PACKAGE_DIR / "static"
FIXTURES = PACKAGE_DIR / "fixtures"
EXAMPLES = {
    "driver": "driver-too-old.json",
    "selectors": "selector-conflict.json",
    "components": "driver-component-mismatch.json",
}
MAX_BODY = 256 * 1024
MODEL_BUDGET = int(os.environ.get("STACKLENS_MODEL_CALL_BUDGET", "100"))
_calls = 0
_usage: dict[str, deque[float]] = defaultdict(deque)
_lock = Lock()


def _allow_model_call(ip: str) -> bool:
    global _calls
    with _lock:
        now = monotonic()
        history = _usage[ip]
        while history and now - history[0] > 60:
            history.popleft()
        if len(history) >= 4 or _calls >= MODEL_BUDGET:
            return False
        history.append(now)
        _calls += 1
        return True


class Handler(BaseHTTPRequestHandler):
    server_version = "StackLensAI/0.1"

    def log_message(self, fmt: str, *args: Any) -> None:
        # Never log raw snapshots or credentials.
        print(f"[stacklens-ai] {self.address_string()} {fmt % args}")

    def _send(self, status: int, data: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header(
            "Content-Security-Policy",
            "default-src 'self'; connect-src 'self'; img-src 'self'; "
            "script-src 'self'; style-src 'self'; object-src 'none'; base-uri 'none'",
        )
        self.end_headers()
        self.wfile.write(data)

    def _json(self, status: int, body: dict[str, Any]) -> None:
        self._send(status, json.dumps(body).encode("utf-8"), "application/json; charset=utf-8")

    def do_GET(self) -> None:
        path = urlsplit(self.path).path
        if path == "/api/health":
            self._json(200, {"ok": True, "ai_configured": available()})
            return
        if path == "/api/examples":
            self._json(200, {"examples": [{"id": k, "title": v.replace(".json", "").replace("-", " ").title()} for k, v in EXAMPLES.items()]})
            return
        if path.startswith("/api/examples/"):
            key = path[len("/api/examples/") :]
            if key not in EXAMPLES:
                self._json(404, {"error": "Unknown example."})
                return
            self._send(200, (FIXTURES / EXAMPLES[key]).read_bytes(), "application/json; charset=utf-8")
            return

        files = {
            "/": ("index.html", "text/html; charset=utf-8"),
            "/index.html": ("index.html", "text/html; charset=utf-8"),
            "/app.js": ("app.js", "text/javascript; charset=utf-8"),
            "/styles.css": ("styles.css", "text/css; charset=utf-8"),
        }
        target = files.get(path)
        if not target:
            self._json(404, {"error": "Not found."})
            return
        name, media = target
        self._send(200, (STATIC / name).read_bytes(), media)

    def do_POST(self) -> None:
        if urlsplit(self.path).path != "/api/analyze":
            self._json(404, {"error": "Not found."})
            return
        try:
            length = int(self.headers.get("Content-Length", "-1"))
        except ValueError:
            length = -1
        if length <= 0 or length > MAX_BODY:
            self._json(413, {"error": "Send a JSON request no larger than 256 KiB."})
            return
        if self.headers.get("Content-Type", "").split(";")[0].strip() != "application/json":
            self._json(415, {"error": "Content-Type must be application/json."})
            return
        try:
            payload = json.loads(self.rfile.read(length))
            if not isinstance(payload, dict):
                raise SnapshotError("Request must be an object.")
            findings = evaluate(payload.get("snapshot"))
        except (ValueError, UnicodeDecodeError, SnapshotError) as error:
            self._json(400, {"error": str(error)[:250]})
            return

        result: dict[str, Any] = {
            "findings": findings,
            "finding_count": len(findings),
            "ai_status": "not_requested",
            "ai_explanations": [],
            "note": "No finding does not mean the environment is healthy. "
            "Unimplemented PyTorch and ELF collectors are not evaluated.",
        }
        if payload.get("use_ai") is True and findings:
            if not available():
                result["ai_status"] = "not_configured"
            elif not _allow_model_call(self.client_address[0]):
                result["ai_status"] = "rate_limited"
            else:
                try:
                    model, rows = explain(findings)
                    result["ai_model"] = model
                    result["ai_explanations"] = rows
                    result["ai_status"] = "completed"
                except NebiusError as error:
                    result["ai_status"] = "provider_error"
                    result["ai_error"] = str(error)
        elif payload.get("use_ai") is True:
            result["ai_status"] = "no_supported_findings"
        self._json(200, result)


def main() -> None:
    port = int(os.environ.get("PORT", "8000"))
    host = os.environ.get("HOST", "127.0.0.1")
    server = ThreadingHTTPServer((host, port), Handler)
    print(f"StackLens AI listening on http://{host}:{port}; Nebius configured: {available()}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
