#!/usr/bin/env bash
# Capture and resume on GNOME Shell (headless, virtual monitor) with the
# Lyapunov extension, in a Fedora container. Needs Docker or Podman.
set -euo pipefail
cd "$(dirname "$0")/.."
engine=$(command -v docker || command -v podman) || { echo "Docker or Podman is required" >&2; exit 2; }
cargo build --release --locked
"$engine" build -q -t lyapunov-test-gnome -f tests/e2e/Containerfile.gnome tests/e2e >/dev/null
"$engine" run --rm -v "$PWD:/src:ro" lyapunov-test-gnome dbus-run-session -- bash /src/tests/e2e/in-gnome.sh
