#!/usr/bin/env bash
# Builds Lyapunov-x86_64.AppImage with Qt bundled, from the repository root.
# Needs Qt 6 in QT_ROOT_DIR (install-qt-action sets it) and the oldest system
# the AppImage should run on: it runs on newer distributions, never on older.
set -euo pipefail
: "${QT_ROOT_DIR:?set QT_ROOT_DIR to the Qt 6 installation}"
tools=$(mktemp -d)
fetch() { # name url sha256
    curl -sSfL -o "$tools/$1" "$2"
    echo "$3  $tools/$1" | sha256sum -c -
    chmod +x "$tools/$1"
}
fetch linuxdeploy-x86_64.AppImage \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage \
    c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
fetch linuxdeploy-plugin-qt-x86_64.AppImage \
    https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/1-alpha-20250213-1/linuxdeploy-plugin-qt-x86_64.AppImage \
    15106be885c1c48a021198e7e1e9a48ce9d02a86dd0a1848f00bdbf3c1c92724

cargo build --release --locked
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF -DCMAKE_PREFIX_PATH="$QT_ROOT_DIR"
cmake --build build --parallel "$(nproc)"

appdir=build/AppDir
rm -rf "$appdir"
install -Dm755 target/release/lyapunov "$appdir/usr/bin/lyapunov"
# Lyapunov's license, and Qt's (bundled below) with the crates' notices.
install -Dm644 LICENSE "$appdir/usr/share/licenses/lyapunov/LICENSE"
install -Dm644 packaging/licenses/LGPL-3.0.txt "$appdir/usr/share/licenses/lyapunov/LGPL-3.0.txt"
packaging/licenses/notices.sh "$appdir/usr/share/licenses/lyapunov/THIRD-PARTY-NOTICES.txt" >/dev/null
# The AppImage path changes at every mount: `lyapunov integrate`, run from
# inside it, writes a menu entry that points at the AppImage file itself.
cat >build/lyapunov.desktop <<'EOT'
[Desktop Entry]
Type=Application
Name=Lyapunov
GenericName=Workspace cards
Comment=Save an activity as a card and pick it up later
Exec=lyapunov-desktop
Icon=lyapunov
Terminal=false
Categories=Utility;
StartupWMClass=lyapunov-desktop
EOT
mkdir -p dist
export PATH="$QT_ROOT_DIR/bin:$tools:$PATH" QMAKE="$QT_ROOT_DIR/bin/qmake"
# xcb runs under X11 and XWayland; offscreen serves the CI smoke test.
export EXTRA_PLATFORM_PLUGINS=libqoffscreen.so OUTPUT=dist/Lyapunov-x86_64.AppImage
APPIMAGE_EXTRACT_AND_RUN=1 linuxdeploy-x86_64.AppImage --appdir "$appdir" \
    --executable build/lyapunov-desktop --desktop-file build/lyapunov.desktop \
    --icon-file assets/lyapunov.svg --plugin qt --output appimage
ls -l dist
