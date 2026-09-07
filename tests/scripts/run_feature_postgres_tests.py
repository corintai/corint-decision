#!/usr/bin/env python3
"""Run PostgreSQL Feature → Core integration tests in a private, disposable Unix-socket cluster."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
for command in ("initdb", "pg_ctl", "cargo"):
    if shutil.which(command) is None:
        raise SystemExit(f"Required executable not found: {command}")
with tempfile.TemporaryDirectory(prefix="corint-feature-pg-") as temporary:
    directory = Path(temporary)
    data = directory / "data"
    subprocess.run(["initdb", "-D", str(data), "-U", "corint_test", "--auth-local=trust", "--auth-host=reject", "--no-locale", "--encoding=UTF8"], check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["pg_ctl", "-D", str(data), "-l", str(directory / "postgres.log"), "-o", f"-k {directory} -c listen_addresses=''", "-w", "start"], check=True)
    try:
        environment = dict(os.environ, CARGO_INCREMENTAL="0", RUSTC_WRAPPER="", CORINT_TEST_POSTGRES_URL=f"postgresql://corint_test@localhost/postgres?host={directory}")
        subprocess.run(["cargo", "nextest", "run", "-p", "corint-decision-engine", "--test", "feature_pipeline", "--all-features", "--locked", "--offline", "--run-ignored", "ignored-only", "-E", "test(postgres_feature_to_core_matches_offline_oracle)"], cwd=root, env=environment, check=True)
    finally:
        subprocess.run(["pg_ctl", "-D", str(data), "-m", "fast", "-w", "stop"], check=True)
