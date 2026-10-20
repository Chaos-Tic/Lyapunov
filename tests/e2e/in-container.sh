#!/usr/bin/env bash
# Inside the test container: starts the window manager named by $WM on a
# virtual screen, then runs session_check.py against it.
set -euo pipefail
export XDG_RUNTIME_DIR=/tmp/runtime XDG_DATA_HOME=/tmp/data LYAPUNOV_DATA_DIR=/tmp/cards
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_DATA_HOME/applications" /tmp/cards /tmp/evidence
chmod 700 "$XDG_RUNTIME_DIR"
export LYAPUNOV=/src/target/release/lyapunov LYA_EVIDENCE=/tmp/evidence

x_session() {
    Xvfb :99 -screen 0 1600x900x24 -nolisten tcp >/tmp/xvfb.log 2>&1 &
    export DISPLAY=:99
    for _ in $(seq 50); do xprop -root >/dev/null 2>&1 && break; sleep 0.1; done
    # A test application with its own class and menu entry.
    printf '[Desktop Entry]\nType=Application\nName=Lyapunov test\nExec=xterm -class LyaTest\nStartupWMClass=LyaTest\n' \
        >"$XDG_DATA_HOME/applications/lya-test.desktop"
    export LYA_TEST_CMD="xterm -class LyaTest" LYA_TEST_ID=lya-test.desktop
}

case $WM in
openbox)
    x_session
    openbox >/tmp/wm.log 2>&1 &
    export LYAPUNOV_BACKEND=x11 LYA_SECOND_DESKTOP=1
    ;;
xfwm4)
    x_session
    xfwm4 --compositor=off >/tmp/wm.log 2>&1 &
    export LYAPUNOV_BACKEND=x11 LYA_SECOND_DESKTOP=1
    # Xfwm4 may start after the fixed one-second delay, especially in CI.
    # Wait for its EWMH announcement before Lyapunov probes the session.
    for _ in $(seq 100); do
        if xprop -root _NET_SUPPORTING_WM_CHECK 2>/dev/null | grep -Eq '= window id # 0x[1-9a-fA-F][0-9a-fA-F]*'; then
            break
        fi
        sleep 0.1
    done
    ;;
i3)
    x_session
    printf 'font pango:DejaVu Sans 9\nbar {\n  mode invisible\n}\n' >/tmp/i3.conf
    i3 -c /tmp/i3.conf >/tmp/wm.log 2>&1 &
    export LYAPUNOV_BACKEND=i3
    ;;
sway)
    export WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman
    printf 'output HEADLESS-1 resolution 1600x900\n' >/tmp/sway.conf
    # Arch ships sway with a file capability that containers refuse; a copy has none.
    cp /usr/bin/sway /tmp/sway
    /tmp/sway -c /tmp/sway.conf >/tmp/wm.log 2>&1 &
    for _ in $(seq 50); do ls "$XDG_RUNTIME_DIR"/sway-ipc.*.sock >/dev/null 2>&1 && break; sleep 0.1; done
    SWAYSOCK=$(ls "$XDG_RUNTIME_DIR"/sway-ipc.*.sock)
    WAYLAND_DISPLAY=$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-* | grep -v lock | head -1)")
    export SWAYSOCK WAYLAND_DISPLAY LYAPUNOV_BACKEND=sway LYA_TEST_CMD=foot LYA_TEST_ID=foot.desktop
    ;;
*)
    echo "unknown window manager $WM" >&2
    exit 2
    ;;
esac
sleep 1
if ! python3 /src/tests/e2e/session_check.py; then
    echo "--- window manager log" >&2
    tail -20 /tmp/wm.log >&2
    for f in /tmp/evidence/*; do echo "--- $f" >&2; cat "$f" >&2; done
    exit 1
fi
