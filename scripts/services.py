#!/usr/bin/env python3
"""Local process lifecycle for Decision + HTTP MCP (macOS/Linux, stdlib only)."""
import argparse
import ctypes
import fcntl
import json
import os
from pathlib import Path
import re
import secrets
import signal
import socket
import struct
import subprocess
import sys
import time
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]
STATE = Path(os.environ.get("CORINT_RUN_DIR", ROOT / ".run")).resolve()
START_TIMEOUT = float(os.environ.get("CORINT_START_TIMEOUT", "60"))
STOP_TIMEOUT = float(os.environ.get("CORINT_STOP_TIMEOUT", "180"))
NAMES = ("decision", "mcp")
RUNNER_ENV = {"CORINT_RUN_DIR", "CORINT_START_TIMEOUT", "CORINT_STOP_TIMEOUT",
              "CORINT_MCP_CONFIG", "CORINT_MCP_LISTEN"}


def process_details(pid):
    """Read executable and cwd without inspecting command-line secrets."""
    if sys.platform == "linux":
        base = Path("/proc") / str(pid)
        try:
            # Cargo may have atomically replaced the on-disk executable. Linux
            # then appends ' (deleted)' to the still-running inode's path.
            executable = os.readlink(base / "exe").removesuffix(" (deleted)")
            return Path(executable).resolve(), (base / "cwd").resolve(strict=True)
        except OSError:
            return None
    result = subprocess.run(["lsof", "-a", "-p", str(pid), "-d", "cwd,txt", "-Fn"],
                            capture_output=True, text=True, check=False)
    cwd, executable, descriptor = None, None, None
    for line in result.stdout.splitlines():
        if line.startswith("f"):
            descriptor = line[1:]
        elif line.startswith("n"):
            if descriptor == "cwd":
                cwd = Path(line[1:]).resolve()
            elif descriptor == "txt" and executable is None:
                executable = Path(line[1:]).resolve()
    return (executable, cwd) if executable and cwd else None


def process_configuration(pid):
    """Read argv/environment as NUL-delimited OS data; never log their contents."""
    if sys.platform == "linux":
        base = Path("/proc") / str(pid)
        arguments = (base / "cmdline").read_bytes().split(b"\0")[:-1]
        environment = (base / "environ").read_bytes().split(b"\0")
    elif sys.platform == "darwin":
        libc = ctypes.CDLL(None, use_errno=True)
        mib = (ctypes.c_int * 3)(1, 49, pid)  # CTL_KERN, KERN_PROCARGS2
        size = ctypes.c_size_t()
        if libc.sysctl(mib, 3, None, ctypes.byref(size), None, 0):
            raise OSError(ctypes.get_errno(), "Cannot read existing service configuration")
        buffer = ctypes.create_string_buffer(size.value)
        if libc.sysctl(mib, 3, buffer, ctypes.byref(size), None, 0):
            raise OSError(ctypes.get_errno(), "Cannot read existing service configuration")
        data = buffer.raw[:size.value]
        argc = struct.unpack_from("i", data)[0]
        position = data.index(b"\0", 4) + 1  # executable path + alignment padding
        while data[position] == 0:
            position += 1
        arguments = []
        for _ in range(argc):
            end = data.index(b"\0", position)
            arguments.append(data[position:end])
            position = end + 1
        environment = data[position:].split(b"\0")
    else:
        raise RuntimeError("Legacy service discovery requires macOS or Linux")
    values = {}
    for item in environment:
        if b"=" not in item:
            continue
        key, value = (os.fsdecode(part) for part in item.split(b"=", 1))
        # Retain runtime configuration, not unrelated session secrets or demo
        # operator credentials that the server itself does not consume.
        if (key.startswith("CORINT_") and key not in RUNNER_ENV
                and not key.startswith("CORINT_DEMO_")) or key == "DATABASE_URL":
            values[key] = value
    return [os.fsdecode(value) for value in arguments], values


def workspace_instances(name, target):
    binary = "corint-decision-server" if name == "decision" else "corint-mcp"
    allowed = {(target / profile / binary).resolve() for profile in ("debug", "release")}
    processes = subprocess.run(["ps", "-ww", "-ax", "-o", "pid=", "-o", "uid=", "-o", "comm="],
                               capture_output=True, text=True, check=True)
    matches = []
    for line in processes.stdout.splitlines():
        fields = line.split(None, 2)
        if len(fields) != 3 or fields[1] != str(os.getuid()) or Path(fields[2]).name != binary:
            continue
        pid = int(fields[0])
        details = process_details(pid)
        if details and details[0] in allowed and details[1] == ROOT:
            matches.append(pid)
    return matches


def recover_legacy(name, target, command):
    """Adopt only a same-user, exact workspace executable with the same cwd."""
    current = running(name)
    candidates = [pid for pid in workspace_instances(name, target)
                  if not current or pid != current["pid"]]
    if not candidates:
        return
    if current or len(candidates) != 1:
        raise RuntimeError(f"{name}: multiple workspace instances found; resolve them before restarting")
    pid = candidates[0]
    birth = identity(pid)
    if birth is None:
        return
    arguments, environment = process_configuration(pid)
    if (name == "decision" and len(arguments) != 1) or (
            name == "mcp" and arguments[1:] != command[1:]):
        raise RuntimeError(f"{name}: existing PID {pid} uses different launch arguments; not adopted")
    if identity(pid) != birth:
        raise RuntimeError(f"{name}: existing process changed during discovery; retry")
    path = STATE / f"{name}.environment.json"
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(environment))
    temporary.chmod(0o600)
    temporary.replace(path)
    save(name, {"pid": pid, "identity": birth, "url": "existing workspace instance"})
    print(f"{name}: recovered existing workspace PID {pid}; runtime configuration retained", flush=True)


def identity(pid):
    """Match birth time and executable as well as PID before signaling a process."""
    result = subprocess.run(
        ["ps", "-p", str(pid), "-o", "lstart=", "-o", "comm=", "-o", "stat="],
        capture_output=True, text=True, check=False,
    )
    value = result.stdout.strip()
    if result.returncode or not value or value.split()[-1].startswith("Z"):
        return None
    # Running/sleeping state changes; exclude it from the identity.
    return value.rsplit(None, 1)[0]


def state_path(name):
    return STATE / f"{name}.pid.json"


def running(name):
    path = state_path(name)
    if not path.exists():
        return None
    state = json.loads(path.read_text())
    if identity(state["pid"]) == state["identity"]:
        return state
    path.unlink()  # Stale PID: never signal an unrelated process.
    return None


def save(name, state):
    temporary = STATE / f"{name}.pid.tmp"
    temporary.write_text(json.dumps(state))
    temporary.replace(state_path(name))


def stop(name):
    state = running(name)
    if not state:
        print(f"{name}: no managed process", flush=True)
        return
    os.kill(state["pid"], signal.SIGTERM)
    deadline = time.monotonic() + STOP_TIMEOUT
    while identity(state["pid"]) == state["identity"]:
        if time.monotonic() >= deadline:
            raise RuntimeError(f"{name}: graceful stop timed out; process retained, no SIGKILL sent")
        time.sleep(0.2)
    state_path(name).unlink(missing_ok=True)
    print(f"{name}: stopped", flush=True)


def credentials(environment):
    # Preserve existing operator auth and .env behavior. Only bootstrap a fresh
    # local compatibility launch with neither credentials nor an auth config.
    names = ("CORINT_DECISION_TOKEN", "CORINT_PUBLISHER_TOKEN")
    configured = ("CORINT_AUTH_CONFIG", "CORINT_CORE_CONFIG", "CORINT_TENANT_CONFIG", *names)
    if any(environment.get(key) for key in configured) or (ROOT / ".env").exists():
        return
    path = STATE / "local-credentials.json"
    if not path.exists():
        with path.open("x") as output:
            json.dump({name: secrets.token_hex(32) for name in names}, output)
        path.chmod(0o600)
    environment.update(json.loads(path.read_text()))
    print(f"Local decision credentials: {path} (reused on restart)", flush=True)


def wait_ready(name, process, log_path, offset):
    deadline = time.monotonic() + START_TIMEOUT
    pattern = re.compile(
        r"(?:HTTP Server|Experimental strict Core server|Tenant decision host|MCP Server) "
        r"listening on (?:http://)?([^\s]+)"
    )
    # Read the actual bound address, so custom compatibility/Core/tenant ports
    # work without maintaining a second parser for their configuration formats.
    with log_path.open() as log:
        log.seek(offset)
        pending = ""
        while time.monotonic() < deadline:
            if process.poll() is not None:
                # Surface the actual startup error, rather than a misleading
                # 'stopped' message followed only by a log path.
                pending += log.read()
                clean = re.sub(r"\x1b\[[0-9;]*m", "", pending)
                errors = [line for line in clean.splitlines() if line.startswith("Error:")]
                detail = f" ({errors[-1]})" if errors else ""
                raise RuntimeError(f"{name} exited with {process.returncode}{detail}; see {log_path}")
            pending = (pending + log.read())[-65536:]
            clean = re.sub(r"\x1b\[[0-9;]*m", "", pending)
            match = pattern.search(clean)
            if match:
                url = "http://" + match[1]
                address = urlsplit(url)
                try:
                    with socket.create_connection((address.hostname, address.port), timeout=0.5):
                        if process.poll() is None:
                            return url
                except OSError:
                    pass
            time.sleep(0.1)
    raise RuntimeError(f"{name} did not become ready in {START_TIMEOUT:g}s; see {log_path}")


def start(name, command, environment):
    previous = running(name)
    if previous:
        print(f"{name}: already running (PID {previous['pid']}) at {previous['url']}", flush=True)
        return False
    log_path = STATE / f"{name}.log"
    with log_path.open("ab") as log:
        offset = log.tell()
        process = subprocess.Popen(
            command, cwd=ROOT, env=environment, stdin=subprocess.DEVNULL,
            stdout=log, stderr=log, start_new_session=True,
        )
    state = {"pid": process.pid, "identity": identity(process.pid), "url": "starting"}
    if state["identity"] is None:
        process.wait()
        raise RuntimeError(f"{name} exited during startup; see {log_path}")
    save(name, state)
    try:
        state["url"] = wait_ready(name, process, log_path, offset)
        # A shebang launcher may exec its interpreter after Popen returns.
        # Record the final executable identity once the listener is ready.
        state["identity"] = identity(process.pid)
        if state["identity"] is None:
            raise RuntimeError(f"{name} exited during readiness; see {log_path}")
        save(name, state)
    except BaseException:
        # This is still our unreaped child, so terminate it directly even if a
        # shebang exec changed its command between launch and readiness.
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=STOP_TIMEOUT)
        state_path(name).unlink(missing_ok=True)
        raise
    suffix = "/mcp" if name == "mcp" else ""
    print(f"{name}: ready (PID {process.pid}) at {state['url']}{suffix}", flush=True)
    print(f"  log: {log_path}", flush=True)
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("start", "restart", "stop"))
    parser.add_argument("--no-build", action="store_true", help="Use existing binaries")
    parser.add_argument("--release", action="store_true", help="Build/use release binaries")
    parser.add_argument("--only", choices=NAMES, help="Manage just one service (default: both)")
    args = parser.parse_args()
    names = (args.only,) if args.only else NAMES
    os.umask(0o077)
    STATE.mkdir(parents=True, exist_ok=True, mode=0o700)
    # Kernel lock releases on errors/signals; no stale lock-directory cleanup.
    with (STATE / "services.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError("Another start/restart/stop operation is in progress")
        if args.action == "stop":
            for name in reversed(names):
                stop(name)
            return
        if args.action == "start" and all(running(name) for name in names):
            for name in names:
                state = running(name)
                print(f"{name}: already running (PID {state['pid']}) at {state['url']}")
            return
        environment = os.environ.copy()
        # Keep listener announcements available for readiness detection.
        environment["RUST_LOG"] = environment.get("RUST_LOG", "info") + ",corint_decision_server=info"
        default_config = ROOT / "config/mcp.json"
        configured = environment.get("CORINT_MCP_CONFIG")
        mcp_config = Path(configured).resolve() if configured else (
            default_config if default_config.exists() else None)
        if "mcp" in names and mcp_config is not None and not mcp_config.is_file():
            raise RuntimeError(f"MCP configuration not found: {mcp_config}")
        automatic = "mcp" in names and mcp_config is None
        target = Path(environment.get("CARGO_TARGET_DIR", ROOT / "target"))
        binaries = target / ("release" if args.release else "debug")
        commands = {
            "decision": [str((binaries / "corint-decision-server").resolve())],
            "mcp": [str((binaries / "corint-mcp").resolve())],
        }
        build_names = tuple(dict.fromkeys((*names, "decision"))) if automatic else names
        # Build before stopping existing services, so compilation failure leaves
        # the running pair untouched.
        if not args.no_build:
            command = ["cargo", "build", "--locked"]
            for name in build_names:
                command.extend(["-p", "corint-decision-server" if name == "decision" else "corint-decision-mcp"])
            if args.release:
                command.append("--release")
            subprocess.run(command, cwd=ROOT, check=True)
        for name in build_names:
            command = commands[name]
            if not os.access(command[0], os.X_OK):
                raise RuntimeError(f"Executable missing: {command[0]}; run without --no-build")
        if args.action == "restart" and "decision" in names:
            recover_legacy("decision", target, commands["decision"])
        environments = {}
        for name in NAMES:
            retained = STATE / f"{name}.environment.json"
            previous = json.loads(retained.read_text()) if retained.exists() else {}
            state = running(name)
            if state:
                _, live_environment = process_configuration(state["pid"])
                previous.update(live_environment)
            environments[name] = dict(previous, **environment)
        if "mcp" in names:
            if automatic:
                info = subprocess.run(commands["decision"] + ["--repository-info"],
                                      cwd=ROOT, env=environments["decision"],
                                      capture_output=True, text=True, timeout=30)
                if info.returncode:
                    raise RuntimeError(f"Cannot discover Decision repository: {info.stderr.strip()}")
                repository = json.loads(info.stdout)
                if repository.get("type") != "filesystem":
                    raise RuntimeError("Automatic MCP discovery requires a filesystem repository")
                path = Path(repository["path"]).resolve()
                if not path.is_dir():
                    raise RuntimeError(f"Policy repository not found: {path}")
                commands["mcp"].extend(["--repository", str(path)])
                print(f"MCP policy repository: {path}")
            else:
                commands["mcp"].extend(["--config", str(mcp_config)])
            commands["mcp"].extend(["--http-listen", environment.get("CORINT_MCP_LISTEN", "127.0.0.1:8082")])
            if args.action == "restart":
                recover_legacy("mcp", target, commands["mcp"])
        if "decision" in names:
            credentials(environments["decision"])
        if args.action == "restart":
            for name in reversed(names):
                stop(name)
        started = []
        try:
            for name in names:
                if start(name, commands[name], environments[name]):
                    started.append(name)
        except BaseException:
            for name in reversed(started):
                stop(name)
            raise


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Error: {error}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)
