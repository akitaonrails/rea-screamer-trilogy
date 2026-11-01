# Project rules

This repository builds a self-contained compatibility package that runs the
user's own copy of the 1995 PC **Screamer** trilogy (**Screamer** [1995],
**Screamer 2** [1996], and **Screamer Rally** [1997] by Graffiti / Milestone)
on modern Linux and Windows. It is not a distribution of the games.

## Non-negotiable boundaries

- Never commit, vendor, embed, or publish original executables, `.OVL`
  drivers, media, `.DAT` archives, `.GOG`/`.ISO` CD images, fonts, palettes,
  tracks, car data, or other game assets, or anything copied out of them.
- The user's game folder is strictly read-only. The launcher builds a separate
  private runtime folder from it (`runtime-data/` or OS-appropriate user data
  dir) and mounts large CD images directly from the read-only source folder.
- Never commit a developer's absolute game path. Commands and code take an
  explicit `--game <path>` flag or `SCREAMER_GAME_DIR` environment variable.
- Put local experiments, headless probes, traces, and investigation output
  under `.work/`.
- Running the user's original game binaries for testing is allowed: run them
  from a local folder outside the repository in a headless compositor
  (`gamescope --backend headless`) with a private PipeWire null sink, and
  record notable runtime observations in `docs/RUNTIME_NOTES.md`.

## Product rules

- **Self-contained.** The user installs this package and points it at their
  game directory. Never require manual `SETUP.EXE` runs, manual `Ctrl+F1`
  mapper files, CloneCD `.img` conversion tools, or manual `dosbox.conf`
  editing; whatever the game needs is configured and synthesized automatically.
- **Cross-platform.** Ship native Linux (`x86_64-unknown-linux-gnu`) and
  Windows (`x86_64-pc-windows-gnu`) binaries (`screamer-launcher` /
  `ScreamerLauncher.exe`) and runtime hook libraries (`libscreamer_hooks.so` /
  `screamerhooks.dll`).
- **Reuse open source.** Where a maintained open-source component already does
  the job (`dosbox-staging`, `eframe`/`egui`, SDL3), integrate and configure
  it cleanly. Record each component with its licence and source in
  `THIRD_PARTY.md`.
- **Exact binary verification.** Game-specific patches (`crates/screamer-launcher/src/fixes.rs`)
  and memory hooks (`crates/screamer-hooks`) are keyed to the exact executable
  by SHA-256 hash and verified against original bytes and LE relocation fixup
  tables before patching the private runtime copy (leaving the user's source
  game folder untouched).
- The faithful, unchanged game is available via the `Original 1995` preset,
  while `Enhanced` and `Steam Deck` presets enable SVGA/3dfx, 4K upscaling,
  Hor+ 16:9 widescreen, true 60 FPS with 1.0x physics, and modern analog
  gamepad controls with triggers and rumble.

## Code and commits

- General solutions first. Before patching, find the single bottleneck or matrix
  a whole class of cases goes through (e.g., the 3x3 camera rotation matrix for
  Hor+ widescreen, `smoothed_dt` clamp + SOS timer for 60/120 FPS, `PollControls`
  normalized array for analog controls) and wire it cleanly.
- Rust for our own code. Keep `unsafe` and FFI in narrow modules.
- Unit-test all host-testable logic (Watcom `LE` parser & fixup overlap checker,
  SHA-256 edition detection, binary patch tables, `SCREAMER.CFG` / `SETUP.CFG` /
  `CONTROLS.CFG` / `CHOICE.DAT` binary synthesizers, runtime folder sync,
  `launcher.cfg` parser, UI form state) before each milestone commit.
- Use Conventional Commit subjects. Commit at working milestones.
