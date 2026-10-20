# User guide

[Back to the README](../README.md)

## Open Lyapunov

From the applications menu (**Lyapunov**). On KDE Plasma, **Meta+Alt+Space**
opens it; change the shortcut in *System Settings → Keyboard → Shortcuts*. On
other desktops, bind any shortcut to `lyapunov-desktop` in your desktop's
keyboard settings. Opening it again brings the existing window forward.

The menu entry also has **Capture this space** (right-click the launcher), which
opens Lyapunov straight on the capture sheet.

## Capture a space

1. Arrange your work as usual, then open Lyapunov.
2. Press **Capture this space** (or Ctrl+N).
3. Give the activity a name and pick a colour.
4. The open applications are listed with their windows. Untick what does not
   belong to this activity. Window titles are shown to help you recognise them;
   they are not saved.
5. Add folders and documents with the buttons, or drop them on the sheet.
6. **Save card**. Nothing on your desktop moved.

An application marked *Lyapunov cannot restart it* has no menu entry (a script,
an AppImage without integration): its windows are arranged when it is open,
but Lyapunov will not start it.

## Resume a space

Select a card (click its band, arrow keys, mouse wheel or a vertical drag),
then **Resume this space** (or Enter, or a double-click on the open card).

The preview lists every step:

- **Will be started**: the application is not open.
- **Open — project not verified**: the application is open, but Lyapunov cannot
  identify its project or document. It leaves existing windows in place by
  default. You may opt in to moving them, after reviewing the preview.
- **Will be opened**: a folder or document.
- **Unavailable**: not installed, not found, or not something a card opens.

Untick any step, then **Resume**. Lyapunov reports each step:
*Window found*, *Application started*, *Already open*, *Opened*, *Unavailable*,
*Failed* or *Left out*, with how many windows were placed. **Close Lyapunov**
gets it out of the way.

To skip the preview for a card, open **More → Edit…** and untick *Show what
will happen before resuming*. The preview still appears if moving already open
windows is enabled.

A card is a saved recipe, not a paused virtual desktop. Lyapunov never closes,
freezes, or autosaves applications. For example, a VS Code window open on
project A stays open on A when you resume a card for project B; Lyapunov does
not yet know how to distinguish those projects or launch B beside A. Add the
project folder to the card to open it explicitly, but check which application
your desktop uses to open that folder.

From the command line, `lyapunov resume CARD` also leaves open windows in
place, including on older cards that saved the move option. Use `--arrange`
only after checking which windows are open.

## Keep cards up to date

**More** (or right-click a card, or the menu key) offers:

- **Edit…**: name, note, colour, folders and documents, resume preferences.
- **Update from what is open…**: capture again and see what changed (added,
  no longer open, moved) before replacing the card.
- **Duplicate**, **Move up**, **Move down**, **Export…**, **Delete…**.
  Deleting removes the card only, never your files or applications.

## Several screens and desktops

Windows that Lyapunov arranges return to the screen and virtual desktop they
were on. If a screen is no longer connected, those windows move to the primary
screen at the same
relative place and size. If a desktop no longer exists, windows stay on the
current one, and the report says so. On sway, i3 and Hyprland, a missing
workspace is created.

## GNOME

GNOME lets an extension, not an application, see and move windows. When
Lyapunov runs on GNOME without it, a line under the title offers **Set up GNOME
integration**. Log out and back in once afterwards. Without the extension,
cards still reopen applications, folders and documents.

## Move cards to another computer

**⋯ → Export all cards…** writes one JSON file. On the other computer,
**⋯ → Import cards…** shows what each card would start and open before adding
it. Window positions are kept; they are fitted to that computer's screens.

## Preferences

**⋯** in the header: language (automatic, English, French), *Reduce motion*,
*Reduce transparency*, the cards folder, diagnostics. Lyapunov also follows
the system's colour scheme, and its animation preference on KDE and GNOME.

## Command line

Everything is also available in a terminal; see `lyapunov --help`:

```sh
lyapunov capture "Paperwork" --place ~/Documents/Taxes --tint lilac
lyapunov resume "Paperwork" --no-arrange
lyapunov export --output ~/cards.json
```
