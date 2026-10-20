#!/usr/bin/env bash
# Capture and resume in a private KWin (Wayland, virtual screen) with its own
# D-Bus session and data folders. The desktop you are logged into is never
# contacted. Needs kwin_wayland, qdbus6 (or qdbus), foot, python3.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -z "${LYA_IN_SESSION:-}" ]]; then
    cargo build --release --locked
    run=$(mktemp -d "${TMPDIR:-/tmp}/lyapunov-kwin-XXXXXX")
    export LYA_RUN=$run LYA_IN_SESSION=1
    exec dbus-run-session -- "$0" "$@"
fi
run=$LYA_RUN
mkdir -p "$run/runtime" "$run/config" "$run/data" "$run/cards" "$run/evidence"
chmod 700 "$run/runtime"
export XDG_RUNTIME_DIR=$run/runtime XDG_CONFIG_HOME=$run/config XDG_DATA_HOME=$run/data
export LYAPUNOV_DATA_DIR=$run/cards LYAPUNOV_BACKEND=kwin
unset WAYLAND_DISPLAY DISPLAY
kwin_wayland --virtual --width 1600 --height 900 --socket lyapunov-test \
    --no-lockscreen --no-global-shortcuts --no-kactivities >"$run/kwin.log" 2>&1 &
kwin=$!
trap 'kill $kwin 2>/dev/null; wait $kwin 2>/dev/null || true' EXIT
qdbus=$(command -v qdbus6 || command -v qdbus)
for _ in $(seq 100); do "$qdbus" org.kde.KWin /KWin >/dev/null 2>&1 && break; sleep 0.1; done
export WAYLAND_DISPLAY=lyapunov-test
"$qdbus" org.kde.KWin /VirtualDesktopManager org.kde.KWin.VirtualDesktopManager.createDesktop 1 Second >/dev/null
second=$("$qdbus" --literal org.kde.KWin /VirtualDesktopManager org.freedesktop.DBus.Properties.Get \
    org.kde.KWin.VirtualDesktopManager desktops | grep -o '[0-9a-f]\{8\}-[0-9a-f-]\{27\}' | sed -n 2p)
LYAPUNOV=$PWD/target/release/lyapunov LYA_TEST_CMD=foot LYA_TEST_ID=foot.desktop \
    LYA_SECOND_DESKTOP=$second LYA_EVIDENCE=$run/evidence python3 tests/e2e/session_check.py
echo "Evidence: $run/evidence"
