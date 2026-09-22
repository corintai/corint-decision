#!/usr/bin/env python3
"""Verify CSV replay and label removal against a local HTTP server."""

from contextlib import redirect_stderr, redirect_stdout
import csv
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import threading
import time
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("replay", ROOT / "scripts/replay_simulated_data.py")
REPLAY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPLAY)
FIELDS = ["TRANSACTION_ID", "TX_DATETIME", "CUSTOMER_ID", "TERMINAL_ID", "TX_AMOUNT",
          "TX_TIME_SECONDS", "TX_TIME_DAYS", "TX_FRAUD", "TX_FRAUD_SCENARIO"]


class ReplayTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.csv_path = Path(self.temp.name) / "transactions.csv"
        self.rows = [[str(i), f"2018-04-01 00:00:0{i}", "596", "3156", "57.16", str(i), "0", "1", "3"]
                     for i in range(3)]
        self.write_csv()

    def write_csv(self, fields=FIELDS):
        with self.csv_path.open("w", encoding="utf-8-sig", newline="") as output:
            writer = csv.writer(output)
            writer.writerow(fields)
            writer.writerows(self.rows)

    def run_replay(self, *arguments):
        output, errors = io.StringIO(), io.StringIO()
        with redirect_stdout(output), redirect_stderr(errors):
            code = REPLAY.main([str(self.csv_path), *arguments])
        return code, [json.loads(line) for line in output.getvalue().splitlines()], errors.getvalue()

    def test_preview_drops_both_labels_and_preserves_names_order_and_types(self):
        with patch.object(REPLAY.urllib.request.OpenerDirector, "open", side_effect=AssertionError("network call")):
            code, records, _ = self.run_replay("--dry-run", "--limit", "2")
        self.assertEqual(code, 0)
        self.assertEqual([item["row"] for item in records], [1, 2])
        for i, record in enumerate(records):
            self.assertEqual(set(record["request"]), {"event"})
            event = record["request"]["event"]
            self.assertEqual(list(event), FIELDS[:7])
            self.assertEqual(event["TRANSACTION_ID"], i)
            self.assertEqual(event["TX_DATETIME"], self.rows[i][1])
            self.assertEqual(event["TX_AMOUNT"], 57.16)
            self.assertTrue(all(isinstance(event[key], int) for key in REPLAY.INTEGER_FIELDS))
            self.assertNotIn("TX_FRAUD", json.dumps(record))

    def test_resume_selects_data_rows_and_does_not_read_beyond_limit(self):
        self.rows[2][4] = "invalid"
        self.write_csv()
        code, records, _ = self.run_replay("--dry-run", "--start-row", "2", "--limit", "1")
        self.assertEqual(code, 0)
        self.assertEqual(records[0]["row"], 2)
        self.assertEqual(records[0]["request"]["event"]["TRANSACTION_ID"], 1)

    def test_optional_routing_and_tenant_metadata_do_not_rename_csv_fields(self):
        code, records, _ = self.run_replay("--dry-run", "--limit", "1", "--tenant-id", "demo",
                                         "--event-type", "transaction", "--source", "simulated")
        self.assertEqual(code, 0)
        body = records[0]["request"]
        self.assertEqual(body["tenant_id"], "demo")
        self.assertEqual(body["business_event_id"], "simulated-0")
        self.assertEqual(set(body["event"]), set(FIELDS[:7]) | {"type", "source"})
        self.assertNotIn("tenant_id", body["event"])

    def test_bad_headers_and_nonfinite_amounts_fail(self):
        for fields in [FIELDS + ["TX_AMOUNT"], FIELDS + ["UNKNOWN_LABEL"], FIELDS[1:]]:
            self.write_csv(fields)
            self.assertEqual(self.run_replay("--dry-run")[0], 1)
        self.rows[0][4] = "NaN"
        self.write_csv()
        code, records, errors = self.run_replay("--dry-run")
        self.assertEqual(code, 1)
        self.assertEqual(records, [])
        self.assertIn("TX_AMOUNT must be finite", errors)

    def test_empty_selection_fails(self):
        self.assertEqual(self.run_replay("--dry-run", "--start-row", "10")[0], 1)

    def test_summary_counts_decisions_and_unique_rule_hits_per_transaction(self):
        responses = [(200, {"pipeline_id": "test_pipeline", "decision": {
            "result": result, "evidence": {"triggered_rules": rules}}})
            for result, rules in [("approve", []), ("review", ["r1", "r1", "r2"]), ("decline", ["r2"])]]
        with patch.dict(os.environ, {"CORINT_DECISION_TOKEN": "test-token"}), patch.object(REPLAY, "post_decision", side_effect=responses):
            code, records, summary = self.run_replay("--url", "http://localhost:1/v1/decide", "--interval", "0")
        self.assertEqual(code, 0)
        self.assertEqual(len(records), 3)
        self.assertIn("approve: 1 (33.33%)", summary)
        self.assertIn("review: 1 (33.33%)", summary)
        self.assertIn("decline: 1 (33.33%)", summary)
        self.assertIn("test_pipeline: 3 (100.00%)", summary)
        self.assertIn("命中任意规则的交易: 2 (66.67%)", summary)
        self.assertIn("未命中规则的交易: 1 (33.33%)", summary)
        self.assertIn("r1: 1 (33.33%)", summary)
        self.assertIn("r2: 2 (66.67%)", summary)

    def test_summary_survives_failure_and_interrupt_without_counting_failed_requests(self):
        first = (200, {"decision": {"result": "review", "evidence": {"triggered_rules": ["r1"]}}})
        for error, expected_code in [(ValueError("invalid response"), 1), (KeyboardInterrupt(), 130)]:
            with self.subTest(error=type(error).__name__), patch.dict(os.environ, {"CORINT_DECISION_TOKEN": "test-token"}), patch.object(REPLAY, "post_decision", side_effect=[first, error]):
                code, records, summary = self.run_replay("--url", "http://localhost:1/v1/decide", "--interval", "0")
                self.assertEqual(code, expected_code)
                self.assertEqual(len(records), 1)
                self.assertIn("仅计入已完成响应，共 1 条", summary)
                self.assertIn("命中任意规则的交易: 1 (100.00%)", summary)

    def test_missing_evidence_is_not_counted_as_no_hit_and_preview_has_no_statistics(self):
        statistics = REPLAY.ReplayStats()
        statistics.record({"decision": {"result": "approve"}})
        output = io.StringIO()
        with redirect_stderr(output):
            statistics.print_summary()
        self.assertIn("未命中规则的交易: 0 (0.00%)", output.getvalue())
        self.assertIn("缺少有效命中信息的交易: 1 (100.00%)", output.getvalue())
        self.assertNotIn("回放统计", self.run_replay("--dry-run", "--limit", "1")[2])

    def start_server(self, fail_id=None):
        captured, active = [], []

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                active.append(body)
                captured.append((self.path, self.headers["Authorization"], body, len(active)))
                time.sleep(0.005)
                status = 500 if body["event"]["TRANSACTION_ID"] == fail_id else 200
                data = json.dumps({"decision": {"result": "approve"}}).encode()
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                active.remove(body)
                self.wfile.write(data)

            def log_message(self, *args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()

        def close():
            server.shutdown()
            server.server_close()
            worker.join(timeout=2)

        self.addCleanup(close)
        return f"http://127.0.0.1:{server.server_port}/v1/decide", captured

    def test_real_http_requests_are_sequential_and_never_include_labels(self):
        url, captured = self.start_server()
        with patch.dict(os.environ, {"CORINT_DECISION_TOKEN": "test-token"}):
            code, records, errors = self.run_replay("--url", url, "--interval", "0", "--limit", "3")
        self.assertEqual(code, 0, errors)
        self.assertEqual(len(records), 3)
        self.assertEqual([item[2]["event"]["TRANSACTION_ID"] for item in captured], [0, 1, 2])
        for path, auth, body, concurrent in captured:
            self.assertEqual(path, "/v1/decide")
            self.assertEqual(auth, "Bearer test-token")
            self.assertEqual(set(body["event"]), set(FIELDS[:7]))
            self.assertEqual(concurrent, 1)
        self.assertNotIn("test-token", errors)

    def test_http_failure_stops_without_retry_or_later_requests(self):
        url, captured = self.start_server(fail_id=1)
        with patch.dict(os.environ, {"CORINT_DECISION_TOKEN": "test-token"}):
            code, records, errors = self.run_replay("--url", url, "--interval", "0")
        self.assertEqual(code, 1)
        self.assertEqual(len(captured), 2)
        self.assertEqual(len(records), 1)
        self.assertIn("data row 2: HTTP 500", errors)

    def test_concurrent_http_is_bounded_complete_and_label_free(self):
        template = self.rows[0]
        self.rows = [[str(i), *template[1:]] for i in range(24)]
        self.write_csv()
        url, captured = self.start_server()
        with patch.dict(os.environ, {"CORINT_DECISION_TOKEN": "test-token"}):
            code, records, summary = self.run_replay("--url", url, "--concurrency", "4", "--interval", "0")
        self.assertEqual(code, 0, summary)
        self.assertEqual(len(records), 24)
        self.assertEqual({r['row'] for r in records}, set(range(1, 25)))
        self.assertEqual({r['transaction_id'] for r in records}, set(range(24)))
        self.assertGreater(max(item[3] for item in captured), 1)
        self.assertLessEqual(max(item[3] for item in captured), 4)
        self.assertTrue(all(set(item[2]['event']) == set(FIELDS[:7]) for item in captured))
        self.assertIn("并发上限: 4", summary)
        self.assertIn("完成吞吐量:", summary)
        self.assertIn("approve: 24 (100.00%)", summary)

    def test_concurrent_failure_drains_successes_before_token_cleanup_and_stops_dispatch(self):
        self.rows.append(["3", *self.rows[0][1:]])
        self.rows[3][4] = "must not be read"
        self.write_csv()
        barrier = threading.Barrier(3)
        captured = []

        def respond(opener, args, payload):
            transaction_id = payload['event']['TRANSACTION_ID']
            captured.append(transaction_id)
            barrier.wait(timeout=3)
            if transaction_id == 0:
                raise REPLAY.urllib.error.HTTPError(args.url, 500, 'failed', {}, io.BytesIO())
            time.sleep(0.03)
            self.assertEqual(args.token, 'test-token')
            return 200, {'decision': {'result': 'approve', 'evidence': {'triggered_rules': []}}}

        with patch.dict(os.environ, {"CORINT_DECISION_TOKEN": "test-token"}), patch.object(REPLAY, 'post_decision', side_effect=respond):
            code, records, summary = self.run_replay('--url', 'http://localhost:1/v1/decide', '--concurrency', '3', '--interval', '0')
        self.assertEqual(code, 1, summary)
        self.assertEqual(set(captured), {0, 1, 2})
        self.assertEqual({r['transaction_id'] for r in records}, {1, 2})
        self.assertIn('data row 1: HTTP 500', summary)
        self.assertIn('失败或未确认的请求: 1', summary)
        self.assertIn('仅计入已完成响应，共 2 条', summary)

    def test_concurrent_interrupt_drains_inflight_responses(self):
        barrier = threading.Barrier(3)
        waiting = threading.Event()
        real_wait = REPLAY.wait

        def respond(opener, args, payload):
            barrier.wait(timeout=3)
            waiting.set()
            time.sleep(0.03)
            self.assertEqual(args.token, 'test-token')
            return 200, {'decision': {'result': 'approve', 'evidence': {'triggered_rules': []}}}

        calls = 0
        def interrupted_wait(*args, **kwargs):
            nonlocal calls
            calls += 1
            if calls == 1:
                self.assertTrue(waiting.wait(timeout=3))
                REPLAY.signal.getsignal(REPLAY.signal.SIGTERM)(REPLAY.signal.SIGTERM, None)
            return real_wait(*args, **kwargs)

        with patch.dict(os.environ, {"CORINT_DECISION_TOKEN": "test-token"}), patch.object(REPLAY, 'post_decision', side_effect=respond), patch.object(REPLAY, 'wait', side_effect=interrupted_wait):
            code, records, summary = self.run_replay('--url', 'http://localhost:1/v1/decide', '--concurrency', '3', '--interval', '0')
        self.assertEqual(code, 130, summary)
        self.assertEqual(len(records), 3)
        self.assertIn('仅计入已完成响应，共 3 条', summary)

    def local_credentials(self, url):
        directory = Path(self.temp.name) / "run"
        directory.mkdir(exist_ok=True)
        (directory / "decision.pid.json").write_text(json.dumps({"pid": os.getpid(), "url": url.removesuffix("/v1/decide")}))
        credentials = directory / "local-credentials.json"
        credentials.write_text(json.dumps({"CORINT_DECISION_TOKEN": "borrowed-local-token"}))
        credentials.chmod(0o600)
        return directory

    def test_local_url_and_token_are_discovered_then_cleared(self):
        url, captured = self.start_server()
        directory = self.local_credentials(url)
        with patch.dict(os.environ, {"CORINT_RUN_DIR": str(directory), "CORINT_SERVER_URL": ""}):
            os.environ.pop("CORINT_DECISION_TOKEN", None)
            code, records, errors = self.run_replay("--url", url, "--limit", "1", "--interval", "0")
            self.assertNotIn("CORINT_DECISION_TOKEN", os.environ)
        self.assertEqual(code, 0, errors)
        self.assertEqual(len(records), 1)
        self.assertEqual(captured[0][1], "Bearer borrowed-local-token")
        self.assertNotIn("borrowed-local-token", errors + json.dumps(records))
        self.assertTrue((directory / "local-credentials.json").exists())

    def test_borrowed_token_is_cleared_on_http_failure_and_interrupt(self):
        url, captured = self.start_server(fail_id=0)
        directory = self.local_credentials(url)
        with patch.dict(os.environ, {"CORINT_RUN_DIR": str(directory)}):
            os.environ.pop("CORINT_DECISION_TOKEN", None)
            self.assertEqual(self.run_replay("--url", url, "--limit", "1")[0], 1)
            self.assertNotIn("CORINT_DECISION_TOKEN", os.environ)

            def interrupted(opener, args, payload):
                self.assertEqual(os.environ["CORINT_DECISION_TOKEN"], "borrowed-local-token")
                raise KeyboardInterrupt

            with patch.object(REPLAY, "post_decision", side_effect=interrupted):
                self.assertEqual(self.run_replay("--url", url, "--limit", "1")[0], 130)
            self.assertNotIn("CORINT_DECISION_TOKEN", os.environ)
        self.assertEqual(len(captured), 1)

    def test_existing_token_is_restored_and_preview_does_not_read_credentials(self):
        with patch.dict(os.environ, {"CORINT_DECISION_TOKEN": " existing-token "}):
            args = REPLAY.arguments(["--url", "http://127.0.0.1:1/v1/decide"])
            with self.assertRaises(ValueError):
                with REPLAY.temporary_token(args):
                    self.assertEqual(args.token, "existing-token")
                    raise ValueError("test failure")
            self.assertEqual(os.environ["CORINT_DECISION_TOKEN"], " existing-token ")
            self.assertEqual(args.token, "")
            with patch.object(Path, "read_text", side_effect=AssertionError("credential read")):
                args.dry_run = True
                with REPLAY.temporary_token(args):
                    self.assertEqual(args.token, "")

    def test_local_token_is_not_used_for_another_origin(self):
        url, captured = self.start_server()
        directory = self.local_credentials(url)
        with patch.dict(os.environ, {"CORINT_RUN_DIR": str(directory)}):
            os.environ.pop("CORINT_DECISION_TOKEN", None)
            with patch.object(REPLAY.urllib.request.OpenerDirector, "open", side_effect=AssertionError("unexpected network")):
                code, records, errors = self.run_replay("--url", "http://example.invalid/v1/decide", "--limit", "1")
            self.assertNotIn("CORINT_DECISION_TOKEN", os.environ)
        self.assertEqual(code, 1)
        self.assertEqual(records, [])
        self.assertEqual(captured, [])
        self.assertNotIn("borrowed-local-token", errors)


if __name__ == "__main__":
    unittest.main()
