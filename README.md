# Screamer Launcher (`rea-screamer`)

A self-contained, cross-platform compatibility launcher and runtime enhancer for the 1995–1997 **Screamer** trilogy by Graffiti / Milestone:
- **Screamer** (`1995`) — `STARTH.EXE` (SVGA 640×480), `STARTL.EXE` (VGA 320×200), `SETUP.EXE`
- **Screamer 2** (`1996`) — `S2_3DFX.EXE` (3dfx Voodoo Glide), `START65H.EXE` (SVGA 640×480 16-bit), `STARTH.EXE` (SVGA 640×480 8-bit), `SETUP.EXE`
- **Screamer Rally** (`1997`) — `STRT3FX.EXE` (3dfx Voodoo Glide), `START65H.EXE` (SVGA 640×480 16-bit), `STARTH.EXE` (SVGA 640×480 8-bit), `ST.EXE`

Point `ScreamerLauncher` at your own read-only GOG or CD-ROM installation folder and click **Play**. No manual `SETUP.EXE` sound card configuration, no `Ctrl+F1` DOSBox mapper files, no manual `dosbox.conf` editing, and no modifying your original game files.

---

## Features & Engine Enhancements

### Automatic Zero-Touch Configuration
- **Read-Only Source Directory**: Your game installation directory is never modified. `ScreamerLauncher` stages a private runtime folder (`~/.local/share/screamer-launcher/runtime/<game>`), symlinks static archives and CD images, and syncs save files (`*.SAV`, `CHOICE.DAT`, `REPLAYS/*.RPL`) before and after every session.
- **Synthesized Binary Configs**: Automatically generates exact binary `SCREAMER.CFG` (`11`/`14` bytes), `CONFIG/SETUP.CFG` (`144`/`160` bytes pre-configured for Sound Blaster 16 `Port 0x220, IRQ 7, DMA 1` + HMI `HMIMDRV.386`), and `CONFIG/CONTROLS.CFG` (`1,328`/`1,496` bytes with pre-calibrated analog joystick tables).
- **Redbook CD Audio & CUE Sheet Detection**: Automatically detects GOG CUE-formatted `.DAT` files (`SCREAM.DAT`, `GAME.DAT`) as well as `.cue`/`.ins`/`.iso` images so tracks 02–08 play at 48 kHz without resampling.
- **3dfx Voodoo Glide Auto-Repair for Screamer 2**: Automatically bypasses the broken `31,425`-byte Windows `glide2x.ovl` stub bundled with GOG's Screamer 2 and links a genuine DOS `GLIDE2X.OVL` when available.

### Verified Binary Fixes & Modern Enhancements (`crates/screamer-launcher/src/fixes.rs`)
Every binary patch is verified by **SHA-256**, checked against expected original instruction bytes, and validated against the **Watcom `DOS4GW` Linear Executable (`LE`) Fixup Record Table** before writing to the private runtime copy:
- **`F2` (Screamer 1 DPMI Heap Clamp)**: Fixes the unguarded 100-element stack array overflow and 32-bit signed integer overflow (`>= 2 GB` RAM) in `STARTH.EXE` / `STARTL.EXE` startup memory probing (`0x02F8CA` / `0x02F682`).
- **`F3` (Screamer 1 VESA `WinGranularity` Fix)**: Neutralizes the `and eax, 7` mask on `WinGranularity` (`0x08BE5F`) in `STARTH.EXE`.
- **`F5` (Screamer 2 VBE 2.0 Mode `0x111` Fix)**: Forces standard VESA mode `0x111` (`640×480×16bpp`) in `START65H.EXE` (`0x0D782F`).
- **`F6` (`SETUP.EXE` / `ST.EXE` Speed Benchmark Fix)**: Prevents `idiv ecx` divide-by-zero crashes on fast CPUs.
- **`E1` (True 60+ FPS with 1.0× Physics Speed)**: Unlocks the `frame_ticks >= 2` (`50 FPS` / `30 FPS` under 60 Hz VSync) spin-wait loop AND lowers the hardcoded `0.02s` (`20 ms`) minimum `smoothed_dt` clamp to `0.004s` (`250 FPS` max) across all 8 racing executables so 60+ FPS runs at true 1.0× physics speed instead of running 20% too fast.
- **`E2` (Hor+ 16:9 Widescreen 3D Projection)**: Scales the X-row (`M00`, `M01`, `M02`) of `camera->view_matrix` in `SetCamera3D` by `3/4` (`0.75`), expanding the 3D horizontal field of view from 90° to ~106.26° at 16:9 aspect ratio with zero frustum culling pop-in at screen edges.
- **`E5` (Zero Deadzone Analog Steering)**: Removes the hardcoded `10%` (`0x66 = 102/1024`) center steering deadzone in `PollControls`.
- **`E6` (`libscreamer_hooks` Gamepad & Rumble Bridge)**: Intercepts the host loop in `dosbox-staging` to read modern SDL gamepads (`Left Stick X` steering + independent `L2`/`R2` analog triggers + rumble feedback) and inject directly into DOSBox's joystick axes.

---

## Quick Start

### Build from Source
```bash
cargo test --workspace
cargo build --release --workspace
```

### Verify a Game Installation (Read-Only)
```bash
./target/release/ScreamerLauncher --check --game /path/to/Screamer
```

### Launch GUI or Direct Play
```bash
# Open the graphical settings launcher:
./target/release/ScreamerLauncher --game /path/to/Screamer

# Or launch directly with CLI overrides:
./target/release/ScreamerLauncher --no-window \
  --game /path/to/Screamer \
  --resolution 3840x2160 \
  --widescreen on \
  --fps-limit 60 \
  --gamepad-axes on \
  --deadzone 0
```

On Linux, you can also use [`linux/screamer-play`](file:///home/akitaonrails/Projects/rea-screamer/linux/screamer-play), which automatically configures hybrid GPU offload (`__NV_PRIME_RENDER_OFFLOAD=1` / `DRI_PRIME=1`), `libscreamer_hooks.so`, and `gamescope` scaling.

---

## Architecture & Research Documentation

- [`docs/01-distrobox-gaming-findings.md`](file:///home/akitaonrails/Projects/rea-screamer/docs/01-distrobox-gaming-findings.md) — Analysis of the legacy `distrobox-gaming` setup and its pain points.
- [`docs/02-cmr-launcher-architecture.md`](file:///home/akitaonrails/Projects/rea-screamer/docs/02-cmr-launcher-architecture.md) — Architecture patterns adapted from `rea-colin-mcrae`.
- [`docs/03-screamer-binary-analysis.md`](file:///home/akitaonrails/Projects/rea-screamer/docs/03-screamer-binary-analysis.md) — Watcom `LE` object tables, `SCREAMER.CFG` / `SETUP.CFG` / `CONTROLS.CFG` binary layouts, and CD image structures.
- [`docs/04-screamer-engine-and-patches.md`](file:///home/akitaonrails/Projects/rea-screamer/docs/04-screamer-engine-and-patches.md) — Deep dive into `SetCamera3D`, 3dfx Glide 2.x `GLIDE2X.OVL` thunks, `smoothed_dt` 60 FPS timing, and `PollControls`.
- [`docs/THIN_APP_PLAN.md`](file:///home/akitaonrails/Projects/rea-screamer/docs/THIN_APP_PLAN.md) — Milestone plan and patch catalog (`F1`–`F6`, `E1`–`E6`).
- [`docs/RUNTIME_NOTES.md`](file:///home/akitaonrails/Projects/rea-screamer/docs/RUNTIME_NOTES.md) — Runtime verification findings across Screamer, Screamer 2, and Screamer Rally.

---

## License

Dual-licensed under [`LICENSE-MIT`](file:///home/akitaonrails/Projects/rea-screamer/LICENSE-MIT) or [`LICENSE-APACHE`](file:///home/akitaonrails/Projects/rea-screamer/LICENSE-APACHE) at your option. No copyrighted game code or assets from Screamer, Screamer 2, or Screamer Rally are included in this repository.
