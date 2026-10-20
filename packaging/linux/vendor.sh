#!/bin/sh
# Writes dist/lyapunov-VERSION-vendor.tar.xz: the crates in Cargo.lock, for
# builds without network (Fedora COPR, Ubuntu PPA). Run from the repository root.
# The archive is reproducible: the same Cargo.lock gives the same bytes.
set -eu
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cargo vendor --quiet --locked "$work/vendor" >/dev/null
mkdir -p dist
tar -C "$work" --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner \
    -cf - vendor | xz -9 -T1 >"dist/lyapunov-$version-vendor.tar.xz"
echo "dist/lyapunov-$version-vendor.tar.xz"
