#!/bin/sh
# Builds the Linux AppImage: ScreamerLauncher, bundled dosbox (compiled from
# vendor/dosbox-staging with static libscreamer_hooks.a), and screamer-play.
#
#   packaging/appimage/build.sh <package-folder> <output-folder>
set -eu
PKG=$(readlink -f "$1"); OUT=$(readlink -f "$2")
HERE=$(dirname "$(readlink -f "$0")")
CACHE=${SCREAMER_CACHE:-$HOME/.cache/screamer-appimage}

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

fetch "$TOOL_URL" "$TOOL_SHA" appimagetool
chmod +x "$CACHE/appimagetool"

APPDIR=$(mktemp -d)/ScreamerLauncher.AppDir
trap 'rm -rf "$(dirname "$APPDIR")"' EXIT
mkdir -p "$APPDIR/usr/lib/screamer/lib"

cp "$PKG/ScreamerLauncher" "$PKG/dosbox" "$PKG/linux/screamer-play" "$PKG/linux/rea-play.sh" "$APPDIR/usr/lib/screamer/"
if [ -d "$PKG/resources" ]; then
  cp -r "$PKG/resources" "$APPDIR/usr/lib/screamer/resources"
fi

# Bundle non-base shared libraries needed by dosbox (SDL2, opusfile, speexdsp, libpng, ogg, opus)
# while leaving host glibc, libGL/mesa/nvidia, X11/Wayland, and ALSA/PipeWire/PulseAudio to the host OS.
for libpath in $(ldd "$PKG/dosbox" | awk '/=> \// {print $3}'); do
  libname=$(basename "$libpath")
  case "$libname" in
    libSDL2*|libopus*|libogg*|libspeexdsp*|libpng*|libz-ng*)
      cp -L "$libpath" "$APPDIR/usr/lib/screamer/lib/$libname"
      ;;
  esac
done

cp "$HERE/AppRun" "$APPDIR/AppRun"
cp "$HERE/screamer-launcher.desktop" "$APPDIR/"
if [ -f "$HERE/screamer-launcher.png" ]; then
  cp "$HERE/screamer-launcher.png" "$APPDIR/"
fi
chmod +x "$APPDIR/AppRun" "$APPDIR/usr/lib/screamer/screamer-play" "$APPDIR/usr/lib/screamer/ScreamerLauncher" "$APPDIR/usr/lib/screamer/dosbox"
mkdir -p "$OUT"
ARCH=x86_64 "$CACHE/appimagetool" --appimage-extract-and-run --no-appstream \
  "$APPDIR" "$OUT/ScreamerLauncher-x86_64.AppImage"
echo "built $OUT/ScreamerLauncher-x86_64.AppImage"
