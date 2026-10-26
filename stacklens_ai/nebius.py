"""NVIDIA Nemotron integration via Nebius Token Factory.

Uses the OpenAI-compatible chat completion endpoint on Nebius Token Factory.
Enforces rule ID grounding and restricts suggested follow-up checks to a fixed allowlist.
"""

from __future__ import annotations

import json
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from .config import get_config
from .engine import evidence_for_model, validate_suggested_check

SYSTEM_PROMPT = """You explain deterministic AI-compute environment findings to developers.
You do not diagnose from unseen host data. Evidence and findings are authoritative.
Do not claim a repair was tested or executed. Do not invent version numbers, files,
causes, tool output, or documentation. Explain uncertainty where evidence is partial.

For each finding, provide:
1. "rule_id": the exact matching finding ID provided.
2. "explanation": 1-2 concise sentences grounded strictly in the supplied evidence.
3. "suggested_check": select ONE read-only verification command strictly from that rule's "allowed_follow_up_checks". Do NOT invent new commands.

Reply with ONLY a JSON object of this exact form:
{"explanations":[{"rule_id":"EXACT_RULE_ID","explanation":"...","suggested_check":"..."}]}"""


class NebiusError(RuntimeError):
    """Raised when Nebius Token Factory inference cannot complete."""


def available() -> bool:
    """Return whether a Nebius API key is configured on the server."""
    return get_config().is_ai_configured


def explain(findings: list[dict[str, Any]]) -> tuple[str, list[dict[str, str]]]:
    """Return (model identifier, explanations) with exact-ID and allowlisted check validation.

    Grounding validation here verifies IDs and enforces the command allowlist;
    the deterministic evidence and verification steps are retained separately in the UI.
    """
    cfg = get_config()
    api_key = cfg.nebius_api_key
    if not api_key:
        raise NebiusError("NEBIUS_API_KEY has not been configured.")
    if not findings:
        return cfg.nebius_model, []

    model = cfg.nebius_model
    base = cfg.nebius_base_url.rstrip("/")
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
        "max_tokens": 800,
    }

    request = Request(
        base + "/chat/completions",
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
            "User-Agent": "StackLensAI/0.1",
        },
        method="POST",
    )

    try:
        with urlopen(request, timeout=30) as response:
            result = json.load(response)
    except HTTPError as error:
        # Never expose request headers, API keys, or raw provider bodies
        raise NebiusError(f"Nebius Token Factory returned HTTP {error.code}.") from error
    except (URLError, TimeoutError, OSError) as error:
        raise NebiusError("Nebius Token Factory could not be reached within the timeout.") from error
    except (ValueError, TypeError) as error:
        raise NebiusError("Nebius Token Factory returned an unreadable response.") from error

    try:
        content = result["choices"][0]["message"]["content"]
        if not isinstance(content, str):
            raise ValueError("Unexpected model content type")
        clean = content.strip()
        if clean.startswith("```"):
            clean = clean.split("\n", 1)[1].rsplit("```", 1)[0].strip()
        decoded = json.loads(clean)
        rows = decoded.get("explanations")
        if not isinstance(rows, list):
            raise ValueError("Expected 'explanations' list in response")
    except (KeyError, IndexError, ValueError, TypeError) as error:
        raise NebiusError("Nemotron did not return the expected structured explanations.") from error

    allowed_ids = {item["rule_id"] for item in findings}
    seen: set[str] = set()
    validated_explanations: list[dict[str, str]] = []

    for row in rows:
        if not isinstance(row, dict):
            continue
        rule_id = row.get("rule_id")
        text = row.get("explanation")
        suggested = row.get("suggested_check")

        if (
            isinstance(rule_id, str)
            and rule_id in allowed_ids
            and rule_id not in seen
            and isinstance(text, str)
            and text.strip()
        ):
            safe_check = validate_suggested_check(rule_id, suggested)
            validated_explanations.append(
                {
                    "rule_id": rule_id,
                    "explanation": text.strip()[:650],
                    "suggested_check": safe_check,
                }
            )
            seen.add(rule_id)

    if not validated_explanations:
        raise NebiusError("Nemotron did not explain any recognized finding IDs.")

    return model, validated_explanations


def smoke_test(api_key: str | None = None) -> tuple[bool, str]:
    """Execute a live smoke test against Nebius Token Factory.

    Returns (True, message) if inference succeeds, or (False, error) if it fails.
    """
    cfg = get_config()
    key = (api_key or cfg.nebius_api_key).strip()
    if not key:
        return False, "NEBIUS_API_KEY is not set."

    base = cfg.nebius_base_url.rstrip("/")
    model = cfg.nebius_model

    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a test assistant. Reply with: OK"},
            {"role": "user", "content": "ping"},
        ],
        "max_tokens": 10,
        "temperature": 0.0,
    }

    req = Request(
        base + "/chat/completions",
        data=json.dumps(payload).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
            "User-Agent": "StackLensAI/0.1-SmokeTest",
        },
        method="POST",
    )

    try:
        with urlopen(req, timeout=20) as resp:
            data = json.load(resp)
            reply = data["choices"][0]["message"]["content"].strip()
            return True, f"Smoke test succeeded. Model '{model}' responded: {reply[:100]}"
    except HTTPError as e:
        return False, f"HTTP Error {e.code}: Token Factory refused the request."
    except Exception as e:
        return False, f"Connection failed: {e}"
