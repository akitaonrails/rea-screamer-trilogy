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
  - `libscreamer_hooks.so` locates the active Data Object inside `*MemBase`, yields individual controls to the keyboard whenever `DataObj[2 + scancode] != 0`, and otherwise sets `record[r].enabled = 2`, `record[2].mode = 0`, and writes live proportional gamepad values directly into `normalized_controls[0,1,2,4,5]`.
  - Writing `-1` into `normalized_controls[3]` (which is unused in `record[2].mode == 0` analog steering mode but overwritten with `0` by `PollControls` every active race frame) provides an exact `PollControls` heartbeat that distinguishes active racing (`in_race == true`) from menus, intros, and the pause screen (`in_race == false`), allowing D-Pad, Left Stick, `A`/`B`, and `Start`/`Select` to navigate menus via `_Z15KEYBOARD_AddKey8KBD_KEYSb` automatically.

---

## 7. Screamer 1 `UpdateFrameTiming` 50-Entry Physics Lookup Table (`0x80e8[1..50]`)

- In **Screamer (1995)** (`STARTH.EXE` at `0x01FC38..0x01FD62` and `STARTL.EXE` at `0x01FC28..0x01FD52`), `UpdateFrameTiming`:
  1. Spin-waits at `0x01FC46` (`cmp $0x2, %edx; jb 0x01FC40`) until at least 2 ticks (`20 ms`) of the 100 Hz timer (`0x7f9`) have elapsed.
  2. Computes `raw_dt = ticks * 0.01` and stores it unclamped into `0x18e08` (and adds it to `total_race_time` at `0x18e10`).
  3. Computes `smoothed_dt` (`0x18e04`), clamps it to `[0.02, 0.20]` (`0xc70` / `0x01FCD0`), and then immediately indexes a precomputed **50-entry physics lookup table** (`400` / `0x190` bytes per entry, located at `DataObj + 0x80e8..0xcf08`) using `edx = int(1.0 / smoothed_dt)`:
     ```text
     1fcd4: fld1
     1fcd6: fdivs  0x18e04         ; st(0) = 1.0 / smoothed_dt (<= 50.0)
     ...
     1fcef: fistpl -0x4(%ebp)      ; edx = int(1.0 / smoothed_dt)
     ...
     1fd0f: mov    $0x80e8, %edx
     1fd19: add    %eax, %edx      ; edx = 0x80e8 + int(1.0 / smoothed_dt) * 400
     1fd23: mov    %edx, 0xcf08
     ```
- Lowering `0.02` (`DBL_0_02` / `FLT_0_02`) to `0.004` in **Screamer 1** causes `int(1.0 / smoothed_dt)` to exceed `50` (up to `250`), advancing `0xcf08` past the end of `0x80e8..0xcf08` and causing an out-of-bounds read when a race level loads.
- In **Screamer 2 (1996)** (`S2_3DFX.EXE` at `0x050DEC`) and **Screamer Rally (1997)**, Milestone explicitly added a `>= 50.0` bounds check before the `* 0x190` multiplication (`fcomps 0x1ca4; jb ...; movl $0x14948, 0x14ad8`), clamping the table pointer to entry `50` whenever `1.0 / smoothed_dt >= 50.0`.
- Therefore, for **Screamer 1** (`STARTH.EXE` and `STARTL.EXE`), `E1` patches only the spin-wait threshold (`0x01FC48` / `0x01FC38`: `0x02 -> 0x01`), unlocking 60+ FPS rendering while keeping the `0.02s` (`entry 50`) table index clamp intact.
