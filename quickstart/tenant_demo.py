#!/usr/bin/env python3
"""Prepare and verify a two-tenant local demo using synthetic Core fixtures."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import shutil
import subprocess
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def fingerprint(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="New directory for private demo configuration")
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        parser.error("output must be a new directory")
    subprocess.run(["cargo", "build", "-p", "corint-decision-cli", "-p", "corint-decision-server", "--locked"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT))
    binaries = Path(metadata["target_directory"]) / "debug"
    output.mkdir(parents=True, mode=0o700)
    tokens = {"TENANT_DEMO_" + name: secrets.token_hex(32) for name in ("ADMIN", "LOCAL", "ACME", "AGENT")}
    limits = {"max_inflight": 16, "requests_per_second": 100, "burst": 100, "max_connections": 32}
    deployments, principals = [], [{"id": "admin", "token_env": "TENANT_DEMO_ADMIN", "platform_admin": True, "grants": []}]
    resources = ["rule.yaml", "ruleset.yaml", "pipeline.yaml", "registry.yaml"]
    for tenant in ("local", "acme"):
        directory = output / tenant
        author = directory / "author"
        author.mkdir(parents=True)
        for name in resources + ["input-schema.yaml"]:
            shutil.copyfile(ROOT / "tests/conformance/cdl_core" / name, author / name)
        for name, fixture in [("context.yaml", "contracts/business-context.yaml"), ("target.json", "contracts/target-capabilities.json"), ("cases.yaml", "cdl_core/behavior.yaml")]:
            shutil.copyfile(ROOT / "tests/conformance" / fixture, directory / name)
        command = [str(binaries / "corint"), "prepare-repository", "--root", str(author), "--input-schema", "input-schema.yaml", "--cases", str(directory / "cases.yaml"), "--context", str(directory / "context.yaml"), "--target", str(directory / "target.json"), "--revision", "tenant-demo-v1", "--output", str(directory / "repository"), "--format", "json", *resources]
        candidate = json.loads(subprocess.check_output(command, cwd=directory))
        scope = {"tenant_id": tenant, "environment": "dev", "deployment": "risk"}
        approval = {"policy_sha256": candidate["candidate"]["policy_sha256"], "tenant_scope": scope, "resource_scope_sha256": fingerprint({"scope": scope, "resources": []})}
        for name, file in [("context", "context.yaml"), ("target", "target.json"), ("cases", "cases.yaml")]:
            approval[name + "_sha256"] = hashlib.sha256((directory / file).read_bytes()).hexdigest()
        journal = {"path": "journal.sqlite", "max_records": 10000, "max_bytes": 100000000, "export_replay": True}
        if tenant != "local":
            journal["tenant_id"] = tenant
        save(directory / "core.json", {"config_version": "3", "listen": "127.0.0.1:0", "repository": "repository", "context": "context.yaml", "target": "target.json", "cases": "cases.yaml", "decision_token_env": "", "publisher_token_env": "", "approvals": [approval], "journal": journal})
        deployments.append({"scope": scope, "root": tenant, "core_config": "core.json", "limits": limits, "timeout_ms": 10000, "idle_seconds": 300, "resources": []})
        grant = {"scope": scope, "permissions": ["decide", "inspect", "publish", "consume", "export", "manage"]}
        principals.append({"id": tenant, "token_env": "TENANT_DEMO_" + tenant.upper(), "grants": [grant]})
        if tenant == "local":
            principals.append({"id": "agent", "token_env": "TENANT_DEMO_AGENT", "delegated_by": "local", "expires_at_ms": int(time.time() * 1000) + 3600000, "grants": [{"scope": scope, "permissions": ["decide"]}]})
    save(output / "credentials.json", {"format_version": "1", "principals": principals})
    save(output / "tenants.json", {"format_version": "1", "listen": "127.0.0.1:0", "credentials": "credentials.json", "control_store": {"type": "sqlite", "path": "control.sqlite"}, "platform_limits": {**limits, "max_inflight": 64, "max_connections": 128}, "tenant_limits": {"local": limits, "acme": limits}, "max_loaded": 8, "max_preparations": 2, "deployments": deployments})
    environment = {**tokens, "CORINT_TENANT_CONFIG": str(output / "tenants.json")}
    credentials = output / "credentials.env"
    fd = os.open(credentials, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "w") as stream:
        stream.write("unset CORINT_CORE_CONFIG\n")
        for key, value in environment.items():
            stream.write(f"export {key}={shlex.quote(value)}\n")
    if args.prepare_only:
        print(json.dumps({"configuration": str(output / "tenants.json"), "credentials_file": str(credentials)}))
        return
    server_env = {**os.environ, **environment, "NO_COLOR": "1", "RUST_LOG": "corint_decision_server=info"}
    server_env.pop("CORINT_CORE_CONFIG", None)
    log_path = output / "server.log"
    with log_path.open("w") as log:
        process = subprocess.Popen([str(binaries / "corint-decision-server")], cwd=output, env=server_env, stdout=log, stderr=log)
    try:
        deadline = time.monotonic() + 30
        while True:
            if process.poll() is not None:
                raise RuntimeError(f"Demo server failed; inspect {log_path}")
            match = re.search(r"Tenant decision host listening on (127\.0\.0\.1:\d+)", log_path.read_text())
            if match:
                url = "http://" + match[1]
                break
            if time.monotonic() >= deadline:
                raise TimeoutError("Demo readiness")
            time.sleep(0.05)
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))

        def call(tenant, actor, route="decide", body=None, method="POST"):
            payload = dict(body or {})
            endpoint = f"/v1/tenants/{tenant}/environments/dev/deployments/risk/{route}"
            if route == "credentials":
                endpoint = "/v1/tenancy/credentials"
            if route == "decide":
                endpoint = "/v1/decide"
                payload["tenant_id"] = tenant
            request = urllib.request.Request(url + endpoint, data=json.dumps(payload).encode(), method=method, headers={"Authorization": "Bearer " + tokens["TENANT_DEMO_" + actor.upper()], "Content-Type": "application/json"})
            try:
                with opener.open(request, timeout=30) as response:
                    return response.status, json.load(response)
            except urllib.error.HTTPError as error:
                return error.code, json.load(error)

        event = {"business_event_id": "same-event", "idempotency_key": "same-key", "event": {"amount": 1001}}
        status, issued = call("local", "admin", "credentials", {
            "action": "create", "id": "demo_client",
            "grants": [{"scope": {"tenant_id": "local", "environment": "dev", "deployment": "risk"}, "permissions": ["decide"]}],
        })
        assert status == 200, "Credential creation failed"
        tokens["TENANT_DEMO_CLIENT"] = issued["token"]
        assert call("local", "client", body=event)[0] == 200
        assert call("acme", "client", body=event)[0] == 403
        status, rotated = call("local", "admin", "credentials", {"action": "rotate", "id": "demo_client"})
        assert status == 200, "Credential rotation failed"
        assert call("local", "client", body=event)[0] == 401
        tokens["TENANT_DEMO_CLIENT"] = rotated["token"]
        assert call("local", "client", body=event)[0] == 200
        assert call("local", "admin", "credentials", {"action": "revoke", "id": "demo_client"})[0] == 200
        assert call("local", "client", body=event)[0] == 401
        local = call("local", "local", body=event)
        acme = call("acme", "acme", body=event)
        assert local[0] == acme[0] == 200, (local, acme)
        assert local[1]["record"]["decision_id"] != acme[1]["record"]["decision_id"]
        assert call("local", "local", body=event) == local
        assert call("acme", "local", body=event)[0] == 403
        assert call("local", "agent", "repo/reload")[0] == 403
        state = call("local", "local", "runtime", method="GET")[1]["state"]
        assert call("local", "local", "runtime", {"expected_revision": state["revision"], "paused": True})[0] == 200
        assert call("local", "local", body=event)[0] == 503
        assert call("acme", "acme", body=event) == acme
        assert call("local", "local", "runtime", {"expected_revision": state["revision"] + 1, "paused": False})[0] == 200
        print(json.dumps({"verified": ["tenant_scope", "idempotency", "cross_tenant_denial", "agent_permissions", "independent_pause", "credential_lifecycle"], "configuration": str(output / "tenants.json"), "credentials_file": str(credentials)}))
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=130)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == "__main__":
    main()
