# Screamer Launcher — Master Architecture & Implementation Plan (`THIN_APP_PLAN.md`)

## 1. Goal & Scope

Build a self-contained compatibility launcher (`screamer-launcher` / `ScreamerLauncher.exe`) and runtime hook library (`libscreamer_hooks.so` / `screamerhooks.dll`) modeled after `rea-colin-mcrae`, supporting the entire 1995–1997 Graffiti / Milestone **Screamer** DOS4GW trilogy:
- **Screamer** (1995) — SVGA (`STARTH.EXE`) and VGA (`STARTL.EXE`)
- **Screamer 2** (1996) — 3dfx Voodoo Glide (`S2_3DFX.EXE`), 16-bit SVGA (`START65H.EXE`), and 8-bit SVGA (`STARTH.EXE`)
- **Screamer Rally** (1997) — 3dfx Voodoo Glide (`STRT3FX.EXE`), 16-bit SVGA (`START65H.EXE`), and 8-bit SVGA (`STARTH.EXE`)

The user points the launcher at their read-only game directory (GOG install, extracted GOG installer, or CD/ISO copy) and clicks **PLAY** (or runs `screamer-play --no-window`). No manual `SETUP.EXE`, no `Ctrl+F1` DOSBox mapper files, no manual `dosbox.conf` editing, and no fragility across audio/video/gamepad setups.

---

## 2. Supported Editions & Verified SHA-256 Hashes

All binary patches and memory offsets are keyed strictly by SHA-256 digest:

| Game | Executable | Renderer / Role | Size (B) | SHA-256 Digest |
|---|---|---|---|---|
| **Screamer (1995)** | `STARTH.EXE` | SVGA 640×480×8bpp (VESA 101h) | `1,034,923` | `1246afc95463f96598d5fee764fc99c847b6d3e0bf8ca2268bd362c664bb28a3` |
| **Screamer (1995)** | `STARTL.EXE` | VGA 320×200×8bpp (Mode 13h) | `1,034,919` | `ad20fa3919ecf86da5931e2e0e495d4043f6be8c44e57299114528d87ee27ec7` |
| **Screamer (1995)** | `SETUP.EXE` | Hardware / Control Setup | `251,079` | `33d88269a37bc67b08b2ec9f9f7be32c429619f6d946837c35b7e08f33f2d189` |
| **Screamer 2 (1996)** | `S2_3DFX.EXE` | 3dfx Glide 640×480×16bpp | `1,055,262` | `e6c24c84650938f2baee5f4b571baa45bb09a93ba12dd24e3e1d2cefd9b6f3ec` |
| **Screamer 2 (1996)** | `START65H.EXE` | SVGA 640×480×16bpp (VESA 111h) | `1,201,082` | `05f576866f26602be303343c5e9104132616f464b71ed6ae958e99a668e5c7c8` |
| **Screamer 2 (1996)** | `STARTH.EXE` | SVGA 640×480×8bpp (VESA 101h) | `1,055,490` | `91a49f26c7b5a26bc84552126ed337dfa733b81c61ec4ad4195d205809a2f400` |
| **Screamer 2 (1996)** | `START65L.EXE` | VGA 320×200×16bpp | `1,205,090` | `f2d6c1389f2e09e507b822fc6d530cad898671fcf55e8bb02bc00de861515d46` |
| **Screamer 2 (1996)** | `STARTL.EXE` | VGA 320×200×8bpp | `1,055,478` | `78a78edbc92196647d6bb3980bf842d42e68a25405fb78098072958d60e66f0a` |
| **Screamer 2 (1996)** | `SETUP.EXE` | Hardware / Control Setup | `274,089` | `ef8d7665f1a8979ee9f699ff0bd476b8e9679fbe423a6b53a773539ce51d178b` |
| **Screamer Rally (1997)** | `STRT3FX.EXE` | 3dfx Glide 640×480×16bpp | `1,044,502` | `9eac6e12ab309389bdda019cfa695bfbbc791fbda1a4634464a444fce35165d1` |
| **Screamer Rally (1997)** | `START65H.EXE` | SVGA 640×480×16bpp (VESA 111h) | `1,145,058` | `0785e38567c475f25663e4022ec125e447a799004e34efa59758b50cfd84901d` |
| **Screamer Rally (1997)** | `STARTH.EXE` | SVGA 640×480×8bpp (VESA 101h) | `1,045,098` | `18d32c27d61873dc67381f87880d19774afaec1f7ee391d14ab255a9040a12d7` |
| **Screamer Rally (1997)** | `START65L.EXE` | VGA 320×200×16bpp | `1,149,630` | `d504a5a95ed3344a798f4a2f5d736c4a8ba8c493417d86d8b0617178b6818472` |
| **Screamer Rally (1997)** | `STARTL.EXE` | VGA 320×200×8bpp | `1,049,186` | `a8822060bfa54141952d051afd0efc05a0f1ae97dc23e78dea6cce9192f9e9de` |
| **Screamer Rally (1997)** | `ST.EXE` | Hardware / Control Setup | `297,736` | `e6083738a0130e77d8403f2948d70f4c554481405be8197ab70123ca0d57393d` |

---

## 3. Requirements (`R1`–`R12`), Fixes (`F1`–`F10`), and Enhancements (`E1`–`E8`)

### Core Requirements (`R1`–`R12`)
- **R1 — Read-only game directory**: Never write to the user's game folder. Build an isolated runtime directory (`runtime-data/` or user data dir) containing patched executables and generated config files, while referencing large read-only `.GOG`/`.ISO`/`.DAT` files in place.
- **R2 — Automatic edition & layout detection**: Detect Screamer 1, Screamer 2, and Screamer Rally across flat directories, GOG `app/` layouts, and `hdd/SR` + `Screamer_Rally_ISO` layouts.
- **R3 — Automatic config synthesis**: Generate binary-accurate `SCREAMER.CFG` (`C:\ D: C:\ c`), `CONFIG/SETUP.CFG` (`144` B for S1, `160` B for S2/SR with Sound Blaster 16 `Port=0x220, IRQ=7, DMA=1` matching `dosbox-staging`), `CONFIG/CONTROLS.CFG` (`1328` B for S1, `1496` B for S2/SR), and `CONFIG/CHOICE.DAT`.
- **R4 — High-resolution SVGA (`STARTH.EXE` / `START65H.EXE`) & 3dfx Voodoo (`S2_3DFX.EXE` / `STRT3FX.EXE`)**: Default to the highest quality renderer for each game (`STARTH.EXE` SVGA 640×480 for Screamer 1; `S2_3DFX.EXE` / `START65H.EXE` for Screamer 2; `STRT3FX.EXE` / `START65H.EXE` for Screamer Rally).
- **R5 — CD Audio & Data Track Mounting**: Automatically discover CUE/GOG/ISO pairs (`game.ins`/`game.gog`, `scream2.inst`/`scream2.gog`, `srally.cue`/`srally.iso`), convert raw 2352-byte sector `.img` paths or generate `.cue` sheets in the runtime folder when needed so Redbook CD audio tracks play out of the box.
- **R6 — Persistent Saves, Replays & Records**: Mirror `CONFIG/*.SAV`, `REPLAYS/*`, and high scores in `CHOICE.DAT` / `CONFIG/` to a persistent user save directory (`--saves`).
- **R7 — Native Settings GUI & Headless CLI**: Provide an 880×640 `eframe`/`egui` launcher (`Play`, `Picture`, `Controls`, `Buttons` tabs) plus `--no-window`, `--settings`, `--check`, and `--controllers` CLI modes.
- **R8 — Comment-preserving `launcher.cfg`**: Human-readable `key = value` config file with round-trip comment preservation.

### Binary Fixes (`F1`–`F10`)
- **F1 — `SETUP.EXE` / `ST.EXE` High-Cycle Divide-by-Zero Fix**: Replace `idiv ecx` (`F7 F9`) in the VRAM and CPU speed benchmarks with safe `mov eax, 1000` (`B8 E8 03 00 00`) when `ECX == 0` (S1 `SETUP.EXE` `0x03384B` & `0x0338F2`; S2 `SETUP.EXE` `0x038553` & `0x038606`; SR `ST.EXE` `0x03B2A3` & `0x03B356`).
- **F2 — Screamer 1 `STARTH.EXE` / `STARTL.EXE` DPMI Memory Probe Overflow Fix**: Cap the DPMI allocation loop (`VA 0x024DC4`, `FileOff 0x02F5C4`) and configure `memsize = 32` so large host memory sizes never overflow the 100-entry stack buffer `[ebp - 0x1A4]` or 32-bit signed byte check in `main_`.
- **F3 — Screamer 1 `STARTH.EXE` / `STARTL.EXE` No-CD Verification Bypass**: Patch `VerifyCD` (`VA 0x024580`, `FileOff 0x02ED80` in `STARTH.EXE`; `VA 0x024570`, `FileOff 0x02DD70` in `STARTL.EXE`) to return `1` (`B8 01 00 00 00 C3`) when running from extracted `DATA_*` / `SCREAM.DAT` folders without a mounted optical disc.
- **F4 — Screamer 1 `STARTH.EXE` VESA `WinGranularity` Shift Fix**: Guard the `WinGranularity & 7` check (`VA 0x08BE5B`, `FileOff 0x09665B`) against quirky VESA BIOS granularities.
- **F5 — Screamer 2 `START65H.EXE` VBE 2.0 Mode `0x120 -> 0x111` Fix**: Patch `jl` (`7C 16`) at `VA 0x0D782F` (`FileOff 0x0EB02F`) to `jmp` (`EB 16`) so `START65H.EXE` requests standard VESA mode `0x111` (`640x480x16bpp`) instead of proprietary mode `0x120` (`1600x1200x8bpp` on S3 Trio64 VBE 2.0).
- **F6 — Screamer 2 `S2_3DFX.EXE` Genuine DOS 3dfx `GLIDE2X.OVL` Provisioning**: Detect when `GLIDE2X.OVL` is missing or replaced by the `31,425`-byte Windows port-`0x600` stub, and either supply a genuine DOS `GLIDE2X.OVL` from a sibling Screamer Rally installation or transparently fall back to patched 16-bit SVGA `START65H.EXE` (`640x480x16bpp`).

### Modern Enhancements (`E1`–`E6`)
- **E1 — True 60 FPS & 120 FPS Unlock with 1.0x Real-Time Physics**:
  - Patch the 50 FPS spin-wait (`cmp reg, 2` -> `cmp reg, 1`) in `STARTH.EXE`, `STARTL.EXE`, `S2_3DFX.EXE`, and `START65H.EXE`.
  - Lower the hardcoded `0.02` (`20 ms`) minimum `smoothed_dt` clamp in all 6 executables (`0.02` -> `0.004`), and normalize the `1.1x` race `dt` multiplier in S2/SR when 60/120 FPS is enabled.
  - Optionally raise the HMI SOS timer from `100 Hz` (`0x64`) to `120 Hz` (`0x78`) or `240 Hz` (`0xF0`) with proportional `dt` tick scale (`1/120` or `1/240`) so 60 FPS (`2` ticks) and 120 FPS (`1` or `2` ticks) pace smoothly without judder.
- **E2 — Hor+ 16:9 / 16:10 Widescreen 3D Projection**:
  - Scale the X-row (`Right` basis vector) of the 3×3 camera rotation matrix (`0x082BB4..0x082BBC` in S1 `STARTH.EXE`, `0x07C3F4..0x07C41C` in `S2_3DFX.EXE`, `0x07AD50..0x07AD70` in `STRT3FX.EXE`) by `(4/3) / aspect_ratio` (`0.75` for 16:9, `0.8333` for 16:10) and set `aspect_correction` / widescreen viewport in `dosbox-staging` / `gamescope`.
- **E3 — 4K Upscaling & Display Pipeline**:
  - Configure `dosbox-staging` OpenGL / integer or sharp-bilinear shader pipeline and optional `gamescope` compositor scaling up to `3840x2160` (4K) or native desktop resolution with VSync control.
- **E4 — Zero-Lag Proportional Analog Steering & Separate L2/R2 Triggers**:
  - Pre-calibrate `CONTROLS.CFG` with `dev = 1, mode = 0` on Steering (`binding[2]`), Accelerate (`binding[0]`), and Brake (`binding[1]`) so the engine bypasses the keyboard steering rate limiter (`0x02D380`) and reads continuous analog values (`[car + 0x108]`, `[car + 0x144]`, `[car + 0x114]`).
- **E5 — Custom Analog Deadzone Removal**:
  - Patch the hardcoded `10%` (`0x66` = `102/1024`) center deadzone in `PollControls` (`FileOff 0x02F73E` in S1 `STARTH.EXE`, `0x03FE04` in `S2_3DFX.EXE`, `0x03F644` in `SR_STRT3FX.EXE`) to `0x00` so our launcher/hook applies a smooth radial/axial deadzone configurable in `launcher.cfg` (`0–25%`).
- **E6 — In-Process `libscreamer_hooks` SDL Gamepad & Rumble Bridge**:
  - `LD_PRELOAD` hook library (`crates/screamer-hooks`) that intercepts `GFX_Events()` in `dosbox-staging`, locates the game's Data Object in `GetMemBase()`, injects independent analog L2/R2 triggers + 10-bit steering + custom face/shoulder buttons + D-pad menu navigation, and triggers controller rumble from car collision/skid telemetry.

---

## 4. Milestones (`M0`–`M6`)

- **M0 — Research & Reverse Engineering Documentation (`docs/`)**:
  - Complete binary analysis of Screamer 1, Screamer 2, and Screamer Rally (`docs/01` through `docs/04` + `docs/THIN_APP_PLAN.md`).
- **M1 — Headless Runtime Verification Harness & Probes (`.work/rt/`)**:
  - Verify Screamer 1 `STARTH.EXE` (SVGA 640×480), Screamer 2 (`START65H.EXE` / `S2_3DFX.EXE`), and Screamer Rally (`STRT3FX.EXE` / `START65H.EXE`) booting cleanly in headless `gamescope` + private PipeWire null sink with our binary fixes and synthesized configs.Record findings in `docs/RUNTIME_NOTES.md`.
- **M2 — Core Launcher Library (`crates/screamer-launcher`)**:
  - Implement and unit-test `le.rs` (Watcom LE parser + fixup overlap verifier), `edition.rs` (SHA-256 detection), `fixes.rs` (binary patch table), `game_cfg.rs` (`SCREAMER.CFG`, `SETUP.CFG`, `CONTROLS.CFG`, `CHOICE.DAT` synthesizers), `runtime.rs` (read-only game sync + CD CUE/ISO handling + save mirroring), and `config.rs` (`launcher.cfg`).
- **M3 — Settings GUI & CLI (`crates/screamer-launcher/src/form.rs`, `ui.rs`, `main.rs`)**:
  - Implement the 4-tab `eframe`/`egui` settings window (`Play`, `Picture`, `Controls`, `Buttons`), presets (`Original 1995`, `Enhanced`, `Steam Deck`, `Custom`), and CLI flags (`--no-window`, `--settings`, `--check`, `--controllers`).
- **M4 — In-Process Input & Rumble Hook (`crates/screamer-hooks`)**:
  - Implement `libscreamer_hooks.so` / `screamerhooks.dll` for `dosbox-staging` memory/input hooking and SDL gamepad rumble.
- **M5 — Linux Wrapper (`linux/screamer-play`), Packaging (`packaging/appimage`), & CI (`.github/workflows/build.yml`)**:
  - Complete end-to-end launcher script, AppImage builder, GitHub Actions workflow, and `README.md`.
- **M6 — Full End-to-End Headless Verification & Final Clean Audit**:
  - Run all unit tests, clippy, rustfmt, and headless runtime verification across Screamer 1, Screamer 2, and Screamer Rally.
