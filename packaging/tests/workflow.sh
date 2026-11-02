#!/bin/sh
# The Windows zip's programs are import-checked where they are packaged
# (MSYS2), after the DLLs are copied in and before the zip is made.
set -u
cd "$(dirname "$0")/../.." || exit 1
job=$(awk '
  /^  windows-packages:/ { on = 1; print; next }
  on && /^  [a-z]/ { exit }
  on { print }
' .github/workflows/build.yml)
check=$(printf '%s\n' "$job" | grep -n 'sdk/packaging/check-imports.sh "\$package"/\*\.exe "\$package"/\*\.dll' | cut -d: -f1)
copy=$(printf '%s\n' "$job" | grep -n 'ntldd -R' | cut -d: -f1)
zip=$(printf '%s\n' "$job" | grep -n 'zip -r' | cut -d: -f1)
if [ -n "$check" ] && [ -n "$copy" ] && [ -n "$zip" ] && [ "$copy" -lt "$check" ] && [ "$check" -lt "$zip" ]; then
  echo "ok   windows-packages import-checks the packaged exe and DLLs"
else
  echo "FAIL windows-packages must run check-imports.sh on \$package/*.exe and *.dll after the DLL copy, before the zip"
  exit 1
fi
