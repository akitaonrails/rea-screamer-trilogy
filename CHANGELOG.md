# Changelog

## Unreleased

- Fix: the runtime folder no longer symlinks files and whole directories into the user's game folder, which let the game write into it. Files are hard-linked (code and drivers only) or copied one at a time into real directories, launcher writes replace whole files, and a manifest of size and modification time brings changed source files in again while keeping the game's saves and replays. Runtime folders from older versions are converted on the next launch.
- Refuse a runtime or saves folder that lies inside the game folder (or contains it), resolving symlinks, `..` and folders that do not exist yet, before anything is written.
- Save sync between the runtime folder and the saves folder no longer overwrites unconditionally. The runtime folder records each save's size and modification time as of the last sync (`.screamer-saves`): at launch a save changed in the saves folder wins (even a restored older backup), otherwise one changed in the runtime folder (a session that ended before its saves were copied back) is copied out; at exit a save is copied back whenever its contents differ. Saves with no recorded state yet: the newer one wins.
- Fix (Linux): `resolution = fullscreen` reached gamescope as `-w fullscreen -h fullscreen`. `screamer-play` now validates `WIDTHxHEIGHT` (any case or spacing), uses the screen size for `fullscreen` (`SCREAMER_SCREEN` overrides the xrandr probe) and keeps 1920x1080 for `original` and anything malformed.
- Fix: a resolution above 1920x1080 (e.g. `3840x2160`) was silently turned into fullscreen when `launcher.cfg` was read. It is now kept as written and shown as its own choice in the Picture tab; only when it is larger than the screen (DOSBox cannot open such a window) does the launcher play fullscreen, with a warning.

## 0.6.0 - 2026-11-01

- Add optional E6 extended draw distance to the Enhanced and Steam Deck presets: managed animated objects are visible 20% farther away in Screamer, while Screamer 2 and Screamer Rally use twice the original scenery/object admission radius across all supported renderers.
- Verify every E6 edit against the exact executable SHA-256, original bytes, and LE relocation records; keep the user's game directory read-only and test Original-versus-Enhanced runtime copies byte for byte.
- Add opt-in admission-count telemetry for runtime verification without affecting normal play.
- Retain the in-place Screamer 2/Rally implementation that completed a Rally race segment; discard experimental out-of-line guard variants that reproduced the first-render exit.
- Build Linux zip, AppImage, Windows zip, and recursive corresponding-source archives in GitHub Actions with pinned actions, checksums, and build provenance attestations.
- Link the controller/telemetry hooks directly into the bundled DOSBox on Linux and Windows, and package the required shared runtime libraries.

## 0.5.0

- Initial self-contained launcher for Screamer, Screamer 2, and Screamer Rally with exact-edition fixes, enhanced display presets, modern controller support, and bundled DOSBox Staging.
