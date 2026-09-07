#!/usr/bin/env python3
"""Measure real loopback HTTP, including concurrent repository reload and v3 journal."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import secrets
import shutil
import sqlite3
import subprocess
import tempfile
import time
import urllib.request

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--samples", type=int, default=200)
parser.add_argument("--profile", choices=["debug", "release"], default="release")
args = parser.parse_args()
if args.output.exists() or not 100 <= args.samples <= 100000:
    parser.error("output must be new; samples must be 100–100000")
build = ["cargo", "build", "-p", "corint-decision-cli", "-p", "corint-decision-server", "--locked", "--offline"]
if args.profile == "release":
    build.append("--release")
subprocess.run(build, cwd=root, check=True)
binary = root / "target" / args.profile
measurements = []
with tempfile.TemporaryDirectory(prefix="corint-http-bench-") as directory:
    directory = Path(directory)
    author = directory / "author"
    author.mkdir()
    resources = ["rule.yaml", "ruleset.yaml", "pipeline.yaml", "registry.yaml"]
    fixtures = root / "tests/conformance"
    for name in resources + ["input-schema.yaml"]:
        shutil.copyfile(fixtures / "cdl_core" / name, author / name)
    for name, source in [("cases.yaml", "cdl_core/behavior.yaml"), ("context.yaml", "contracts/business-context.yaml"), ("target.json", "contracts/target-capabilities.json")]:
        shutil.copyfile(fixtures / source, directory / name)
    command = [str(binary / "corint"), "prepare-repository", "--root", str(author), "--input-schema", "input-schema.yaml", "--cases", str(directory / "cases.yaml"), "--context", str(directory / "context.yaml"), "--target", str(directory / "target.json"), "--revision", "synthetic-benchmark-v1", "--output", str(directory / "repository"), "--format", "json", *resources]
    candidate = json.loads(subprocess.check_output(command, cwd=directory))
    approval = {"policy_sha256": candidate["candidate"]["policy_sha256"]}
    for key, name in [("cases", "cases.yaml"), ("context", "context.yaml"), ("target", "target.json")]:
        approval[f"{key}_sha256"] = hashlib.sha256((directory / name).read_bytes()).hexdigest()
    tokens = {name: secrets.token_hex(32) for name in ("BENCH_DECISION", "BENCH_PUBLISHER", "BENCH_CONSUMER")}
    config = {"config_version": "3", "listen": "127.0.0.1:0", "repository": "repository", "context": "context.yaml", "target": "target.json", "cases": "cases.yaml", "decision_token_env": "BENCH_DECISION", "publisher_token_env": "BENCH_PUBLISHER", "approvals": [approval], "journal": {"path": "journal.sqlite", "tenant_id": "benchmark", "max_records": min(100000, args.samples + 100), "max_bytes": 1000000000, "consumer_token_env": "BENCH_CONSUMER"}}
    config_path = directory / "core.json"
    config_path.write_text(json.dumps(config))
    log_path = directory / "server.log"
    with log_path.open("w") as log:
        process = subprocess.Popen([str(binary / "corint-decision-server")], cwd=directory, stdout=log, stderr=log, env={**os.environ, **tokens, "CORINT_CORE_CONFIG": str(config_path), "NO_COLOR": "1", "RUST_LOG": "corint_decision_server=info"})
    try:
        deadline = time.monotonic() + 60
        while True:
            if process.poll() is not None:
                raise RuntimeError("Benchmark server exited before readiness")
            match = re.search(r"Experimental strict Core server listening on (127\.0\.0\.1:\d+)", log_path.read_text())
            if match:
                url = "http://" + match[1]
                break
            if time.monotonic() > deadline:
                raise TimeoutError("Benchmark server readiness")
            time.sleep(0.05)

        def call(path, data=None, publisher=False):
            token = tokens["BENCH_PUBLISHER" if publisher else "BENCH_DECISION"]
            request = urllib.request.Request(url + path, data=None if data is None else json.dumps(data).encode(), headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
            # No system proxies; only the private server created above is contacted.
            with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=30) as response:
                return json.load(response)

        def reload_repository():
            state = call("/v1/core/target", publisher=True)
            start = time.perf_counter()
            result = call("/v1/core/repo/reload", {"expected_revision": state["revision"]}, publisher=True)
            assert result["repository"]["revision"] == "synthetic-benchmark-v1"
            return (time.perf_counter() - start) * 1000

        def decide(trace):
            start = time.perf_counter()
            result = call("/v1/core/decide", {"business_event_id": "bench-" + secrets.token_hex(12), "event": {"amount": 1001}, "enable_trace": trace})
            assert result["decision"]["result"]["score"] == 60
            assert result["snapshot"]["repository"]["revision"] == "synthetic-benchmark-v1"
            return (time.perf_counter() - start) * 1000

        for trace in (False, True):
            for concurrency in (1, 4, 16):
                for reloading in (False, True):
                    for _ in range(10):
                        decide(trace)
                    # Only this harness-owned disposable database is reset.
                    # Drain all previous requests before fixing the next workload's history.
                    with sqlite3.connect(directory / "journal.sqlite") as journal:
                        journal.execute("DELETE FROM events")
                    with ThreadPoolExecutor(max_workers=concurrency) as executor, ThreadPoolExecutor(max_workers=1) as control:
                        reload_job = control.submit(reload_repository) if reloading else None
                        start = time.perf_counter()
                        times = sorted(executor.map(decide, [trace] * args.samples))
                        seconds = time.perf_counter() - start
                        reload_ms = reload_job.result() if reload_job else None
                    measurements.append({"trace": trace, "concurrency": concurrency, "samples": args.samples, "initial_journal_records": 0, "during_reload": reloading, "reload_ms": reload_ms, "p95_ms": times[math.ceil(len(times) * .95) - 1], "p99_ms": times[math.ceil(len(times) * .99) - 1], "requests_per_second": args.samples / seconds})
    finally:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
report = {"format_version": "1", "scope": "synthetic_loopback_http_v3_journal", "build_mode": args.profile, "binary_sha256": hashlib.sha256((binary / "corint-decision-server").read_bytes()).hexdigest(), "host": platform.platform(), "measurements": measurements, "live_work_integration": False, "multi_node": False}
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open("x") as output:
    json.dump(report, output, indent=2)
    output.write("\n")
print(f"Measured {len(measurements)} real HTTP workloads: {args.output}")
