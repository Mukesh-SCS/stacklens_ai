"""Preflight verification tool for StackLens AI.

Runs offline-first sanity checks on configuration, deterministic diagnostic rules,
fixture snapshots, static assets, and local compatibility tables.

Usage:
    python -m stacklens_ai.preflight
    python -m stacklens_ai.preflight --smoke-test
"""

from __future__ import annotations

import json
import socket
import sys
from pathlib import Path
from typing import Any

from .config import get_config
from .engine import (
    DEFAULT_SAFE_CHECKS,
    MAJOR_FAMILY_MIN_DRIVER_BRANCH,
    SAFE_CHECKS_ALLOWLIST,
    evaluate,
    evidence_for_model,
)
from .nebius import available, smoke_test

PACKAGE_DIR = Path(__file__).resolve().parent
FIXTURES_DIR = PACKAGE_DIR / "fixtures"
STATIC_DIR = PACKAGE_DIR / "static"

EXPECTED_FIXTURE_FINDINGS: dict[str, list[str]] = {
    "driver-too-old.json": ["CUDA-DRIVER-0001"],
    "selector-conflict.json": ["CUDA-PATH-0001", "CUDA-RUNTIME-0001"],
    "driver-component-mismatch.json": ["NVIDIA-DRIVER-0001"],
}


def _check(title: str, ok: bool, details: str = "") -> bool:
    status = "[PASS]" if ok else "[FAIL]"
    message = f"  {status} {title}"
    if details:
        message += f" - {details}"
    print(message)
    return ok


def run_preflight(run_live_smoke: bool = False) -> int:
    print("=" * 72)
    print(" StackLens AI - System Preflight Verification")
    print("=" * 72)

    failures = 0
    cfg = get_config()

    # 1. Environment & Configuration
    print("\n1. Configuration Check:")
    _check("Config loaded", True, f"Host: {cfg.host}:{cfg.port}, Budget: {cfg.model_budget}")
    _check(
        "NVIDIA Model Target",
        cfg.nebius_model == "nvidia/nemotron-3-super-120b-a12b",
        cfg.nebius_model,
    )
    if cfg.is_ai_configured:
        masked = cfg.nebius_api_key[:4] + "..." + cfg.nebius_api_key[-4:]
        _check("Nebius API Key", True, f"Configured ({masked})")
    else:
        print("  [INFO] Nebius API Key - Empty / Pending account approval (deterministic mode active)")

    # 2. Local Authoritative Compatibility Tables
    print("\n2. Compatibility Table & Allowlist Integrity:")
    has_compat = all(k in MAJOR_FAMILY_MIN_DRIVER_BRANCH for k in (10, 11, 12, 13))
    if not _check("CUDA Major Family Minimums", has_compat, f"Keys: {sorted(MAJOR_FAMILY_MIN_DRIVER_BRANCH)}"):
        failures += 1

    all_rules = ["CUDA-DRIVER-0001", "CUDA-PATH-0001", "CUDA-RUNTIME-0001", "NVIDIA-DRIVER-0001"]
    has_allowlists = all(r in SAFE_CHECKS_ALLOWLIST and r in DEFAULT_SAFE_CHECKS for r in all_rules)
    if not _check("Safe Command Allowlists", has_allowlists, f"{len(all_rules)} rules mapped"):
        failures += 1

    # 3. Synthetic Demonstration Fixtures
    print("\n3. Synthetic Demonstration Scenarios:")
    for filename, expected_rules in EXPECTED_FIXTURE_FINDINGS.items():
        fixture_path = FIXTURES_DIR / filename
        if not fixture_path.is_file():
            _check(f"Fixture {filename}", False, "File missing")
            failures += 1
            continue

        try:
            content = json.loads(fixture_path.read_text(encoding="utf-8"))
            if content.get("schema_version") != 1:
                _check(f"Fixture {filename}", False, "Invalid schema_version")
                failures += 1
                continue

            findings = evaluate(content)
            found_ids = [f["rule_id"] for f in findings]
            all_found = all(r in found_ids for r in expected_rules)

            if not _check(f"Scenario: {filename}", all_found, f"Expected {expected_rules}, Got {found_ids}"):
                failures += 1
            else:
                # Validate evidence redaction
                model_ev = evidence_for_model(findings)
                ev_str = json.dumps(model_ev)
                if "/home/" in ev_str or "C:\\Users" in ev_str:
                    _check(f"Redaction check {filename}", False, "Detected unredacted path")
                    failures += 1
                else:
                    _check(f"Redaction check {filename}", True, "No raw usernames or user paths in model payload")

        except Exception as ex:
            _check(f"Fixture {filename}", False, f"Exception: {ex}")
            failures += 1

    # 4. Frontend Assets
    print("\n4. Static Web Assets:")
    for asset in ("index.html", "app.js", "styles.css"):
        path = STATIC_DIR / asset
        exists = path.is_file() and path.stat().st_size > 0
        if not _check(f"Static asset: {asset}", exists, f"{path.stat().st_size if exists else 0} bytes"):
            failures += 1

    # 5. Network Port Binding Check
    print("\n5. Network Readiness:")
    port_free = False
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            s.bind((cfg.host, cfg.port))
            port_free = True
    except OSError as err:
        port_free = False
        _check(f"Port {cfg.host}:{cfg.port}", False, f"Already in use or restricted ({err})")
    if port_free:
        _check(f"Port {cfg.host}:{cfg.port}", True, "Available for binding")

    # 6. Live Inference Smoke Test (Optional / Opt-in)
    print("\n6. Model Inference Verification:")
    if run_live_smoke:
        if not cfg.is_ai_configured:
            print("  [SKIP] Live smoke test skipped: NEBIUS_API_KEY is not set.")
        else:
            print("  Executing live smoke test against Nebius Token Factory...")
            ok, msg = smoke_test()
            if not _check("Nebius Token Factory Smoke Test", ok, msg):
                failures += 1
    else:
        if cfg.is_ai_configured:
            print("  [INFO] NEBIUS_API_KEY detected. Run with '--smoke-test' to execute a live API ping.")
        else:
            print("  [INFO] Offline mode verified. Real inference requires NEBIUS_API_KEY in .env.")
            print("  [INFO] Smoke-test command when key arrives: python -m stacklens_ai.preflight --smoke-test")

    print("\n" + "=" * 72)
    if failures == 0:
        print(" PREFLIGHT RESULT: ALL OFFLINE CHECKS PASSED (100% READY)")
        print("=" * 72)
        return 0
    else:
        print(f" PREFLIGHT RESULT: {failures} CHECK(S) FAILED")
        print("=" * 72)
        return 1


def main() -> None:
    smoke = "--smoke-test" in sys.argv or "--live" in sys.argv
    sys.exit(run_preflight(run_live_smoke=smoke))


if __name__ == "__main__":
    main()
