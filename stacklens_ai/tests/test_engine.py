import json
import unittest
from pathlib import Path

from stacklens_ai.engine import SnapshotError, evaluate, evidence_for_model


FIXTURES = Path(__file__).resolve().parents[1] / "fixtures"


def load(name):
    return json.loads((FIXTURES / name).read_text(encoding="utf-8"))


class EngineTests(unittest.TestCase):
    def test_driver_below_minimum(self):
        result = evaluate(load("driver-too-old.json"))
        self.assertEqual([f["rule_id"] for f in result], ["CUDA-DRIVER-0001"])
        self.assertEqual(result[0]["severity"], "high")

    def test_conflicting_selectors_are_not_proof_of_breakage(self):
        result = evaluate(load("selector-conflict.json"))
        self.assertEqual({f["rule_id"] for f in result}, {"CUDA-PATH-0001", "CUDA-RUNTIME-0001"})
        self.assertIn("do not establish", result[1]["interpretation"])

    def test_driver_component_mismatch(self):
        result = evaluate(load("driver-component-mismatch.json"))
        self.assertEqual(result[0]["rule_id"], "NVIDIA-DRIVER-0001")

    def test_no_finding_is_not_health_assessment(self):
        self.assertEqual(evaluate({"schema_version": 1}), [])

    def test_requires_schema_one(self):
        for value in (0, 2, True, "1", None):
            with self.subTest(value=value):
                with self.assertRaises(SnapshotError):
                    evaluate({"schema_version": value})

    def test_model_evidence_excludes_raw_snapshot_paths(self):
        snapshot = load("selector-conflict.json")
        snapshot["environment"]["data"]["variables"]["SECRET_EXAMPLE"] = "never-sent"
        findings = evaluate(snapshot)
        outbound = json.dumps(evidence_for_model(findings))
        self.assertNotIn("never-sent", outbound)
        self.assertNotIn("/usr/local/", outbound)
        self.assertNotIn("/opt/a/", outbound)

    def test_missing_probe_not_mistaken_for_observation(self):
        snapshot = load("driver-too-old.json")
        snapshot["cuda"] = {"status": "not_implemented"}
        self.assertEqual(evaluate(snapshot), [])


if __name__ == "__main__":
    unittest.main()
