#!/bin/sh
# Installs the built executables, the launcher, the icons and the GNOME Shell
# extension under DESTDIR/usr, for packages. Run from the repository root
# after building:
#   cargo build --release --locked && cmake --build build
#   sh packaging/linux/stage.sh DESTDIR
set -eu
dest=${1:?usage: packaging/linux/stage.sh DESTDIR}
install -Dm755 target/release/lyapunov "$dest/usr/bin/lyapunov"
install -Dm755 build/lyapunov-desktop "$dest/usr/bin/lyapunov-desktop"
strip "$dest/usr/bin/lyapunov" "$dest/usr/bin/lyapunov-desktop" 2>/dev/null || true
# The same entries `lyapunov integrate` writes for one user, pointing at the
# system executable, plus the GNOME Shell extension (enabled by each user).
"$dest/usr/bin/lyapunov" integrate --data-dir "$dest/usr/share" --exe /usr/bin/lyapunov-desktop --gnome-extension >/dev/null
install -Dm644 LICENSE "$dest/usr/share/licenses/lyapunov/LICENSE"
