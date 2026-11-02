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
# The race size: WIDTHxHEIGHT as written (any case/spacing); fullscreen is
# the screen's size; original and anything else keep 1920x1080.
size_case() {  # resolution, screen, expected "-w W -h H"
  printf 'resolution = %s\nscaler = fsr\n' "$1" > "$CFGDIR/launcher.cfg"
  SCREAMER_SCREEN=$2 start > /dev/null
  case "$(cat "$T/ran")" in
    *"gamescope $3 -f "*) echo "ok   size '$1' (screen '$2')" ;;
    *) echo "FAIL size '$1' (screen '$2'): $(cat "$T/ran")"; fails=$((fails + 1)) ;;
  esac
}
size_case 2560x1440 "" "-w 2560 -h 1440"
size_case " 3840X2160" "" "-w 3840 -h 2160"
size_case fullscreen 2560x1600 "-w 2560 -h 1600"
size_case fullscreen bogus "-w 1920 -h 1080"
size_case original 2560x1600 "-w 1920 -h 1080"
size_case 1920x "" "-w 1920 -h 1080"
size_case abc "" "-w 1920 -h 1080"
[ "$fails" = 0 ] && echo "all passed" || { echo "$fails failed"; exit 1; }
