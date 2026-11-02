# Project rules

This repository builds a self-contained compatibility package that runs the
player's own copy of the **Screamer** trilogy (**Screamer** [1995],
**Screamer 2** [1996] and **Screamer Rally** [1997], by Graffiti / Milestone,
published by Virgin Interactive) on modern Linux and Windows. It is not a
distribution of the games.

## Read first

The SDK's rules are binding here: [rea-launcher-sdk](https://github.com/akitaonrails/rea-launcher-sdk)
`docs/STANDARDS.md`, and its playbooks in `docs/playbooks/`:

- `new-launcher.md` (order of work), `sdk-change.md` (SDK bumps; never
  patch `sdk/` or copy SDK code here, report gaps to the SDK),
- `community-components.md` (research community fixes first, record them
  in `docs/RESEARCH.md`; pin, bundle or download on request),
- `binary-patching.md` (exact builds by SHA-256, original → new bytes,
  golden tests, real-game checks; `fixes.rs` is this game's table),
- `contribute-back.md` (drafts in `docs/CONTRIBUTIONS.md`; the owner posts),

and STANDARDS' sections on the settings window and CLI (design skill
`frontend-design` then a `web-design-guidelines` review for any window
change; accessibility first: contrast test, keyboard and gamepad, focus
ring, disabled reasons; GUI/CLI parity through `config::OPTIONS` with the
parity test; every option tested end to end), robustness and support, and
testing (headless by default: Windows-target tests under Wine with
`sdk/scripts/wine-test --target x86_64-pc-windows-gnu`, real-game runs in
`gamescope --backend headless`). Credentials (e.g.
`~/.config/rea/pcgamingwiki.txt`) are never printed, logged, committed,
passed as arguments or handed to other agents.

## Non-negotiable boundaries

- Never commit, vendor, embed, or publish original executables, `.OVL`
  drivers, media, `.DAT` archives, `.GOG`/`.ISO` CD images, fonts, palettes,
  tracks, car data, or other game assets, or anything copied out of them.
- The player's game folder is strictly read-only. The launcher builds a
  private runtime folder from it and mounts CD images read-only from it.
- Never commit a developer's absolute game path. Commands and code take
  `--game <path>` or `SCREAMER_GAME_DIR`; real-game tests are env-gated.
- Local experiments, probes and traces go under `.work/`.
- Running the player's original game for testing is allowed from a folder
  outside the repository, headless, with notes in `docs/RUNTIME_NOTES.md`.

## Owner decisions

- **2026-11-02 — box art.** The three games' box art
  (`crates/screamer-launcher/assets/cover_*.jpg`, © Virgin Interactive,
  MobyGames scans) may be used in the settings window to identify each
  game, with a credit in the window. Recorded in `THIRD_PARTY.md`.
- **2026-11-02 — no-op options removed.** The 30/50 FPS distinction (the
  same run: `E1` has no bytes) became one Frame generation switch, and
  Vsync "As the game" (the same as On) became a plain switch. Old
  `launcher.cfg` values are still read.

## Product rules

- **Self-contained.** No manual `SETUP.EXE` runs, mapper files, CD-image
  conversion or `dosbox.conf` editing: the launcher generates what the game
  needs (`SCREAMER.CFG`, `CONFIG/SETUP.CFG`, `CONFIG/CONTROLS.CFG`, the DOSBox
  configuration).
- **Native builds.** Linux (`x86_64-unknown-linux-gnu`) and Windows
  (`x86_64-pc-windows-gnu`, linked with LLD) builds of `ScreamerLauncher`,
  and a `dosbox-staging` built from the pinned submodule with
  `patches/dosbox-staging-screamer-hooks.patch`, into which the
  `screamer-hooks` static library (`libscreamer_hooks.a`) is linked. DOSBox
  uses SDL2. No Wine at runtime: the SDK's Wine-only rules do not apply.
- **Exact binary verification.** Patches (`fixes.rs`) and hooks are keyed
  to executables by SHA-256 (`edition.rs`) and checked against original
  bytes and LE relocation records before the private runtime copy is
  patched. An unknown build is refused with its SHA-256 and the issue link.
- **Looks.** Original (as shipped), Enhanced (1080p Hor+ widescreen, longer
  view, crisp pixels; recommended off the Deck) and Steam Deck. The game
  always runs at its fixed 50 Hz: every frame-rate unlock broke the physics
  (`docs/04-…`, `docs/RESEARCH.md`); Frame generation shows 100 FPS without
  touching the game.

## Code and commits

- General solutions first: find the one place a class of cases goes through
  (the camera matrix for Hor+, `PollControls` for analog input) and fix it
  there.
- Rust for our own code; `unsafe`/FFI only in the hooks crate (the DOSBox
  boundary) and `src/win32.rs` (WinMM joystick).
- Unit-test host-testable logic (LE parser, editions, fix tables, config
  generators, runtime sync, `launcher.cfg`, the launch plan, the window
  through `egui_kittest`) before each milestone commit.
- Conventional Commit subjects; commit at working milestones after the full
  check (fmt, tests, clippy for both targets, shell tests, `cargo deny`,
  `sdk-sync --check`, `wine-test`).
- To bump the SDK: change the three `rea-launcher*` tags in `Cargo.toml` and
  the workflows' `@vX.Y.Z`, `cargo update -p rea-launcher -p
  rea-launcher-win32 -p rea-launcher-ui`, run `<sdk>/scripts/sdk-sync .` from
  a clean checkout at the tag, and commit `sdk/` with `Cargo.lock`.
