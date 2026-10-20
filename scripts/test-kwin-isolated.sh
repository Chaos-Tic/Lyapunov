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
# A private copy avoids inheriting Arch's file capability when nested KWin
# starts; the test still uses a separate D-Bus and virtual screen.
cp "$(command -v kwin_wayland)" "$run/kwin_wayland"
"$run/kwin_wayland" --virtual --width 1600 --height 900 --socket lyapunov-test \
    --no-lockscreen --no-global-shortcuts --no-kactivities >"$run/kwin.log" 2>&1 &
kwin=$!
cleanup() {
    result=$?
    if (( result != 0 )); then
        echo "--- KWin startup log" >&2
        tail -60 "$run/kwin.log" >&2
    fi
    kill "$kwin" 2>/dev/null || true
    wait "$kwin" 2>/dev/null || true
}
trap cleanup EXIT
qdbus=$(command -v qdbus6 || command -v qdbus)
ready=0
for _ in $(seq 100); do
    if "$qdbus" org.kde.KWin /KWin >/dev/null 2>&1; then ready=1; break; fi
    if ! kill -0 "$kwin" 2>/dev/null; then break; fi
    sleep 0.1
done
(( ready )) || { echo "KWin did not register on the isolated D-Bus session" >&2; exit 1; }
export WAYLAND_DISPLAY=lyapunov-test
# KWin registers its objects one after the other: wait for the desktops.
for _ in $(seq 100); do
    "$qdbus" org.kde.KWin /VirtualDesktopManager org.kde.KWin.VirtualDesktopManager.createDesktop 1 Second >/dev/null 2>&1 && break
    sleep 0.1
done
second=$(gdbus call --session --dest org.kde.KWin --object-path /VirtualDesktopManager \
    --method org.freedesktop.DBus.Properties.Get org.kde.KWin.VirtualDesktopManager desktops \
    | grep -oE '[0-9a-f]{8}-[0-9a-f-]{27}' | sed -n 2p)
[[ -n "$second" ]] || { echo "KWin did not expose a second desktop" >&2; exit 1; }
LYAPUNOV=$PWD/target/release/lyapunov LYA_TEST_CMD=foot LYA_TEST_ID=foot.desktop \
    LYA_SECOND_DESKTOP=$second LYA_EVIDENCE=$run/evidence python3 tests/e2e/session_check.py
echo "Evidence: $run/evidence"
