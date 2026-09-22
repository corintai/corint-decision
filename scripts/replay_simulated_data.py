#!/usr/bin/env python3
"""Replay simulated-data.csv through POST /v1/decide (stdlib only).

Preview without sending requests:
  python3 scripts/replay_simulated_data.py --dry-run --limit 3

Register a temporary SQLite credential, start Decision, replay, and clean up:
  python3 scripts/replay_simulated_data.py --limit 100 --interval 0.1

Optional routing for policies that require event.type / event.source:
  python3 scripts/replay_simulated_data.py --event-type transaction --source simulated

Concurrent load test (bounded in-flight requests, no artificial delay):
  python3 scripts/replay_simulated_data.py --limit 4000 --concurrency 8 --interval 0

Use --tenant-id for the multi-tenant API. CSV field names and historical timestamps
are preserved. TX_FRAUD and TX_FRAUD_SCENARIO never enter the request. Numeric CSV
columns become JSON numbers. --concurrency defaults to 1 (serial). Concurrent
responses are emitted as they finish, retaining their CSV row/transaction IDs;
completion order is not CSV order. No automatic retries. On failure/interruption,
stop dispatching, collect in-flight responses, then clean up credentials/services.
Some in-flight failures have unknown server outcomes; inspect results before resuming.
No limit means the entire file (about 1.75 million records for the supplied CSV).
Each stdout line is a preview request or a result; progress/summary go to stderr.
The final summary counts decisions, pipelines, transactions with rule hits, and
per-rule hits (each rule counted at most once per transaction). Interrupted runs
summarize only completed responses. Missing rule evidence is reported separately.
This sends decisions only: it does not seed a historical feature database or send
feedback labels. Policies must accept the dataset's original field names.
By default the managed local Decision server uses its configured SQLite credential
database. A decide-only token hash is registered BEFORE Decision starts/restarts.
After replay the test service stops, the temporary database entry is removed, and
any previously running service is restored. Other credentials are preserved.
The local test instance loads tests/policies/simulated-data (override with
--repository). The default repository and server config files are not edited.
Use --auth-config if no managed authentication configuration is available.
Use --url (or CORINT_SERVER_URL) to call an already running service instead;
that mode uses an existing token and does not manage its database or lifecycle.
Tokens are scoped to this Python process and restored/removed on exit, including
failures. --dry-run does not load credentials or start/stop any service.
"""

import argparse
from collections import Counter
from concurrent.futures import FIRST_COMPLETED, ThreadPoolExecutor, wait
from contextlib import contextmanager, nullcontext
import csv
from datetime import datetime
import importlib.util
import itertools
import json
import math
import os
from pathlib import Path
import signal
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
LABELS = {"TX_FRAUD", "TX_FRAUD_SCENARIO"}
INTEGER_FIELDS = {"TRANSACTION_ID", "CUSTOMER_ID", "TERMINAL_ID",
                  "TX_TIME_SECONDS", "TX_TIME_DAYS"}
EVENT_FIELDS = INTEGER_FIELDS | {"TX_DATETIME", "TX_AMOUNT"}


class NoRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def positive_integer(value):
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("must be at least 1")
    return number


def nonnegative_number(value):
    number = float(value)
    if not math.isfinite(number) or number < 0:
        raise argparse.ArgumentTypeError("must be a finite nonnegative number")
    return number


def local_service(run_dir):
    try:
        state = json.loads((run_dir / "decision.pid.json").read_text())
        pid = state.get("pid")
        if not isinstance(pid, int) or isinstance(pid, bool) or pid <= 0:
            return None
        os.kill(pid, 0)
        url = state.get("url", "")
        parsed = urllib.parse.urlsplit(url)
        if parsed.scheme not in {"http", "https"} or parsed.hostname not in {"127.0.0.1", "localhost", "::1"}:
            return None
        if parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.path not in {"", "/"}:
            return None
        return url.rstrip("/")
    except (OSError, ValueError, TypeError, AttributeError):
        return None


def origin(url):
    parsed = urllib.parse.urlsplit(url)
    host = "127.0.0.1" if parsed.hostname == "localhost" else parsed.hostname
    return parsed.scheme, host, parsed.port or (443 if parsed.scheme == "https" else 80)


def local_auth(args):
    """Inspect only the registered local server, after verifying its process identity."""
    spec = importlib.util.spec_from_file_location("replay_services", ROOT / "scripts/services.py")
    services = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(services)
    services.STATE = args.run_dir
    state = json.loads((args.run_dir / "decision.pid.json").read_text())
    if not state.get("identity") or services.identity(state["pid"]) != state["identity"]:
        return None
    if origin(state["url"]) != origin(args.url):
        return None
    _, environment = services.process_configuration(state["pid"])
    auth_path = environment.get("CORINT_AUTH_CONFIG")
    if not auth_path:
        token = environment.get("CORINT_DECISION_TOKEN")
        if token:
            return {"token": token}
        return None
    raise ValueError("local service uses database authentication; supply a valid decision "
                     f"credential through {args.token_env}")


@contextmanager
def temporary_token(args):
    """Borrow local credentials for this process; always restore its environment."""
    previous = os.environ.get(args.token_env)
    args.token = ""
    if args.dry_run:
        yield
        return
    try:
        token = (previous or "").strip()
        if not token:
            service = local_service(args.run_dir)
            if service is None or origin(args.url) != origin(service):
                raise ValueError("no token for this endpoint; start the local service with scripts/start.sh "
                                 f"or set {args.token_env} for the target service")
            auth = local_auth(args)
            if auth:
                token = auth["token"]
            else:
                try:
                    credentials = json.loads((args.run_dir / "local-credentials.json").read_text())
                    token = credentials.get("CORINT_DECISION_TOKEN")
                except (OSError, ValueError, AttributeError):
                    raise ValueError("local service credentials are unavailable") from None
            if not isinstance(token, str) or not token.strip():
                raise ValueError("local service has no decision token")
            token = token.strip()
        os.environ[args.token_env] = token
        args.token = token
        yield
    finally:
        args.token = ""
        if previous is None:
            os.environ.pop(args.token_env, None)
        else:
            os.environ[args.token_env] = previous


def arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("csv_file", nargs="?", type=Path, default=ROOT / "simulated-data.csv")
    run_dir = Path(os.environ.get("CORINT_RUN_DIR", ROOT / ".run")).expanduser().resolve()
    base = (os.environ.get("CORINT_SERVER_URL") or local_service(run_dir) or "http://127.0.0.1:8080").rstrip("/")
    parser.add_argument("--url", help="existing endpoint; disables local database/service setup")
    parser.add_argument("--auth-config", type=Path, help="local SQLite authentication configuration")
    parser.add_argument("--repository", type=Path,
                        help="test policy repository (default: tests/policies/simulated-data)")
    parser.add_argument("--server-bin", type=Path, default=ROOT / "target/debug/corint-decision-server")
    parser.add_argument("--credential-ttl", type=positive_integer, default=604800,
                        help="temporary credential expiry in seconds (default: 7 days; removed on exit)")
    parser.add_argument("--token-env", default="CORINT_DECISION_TOKEN",
                        help="environment variable containing the Bearer token")
    parser.add_argument("--tenant-id", help="multi-tenant API tenant; omitted for compatibility API")
    parser.add_argument("--event-type", help="optionally add event.type for Registry routing")
    parser.add_argument("--source", help="optionally add event.source for Registry routing")
    parser.add_argument("--start-row", type=positive_integer, default=1,
                        help="1-based data row, excluding the header (default: 1)")
    parser.add_argument("--limit", type=positive_integer, help="maximum requests; default: all remaining rows")
    parser.add_argument("--concurrency", type=positive_integer, default=1,
                        help="maximum in-flight requests (default: 1; preserves CSV order)")
    parser.add_argument("--interval", type=nonnegative_number, default=0.1,
                        help="delay before refilling a completed request slot (default: 0.1; use 0 for load tests)")
    parser.add_argument("--timeout", type=nonnegative_number, default=30,
                        help="HTTP timeout in seconds (default: 30)")
    parser.add_argument("--dry-run", action="store_true", help="print requests without network calls or waits")
    args = parser.parse_args(argv)
    args.run_dir = run_dir
    args.manage_local = args.url is None and not os.environ.get("CORINT_SERVER_URL")
    args.url = args.url or base + "/v1/decide"
    if args.auth_config and not args.manage_local:
        parser.error("--auth-config requires local managed mode; omit --url and CORINT_SERVER_URL")
    if args.repository and not args.manage_local:
        parser.error("--repository requires local managed mode; it cannot change a remote server")
    args.repository = (args.repository or ROOT / "tests/policies/simulated-data").resolve()
    if args.manage_local and args.tenant_id and not args.dry_run:
        parser.error("--tenant-id requires an explicit --url to a multi-tenant service")
    if args.timeout == 0:
        parser.error("--timeout must be greater than zero")
    url = urllib.parse.urlsplit(args.url)
    if url.scheme not in {"http", "https"} or not url.hostname or url.username or url.password or url.fragment:
        parser.error("--url must be an HTTP(S) endpoint without credentials or a fragment")
    return args


def read_events(path, start_row=1, limit=None):
    """Keep only one CSV record in memory, validating before each request."""
    with path.open(encoding="utf-8-sig", newline="") as source:
        reader = csv.DictReader(source)
        fields = reader.fieldnames or []
        if len(fields) != len(set(fields)):
            raise ValueError("duplicate CSV column names")
        missing = EVENT_FIELDS - set(fields)
        unexpected = set(fields) - EVENT_FIELDS - LABELS
        if missing or unexpected:
            raise ValueError(f"unexpected CSV schema: missing={sorted(missing)}, unknown={sorted(unexpected)}")
        rows = itertools.islice(reader, start_row - 1,
                                None if limit is None else start_row - 1 + limit)
        for row_number, row in enumerate(rows, start_row):
            try:
                if None in row or any(value is None for value in row.values()):
                    raise ValueError("column count does not match the header")
                # Explicitly omit labels before conversion and request construction.
                event = {key: value for key, value in row.items() if key not in LABELS}
                for key in INTEGER_FIELDS:
                    event[key] = int(event[key])
                event["TX_AMOUNT"] = float(event["TX_AMOUNT"])
                if not math.isfinite(event["TX_AMOUNT"]):
                    raise ValueError("TX_AMOUNT must be finite")
                datetime.strptime(event["TX_DATETIME"], "%Y-%m-%d %H:%M:%S")
            except ValueError as error:
                raise ValueError(f"data row {row_number}: {error}") from error
            yield row_number, event


def make_payload(event, args):
    # Apply an allowlist again at the HTTP boundary, including for direct callers.
    clean = {key: value for key, value in event.items() if key in EVENT_FIELDS}
    if args.event_type is not None:
        clean["type"] = args.event_type
    if args.source is not None:
        clean["source"] = args.source
    payload = {"event": clean}
    if args.tenant_id is not None:
        payload["tenant_id"] = args.tenant_id
        payload["business_event_id"] = f"simulated-{clean['TRANSACTION_ID']}"
    return payload


def post_decision(opener, args, payload):
    data = json.dumps(payload, allow_nan=False).encode("utf-8")
    request = urllib.request.Request(args.url, data=data, method="POST", headers={
        "Content-Type": "application/json",
        "Authorization": "Bearer " + args.token,
    })
    with opener.open(request, timeout=args.timeout) as response:
        body = json.load(response)
        if not isinstance(body, dict):
            raise ValueError("decision response must be a JSON object")
        return response.status, body


def emit(record):
    print(json.dumps(record, ensure_ascii=False, allow_nan=False), flush=True)


def replay_concurrently(args, on_success, on_failure):
    """Keep at most N CSV records/futures, and drain them before auth cleanup."""
    stop = threading.Event()
    args.concurrent_stop = stop
    args.concurrent_interrupted = False
    events = read_events(args.csv_file, args.start_row, args.limit)
    pending = {}
    failure = None
    exhausted = False

    def send(event, delay):
        if stop.wait(delay):
            return None
        # Opener handlers have mutable state; never share them across threads.
        opener = urllib.request.build_opener(NoRedirects())
        started = time.monotonic()
        try:
            status, body = post_decision(opener, args, make_payload(event, args))
            return status, body, (time.monotonic() - started) * 1000
        except BaseException:
            stop.set()
            raise

    def fail(row, error):
        nonlocal failure
        stop.set()
        if failure is None:
            failure = row, error
        elif isinstance(error, urllib.error.HTTPError):
            error.close()

    try:
        with ThreadPoolExecutor(max_workers=args.concurrency) as executor:
            def fill(delay):
                nonlocal exhausted
                while not exhausted and not stop.is_set() and len(pending) < args.concurrency:
                    try:
                        row, event = next(events)
                    except StopIteration:
                        exhausted = True
                        break
                    except BaseException as error:
                        fail(None, error)
                        break
                    future = executor.submit(send, event, delay)
                    pending[future] = row, event

            fill(0)
            while pending:
                try:
                    finished, _ = wait(pending, return_when=FIRST_COMPLETED)
                except KeyboardInterrupt as error:
                    fail(None, error)
                    continue
                for future in finished:
                    row, event = pending.pop(future)
                    try:
                        result = future.result()
                    except BaseException as error:
                        on_failure()
                        fail(row, error)
                    else:
                        if result is not None:
                            on_success(row, event, *result)
                fill(args.interval)
    finally:
        stop.set()
        events.close()
        args.concurrent_stop = None
    if args.concurrent_interrupted:
        if failure is not None and isinstance(failure[1], urllib.error.HTTPError):
            failure[1].close()
        raise KeyboardInterrupt
    if failure is not None:
        args.failed_row = failure[0]
        raise failure[1]


class ReplayStats:
    def __init__(self):
        self.total = 0
        self.decisions = Counter()
        self.pipelines = Counter()
        self.rules = Counter()
        self.matched = 0
        self.unknown_evidence = 0
        self.failed = 0
        self.latency_total_ms = 0.0
        self.latency_max_ms = 0.0

    def record(self, response, elapsed_ms=0.0):
        self.total += 1
        self.latency_total_ms += elapsed_ms
        self.latency_max_ms = max(self.latency_max_ms, elapsed_ms)
        decision = response.get("decision")
        decision = decision if isinstance(decision, dict) else {}
        result = decision.get("result")
        self.decisions[result if isinstance(result, str) and result else "unknown"] += 1
        pipeline = response.get("pipeline_id")
        self.pipelines[pipeline if isinstance(pipeline, str) and pipeline else "unknown"] += 1
        evidence = decision.get("evidence")
        rules = evidence.get("triggered_rules") if isinstance(evidence, dict) else None
        if not isinstance(rules, list) or not all(isinstance(rule, str) and rule for rule in rules):
            self.unknown_evidence += 1
            return
        if rules:
            self.matched += 1
            self.rules.update(set(rules))

    def print_summary(self, concurrency=None, elapsed=None):
        def count(value):
            return f"{value} ({value / self.total:.2%})" if self.total else "0 (0.00%)"

        print(f"\n回放统计（仅计入已完成响应，共 {self.total} 条）", file=sys.stderr)
        print(f"失败或未确认的请求: {self.failed}", file=sys.stderr)
        if concurrency is not None and elapsed is not None:
            throughput = self.total / elapsed if elapsed > 0 else 0
            average = self.latency_total_ms / self.total if self.total else 0
            print(f"并发上限: {concurrency}; 回放耗时: {elapsed:.3f}s（不含应用启动和清理）", file=sys.stderr)
            print(f"完成吞吐量: {throughput:.2f} 请求/s; 平均响应: {average:.2f}ms; "
                  f"最大响应: {self.latency_max_ms:.2f}ms", file=sys.stderr)
        print("决策结果：", file=sys.stderr)
        known = ("approve", "review", "decline", "hold", "pass")
        for result in (*known, *sorted(set(self.decisions) - set(known))):
            print(f"  {result}: {count(self.decisions[result])}", file=sys.stderr)
        print("Pipeline：", file=sys.stderr)
        for pipeline, total in sorted(self.pipelines.items()):
            print(f"  {pipeline}: {count(total)}", file=sys.stderr)
        print(f"命中任意规则的交易: {count(self.matched)}", file=sys.stderr)
        print(f"未命中规则的交易: {count(self.total - self.matched - self.unknown_evidence)}", file=sys.stderr)
        if self.unknown_evidence:
            print(f"缺少有效命中信息的交易: {count(self.unknown_evidence)}", file=sys.stderr)
        print("各规则命中（每条交易对同一规则最多计一次，可命中多条规则）：", file=sys.stderr)
        for rule, total in sorted(self.rules.items(), key=lambda item: (-item[1], item[0])):
            print(f"  {rule}: {count(total)}", file=sys.stderr)
        if not self.rules:
            print("  无已确认的规则命中", file=sys.stderr)


def main(argv=None):
    args = arguments(argv)
    opener = urllib.request.build_opener(NoRedirects())
    completed = 0
    current_row = args.start_row
    started = time.monotonic()
    statistics = ReplayStats()
    replay_elapsed = 0.0

    def received(row, event, status, body, elapsed_ms):
        nonlocal completed, current_row
        current_row = row
        emit({"row": row, "transaction_id": event["TRANSACTION_ID"],
              "http_status": status, "elapsed_ms": round(elapsed_ms, 2), "response": body})
        statistics.record(body, elapsed_ms)
        completed += 1
        if completed % 100 == 0:
            print(f"Completed {completed} rows; latest completed row {row}", file=sys.stderr)

    def failed():
        statistics.failed += 1
    runtime = nullcontext()
    if args.manage_local and not args.dry_run:
        spec = importlib.util.spec_from_file_location("replay_local", ROOT / "scripts/replay_local.py")
        local = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(local)
        runtime = local.local_decision(args)
    previous_sigterm = signal.getsignal(signal.SIGTERM)
    previous_sigint = signal.getsignal(signal.SIGINT)

    def terminate(signum, frame):
        stop = getattr(args, "concurrent_stop", None)
        if stop is not None:
            args.concurrent_interrupted = True
            stop.set()
            return
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, terminate)
    signal.signal(signal.SIGINT, terminate)
    try:
        with runtime, temporary_token(args):
            replay_started = time.monotonic()
            try:
                if args.concurrency > 1 and not args.dry_run:
                    replay_concurrently(args, received, failed)
                else:
                    for current_row, event in read_events(args.csv_file, args.start_row, args.limit):
                        payload = make_payload(event, args)
                        if args.dry_run:
                            emit({"row": current_row, "request": payload})
                            completed += 1
                            continue
                        if completed and args.interval:
                            time.sleep(args.interval)
                        sent = time.monotonic()
                        try:
                            status, body = post_decision(opener, args, payload)
                        except BaseException:
                            failed()
                            raise
                        received(current_row, event, status, body, (time.monotonic() - sent) * 1000)
            finally:
                replay_elapsed = time.monotonic() - replay_started
            if completed == 0:
                raise ValueError("no data rows selected; check the file and --start-row")
    except urllib.error.HTTPError as error:
        # Do not echo server error bodies: they can contain credentials or inputs.
        row = getattr(args, "failed_row", None) or current_row
        print(f"Stopped at data row {row}: HTTP {error.code}; no retry performed.", file=sys.stderr)
        error.close()
        return 1
    except (OSError, ValueError, RuntimeError, urllib.error.URLError) as error:
        row = getattr(args, "failed_row", None) or current_row
        print(f"Stopped near data row {row}: {error}; no retry performed.", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        if args.concurrency > 1:
            print("Interrupted concurrent replay; inspect completed row IDs and server records before resuming.",
                  file=sys.stderr)
        else:
            print(f"Interrupted near data row {current_row}; check server records before resuming.", file=sys.stderr)
        return 130
    finally:
        signal.signal(signal.SIGTERM, previous_sigterm)
        signal.signal(signal.SIGINT, previous_sigint)
        print(f"{'Previewed' if args.dry_run else 'Completed'} {completed} rows in "
              f"{time.monotonic() - started:.2f}s.", file=sys.stderr)
        if not args.dry_run:
            statistics.print_summary(args.concurrency, replay_elapsed)
    return 0


if __name__ == "__main__":
    sys.exit(main())
