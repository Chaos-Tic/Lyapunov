# Changelog

## 0.2.0 — 2026-10-20

- The window checks GitHub once a day for a newer release and shows how to
  update for the way Lyapunov was installed (package manager, winget, AppImage,
  source). One anonymous request; turn it off in the menu.
- New icon in the Liquid Glass family of Lorenz: an open card between two bands,
  with its Resume button. Card emblems and the README banner follow it.
- Standard `cmake --install` rules install the core, the window, the launcher,
  the icons, the license and the GNOME extension; every package uses them.
- The GNOME extension also refuses a caller running under a debugger.
- `TRADEMARKS.md` and `CONTRIBUTING.md` state how the name and logo may be used
  and how to contribute.

## 0.1.0 — 2026-10-20

First version.

- Cards: capture the applications, windows, folders and documents of an
  activity; resume them with a preview of every step and an exact report.
- Window backends for KDE Plasma (KWin), GNOME (with the Lyapunov Shell
  extension), X11 window managers, sway, i3, Hyprland and Windows; elsewhere,
  applications, folders and documents still reopen.
- A Liquid Glass card stack in English and French, light and dark, with
  keyboard navigation, reduced motion and reduced transparency.
- Command line with JSON output for every action; export and import.
- Optional Linux VS Code project integration: one local folder per card,
  separate A/B windows, verified reuse without a duplicate, and an isolated
  end-to-end test.
- Packages for Debian, Ubuntu, Fedora, Arch, an AppImage and a Windows
  installer, built and tested by the full CI workflow when it runs.
