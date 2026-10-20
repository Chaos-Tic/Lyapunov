#!/usr/bin/env bash
# Builds a Linux package inside a container of the target distribution, from
# the repository root:
#   packaging/linux/build-package.sh deb    # in debian:12 (runs on Debian 12+, Ubuntu 24.04+)
#   packaging/linux/build-package.sh rpm    # in fedora:latest
#   packaging/linux/build-package.sh arch   # in archlinux:latest
# The package lands in dist/.
set -euo pipefail
format=${1:?usage: packaging/linux/build-package.sh deb|rpm|arch}
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
homepage=https://github.com/Chaos-Tic/Lyapunov
maintainer='Chaos-Tic <58691435+Chaos-Tic@users.noreply.github.com>'
summary='Save an activity as a card and resume it later'
description='Lyapunov keeps each activity as a card: the applications, windows,
folders and documents that belong to it. Resuming a card reopens what is
missing and puts windows back where they were, on KDE Plasma, GNOME (with
the included Shell extension), X11 window managers, sway, i3 and Hyprland.
Everything stays on this computer.'
mkdir -p dist

build() {
    cargo build --release --locked
    cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF
    cmake --build build --parallel "$(nproc)"
}

case $format in
deb)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y --no-install-recommends build-essential cmake qt6-base-dev dpkg-dev curl ca-certificates
    # Debian 12's Rust is older than the 1.85 that edition 2024 needs.
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    . "$HOME/.cargo/env"
    build
    stage=$(mktemp -d)/lyapunov
    DESTDIR="$stage" cmake --install build --prefix /usr --strip
    install -d "$stage/usr/share/doc/lyapunov"
    cat >"$stage/usr/share/doc/lyapunov/copyright" <<EOT
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: Lyapunov
Source: $homepage

Files: *
Copyright: Chaos-Tic
License: GPL-3.0+
 On Debian systems, the full text is in /usr/share/common-licenses/GPL-3.
EOT
    rm -rf "$stage/usr/share/licenses"
    arch=$(dpkg --print-architecture)
    # dpkg-shlibdeps maps the linked libraries to package names and versions;
    # it only needs a control file to exist.
    work=$(mktemp -d); mkdir "$work/debian"
    printf 'Source: lyapunov\n\nPackage: lyapunov\nArchitecture: any\n' >"$work/debian/control"
    depends=$(cd "$work" && dpkg-shlibdeps -O "$stage"/usr/bin/* | sed 's/^shlibs:Depends=//')
    size=$(du -sk "$stage" | cut -f1)
    mkdir "$stage/DEBIAN"
    {
        cat <<EOT
Package: lyapunov
Version: $version
Architecture: $arch
Maintainer: $maintainer
Installed-Size: $size
Depends: $depends, qt6-qpa-plugins
Recommends: xdg-utils, qt6-wayland
Section: utils
Priority: optional
Homepage: $homepage
Description: $summary
EOT
        printf '%s\n' "$description" | sed 's/^/ /'
    } >"$stage/DEBIAN/control"
    dpkg-deb --root-owner-group --build "$stage" "dist/lyapunov_${version}_${arch}.deb"
    ;;
rpm)
    dnf install -y gcc-c++ cmake qt6-qtbase-devel cargo rust rpm-build
    build
    stage=$(mktemp -d)/lyapunov
    DESTDIR="$stage" cmake --install build --prefix /usr --strip
    top=$(mktemp -d)
    # Library requirements are generated from the executables by rpmbuild.
    cat >"$top/lyapunov.spec" <<EOT
%global debug_package %{nil}
Name: lyapunov
Version: $version
Release: 1%{?dist}
Summary: $summary
License: GPL-3.0-or-later
URL: $homepage
Requires: qt6-qtbase-gui
Recommends: xdg-utils qt6-qtwayland

%description
$description

%install
cp -a $stage/. %{buildroot}/

%files
/usr/bin/lyapunov
/usr/bin/lyapunov-desktop
/usr/share/applications/lyapunov.desktop
/usr/share/icons/hicolor/scalable/apps/lyapunov.svg
/usr/share/icons/hicolor/symbolic/apps/lyapunov-symbolic.svg
/usr/share/gnome-shell/extensions/lyapunov@chaos-tic.github.io
%license /usr/share/licenses/lyapunov/LICENSE
EOT
    rpmbuild -bb "$top/lyapunov.spec" --define "_topdir $top" --define "_rpmdir $PWD/dist" \
        --define '_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm'
    ;;
arch)
    pacman -Syu --noconfirm --needed base-devel cmake rust qt6-base
    # makepkg refuses to run as root, and builds from the source tarball the
    # PKGBUILD names; a local one with that name is used instead of downloading.
    id builder >/dev/null 2>&1 || useradd -m builder
    work=$(mktemp -d)
    sed "s/^pkgver=.*/pkgver=$version/" packaging/arch/PKGBUILD >"$work/PKGBUILD"
    tar --exclude=./.git --exclude=./target --exclude=./build --exclude=./dist \
        --transform "s,^\.,Lyapunov-$version," -czf "$work/lyapunov-$version.tar.gz" .
    chown -R builder "$work"
    (cd "$work" && runuser -u builder -- makepkg --force --noconfirm)
    cp "$work"/lyapunov-"$version"-*.pkg.tar.zst dist/
    cp "$work/PKGBUILD" dist/PKGBUILD
    ;;
*)
    echo "Unknown format: $format (deb, rpm or arch)" >&2
    exit 2
    ;;
esac
ls -l dist
