"""Local SQLite credential and Decision lifecycle for CSV replay.

This is operator-side test setup, not a remote authentication fallback. Only the
managed workspace process and its configured SQLite control store are supported.
The credential store must already have been initialized by Decision.
"""

from contextlib import contextmanager, redirect_stdout
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import secrets
import sqlite3
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]


def services_module(run_dir):
    spec = importlib.util.spec_from_file_location("replay_services", ROOT / "scripts/services.py")
    services = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(services)
    services.STATE = run_dir
    return services


def credential_database(config_path):
    config_path = config_path.resolve(strict=True)
    config = json.loads(config_path.read_text())
    store = config["control_store"]
    if store.get("type") != "sqlite":
        raise ValueError("automatic local replay currently requires a SQLite credential store")
    raw = config_path.parent / store["path"]
    if raw.is_symlink():
        raise ValueError("credential database must not be a symlink")
    database = raw.resolve(strict=True)
    if not database.is_relative_to(config_path.parent) or not database.is_file():
        raise ValueError("credential database must be a regular file within the authentication directory")
    scope = config["scope"]
    if set(scope) != {"tenant_id", "environment", "deployment"} or not all(
            isinstance(value, str) and value for value in scope.values()):
        raise ValueError("invalid authentication scope")
    return database, scope


def change_credential(database, principal_id, entry=None):
    """Update just our principal under a write lock, preserving concurrent changes."""
    try:
        connection = sqlite3.connect(database.as_uri() + "?mode=rw", uri=True, timeout=5)
    except sqlite3.Error as error:
        raise ValueError("could not open the local credential database") from error
    try:
        connection.execute("BEGIN IMMEDIATE")
        row = connection.execute("SELECT revision, document FROM tenant_credentials WHERE id=1").fetchone()
        if row is None:
            raise ValueError("credential database has not been initialized")
        revision, serialized = row
        registry = json.loads(serialized)
        entries = registry["entries"]
        if entry is not None:
            if principal_id in entries or len(entries) >= 2048:
                raise ValueError("cannot register temporary decision credential")
            entries[principal_id] = entry
            action = "create"
        else:
            if principal_id not in entries:
                connection.rollback()
                return
            del entries[principal_id]
            action = "revoke"
        connection.execute("UPDATE tenant_credentials SET revision=revision+1, document=? WHERE id=1 AND revision=?",
                           (json.dumps(registry), revision))
        connection.execute(
            "INSERT INTO tenant_credential_audit(revision,actor,action,principal_id,occurred_at_ms) VALUES(?,?,?,?,?)",
            (revision + 1, "local_csv_replay", action, principal_id, int(time.time() * 1000)))
        connection.commit()
    except sqlite3.Error as error:
        raise ValueError("could not update the local credential database") from error
    finally:
        connection.close()


def verify_repository(command, environment, repository):
    """Reject old binaries that would silently ignore the repository override."""
    try:
        result = subprocess.run(command + ["--repository-info"], cwd=ROOT, env=environment,
                                capture_output=True, text=True, timeout=30)
        reported = json.loads(result.stdout) if result.returncode == 0 else {}
        matches = (reported.get("type") == "filesystem"
                   and Path(reported.get("path", "")).resolve() == repository)
    except (subprocess.TimeoutExpired, ValueError, AttributeError, TypeError):
        matches = False
    if not matches:
        raise ValueError("Decision did not select the test repository; "
                         "run cargo build --locked -p corint-decision-server before replay")


@contextmanager
def local_decision(args):
    """Register -> start/restart -> replay -> stop -> remove -> restore service."""
    services = services_module(args.run_dir)
    args.run_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
    replay_stdout = sys.stdout
    with (args.run_dir / "services.lock").open("a") as lock, redirect_stdout(sys.stderr):
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("another local service operation or replay is running") from None
        original = services.running("decision")
        retained = args.run_dir / "decision.environment.json"
        saved = json.loads(retained.read_text()) if retained.exists() else {}
        command = [str(args.server_bin.resolve())]
        if original:
            details = services.process_details(original["pid"])
            if not details or details[1] != ROOT:
                raise ValueError("managed Decision must run from this workspace")
            command, live = services.process_configuration(original["pid"])
            saved.update(live)
        environment = dict(os.environ, **saved)
        if args.auth_config:
            environment["CORINT_AUTH_CONFIG"] = str(args.auth_config.resolve())
        auth_path = environment.get("CORINT_AUTH_CONFIG")
        if not auth_path:
            raise ValueError("no local database authentication config; specify --auth-config PATH")
        if environment.get("CORINT_CORE_CONFIG") or environment.get("CORINT_TENANT_CONFIG"):
            raise ValueError("automatic local replay supports the compatibility Decision server only")
        if not command or not os.access(command[0], os.X_OK):
            raise ValueError("Decision executable missing; run cargo build --locked -p corint-decision-server")
        database, scope = credential_database(ROOT / auth_path)
        # Do not pass a plaintext test token to the server: database authentication
        # reads its hash directly. Preserve the original server environment separately.
        restore_environment = dict(os.environ, **saved)
        repository = args.repository.resolve(strict=True)
        if not repository.is_dir() or not (repository / "registry.yaml").is_file():
            raise ValueError("test repository must be a directory containing registry.yaml")
        environment["CORINT_REPOSITORY_PATH"] = str(repository)
        # services.start discovers the bound address from this INFO startup line.
        # Keep it visible even when the caller exports RUST_LOG=warn/error.
        for launch_environment in (environment, restore_environment):
            filters = launch_environment.get("RUST_LOG", "warn")
            launch_environment["RUST_LOG"] = filters + ",corint_decision_server=info"
        environment.pop(args.token_env, None)
        verify_repository(command, environment, repository)
        token = secrets.token_hex(32)
        principal_id = "csv_replay_" + uuid.uuid4().hex
        entry = {"principal": {"id": principal_id, "platform_admin": False, "revoked": False,
                               "expires_at_ms": int((time.time() + args.credential_ttl) * 1000),
                               "delegated_by": None,
                               "grants": [{"scope": scope, "permissions": ["decide"]}]},
                 "digest": list(hashlib.sha256(token.encode()).digest())}
        previous_token = os.environ.get(args.token_env)
        previous_url = args.url
        registered = False
        launch_attempted = False
        original_stopped = False
        try:
            registered = True
            change_credential(database, principal_id, entry)
            os.environ[args.token_env] = token
            print(f"Registered temporary decide-only credential in SQLite: {principal_id}", file=sys.stderr)
            if original:
                services.stop("decision")
                original_stopped = True
            launch_attempted = True
            print(f"Test policy repository: {repository}", file=sys.stderr)
            services.start("decision", command, environment)
            args.url = services.running("decision")["url"].rstrip("/") + "/v1/decide"
            print(f"Replay endpoint: {args.url}", file=sys.stderr)
            # Keep JSONL replay results on stdout; service lifecycle messages go to stderr.
            with redirect_stdout(replay_stdout):
                yield
        finally:
            try:
                if launch_attempted:
                    services.stop("decision")
            finally:
                try:
                    if registered:
                        change_credential(database, principal_id)
                        print(f"Removed temporary decision credential: {principal_id}", file=sys.stderr)
                finally:
                    args.url = previous_url
                    if previous_token is None:
                        os.environ.pop(args.token_env, None)
                    else:
                        os.environ[args.token_env] = previous_token
                    token = None
                    if original_stopped:
                        services.start("decision", command, restore_environment)
                        print("Restored the original Decision service configuration.", file=sys.stderr)
