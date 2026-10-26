"""Deterministic, conservative diagnostics for StackLens schema-v1 snapshots.

This module makes NO network calls and does NOT depend on AI inference.
Findings are strictly determined by local rules and authoritative compatibility data.
"""

from __future__ import annotations

import re
from typing import Any

# Authoritative minimum NVIDIA driver branch per CUDA major family.
# Source: NVIDIA CUDA Compatibility Documentation (reviewed August 2026).
# https://docs.nvidia.com/deploy/cuda-compatibility/
MAJOR_FAMILY_MIN_DRIVER_BRANCH: dict[int, int] = {
    10: 410,
    11: 450,
    12: 525,
    13: 580,
}

COMPATIBILITY_SOURCE = "NVIDIA CUDA Compatibility documentation (reviewed August 2026)"

# Fixed, rule-specific allowlist of safe, read-only diagnostic commands.
# Model suggestions are strictly validated against these allowlists.
SAFE_CHECKS_ALLOWLIST: dict[str, list[str]] = {
    "CUDA-DRIVER-0001": [
        "nvidia-smi",
        "cat /proc/driver/nvidia/version",
        "modinfo nvidia",
        "nvcc --version",
    ],
    "CUDA-PATH-0001": [
        'echo "CUDA_HOME=$CUDA_HOME CUDA_PATH=$CUDA_PATH CUDA_ROOT=$CUDA_ROOT"',
        "which nvcc || command -v nvcc",
        "nvcc --version",
        "env | grep -E '^(CUDA_|PATH|LD_LIBRARY_PATH)'",
    ],
    "CUDA-RUNTIME-0001": [
        "ldconfig -p | grep libcudart",
        'echo "LD_LIBRARY_PATH=$LD_LIBRARY_PATH"',
        "which -a libcudart.so 2>/dev/null || true",
    ],
    "NVIDIA-DRIVER-0001": [
        "cat /proc/driver/nvidia/version",
        "nvidia-smi",
        "dmesg | grep -i nvidia | tail -n 20",
        "dpkg -l | grep -i nvidia || rpm -qa | grep -i nvidia",
    ],
}

DEFAULT_SAFE_CHECKS: dict[str, str] = {
    "CUDA-DRIVER-0001": "nvidia-smi",
    "CUDA-PATH-0001": 'echo "CUDA_HOME=$CUDA_HOME CUDA_PATH=$CUDA_PATH CUDA_ROOT=$CUDA_ROOT"',
    "CUDA-RUNTIME-0001": "ldconfig -p | grep libcudart",
    "NVIDIA-DRIVER-0001": "cat /proc/driver/nvidia/version",
}


class SnapshotError(ValueError):
    """A supplied snapshot cannot be safely interpreted."""


def _collected(snapshot: dict[str, Any], name: str) -> dict[str, Any]:
    probe = snapshot.get(name)
    if not isinstance(probe, dict) or probe.get("status") != "collected":
        return {}
    data = probe.get("data")
    return data if isinstance(data, dict) else {}


def _parse_driver_branch(version: Any) -> int | None:
    """Extract major driver branch integer from version string (e.g. '535.183.01' -> 535)."""
    if not isinstance(version, str):
        return None
    part = version.strip().split(".")[0]
    if part.isdigit():
        try:
            val = int(part)
            return val if 0 < val < 10000 else None
        except ValueError:
            return None
    return None


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


def validate_suggested_check(rule_id: str, command: str | None) -> str:
    """Validate a command against the fixed allowlist for the rule.

    Rejects any un-allowlisted or arbitrary command and returns the rule's default safe check.
    """
    allowlist = SAFE_CHECKS_ALLOWLIST.get(rule_id, [])
    if command and isinstance(command, str):
        cleaned = command.strip()
        if cleaned in allowlist:
            return cleaned
    return DEFAULT_SAFE_CHECKS.get(rule_id, "nvidia-smi")


def evaluate(snapshot: dict[str, Any]) -> list[dict[str, Any]]:
    """Evaluate deterministic diagnostic rules against a schema-v1 snapshot.

    Never relies on AI inference. Uses authoritative local compatibility tables.
    """
    if not isinstance(snapshot, dict):
        raise SnapshotError("Snapshot must be a JSON object.")
    if type(snapshot.get("schema_version")) is not int or snapshot["schema_version"] != 1:
        raise SnapshotError("Only StackLens snapshot schema_version 1 is supported.")

    cuda = _collected(snapshot, "cuda")
    driver = _collected(snapshot, "nvidia_driver")
    findings: list[dict[str, Any]] = []

    # --------------------------------------------------------------------------
    # 1. Rule CUDA-DRIVER-0001: Driver below documented CUDA family minimum
    # Authoritative compatibility table is strictly used instead of blindly trusting
    # thresholds or notes supplied inside the snapshot.
    # --------------------------------------------------------------------------
    observed_branch: int | None = None
    kernel_ver = driver.get("kernel_module_version")
    userspace_ver = driver.get("userspace_version")
    driver_api_ver = cuda.get("driver_api_version")

    for v in (kernel_ver, userspace_ver, driver_api_ver):
        b = _parse_driver_branch(v)
        if b is not None:
            observed_branch = b
            break

    # Check compatibility entries if present
    compat = cuda.get("compatibility")
    has_compat_probes = False

    if isinstance(compat, list) and len(compat) > 0:
        has_compat_probes = True
        for item in compat[:20]:
            if not isinstance(item, dict):
                continue
            family = item.get("cuda_major")
            if type(family) is not int or family not in MAJOR_FAMILY_MIN_DRIVER_BRANCH:
                continue

            # Use observed branch from item if valid, else fallback to driver probe
            item_branch = item.get("observed_driver_branch")
            branch = item_branch if (type(item_branch) is int and 0 < item_branch < 10000) else observed_branch
            if branch is None:
                continue

            authoritative_minimum = MAJOR_FAMILY_MIN_DRIVER_BRANCH[family]
            if branch < authoritative_minimum:
                findings.append(
                    _finding(
                        "CUDA-DRIVER-0001",
                        "high",
                        "Driver below documented CUDA family minimum",
                        [
                            f"Observed NVIDIA driver branch: {branch}",
                            f"Authoritative documented CUDA {family}.x minimum branch: {authoritative_minimum}",
                            f"Reference: {COMPATIBILITY_SOURCE}",
                        ],
                        f"The observed driver branch ({branch}) is below the documented CUDA {family}.x "
                        f"major-family minimum of {authoritative_minimum}. This limitation does not identify "
                        "every possible application failure.",
                        "Compare the installed driver with NVIDIA official CUDA compatibility guidance "
                        "before planning a driver or toolkit change.",
                    )
                )

    # If compatibility probe was missing or not a list, fallback to discovered toolkits or nvcc
    if not has_compat_probes:
        discovered_majors: set[int] = set()
        toolkits = cuda.get("toolkits")
        if isinstance(toolkits, list):
            for tk in toolkits[:10]:
                if isinstance(tk, dict):
                    v = tk.get("version")
                    if isinstance(v, dict) and isinstance(v.get("major"), int):
                        discovered_majors.add(v["major"])
        nvcc = cuda.get("nvcc")
        if isinstance(nvcc, dict):
            v = nvcc.get("version")
            if isinstance(v, dict) and isinstance(v.get("major"), int):
                discovered_majors.add(v["major"])

        for fam in sorted(discovered_majors):
            if fam in MAJOR_FAMILY_MIN_DRIVER_BRANCH and observed_branch is not None:
                auth_min = MAJOR_FAMILY_MIN_DRIVER_BRANCH[fam]
                if observed_branch < auth_min:
                    findings.append(
                        _finding(
                            "CUDA-DRIVER-0001",
                            "high",
                            "Driver below documented CUDA family minimum",
                            [
                                f"Observed NVIDIA driver branch: {observed_branch}",
                                f"Authoritative documented CUDA {fam}.x minimum branch: {auth_min}",
                                f"Reference: {COMPATIBILITY_SOURCE}",
                            ],
                            f"The observed driver branch ({observed_branch}) is below the documented CUDA {fam}.x "
                            f"major-family minimum of {auth_min}.",
                            "Compare the installed driver with NVIDIA official CUDA compatibility guidance.",
                        )
                    )

    # --------------------------------------------------------------------------
    # 2. Rule CUDA-PATH-0001: Conflicting CUDA environment selectors
    # --------------------------------------------------------------------------
    selectors = cuda.get("selectors")
    if isinstance(selectors, dict):
        locations: dict[str, str] = {}
        for label in ("cuda_home", "cuda_path", "cuda_root"):
            value = selectors.get(label)
            if not isinstance(value, dict):
                continue
            path = value.get("resolved_path") or value.get("path")
            if isinstance(path, str) and path.strip():
                # Normalize trailing slashes
                locations[label.upper()] = path.strip().rstrip("/\\")
        if len(set(locations.values())) > 1:
            names = ", ".join(sorted(locations))
            findings.append(
                _finding(
                    "CUDA-PATH-0001",
                    "medium",
                    "CUDA environment selectors disagree",
                    [f"Different CUDA installation directories were found in: {names}"],
                    "The environment points to more than one CUDA location. This can cause "
                    "confusion but does not by itself prove a runtime failure.",
                    "Inspect CUDA_HOME, CUDA_PATH, CUDA_ROOT, and verify which nvcc binary is active in PATH.",
                )
            )

    # --------------------------------------------------------------------------
    # 3. Rule CUDA-RUNTIME-0001: Multiple CUDA runtime candidates
    # --------------------------------------------------------------------------
    runtime_libraries = cuda.get("runtime_libraries")
    if isinstance(runtime_libraries, list):
        paths = {
            item.get("path")
            for item in runtime_libraries[:200]
            if isinstance(item, dict)
            and isinstance(item.get("path"), str)
            and ("libcudart" in item["path"].lower() or "cudart64" in item["path"].lower())
        }
        if len(paths) > 1:
            findings.append(
                _finding(
                    "CUDA-RUNTIME-0001",
                    "low",
                    "Multiple CUDA runtime candidates discovered",
                    [f"Distinct libcudart candidate paths found: {len(paths)}"],
                    "Multiple discovered library files do not establish which one the dynamic linker will select at runtime.",
                    "Inspect dynamic loader search order and application binary dependencies before altering paths.",
                )
            )

    # --------------------------------------------------------------------------
    # 4. Rule NVIDIA-DRIVER-0001: Kernel and userspace driver mismatch
    # --------------------------------------------------------------------------
    version_pattern = re.compile(r"^[0-9]{2,4}(?:\.[0-9]{1,4}){0,3}$")
    if (
        isinstance(kernel_ver, str)
        and isinstance(userspace_ver, str)
        and version_pattern.fullmatch(kernel_ver.strip())
        and version_pattern.fullmatch(userspace_ver.strip())
    ):
        k_clean = kernel_ver.strip()
        u_clean = userspace_ver.strip()
        if k_clean != u_clean:
            findings.append(
                _finding(
                    "NVIDIA-DRIVER-0001",
                    "high",
                    "NVIDIA kernel and userspace versions disagree",
                    [f"Kernel module version: {k_clean}", f"Userspace version: {u_clean}"],
                    "The NVIDIA driver components report different versions. Recheck these sources "
                    "before assuming a specific repair is safe.",
                    "Compare /proc/driver/nvidia/version with nvidia-smi and review recent package or driver upgrades.",
                )
            )

    order = {"high": 0, "medium": 1, "low": 2, "info": 3}
    return sorted(findings, key=lambda f: (order.get(f["severity"], 99), f["rule_id"]))


def evidence_for_model(findings: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Send only rule IDs, titles, sanitized facts, and interpretation to Nemotron.

    Raw filesystem paths, usernames, hostnames, and arbitrary snapshot trees are strictly excluded.
    """
    outbound: list[dict[str, Any]] = []
    for f in findings:
        sanitized_evidence: list[str] = []
        for line in f.get("evidence", []):
            if isinstance(line, str):
                cleaned = re.sub(r"/(?:home|Users)/[^/\\]+/", "/<user>/", line)
                sanitized_evidence.append(cleaned[:300])

        outbound.append(
            {
                "rule_id": f["rule_id"],
                "title": f["title"],
                "evidence": sanitized_evidence,
                "interpretation": f["interpretation"],
            }
        )
    return outbound
