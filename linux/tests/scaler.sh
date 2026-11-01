#!/bin/sh
# Tests that screamer-play hands launcher.cfg's scaler to gamescope (-F), for
# every scaler, and the race size (-w/-h): sh linux/tests/scaler.sh
set -u
unset GAMESCOPE_WAYLAND_DISPLAY DRI_PRIME __NV_PRIME_RENDER_OFFLOAD
HERE=$(dirname "$(readlink -f "$0")")
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
export HOME=$T SCREAMER_WAIT=1 SCREAMER_SYSFS=$T/nosys
CFGDIR=$HOME/.config/screamer-launcher
mkdir -p "$CFGDIR"
printf '#!/bin/sh\nexit 0\n' > "$T/ScreamerLauncher"
chmod +x "$T/ScreamerLauncher"

fake_gs() { return 0; }
fake_launch() { echo "$*" > "$T/ran"; }
export SCREAMER_GAMESCOPE_CHECK=fake_gs
export SCREAMER_LAUNCH_CMD=fake_launch
SCREAMER_PLAY_FUNCTIONS_ONLY=1 . "$HERE/../screamer-play"
LAUNCHER=$T/ScreamerLauncher GPU=
fails=0
for scaler in nearest pixel fsr linear; do
  printf 'resolution = 1920x1080\nscaler = %s\n' "$scaler" > "$CFGDIR/launcher.cfg"
  CFG=$CFGDIR/launcher.cfg
  start > /dev/null
  case "$(cat "$T/ran")" in
    *"gamescope -w 1920 -h 1080 -f -S fit -F $scaler --"*) echo "ok   $scaler" ;;
    *) echo "FAIL $scaler: $(cat "$T/ran")"; fails=$((fails + 1)) ;;
  esac
done
printf 'resolution = 1920x1080\n' > "$CFGDIR/launcher.cfg"
start > /dev/null
case "$(cat "$T/ran")" in *"-F nearest --"*) echo "ok   default is nearest" ;; *) echo "FAIL default"; fails=$((fails + 1)) ;; esac
[ "$fails" = 0 ] && echo "all passed" || { echo "$fails failed"; exit 1; }
