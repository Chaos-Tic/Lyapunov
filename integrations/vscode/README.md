# Lyapunov Workspace for VS Code

This optional extension lets Lyapunov distinguish project A from project B.
For a window containing exactly one local folder, it writes a small presence
record in the current user's runtime directory every two seconds. The record
contains only the canonical folder path and the extension host process ID.
No document contents, tabs, window titles or network requests are involved.

Install it locally with `./scripts/install-vscode-extension.sh`. VS Code may
ask you to trust the extension publisher. Restart or reload existing VS Code
windows after installation. A workspace with multiple roots, a remote folder,
or a disabled extension cannot be identified and Lyapunov will not launch a
second window when a prior launch could not be verified.

Only Linux VS Code is supported by this first integration. The extension is
not published to the Marketplace.
