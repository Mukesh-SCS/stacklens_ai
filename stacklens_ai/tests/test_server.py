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
        self.assertIn("nebius_model", body)
        self.assertIn("budget", body)

    def test_homepage(self):
        with urlopen(self.base + "/") as response:
            html = response.read().decode("utf-8")
            headers = dict(response.headers)
        self.assertIn("StackLens AI", html)
        self.assertIn("Evidence-Grounded", html)
        self.assertEqual(headers.get("X-Content-Type-Options"), "nosniff")
        self.assertIn("Content-Security-Policy", headers)

    def test_static_assets(self):
        for path, mime in [("/styles.css", "text/css"), ("/app.js", "text/javascript")]:
            with self.subTest(path=path):
                with urlopen(self.base + path) as response:
                    self.assertEqual(response.status, 200)
                    self.assertIn(mime, response.headers.get("Content-Type"))

    def test_examples_endpoints(self):
        with urlopen(self.base + "/api/examples") as response:
            body = json.load(response)
            self.assertEqual(len(body["examples"]), 3)

        for key in ["driver", "selectors", "components"]:
            with self.subTest(key=key):
                with urlopen(self.base + f"/api/examples/{key}") as response:
                    snapshot = json.load(response)
                    self.assertEqual(snapshot.get("schema_version"), 1)

    def test_deterministic_api_end_to_end(self):
        with urlopen(self.base + "/api/examples/driver") as response:
            example = json.load(response)
        payload = json.dumps({"snapshot": example, "use_ai": False}).encode("utf-8")
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

    def test_ai_requested_without_key_reports_not_configured(self):
        with urlopen(self.base + "/api/examples/driver") as response:
            example = json.load(response)
        payload = json.dumps({"snapshot": example, "use_ai": True}).encode("utf-8")
        request = Request(
            self.base + "/api/analyze",
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        with urlopen(request) as response:
            body = json.load(response)
        self.assertEqual(body["ai_status"], "not_configured")
        self.assertEqual(body["finding_count"], 1)

    def test_bad_schema_returns_http_400(self):
        payload = json.dumps({"snapshot": {"schema_version": 99}}).encode("utf-8")
        request = Request(
            self.base + "/api/analyze",
            data=payload,
            headers={"Content-Type": "application/json"},
        )
        with self.assertRaises(HTTPError) as ctx:
            urlopen(request)
        self.assertEqual(ctx.exception.code, 400)

    def test_bad_content_type_returns_http_415(self):
        request = Request(
            self.base + "/api/analyze",
            data=b"not json",
            headers={"Content-Type": "text/plain"},
        )
        with self.assertRaises(HTTPError) as ctx:
            urlopen(request)
        self.assertEqual(ctx.exception.code, 415)

    def test_oversized_payload_returns_http_413(self):
        oversized = b"x" * (260 * 1024)
        request = Request(
            self.base + "/api/analyze",
            data=oversized,
            headers={"Content-Type": "application/json"},
        )
        with self.assertRaises(HTTPError) as ctx:
            urlopen(request)
        self.assertEqual(ctx.exception.code, 413)

    def test_unknown_example_returns_http_404(self):
        with self.assertRaises(HTTPError) as ctx:
            urlopen(self.base + "/api/examples/nonexistent")
        self.assertEqual(ctx.exception.code, 404)


if __name__ == "__main__":
    unittest.main()
