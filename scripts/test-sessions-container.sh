#!/usr/bin/env bash
# Capture and resume under X11 window managers (Openbox, Xfwm4), i3 and sway,
# each in a container on a virtual screen:
#   scripts/test-sessions-container.sh            # every window manager
#   scripts/test-sessions-container.sh sway i3    # some of them
# Needs Docker or Podman. The lyapunov executable is built on the host and
# mounted read-only; the container image is cached as lyapunov-test-sessions.
set -euo pipefail
cd "$(dirname "$0")/.."
engine=$(command -v docker || command -v podman) || { echo "Docker or Podman is required" >&2; exit 2; }
wms=("$@")
[[ ${#wms[@]} -gt 0 ]] || wms=(openbox xfwm4 i3 sway)
cargo build --release --locked
"$engine" build -q -t lyapunov-test-sessions -f tests/e2e/Containerfile tests/e2e >/dev/null
failed=()
for wm in "${wms[@]}"; do
    echo "== $wm"
    if "$engine" run --rm -v "$PWD:/src:ro" -e WM="$wm" lyapunov-test-sessions \
        dbus-run-session -- bash /src/tests/e2e/in-container.sh; then
        :
    else
        failed+=("$wm")
    fi
done
if [[ ${#failed[@]} -gt 0 ]]; then
    echo "FAILED: ${failed[*]}" >&2
    exit 1
fi
echo "All sessions passed: ${wms[*]}"
