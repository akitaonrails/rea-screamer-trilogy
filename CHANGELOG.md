# Changelog

## Unreleased

- Fix: the runtime folder no longer symlinks files and whole directories into the user's game folder, which let the game write into it. Files are hard-linked (code and drivers only) or copied one at a time into real directories, launcher writes replace whole files, and a manifest of size and modification time brings changed source files in again while keeping the game's saves and replays. Runtime folders from older versions are converted on the next launch.
- Refuse a runtime or saves folder that lies inside the game folder (or contains it), resolving symlinks, `..` and folders that do not exist yet, before anything is written.

## 0.6.0 - 2026-11-01

- Add optional E6 extended draw distance to the Enhanced and Steam Deck presets: managed animated objects are visible 20% farther away in Screamer, while Screamer 2 and Screamer Rally use twice the original scenery/object admission radius across all supported renderers.
- Verify every E6 edit against the exact executable SHA-256, original bytes, and LE relocation records; keep the user's game directory read-only and test Original-versus-Enhanced runtime copies byte for byte.
- Add opt-in admission-count telemetry for runtime verification without affecting normal play.
- Retain the in-place Screamer 2/Rally implementation that completed a Rally race segment; discard experimental out-of-line guard variants that reproduced the first-render exit.
- Build Linux zip, AppImage, Windows zip, and recursive corresponding-source archives in GitHub Actions with pinned actions, checksums, and build provenance attestations.
- Link the controller/telemetry hooks directly into the bundled DOSBox on Linux and Windows, and package the required shared runtime libraries.

## 0.5.0

- Initial self-contained launcher for Screamer, Screamer 2, and Screamer Rally with exact-edition fixes, enhanced display presets, modern controller support, and bundled DOSBox Staging.
