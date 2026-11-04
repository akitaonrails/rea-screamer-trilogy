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

These launcher changes are covered by unit tests. The 2026-11-02 headless run on the owner's installs (§14) checked most of them; each item says what it showed.

- **Runtime staging without links.** The runtime folder is now built from real directories with one file at a time: `.EXE`/`.COM`/`.386`/`.OVL`/`.DLL`/`.ICO`/`.PDF` files are hard-linked when the runtime shares a volume with the game folder, everything else (including `DATA/`, `GFX/`, `TABS/`) is copied, and the CD image plus the files its cue sheet names stay mounted from the source. Check on each game that a race starts, CD audio plays, `CHOICE.DAT`/replays are written in the runtime folder, and that the game folder's modification times are unchanged afterwards. If the games never write into `DATA/`, `GFX/` or `TABS/`, those could be hard-linked too to save space; that is unverified, so they are copied for now. A runtime folder made by an older launcher (symbolic links) is converted on the next launch.
  - *2026-11-02:* VERIFIED on all three games (§14): races start, CD-DA plays from the mounted image, code and drivers are hard-linked (link count 2; the shared genuine `GLIDE2X.OVL` 4), patched executables and `DATA/`/`GFX/`/`TABS/` are copies, no symbolic links. Screamer wrote `CONFIG/CHOICE.DAT` in the runtime folder during a race; Screamer 2 and Rally wrote nothing in 15-second races (their `CHOICE.DAT` is written later in a career; not reached headless). The game folder (1,042 entries) has the same names, sizes, modification times and modes after all runs. Whether the games ever write `DATA/`/`GFX/`/`TABS/` was not checked; they stay copied.
- **`resolution = fullscreen` under gamescope (Linux).** `screamer-play` now gives gamescope the screen size (current xrandr mode) for `fullscreen`. Check on a desktop and on a Steam Deck that DOSBox fills the screen and the xrandr probe reports physical pixels on a scaled Wayland output (if it reports a scaled size, the picture is only upscaled by gamescope, not broken).
  - *2026-11-02:* NOT CHECKABLE HEADLESS (§14): without a display xrandr has nothing to report, so the helper used its 1920x1080 default. A person must check it on a desktop and on a Deck.
- **Race sizes above 1080p.** `resolution = 2560x1440` (or larger) now reaches DOSBox as `windowresolution` instead of becoming fullscreen. Check on a 1440p/4K screen that DOSBox opens the window at that size, and on a smaller screen that the launcher warns and plays fullscreen.
  - *2026-11-02:* PARTLY VERIFIED (§14): `resolution = 2560x1440` reached DOSBox as a 2560x1440 window and gamescope was started at 2560x1440. Under the helper's gamescope the launcher's screen check reads gamescope's own nested 2560x1440 display through xrandr, so the "larger than the screen, playing fullscreen" warning never fires there; gamescope fits the 1440p picture to the real screen instead. The warning path (no gamescope, smaller screen) and a real 1440p/4K screen need a person.
- **`screamer-play stop` and supervision.** Tested with fake launchers only. Check with the real game in gamescope that `screamer-play stop` from another terminal ends gamescope, the launcher and DOSBox together, that the saves are still copied back when the game is quit normally, and that `status` reports `running` during a race.
  - *2026-11-02:* VERIFIED (§14): `status` says `running` during a race and `idle` after; `stop` ends gamescope, Xwayland, the launcher and DOSBox in 0.4 s; after a normal quit the saves are copied back (`dosbox ended: exit status: 0`, saves folder updated).
- **No settings window.** With no display or OpenGL the launcher now plays with the saved settings (exit code 4 from `--settings`). Check that a Steam Deck in Game Mode and a headless gamescope session still show the window when they can, and that `launcher.log` records the fallback when they cannot.
  - *2026-11-02:* VERIFIED for the headless case (§14): `screamer-play` with no display logs `settings window unavailable (winit EventLoopError …); playing with saved settings` in `launcher.log`, the helper prints `no settings window could open; playing with the saved settings`, and the game starts. The Deck Game Mode case needs a device.
- **Save sync state (`.screamer-saves`).** Tested with synthetic files only. Check with a real career that progress written in a race survives a quit, a killed DOSBox (next launch copies it back), and a `CHOICE.DAT` restored from a backup into the saves folder.
  - *2026-11-02:* VERIFIED with a real Screamer `CHOICE.DAT` (§14): (a) race then a normal DOSBox exit: the saves folder gets the race's new file at once; (b) race ended by `screamer-play stop` (launcher killed, nothing copied): the next launch copies the runtime's newer file to the saves folder; (c) an older backup copied into the saves folder replaces the runtime's file at the next launch.
- **CD image sets left in the game folder.** With a mounted image every `.iso/.gog/.bin/.img/.cue/.ins/.ccd/.sub/.mds/.mdf` file is no longer copied into the runtime. Check that none of the three GOG/ISO layouts needs such a file on drive `C:` (e.g. a data `.BIN`), and that CD audio still plays.
  - *2026-11-02:* VERIFIED (§14): no `.gog/.iso/.dat` CD image or cue sheet in any runtime folder; all three games mount theirs from the game folder (`IMGMOUNT … found`, `CDAUDIO: Operating at 48000 Hz`), races start, and the audio output is not silent (RMS 0.07–0.19 in attract mode and races).
- **Settings window on rea-launcher-sdk v0.2.0.** The window now uses the SDK's widgets and the Picture tab was reorganised (four switches in two pairs, a help board under the settings). Its layout was checked only in the headless harness (at 980×700 since rea-launcher-sdk v0.3.0): check it once by eye on a desktop, a Steam Deck (1280×800 at 1.25× scaling) and a 4K screen at 200%, with the keyboard (Tab, arrows, Space, Enter) and a gamepad.
  - *2026-11-02:* Still needs eyes on real screens. The README screenshots were retaken headless on 2026-11-02 from the current window and match the committed images.
- **First run saves the recommended look.** A missing `launcher.cfg` is now written with the recommended look's values for the detected screen (Steam Deck look on a Deck, Enhanced elsewhere). Check on a Steam Deck that `--no-window` on a first run plays at 1280×800.
  - *2026-11-02:* VERIFIED on a desktop (§14): with no `launcher.cfg`, the launcher wrote the Enhanced look (1920x1080, widescreen, longer view, Sharp) and played. The Deck case needs a device.
- **`frame_generation` replaces `fps_limit`.** `fps_limit = 30` and `50` were the same run (the `E1` fix has no bytes), so the window has one Frame generation switch; `fps_limit` of 60 or more is read as `frame_generation = on`. Nothing changes in the game; check once that Frame generation still shows 100 FPS and that a 0.6.0 `launcher.cfg` with `fps_limit = 100` still turns it on.
  - *2026-11-02:* VERIFIED (§14): a 0.6.0 `launcher.cfg` with `fps_limit = 100` set `SCREAMER_FRAME_GEN=1`, and DOSBox presented 100–101 frames a second in a Screamer race (swap count), against 50 without it. DOSBox switched vsync off for it (`OPENGL: Disabled vsync`: the headless output is 60 Hz, below 100). In Rally at the Steam Deck size it presented 51–63 frames a second: frame generation doubles what the 2× Voodoo renderer manages there (25–33).
- **Vsync "As the game" removed.** It wrote no `vsync` key, which DOSBox ran as `vsync = auto` (the same as On). The Original look now saves `vsync = on`; the DOSBox configuration is unchanged.
- **Unknown builds refused.** A `STARTH.EXE` (or another game executable name) with an unknown SHA-256 is now reported with its hash and the issue link (window, `--check`, `launcher.log`) instead of being skipped silently. Verified with synthetic files only.
  - *2026-11-02:* VERIFIED with real files (§14): the owner's Screamer 2 and Rally folders hold a `STARTL.EXE` the launcher does not know; `--check` and every launch log its SHA-256 and the issue link, and the known executables still play.
- **One-run `--scaler` / `--resolution` (rea-launcher-sdk v0.3.0).** `screamer-play --play --scaler pixel --resolution 1280x720` now gives gamescope those values for that run only (`REA_HELPER_KEYS`), and `launcher.cfg` keeps its own (`--scaler` used to be saved into it). Tested with a fake launcher (`linux/tests/scaler.sh`). Check once with the real game that `play.log` shows `-F pixel -w 1280 -h 720`, and that a plain `--play` afterwards uses `launcher.cfg`'s scaler again.
  - *2026-11-02:* VERIFIED (§14): `--play --scaler pixel --resolution 1280x720` started gamescope with `starting at 1280x720, scaler pixel`, DOSBox opened a 1280x720 window, `launcher.cfg` was unchanged, and the next plain `--play` started at 1920x1080 with `fsr` again.
- **Clean exit, single instance (rea-launcher-sdk v0.3.0).** Tested with fakes only (two locks in one test, the Windows mutex under Wine). The launcher starts DOSBox and waits for it; it never stops the game itself, so the SDK's `close_gracefully` has no place to go yet. Pending with the real game, per platform (Windows 10/11, Linux desktop, Steam Deck Game Mode): quit from each game's own menu; Alt+F4 and DOSBox's close button; Steam's Stop on the Deck; `screamer-play stop` and Ctrl+C on the helper; closing gamescope; closing the console a Windows `ScreamerLauncher.exe` was started from (does DOSBox end with it, and are the saves copied back?). After each, within a few seconds: no DOSBox, no `ScreamerLauncher`, no gamescope, and `.screamer-running` free (the next Play works). Also a second double-click while the window is open (it comes forward) and while a race runs (the "already running" message).
  - *2026-11-02:* VERIFIED on the Linux desktop path (§14); see the table in §14. Pending: Windows 10/11, the Deck and Steam's Stop, and the game's own Quit in Screamer 2 and Rally.
- **Windows build linked with LLD.** The x86_64 `ScreamerLauncher.exe` and its tests now run under Wine 11.21 (they crashed before `main` with GNU ld 2.47). Not yet run on real Windows 10/11.
  - *2026-11-02:* Not run in this pass (§14): Linux only.

## 13. Verification Matrix (Options × Platforms)

What has been seen in the real game, per option and platform. "headless" is a run in `gamescope --backend headless`; "unit" means only the launcher's tests cover it (the setting reaches the right file, variable or patch, but its effect in the game was not looked at).

| Option | Linux desktop | Steam Deck | Windows 10/11 | 1440p / 4K | Ultrawide |
|---|---|---|---|---|---|
| `renderer` (3dfx / SVGA / VGA) | headless (§5); 3dfx in Screamer 2/Rally races and SVGA 8-bit in Screamer races (§14) | not yet | not yet | not yet | not yet |
| `resolution` up to 1080p | 1080p in the gameplay video | not yet | not yet | — | — |
| `resolution` above 1080p, `fullscreen` | 2560x1440 window under gamescope (§14); `fullscreen` not checkable headless | not yet | not yet | not yet (§12) | not yet |
| `widescreen` (E2) | all three games in headless races: the 3D view looks Hor+ (same car size, more on the sides), the 2D HUD is stretched; screenshots for the owner (§14) | not yet | not yet | not yet | not yet (16:9 math only) |
| `extended_draw_distance` (E6) | headless Rally segment (§8) | not yet | not yet | — | — |
| `frame_generation` | 100 FPS presented in a Screamer race; 51–63 in Rally at 1280x800 (§14) | not yet | not yet | not yet | — |
| `vsync` | `OPENGL: Enabled non-adaptive vsync` with it on; frame generation switches it off below 100 Hz (§14) | not yet | not yet | — | — |
| `texture_filter` | Sharp in the gameplay video; CRT, FSR, Smooth not recorded | not yet | not yet | not yet | — |
| `gamepad_axes`, `buttons`, `rumble` | used with an 8BitDo Ultimate 2 (§6) | not yet | not yet | — | — |
| `deadzone` (E5, DOSBox gameport) | not yet (the launcher's injection bypasses it) | not yet | not yet | — | — |
| `scaler` (gamescope, Linux helper) | one-run `--scaler pixel` reached gamescope (§14) | not yet | — | not yet | — |
| Looks: Original / Enhanced / Steam Deck | all three, all three games, headless races (§14; screenshots for the owner) | Steam Deck look not yet tried on the device | not yet | — | — |
| Window focus loss mid-race | not yet | not yet | not yet | — | — |

The Steam Deck look is made only of individually verified values and ships before a device check (SDK standard); multi-monitor, HiDPI and Proton-without-the-helper are not applicable or not yet checked: the launcher is native on both platforms.

### Generated game files

`SCREAMER.CFG`, `CONFIG/SETUP.CFG` and `CONFIG/CONTROLS.CFG` are written at every launch (only when their bytes differ). **The games never write `CONTROLS.CFG` (established 2026-11-02, static evidence plus the runs).** In every race executable (`STARTH.EXE`, `STARTL.EXE` of Screamer; `S2_3DFX.EXE`, `START65H.EXE` of Screamer 2; `STRT3FX.EXE` and the other `START*`/`STRT*` of Rally) the C library strings come in path/mode pairs: `%s\config\controls.cfg` is followed only by `rb`, and `setup.cfg` only by `rb`, while `choice.dat`, `replays\%s.rpl` and `.atk` have both an `rb` and a `wb` entry. The only writer of `CONTROLS.CFG` is the setup program (Rally's `ST.EXE`: "Cannot save CONTROLS.CFG file."), which the launcher never runs. In the runs, Screamer's runtime `CONFIG/CONTROLS.CFG` kept the launcher's first write time through eight launches. So rewriting it loses nothing the games save; no change is needed. A player who runs `SETUP.EXE`/`ST.EXE` by hand inside the runtime folder would lose those bindings at the next launch.

## 14. Real-Game Verification, 2026-11-02 (Linux, Headless)

**Setup.** The owner's GOG Screamer and Screamer 2 and the Screamer Rally ISO install (`/mnt/data/distrobox/gaming/tools/rea-screamer/games`, read-only), the Linux release build of `ScreamerLauncher` (`cargo build --release --locked -p screamer-launcher`) and the DOSBox from `scripts/build-dosbox.sh`, run through `linux/screamer-play --play` inside the `gaming` distrobox. NVIDIA RTX 5090, gamescope 3.17.0. Each run had its own HOME/XDG folders under `.work/realgame/home`, a private `XDG_RUNTIME_DIR`, a scrubbed environment (`env -i` plus the paths it needs), and a `gamescope` shim first in `PATH` that adds `--backend headless`. Audio went to SDL's `disk` driver (`SDL_AUDIODRIVER=disk`), not a PipeWire sink, so the output could be measured. A supervisor with a finite deadline waited for DOSBox, drove the menus with XTEST key presses on gamescope's Xwayland, took screenshots with `gamescopectl screenshot`, sent the quit, timed each process's exit, and checked the process table (by a tag in the environment, gamescope by command line) for leftovers, zombies included. An `LD_PRELOAD` counter on `SDL_GL_SwapWindow` (through `SCREAMER_DOSBOX` pointing at a small wrapper) logged presented frames per second. Commands: `distrobox enter gaming -- bash .work/realgame/bin/batch{1,2,3,4}.sh`; logs, swap counts and screenshots in `.work/realgame/out/<run>/`, a selection in `.work/realgame/evidence/` (none committed).

| Check | Result | Evidence |
|---|---|---|
| Original look, all three games | VERIFIED: race reached; Screamer `VESA 640x480 256-colour mode 101h` with the CRT shader (`crt/vga-1080p`) in a 1280x960 window; Screamer 2 and Rally on the Voodoo (`VOODOO: Initialized with 4 MB`), 1280x960 | `out/{s1,s2,rally}-original/` |
| Enhanced look | VERIFIED: 1920x1080 window, `interpolation/sharp`, `SCREAMER_VOODOO_SCALE=2` for 3dfx, E2/E6 applied (`fixes left out: none`) | `out/*-enhanced/` |
| Steam Deck look | VERIFIED on a desktop: 1280x800 window, Sharp, widescreen; gamescope at 1280x800 | `out/*-deck/`; DOSBox environment equal to `tests/snapshots/look-steam-deck-rally-3dfx.txt` but for the GPU variables |
| Widescreen in Screamer 2 and Rally | Screenshot for the owner: next to the 4:3 picture the 3D view keeps the car's and the start gate's proportions and shows more at the sides (Hor+); the 2D HUD (speedometer, lap panel) is stretched to 16:9 | `evidence/*-race-original-enhanced-deck.jpg` |
| Frame rate in races | Original look: 50 presented FPS in all three. Enhanced and Steam Deck looks: Screamer 50, Screamer 2 40–47, Rally 25–33. The game clock stays real-time (Rally's lap timer advanced 7.51 s between screenshots 7.51 s apart): the 2× software Voodoo renderer drops frames, the race does not slow down. Other launchers' tests were running on the machine (load average about 5) | `out/*/swaps.txt`, `evidence/rally-enhanced-lap-timer-7.5s-apart.jpg` |
| Frame generation | VERIFIED: 100–101 presented FPS in a Screamer race; 51–63 in Rally at 1280x800 | `out/fg-v060-fps100-s1/`, `out/fg-rally-deck/` |
| Quit: the game's own menu (Screamer) | VERIFIED: Esc in a race, "Play again? No", Esc in the main menu: DOSBox exits 0 at once, saves copied back | `out/s1-enhanced/`, `evidence/screamer1-own-quit-path.jpg` |
| Quit: the game's own menu (Screamer 2, Rally) | NOT VERIFIED: Rally's in-race Esc menu restarted the race with the same keys, and Esc from its name screen returns to the intro; the quit item was not found by blind key presses. A person must quit once from each game's menus | `out/rally-original/`, `out/rally-esc/` |
| Quit: window close (`WM_DELETE_WINDOW` to DOSBox's window) | VERIFIED: DOSBox exits 0 and the launcher copies the saves back in 0.1–0.3 s (once 5.1 s); gamescope then needs 1.9–5.4 s (see below) | `out/timing-close/`, `out/s1-original/`, `out/s2-deck/` |
| Quit: `screamer-play stop` | VERIFIED: launcher and DOSBox in 0.03 s, everything in 0.39 s | `out/timing-stop/` |
| Quit: SIGINT / SIGTERM to the helper | VERIFIED: everything in 0.38 s (exit 143) | `out/timing-int/`, `out/timing-term/` |
| Quit: SIGTERM to gamescope | VERIFIED: everything in 0.38 s; the launcher is ended too, so the saves are copied at the next launch | `out/timing-gamescope/` |
| Nothing left behind | VERIFIED after every run: no tagged process, no zombie, no gamescope or Xwayland of ours; `.screamer-running` free (the next Play always started) | `out/*/after.txt` |
| gamescope's own exit | gamescope 3.17.0's headless backend crashes (SIGSEGV, core dumped) when it shuts down after its child ended by itself; the helper then exits 139. It is gamescope's crash, not the launcher's, and it is what makes a window close take up to about 5 s | `out/*/helper.out` |
| Single instance | VERIFIED: during a race, `screamer-play --play` and `screamer-play` print `already running` (exit 0) and start nothing; `ScreamerLauncher --no-window` refuses with "The game is already running from …runtime/screamer. Quit it first, then press Play again." (exit 2); the process list is unchanged | `out/single-instance/` |
| Windows second start | Not run. DOSBox's window is titled `STARTH.EXE - max cycles/ms - …` with SDL's class, so it cannot match the settings window lookup (`Window Class`, `Screamer Trilogy`) | `out/s1-original/xctl.log` |
| CD audio | VERIFIED: images mounted from the game folder, non-silent output; a 12 s sample of Screamer's attract mode to listen to | `evidence/screamer1-attract-audio-12s.opus` |
| Game folder untouched | VERIFIED: same names, sizes, modification times and modes for all 1,042 entries | `out/gamefolder-{before,after}.tsv` |

Not checked here: Windows 10/11 and Wine, the Steam Deck, real 1440p/4K and ultrawide screens, `fullscreen`, controllers, focus loss.

## 15. Pending Real-Game Checks (rea-launcher-sdk v0.4.1, Unreleased)

Covered by unit, headless-window and shell tests; each needs one headless run with the real games (as in §14) before a release.

- **`--look` / `look =`.** Each look (`--play --look original|enhanced|steam_deck`) reaches the game with that look's window size, shader and fixes, and gamescope starts at the look's size (`REA_LOOK_SIZES`: Steam Deck 1280x800, Enhanced and Original 1920x1080) unless a `resolution` says otherwise. `launcher.cfg` is unchanged by a one-run `--look`.
- **Play-time errors under the Linux helper.** `screamer-play` now starts the game run with `--no-window --linux`, so an error before the game (no game folder, an unknown build, a busy runtime folder) opens the SDK's message window on a desktop (and is still printed to `play.log` and `launcher.log`). Headless, the window opens on gamescope's display.
- **Second start exits 5.** During a race, `ScreamerLauncher --no-window` and a direct second `ScreamerLauncher` (not through the helper) are refused with exit 5 (they exited 2, or opened a second window on Linux); `screamer-play --play` says `already running` and exits 5 (the SDK helper's code since v0.4.0; it was 0).
- **Quit paths again** (§14's table: the game's own menu, window close, `screamer-play stop`, SIGINT/SIGTERM, gamescope closed): nothing left behind, now also no `rea-launcher-rea-screamer.lock` holder (the lock file stays; the kernel releases it).

### Results, 2026-11-03 (Linux, headless)

**Setup.** As §14: the owner's three installs (read-only), the Linux release build of `ScreamerLauncher` and `scripts/build-dosbox.sh`'s DOSBox, run through `linux/screamer-play` in the `gaming` distrobox with a `gamescope` shim that adds `--backend headless`, a private HOME/XDG tree and `XDG_RUNTIME_DIR` per run, `env -i` with a minimal environment, and audio to SDL's `disk` driver (no device). A new supervisor (`.work/rg041/bin/run.sh`, not committed) found our processes by name (`pgrep -x`, pids only) and kept those whose executable is in this repository or whose command line names the harness folder (compared, never printed); it only signalled those, wrote only pid, state and command name, and counted zombies as leftovers. Keys and window closes went through XTEST/X11 on gamescope's Xwayland; screenshots with `gamescopectl`.

| Check | Result |
|---|---|
| `--play --look original` (Screamer, over an Enhanced `launcher.cfg`) | VERIFIED: 1280x960 window, `crt-auto` shader, E2/E6 left out; `launcher.cfg` unchanged |
| `--play --look steam_deck` (Screamer) | VERIFIED: gamescope started at 1280x800, DOSBox window 1280x800, `sharp`, E2/E6 applied; `launcher.cfg` unchanged |
| `look = enhanced` alone in `launcher.cfg` (Screamer) | VERIFIED: gamescope and DOSBox 1920x1080, `sharp`, no fix left out |
| `look = steam_deck` alone (Rally) | VERIFIED: gamescope 1280x800, DOSBox 1280x800, 3dfx at 2× (`SCREAMER_VOODOO_SCALE=2`); the attract screen showed |
| `look = original` alone (Screamer 2) | VERIFIED: 1280x960, `crt-auto`, E2/E6 left out; gamescope 1920x1080 as for `resolution = original` |
| Play-time error under the helper: game folder missing | VERIFIED: the launcher's message window ("Screamer Trilogy", the error, Copy details, OK) opened on gamescope's display; closing it ended the run; also in `play.log` and `launcher.log` |
| Play-time error: unknown build (a synthetic `STARTH.EXE` in a scratch folder) | VERIFIED: the window shows the file, its SHA-256 and a **Report the edition** button with the unknown-edition form |
| Second start during a race | VERIFIED: `screamer-play --play` and `screamer-play` print `already running`, exit 5; `ScreamerLauncher --no-window` and a plain `ScreamerLauncher` started directly print the "already running" message, exit 5; nothing new started (same four processes) |
| Quit: the game's own menu (Screamer) | VERIFIED: DOSBox exits 0, everything gone 0.8 s after the last key |
| Quit: window close (`WM_DELETE_WINDOW`; Screamer, Rally) | VERIFIED: DOSBox exits 0, everything gone in 0.7–0.8 s |
| Quit: `screamer-play stop` | VERIFIED: everything gone in 0.36 s (helper exit 0) |
| Quit: SIGINT / SIGTERM to the helper | VERIFIED: 0.35 s / 0.74 s (exit 143) |
| Quit: gamescope closed (SIGTERM) | VERIFIED: everything gone in 0.74 s |
| Quit: Alt+F4 as a key press (XTEST) | NOT APPLICABLE headless: gamescope has no window manager to turn Alt+F4 into a close request, and DOSBox ignores the key itself; the run ended at the supervisor's deadline (helper SIGTERM) with nothing left. On a desktop Alt+F4 is the window manager's `WM_DELETE_WINDOW`, verified above |
| Nothing left behind, zombies included | VERIFIED after every run: no process of ours, no zombie child of the supervisor; the instance lock is free (the next run always started) |
| gamescope's own exit | As §14: gamescope 3.17.0's headless backend crashes on shutdown after its child ended (helper exit 139); not the launcher's |

The settings window looks the same as in the README screenshots (the changes only show with the gamepad off), so they were not retaken. Not checked here: Windows 10/11, the Steam Deck, the native message window on a real desktop (only on gamescope's display), and controllers.

## 16. Pending Windows Checks (rea-launcher-sdk v0.4.2, Unreleased)

From the native Windows 10 test of 0.8.0 (rea-launcher-sdk#1). Verified here only with unit tests, `wine-test` and the PE header of the release build (`objdump -p`: Subsystem 2, Windows GUI). Pending on real Windows 10/11:

- **No console window.** A double click on `ScreamerLauncher.exe` opens only the settings window. With a game, Play shows no extra console beside DOSBox (DOSBox starts with `CREATE_NO_WINDOW`); note whether dosbox-staging still opens its own "DOSBox Status Window" (it does `AllocConsole` without `-noconsole` when it has no console).
- **Commands from PowerShell.** `--help`, `--check` and `--controllers` print to the terminal; `--check > report.txt` writes the report; `--bogus-flag` and `--look invalid` print the error and usage and exit 2 with no dialog. A shortcut to `ScreamerLauncher.exe --check` shows the report in a message box.
- **Window close.** Closing the settings window without Play exits 0 (`$LASTEXITCODE`); `--settings` closed exits 3.
- **`--check` with the window open** prints "The Screamer Trilogy launcher window is open…" and exits 5; during a race it says the game is running, exit 5.
