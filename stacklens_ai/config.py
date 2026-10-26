"""Configuration and environment loader for StackLens AI.

Zero external dependencies: loads variables from .env if present without
overriding existing process environment variables.
"""

from __future__ import annotations

import os
from collections import defaultdict, deque
from dataclasses import dataclass
from pathlib import Path
from threading import Lock
from time import monotonic
from typing import Any

DEFAULT_MODEL = "nvidia/nemotron-3-super-120b-a12b"
DEFAULT_BASE_URL = "https://api.tokenfactory.us-central1.nebius.com/v1"
DEFAULT_BUDGET = 100
DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 8000
DEFAULT_MAX_REQUEST_BYTES = 256 * 1024  # 256 KiB


def parse_env_line(line: str) -> tuple[str, str] | None:
    """Parse a single line from a .env file."""
    stripped = line.strip()
    if not stripped or stripped.startswith("#"):
        return None
    if "=" not in stripped:
        return None
    key, val = stripped.split("=", 1)
    key = key.strip()
    val = val.strip()
    if not key:
        return None
    # Strip enclosing quotes if present
    if len(val) >= 2 and ((val[0] == '"' and val[-1] == '"') or (val[0] == "'" and val[-1] == "'")):
        val = val[1:-1]
    return key, val


def load_env_file(env_path: Path | str | None = None) -> dict[str, str]:
    """Load variables from .env into os.environ if not already present.

    Process environment variables always take precedence over .env file entries.
    """
    if env_path is None:
        # Search current working directory, then repository root / parent
        candidates = [
            Path.cwd() / ".env",
            Path(__file__).resolve().parents[1] / ".env",
            Path(__file__).resolve().parent / ".env",
        ]
        target = None
        for candidate in candidates:
            if candidate.is_file():
                target = candidate
                break
    else:
        target = Path(env_path)
        if not target.is_file():
            target = None

    loaded: dict[str, str] = {}
    if target and target.is_file():
        try:
            content = target.read_text(encoding="utf-8")
            for line in content.splitlines():
                pair = parse_env_line(line)
                if pair:
                    k, v = pair
                    loaded[k] = v
                    if k not in os.environ:
                        os.environ[k] = v
        except OSError:
            pass
    return loaded


# Run automatic load upon module import
load_env_file()


@dataclass(frozen=True)
class Config:
    """Immutable snapshot of current configuration."""

    nebius_api_key: str
    nebius_model: str
    nebius_base_url: str
    model_budget: int
    host: str
    port: int
    max_request_bytes: int

    @property
    def is_ai_configured(self) -> bool:
        return bool(self.nebius_api_key)

    def to_safe_dict(self) -> dict[str, Any]:
        """Return safe configuration metadata excluding secrets."""
        return {
            "ai_configured": self.is_ai_configured,
            "nebius_model": self.nebius_model,
            "nebius_base_url": self.nebius_base_url,
            "model_budget": self.model_budget,
            "host": self.host,
            "port": self.port,
        }


def get_config() -> Config:
    """Construct Config from current environment variables."""
    api_key = os.environ.get("NEBIUS_API_KEY", "").strip()
    model = os.environ.get("NEBIUS_MODEL", DEFAULT_MODEL).strip() or DEFAULT_MODEL
    base_url = os.environ.get("NEBIUS_BASE_URL", DEFAULT_BASE_URL).strip().rstrip("/") or DEFAULT_BASE_URL

    try:
        budget = int(os.environ.get("STACKLENS_MODEL_CALL_BUDGET", str(DEFAULT_BUDGET)))
        if budget < 0:
            budget = DEFAULT_BUDGET
    except ValueError:
        budget = DEFAULT_BUDGET

    host = os.environ.get("HOST", DEFAULT_HOST).strip() or DEFAULT_HOST

    try:
        port = int(os.environ.get("PORT", str(DEFAULT_PORT)))
        if port <= 0 or port > 65535:
            port = DEFAULT_PORT
    except ValueError:
        port = DEFAULT_PORT

    return Config(
        nebius_api_key=api_key,
        nebius_model=model,
        nebius_base_url=base_url,
        model_budget=budget,
        host=host,
        port=port,
        max_request_bytes=DEFAULT_MAX_REQUEST_BYTES,
    )


# Thread-safe rate limiter and total budget tracker
class BudgetTracker:
    def __init__(self) -> None:
        self._lock = Lock()
        self._calls: int = 0
        self._ip_usage: dict[str, deque[float]] = defaultdict(deque)

    def allow_call(self, ip: str, budget: int | None = None) -> tuple[bool, str]:
        """Check if an inference request is permitted by rate-limit and budget."""
        if budget is None:
            budget = get_config().model_budget

        with self._lock:
            now = monotonic()
            history = self._ip_usage[ip]
            while history and now - history[0] > 60:
                history.popleft()

            if self._calls >= budget:
                return False, f"Inference budget of {budget} calls reached for this server process."
            if len(history) >= 4:
                return False, "Rate limit exceeded (maximum 4 model requests per minute per IP)."

            history.append(now)
            self._calls += 1
            return True, "Allowed"

    def status(self, budget: int | None = None) -> dict[str, int]:
        if budget is None:
            budget = get_config().model_budget
        with self._lock:
            remaining = max(0, budget - self._calls)
            return {
                "configured_budget": budget,
                "consumed_calls": self._calls,
                "remaining_calls": remaining,
            }

    def reset_for_testing(self) -> None:
        with self._lock:
            self._calls = 0
            self._ip_usage.clear()


GLOBAL_BUDGET = BudgetTracker()
