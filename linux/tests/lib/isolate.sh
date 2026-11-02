# Sourced by every test right after it makes $T: points HOME and the XDG
# folders at $T, so screamer-play's state (play.log, running.pid,
# launcher.cfg) never lands in the player's real folders, and
# real_dirs_untouched fails the test if those folders changed anyway.
_iso_cfg=${XDG_CONFIG_HOME:-$HOME/.config}/screamer-launcher
_iso_data=${XDG_DATA_HOME:-$HOME/.local/share}/screamer-launcher
_iso_snapshot() {
  for _iso_d in "$_iso_cfg" "$_iso_data"; do
    echo "$_iso_d"
    [ -e "$_iso_d" ] && find "$_iso_d" -maxdepth 2 -exec stat -c '%n %s %Y' {} + 2>/dev/null
  done
  return 0
}
_iso_before=$(_iso_snapshot)
export HOME="$T" XDG_CONFIG_HOME="$T/.config" XDG_DATA_HOME="$T/.local/share" \
  XDG_STATE_HOME="$T/.local/state" XDG_CACHE_HOME="$T/.cache"
real_dirs_untouched() {
  if [ "$(_iso_snapshot)" = "$_iso_before" ]; then
    echo "ok   the player's real screamer-launcher folders are untouched"
  else
    echo "FAIL a test changed $_iso_cfg or $_iso_data"
    return 1
  fi
}
