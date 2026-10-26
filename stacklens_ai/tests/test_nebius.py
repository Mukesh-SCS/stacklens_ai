import json
import os
import unittest
from unittest.mock import patch

from stacklens_ai.engine import evaluate
from stacklens_ai.nebius import NebiusError, explain


class FakeHTTP:
    def __init__(self, data):
        self.data = data

    def __enter__(self):
        return self

    def __exit__(self, *args):
        return False

    def read(self, *args):
        return json.dumps(self.data).encode()


class NebiusTests(unittest.TestCase):
    def setUp(self):
        self.findings = evaluate({"schema_version": 1, "nvidia_driver": {
            "status": "collected", "data": {
                "kernel_module_version": "535", "userspace_version": "550"
            }
        }})

    def test_does_not_fake_response_without_credentials(self):
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaises(NebiusError):
                explain(self.findings)

    def test_accepts_only_existing_finding_ids(self):
        response = {"choices": [{"message": {"content": json.dumps({
            "explanations": [
                {"rule_id": "INVENTED", "explanation": "Not real"},
                {"rule_id": "NVIDIA-DRIVER-0001", "explanation": "Versions disagree."}
            ]
        })}}]}
        with patch.dict(os.environ, {"NEBIUS_API_KEY": "testing"}):
            with patch("stacklens_ai.nebius.urlopen", return_value=FakeHTTP(response)) as call:
                model, rows = explain(self.findings)
                self.assertIn("nemotron", model)
                self.assertEqual(rows, [{"rule_id": "NVIDIA-DRIVER-0001", "explanation": "Versions disagree."}])
                sent = json.loads(call.call_args.args[0].data.decode())
                self.assertEqual(sent["model"], model)
                self.assertNotIn("INVENTED", json.dumps(sent))

    def test_rejects_unusable_ai_output(self):
        response = {"choices": [{"message": {"content": "not JSON"}}]}
        with patch.dict(os.environ, {"NEBIUS_API_KEY": "testing"}):
            with patch("stacklens_ai.nebius.urlopen", return_value=FakeHTTP(response)):
                with self.assertRaises(NebiusError):
                    explain(self.findings)


if __name__ == "__main__":
    unittest.main()
