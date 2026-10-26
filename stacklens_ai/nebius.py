"""Optional NVIDIA Nemotron explanations via Nebius Token Factory."""

from __future__ import annotations

import json
import os
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from .engine import evidence_for_model

DEFAULT_MODEL = "nvidia/nemotron-3-super-120b-a12b"
DEFAULT_BASE = "https://api.tokenfactory.us-central1.nebius.com/v1"

SYSTEM_PROMPT = """You explain deterministic AI-compute environment findings to developers.
You do not diagnose from unseen host data. Evidence and findings are authoritative.
Do not claim a repair was tested or executed. Do not invent version numbers, files,
causes, tool output, or documentation. Explain uncertainty where evidence is partial.
Reply with ONLY a JSON object of this form:
{"explanations":[{"rule_id":"EXACT_EXISTING_RULE_ID",
"explanation":"One or two concise sentences grounded in its supplied evidence."}]}
Do not introduce IDs that were not given. There may be no confirmed root cause."""


class NebiusError(RuntimeError):
    pass


def available() -> bool:
    return bool(os.environ.get("NEBIUS_API_KEY", "").strip())


def explain(findings: list[dict[str, Any]]) -> tuple[str, list[dict[str, str]]]:
    """Return (model identifier, explanations) with exact-ID membership checks.

    Grounding validation here verifies IDs, not semantic truth of arbitrary prose.
    The UI always retains the deterministic evidence separately.
    """
    api_key = os.environ.get("NEBIUS_API_KEY", "").strip()
    if not api_key:
        raise NebiusError("NEBIUS_API_KEY has not been configured.")
    if not findings:
        return os.environ.get("NEBIUS_MODEL", DEFAULT_MODEL), []

    model = os.environ.get("NEBIUS_MODEL", DEFAULT_MODEL)
    base = os.environ.get("NEBIUS_BASE_URL", DEFAULT_BASE).rstrip("/")
    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {
                "role": "user",
                "content": json.dumps(
                    {"deterministic_findings": evidence_for_model(findings)},
                    separators=(",", ":"),
                ),
            },
        ],
        "temperature": 0.1,
        "max_tokens": 700,
    }
    request = Request(
        base + "/chat/completions",
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urlopen(request, timeout=25) as response:
            result = json.load(response)
    except HTTPError as error:
        # Do not expose request headers, API keys, or untrusted provider error bodies.
        raise NebiusError(f"Token Factory returned HTTP {error.code}.") from error
    except (URLError, TimeoutError, OSError) as error:
        raise NebiusError("Token Factory could not be reached within the timeout.") from error
    except (ValueError, TypeError) as error:
        raise NebiusError("Token Factory returned an unreadable response.") from error

    try:
        content = result["choices"][0]["message"]["content"]
        if not isinstance(content, str):
            raise ValueError("Unexpected model content")
        clean = content.strip()
        if clean.startswith("```"):
            clean = clean.split("\n", 1)[1].rsplit("```", 1)[0].strip()
        decoded = json.loads(clean)
        rows = decoded["explanations"]
        if not isinstance(rows, list):
            raise ValueError("Not a list")
    except (KeyError, IndexError, ValueError, TypeError) as error:
        raise NebiusError("Nemotron did not return the expected structured explanations.") from error

    allowed = {item["rule_id"] for item in findings}
    seen: set[str] = set()
    explanations: list[dict[str, str]] = []
    for row in rows:
        if not isinstance(row, dict):
            continue
        rule_id = row.get("rule_id")
        text = row.get("explanation")
        if (
            isinstance(rule_id, str)
            and rule_id in allowed
            and rule_id not in seen
            and isinstance(text, str)
            and text.strip()
        ):
            explanations.append({"rule_id": rule_id, "explanation": text.strip()[:650]})
            seen.add(rule_id)
    if not explanations:
        raise NebiusError("Nemotron did not explain any recognized finding IDs.")
    return model, explanations
