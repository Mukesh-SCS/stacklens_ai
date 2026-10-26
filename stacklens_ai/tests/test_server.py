import json
import threading
import unittest
from http.server import ThreadingHTTPServer
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from stacklens_ai.server import Handler


class ServerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        cls.base = f"http://127.0.0.1:{cls.server.server_port}"
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)

    def test_health(self):
        with urlopen(self.base + "/api/health") as response:
            body = json.load(response)
        self.assertTrue(body["ok"])
        self.assertIn("ai_configured", body)

    def test_homepage(self):
        with urlopen(self.base + "/") as response:
            html = response.read().decode()
        self.assertIn("StackLens AI", html)

    def test_deterministic_api_end_to_end(self):
        with urlopen(self.base + "/api/examples/driver") as response:
            example = json.load(response)
        payload = json.dumps({"snapshot": example, "use_ai": False}).encode()
        request = Request(
            self.base + "/api/analyze",
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        with urlopen(request) as response:
            body = json.load(response)
        self.assertEqual(body["finding_count"], 1)
        self.assertEqual(body["findings"][0]["rule_id"], "CUDA-DRIVER-0001")
        self.assertEqual(body["ai_status"], "not_requested")

    def test_bad_schema_returns_http_400(self):
        payload = json.dumps({"snapshot": {"schema_version": 99}}).encode()
        request = Request(
            self.base + "/api/analyze",
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        with self.assertRaises(HTTPError) as error:
            urlopen(request)
        self.assertEqual(error.exception.code, 400)


if __name__ == "__main__":
    unittest.main()
