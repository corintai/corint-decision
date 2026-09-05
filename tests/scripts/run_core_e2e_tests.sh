#!/usr/bin/env bash
# Build the real server, then test it with Cargo's freshly built CLI binary.
set -euo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
build_args=(--locked)
build_only=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    --offline) build_args+=(--offline) ;;
    --build-only) build_only=true ;;
    *) echo "Usage: bash tests/scripts/run_core_e2e_tests.sh [--offline] [--build-only]" >&2; exit 2 ;;
  esac
  shift
done
command -v python3 >/dev/null
# Read Cargo's artifact path instead of assuming target/debug or searching PATH.
# pipefail preserves build failures. No recursive Cargo call occurs inside tests.
server_binary=$(cargo build -p corint-decision-server --bin corint-decision-server \
  "${build_args[@]}" --message-format=json | python3 -c '
import json, sys
paths = []
for line in sys.stdin:
    item = json.loads(line)
    if item.get("reason") == "compiler-message":
        sys.stderr.write(item.get("message", {}).get("rendered") or "")
    if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "corint-decision-server" and item.get("executable"):
        paths.append(item["executable"])
if len(paths) != 1:
    sys.exit("Expected exactly one server executable from Cargo")
print(paths[0])
')
export CORINT_E2E_SERVER="$server_binary"
if [ "$build_only" = true ]; then
  # For workspace --all-features/coverage: stdout is only the artifact path.
  printf '%s\n' "$server_binary"
  exit 0
fi
if cargo nextest --version >/dev/null 2>&1; then
  cargo nextest run -p corint-decision-cli --features process-e2e \
    --test core_process_e2e "${build_args[@]}" --no-fail-fast --test-threads 2
else
  cargo test -p corint-decision-cli --features process-e2e \
    --test core_process_e2e "${build_args[@]}" -- --test-threads=2
fi
