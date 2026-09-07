"""Regression checks for fixture conversions and HTTP loader failures."""
import contextlib
import io
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import tempfile
import threading
import unittest

from generate_clickhouse_data import convert_timestamp
from generate_postgres_data import convert_list_insert_pg
from load_clickhouse_data import batch_load_clickhouse


class FixtureConversionTests(unittest.TestCase):
    def test_postgres_preserves_columns_and_quoted_metadata(self):
        source = "INSERT INTO list_entries (list_id, value, expires_at, metadata) VALUES ('blocked_users_db', 'O''Brien', '2030-01-01T12:00:00', '{\"reason\":\"test, expiry\"}');"
        result = convert_list_insert_pg(source)
        self.assertEqual(result, source.replace('2030-01-01T', '2030-01-01 '))

    def test_clickhouse_preserves_future_expiration_and_timezone(self):
        self.assertEqual(convert_timestamp("'2030-01-01T20:00:00.123456+08:00'"),
                         "toDateTime64('2030-01-01 12:00:00.123', 3, 'UTC')")
        self.assertEqual(convert_timestamp("'2020-01-01T12:00:00Z'"),
                         "toDateTime64('2020-01-01 12:00:00.000', 3, 'UTC')")
        self.assertEqual(convert_timestamp('NULL'), 'NULL')
        with self.assertRaises(ValueError):
            convert_timestamp("'invalid'")

    def test_clickhouse_loader_rejects_http_error_without_exception_text(self):
        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                self.rfile.read(int(self.headers.get('Content-Length', '0')))
                self.send_response(403)
                self.end_headers()
                self.wfile.write(b'Forbidden')

            def log_message(self, *args):
                pass

        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                sql = Path(directory) / 'fixture.sql'
                sql.write_text('CREATE TABLE events (id UInt64);\nINSERT INTO events (id) VALUES (1);\n')
                with contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(batch_load_clickhouse(str(sql), f'http://127.0.0.1:{server.server_port}'), 1)
        finally:
            server.shutdown()
            server.server_close()
            worker.join()


if __name__ == '__main__':
    unittest.main()
