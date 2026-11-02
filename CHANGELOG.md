# Changelog

## Unreleased

- Fix: the runtime folder no longer symlinks files and whole directories into the user's game folder, which let the game write into it. Files are hard-linked (code and drivers only) or copied one at a time into real directories, launcher writes replace whole files, and a manifest of size and modification time brings changed source files in again while keeping the game's saves and replays. Runtime folders from older versions are converted on the next launch.
- Refuse a runtime or saves folder that lies inside the game folder (or contains it), resolving symlinks, `..` and folders that do not exist yet, before anything is written.
- Save sync between the runtime folder and the saves folder no longer overwrites unconditionally. The runtime folder records each save's size and modification time as of the last sync (`.screamer-saves`): at launch a save changed in the saves folder wins (even a restored older backup), otherwise one changed in the runtime folder (a session that ended before its saves were copied back) is copied out; at exit a save is copied back whenever its contents differ. Saves with no recorded state yet: the newer one wins.
- Fix (Linux): `resolution = fullscreen` reached gamescope as `-w fullscreen -h fullscreen`. `screamer-play` now validates `WIDTHxHEIGHT` (any case or spacing), uses the screen size for `fullscreen` (`SCREAMER_SCREEN` overrides the xrandr probe) and keeps 1920x1080 for `original` and anything malformed.
- Fix: a resolution above 1920x1080 (e.g. `3840x2160`) was silently turned into fullscreen when `launcher.cfg` was read. It is now kept as written and shown as its own choice in the Picture tab; only when such a size (above 1920x1080) is larger than the screen (DOSBox cannot open such a window) does the launcher play fullscreen, with a warning. Sizes up to 1080p are passed on unchanged as before, e.g. 1920x1080 on a Steam Deck in Game Mode.
- Fix (Linux): with no `scaler` saved, `screamer-play` defaulted gamescope to `nearest` while `launcher.cfg`'s template and the settings window default to `fsr`; it now uses `fsr` too.
- Fix: the settings window read the monitor size in logical points, so on a scaled display (e.g. a Steam Deck or a 4K screen at 150–200%) it missed the Steam Deck and picked the wrong zoom. The size is now converted to pixels.
- When no settings window can be made (no display or OpenGL), the launcher now plays with the saved settings (and `--settings` exits with code 4, which `screamer-play` already treats as play) instead of stopping with an error. If the window opened and then failed, the game does not start, since it would use settings the player may have just changed. Warnings and errors are written to `launcher.log` next to `launcher.cfg`; on Windows a fatal error is also shown in a message box.
- Linux: `screamer-play` now runs each game in its own session and records its pid with its start time, so `status` and `stop` never trust a stale or reused pid and `stop` ends the whole game (gamescope, launcher and DOSBox). Every launch waits for the game to end. Passing an unknown command with no launcher found is now a clear error instead of `exec ""`. Add `linux/README.md` and the `linux/tests/supervision.sh` test (run in CI).
- Docs: the `rumble` setting is described as what it does (rumble when braking hard in a race), not kerbs, gravel, skids and walls; `--controllers` is described as listing the connected pads (it never logged axis or button events).
- Fix: a symbolic link in the game folder (or a linked `GLIDE2X.OVL` fallback) was hard-linked as the link itself, leaving a broken or user-folder-pointing link in the runtime. The file it points to is now linked or copied.
- Fix (Unix): files copied into the runtime and saves folders were made writable by everyone (mode 0666); only the owner's write permission is added now. A read-only file in the game folder is copied instead of hard-linked.
- Fix: the runtime/saves overlap check refused to launch when a folder could not be canonicalized for a reason other than "not found" (Windows RAM disks, `subst` drives, some network shares); such paths are now compared as written, normalized.
- Fix: runtime staging is more forgiving of unusual game folders. When a CD image is mounted from the game folder, every CD-image file (`.iso`, `.gog`, `.bin`, `.img`, `.cue`, `.ins`, `.ccd`, `.sub`, `.mds`, `.mdf`) and every file its cue sheet names (any case, DOS `\` and `..\` paths) stays there instead of being copied. An unreadable subfolder is skipped with a warning in `launcher.log` instead of stopping the launch, and a symbolic-link loop is not followed.
- CI: check dependencies with `cargo audit` (RustSec advisories) and `cargo deny check` (licences, sources; `deny.toml`).

## 0.6.0 - 2026-11-01

- Add optional E6 extended draw distance to the Enhanced and Steam Deck presets: managed animated objects are visible 20% farther away in Screamer, while Screamer 2 and Screamer Rally use twice the original scenery/object admission radius across all supported renderers.
- Verify every E6 edit against the exact executable SHA-256, original bytes, and LE relocation records; keep the user's game directory read-only and test Original-versus-Enhanced runtime copies byte for byte.
- Add opt-in admission-count telemetry for runtime verification without affecting normal play.
- Retain the in-place Screamer 2/Rally implementation that completed a Rally race segment; discard experimental out-of-line guard variants that reproduced the first-render exit.
- Build Linux zip, AppImage, Windows zip, and recursive corresponding-source archives in GitHub Actions with pinned actions, checksums, and build provenance attestations.
- Link the controller/telemetry hooks directly into the bundled DOSBox on Linux and Windows, and package the required shared runtime libraries.

## 0.5.0

- Initial self-contained launcher for Screamer, Screamer 2, and Screamer Rally with exact-edition fixes, enhanced display presets, modern controller support, and bundled DOSBox Staging.
