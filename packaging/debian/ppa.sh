#!/usr/bin/env bash
# Builds signed Ubuntu source packages of a release and uploads them to the
# Launchpad PPA, which compiles them for each series. Runs in Ubuntu (the CI
# release job uses ubuntu:24.04), from the repository root:
#   packaging/debian/ppa.sh VERSION [SERIES...]
# The signing key is $PPA_SIGNING_KEY (ASCII-armored secret key) or, when it
# is empty, the first secret key in gpg. Set PPA_UPLOAD=0 to build only, and
# PPA_REVISION=2 to rebuild a version already published. Uploads go to
# ppa:$PPA (default chaos-tic/lyapunov). Ubuntu 22.04 is left out: its Qt is
# older than 6.4.
set -euo pipefail
version=${1:?usage: packaging/debian/ppa.sh VERSION [SERIES...]}
shift
series=("$@")
[ ${#series[@]} -gt 0 ] || series=(noble resolute stonking)
ppa=${PPA:-chaos-tic/lyapunov}
revision=${PPA_REVISION:-1}
url=https://github.com/Chaos-Tic/Lyapunov
maintainer='Chaos-Tic <58691435+Chaos-Tic@users.noreply.github.com>'

export DEBIAN_FRONTEND=noninteractive
command -v dpkg-buildpackage >/dev/null && command -v dput >/dev/null || {
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends dpkg-dev debhelper devscripts dput gnupg curl ca-certificates >/dev/null
}
if [ -n "${PPA_SIGNING_KEY:-}" ]; then
    export GNUPGHOME=$(mktemp -d)
    gpg --batch --quiet --import <<<"$PPA_SIGNING_KEY"
fi
key=$(gpg --list-secret-keys --with-colons | awk -F: '$1 == "fpr" { print $10; exit }')
[ -n "$key" ] || { echo 'No signing key: set PPA_SIGNING_KEY.' >&2; exit 1; }

# Older release tarballs lack packaging/debian: the checkout's is used then.
here_debian=$PWD/packaging/debian
out=$PWD/dist/ppa
mkdir -p "$out"
cd "$out"
# The release tarball is the upstream source; the vendor archive is its
# second component, unpacked in vendor/.
[ -f "lyapunov_$version.orig.tar.gz" ] ||
    curl -fsSL -o "lyapunov_$version.orig.tar.gz" "$url/archive/refs/tags/v$version.tar.gz"
[ -f "lyapunov_$version.orig-vendor.tar.xz" ] ||
    curl -fsSL -o "lyapunov_$version.orig-vendor.tar.xz" "$url/releases/download/v$version/lyapunov-$version-vendor.tar.xz"

# PPA_ORIG_UPLOADED=1: Launchpad already has this version's orig tarballs (an
# earlier series went through), so every series refers to them.
first=1
[ -n "${PPA_ORIG_UPLOADED:-}" ] && first=0
for s in "${series[@]}"; do
    case $s in
    noble) number=24.04 rust='cargo-1.85, rustc-1.85' ;;
    resolute) number=26.04 rust='cargo (>= 1.85), rustc (>= 1.85)' ;;
    stonking) number=26.10 rust='cargo (>= 1.85), rustc (>= 1.85)' ;;
    # The number orders versions across upgrades: add each new series here.
    *) echo "Unknown series $s: add it with its version number." >&2; exit 2 ;;
    esac
    full=$version-1~ppa$revision~ubuntu$number
    work=$(mktemp -d)
    mkdir "$work/lyapunov-$version" "$work/lyapunov-$version/vendor"
    tar -xzf "lyapunov_$version.orig.tar.gz" -C "$work/lyapunov-$version" --strip-components=1
    tar -xJf "lyapunov_$version.orig-vendor.tar.xz" -C "$work/lyapunov-$version/vendor" --strip-components=1
    debian=$work/lyapunov-$version/packaging/debian
    [ -d "$debian" ] || debian=$here_debian
    rm -rf "$work/lyapunov-$version/debian"
    cp -r "$debian" "$work/lyapunov-$version/debian"
    rm "$work/lyapunov-$version/debian/ppa.sh"
    sed -i "s/@RUST@/$rust/" "$work/lyapunov-$version/debian/control"
    cat >"$work/lyapunov-$version/debian/changelog" <<EOF
lyapunov ($full) $s; urgency=medium

  * Lyapunov $version, built from source for Ubuntu $number.

 -- $maintainer  $(date -R)
EOF
    # Launchpad keeps the first upload of the orig tarballs; later series refer to them.
    if [ $first = 1 ]; then sources=-sa; else sources=-sd; fi
    first=0
    ln -sf "$out"/lyapunov_$version.orig*.tar.* "$work/"
    (cd "$work/lyapunov-$version" && dpkg-buildpackage -S -d $sources -k"$key" >/dev/null)
    cp "$work"/lyapunov_${full}* "$out/"
    rm -rf "$work"
    echo "built lyapunov_${full}_source.changes"
    if [ "${PPA_UPLOAD:-1}" = 1 ]; then
        # Launchpad's FTP sometimes answers "550 internal server error" right
        # after another upload: try again after a pause, forcing the re-upload.
        for try in 1 2 3 4; do
            force=; [ "$try" = 1 ] || force=-f
            dput $force "ppa:$ppa" "lyapunov_${full}_source.changes" && break
            [ "$try" = 4 ] && exit 1
            echo "Upload failed (attempt $try); retrying in 90 s" >&2
            sleep 90
        done
    fi
done
