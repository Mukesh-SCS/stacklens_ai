import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from stacklens_ai.config import (
    DEFAULT_BASE_URL,
    DEFAULT_BUDGET,
    DEFAULT_MODEL,
    BudgetTracker,
    get_config,
    load_env_file,
    parse_env_line,
)


class ConfigTests(unittest.TestCase):
    def test_parse_env_line(self):
        self.assertIsNone(parse_env_line(""))
        self.assertIsNone(parse_env_line("   "))
        self.assertIsNone(parse_env_line("# comment line"))
        self.assertIsNone(parse_env_line("NO_EQUALS_SIGN"))

        self.assertEqual(parse_env_line("KEY=VAL"), ("KEY", "VAL"))
        self.assertEqual(parse_env_line("KEY=VAL # comment"), ("KEY", "VAL # comment"))
        self.assertEqual(parse_env_line('KEY="quoted val"'), ("KEY", "quoted val"))
        self.assertEqual(parse_env_line("KEY='single quoted'"), ("KEY", "single quoted"))
        self.assertEqual(parse_env_line("EMPTY="), ("EMPTY", ""))

    def test_env_file_loading_precedence(self):
        with tempfile.NamedTemporaryFile("w", delete=False, suffix=".env") as f:
            f.write("TEST_VAR_A=from_file\n")
            f.write("TEST_VAR_B=from_file_only\n")
            temp_path = Path(f.name)

        try:
            with patch.dict(os.environ, {"TEST_VAR_A": "from_process"}, clear=False):
                loaded = load_env_file(temp_path)
                self.assertIn("TEST_VAR_A", loaded)
                self.assertIn("TEST_VAR_B", loaded)
                # Process variable must take precedence!
                self.assertEqual(os.environ["TEST_VAR_A"], "from_process")
                # Unset variable must be populated from file
                self.assertEqual(os.environ["TEST_VAR_B"], "from_file_only")
        finally:
            if temp_path.is_file():
                temp_path.unlink()

    def test_default_config_values(self):
        with patch.dict(os.environ, {}, clear=True):
            cfg = get_config()
            self.assertEqual(cfg.nebius_model, DEFAULT_MODEL)
            self.assertEqual(cfg.nebius_base_url, DEFAULT_BASE_URL)
            self.assertEqual(cfg.model_budget, DEFAULT_BUDGET)
            self.assertEqual(cfg.host, "127.0.0.1")
            self.assertEqual(cfg.port, 8000)
            self.assertFalse(cfg.is_ai_configured)

    def test_safe_dict_excludes_secrets(self):
        with patch.dict(os.environ, {"NEBIUS_API_KEY": "super-secret-token"}):
            cfg = get_config()
            self.assertTrue(cfg.is_ai_configured)
            safe = cfg.to_safe_dict()
            self.assertNotIn("super-secret-token", str(safe))
            self.assertNotIn("nebius_api_key", safe)

    def test_budget_tracker_limits_and_rate_limiting(self):
        tracker = BudgetTracker()
        # Budget of 3 calls total
        allowed1, _ = tracker.allow_call("127.0.0.1", budget=3)
        self.assertTrue(allowed1)
        allowed2, _ = tracker.allow_call("127.0.0.1", budget=3)
        self.assertTrue(allowed2)
        allowed3, _ = tracker.allow_call("127.0.0.1", budget=3)
        self.assertTrue(allowed3)

        # 4th call should hit total budget cap of 3
        allowed4, reason = tracker.allow_call("127.0.0.1", budget=3)
        self.assertFalse(allowed4)
        self.assertIn("budget", reason.lower())

        status = tracker.status(budget=3)
        self.assertEqual(status["consumed_calls"], 3)
        self.assertEqual(status["remaining_calls"], 0)


if __name__ == "__main__":
    unittest.main()
