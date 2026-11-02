#!/bin/sh
# Tests that screamer-play hands launcher.cfg's scaler to gamescope (-F), for
# every scaler, and the race size (-w/-h): sh linux/tests/scaler.sh
set -u
unset GAMESCOPE_WAYLAND_DISPLAY DRI_PRIME __NV_PRIME_RENDER_OFFLOAD
HERE=$(dirname "$(readlink -f "$0")")
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
unset XDG_CONFIG_HOME
export HOME=$T SCREAMER_WAIT=1
CFGDIR=$HOME/.config/screamer-launcher
mkdir -p "$CFGDIR"
printf '#!/bin/sh\nexit 0\n' > "$T/ScreamerLauncher"
chmod +x "$T/ScreamerLauncher"

fake_gs() { return 0; }
fake_launch() { echo "$*" > "$T/ran"; }
SCREAMER_SCREEN=2560x1600 SCREAMER_LAUNCHER=$T/ScreamerLauncher \
  REA_FUNCTIONS_ONLY=1 REA_PLAY_HERE=$HERE/.. . "$HERE/../screamer-play"
REA_TEST=1 REA_GAMESCOPE_CHECK=fake_gs REA_LAUNCH_CMD=fake_launch REA_SYSFS=$T/nosys
REA_GPU=
fails=0
# SCREAMER_LAUNCHER and SCREAMER_SCREEN reach the library; the settings live
# in the state folder.
[ "$REA_EXE" = "$T/ScreamerLauncher" ] && [ "$REA_SCREEN" = 2560x1600 ] \
  && [ "$REA_CFG" = "$CFGDIR/launcher.cfg" ] && echo "ok   environment and paths" \
  || { echo "FAIL environment: $REA_EXE $REA_SCREEN $REA_CFG"; fails=$((fails + 1)); }
for scaler in nearest pixel fsr linear; do
  printf 'resolution = 1920x1080\nscaler = %s\n' "$scaler" > "$CFGDIR/launcher.cfg"
  rea_start > /dev/null
  case "$(cat "$T/ran")" in
    *"gamescope -w 1920 -h 1080 -f -S fit -F $scaler -- sh $REA_INNER $T/ScreamerLauncher --no-window"*) echo "ok   $scaler" ;;
    *) echo "FAIL $scaler: $(cat "$T/ran")"; fails=$((fails + 1)) ;;
  esac
done
printf 'resolution = 1920x1080\n' > "$CFGDIR/launcher.cfg"
rea_start > /dev/null
case "$(cat "$T/ran")" in *"-F fsr --"*) echo "ok   default is fsr" ;; *) echo "FAIL default"; fails=$((fails + 1)) ;; esac
printf 'resolution = 1920x1080\nscaler = sharpest\n' > "$CFGDIR/launcher.cfg"
rea_start > /dev/null
case "$(cat "$T/ran")" in *"-F fsr --"*) echo "ok   an unknown scaler is fsr" ;; *) echo "FAIL unknown scaler"; fails=$((fails + 1)) ;; esac
# The race size: WIDTHxHEIGHT as written (any case/spacing); fullscreen is
# the screen's size; original and anything else keep 1920x1080.
size_case() {  # resolution, screen, expected "-w W -h H"
  printf 'resolution = %s\nscaler = fsr\n' "$1" > "$CFGDIR/launcher.cfg"
  REA_SCREEN=$2 rea_start > /dev/null
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
size_case 0x0 "" "-w 1920 -h 1080"
size_case 0x1080 "" "-w 1920 -h 1080"
size_case 01280x0800 "" "-w 1280 -h 800"
size_case fullscreen 0x0 "-w 1920 -h 1080"
[ "$fails" = 0 ] && echo "all passed" || { echo "$fails failed"; exit 1; }
