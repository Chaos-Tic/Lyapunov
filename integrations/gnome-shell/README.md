# GNOME Shell extension

GNOME does not let applications see or move other applications' windows.
This extension runs inside GNOME Shell and offers two methods on the session
bus, `io.github.ChaosTic.Lyapunov.Shell`:

- `Snapshot() → s`: screens, workspaces and application windows, as JSON.
- `Place(s) → s`: moves windows, and reports each one.

It answers only the `lyapunov` executable installed by a package, or one the
user trusted with `lyapunov gnome-extension` (see [SECURITY.md](../../SECURITY.md)).
Nothing is stored and nothing leaves the computer.

## Install

Lyapunov's window offers **Set up GNOME integration** when it is needed; in a
terminal, `lyapunov gnome-extension` does the same. Packages also install it in
`/usr/share/gnome-shell/extensions`. On Wayland, GNOME discovers a new
extension at login: log out and back in once.

Checked on GNOME Shell 50 by `scripts/test-gnome-container.sh`. Versions 45
to 49 are declared in `metadata.json` and use the same API, with fallbacks
for the calls that changed.
