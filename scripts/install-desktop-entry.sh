#!/usr/bin/env bash
# Install the prog-rs KRunner launcher entry and icon into the user's KDE
# environment. Idempotent; safe to re-run after changes or reinstalls.
#
# Uninstall:
#   rm ~/.local/share/applications/prog-rs.desktop \
#      ~/.local/share/icons/hicolor/scalable/apps/prog-rs.svg
#   kbuildsycoca6
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

BIN="$(command -v prog-rs || true)"
if [[ -z "$BIN" ]]; then
    echo "error: prog-rs not on PATH; run 'cargo install --path .' first" >&2
    exit 1
fi

mkdir -p ~/.local/share/applications ~/.local/share/icons/hicolor/scalable/apps

# The repo template keeps the portable `Exec=prog-rs`; the installed copy gets
# the absolute path so KDE does not depend on the session's PATH.
sed "s|^Exec=prog-rs$|Exec=$BIN|" "$root/assets/prog-rs.desktop" \
    > ~/.local/share/applications/prog-rs.desktop
desktop-file-validate ~/.local/share/applications/prog-rs.desktop

cp "$root/assets/prog-rs.svg" ~/.local/share/icons/hicolor/scalable/apps/prog-rs.svg
# KDE scans this dir itself; refresh the GTK cache only when a theme exists.
if [[ -f ~/.local/share/icons/hicolor/index.theme ]]; then
    gtk-update-icon-cache -q ~/.local/share/icons/hicolor
fi

# Rebuild KDE's service cache so KRunner picks up the new entry.
kbuildsycoca6

echo "installed: $BIN -> ~/.local/share/applications/prog-rs.desktop"
