#!/usr/bin/env python3
"""Build a release benchmark, capture machine-readable latency/throughput and peak RSS."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import time

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--samples", type=int, default=1000)
parser.add_argument("--baseline", type=Path)
parser.add_argument("--max-regression", type=float, default=0.25)
args = parser.parse_args()
if not 100 <= args.samples <= 100000 or args.max_regression < 0:
    parser.error("samples must be 100–100000 and regression must be nonnegative")
if args.output.exists():
    parser.error("output must be a new file")
subprocess.run(["cargo", "build", "-p", "corint-decision-engine", "--example", "core_benchmark", "--release", "--locked", "--offline"], cwd=root, check=True)
binary = root / "target/release/examples/core_benchmark"
with tempfile.TemporaryFile(mode="w+") as output:
    process = subprocess.Popen([str(binary), str(args.samples)], cwd=root, stdout=output)
    _, status, usage = os.wait4(process.pid, 0)
    process.returncode = os.waitstatus_to_exitcode(status)
    if process.returncode:
        raise SystemExit(process.returncode)
    output.seek(0)
    report = json.load(output)
report.update({"binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "host": platform.platform(), "cpu": platform.machine(), "logical_cpus": os.cpu_count(), "peak_rss_bytes": usage.ru_maxrss * (1 if sys.platform == "darwin" else 1024), "measured_at_unix": int(time.time()), "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(), "dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root)), "regressions": []})
if args.baseline:
    baseline = json.loads(args.baseline.read_text())
    for key in ("scope", "build_mode", "host", "logical_cpus", "worker_threads"):
        if report[key] != baseline[key]:
            raise SystemExit(f"Cannot compare different {key}")
    keyed = {(v["rules"], v["concurrency"], v["trace"]): v for v in baseline["measurements"]}
    for row in report["measurements"]:
        previous = keyed.get((row["rules"], row["concurrency"], row["trace"]))
        if previous is None or previous["samples"] != row["samples"]:
            raise SystemExit("Baseline workload differs")
        for metric in ("p95_us", "p99_us"):
            if row[metric] > previous[metric] * (1 + args.max_regression):
                report["regressions"].append({"rules": row["rules"], "trace": row["trace"], "concurrency": row["concurrency"], "metric": metric, "before": previous[metric], "after": row[metric]})
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open("x") as output:
    json.dump(report, output, indent=2)
    output.write("\n")
print(f"Measured {len(report['measurements'])} workloads; peak RSS {report['peak_rss_bytes']} bytes; report: {args.output}")
raise SystemExit(1 if report["regressions"] else 0)
