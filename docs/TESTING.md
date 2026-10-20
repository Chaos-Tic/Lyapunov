# Testing

[Back to the README](../README.md)

## Automated

| Command | What it proves | Where |
| --- | --- | --- |
| `cargo test` | Card validation, storage (atomic writes, corrupt and newer cards, permissions, paths), desktop entries and `Exec` splitting, application recognition, refused places, geometry fitting, desktop matching, plan and resume with a fake backend, backend parsers and command builders | Linux, Windows, Rust 1.85 |
| `ctest --test-dir build` | The card stack offscreen: layout, every band visible, hit testing, keyboard, Resume, More, Delete, accessible name | Linux, Windows |
| `lyapunov-desktop --smoke-test` | The window starts with the installed Qt | every package |
| `scripts/test-kwin-isolated.sh` | Capture and resume end to end in a private nested KWin (Wayland) with its own D-Bus | Linux with KWin; CI in an Arch container |
| `scripts/test-gnome-container.sh` | The same on a headless GNOME Shell with the extension; other callers and untrusted copies of `lyapunov` are refused | Fedora container |
| `scripts/test-sessions-container.sh` | The same under Openbox, Xfwm4, i3 and headless sway | Arch container |
| `scripts/test-packages-container.sh` | `.deb`, `.rpm` and Arch packages built in their distribution, installed, used and removed in clean containers | Docker or Podman |
| `scripts/test-install-container.sh IMAGE` | `./install.sh --yes` then `./uninstall.sh` as a normal user | Debian, Ubuntu, Fedora, Arch, openSUSE |

The end-to-end check ([`tests/e2e/session_check.py`](../tests/e2e/session_check.py))
opens a window, captures it, checks that no title reached the card, moves it
through the card, closes the application and lets Lyapunov start it again,
simulates an unplugged screen, a second desktop, maximising and restoring, and
deletes the card without closing anything. Each step's JSON is kept as
evidence.

## By hand, before a release

On each desktop declared compatible, in a real session, dated in
[COMPATIBILITY.md](COMPATIBILITY.md):

1. Capture three real activities with several applications, two screens if
   available, and two virtual desktops.
2. Close everything, resume each card: every window on its screen, desktop
   and place; the report matches what happened.
3. Unplug a screen (or change its resolution), resume: windows on a present
   screen, fully visible.
4. Resume with an application uninstalled and a folder renamed: both reported
   as unavailable, the rest done.
5. Keyboard only: Tab, arrows, Enter, Escape, the menu key; Orca or Narrator
   announces the active card.
6. Reduce motion and reduce transparency; light and dark; English and French.
7. Import a card exported from another computer: the preview lists what it
   starts and opens; the first resume shows the plan.

## Screenshots

`scripts/render-docs.sh` regenerates `docs/media` from demo data, so images
never show a real card, window or path.
