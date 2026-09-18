#!/usr/bin/env python3
"""Exercise the launch scripts with isolated real listener subprocesses."""
import json
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("services", ROOT / "scripts/services.py")
SERVICES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SERVICES)
STUB = '''#!/usr/bin/env python3
import json, os, signal, socket, sys, time
from pathlib import Path
name = "mcp" if Path(sys.argv[0]).name == "corint-mcp" else "decision"
if "--repository-info" in sys.argv:
    if os.environ.get("TEST_UNSUPPORTED_REPOSITORY"):
        print("Automatic MCP discovery does not yet support database/API repositories", file=sys.stderr)
        sys.exit(1)
    print(json.dumps({"type": "filesystem", "path": str(Path(__file__).parent)}))
    sys.exit(0)
Path(os.environ["CORINT_RUN_DIR"], name + ".args.json").write_text(json.dumps(sys.argv[1:]))
if name == "mcp" and os.environ.get("TEST_FAIL_MCP"):
    sys.exit(3)
listener = socket.socket()
listener.bind(("127.0.0.1", 0))
listener.listen()
label = "MCP Server" if name == "mcp" else "HTTP Server"
print(f"{label} listening on http://127.0.0.1:{listener.getsockname()[1]}", flush=True)
signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
while True:
    time.sleep(.1)
'''


class ServicesTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="corint-services-")
        self.directory = Path(self.temporary.name)
        self.state = self.directory / "run"
        target = self.directory / "target"
        binaries = target / "debug"
        binaries.mkdir(parents=True)
        for name in ("corint-decision-server", "corint-mcp"):
            path = binaries / name
            path.write_text(STUB)
            path.chmod(0o755)
        self.environment = dict(os.environ, CORINT_RUN_DIR=str(self.state),
                                CARGO_TARGET_DIR=str(target), CORINT_START_TIMEOUT="4",
                                CORINT_STOP_TIMEOUT="4")

    def tearDown(self):
        self.run_script("stop", check=False)
        self.temporary.cleanup()

    def run_script(self, name, check=True):
        return subprocess.run([str(ROOT / "scripts" / f"{name}.sh"), "--no-build"],
                              cwd=self.directory, env=self.environment, text=True,
                              capture_output=True, check=check, timeout=20)

    def pids(self):
        return {name: json.loads((self.state / f"{name}.pid.json").read_text())["pid"]
                for name in ("decision", "mcp")}

    def test_start_is_idempotent_restart_replaces_both_and_stop_cleans_up(self):
        self.run_script("start")
        before = self.pids()
        self.run_script("start")
        self.assertEqual(before, self.pids())
        self.run_script("restart")
        after = self.pids()
        self.assertTrue(all(before[name] != after[name] for name in before))
        self.run_script("stop")
        self.assertFalse(list(self.state.glob("*.pid.json")))

    def test_failed_second_service_rolls_back_first(self):
        self.environment["TEST_FAIL_MCP"] = "1"
        result = self.run_script("start", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mcp", result.stderr)
        self.assertFalse(list(self.state.glob("*.pid.json")))

    def test_default_uses_discovered_repository_and_explicit_catalog_still_works(self):
        self.run_script("start")
        arguments = json.loads((self.state / "mcp.args.json").read_text())
        self.assertEqual(arguments[:2], ["--repository", str((self.directory / "target/debug").resolve())])
        self.environment["CORINT_MCP_CONFIG"] = str(ROOT / "tests/fixtures/mcp/catalog.json")
        self.run_script("restart")
        arguments = json.loads((self.state / "mcp.args.json").read_text())
        self.assertEqual(arguments[:2], ["--config", str(ROOT / "tests/fixtures/mcp/catalog.json")])

    def test_unsupported_repository_fails_before_stopping_running_services(self):
        self.run_script("start")
        before = self.pids()
        self.environment["TEST_UNSUPPORTED_REPOSITORY"] = "1"
        result = self.run_script("restart", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("database/API", result.stderr)
        self.assertEqual(before, self.pids())
        for pid in before.values():
            os.kill(pid, 0)

    def test_stale_pid_never_signals_an_unrelated_process(self):
        self.state.mkdir()
        (self.state / "decision.pid.json").write_text(json.dumps({
            "pid": os.getpid(), "identity": "not this process", "url": "unused"
        }))
        self.run_script("stop")
        self.assertFalse((self.state / "decision.pid.json").exists())

    def test_existing_decision_survives_mcp_start_failure(self):
        self.run_script("start")
        before = self.pids()
        os.kill(before["mcp"], 15)
        # The helper detects a stale record, starts just MCP and leaves Decision
        # running when the replacement MCP fails.
        import time
        time.sleep(0.3)
        self.environment["TEST_FAIL_MCP"] = "1"
        result = self.run_script("start", check=False)
        self.assertNotEqual(result.returncode, 0)
        state = json.loads((self.state / "decision.pid.json").read_text())
        self.assertEqual(state["pid"], before["decision"])
        os.kill(before["decision"], 0)

    def test_legacy_recovery_retains_auth_configuration_without_pid_file(self):
        self.state.mkdir()
        auth = {"CORINT_AUTH_CONFIG": "/workspace/auth.json", "CORINT_TENANT_ID": "demo"}
        with patch.object(SERVICES, "STATE", self.state), \
             patch.object(SERVICES, "workspace_instances", return_value=[12345]), \
             patch.object(SERVICES, "identity", return_value="verified birth and executable"), \
             patch.object(SERVICES, "process_configuration", return_value=(["old-release"], auth)):
            SERVICES.recover_legacy("decision", self.directory / "target", ["new-debug"])
        state = json.loads((self.state / "decision.pid.json").read_text())
        self.assertEqual(state["pid"], 12345)
        path = self.state / "decision.environment.json"
        self.assertEqual(json.loads(path.read_text()), auth)
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        # Don't let tearDown signal the fake PID even if it happens to exist.
        (self.state / "decision.pid.json").unlink()

    def test_discovery_excludes_other_workspaces_users_and_executables(self):
        target = self.directory / "target"
        binary = (target / "release/corint-decision-server").resolve()
        uid = os.getuid()
        output = f"11 {uid} {binary}\n12 {uid} {binary}\n13 {uid+1} {binary}\n14 {uid} {binary}\n"
        details = {11: (binary, ROOT), 12: (binary, self.directory),
                   14: (Path("/other/corint-decision-server"), ROOT)}
        with patch.object(SERVICES.subprocess, "run", return_value=SimpleNamespace(stdout=output)), \
             patch.object(SERVICES, "process_details", side_effect=details.get):
            self.assertEqual(SERVICES.workspace_instances("decision", target), [11])

    def test_ambiguous_legacy_instances_are_not_adopted(self):
        self.state.mkdir()
        with patch.object(SERVICES, "STATE", self.state), \
             patch.object(SERVICES, "workspace_instances", return_value=[12345, 12346]):
            with self.assertRaisesRegex(RuntimeError, "multiple workspace instances"):
                SERVICES.recover_legacy("decision", self.directory / "target", ["binary"])
        self.assertFalse(list(self.state.glob("*.pid.json")))


if __name__ == "__main__":
    unittest.main()
