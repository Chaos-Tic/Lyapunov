#!/usr/bin/env bash
# Installs a built package in a clean container, checks it, then removes it:
#   packaging/linux/test-package.sh dist/lyapunov_0.1.0_amd64.deb
set -euo pipefail
package=$(realpath "${1:?usage: packaging/linux/test-package.sh PACKAGE}")
case $package in
*.deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y --no-install-recommends "$package"
    remove() { apt-get remove -y lyapunov; } ;;
*.rpm)
    dnf install -y --setopt=install_weak_deps=False "$package"
    remove() { dnf remove -y lyapunov; } ;;
*.pkg.tar.zst)
    pacman -Sy --noconfirm
    pacman -U --noconfirm "$package"
    remove() { pacman -R --noconfirm lyapunov; } ;;
*) echo "Unknown package type: $package" >&2; exit 2 ;;
esac

lyapunov --version
lyapunov doctor
for file in /usr/share/applications/lyapunov.desktop /usr/share/icons/hicolor/scalable/apps/lyapunov.svg \
    /usr/share/gnome-shell/extensions/lyapunov@chaos-tic.github.io/extension.js; do
    test -f "$file" || { echo "missing $file" >&2; exit 1; }
done
grep -q '^Exec="/usr/bin/lyapunov-desktop"$' /usr/share/applications/lyapunov.desktop
# The packaged core is owned by root and writable by nobody else: the GNOME
# extension answers it without a trust file.
[ "$(stat -c %u /usr/bin/lyapunov)" = 0 ] && [ "$(( $(stat -c %a /usr/bin/lyapunov) & 022 ))" = 0 ]
# A card round trip in a private data folder.
export LYAPUNOV_DATA_DIR=$(mktemp -d) LYAPUNOV_BACKEND=launch-only
lyapunov capture "Package check" --json >/dev/null
[ "$(lyapunov list --json | grep -c '"Package check"')" = 1 ]
# The Qt libraries and platform plugin the package depends on are enough to start.
QT_QPA_PLATFORM=offscreen lyapunov-desktop --smoke-test
remove
for file in /usr/bin/lyapunov /usr/share/applications/lyapunov.desktop; do
    test ! -e "$file" || { echo "removal left $file" >&2; exit 1; }
done
echo "OK: $(basename "$package")"
