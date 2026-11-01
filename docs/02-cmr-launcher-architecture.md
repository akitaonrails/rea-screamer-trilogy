# Architecture & Conventions from `rea-colin-mcrae` (`CMRLauncher`)

This document records the architecture, engineering principles, build/packaging layout, and testing practices of `~/Projects/rea-colin-mcrae` (and relevant SDL3 patterns from `~/Projects/OpenCMR2`) to carry over to `rea-screamer`.

---

## 1. Repository & Workspace Structure

- **Cargo Workspace (`Cargo.toml`)**:
  - `resolver = "3"`, `edition = "2024"`, `license = "MIT OR Apache-2.0"`.
  - Release profile: `panic = "abort"`, `opt-level = "s"`, `lto = true`, `strip = true` (with `opt-level = 3` on performance-critical rendering/compositing crates).
- **Core Files**:
  - `AGENTS.md`: Project rules, legal boundaries, headless testing rules, and architecture invariants.
  - `README.md`: End-user and developer documentation, CLI flags, presets, build instructions.
  - `THIRD_PARTY.md`: Full attribution and licenses for all bundled or linked components (fonts, runtime engines, icons).
  - `LICENSE-MIT` & `LICENSE-APACHE`: Dual license.
  - `.gitignore` / `.ignore`: Excludes `target/`, `dist/`, `runtime-data/`, `.work/`, `.slim/`, and any game binaries/assets (`*.exe`, `*.dll`, `*.dat`, `*.gog`, `*.iso`, `*.bin`, `*.cue`).
- **Docs (`docs/`)**:
  - Living engineering documents tracking requirements (`R*`), binary fixes (`F*`), enhancements (`E*`), graphics pipeline analysis (`GRAPHICS.md`), upscaling (`UPSCALING.md`), and chronological runtime verification notes (`RUNTIME_NOTES.md`).

---

## 2. Launcher GUI, CLI & Configuration Design

### GUI (`eframe` / `egui` + `glow`)
- Uses `eframe` with embedded Barlow / Barlow Condensed OFL fonts (no system font dependency, consistent rendering on Linux and Windows).
- Auto-DPI zoom calculation (`window_zoom`) based on screen resolution (`1.5x` on 4K displays).
- Clean tabbed interface (`880x640` base design size):
  - **Play**: Game folder picker + SHA-256 edition detection, one-click preset cards (`Original`, `Enhanced`, `Steam Deck`, `Custom`), and Saves/Replays directory picker.
  - **Picture**: Resolution selector (`Original`, `Screen WxH`, `1080p`, `1440p`, `4K`), Aspect Ratio / Widescreen toggle, Texture Filtering (`Original`, `Smooth`, `Sharp`), Frame-rate limit (`Original`, `30`, `60`, `120`, `144`), Vsync, HD Art toggle, and Linux/gamescope scaler selector (`nearest`, `pixel`, `fsr`, `linear`).
  - **Controls**: Analog gamepad axes toggle, Rumble strength (`Off`, `Light`, `Medium`, `Strong`) with live `Test` button and controller connection indicator, and `Close quits` toggle.
  - **Buttons**: Controller button layout presets (`Original`, `Modern`, `Custom`) and per-action button chip selectors.

### Comment-Preserving Config (`launcher.cfg`) & 1-to-1 CLI Flags
- Human-readable `launcher.cfg` generated from a documented `TEMPLATE`.
- `set_key` and `clear_key` mutate only the target key line while preserving all user comments and unknown keys.
- Every config key has a corresponding CLI argument (`--game`, `--saves`, `--runtime`, `--resolution`, `--widescreen`, `--fps-limit`, `--vsync`, `--texture-filter`, `--rumble`, `--buttons`, `--no-window`, `--settings`, `--check`, `--controllers`, etc.).

---

## 3. Private Runtime & Read-Only User Game Folder Invariant

1. **Never Mutate the User's Game Folder**:
   - The user's original game directory (or GOG install directory / mounted ISO) is treated strictly as read-only input.
   - `runtime::sync` copies game files into a private runtime folder (e.g., `~/.local/share/ScreamerLauncher/runtime/<slug>` or `%LOCALAPPDATA%\ScreamerLauncher\<slug>`), stripping read-only attributes and recording size + mtime in a manifest so subsequent launches copy `0` files.
2. **Save / Replay Mirroring**:
   - User progress (`CONFIG/CHOICE.DAT`, `CONFIG/CONTROLS.CFG`, `REPLAYS/*`, high scores, championships) is mirrored between a dedicated user save directory and the runtime directory before launch and after exit.
3. **SHA-256 Edition Identification**:
   - Known game executables (`STARTL.EXE`, `STARTH.EXE`, `START65H.EXE`, `S2_3DFX.EXE`, `STRT3FX.EXE`, `SETUP.EXE`) are identified by SHA-256 hash (`edition.rs`) so binary patches target verified offsets and reject unknown/corrupted binaries safely.

---

## 4. Binary Patching & Hooking Methodology

- **Declarative Patch Assembler (`fixes.rs`)**:
  - Patches are declared as structured `Fix` definitions (`id`, `name`, `pieces: &[Piece]`) using `At::Image(addr)` and `At::Cave(offset)` with typed x86 ops (`Op::Raw`, `Op::Jmp`, `Op::Jcc`, `Op::Call`, `Op::Addr`).
  - Pre-write verification ensures the original bytes at every patch site match expected bytes before applying patches.
  - Unit tests verify that all fixes assemble cleanly, no two fixes write overlapping byte ranges unless identical, and every patch site falls within valid executable sections of the binary.
- **General Solutions at Central Primitives**:
  - Instead of patching dozens of individual menus or race screens, hook central bottlenecks:
    - Central 2D blit / sprite / text dispatcher for virtual 640x480 canvas scaling and HUD corner anchoring (`canvas.rs`).
    - Central 3D projection & view-frustum culling parameters for Hor+ widescreen (`xs`, `ys`, frustum wedge angles).
    - Central frame flip / present primitive for sub-millisecond frame pacing (`pacing.rs`) and vsync control.
    - Central input polling primitive for analog steering/triggers, sticky controller presence, and button remapping (`pad.rs`).

---

## 5. Testing & Non-Intrusive Verification Discipline

1. **Host-Testable Pure Logic (`cargo test --workspace`)**:
   - Keep OS/FFI/hardware glue in thin boundary modules so all binary parsers (PE / LE), patch assemblers, config parsers, UI form state machines, coordinate/aspect math, audio/CD cuesheet parsers, and input mappers have fast unit tests running natively on Linux and Windows.
2. **Headless Runtime Verification (`.work/rt/`)**:
   - Never pop windows onto the user's active desktop or play audio to physical speakers during automated verification.
   - Use headless display backends (`gamescope --backend headless` or SDL offscreen/dummy/xvfb + screenshot capture) and a dedicated PipeWire/PulseAudio null sink (`PULSE_SINK` + `parec` non-zero sample verification), with strict timeouts (`timeout 25s`) and process cleanup checks.
