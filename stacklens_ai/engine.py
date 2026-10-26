"""Deterministic, conservative diagnostics for StackLens schema-v1 snapshots.

This module deliberately makes no network calls. AI explanations are optional,
downstream commentary on these findings, never the source of the findings.
"""

from __future__ import annotations

import re
from typing import Any


class SnapshotError(ValueError):
    """A supplied snapshot cannot be safely interpreted."""


def _collected(snapshot: dict[str, Any], name: str) -> dict[str, Any]:
    probe = snapshot.get(name)
    if not isinstance(probe, dict) or probe.get("status") != "collected":
        return {}
    data = probe.get("data")
    return data if isinstance(data, dict) else {}


def _finding(
    rule_id: str,
    severity: str,
    title: str,
    evidence: list[str],
    interpretation: str,
    check: str,
) -> dict[str, Any]:
    return {
        "rule_id": rule_id,
        "severity": severity,
        "title": title,
        "evidence": evidence,
        "interpretation": interpretation,
        "verification": check,
    }


def evaluate(snapshot: dict[str, Any]) -> list[dict[str, Any]]:
    if not isinstance(snapshot, dict):
        raise SnapshotError("Snapshot must be a JSON object.")
    if type(snapshot.get("schema_version")) is not int or snapshot["schema_version"] != 1:
        raise SnapshotError("Only StackLens snapshot schema_version 1 is supported.")

    cuda = _collected(snapshot, "cuda")
    driver = _collected(snapshot, "nvidia_driver")
    findings: list[dict[str, Any]] = []

    # Minimum *major-family* driver branch, not a complete CUDA compatibility test.
    compat = cuda.get("compatibility", [])
    if isinstance(compat, list):
        for item in compat[:20]:
            if not isinstance(item, dict):
                continue
            branch = item.get("observed_driver_branch")
            minimum = item.get("documented_minimum_driver_branch")
            family = item.get("cuda_major")
            if not all(type(x) is int and 0 < x < 10000 for x in (branch, minimum, family)):
                continue
            if branch < minimum:
                findings.append(
                    _finding(
                        "CUDA-DRIVER-0001",
                        "high",
                        "Driver below documented CUDA family minimum",
                        [
                            f"Observed NVIDIA driver branch: {branch}",
                            f"Documented CUDA {family}.x minimum branch: {minimum}",
                        ],
                        "The observed driver branch is below this CUDA major family's documented minimum. "
                        "This does not identify every possible application failure.",
                        "Compare the installed driver with NVIDIA's official CUDA compatibility guidance "
                        "before planning a driver or toolkit change.",
                    )
                )

    # Different environment selectors are suspicious, not proof of a broken loader.
    selectors = cuda.get("selectors", {})
    if isinstance(selectors, dict):
        locations: dict[str, str] = {}
        for label in ("cuda_home", "cuda_path", "cuda_root"):
            value = selectors.get(label)
            if not isinstance(value, dict):
                continue
            path = value.get("resolved_path") or value.get("path")
            if isinstance(path, str) and path.strip():
                locations[label.upper()] = path.rstrip("/")
        if len(set(locations.values())) > 1:
            names = ", ".join(locations)
            findings.append(
                _finding(
                    "CUDA-PATH-0001",
                    "medium",
                    "CUDA environment selectors disagree",
                    [f"Different CUDA installation directories were found in: {names}"],
                    "The environment points to more than one CUDA location. This can cause "
                    "confusion but does not by itself prove a runtime failure.",
                    "Inspect CUDA_HOME, CUDA_PATH, CUDA_ROOT, and the nvcc executable selected by PATH.",
                )
            )

    # Candidate enumeration is not loader-resolution evidence.
    runtime_libraries = cuda.get("runtime_libraries", [])
    if isinstance(runtime_libraries, list):
        paths = {
            item.get("path")
            for item in runtime_libraries[:200]
            if isinstance(item, dict)
            and isinstance(item.get("path"), str)
            and "libcudart" in item["path"].lower()
        }
        if len(paths) > 1:
            findings.append(
                _finding(
                    "CUDA-RUNTIME-0001",
                    "low",
                    "Multiple CUDA runtime candidates discovered",
                    [f"Distinct libcudart candidate paths found: {len(paths)}"],
                    "Multiple discovered library files do not establish which one the loader will use.",
                    "Inspect loader search order and the application's resolved dependencies before changing paths.",
                )
            )

    kernel = driver.get("kernel_module_version")
    userspace = driver.get("userspace_version")
    version_pattern = re.compile(r"[0-9]{2,4}(?:\.[0-9]{1,4}){0,3}\Z")
    if (isinstance(kernel, str) and isinstance(userspace, str)
            and version_pattern.fullmatch(kernel) and version_pattern.fullmatch(userspace)):
        if kernel != userspace:
            findings.append(
                _finding(
                    "NVIDIA-DRIVER-0001",
                    "high",
                    "NVIDIA kernel and userspace versions disagree",
                    [f"Kernel module version: {kernel}", f"Userspace version: {userspace}"],
                    "The NVIDIA driver components report different versions. Recheck these sources "
                    "before assuming a specific repair is safe.",
                    "Compare /proc/driver/nvidia/version with nvidia-smi and review recent driver changes.",
                )
            )

    order = {"high": 0, "medium": 1, "low": 2, "info": 3}
    return sorted(findings, key=lambda f: (order[f["severity"]], f["rule_id"]))


def evidence_for_model(findings: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Send only rule IDs and curated facts, never the original environment."""
    return [
        {
            "rule_id": f["rule_id"],
            "title": f["title"],
            "evidence": f["evidence"],
            "interpretation": f["interpretation"],
        }
        for f in findings
    ]
