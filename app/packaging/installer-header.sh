#!/bin/sh
# Study Library — universal Linux installer (any distribution, x86_64 or arm64).
# Usage:  sh StudyLibrary-Installer.run            install for your user (no password needed)
#         sudo sh StudyLibrary-Installer.run       install for everyone on this computer
#         sh StudyLibrary-Installer.run --uninstall
set -e
APP=study-library
NAME="Study Library"

case "$(uname -m)" in
  x86_64|amd64) ARCH=x86_64 ;;
  aarch64|arm64) ARCH=aarch64 ;;
  *) echo "Sorry: this computer's processor ($(uname -m)) is not supported (x86_64 and arm64 are)."; exit 1 ;;
esac

if [ "$(id -u)" = "0" ]; then
  PREFIX=/opt/$APP
  BINDIR=/usr/local/bin
  APPS=/usr/share/applications
  ICONS=/usr/share/icons/hicolor/128x128/apps
else
  DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
  PREFIX="$DATA/$APP-app"
  BINDIR="$HOME/.local/bin"
  APPS="$DATA/applications"
  ICONS="$DATA/icons/hicolor/128x128/apps"
fi

uninstall() {
  rm -rf "$PREFIX"
  rm -f "$BINDIR/$APP" "$APPS/$APP.desktop" "$ICONS/$APP.png"
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" >/dev/null 2>&1 || true
  echo "$NAME was removed. (Your notes and downloads stay in ~/.local/share/study-library.)"
}

if [ "$1" = "--uninstall" ]; then uninstall; exit 0; fi

echo "Installing $NAME for $ARCH into $PREFIX ..."
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
LINE=$(awk '/^__PAYLOAD_BELOW__$/ { print NR + 1; exit 0; }' "$0")
tail -n +"$LINE" "$0" | tar -xzf - -C "$TMP"

mkdir -p "$PREFIX" "$BINDIR" "$APPS" "$ICONS"
cp "$TMP/bin/$ARCH/$APP" "$PREFIX/$APP"
chmod 755 "$PREFIX/$APP"
cp "$TMP/$APP.png" "$PREFIX/$APP.png"
cp "$TMP/$APP.png" "$ICONS/$APP.png"
cp "$0" "$PREFIX/installer.run" 2>/dev/null || true
ln -sf "$PREFIX/$APP" "$BINDIR/$APP"
cat > "$APPS/$APP.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$NAME
Comment=Read the Bible and publications, take notes, watch videos
Exec=$PREFIX/$APP
Icon=$PREFIX/$APP.png
Terminal=false
Categories=Education;Literature;
StartupWMClass=study-library
Keywords=Bible;library;reader;study;
EOF
chmod 644 "$APPS/$APP.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" >/dev/null 2>&1 || true

# The app draws with OpenGL; most desktops have it. Warn if it seems missing.
if command -v ldconfig >/dev/null 2>&1; then
  if ! ldconfig -p 2>/dev/null | grep -qE 'libEGL\.so|libGL\.so'; then
    echo "Note: OpenGL libraries were not found. If the app does not open, install your distribution's Mesa package (e.g. 'mesa-utils', 'libgl1' or 'mesa')."
  fi
fi

# Videos and audio play inside the app with its own decoders; sound goes through the desktop's
# sound server (PulseAudio or PipeWire), present on practically every desktop.
if command -v ldconfig >/dev/null 2>&1 && ! ldconfig -p 2>/dev/null | grep -q 'libpulse-simple\.so'; then
  echo "Note: no PulseAudio/PipeWire sound library was found; videos will play without sound."
fi
echo "Done. Open \"$NAME\" from your applications menu, or run: $APP"
echo "To remove it later: sh \"$PREFIX/installer.run\" --uninstall"
exit 0
__PAYLOAD_BELOW__
