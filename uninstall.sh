#!/usr/bin/env bash
# Removes Lyapunov from the current user's account. Cards are kept unless you
# ask, and the applications, folders and documents they list are never touched.
set -euo pipefail
cd "$(dirname "$0")"
data=${XDG_DATA_HOME:-$HOME/.local/share}
config=${XDG_CONFIG_HOME:-$HOME/.config}
uuid=lyapunov@chaos-tic.github.io

pkill -x lyapunov-desktop 2>/dev/null || true
if command -v gnome-extensions >/dev/null && [[ -d $data/gnome-shell/extensions/$uuid ]]; then
    gnome-extensions disable "$uuid" 2>/dev/null || true
fi
if [[ -x $HOME/.local/bin/lyapunov ]]; then "$HOME/.local/bin/lyapunov" integrate --remove >/dev/null || true; fi
rm -rf "$data/gnome-shell/extensions/$uuid"
rm -f "$HOME/.local/bin/lyapunov" "$HOME/.local/bin/lyapunov-desktop" "$config/lyapunov/gnome-trusted"

ask_remove() {
    local what=$1 path=$2 reply=n
    [[ -e $path ]] || return 0
    [[ -t 0 ]] && read -r -p "Also remove $what ($path)? [y/N] " reply
    [[ ${reply,,} == y* ]] && rm -rf "$path"
    return 0
}
ask_remove "your cards" "$data/lyapunov"
ask_remove "your Lyapunov settings" "$config/lyapunov"
printf 'Lyapunov has been removed. Your applications, folders and documents were not touched.\n'
