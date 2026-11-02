#!/bin/sh
# Owned-session tests for screamer-play; only harmless sleeps and fake
# launchers run: sh linux/tests/supervision.sh
set -u
unset GAMESCOPE_WAYLAND_DISPLAY DRI_PRIME __NV_PRIME_RENDER_OFFLOAD SCREAMER_LAUNCH_CMD SCREAMER_LAUNCHER
HERE=$(dirname "$(readlink -f "$0")")
T=$(mktemp -d)
export T HOME=$T SCREAMER_SYSFS=$T/nosys
STALE_PID= STALE_START= OTHER_PID= OTHER_START= PLAY_PID=
stop_helper() {
  [ -n "$1" ] || return 0
  stats=$(proc_stat "$1") || return 0
  if [ "${stats#* }" = "$2" ]; then kill "$1" 2>/dev/null || true; fi
  wait "$1" 2>/dev/null || true
}
cleanup() {
  stop_helper "$STALE_PID" "$STALE_START"
  stop_helper "$OTHER_PID" "$OTHER_START"
  [ -n "$PLAY_PID" ] && kill "$PLAY_PID" 2>/dev/null
  [ -f "$T/child.pid" ] && kill "$(cat "$T/child.pid")" 2>/dev/null
  rm -rf "$T"
}
trap cleanup EXIT HUP INT TERM
SCREAMER_PLAY_FUNCTIONS_ONLY=1 . "$HERE/../screamer-play"
fails=0
ok() { echo "ok   $1"; }
fail() { echo "FAIL $1"; fails=$((fails + 1)); }
no_gs() { return 1; }
export SCREAMER_GAMESCOPE_CHECK=no_gs
GPU=

# A live pid with the wrong start time models a reused/stale lock.
sleep 30 & STALE_PID=$!
stats=$(proc_stat "$STALE_PID"); STALE_START=${stats#* }
printf '%s %s\n' "$STALE_PID" "$((STALE_START + 1))" > "$LOCK"
if running; then fail "stale identity reported running"; else ok "stale/reused pid identity refused"; fi
stop_session > /dev/null
if kill -0 "$STALE_PID" 2>/dev/null; then ok "stop did not signal stale/reused pid"; else fail "stale helper was signalled"; fi
stop_helper "$STALE_PID" "$STALE_START"; STALE_PID= STALE_START=

# An old-style lock (pid only) is not trusted either.
echo "$$" > "$LOCK"
if running; then fail "pid-only lock reported running"; else ok "pid-only lock refused"; fi

# An unrelated process with the launcher's name does not own this launch.
ln -s "$(command -v sleep)" "$T/ScreamerLauncher-other"
"$T/ScreamerLauncher-other" 30 & OTHER_PID=$!
stats=$(proc_stat "$OTHER_PID"); OTHER_START=${stats#* }
rm -f "$LOCK"
if running; then fail "unrelated process blocked launch"; else ok "unrelated process does not block"; fi
stop_session > /dev/null
if kill -0 "$OTHER_PID" 2>/dev/null; then ok "unrelated process was not signalled"; else fail "unrelated helper was signalled"; fi
stop_helper "$OTHER_PID" "$OTHER_START"; OTHER_PID= OTHER_START=

# A natural exit removes the lock.
printf '#!/bin/sh\nexit 0\n' > "$T/ScreamerLauncher"; chmod +x "$T/ScreamerLauncher"
LAUNCHER=$T/ScreamerLauncher
start > /dev/null
if [ -f "$LOCK" ]; then fail "natural exit left lock"; else ok "natural supervised exit removes lock"; fi

# stop ends the whole session: the launcher and the game it started.
cat > "$T/ScreamerLauncher" <<'FAKE'
#!/bin/sh
sleep 30 & echo $! > "$T/child.pid"
wait
FAKE
( start > /dev/null ) & PLAY_PID=$!
i=0
while [ $i -lt 50 ] && ! { running && [ -s "$T/child.pid" ]; }; do sleep 0.1; i=$((i + 1)); done
if running; then ok "running while the game plays"; else fail "launch not recorded as running"; fi
stop_session > /dev/null
wait "$PLAY_PID" 2>/dev/null; PLAY_PID=
sleep 0.2
if kill -0 "$(cat "$T/child.pid")" 2>/dev/null; then fail "stop left the game running"; else ok "stop ends the launcher and the game"; fi
if [ -f "$LOCK" ]; then fail "stop left the lock"; else ok "stop removes the lock"; fi

# The settings window: Play (0) and no window (4) continue to the game;
# closing it (3) or any other failure does not.
cat > "$T/ScreamerLauncher" <<'FAKE'
#!/bin/sh
exit "$(cat "$T/rc")"
FAKE
for case in 0:0 4:0 3:1 1:1; do
  echo "${case%:*}" > "$T/rc"
  if settings; then got=0; else got=1; fi
  if [ "$got" = "${case#*:}" ]; then ok "settings exit ${case%:*} -> $got"; else fail "settings exit ${case%:*} gave $got"; fi
done

# Passing anything else on without a launcher is a clear error, not
# `exec ""`.
mkdir -p "$T/alone/bin"; cp "$HERE/../screamer-play" "$T/alone/bin/"
out=$(env -u SCREAMER_PLAY_FUNCTIONS_ONLY sh "$T/alone/bin/screamer-play" --check 2>&1); rc=$?
case "$rc:$out" in
  1:*"ScreamerLauncher not found"*) ok "missing launcher is reported" ;;
  *) fail "missing launcher gave $rc: $out" ;;
esac

[ "$fails" = 0 ] && echo "all passed" || { echo "$fails failed"; exit 1; }
