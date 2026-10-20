#!/usr/bin/env bash
# The isolated KWin check (scripts/test-kwin-isolated.sh) inside an Arch
# container, for CI runners without KDE.
set -euo pipefail
cd "$(dirname "$0")/.."
engine=$(command -v docker || command -v podman) || { echo "Docker or Podman is required" >&2; exit 2; }
"$engine" run --rm -v "$PWD:/src:ro" archlinux:latest bash -c '
set -e
pacman -Syu --noconfirm --needed kwin foot python dbus qt6-tools rust base-devel ttf-dejavu >/dev/null
cp -r /src /tmp/w && cd /tmp/w && rm -rf target build dist
export PATH=/usr/lib/qt6/bin:$PATH
./scripts/test-kwin-isolated.sh
'
