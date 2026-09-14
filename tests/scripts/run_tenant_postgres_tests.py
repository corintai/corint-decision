#!/usr/bin/env python3
"""Verify tenant isolation against a disposable, Unix-socket-only PostgreSQL cluster."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
for command in ("initdb", "pg_ctl", "cargo"):
    if shutil.which(command) is None:
        raise SystemExit(f"Required executable not found: {command}")
with tempfile.TemporaryDirectory(prefix="corint-tenant-pg-") as temporary:
    directory = Path(temporary)
    data = directory / "data"
    subprocess.run(["initdb", "-D", str(data), "-U", "corint_test", "--auth-local=trust", "--auth-host=reject", "--no-locale", "--encoding=UTF8"], check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["pg_ctl", "-D", str(data), "-l", str(directory / "postgres.log"), "-o", f"-k {directory} -c listen_addresses=''", "-w", "start"], check=True)
    try:
        environment = dict(os.environ, CORINT_TEST_POSTGRES_URL=f"postgresql://corint_test@localhost/postgres?host={directory}")
        subprocess.run(["cargo", "test", "-p", "corint-decision-server", "--test", "tenancy", "--test", "journal_backends", "--test", "core_activation", "--locked", "postgres_", "--", "--ignored"], cwd=root, env=environment, check=True)
    finally:
        subprocess.run(["pg_ctl", "-D", str(data), "-m", "fast", "-w", "stop"], check=True)
