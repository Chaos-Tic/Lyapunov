#!/usr/bin/env bash
# Runs ./install.sh --yes in a clean container of a distribution, as a user
# with sudo, then checks the installed commands:
#   scripts/test-install-container.sh debian:stable
set -euo pipefail
cd "$(dirname "$0")/.."
image=${1:?usage: scripts/test-install-container.sh IMAGE}
engine=$(command -v docker || command -v podman) || { echo "Docker or Podman is required" >&2; exit 2; }
"$engine" run --rm -v "$PWD:/src:ro" "$image" bash -c '
set -e
# sudo and runuser (util-linux) are missing from minimal images.
if command -v apt-get >/dev/null; then apt-get update -qq && apt-get install -y -qq sudo >/dev/null
elif command -v pacman >/dev/null; then pacman -Sy --noconfirm --needed sudo util-linux >/dev/null
elif command -v dnf >/dev/null; then dnf install -y -q sudo util-linux shadow-utils >/dev/null
elif command -v zypper >/dev/null; then zypper --non-interactive install sudo util-linux shadow >/dev/null
fi
useradd -m tester && echo "tester ALL=(ALL) NOPASSWD: ALL" >/etc/sudoers.d/tester
cp -r /src /home/tester/lyapunov && rm -rf /home/tester/lyapunov/{target,build,dist} && chown -R tester /home/tester/lyapunov
runuser -u tester -- bash -c "cd ~/lyapunov && ./install.sh --yes"
runuser -u tester -- bash -c "~/.local/bin/lyapunov --version && test -f ~/.local/share/applications/lyapunov.desktop && QT_QPA_PLATFORM=offscreen ~/.local/bin/lyapunov-desktop --smoke-test && cd ~/lyapunov && ./uninstall.sh </dev/null && test ! -e ~/.local/bin/lyapunov"
echo "OK: install and uninstall"
'
