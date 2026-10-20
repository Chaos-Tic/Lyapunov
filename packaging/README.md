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

Shared pieces: `linux/stage.sh` lays out the files every Linux package
installs (executables, menu entry, icons, GNOME Shell extension);
`linux/test-package.sh` installs a package, checks it and removes it;
`licenses/notices.sh` writes the third-party notices bundled with Qt in the
AppImage and the Windows installer.

## Publishing (not set up yet)

Nothing is published until a first release is decided. Then, for each
channel: create the COPR project `chaostic/lyapunov` and store its webhook as
the `COPR_WEBHOOK` secret; create the PPA `chaos-tic/lyapunov`, add the
`PPA_SIGNING_KEY` secret and set the `PUBLISH_PPA` variable to `true`; create
the AUR package `lyapunov`, add `AUR_SSH_KEY` and set `PUBLISH_AUR` to `true`;
add `lyapunov` to `projects.txt` in the `Chaos-Tic/apt` repository. A `v*` tag
then publishes everything.

Local check of the Linux packages: `scripts/test-packages-container.sh`.
