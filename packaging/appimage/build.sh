#!/bin/sh
# Builds the Linux AppImage: ScreamerLauncher, libscreamer_hooks.so,
# screamer-play, and portable dosbox-staging v0.82.2.
#
#   packaging/appimage/build.sh <package-folder> <output-folder>
set -eu
PKG=$(readlink -f "$1"); OUT=$(readlink -f "$2")
HERE=$(dirname "$(readlink -f "$0")")
CACHE=${SCREAMER_CACHE:-$HOME/.cache/screamer-appimage}

DOSBOX_VERSION=0.82.2
DOSBOX_URL=https://github.com/dosbox-staging/dosbox-staging/releases/download/v$DOSBOX_VERSION/dosbox-staging-linux-x86_64-v$DOSBOX_VERSION.tar.xz
DOSBOX_SHA=c1abef833a44abce00e3d0f7bb9faeddc7fb66294b4ea4bcfe1b1c5a46ec6e36
TOOL_URL=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
TOOL_SHA=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0

fetch() { # url sha file
  mkdir -p "$CACHE"
  if [ ! -f "$CACHE/$3" ]; then
    curl -sfL "$1" -o "$CACHE/$3.part"
    mv "$CACHE/$3.part" "$CACHE/$3"
  fi
  echo "$2  $CACHE/$3" | sha256sum -c --quiet || { echo "checksum mismatch: $3"; rm -f "$CACHE/$3"; exit 1; }
}

fetch "$DOSBOX_URL" "$DOSBOX_SHA" dosbox-staging.tar.xz
fetch "$TOOL_URL" "$TOOL_SHA" appimagetool
chmod +x "$CACHE/appimagetool"

APPDIR=$(mktemp -d)/ScreamerLauncher.AppDir
trap 'rm -rf "$(dirname "$APPDIR")"' EXIT
mkdir -p "$APPDIR/usr/lib/screamer" "$APPDIR/usr/lib/dosbox-staging"
tar -xJf "$CACHE/dosbox-staging.tar.xz" -C "$APPDIR/usr/lib/dosbox-staging" --strip-components=1

cp "$PKG/ScreamerLauncher" "$PKG/libscreamer_hooks.so" "$PKG/linux/screamer-play" "$APPDIR/usr/lib/screamer/"
cp "$HERE/AppRun" "$APPDIR/AppRun"
cp "$HERE/screamer-launcher.desktop" "$APPDIR/"
if [ -f "$HERE/screamer-launcher.png" ]; then
  cp "$HERE/screamer-launcher.png" "$APPDIR/"
fi
chmod +x "$APPDIR/AppRun" "$APPDIR/usr/lib/screamer/screamer-play" "$APPDIR/usr/lib/screamer/ScreamerLauncher"
mkdir -p "$OUT"
ARCH=x86_64 "$CACHE/appimagetool" --appimage-extract-and-run --no-appstream \
  "$APPDIR" "$OUT/ScreamerLauncher-x86_64.AppImage"
echo "built $OUT/ScreamerLauncher-x86_64.AppImage (dosbox-staging $DOSBOX_VERSION)"
