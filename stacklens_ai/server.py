"""Dependency-free HTTP server for StackLens AI.

Run from repository root:
    python -m stacklens_ai.server

When NEBIUS_API_KEY is configured in .env or the process environment,
NVIDIA Nemotron explanations via Nebius Token Factory are enabled.
Without a key, the server operates in 100% offline deterministic mode.
"""

from __future__ import annotations

import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

from .config import GLOBAL_BUDGET, get_config
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


class Handler(BaseHTTPRequestHandler):
    server_version = "StackLensAI/0.1"

    def log_message(self, fmt: str, *args: Any) -> None:
        # Security: Never log raw payloads or credential values
        print(f"[stacklens-ai] {self.address_string()} {fmt % args}")

    def _send(self, status: int, data: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header(
            "Content-Security-Policy",
            "default-src 'self'; connect-src 'self'; img-src 'self' data:; "
            "script-src 'self'; style-src 'self' 'unsafe-inline'; object-src 'none'; base-uri 'none'",
        )
        self.end_headers()
        self.wfile.write(data)

    def _json(self, status: int, body: dict[str, Any]) -> None:
        self._send(status, json.dumps(body).encode("utf-8"), "application/json; charset=utf-8")

    def do_GET(self) -> None:
        path = urlsplit(self.path).path
        cfg = get_config()

        if path == "/api/health":
            self._json(
                200,
                {
                    "ok": True,
                    "service": "StackLens AI",
                    "version": "0.1.0",
                    "ai_configured": available(),
                    "nebius_model": cfg.nebius_model,
                    "budget": GLOBAL_BUDGET.status(cfg.model_budget),
                },
            )
            return

        if path == "/api/examples":
            self._json(
                200,
                {
                    "examples": [
                        {
                            "id": k,
                            "filename": v,
                            "title": v.replace(".json", "").replace("-", " ").title(),
                        }
                        for k, v in EXAMPLES.items()
                    ]
                },
            )
            return

        if path.startswith("/api/examples/"):
            key = path[len("/api/examples/") :]
            if key not in EXAMPLES:
                self._json(404, {"error": "Unknown example scenario."})
                return
            target_file = FIXTURES / EXAMPLES[key]
            if not target_file.is_file():
                self._json(404, {"error": "Scenario fixture missing on server."})
                return
            self._send(200, target_file.read_bytes(), "application/json; charset=utf-8")
            return

        files = {
            "/": ("index.html", "text/html; charset=utf-8"),
            "/index.html": ("index.html", "text/html; charset=utf-8"),
            "/app.js": ("app.js", "text/javascript; charset=utf-8"),
            "/styles.css": ("styles.css", "text/css; charset=utf-8"),
        }
        target = files.get(path)
        if not target:
            self._json(404, {"error": "Resource not found."})
            return
        name, media = target
        file_path = STATIC / name
        if not file_path.is_file():
            self._json(404, {"error": f"Static asset {name} not found."})
            return
        self._send(200, file_path.read_bytes(), media)

    def do_POST(self) -> None:
        if urlsplit(self.path).path != "/api/analyze":
            self._json(404, {"error": "Endpoint not found."})
            return

        cfg = get_config()
        try:
            length = int(self.headers.get("Content-Length", "-1"))
        except ValueError:
            length = -1

        if length <= 0 or length > cfg.max_request_bytes:
            self._json(413, {"error": f"Request body must be between 1 and {cfg.max_request_bytes // 1024} KiB."})
            return

        content_type = self.headers.get("Content-Type", "").split(";")[0].strip().lower()
        if content_type != "application/json":
            self._json(415, {"error": "Content-Type must be application/json."})
            return

        try:
            raw_body = self.rfile.read(length)
            payload = json.loads(raw_body)
            if not isinstance(payload, dict):
                raise SnapshotError("Request payload must be a JSON object.")
            snapshot = payload.get("snapshot")
            findings = evaluate(snapshot)
        except (ValueError, UnicodeDecodeError, SnapshotError) as error:
            self._json(400, {"error": str(error)[:250]})
            return

        result: dict[str, Any] = {
            "findings": findings,
            "finding_count": len(findings),
            "ai_status": "not_requested",
            "ai_explanations": [],
            "note": (
                "Deterministic evaluation complete. No finding firing does NOT prove the environment is healthy. "
                "Phase 1C does not evaluate planned PyTorch and ELF loader collectors."
            ),
        }

        if payload.get("use_ai") is True and findings:
            if not available():
                result["ai_status"] = "not_configured"
            else:
                client_ip = self.client_address[0]
                allowed, reason = GLOBAL_BUDGET.allow_call(client_ip, cfg.model_budget)
                if not allowed:
                    result["ai_status"] = "rate_limited"
                    result["ai_error"] = reason
                else:
                    try:
                        model_name, explanations = explain(findings)
                        result["ai_model"] = model_name
                        result["ai_explanations"] = explanations
                        result["ai_status"] = "completed"
                    except NebiusError as error:
                        result["ai_status"] = "provider_error"
                        result["ai_error"] = str(error)
        elif payload.get("use_ai") is True:
            result["ai_status"] = "no_supported_findings"

        self._json(200, result)


def main() -> None:
    cfg = get_config()
    server = ThreadingHTTPServer((cfg.host, cfg.port), Handler)
    status_str = f"NVIDIA Nemotron ({cfg.nebius_model})" if cfg.is_ai_configured else "OFFLINE / DETERMINISTIC ONLY"
    print("=" * 72)
    print(f"StackLens AI server listening on http://{cfg.host}:{cfg.port}")
    print(f"Mode: {status_str}")
    print(f"Inference Budget: {cfg.model_budget} calls max")
    print("=" * 72)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopping StackLens AI server.")
        server.server_close()


if __name__ == "__main__":
    main()
