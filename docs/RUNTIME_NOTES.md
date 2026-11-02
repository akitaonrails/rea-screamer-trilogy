# Runtime Verification Notes

This document records notable observations from running the original 1995–1997 **Screamer** trilogy binaries (`Screamer` [1995], `Screamer 2` [1996], and `Screamer Rally` [1997]) through `ScreamerLauncher` and the bundled, statically hooked `dosbox-staging v0.82.2` in headless X11/OpenGL sessions.

---

## 1. `dosbox-staging v0.82.2` Static Hook Integration

- `crates/screamer-hooks` builds as `libscreamer_hooks.a` and is linked directly into the bundled Linux and Windows DOSBox binaries.
- A source patch calls `screamer_hooks_on_tick` at the end of main-thread `GFX_Events()` after SDL event pumping and passes explicit memory, keyboard, joystick, and SDL callbacks. No `LD_PRELOAD`, `dlsym`, `clock_gettime` interception, background polling thread, or separately shipped hook `.so`/`.dll` remains.
- Keeping controller polling on the SDL main thread avoids the non-reentrant joystick-mutex deadlock observed with the earlier interception prototype.

---

## 2. GOG CUE Sheet `.DAT` Files & Redbook CD Audio

- In GOG's releases of **Screamer** (`1995`) and **Screamer 2** (`1996`), `SCREAM.DAT` (`359` bytes) and `GAME.DAT` (`399` bytes) are plain-text CUE sheets referencing `SCREAM.gog` and `GAME.GOG` (`BINARY`), defining both Mode 1 Data Track 01 and Redbook CD-DA Audio Tracks 02..08.
- Mounting `.gog` directly as a raw ISO omits the audio track table and causes `STARTH.EXE` / `S2_3DFX.EXE` to fail CD verification or lose CD music.
- `ScreamerLauncher`'s `find_cd_image()` inspects `.DAT` files `< 16 KB` whose content starts with `FILE ` and mounts them via `imgmount d "<path>" -t iso -fs iso`, enabling both the data track and `CDAUDIO: Operating at 48000 Hz without resampling`.

---

## 3. Why `VerifyCD` (`F1`) Is Not Needed and Why `0x15460` / `0x39B40` Must Not Be Bypassed

- During headless testing of `STARTH.EXE`, patching `VA 0x015460` (`S1_STARTH`) / `VA 0x039B40` (`S2_3DFX`) with `xor eax, eax; ret 8` caused `STARTH.EXE` to exit with:
  ```text
  An error occurred while opening file: .3dx
  ```
- Disassembly of `0x015460` (`S1_STARTH`) and `0x039B40` (`S2_3DFX`) revealed that this function is actually **`ParseDtrConfig`** (`"%s\\data\\%s.dat"`, `"%s.dtr"`, `"END_LIST"`), which parses `MENU.DTR`, `GEN.DTR`, `TRACK_DATA.DTR`, `OPPONENTS.DTR`, `ANIMATIONS.DTR`, and `CARS_DATA.DTR` to populate the engine's 3D mesh (`.3dx`) and texture filename tables!
- Because `ScreamerLauncher` synthesizes `SCREAMER.CFG` (`C:\ D: C:\ c`) and mounts the CUE/ISO image on `D:` automatically, all three games pass their native CD presence checks out of the box without touching `ParseDtrConfig`.

---

## 4. Screamer 2 `S2_3DFX.EXE` vs. Windows `glide2x.ovl` Stub

- Both **Screamer 2** (`S2_3DFX.EXE`) and **Screamer Rally** (`STRT3FX.EXE`) dynamically load `GLIDE2X.OVL` from the working directory or `PATH`.
- GOG's **Screamer 2** ships with a `31,425`-byte Windows port-`0x600` stub `glide2x.ovl` (31,425 bytes; SHA-256 `fe6f33d3add608eee61f956b82eaa9c9027ee79441dde29b3fbc687f0605f37b`, measured on the GOG files) intended for Windows 9x DOS boxes, whereas **Screamer Rally** ships with the genuine `348,791`-byte DOS 3dfx Voodoo driver `GLIDE2X.OVL` (348,791 bytes; SHA-256 `6981a7bd77445be7b360ca367af1e527927db396546db15b69803b16a67676a9`, from the Screamer Rally ISO). The launcher tells them apart by size (under 50,000 bytes: the stub; over 100,000: the DOS driver), so a different copy of either still works.
- `ScreamerLauncher` automatically skips the `31,425`-byte Windows stub when staging the runtime folder and links a genuine DOS `GLIDE2X.OVL` if present (via `SCREAMER_GLIDE2X_OVL` or a sibling `Screamer Rally` installation), allowing **Screamer 2** to run in full hardware-accelerated 3dfx Voodoo Glide mode (`S2_3DFX.EXE`) as well as 16-bit SVGA mode (`START65H.EXE`).

---

## 5. Headless Verification Summary Across All 3 Games

| Game | Selected Executable | Verified Patches Applied | Graphics Mode Entered | CD Audio & SB16 Status |
| :--- | :--- | :--- | :--- | :--- |
| **Screamer (1995)** | `STARTH.EXE` (`SVGA 640x480`) | `F2`, `F3`, `E1`, `E2`, `E5`, `E6` (8 writes) | `VESA 640x480 256-colour mode 101h` | `SCREAM.DAT` mounted, `48000 Hz` CD-DA + `SB16 IRQ 7` |
| **Screamer 2 (1996)** (3dfx) | `S2_3DFX.EXE` (`3dfx Voodoo Glide`) | `F7`, `E1`, `E2`, `E5`, `E6` (11 writes) | `3dfx Voodoo SVGA 640x480 16-bit mode 00h` | `GAME.DAT` mounted, `48000 Hz` CD-DA + `SB16 IRQ 7` |
| **Screamer 2 (1996)** (SVGA16) | `START65H.EXE` (`SVGA 640x480 16-bit`) | `F5`, `E1`, `E2`, `E5`, `E6` (9 writes) | `VESA 640x480 16-bit mode 111h` | `GAME.DAT` mounted, `48000 Hz` CD-DA + `SB16 IRQ 7` |
| **Screamer Rally (1997)** | `STRT3FX.EXE` (`3dfx Voodoo Glide`) | `F7`, `E1`, `E2`, `E5`, `E6` (11 writes) | `3dfx Voodoo SVGA 640x480 16-bit mode 00h` | `srally.iso` mounted, `48000 Hz` CD-DA + `SB16 IRQ 7` |

---

## 6. Direct Guest RAM (`MemBase`) Gamepad Injection & `SDL_Joystick*` Fallback

- **`SDL_Joystick*` Fallback for Newer Controllers**:
  - `dosbox-staging v0.82.2`'s statically linked SDL2 does not include newer controller GUIDs (such as the `8BitDo Ultimate 2 Wireless Controller`, GUID `0300f6b6c82d00000b31000014010000`) in its built-in `SDL_GameControllerDB`, causing `SDL_GameControllerOpen(0)` to return `NULL`.
  - Falling back to `SDL_JoystickOpen` / `SDL_JoystickGetAxis` / `SDL_JoystickGetButton` / `SDL_JoystickGetHat` / `SDL_JoystickRumble` opens any connected XInput/evdev controller immediately and supports hot-plug/wake-from-sleep reconnection.
- **Bypassing DOS Port `0x201` Timing via `MemBase` + `PollControls` Record Skip**:
  - At `cpu_cycles_protected = max`, the game's DOS port `0x201` RC discharge loop (`DetectJoystick`, `max 0x800` iterations) overflows before the simulated capacitor discharges.
  - Across all 8 racing executables, `PollControls` checks `if (record[r].enabled == 1)` before updating `normalized_controls[r]`, while the car physics engine reads `normalized_controls[0..5]` directly without checking `record[r].enabled` (and applies instantaneous full-range `-1023..+1023` analog steering when `record[2].mode == 0`).
  - **CRITICAL — `MenuPollInput` Also Calls `PollControls` in Menus**:
    - In all 3 games (`STARTH.EXE` at `0x025550`, `S2_3DFX.EXE` at `0x03B1E4`, `STRT3FX.EXE` at `0x03B424`), `MenuPollInput` calls `PollControls` on every menu frame and checks `record[0..3]`: if `record[r].ctrl_type == 2` and `record[r].mode != 0` and `normalized_controls[r] != 0`, it converts `normalized_controls[r]` into synthetic menu Up/Down/Left/Right actions!
    - Writing `-1` into `normalized_controls[3]` or leaving `ctrl_type = 2` while `mode = 1` poisons `MenuPollInput`. Setting `record[r].enabled = 2`, `record[r].ctrl_type = 1`, `record[r].mode = 0`, and `normalized_controls[3] = 0` ensures `MenuPollInput` ignores `normalized_controls` in menus while the race physics engine reads `normalized_controls[0,1,2,4,5]` directly.
  - **CRITICAL — Watcom `LE` Keyboard State BSS Object vs. Data Object (`has_clean_kbd_array`)**:
    - In the Watcom `LE` object table, the 130-byte (`0x82`-byte) keyboard scancode state array lives in a separate preceding BSS `LE` object (`Object 2` in Screamer 1, `Object 5` in 3dfx binaries, `Object 3` in SVGA binaries), whereas `data_obj[0..130]` contains initialized ASCII strings (for example, `data_obj[2 + 72] == 'f'` = `102 != 0`).
    - Checking `data_obj[2 + scancode] != 0` without verifying `has_clean_kbd_array` (`data_obj[2..90].iter().all(|&b| b <= 1)`) mistakes the ASCII byte `'f'` at `data_obj[74]` for a held physical Up Arrow key (`scancode 72`), causing `inject_single_control_block` to reset `record[0].enabled = 1` and cut out gamepad `A`/`R2` throttle!
    - `inject_single_control_block` therefore checks `gp.throttle == 0 && is_key_down(...)` and requires `has_clean_kbd_array` (`data_obj[2..90]` all `0` or `1`) before yielding to keyboard scancodes.
  - **CRITICAL — Source-Level Hook in `vendor/dosbox-staging` (`GFX_Events()`) & Page-Aligned Scanning**:
    - Instead of `LD_PRELOAD` + `dlsym` + `clock_gettime` interception (which ran across background mixer/OPL/Voodoo threads and could deadlock SDL's non-reentrant joystick mutex if `SDL_JoystickUpdate` was called), `dosbox-staging` (`v0.82.2`) is included as a git submodule at `vendor/dosbox-staging` and patched in `src/gui/sdlmain.cpp` (`patches/dosbox-staging-screamer-hooks.patch`) to statically link `libscreamer_hooks.a` and invoke `screamer_hooks_on_tick(&api, now_ns)` on the main thread at the end of `GFX_Events()` right after `while (SDL_PollEvent(&event))`.
    - Always step by `4096`-byte pages in `locate_binary_data_object` and throttle uncached RAM scans to at most once every `100 ms` (`LAST_SCAN_ATTEMPT_NS`) so `dosbox-staging` is never CPU-starved while DOS/4GW is booting.

---

## 7. Why the 50 Hz (`0.02s`) SOS Timer & 50-Entry Physics Lookup Table Must Not Be Altered

- Across all three games (**Screamer**, **Screamer 2**, and **Screamer Rally**), the HMI SOS timer ticks at **100 Hz (`0.01s` per tick)**, and `UpdateFrameTiming` uses `smoothed_dt` (`clamped to [0.02, 0.20]`) to index a precomputed **50-entry physics lookup table** (`400` / `0x190` bytes per entry, indexed by `int(1.0 / smoothed_dt)` for `1..50 Hz`):
  1. **Screamer (1995) (`STARTH.EXE` at `0x01FC38..0x01FD62`, `STARTL.EXE` at `0x01FC28..0x01FD52`)**:
     - Has **no upper bounds check** on `int(1.0 / smoothed_dt)` before multiplying by `0x190` (`400`) and adding `0x80e8`.
     - If the `0.02s` (`DBL_0_02` / `FLT_0_02`) floor is lowered to `0.004s`, `int(1.0 / smoothed_dt)` exceeds `50` (up to `250`), reading out of bounds past `0xcf08` and crashing when a level loads.
     - Conversely, if the `2`-tick (`20 ms`) spin-wait at `0x01FC48` is lowered to `1` tick (`10 ms`) while keeping the `0.02s` floor, every `10 ms` frame advances car physics by `20 ms`, making the game run at **1.4x–2.0x speed**!
  2. **Screamer 2 (1996) (`S2_3DFX.EXE` at `0x050DEC`) & Screamer Rally (1997)**:
     - Milestone added a `>= 50.0` clamp on the physics table pointer (`entry 50`, which holds precomputed suspension/gravity/friction coefficients for `dt = 0.02s`).
     - If the `0.02s` `smoothed_dt` clamp is lowered to `0.004s`, `smoothed_dt` (`0.004s..0.01s`) no longer matches the `0.02s` coefficients in `entry 50` of the physics table, causing suspension forces on frame 1 to spike and **all cars to jump into the air and crash into each other at the start of a race**!
- **Rule**: Keep both the timer spin-wait and the `0.02s` (`50 Hz`) `smoothed_dt` floor completely unpatched (`code: &[]` for `E1`) across all 8 executables so all three games run at their exact 1.0x physics speed and suspension stability.

---

## 8. E6 Draw-Distance Verification

- Static verification covers both Screamer 1 renderers and all six supported Screamer 2/Rally renderers. Every E6 target matches the original instruction/data bytes and has zero LE relocation overlap.
- Original mode skips E6. A private-runtime differential test applies the required fixes to both variants, adds E6 only to Enhanced, and confirms that every differing byte lies inside a declared E6 range. SHA-256 checks before and after confirm the source game executables remain unchanged.
- Screamer 1's improvement is deliberately limited to managed animated objects: `40,000 -> 48,000` units, with `52,000 -> 62,400` coarse gates. It does not claim to extend authored road or terrain visibility.
- Screamer 2/Rally use a `52,000 -> 104,000` shared admission radius. Official track enumeration found at most 192 source records; a deliberately pessimistic five-collector bound is 960, below the shared 1,600-pair list capacity.
- A first out-of-line guard prototype entered apparent padding after the known-safe E2 camera stub and exited on the first rendered race frame. Three revised call/JMP encodings and five independent direct-JMP stubs reproduced the same failure, proving the suffix was indirectly reachable despite having no static cross-reference. Those variants were discarded.
- The final in-place E6 comparison completed a headless Screamer Rally China race segment without the former mid-level exit. It changes no instruction length, branch target, register use, stack state, object-table metadata, or relocation.
- Release-candidate verification benchmark (`hyperfine`, 3 warmups + 20 runs, full trilogy `--check`): origin/master v0.5.0 averaged `67.7 ± 10.4 ms` and v0.6.0 averaged `70.6 ± 12.2 ms`; peak memory remained `6.9 MiB`. The 4.2% mean delta is smaller than the run-to-run variance and does not establish a regression.

---

## 9. Screamer Rally (`STRT3FX.EXE`) & Screamer 2 (`S2_3DFX.EXE`) Texture Cache

- **Original analysis**:
  - In `STRT3FX.EXE` (`0x01C26C..0x01C29C`) and `S2_3DFX.EXE` (`0x01DBBC..0x01DBEC`), the 3dfx Glide texture cache manager stores cached texture descriptors in a fixed **1,024-entry array** (`MAXCACHE = 0x400`, 32 bytes per slot).
  - As textures are uploaded to TMU0, the game advances `next_tmu_addr` until it reaches `grTexMaxAddress(GR_TMU0)` (`tmu_max_addr`), at which point it resets `cache_count = 0` and wraps around.
  - On a real 1996 3dfx Voodoo 1 (`4 MB` total = `2 MB FBI + 2 MB TMU0`), `2 MB` of TMU RAM fits roughly `32..256` textures before wrapping around `tmu_max_addr`, so `cache_count` never reaches `1,024`.
  - When `voodoo_memsize = 12` (`4 MB FBI + 8 MB TMU0`) was configured, `tmu_max_addr` was `8 MB`, allowing more than `1,024` textures to be uploaded before `next_tmu_addr` wrapped around. On texture #1,025 (`cmp edx, 0x400; jle ...`), the game executed `printf("MAXCACHE\n"); exit(-1);`!
  - A separate software descriptor pool contains **3,064 entries of 56 bytes** (`0xBF8 * 0x38 = 0x29E40`) and has its own fatal exhaustion guard.
- **Correct configuration and boundary fix (2026-11-01)**:
  1. Keep `voodoo_memsize = 4` (`2 MB FBI + 2 MB TMU0`) so `grTexMaxAddress(GR_TMU0)` reports the authentic Voodoo 1 limit. The fixed 1,024-entry residency table and separate 3,064-entry descriptor pool remain unchanged.
  2. Merely reaching the title screen is not a sufficient cache test. A silent headless run that switched track previews reproduced the real exit with the original executable: `grTexDownloadMipMap: mipmap cannot span 2 Mbyte boundary`.
  3. The common upload bottleneck computes the exclusive texture end and compares it with `tmu_max_addr`, but its first-allocation path originally uses signed `jle` to continue uploading. Equality therefore submits a mipmap that reaches the 2 MiB boundary, which Glide rejects. Required fix `F7` changes only that condition byte from `jle` (`0x8E`) to `jl` (`0x8C`) at `VA 0x0CD857` in `S2_3DFX.EXE` and `VA 0x0CBDF7` in `STRT3FX.EXE`.
  4. Two cache-slot-reuse paths have a second bug: when they find a reusable descriptor, `jge` jumps directly to `grTexDownloadMipMap` and bypasses the TMU-end comparison. F7 NOPs those two-byte bypasses at `0x0CDA58` / `0x0CDC26` in Screamer 2 and `0x0CBFF8` / `0x0CC1C6` in Rally. All three paths now use each game's existing boundary check, TMU reset, and re-upload code; no replacement allocator or code cave is introduced.
  5. The earlier multi-site F7 cache-limit detour was invalid because it altered normal address-wrap control flow and itself caused the same Glide boundary abort. F8 addressed an unobserved descriptor-exhaustion path. Neither approach is retained.

---

## 10. Internal 2× (`1280×960`) 3dfx Voodoo Superscaling & `SDL_GameControllerUpdate` Polling

- **Internal 2× (`1280×960`) 3dfx Voodoo Rasterization (`SCREAMER_VOODOO_SCALE=2`)**:
  - Why external Glide wrappers (`nGlide` / `dgVoodoo2`) cannot be used: DOS/4GW loads `GLIDE2X.OVL` inside emulated 32-bit DOS memory and communicates with the emulated Voodoo 1 SST-1 device over PCI MMIO (`0xD0000000`) inside `vendor/dosbox-staging/src/hardware/voodoo.cpp`.
  - Because `dosbox-staging v0.82.2` uses a software triangle setup/rasterizer (`voodoo.cpp`), we extended `fbi_state` with `scale = 2` (enabled via `SCREAMER_VOODOO_SCALE=2` whenever an enhanced resolution is active):
    - Allocates `fbmemsize * scale * scale` (`8 MB` FBI RAM at 2× so two `1280×960×2` color buffers + one `1280×960×2` Z-buffer fit cleanly) while keeping `tmumem0 = 2 MB` untouched for `MAXCACHE` compatibility.
    - Scales `fbi.width`, `fbi.height`, `fbi.rowpixels`, and buffer offsets by `scale`.
    - Scales triangle vertex coordinates (`ax..cy` × `scale`) and divides screen-space parameter gradients (`drdx..dwdy`, `dsdx..dwdy` / `scale`) inside `triangle()` with a `ScaleRestoreGuard` so subsequent triangles sharing vertices are unaffected.
    - Scales `clipLowYHighY` and `clipLeftRight` by `scale` in `raster_generic` and `fastfill`, widens the `scry` scanline mask from `& 0x3ff` (`1023`) to `& 0xfff` (`4095`), and scales 2D `lfb_w` / `lfb_r` writes into `scale × scale` pixel blocks so the 2D HUD and menus overlay cleanly on top of the `1280×960` 3D scene.
- **Explicit `SDL_GameControllerUpdate()` / `SDL_JoystickUpdate()` on Main Thread**:
  - Because `dosbox-staging`'s `sdl_mapper.cpp` calls `SDL_JoystickEventState(SDL_DISABLE)`, `SDL_PollEvent` does not automatically pump SDL joystick/controller axes (`SDL_CONTROLLER_AXIS_TRIGGERLEFT` / `TRIGGERRIGHT`).
  - Calling `SDL_GameControllerUpdate()` and `SDL_JoystickUpdate()` directly on the main thread inside `GFX_Events()` right before `screamer_hooks_on_tick()` ensures `RT` (`R2`) and `LT` (`L2`) trigger axes and stick axes are always fresh every frame without any background-thread mutex contention.

---

## 11. Why Native DLSS / FSR 2–3 Cannot Be Used on DOS/Glide, and How FSR 1.0 + Optical-Flow Frame Generation (`50 -> 100 FPS`) Work

- **Why DLSS 2/3 & FSR 2/3 Cannot Attach Directly to a DOS Engine**:
  - Temporal upscalers and frame generators (NVIDIA DLSS Super Resolution / DLSS 3 Frame Generation and AMD FSR 2/3) require DirectX 12 or Vulkan command queues plus engine-supplied **per-pixel 3D motion vectors (`MV_x, MV_y`)**, **sub-pixel camera projection jitter**, and a **linear depth buffer**.
  - Neither the 1995–1997 DOS software rasterizers nor the 3dfx Glide 2.x API generate motion vectors or jittered projection matrices, and `dosbox-staging` presents final 2D RGBA frames over **OpenGL**.
- **Hardware-Agnostic Solution 1 — AMD FSR 1.0 (`EASU + RCAS`) in GLSL (`interpolation/fsr.glsl`)**:
  - Implemented in `vendor/dosbox-staging/contrib/resources/glshaders/interpolation/fsr.glsl` and selectable as **FSR (Super Res)** in the Launcher's **Picture** tab:
    - **EASU (Edge-Adaptive Spatial Upsampling)**: Computes local luminance gradients across a 12-tap cross neighborhood around each texel and stretches an elliptical Lanczos2 kernel along detected edge directions, clamped to the inner 2×2 neighborhood min/max to eliminate ringing.
    - **RCAS (Robust Contrast-Adaptive Sharpening)**: Applies a 5-tap contrast-bounded sharpening pass after upscaling.
- **Hardware-Agnostic Solution 2 — GPU Optical-Flow Frame Generation (`50 FPS -> 100 FPS`, `SCREAMER_FRAME_GEN=1`)**:
  - Because the DOS game's internal SOS timer and 50-entry physics lookup table must remain at `50 Hz` (`20 ms` per frame), presentation-layer Frame Generation in `vendor/dosbox-staging/src/gui/sdlmain.cpp` (`ScreamerFrameGenState`) maintains two OpenGL textures (`GL_TEXTURE0` = current frame $F_t$, `GL_TEXTURE1` = previous frame $F_{t-1}$):
    - At `t = 0 ms` (when `update_frame_gl` receives a new 50 Hz frame from the DOS game), `present_frame_gl()` renders with `rubyFrameBlend = 0.5`, invoking a bidirectional optical-flow motion search in the fragment shader (`fsr.glsl`, `sharp.glsl`, `bilinear.glsl`) that protects static HUD/UI pixels (`|L_curr - L_prev| < 0.015`) while warping moving 3D geometry along the best-matching motion vector $\vec{v}$ with disocclusion fallback.
    - At `t = +10 ms` (checked inside `GFX_Events()` on the main thread), `present_frame_gl()` renders the exact unblended frame $F_t$ (`rubyFrameBlend = 0.0`), doubling the output frame rate from **50 FPS to 100 FPS** with zero impact on game timers or physics.

---

## 12. Pending Real-Game Checks (Launcher Normalisation, Unreleased)

These launcher changes are covered by unit tests but have not yet been run against the real games:

- **Runtime staging without links.** The runtime folder is now built from real directories with one file at a time: `.EXE`/`.COM`/`.386`/`.OVL`/`.DLL`/`.ICO`/`.PDF` files are hard-linked when the runtime shares a volume with the game folder, everything else (including `DATA/`, `GFX/`, `TABS/`) is copied, and the CD image plus the files its cue sheet names stay mounted from the source. Check on each game that a race starts, CD audio plays, `CHOICE.DAT`/replays are written in the runtime folder, and that the game folder's modification times are unchanged afterwards. If the games never write into `DATA/`, `GFX/` or `TABS/`, those could be hard-linked too to save space; that is unverified, so they are copied for now. A runtime folder made by an older launcher (symbolic links) is converted on the next launch.
- **`resolution = fullscreen` under gamescope (Linux).** `screamer-play` now gives gamescope the screen size (current xrandr mode) for `fullscreen`. Check on a desktop and on a Steam Deck that DOSBox fills the screen and the xrandr probe reports physical pixels on a scaled Wayland output (if it reports a scaled size, the picture is only upscaled by gamescope, not broken).
- **Race sizes above 1080p.** `resolution = 2560x1440` (or larger) now reaches DOSBox as `windowresolution` instead of becoming fullscreen. Check on a 1440p/4K screen that DOSBox opens the window at that size, and on a smaller screen that the launcher warns and plays fullscreen.
- **`screamer-play stop` and supervision.** Tested with fake launchers only. Check with the real game in gamescope that `screamer-play stop` from another terminal ends gamescope, the launcher and DOSBox together, that the saves are still copied back when the game is quit normally, and that `status` reports `running` during a race.
- **No settings window.** With no display or OpenGL the launcher now plays with the saved settings (exit code 4 from `--settings`). Check that a Steam Deck in Game Mode and a headless gamescope session still show the window when they can, and that `launcher.log` records the fallback when they cannot.
- **Save sync state (`.screamer-saves`).** Tested with synthetic files only. Check with a real career that progress written in a race survives a quit, a killed DOSBox (next launch copies it back), and a `CHOICE.DAT` restored from a backup into the saves folder.
- **CD image sets left in the game folder.** With a mounted image every `.iso/.gog/.bin/.img/.cue/.ins/.ccd/.sub/.mds/.mdf` file is no longer copied into the runtime. Check that none of the three GOG/ISO layouts needs such a file on drive `C:` (e.g. a data `.BIN`), and that CD audio still plays.
- **Settings window on rea-launcher-sdk v0.2.0.** The window now uses the SDK's widgets and the Picture tab was reorganised (four switches in two pairs, a help board under the settings). Its layout was checked only in the headless harness (at 980×700 since rea-launcher-sdk v0.3.0): check it once by eye on a desktop, a Steam Deck (1280×800 at 1.25× scaling) and a 4K screen at 200%, with the keyboard (Tab, arrows, Space, Enter) and a gamepad.
- **First run saves the recommended look.** A missing `launcher.cfg` is now written with the recommended look's values for the detected screen (Steam Deck look on a Deck, Enhanced elsewhere). Check on a Steam Deck that `--no-window` on a first run plays at 1280×800.
- **`frame_generation` replaces `fps_limit`.** `fps_limit = 30` and `50` were the same run (the `E1` fix has no bytes), so the window has one Frame generation switch; `fps_limit` of 60 or more is read as `frame_generation = on`. Nothing changes in the game; check once that Frame generation still shows 100 FPS and that a 0.6.0 `launcher.cfg` with `fps_limit = 100` still turns it on.
- **Vsync "As the game" removed.** It wrote no `vsync` key, which DOSBox ran as `vsync = auto` (the same as On). The Original look now saves `vsync = on`; the DOSBox configuration is unchanged.
- **Unknown builds refused.** A `STARTH.EXE` (or another game executable name) with an unknown SHA-256 is now reported with its hash and the issue link (window, `--check`, `launcher.log`) instead of being skipped silently. Verified with synthetic files only.
- **One-run `--scaler` / `--resolution` (rea-launcher-sdk v0.3.0).** `screamer-play --play --scaler pixel --resolution 1280x720` now gives gamescope those values for that run only (`REA_HELPER_KEYS`), and `launcher.cfg` keeps its own (`--scaler` used to be saved into it). Tested with a fake launcher (`linux/tests/scaler.sh`). Check once with the real game that `play.log` shows `-F pixel -w 1280 -h 720`, and that a plain `--play` afterwards uses `launcher.cfg`'s scaler again.
- **Windows build linked with LLD.** The x86_64 `ScreamerLauncher.exe` and its tests now run under Wine 11.21 (they crashed before `main` with GNU ld 2.47). Not yet run on real Windows 10/11.

## 13. Verification Matrix (Options × Platforms)

What has been seen in the real game, per option and platform. "headless" is a run in `gamescope --backend headless`; "unit" means only the launcher's tests cover it (the setting reaches the right file, variable or patch, but its effect in the game was not looked at).

| Option | Linux desktop | Steam Deck | Windows 10/11 | 1440p / 4K | Ultrawide |
|---|---|---|---|---|---|
| `renderer` (3dfx / SVGA / VGA) | headless (§5) | not yet | not yet | not yet | not yet |
| `resolution` up to 1080p | 1080p in the gameplay video | not yet | not yet | — | — |
| `resolution` above 1080p, `fullscreen` | unit | not yet | not yet | not yet (§12) | not yet |
| `widescreen` (E2) | Screamer 1: in the gameplay video (owner to confirm it shows Hor+, not a stretch); Screamer 2/Rally not yet | not yet | not yet | not yet | not yet (16:9 math only) |
| `extended_draw_distance` (E6) | headless Rally segment (§8) | not yet | not yet | — | — |
| `frame_generation` | implemented (§11); no recorded run confirms 100 FPS | not yet | not yet | not yet | — |
| `vsync` | unit (DOSBox setting) | not yet | not yet | — | — |
| `texture_filter` | Sharp in the gameplay video; CRT, FSR, Smooth not recorded | not yet | not yet | not yet | — |
| `gamepad_axes`, `buttons`, `rumble` | used with an 8BitDo Ultimate 2 (§6) | not yet | not yet | — | — |
| `deadzone` (E5, DOSBox gameport) | not yet (the launcher's injection bypasses it) | not yet | not yet | — | — |
| `scaler` (gamescope, Linux helper) | unit (`linux/tests/scaler.sh`) | not yet | — | not yet | — |
| Looks: Original / Enhanced / Steam Deck | Enhanced in the gameplay video | Steam Deck look not yet tried on the device | not yet | — | — |
| Window focus loss mid-race | not yet | not yet | not yet | — | — |

The Steam Deck look is made only of individually verified values and ships before a device check (SDK standard); multi-monitor, HiDPI and Proton-without-the-helper are not applicable or not yet checked: the launcher is native on both platforms.

### Generated game files

`SCREAMER.CFG`, `CONFIG/SETUP.CFG` and `CONFIG/CONTROLS.CFG` are rewritten at every launch. Whether a game writes `CONTROLS.CFG` itself (an in-game key redefinition) has not been established: the menus were not driven to that screen, and the executables were not searched for a write of that file. If a game does write it, a player's in-game key bindings are lost at the next launch. Recommendation for the owner: verify in the real game; if it writes the file, seed it only when it is missing (or still byte-identical to what the launcher last wrote), as the SDK standard asks for game config files. `SCREAMER.CFG` and `SETUP.CFG` hold paths and the sound card, which only `SETUP.EXE`/`ST.EXE` write, and the launcher never runs those.
