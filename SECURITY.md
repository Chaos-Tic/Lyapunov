# Security

Lyapunov reads which applications and windows are open, starts applications,
opens folders and documents, and moves windows. This page says what it
protects, how, and where the limits are.

## Reporting a problem

Please open a [private security advisory](https://github.com/Chaos-Tic/Lyapunov/security/advisories/new)
rather than a public issue. Describe what an attacker controls and what they
obtain.

## What Lyapunov never does

- Connect to a network, listen on a port, send telemetry, or require an account.
- Store window titles, URLs, passwords, tokens or environment variables in a card.
- Run a shell. Applications start from their desktop entry's `Exec` line, split
  without a shell, or from an executable path with no arguments.
- Close an application, or touch the files a card lists when the card is deleted.

## Cards from elsewhere

A card file may come from another computer or another person. Lyapunov treats
every card the same way, imported or not:

- **Strict validation** of every field (lengths, control characters, absolute
  paths, desktop entry IDs without path separators, executables ending in
  `.exe`, bounded geometry). Anything else is rejected with its reason.
- **Folders and documents only.** A place is resolved to the file it really
  points to (links included) before being checked. Programs, scripts,
  launchers and installers are never opened: executable files, `.desktop`,
  `.exe`, `.bat`, `.ps1`, `.lnk`, `.url`, `.chm`, `.msi`, `.jar`, `.appimage`,
  `.sh` and other script and shortcut types. Names that Windows would shorten
  to an executable (trailing dots or spaces) and alternate data streams are
  refused. The kind recorded (folder or document) must match what is on disk.
- **No network locations.** `\\server\share`, `//server` and their verbatim
  forms are refused: opening one would contact that server and, on Windows,
  could send it the user's credentials.
- **Preview first.** An import lists what each card would start and open
  before anything is saved, and an imported card always shows its plan before
  its first resume.

## The GNOME Shell extension

GNOME does not let applications see or move other windows, so Lyapunov ships
an extension that runs inside GNOME Shell and offers two D-Bus methods. It
answers only a caller whose executable is named `lyapunov` **and** is either
installed by a package (owned by root, writable by nobody else) or listed in
the user's private trust file `~/.config/lyapunov/gnome-trusted` (mode 0600),
written by `lyapunov gnome-extension`. A deleted binary is refused. Any other
program receives an access-denied error.

## KWin scripts

On KDE Plasma, Lyapunov loads a short script into KWin for each request and
unloads it at once. The script is written to a private temporary folder; data
from a card is embedded as JSON, never as code.

## Files

Cards live in a folder only the user can open (0700, files 0600), written
atomically. A symbolic link in place of the cards folder is refused.

## Limits

A program already running under your account can read and change your files,
including your cards and the GNOME trust file, and on X11 it can move windows
itself. Lyapunov does not protect against software you already run; it avoids
widening what such software can do, and it never makes your data reachable
from the network.
