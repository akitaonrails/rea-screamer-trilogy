#!/bin/sh
# Owned-session tests for screamer-play on rea-play.sh, as Screamer
# configures it; only harmless sleeps and fake launchers run:
# sh linux/tests/supervision.sh
set -u
unset GAMESCOPE_WAYLAND_DISPLAY DRI_PRIME __NV_PRIME_RENDER_OFFLOAD SCREAMER_LAUNCHER XDG_CONFIG_HOME
HERE=$(dirname "$(readlink -f "$0")")
T=$(mktemp -d)
export T
. "$HERE/lib/isolate.sh"
STALE_PID= STALE_START= OTHER_PID= OTHER_START= PLAY_PID=
stop_helper() {
  [ -n "$1" ] || return 0
  stats=$(rea_proc_stat "$1") || return 0
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
REA_FUNCTIONS_ONLY=1 REA_PLAY_HERE=$HERE/.. . "$HERE/../screamer-play"
fails=0
ok() { echo "ok   $1"; }
fail() { echo "FAIL $1"; fails=$((fails + 1)); }
no_gs() { return 1; }
REA_TEST=1 REA_GAMESCOPE_CHECK=no_gs REA_SYSFS=$T/nosys
REA_GPU=

# A live pid with the wrong start time models a reused/stale lock.
sleep 30 & STALE_PID=$!
stats=$(rea_proc_stat "$STALE_PID"); STALE_START=${stats#* }
printf '%s %s\n' "$STALE_PID" "$((STALE_START + 1))" > "$REA_LOCK"
if rea_running; then fail "stale identity reported running"; else ok "stale/reused pid identity refused"; fi
rea_stop > /dev/null
if kill -0 "$STALE_PID" 2>/dev/null; then ok "stop did not signal stale/reused pid"; else fail "stale helper was signalled"; fi
stop_helper "$STALE_PID" "$STALE_START"; STALE_PID= STALE_START=

# A lock from an older screamer-play (pid only): running while that pid
# lives, and stop ends it; a dead one is ignored.
sleep 30 & OLD_PID=$!
echo "$OLD_PID" > "$REA_LOCK"
if rea_running; then ok "live pid-only lock is running"; else fail "live pid-only lock not running"; fi
rea_stop > /dev/null
wait "$OLD_PID" 2>/dev/null
if kill -0 "$OLD_PID" 2>/dev/null; then fail "stop left the old game running"; else ok "stop ends an old-style lock's game"; fi
echo "$OLD_PID" > "$REA_LOCK"
if rea_running; then fail "dead pid-only lock reported running"; else ok "dead pid-only lock ignored"; fi
rm -f "$REA_LOCK"

# An unrelated process with the launcher's name does not own this launch.
ln -s "$(command -v sleep)" "$T/ScreamerLauncher-other"
"$T/ScreamerLauncher-other" 30 & OTHER_PID=$!
stats=$(rea_proc_stat "$OTHER_PID"); OTHER_START=${stats#* }
rm -f "$REA_LOCK"
if rea_running; then fail "unrelated process blocked launch"; else ok "unrelated process does not block"; fi
rea_stop > /dev/null
if kill -0 "$OTHER_PID" 2>/dev/null; then ok "unrelated process was not signalled"; else fail "unrelated helper was signalled"; fi
stop_helper "$OTHER_PID" "$OTHER_START"; OTHER_PID= OTHER_START=

# A natural exit removes the lock.
printf '#!/bin/sh\nexit 0\n' > "$T/ScreamerLauncher"; chmod +x "$T/ScreamerLauncher"
REA_EXE=$T/ScreamerLauncher
rea_start > /dev/null
if [ -f "$REA_LOCK" ]; then fail "natural exit left lock"; else ok "natural supervised exit removes lock"; fi

# stop ends the whole session: the launcher and the game it started.
cat > "$T/ScreamerLauncher" <<'FAKE'
#!/bin/sh
sleep 30 & echo $! > "$T/child.pid"
wait
FAKE
( rea_start > /dev/null ) & PLAY_PID=$!
i=0
while [ $i -lt 50 ] && ! { rea_running && [ -s "$T/child.pid" ]; }; do sleep 0.1; i=$((i + 1)); done
if rea_running; then ok "running while the game plays"; else fail "launch not recorded as running"; fi
rea_stop > /dev/null
wait "$PLAY_PID" 2>/dev/null; PLAY_PID=
sleep 0.2
if kill -0 "$(cat "$T/child.pid")" 2>/dev/null; then fail "stop left the game running"; else ok "stop ends the launcher and the game"; fi
if [ -f "$REA_LOCK" ]; then fail "stop left the lock"; else ok "stop removes the lock"; fi

# Interrupting screamer-play ends the session and waits for it before the
# lock goes.
rm -f "$T/child.pid"
cat > "$T/ScreamerLauncher" <<'FAKE'
#!/bin/sh
trap 'sleep 0.3; echo done > "$T/saved"; exit 0' TERM
sleep 30 & echo $! > "$T/child.pid"
wait
FAKE
rm -f "$T/saved"
( rea_start > /dev/null ) & PLAY_PID=$!
i=0
while [ $i -lt 50 ] && ! { rea_running && [ -s "$T/child.pid" ]; }; do sleep 0.1; i=$((i + 1)); done
kill -TERM "$PLAY_PID"
wait "$PLAY_PID" 2>/dev/null; PLAY_PID=
if [ -f "$T/saved" ]; then ok "interrupt waits for the game to finish"; else fail "interrupt did not wait for the game"; fi
if [ -f "$REA_LOCK" ]; then fail "interrupt left the lock"; else ok "interrupt removes the lock"; fi

# The settings window: Play (0) and no window (4) continue to the game;
# closing it (3) or any other failure does not.
cat > "$T/ScreamerLauncher" <<'FAKE'
#!/bin/sh
exit "$(cat "$T/rc")"
FAKE
for case in 0:0 4:0 3:1 1:1; do
  echo "${case%:*}" > "$T/rc"
  if rea_settings > /dev/null; then got=0; else got=1; fi
  if [ "$got" = "${case#*:}" ]; then ok "settings exit ${case%:*} -> $got"; else fail "settings exit ${case%:*} gave $got"; fi
done

# Passing anything else on without a launcher is a clear error, not
# `exec ""`.
mkdir -p "$T/alone/bin"; cp "$HERE/../screamer-play" "$HERE/../../sdk/rea-play.sh" "$T/alone/bin/"
out=$(env -u REA_FUNCTIONS_ONLY -u REA_PLAY_HERE sh "$T/alone/bin/screamer-play" --check 2>&1); rc=$?
case "$rc:$out" in
  1:*"ScreamerLauncher not found"*) ok "missing launcher is reported" ;;
  *) fail "missing launcher gave $rc: $out" ;;
esac

# Dispatch: a game name opens the window with it selected, then plays;
# anything the helper does not know goes to the launcher.
cat > "$T/ScreamerLauncher" <<'FAKE'
#!/bin/sh
echo "$*" >> "$T/calls"
FAKE
fake_launch() { echo "launch $*" >> "$T/calls"; }
rm -f "$T/calls"
( REA_LAUNCH_CMD=fake_launch; rea_main rally > /dev/null )
case "$(cat "$T/calls")" in
  "--settings --linux rally
launch "*" sh $REA_INNER $T/ScreamerLauncher --no-window --linux") ok "a game name: window, then play" ;;
  *) fail "game name dispatch: $(cat "$T/calls")" ;;
esac
rm -f "$T/calls"
( rea_main --check --game x > /dev/null )
if [ "$(cat "$T/calls")" = "--check --game x" ]; then ok "other words go to the launcher"; else fail "pass-through: $(cat "$T/calls")"; fi
rm -f "$T/calls"
( REA_LAUNCH_CMD=fake_launch; rea_main --play 2 > /dev/null )
case "$(cat "$T/calls")" in
  "launch "*" sh $REA_INNER $T/ScreamerLauncher --no-window --linux 2") ok "--play passes its game on" ;;
  *) fail "--play: $(cat "$T/calls")" ;;
esac
if [ "$(rea_main status)" = idle ]; then ok "status"; else fail "status"; fi

real_dirs_untouched || fails=$((fails + 1))
[ "$fails" = 0 ] && echo "all passed" || { echo "$fails failed"; exit 1; }
