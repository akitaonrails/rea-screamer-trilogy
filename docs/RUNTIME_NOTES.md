# Runtime Verification Notes

This document records notable observations from running the original 1995–1997 **Screamer** trilogy binaries (`Screamer` [1995], `Screamer 2` [1996], and `Screamer Rally` [1997]) through `ScreamerLauncher` + `dosbox-staging v0.82.2` + `libscreamer_hooks.so` in headless X11/OpenGL sessions.

---

## 1. `dosbox-staging v0.82.2` Symbol Export & `LD_PRELOAD` Hook Integration

- **Static SDL2 with `--export-dynamic` (`-rdynamic`)**:
  - `dosbox-staging` links `libSDL2` statically into the `dosbox` ELF executable and exports both SDL2 symbols (`SDL_WasInit`, `SDL_GameControllerOpen`, `SDL_GameControllerGetAxis`, `SDL_GameControllerGetButton`, `SDL_GameControllerRumble`) and DOSBox internal symbols (`MemBase`, `_Z15JOYSTICK_Enablehb`, `_Z15JOYSTICK_Move_Xhs`, `_Z15JOYSTICK_Move_Yhs`, `_Z15JOYSTICK_Buttonhib`, `_Z15KEYBOARD_AddKey8KBD_KEYSb`) in its dynamic symbol table (`.dynsym`).
  - Because `SDL_GL_SwapWindow` is called intra-module inside `dosbox`, it does not go through the PLT; however, `clock_gettime@GLIBC_2.17` is an external `R_X86_64_JUMP_SLOT` relocation called continuously by the DOSBox main loop and pic queue.
  - Intercepting `clock_gettime` in `libscreamer_hooks.so` (throttled to `250 Hz` / `4 ms` intervals) and guarding with `SDL_WasInit(0x220) == 0x220` before calling `SDL_GameControllerOpen(0)` allows `libscreamer_hooks.so` to poll the gamepad and inject proportional 16-bit steering (`X`) and combined independent `L2`/`R2` triggers (`Y`) directly into `_Z15JOYSTICK_Move_Xhs` / `_Z15JOYSTICK_Move_Yhs` without any `.map` files or `Ctrl+F1` setup.

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
- GOG's **Screamer 2** ships with a `31,425`-byte Windows port-`0x600` stub `glide2x.ovl` (`SHA-256: b4df3c2d...`) intended for Windows 9x DOS boxes, whereas **Screamer Rally** ships with the genuine `348,791`-byte DOS 3dfx Voodoo driver `GLIDE2X.OVL` (`SHA-256: 65d2a7e5...`).
- `ScreamerLauncher` automatically skips the `31,425`-byte Windows stub when staging the runtime folder and links a genuine DOS `GLIDE2X.OVL` if present (via `SCREAMER_GLIDE2X_OVL` or a sibling `Screamer Rally` installation), allowing **Screamer 2** to run in full hardware-accelerated 3dfx Voodoo Glide mode (`S2_3DFX.EXE`) as well as 16-bit SVGA mode (`START65H.EXE`).

---

## 5. Headless Verification Summary Across All 3 Games

| Game | Selected Executable | Verified Patches Applied | Graphics Mode Entered | CD Audio & SB16 Status |
| :--- | :--- | :--- | :--- | :--- |
| **Screamer (1995)** | `STARTH.EXE` (`SVGA 640x480`) | `F2`, `F3`, `E1`, `E2`, `E5` (6 writes) | `VESA 640x480 256-colour mode 101h` | `SCREAM.DAT` mounted, `48000 Hz` CD-DA + `SB16 IRQ 7` |
| **Screamer 2 (1996)** (3dfx) | `S2_3DFX.EXE` (`3dfx Voodoo Glide`) | `E1`, `E2`, `E5` (8 writes) | `3dfx Voodoo SVGA 640x480 16-bit mode 00h` | `GAME.DAT` mounted, `48000 Hz` CD-DA + `SB16 IRQ 7` |
| **Screamer 2 (1996)** (SVGA16) | `START65H.EXE` (`SVGA 640x480 16-bit`) | `F5`, `E1`, `E2`, `E5` (9 writes) | `VESA 640x480 16-bit mode 111h` | `GAME.DAT` mounted, `48000 Hz` CD-DA + `SB16 IRQ 7` |
| **Screamer Rally (1997)** | `STRT3FX.EXE` (`3dfx Voodoo Glide`) | `E1`, `E2`, `E5` (7 writes) | `3dfx Voodoo SVGA 640x480 16-bit mode 00h` | `srally.iso` mounted, `48000 Hz` CD-DA + `SB16 IRQ 7` |

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

## 8. Screamer Rally (`STRT3FX.EXE`) & Screamer 2 (`S2_3DFX.EXE`) Texture Cache

- **Original analysis**:
  - In `STRT3FX.EXE` (`0x01C26C..0x01C29C`) and `S2_3DFX.EXE` (`0x01DBBC..0x01DBEC`), the 3dfx Glide texture cache manager stores cached texture descriptors in a fixed **1,024-entry array** (`MAXCACHE = 0x400`, 32 bytes per slot).
  - As textures are uploaded to TMU0, the game advances `next_tmu_addr` until it reaches `grTexMaxAddress(GR_TMU0)` (`tmu_max_addr`), at which point it resets `cache_count = 0` and wraps around.
  - On a real 1996 3dfx Voodoo 1 (`4 MB` total = `2 MB FBI + 2 MB TMU0`), `2 MB` of TMU RAM fits roughly `32..256` textures before wrapping around `tmu_max_addr`, so `cache_count` never reaches `1,024`.
  - When `voodoo_memsize = 12` (`4 MB FBI + 8 MB TMU0`) was configured, `tmu_max_addr` was `8 MB`, allowing more than `1,024` textures to be uploaded before `next_tmu_addr` wrapped around. On texture #1,025 (`cmp edx, 0x400; jle ...`), the game executed `printf("MAXCACHE\n"); exit(-1);`!
  - A separate software descriptor pool contains **3,064 entries of 56 bytes** (`0xBF8 * 0x38 = 0x29E40`) and has its own fatal exhaustion guard.
- **Correct configuration and regression finding (2026-11-01)**:
  1. Keep `voodoo_memsize = 4` (`2 MB FBI + 2 MB TMU0`) so `grTexMaxAddress(GR_TMU0)` reports the authentic Voodoo 1 limit. At 2 MB, normal TMU wrapping occurs long before the original 1,024-entry residency table fills.
  2. Do **not** patch either texture manager. A headless A/B run using the same DOSBox build, ISO, saved configuration, 2× renderer, and 2 MB TMU showed the original `STRT3FX.EXE` reaching the title screen, while the patched F7 executable immediately returned to DOS with `grTexDownloadMipMap: mipmap cannot span 2 Mbyte boundary`.
  3. F7's injected branch changed the game's TMU address-wrap control flow and caused the boundary failure it was intended to prevent. F8 addressed a speculative descriptor-exhaustion path that the captured failure never reached. Both patches were removed; the original executable logic plus authentic memory configuration is the fix.

---

## 9. Internal 2× (`1280×960`) 3dfx Voodoo Superscaling & `SDL_GameControllerUpdate` Polling

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

## 10. Why Native DLSS / FSR 2–3 Cannot Be Used on DOS/Glide, and How FSR 1.0 + Optical-Flow Frame Generation (`50 -> 100 FPS`) Work

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
