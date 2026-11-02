# Screamer trilogy launcher — instructions for coding agents

This repository makes the player's own copy of the **Screamer** trilogy
(**Screamer** [1995], **Screamer 2** [1996] and **Screamer Rally** [1997],
by Graffiti / Milestone, published by Virgin Interactive) run on modern
Windows and Linux with a settings window, sensible defaults and no manual
steps. It is not a distribution of the games. It is built on
[rea-launcher-sdk](https://github.com/akitaonrails/rea-launcher-sdk) (the tag
in `sdk/VERSION`).

**Read `sdk/docs/STANDARDS.md` before any work; the playbooks are in
`sdk/docs/playbooks/`** (vendored from the SDK at the tag in `sdk/VERSION`).

## Always

- The game's files are read-only: never committed, copied into the repo or published.
- Everything outside this repository is the user's and read-only (game installs, other repos, Wine prefixes, saves, home configs, containers, the SDK checkout) unless the user asked for or approved that specific change: back up first, never delete by a broad glob, ask when unsure.
- Headless by default; open a visible window only when a check needs a person, and say so first.
- Never print, log or commit credentials.
- Push, tag, release or post only with the user's go-ahead.
- Research community fixes first (`docs/RESEARCH.md`); patch only what nobody solved.
- Never edit `sdk/`; SDK gaps and bugs go to the SDK repository as issues or PRs.
- Clean up after a release or a long session: `sdk/scripts/clean`, then `--yes` (log scratch in `.work/.task-created`).

Running the real game in a test: `sdk/docs/STANDARDS.md`, "Running the real
game in a test" (under "Testing"). Here the real-game tests are `#[ignore]`d
and take `SCREAMER_GAME_DIR` (or `SCREAMER1_DIR`, `SCREAMER2_DIR`,
`SCREAMER_RALLY_DIR`): `SCREAMER_GAME_DIR=… cargo test -- --ignored`.

## Layout

| Path | What |
|---|---|
| `crates/screamer-launcher/src/edition.rs` | Known builds (SHA-256), the unknown-edition refusal |
| `crates/screamer-launcher/src/fixes.rs`, `le.rs` | The binary fixes per edition, the LE executable parser |
| `crates/screamer-launcher/src/config.rs` | The option table (`config::OPTIONS`): `launcher.cfg`, flags, `--help`, template |
| `crates/screamer-launcher/src/form.rs`, `ui.rs` | The settings window, looks, and its headless tests |
| `crates/screamer-launcher/src/game_cfg.rs`, `runtime.rs` | The game's config files, the runtime copy, the launch plan |
| `crates/screamer-launcher/src/main.rs`, `win32.rs` | Modes, `launcher.log`, DOSBox's process; WinMM joystick |
| `crates/screamer-hooks/` | The static library linked into DOSBox (gamepad, telemetry) |
| `patches/`, `vendor/dosbox-staging`, `scripts/build-dosbox.sh` | The pinned dosbox-staging and its hook patch |
| `linux/screamer-play`, `linux/tests/` | The Linux helper (sources `sdk/rea-play.sh`) and its tests |
| `packaging/` | Allowlist, AppImage desktop entry and icon |
| `sdk/` | Vendored by the SDK's `scripts/sdk-sync`; never edit by hand |
| `docs/RESEARCH.md`, `docs/RUNTIME_NOTES.md`, `docs/CONTRIBUTIONS.md` | Findings, verification, drafts to share |

## Checks (all must pass before a commit)

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets --locked -- -D warnings
cargo deny check
for t in linux/tests/*.sh; do sh "$t"; done
../rea-launcher-sdk/scripts/sdk-sync --check .
cargo build --release --locked --target x86_64-pc-windows-gnu
sh sdk/packaging/check-imports.sh target/x86_64-pc-windows-gnu/release/*.exe
sdk/scripts/wine-test --locked --workspace --target x86_64-pc-windows-gnu
```

MinGW, lld and Wine may live in the `gaming` distrobox: run the Windows
lines as `distrobox enter gaming -- sh -c 'cd /absolute/path && …'`.

## Screamer: what is specific to this game

- **Game files.** Besides executables and media: `.OVL` drivers, `.DAT`
  archives, `.GOG`/`.ISO` CD images, fonts, palettes, tracks and car data.
  None of it, or anything copied out of it, is committed, vendored,
  embedded or published.
- **Runtime folder.** The launcher builds a private runtime copy from the
  game folder and mounts CD images read-only from the game folder itself.
- **Self-contained.** No manual `SETUP.EXE` runs, mapper files, CD-image
  conversion or `dosbox.conf` editing: the launcher generates what the game
  needs (`SCREAMER.CFG`, `CONFIG/SETUP.CFG`, `CONFIG/CONTROLS.CFG`, the
  DOSBox configuration).
- **Native builds.** Linux (`x86_64-unknown-linux-gnu`) and Windows
  (`x86_64-pc-windows-gnu`, linked with LLD) builds of `ScreamerLauncher`,
  and a `dosbox-staging` built from the pinned submodule with
  `patches/dosbox-staging-screamer-hooks.patch`, into which the
  `screamer-hooks` static library (`libscreamer_hooks.a`) is linked. DOSBox
  uses SDL2. No Wine at runtime: the SDK's Wine-only rules do not apply.
- **Exact binary verification.** Patches (`fixes.rs`) and hooks are keyed
  to executables by SHA-256 (`edition.rs`) and checked against the
  original bytes and the LE relocation records before the private runtime
  copy is patched.
- **Looks.** Original (as shipped), Enhanced (1080p Hor+ widescreen, longer
  view, crisp pixels; recommended off the Deck) and Steam Deck. The game
  always runs at its fixed 50 Hz: every frame-rate unlock broke the physics
  (`docs/04-…`, `docs/RESEARCH.md`); Frame generation shows 100 FPS without
  touching the game.
- **Where a class of cases goes through.** Hor+ widescreen is fixed in the
  camera matrix; analog input in `PollControls`.

### Owner decisions

- **2026-11-02 — box art.** The three games' box art
  (`crates/screamer-launcher/assets/cover_*.jpg`, © Virgin Interactive,
  MobyGames scans) may be used in the settings window to identify each
  game, with a credit in the window. Recorded in `THIRD_PARTY.md`.
- **2026-11-02 — no-op options removed.** The 30/50 FPS distinction (the
  same run: `E1` has no bytes) became one Frame generation switch, and
  Vsync "As the game" (the same as On) became a plain switch. Old
  `launcher.cfg` values are still read.
