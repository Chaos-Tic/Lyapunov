# Troubleshooting

[Back to the README](../README.md)

Start with **⋯ → Diagnostics** in the window, or in a terminal:

```sh
lyapunov doctor
```

It names the backend in use and what it can do on this desktop.

## "Windows are not arranged on this desktop"

The backend is `launch-only`: this desktop does not let applications see or
move windows. Applications, folders and documents still reopen.

- **GNOME**: use **Set up GNOME integration** (or `lyapunov gnome-extension`),
  then log out and back in. Check with `gnome-extensions info lyapunov@chaos-tic.github.io`
  that it is *ACTIVE*. If your GNOME is newer than the versions the extension
  declares, it stays disabled until Lyapunov is updated.
- **KDE Plasma**: KWin must own `org.kde.KWin` on the session bus
  (`qdbus6 org.kde.KWin /KWin` answers).
- **sway, i3**: `SWAYSOCK` or `I3SOCK` must be set, as the window manager does
  for the programs it starts.
- **Hyprland**: `HYPRLAND_INSTANCE_SIGNATURE` must be set.
- **Other Wayland compositors** (COSMIC, niri, river, labwc, Wayfire…): no
  backend yet.

## A window does not go back to its place

- Run the resume from a terminal with details:
  `LYAPUNOV_DEBUG=1 lyapunov resume "Card"` prints what was asked and answered.
- Some applications restore their own size after appearing; Lyapunov places
  them a second time, but an application that keeps moving itself wins.
- Tiling window managers place tiled windows themselves: only their workspace
  is restored. Floating windows keep their exact geometry.
- On Windows, windows of programs running as administrator cannot be moved
  by a normal program; the report says so.
- On Windows, windows on other virtual desktops are not captured.

## An application is "unavailable"

- *not installed*: its menu entry is gone. Install it again, or update the card.
- *Lyapunov cannot start this application*: it has no menu entry. Give it one
  (for an AppImage, its integration or AppImageLauncher), then use
  **Update from what is open…**.
- *runs in a terminal*: terminal programs are not started from cards.

## A card "could not be read"

The header shows how many, and **Details** names each file and the reason.
Lyapunov never deletes or rewrites such a file. A card made by a newer version
of Lyapunov opens after updating. Otherwise, fix or remove the file in the
cards folder (**⋯ → Open the cards folder**).

## A folder or document does not open

Lyapunov opens places with `xdg-open` (install `xdg-utils`), `gio open` or
`kde-open`, and refuses programs, scripts, launchers and network locations;
the report gives the reason.
