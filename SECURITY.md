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
  without a shell, or from an executable path with no arguments. The optional
  VS Code adapter starts `code` with `--new-window` and one validated local
  folder argument, also without a shell.
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
- **Preview first.** An import lists what each card would start and open,
  including VS Code project folder paths,
  before anything is saved, and an imported card always shows its plan before
  its first resume.

## The GNOME Shell extension

GNOME does not let applications see or move other windows, so Lyapunov ships
an extension that runs inside GNOME Shell and offers two D-Bus methods. It
answers only a caller whose executable is named `lyapunov` **and** is either
installed by a package (owned by root, writable by nobody else) or listed in
the user's private trust file `~/.config/lyapunov/gnome-trusted` (mode 0600),
written by `lyapunov gnome-extension`. A deleted binary, or a process started
with injected libraries (`LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`), or
running under a debugger, is refused. Any other program receives an access-denied error.

This gate keeps other programs from reading window titles and positions by
accident or casually. It is not a barrier against malicious code already
running under your account: such code can also edit the trust file, which
lives in your own configuration folder (see Limits).

## KWin scripts

On KDE Plasma, Lyapunov loads a short script into KWin for each request and
unloads it at once. The script is written to a private temporary folder; data
from a card is embedded as JSON, never as code.

## VS Code project extension

The optional Linux extension reports one canonical local folder per VS Code
window to files in the current user's runtime directory. It refreshes them
every two seconds; each record contains only the folder path and extension
host PID. Lyapunov accepts only a recent record from a live process. The
directory is private (0700) and the records are private (0600). No listener
or network connection is created. Remote and multi-root workspaces are not
reported. A failed launch leaves a guard in the runtime directory so another
resume cannot silently open a duplicate window; a verified project clears it.

## Files

Cards live in a folder only the user can open (0700, files 0600), written
atomically. A symbolic link in place of the cards folder is refused.
VS Code project paths are stored in cards only when explicitly chosen.

## Limits

A program already running under your account can read and change your files,
including your cards and the GNOME trust file, and on X11 it can move windows
itself. Lyapunov does not protect against software you already run; it avoids
widening what such software can do, and it never makes your data reachable
from the network.
