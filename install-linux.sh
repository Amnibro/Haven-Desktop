#!/usr/bin/env bash
# Haven Desktop (Tauri) - Linux install / uninstall.
#
# Installs the release AppImage under ~/.local/share/haven-desktop with a menu
# entry and icon, and checks the runtime libraries the app needs on this distro.
#
#   ./install-linux.sh                  # AppImage next to this script, in ~/Downloads, or the latest release
#   ./install-linux.sh Haven_x.y.z_amd64.AppImage
#   ./install-linux.sh --bare src-tauri/target/release/haven-desktop
#                                       # a plain binary (uses the system WebKitGTK); goes to ~/.local/bin
#   ./install-linux.sh --uninstall
#
# The AppImage bundles libayatana-appindicator, so the tray works out of the box.
# A bare binary needs the distro package for a tray icon; without it the app
# still starts, just without a tray.

set -euo pipefail

REPO="${HAVEN_REPO:-Amnibro/Haven-Desktop}"
INSTALL_DIR="$HOME/.local/share/haven-desktop"
BIN_DIR="$HOME/.local/bin"
ICON_DIR="$HOME/.local/share/icons/hicolor"
DESKTOP_FILE="$HOME/.local/share/applications/haven-desktop.desktop"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

MODE="appimage"
SOURCE=""
for arg in "$@"; do
  case "$arg" in
    --bare) MODE="bare" ;;
    --appimage) MODE="appimage" ;;
    --uninstall) MODE="uninstall" ;;
    -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
    *) SOURCE="$arg" ;;
  esac
done

have() { command -v "$1" >/dev/null 2>&1; }
has_lib() { ldconfig -p 2>/dev/null | grep -q "$1" || [ -n "$(ls /usr/lib*/$1* /usr/lib/*/$1* 2>/dev/null)" ]; }

# Print the distro's install command for a list of packages, keyed by package manager.
pkg_hint() {
  local what="$1" pacman_pkgs="$2" apt_pkgs="$3" dnf_pkgs="$4"
  if have pacman; then echo "    sudo pacman -S --needed $pacman_pkgs   # $what"
  elif have apt-get; then echo "    sudo apt install $apt_pkgs   # $what"
  elif have dnf; then echo "    sudo dnf install $dnf_pkgs   # $what"
  else echo "    install: $what"; fi
}

# ---------------------------------------------------------------- uninstall
if [ "$MODE" = "uninstall" ]; then
  rm -rf "$INSTALL_DIR" "$DESKTOP_FILE" "$BIN_DIR/haven-desktop"
  rm -f "$ICON_DIR"/*/apps/haven-desktop.png
  have update-desktop-database && update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
  echo "Haven Desktop removed. Settings in ~/.local/share/com.haven.desktop were kept."
  exit 0
fi

# ---------------------------------------------------------------- dependency checks
MISSING=()
if [ "$MODE" = "bare" ]; then
  has_lib libwebkit2gtk-4.1.so || MISSING+=("$(pkg_hint 'WebKitGTK 4.1, required' webkit2gtk-4.1 libwebkit2gtk-4.1-0 webkit2gtk4.1)")
  has_lib libayatana-appindicator3.so || has_lib libappindicator3.so \
    || MISSING+=("$(pkg_hint 'tray icon (optional, app runs without it)' libayatana-appindicator libayatana-appindicator3-1 libayatana-appindicator-gtk3)")
fi

# ---------------------------------------------------------------- locate the payload
if [ -z "$SOURCE" ]; then
  if [ "$MODE" = "bare" ]; then
    for f in "$SCRIPT_DIR"/src-tauri/target/release/haven-desktop "$SCRIPT_DIR"/haven-desktop; do
      [ -x "$f" ] && { SOURCE="$f"; break; }
    done
  else
    for f in "$SCRIPT_DIR"/Haven*.AppImage "$HOME"/Downloads/Haven*.AppImage; do
      [ -f "$f" ] && { SOURCE="$f"; break; }
    done
    if [ -z "$SOURCE" ]; then
      echo "No AppImage found locally; fetching the latest release from $REPO ..."
      URL=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep -o '"browser_download_url": *"[^"]*\.AppImage"' | head -1 | sed 's/.*"\(http[^"]*\)"/\1/')
      [ -n "$URL" ] || { echo "Could not find an AppImage on the latest release."; exit 1; }
      mkdir -p "$INSTALL_DIR"
      SOURCE="$INSTALL_DIR/$(basename "$URL")"
      curl -fL --progress-bar -o "$SOURCE" "$URL"
    fi
  fi
fi
[ -n "$SOURCE" ] && [ -f "$SOURCE" ] || { echo "Nothing to install: pass the AppImage or binary path."; exit 1; }

# ---------------------------------------------------------------- install
mkdir -p "$INSTALL_DIR" "$BIN_DIR" "$(dirname "$DESKTOP_FILE")"
ICON_SRC=""
if [ "$MODE" = "bare" ]; then
  install -m 755 "$SOURCE" "$BIN_DIR/haven-desktop"
  EXEC="$BIN_DIR/haven-desktop"
  for f in "$SCRIPT_DIR"/src-tauri/icons/icon.png "$SCRIPT_DIR"/assets/icon.png; do
    [ -f "$f" ] && { ICON_SRC="$f"; break; }
  done
else
  [ "$(readlink -f "$SOURCE")" = "$INSTALL_DIR/Haven.AppImage" ] || cp "$SOURCE" "$INSTALL_DIR/Haven.AppImage"
  chmod +x "$INSTALL_DIR/Haven.AppImage"
  if has_lib libfuse.so.2; then
    EXEC="$INSTALL_DIR/Haven.AppImage"
  else
    # No FUSE 2 on this system: unpack once so the launcher never needs it.
    echo "libfuse2 not found; extracting the AppImage instead of mounting it."
    (cd "$INSTALL_DIR" && rm -rf squashfs-root && ./Haven.AppImage --appimage-extract >/dev/null)
    EXEC="$INSTALL_DIR/squashfs-root/AppRun"
  fi
  # A symlink on PATH so `haven-desktop` works from a terminal too.
  ln -sfn "$EXEC" "$BIN_DIR/haven-desktop"
  ICON_SRC=$( (cd "$INSTALL_DIR" && ./Haven.AppImage --appimage-extract 'usr/share/icons/hicolor/128x128/apps/*.png' >/dev/null 2>&1; ls squashfs-root/usr/share/icons/hicolor/128x128/apps/*.png 2>/dev/null | head -1) || true)
  [ -n "$ICON_SRC" ] && ICON_SRC="$INSTALL_DIR/$ICON_SRC"
  [ -z "$ICON_SRC" ] && [ -f "$SCRIPT_DIR/src-tauri/icons/icon.png" ] && ICON_SRC="$SCRIPT_DIR/src-tauri/icons/icon.png"
fi

if [ -n "$ICON_SRC" ] && [ -f "$ICON_SRC" ]; then
  for size in 32 64 128 256; do
    mkdir -p "$ICON_DIR/${size}x${size}/apps"
    if have magick; then magick "$ICON_SRC" -resize "${size}x${size}" "$ICON_DIR/${size}x${size}/apps/haven-desktop.png"
    elif have convert; then convert "$ICON_SRC" -resize "${size}x${size}" "$ICON_DIR/${size}x${size}/apps/haven-desktop.png"
    else cp "$ICON_SRC" "$ICON_DIR/${size}x${size}/apps/haven-desktop.png"; fi
  done
fi

cat > "$DESKTOP_FILE" <<DESKTOP
[Desktop Entry]
Name=Haven
Comment=Haven Desktop — private self-hosted chat
Exec=$EXEC %U
Icon=haven-desktop
Type=Application
Categories=Network;Chat;InstantMessaging;
Terminal=false
StartupWMClass=haven-desktop
DESKTOP
have update-desktop-database && update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
have gtk-update-icon-cache && gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true

echo "Installed Haven Desktop."
echo "  Launcher: $EXEC"
echo "  Menu entry: $DESKTOP_FILE"
if [ ${#MISSING[@]} -gt 0 ]; then
  echo
  echo "This system is missing libraries the bare binary uses:"
  printf '%s\n' "${MISSING[@]}"
fi
echo "Uninstall with: $0 --uninstall"
