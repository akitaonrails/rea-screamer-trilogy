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
  - **CRITICAL — `clock_gettime` Thread Safety & Page-Aligned Scanning**:
    - In `dosbox-staging`, `clock_gettime` is invoked from background threads (mixer, OPL, Voodoo threads) as well as re-entrantly from the main thread during `SDL_PollEvent`.
    - Never call `SDL_GameControllerUpdate()` or `SDL_JoystickUpdate()` inside `clock_gettime` (`dosbox-staging`'s main thread already pumps `SDL_PollEvent`, and calling `SDL_JoystickUpdate()` from `clock_gettime` deadlocks SDL2's non-reentrant joystick mutex on window creation).
    - Always guard `poll_gamepad_and_inject` with an atomic non-blocking `IN_POLL` try-lock, step by `4096`-byte pages in `locate_binary_data_object`, and throttle uncached RAM scans to at most once every `100 ms` (`LAST_SCAN_ATTEMPT_NS`) so `dosbox-staging` is never CPU-starved while DOS/4GW is booting.

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
