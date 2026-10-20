#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
code_bin=$(command -v code) || { echo 'VS Code is required' >&2; exit 1; }
command -v xvfb-run >/dev/null || { echo 'xvfb-run is required' >&2; exit 1; }
cargo build --manifest-path "$root/Cargo.toml" --locked --quiet
LYAPUNOV_TEST_CODE_BIN="$code_bin" xvfb-run -a python3 "$root/tests/e2e/vscode_check.py" "$root"
