# Compatibility

[Back to the README](../README.md)

Only what has actually been checked is listed, with its date. "Isolated
session" means a private compositor or X server with its own D-Bus and a
virtual screen: it proves the backend against the real window manager, not a
user's full desktop. Each row is redone before a release (see
[TESTING.md](TESTING.md)).

## Desktops

| Desktop | Backend | Checked | Result | Date |
| --- | --- | --- | --- | --- |
| KDE Plasma 6.8, Wayland (Arch, real machine) | `kwin` | `lyapunov doctor` and `scan`: windows listed and applications recognised | pass | 2026-10-20 |
| KWin 6.8, nested Wayland session | `kwin` | full end-to-end check | pass | 2026-10-20 |
| GNOME Shell 50, headless (Fedora 44) | `gnome` | full end-to-end check; other callers and untrusted binaries refused | pass | 2026-10-20 |
| Openbox (X11, Arch) | `x11` | full end-to-end check | pass | 2026-10-20 |
| Xfwm4 (X11, Arch) | `x11` | full end-to-end check | pass | 2026-10-20 |
| i3 (X11, Arch) | `i3` | end-to-end check, floating windows | pass | 2026-10-20 |
| sway, headless (Arch) | `sway` | end-to-end check, floating windows | pass | 2026-10-20 |
| Hyprland | `hyprland` | parsing and command tests only | not run in a session | — |
| Windows Server (GitHub runner) | `windows` | `lyapunov doctor` lists windows | pass | 2026-10-20 |
| GNOME 45–49 | `gnome` | declared in the extension, not run | not verified | — |
| Real sessions: Plasma (moving windows), GNOME, Cinnamon, MATE, Xfce, Hyprland, Windows 10/11 | | | not verified | — |

## VS Code project integration

| Scenario | Checked | Result | Date |
| --- | --- | --- | --- |
| Linux VS Code 1.138, Xvfb, disposable profile | A open; card for B opens a new window; second resume reuses B; two folder markers remain | pass | 2026-10-20 |

This verifies project presence and duplicate prevention in an isolated VS
Code session. Window placement, unsaved buffers, remote workspaces and
multi-root workspaces are outside this integration.

## Packages

Built in CI on every change; see the latest run of the
[workflow](https://github.com/Chaos-Tic/Lyapunov/actions/workflows/ci.yml).

| Package | Built in | Installed, used and removed in | Date |
| --- | --- | --- | --- |
| `.deb` | Debian 12 | Debian 12, Debian stable, Ubuntu 24.04 (local containers) | 2026-10-20 |
| `.rpm` | Fedora 44 | Fedora 44 (local container) | 2026-10-20 |
| Arch package | Arch | Arch (local container) | 2026-10-20 |
| `install.sh` from source | — | Debian stable, Ubuntu 24.04, Arch (CI) | 2026-10-20 |

## Not verified

- KWin end-to-end in GitHub CI: the Arch container crashes before the
  compositor registers on D-Bus. The nested host test above passes; see
  [Testing](TESTING.md).

- Interaction in a real session after capture: the sheet, drag and drop of
  folders, the global shortcut.
- Windows 10 and 11 desktops, SmartScreen on a fresh machine.
- Hyprland, COSMIC and other Wayland compositors.
