import unittest

from stacklens_ai.preflight import run_preflight


class PreflightTests(unittest.TestCase):
    def test_preflight_offline_checks_pass(self):
        exit_code = run_preflight(run_live_smoke=False)
        self.assertEqual(exit_code, 0)


if __name__ == "__main__":
    unittest.main()
