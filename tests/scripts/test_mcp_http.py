#!/usr/bin/env python3
"""Real HTTP MCP wire checks; build corint-decision-mcp first."""
import json
import os
from pathlib import Path
import selectors
import subprocess
import unittest
from urllib.error import HTTPError
from urllib.request import Request, build_opener, ProxyHandler

ROOT = Path(__file__).resolve().parents[2]
BINARY = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug/corint-mcp"
urlopen = build_opener(ProxyHandler({})).open


class HttpTest(unittest.TestCase):
    def test_http_mcp_tools_health_and_origin_checks(self):
        process = subprocess.Popen([str(BINARY), "--config", str(ROOT / "tests/fixtures/mcp/catalog.json"),
                                    "--http-listen", "127.0.0.1:0"],
                                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.PIPE, text=True)
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(process.stderr, selectors.EVENT_READ)
                self.assertTrue(selector.select(timeout=15), "MCP did not start")
            announcement = process.stderr.readline().strip()
            self.assertTrue(announcement.startswith("MCP Server listening on http://"), announcement)
            base = announcement.split(" on ", 1)[1]
            with urlopen(base + "/health", timeout=5) as response:
                self.assertEqual(json.load(response)["service"], "corint-mcp")
            headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}

            def rpc(method, params, extra=None):
                request = Request(base + "/mcp", data=json.dumps({"jsonrpc":"2.0", "id":1,
                                  "method":method, "params":params}).encode(),
                                  headers=dict(headers, **(extra or {})))
                with urlopen(request, timeout=10) as response:
                    session = response.headers.get("Mcp-Session-Id")
                    if session:
                        headers["Mcp-Session-Id"] = session
                    payload = response.read().decode()
                    if response.headers.get_content_type() == "text/event-stream":
                        # Legacy sessions use SSE, including an empty priming
                        # event. Streamable HTTP permits either JSON or SSE.
                        messages = [json.loads(line[5:].strip()) for line in payload.splitlines()
                                    if line.startswith("data:") and line[5:].strip()]
                        return next(message for message in messages if message.get("id") == 1)
                    return json.loads(payload)

            initialized = rpc("initialize", {"protocolVersion":"2025-11-25", "capabilities":{},
                                               "clientInfo":{"name":"http-test", "version":"1"}})
            self.assertEqual(initialized["result"]["serverInfo"]["name"], "corint-decision")
            headers["MCP-Protocol-Version"] = "2025-11-25"
            request = Request(base + "/mcp", data=json.dumps({"jsonrpc":"2.0",
                              "method":"notifications/initialized"}).encode(), headers=headers)
            with urlopen(request, timeout=5) as response:
                self.assertEqual(response.status, 202)
            tools = rpc("tools/list", {})
            self.assertEqual(len(tools["result"]["tools"]), 6)
            decision = rpc("tools/call", {"name":"evaluate_decision", "arguments":{
                "policy_id":"payment-demo", "event":{"amount":1001}, "trace":True}})
            self.assertFalse(decision["result"]["isError"])
            self.assertEqual(decision["result"]["structuredContent"]["response"]["result"]["score"], 60)
            for header in ({"Origin":"https://untrusted.example"}, {"Host":"untrusted.example"}):
                with self.assertRaises(HTTPError) as error:
                    rpc("tools/list", {}, header)
                try:
                    self.assertEqual(error.exception.code, 403)
                finally:
                    error.exception.close()
        finally:
            process.terminate()
            process.wait(timeout=15)
            process.stderr.close()
        self.assertEqual(process.returncode, 0)

    def test_public_listener_is_rejected(self):
        result = subprocess.run([str(BINARY), "--config", str(ROOT / "tests/fixtures/mcp/catalog.json"),
                                 "--http-listen", "0.0.0.0:8082"],
                                capture_output=True, text=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("loopback", result.stderr)


if __name__ == "__main__":
    unittest.main()
