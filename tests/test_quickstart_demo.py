"""Demo startup/HTTP regression checks, without Rust builds or database services.

Run: python3 -m unittest discover -s tests -p 'test_quickstart_demo.py' -v
"""
import contextlib
import http.server
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import unittest

ROOT = Path(__file__).resolve().parents[1]
TOKEN = 'demo-regression-decision-token-00000001'


@contextlib.contextmanager
def endpoint(status=200, body=None):
    requests = []

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_POST(self):
            requests.append((self.headers.get('Authorization'),
                             self.rfile.read(int(self.headers['Content-Length']))))
            self.respond()

        def do_GET(self):
            self.respond()

        def respond(self):
            self.send_response(status)
            self.end_headers()
            self.wfile.write(json.dumps(body).encode())

        def log_message(self, *args):
            pass

    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server.server_port, requests
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


class DemoTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='corint-demo-test-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        shutil.copytree(ROOT / 'quickstart/config', self.root / 'quickstart/config')
        shutil.copytree(ROOT / 'quickstart/risingwave', self.root / 'quickstart/risingwave')
        shutil.copy(ROOT / 'quickstart/decide_demo.sh', self.root / 'quickstart/decide_demo.sh')
        shutil.copytree(ROOT / 'repository', self.root / 'repository')
        (self.root / 'config').mkdir()

    def shell(self, command, **env):
        return subprocess.run(
            ['bash', '-c', 'source "$1"; ' + command, 'demo-test',
             str(self.root / 'quickstart/decide_demo.sh')],
            stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=15,
            env={**os.environ, 'CORINT_DECISION_TOKEN': TOKEN,
                 'CORINT_PUBLISHER_TOKEN': 'demo-regression-publisher-token-000001',
                 'CORINT_TENANT_ID': 'quickstart', **env},
        )

    def test_fresh_checkout_initializes_all_datasources_before_reading(self):
        for datasource in ('sqlite', 'postgresql', 'clickhouse', 'redis', 'risingwave', '5', ''):
            with self.subTest(datasource=datasource):
                (self.root / 'config/server.yaml').unlink(missing_ok=True)
                result = self.shell(
                    'check_database_availability() { echo CONFIG_READY; exit 0; }; main',
                    TEST_DATASOURCE=datasource,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn('CONFIG_READY', result.stdout)
                self.assertTrue((self.root / 'config/server.yaml').exists())

    def test_risingwave_auto_run_checks_live_updates_with_both_protocols(self):
        for protocol in ('http', 'grpc'):
            with self.subTest(protocol=protocol):
                result = self.shell('''
                    DATASOURCE=risingwave; PROTOCOL="$TEST_PROTOCOL"
                    risingwave_sql() { cat; }
                    run_test() { echo "DECISION:$PROTOCOL:${9}"; }
                    run_all_scenarios
                ''', TEST_PROTOCOL=protocol)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                decisions = [line for line in result.stdout.splitlines()
                             if line.startswith('DECISION:')]
                self.assertEqual(decisions, [f'DECISION:{protocol}:{value}'
                                            for value in ('APPROVE', 'DECLINE', 'REVIEW')])
                self.assertEqual(result.stdout.count('FLUSH;'), 3)
                self.assertEqual(result.stdout.count('DELETE FROM'), 2)

    def test_risingwave_config_preserves_url_and_uses_isolated_schema(self):
        url = 'postgresql://root@localhost:4566/dev?application_name=demo&options=a\\b'
        result = self.shell('select_datasource; configure_demo_server',
                            TEST_DATASOURCE='risingwave', RISINGWAVE_URL=url)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        config = (self.root / 'config/server.yaml').read_text()
        connection = next(line.split(': ', 1)[1] for line in config.splitlines()
                          if line.strip().startswith('connection_string:'))
        self.assertEqual(json.loads(connection), url)
        self.assertRegex(config, r'schema: corint_demo_[0-9a-f]{32}\n')
        self.assertNotIn('events_datasource:', config)
        self.assertIn('provider: risingwave', config)

    def test_risingwave_partial_initialization_is_cleaned_up(self):
        result = self.shell('''
            RISINGWAVE_SCHEMA=corint_demo_test
            risingwave_sql() {
                local sql; sql=$(cat); echo "$sql"
                [[ "$sql" != *"CREATE TABLE"* ]]
            }
            if initialize_risingwave_data; then exit 99; fi
            [ "$RISINGWAVE_SCHEMA_CREATED" = true ]
        ''')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('DROP SCHEMA :"rw_schema" CASCADE;', result.stdout)

    def test_occupied_port_is_preserved_and_credentials_are_generated(self):
        with endpoint(body={'service': 'agent-gateway', 'status': 'ok'}) as (port, _):
            result = self.shell('''
                DATASOURCE=sqlite; copy_config_file
                HTTP_PORT="$OCCUPIED_PORT"
                configure_demo_server
                [ "$HTTP_PORT" != "$OCCUPIED_PORT" ]
                [ "${#CORINT_DECISION_TOKEN}" -eq 64 ]
                [ "${#CORINT_PUBLISHER_TOKEN}" -eq 64 ]
                [ "$CORINT_DECISION_TOKEN" != "$CORINT_PUBLISHER_TOKEN" ]
                curl -fsS "http://127.0.0.1:$OCCUPIED_PORT/health"
            ''', OCCUPIED_PORT=str(port), CORINT_DECISION_TOKEN='', CORINT_PUBLISHER_TOKEN='')
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn('agent-gateway', result.stdout)

    def run_scenario(self, port):
        return self.shell('HTTP_HOST="127.0.0.1:$TEST_PORT"; PROTOCOL=http; test_normal_user',
                          TEST_PORT=str(port))

    def test_http_success_sends_auth_and_compares_decision(self):
        with endpoint(body={'request_id': 'test', 'pipeline_id': 'fraud', 'status': 200,
                            'decision': {'result': 'approve'}, 'process_time_ms': 0}) as (port, requests):
            result = self.run_scenario(port)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('RESULT: PASS', result.stdout)
            self.assertEqual(requests[0][0], f'Bearer {TOKEN}')
            self.assertEqual(json.loads(requests[0][1])['event']['user_id'], 'normal_user_001')

    def test_http_error_retains_status_and_original_body(self):
        with endpoint(401, {'error': 'Unauthorized'}) as (port, _):
            result = self.run_scenario(port)
            self.assertIn('HTTP 401', result.stderr)
            self.assertIn('Unauthorized', result.stderr)
            self.assertIn('RESULT: ERROR', result.stdout)
            self.assertNotIn('"request_id": null', result.stdout)

    def test_success_status_without_decision_is_not_projected_to_nulls(self):
        with endpoint(body={'error': 'Unexpected response'}) as (port, _):
            result = self.run_scenario(port)
            self.assertIn('Unexpected response', result.stdout)
            self.assertIn('RESULT: ERROR', result.stdout)
            self.assertNotIn('"request_id": null', result.stdout)

    def test_build_failure_is_not_hidden_by_tail(self):
        result = self.shell('cargo() { echo BUILD_FAILED; return 1; }; start_server')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Failed to build server', result.stdout)
        self.assertNotIn('Server started successfully', result.stdout)

    def test_grpc_auth_and_camel_case_response(self):
        result = self.shell('''
            grpcurl() {
                [ "$1" = "-plaintext" ] && [ "$2" = "-H" ] &&
                [ "$3" = "authorization: Bearer $CORINT_DECISION_TOKEN" ] || return 1
                echo '{"requestId":"grpc-test","pipelineId":"fraud","status":200,"decision":{"result":"approve"}}'
            }
            PROTOCOL=grpc; test_normal_user
        ''')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('RESULT: PASS', result.stdout)
        self.assertIn('"request_id": "grpc-test"', result.stdout)
        self.assertIn('"pipeline_id": "fraud"', result.stdout)
        self.assertIn('"process_time_ms": 0', result.stdout)

    def test_auto_run_exits_nonzero_after_scenario_errors(self):
        result = self.shell('''
            select_datasource() { :; }
            configure_demo_server() { :; }
            check_database_availability() { :; }
            initialize_data() { :; }
            start_server() { :; }
            http_health_check() { echo '{"status":"healthy"}'; }
            select_protocol() { PROTOCOL=http; }
            run_all_scenarios() { TEST_FAILURES=1; }
            main
        ''', TEST_AUTO_RUN='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('1 test scenario(s) failed', result.stdout)
        self.assertNotIn('All test scenarios passed', result.stdout)

    def test_dead_child_is_not_mistaken_for_another_healthy_service(self):
        binary = self.root / 'target/release/corint-decision-server'
        binary.parent.mkdir(parents=True)
        binary.write_text('#!/bin/bash\necho STARTUP_FAILED >&2\nexit 1\n')
        binary.chmod(0o755)
        with endpoint(body={'status': 'healthy'}) as (port, _):
            result = self.shell('''
                cargo() { return 0; }
                HTTP_PORT="$TEST_PORT"; HTTP_HOST="127.0.0.1:$TEST_PORT"
                start_server
            ''', TEST_PORT=str(port))
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('STARTUP_FAILED', result.stdout)
            self.assertNotIn('Server started successfully', result.stdout)


if __name__ == '__main__':
    unittest.main()
