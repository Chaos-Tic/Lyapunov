#!/usr/bin/env bash
# Inside the GNOME test container: a headless GNOME Shell on a virtual
# monitor, with the Lyapunov extension enabled, then session_check.py.
set -euo pipefail
uuid=lyapunov@chaos-tic.github.io
# GNOME Shell also expects a system bus with logind, mocked here.
mkdir -p /run/dbus && dbus-daemon --system --fork
python3 -m dbusmock --system --template logind >/tmp/logind.log 2>&1 &
for _ in $(seq 50); do busctl --system status org.freedesktop.login1 >/dev/null 2>&1 && break; sleep 0.1; done
export HOME=/tmp/home XDG_RUNTIME_DIR=/tmp/runtime XDG_DATA_HOME=/tmp/home/.local/share
export XDG_CONFIG_HOME=/tmp/home/.config LYAPUNOV_DATA_DIR=/tmp/cards XDG_SESSION_TYPE=wayland
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_DATA_HOME/gnome-shell/extensions" /tmp/cards /tmp/evidence
chmod 700 "$XDG_RUNTIME_DIR"
# Installed the way users do: the command writes the extension and trusts
# this lyapunov executable (it is not a root-owned packaged one here).
/src/target/release/lyapunov gnome-extension >/dev/null || true
test -f "$XDG_DATA_HOME/gnome-shell/extensions/$uuid/extension.js"
grep -qx /src/target/release/lyapunov "$XDG_CONFIG_HOME/lyapunov/gnome-trusted"
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "['$uuid']"
gsettings set org.gnome.mutter dynamic-workspaces false
gsettings set org.gnome.desktop.wm.preferences num-workspaces 2
gnome-shell --wayland --headless --virtual-monitor 1600x900 >/tmp/shell.log 2>&1 &
# Enabling works once the Shell itself owns its name on the bus.
for _ in $(seq 150); do
    busctl --user status org.gnome.Shell >/dev/null 2>&1 && break
    sleep 0.2
done
sleep 2
/src/target/release/lyapunov gnome-extension --json
for _ in $(seq 50); do
    busctl --user status io.github.ChaosTic.Lyapunov.Shell >/dev/null 2>&1 && break
    sleep 0.2
done
busctl --user status io.github.ChaosTic.Lyapunov.Shell >/dev/null 2>&1 || {
    echo "the extension did not start" >&2; grep -i -E 'lyapunov|error|JS' /tmp/shell.log | tail -20 >&2; exit 1; }
# Any program other than Lyapunov is refused.
answer=$(gdbus call --session --dest io.github.ChaosTic.Lyapunov.Shell --object-path /io/github/ChaosTic/Lyapunov/Shell \
    --method io.github.ChaosTic.Lyapunov.Shell.Snapshot 2>&1 || true)
if [[ $answer == *AccessDenied* ]]; then
    echo "  ok  other programs cannot list windows through the extension"
else
    echo "FAIL: the extension answered a program other than Lyapunov" >&2; exit 1
fi
# A copy of lyapunov that the user never trusted is refused too.
mkdir -p /tmp/untrusted && cp /src/target/release/lyapunov /tmp/untrusted/lyapunov && chown 1000 /tmp/untrusted/lyapunov
untrusted=$(LYAPUNOV_BACKEND=gnome /tmp/untrusted/lyapunov scan --json 2>&1 || true)
if [[ $untrusted == *AccessDenied* ]]; then
    echo "  ok  an untrusted lyapunov binary is refused"
else
    echo "FAIL: an untrusted lyapunov binary was answered: ${untrusted:0:300}" >&2; exit 1
fi
export WAYLAND_DISPLAY=wayland-0 GDK_BACKEND=wayland
export LYAPUNOV=/src/target/release/lyapunov LYA_EVIDENCE=/tmp/evidence LYAPUNOV_BACKEND=gnome
export LYA_TEST_CMD="gnome-text-editor --standalone" LYA_TEST_ID=org.gnome.TextEditor.desktop LYA_SECOND_DESKTOP=1
if ! python3 /src/tests/e2e/session_check.py; then
    grep -i -E 'lyapunov|error' /tmp/shell.log | tail -20 >&2
    echo "--- debug.log" >&2; cat /tmp/evidence/debug.log >&2
    exit 1
fi
