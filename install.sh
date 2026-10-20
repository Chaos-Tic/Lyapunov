#!/usr/bin/env bash
# Lyapunov installer for Linux distributions.
#
# Builds Lyapunov from this source tree and installs it for the current user
# in ~/.local. Administrator access is used only to install build tools with
# the distribution's package manager. Run ./install.sh --help for options.
set -euo pipefail
cd "$(dirname "$0")"

usage() {
    cat <<'EOT'
Usage: ./install.sh [options]

Without options, the installer asks before each optional step.

  -y, --yes             Accept the recommended answer to every question
      --no-deps         Do not install packages; use what is already installed
      --gnome           Set up the GNOME Shell extension (GNOME only)
      --no-gnome        Skip the GNOME Shell extension
  -h, --help            Show this help

Remove Lyapunov later with ./uninstall.sh. Your cards are kept.
EOT
}

assume_yes=false deps=ask gnome=ask
while (($#)); do
    case "$1" in
        -y|--yes) assume_yes=true ;;
        --no-deps) deps=no ;;
        --gnome) gnome=yes ;;
        --no-gnome) gnome=no ;;
        -h|--help) usage; exit 0 ;;
        *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
    esac
    shift
done

bold=$'\e[1m' green=$'\e[32m' yellow=$'\e[33m' reset=$'\e[0m'
[[ -t 1 ]] || bold='' green='' yellow='' reset=''
step() { printf '\n%s==>%s %s%s%s\n' "$green" "$reset" "$bold" "$*" "$reset"; }
note() { printf '    %s\n' "$*"; }
warn() { printf '%s    ! %s%s\n' "$yellow" "$*" "$reset" >&2; }

# ask PRESET "Question" DEFAULT(y|n) -> returns 0 for yes.
ask() {
    local preset=$1 question=$2 default=$3 reply
    [[ $preset == yes ]] && return 0
    [[ $preset == no ]] && return 1
    if $assume_yes || [[ ! -t 0 ]]; then [[ $default == y ]]; return; fi
    local hint='[y/N]'; [[ $default == y ]] && hint='[Y/n]'
    while true; do
        read -r -p "    $question $hint " reply || reply=''
        case "${reply,,}" in
            '') [[ $default == y ]]; return ;;
            y|yes) return 0 ;;
            n|no) return 1 ;;
        esac
    done
}

as_root() {
    if [[ $EUID -eq 0 ]]; then "$@"; else sudo "$@"; fi
}

# ---------------------------------------------------------------------------
# Distribution detection and package names
# ---------------------------------------------------------------------------
family=unknown
if command -v pacman >/dev/null; then family=arch
elif command -v apt-get >/dev/null; then family=debian
elif command -v dnf >/dev/null; then family=fedora
elif command -v zypper >/dev/null; then family=suse
elif command -v xbps-install >/dev/null; then family=void
elif command -v apk >/dev/null; then family=alpine
elif command -v emerge >/dev/null; then family=gentoo
fi

case $family in
    arch)   build=(base-devel cmake rust qt6-base xdg-utils); install_cmd=(pacman -S --needed --noconfirm) ;;
    debian) build=(build-essential cmake cargo rustc qt6-base-dev xdg-utils curl); install_cmd=(apt-get install -y) ;;
    fedora) build=(gcc-c++ cmake cargo rust qt6-qtbase-devel xdg-utils); install_cmd=(dnf install -y) ;;
    suse)   build=(gcc-c++ cmake cargo rust qt6-widgets-devel qt6-network-devel xdg-utils); install_cmd=(zypper --non-interactive install) ;;
    void)   build=(base-devel cmake rust cargo qt6-base-devel xdg-utils); install_cmd=(xbps-install -Sy) ;;
    alpine) build=(build-base cmake cargo rust qt6-qtbase-dev xdg-utils); install_cmd=(apk add) ;;
    gentoo) build=(dev-build/cmake dev-lang/rust-bin dev-qt/qtbase x11-misc/xdg-utils); install_cmd=(emerge --noreplace) ;;
    *)      build=(); install_cmd=() ;;
esac

install_packages() {
    ((${#install_cmd[@]})) || return 1
    if [[ $family == debian ]]; then as_root apt-get update -qq; fi
    as_root "${install_cmd[@]}" "$@"
}

step "Lyapunov installer"
. /etc/os-release 2>/dev/null || true
note "System: ${PRETTY_NAME:-unknown Linux} · package family: $family"
note "Session: ${XDG_SESSION_TYPE:-unknown} · desktop: ${XDG_CURRENT_DESKTOP:-unknown}"

# ---------------------------------------------------------------------------
# 1. Build tools
# ---------------------------------------------------------------------------
rust_ok() {
    command -v cargo >/dev/null && command -v rustc >/dev/null || return 1
    local version; version=$(rustc --version | awk '{print $2}')
    # Edition 2024 requires Rust 1.85 or newer.
    [[ "$(printf '%s\n1.85.0\n' "$version" | sort -V | head -n1)" == 1.85.0 ]]
}
qt_ok() { command -v qmake6 >/dev/null || [[ -e /usr/lib/qt6/bin/qmake6 || -e /usr/lib64/qt6/bin/qmake6 ]] || pkg-config --exists Qt6Widgets 2>/dev/null; }

step "Build tools"
if [[ $deps != no ]] && ((${#build[@]})); then
    missing=false
    for tool in cmake c++; do command -v "$tool" >/dev/null || missing=true; done
    rust_ok && qt_ok || missing=true
    if $missing; then
        if ask ask "Install the build tools with your package manager (${build[*]})?" y; then
            install_packages "${build[@]}"
        fi
    else
        note "Already installed."
    fi
elif [[ $family == unknown ]]; then
    warn "Unknown package manager. Install manually: Rust ≥ 1.85, a C++17 compiler, CMake ≥ 3.21, Qt 6 ≥ 6.4 (Widgets, Network)."
fi
if ! rust_ok; then
    warn "Rust 1.85 or newer is required (found: $(rustc --version 2>/dev/null || echo none))."
    if ask ask "Install the current Rust toolchain for your user with rustup (downloads from rust-lang.org)?" y; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi
    rust_ok || { echo "Rust 1.85+ is still missing; aborting." >&2; exit 1; }
fi
for tool in cmake c++; do
    command -v "$tool" >/dev/null || { echo "Missing $tool; install it and run the installer again." >&2; exit 1; }
done

# ---------------------------------------------------------------------------
# 2. Build and install for this user
# ---------------------------------------------------------------------------
step "Building Lyapunov"
cargo build --release --locked
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF >/dev/null
cmake --build build --parallel "$(nproc 2>/dev/null || echo 2)"

step "Installing in ~/.local"
pkill -x lyapunov-desktop 2>/dev/null || true
install -Dm755 target/release/lyapunov "$HOME/.local/bin/lyapunov"
install -Dm755 build/lyapunov-desktop "$HOME/.local/bin/lyapunov-desktop"
"$HOME/.local/bin/lyapunov" integrate | sed 's/^/    /'
command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) warn "$HOME/.local/bin is not in PATH. Add it to use the lyapunov command in a terminal." ;;
esac

# ---------------------------------------------------------------------------
# 3. GNOME Shell extension
# ---------------------------------------------------------------------------
if [[ ${XDG_CURRENT_DESKTOP:-} == *GNOME* ]]; then
    step "GNOME integration"
    note "GNOME lets an extension, not an application, see and move windows."
    if ask "$gnome" "Install and enable the Lyapunov GNOME Shell extension?" y; then
        "$HOME/.local/bin/lyapunov" gnome-extension | sed 's/^/    /'
        if [[ ${XDG_SESSION_TYPE:-} == wayland ]]; then
            note "${bold}Log out and back in${reset}, then open Lyapunov: windows will be arranged too."
        fi
    fi
fi

step "Done"
"$HOME/.local/bin/lyapunov" doctor | sed 's/^/    /'
note "Open Lyapunov from the applications menu."
[[ ${XDG_CURRENT_DESKTOP:-} == *KDE* ]] && note "Shortcut: Meta+Alt+Space (change it in System Settings → Shortcuts)."
note "Uninstall: ./uninstall.sh (your cards are kept)."
