# Packaging

Every package is built from source in the distribution it targets, then
installed, used and removed in clean containers by CI
([`.github/workflows/ci.yml`](../.github/workflows/ci.yml)).

| Channel | Files | Built by |
| --- | --- | --- |
| `.deb` (Debian 12+, Ubuntu 24.04+) | `linux/build-package.sh deb` | CI `linux-package`, in Debian 12 |
| `.rpm` (Fedora) | `linux/build-package.sh rpm` | CI `linux-package`, in Fedora |
| Arch | `arch/PKGBUILD`, `linux/build-package.sh arch` | CI `linux-package`, in Arch |
| AUR | `arch/PKGBUILD`, `arch/aur.sh` | CI `aur` after a release, when `PUBLISH_AUR` is `true` |
| Fedora COPR | `fedora/lyapunov.spec`, `../.copr/Makefile` | COPR, from the release tarball and the vendor archive; checked by CI `fedora-spec` |
| Ubuntu PPA | `debian/`, `debian/ppa.sh` | CI `ppa` after a release, when `PUBLISH_PPA` is `true` |
| AppImage | `linux/build-appimage.sh` | CI `appimage`, on Ubuntu 22.04 |
| Windows installer | `windows/lyapunov.iss` | CI `desktop-windows` (Inno Setup) |
| Vendored crates | `linux/vendor.sh` | CI `vendor`, attached to each release for offline builds |

Shared pieces: `cmake --install` lays out the files every Linux package
installs (executables, menu entry, icons, GNOME Shell extension);
`linux/stage.sh` remains a wrapper for older packaging commands;
`linux/test-package.sh` installs a package, checks it and removes it;
`licenses/notices.sh` writes the third-party notices bundled with Qt in the
AppImage and the Windows installer.

For a staged distro install after building the Rust and Qt executables:

```sh
DESTDIR="$pkgdir" cmake --install build --prefix /usr
```

The generated launcher points at `/usr/bin/lyapunov-desktop` after
installation. The interactive `install.sh` uses the same CMake install rule
with a user prefix and offers GNOME extension activation separately.

## Publication of 0.1.0

The public [v0.1.0 release](https://github.com/Chaos-Tic/Lyapunov/releases/tag/v0.1.0)
contains the Linux packages, AppImage, Windows installer, vendor archive and
`SHA256SUMS`. The AUR package is published as
[`lyapunov`](https://aur.archlinux.org/packages/lyapunov) and was built,
installed and removed from AUR source in a clean Arch container. The
[Chaos-Tic apt repository](https://chaos-tic.github.io/apt) indexes `lyapunov`
0.1.0; an install and removal through apt passed in Debian 12. The
[COPR project](https://copr.fedorainfracloud.org/coprs/chaostic/lyapunov/)
built successfully for Fedora 43–45; an install and removal through COPR
passed on Fedora 44. The [Ubuntu PPA](https://launchpad.net/~chaos-tic/+archive/ubuntu/lyapunov)
has accepted the signed source packages for Ubuntu 24.04, 26.04 and 26.10;
its binary builds are pending. The
[winget manifest](https://github.com/microsoft/winget-pkgs/pull/457224) passed
the contributor agreement check and awaits upstream review; it is not yet in
the winget catalog.

Future tags use the configured `AUR_SSH_KEY`, `COPR_WEBHOOK` and
`PPA_SIGNING_KEY` secrets. `PUBLISH_AUR=true` and `PUBLISH_PPA=true` are set.

Local check of the Linux packages: `scripts/test-packages-container.sh`.
