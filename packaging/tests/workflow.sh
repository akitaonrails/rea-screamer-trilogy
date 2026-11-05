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
# The launcher's build.rs runs MinGW's windres (version information, icon
# and manifest, to avoid antivirus false positives): the MSYS2 job installs
# binutils explicitly and checks the packaged exe carries the resources.
if printf '%s\n' "$job" | grep -q 'mingw-w64-x86_64-binutils' \
  && printf '%s\n' "$job" | grep -q 'windres -i "\$package/ScreamerLauncher.exe" -O rc'; then
  echo "ok   windows-packages has windres and checks the exe's resources"
else
  echo "FAIL windows-packages must install mingw-w64-x86_64-binutils and check ScreamerLauncher.exe's resources with windres"
  exit 1
fi
