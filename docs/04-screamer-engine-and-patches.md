# Screamer Engine Deep Dive: 3D Projection, 60 FPS Timing, Glide & Input Internals

This document records the second-stage binary reverse-engineering findings across **Screamer** (`STARTH.EXE`, `STARTL.EXE`), **Screamer 2** (`S2_3DFX.EXE`, `START65H.EXE`, `STARTH.EXE`), and **Screamer Rally** (`STRT3FX.EXE`, `START65H.EXE`, `STARTH.EXE`), including exact Virtual Addresses (`VA`), raw file offsets (`FileOff`), and instruction bytes verified directly against the binaries on disk.

---

## 1. LE Fixup & Address Mapping Note

In Watcom Linear Executables (`LE`):
- Relative jumps/calls (`eb`, `7x`, `0f 8x`, `e8`, `e9`) within the same code object and immediate operands (`cmp reg, imm`, `mov reg, imm`, float/double literals in data sections) have **no fixup records** and are identical on disk and in memory.
- 32-bit absolute memory operands (`mov reg, [addr]`) store the **0-based offset within the target Object** (`VA - obj_base`) on disk, and the LE loader adds `obj_base` (`+0x10000` for `Obj 1`, `+0x20000` for `Obj 2` in 3dfx binaries, `+0x100000`/`+0x120000`/`+0x130000` for the Data Object) using the LE Fixup Record Table when loading pages into memory.
- If patching an instruction on disk in the `.EXE` inside our private runtime folder, any instruction that replaces a relocated memory operand with a non-relocated instruction must either preserve the fixup target offset or neutralize its entry in the LE Fixup Record Table. Conversely, patching immediate operands, register instructions, relative jumps, data constants, or code without fixup records requires zero fixup table changes!

### Exact SHA-256 Hashes & Sizes of Verified Binaries

| Game & Binary | Size (Bytes) | SHA-256 Digest |
| :--- | :--- | :--- |
| **Screamer 1 `STARTH.EXE`** | `1,034,923` | `1246afc95463f96598d5fee764fc99c847b6d3e0bf8ca2268bd362c664bb28a3` |
| **Screamer 1 `STARTL.EXE`** | `1,034,919` | `ad20fa3919ecf86da5931e2e0e495d4043f6be8c44e57299114528d87ee27ec7` |
| **Screamer 1 `SETUP.EXE`** | `251,079` | `33d88269a37bc67b08b2ec9f9f7be32c429619f6d946837c35b7e08f33f2d189` |
| **Screamer 2 `STARTH.EXE`** | `1,055,490` | `91a49f26c7b5a26bc84552126ed337dfa733b81c61ec4ad4195d205809a2f400` |
| **Screamer 2 `STARTL.EXE`** | `1,055,478` | `78a78edbc92196647d6bb3980bf842d42e68a25405fb78098072958d60e66f0a` |
| **Screamer 2 `START65H.EXE`** | `1,201,082` | `05f576866f26602be303343c5e9104132616f464b71ed6ae958e99a668e5c7c8` |
| **Screamer 2 `START65L.EXE`** | `1,205,090` | `f2d6c1389f2e09e507b822fc6d530cad898671fcf55e8bb02bc00de861515d46` |
| **Screamer 2 `S2_3DFX.EXE`** | `1,055,262` | `e6c24c84650938f2baee5f4b571baa45bb09a93ba12dd24e3e1d2cefd9b6f3ec` |
| **Screamer 2 `SETUP.EXE`** | `274,089` | `ef8d7665f1a8979ee9f699ff0bd476b8e9679fbe423a6b53a773539ce51d178b` |
| **Screamer Rally `STARTH.EXE` (`/START0/`)** | `1,045,098` | `18d32c27d61873dc67381f87880d19774afaec1f7ee391d14ab255a9040a12d7` |
| **Screamer Rally `START65H.EXE` (`/START0/`)** | `1,145,058` | `0785e38567c475f25663e4022ec125e447a799004e34efa59758b50cfd84901d` |
| **Screamer Rally `STRT3FX.EXE` (`/START0/`)** | `1,044,502` | `9eac6e12ab309389bdda019cfa695bfbbc791fbda1a4634464a444fce35165d1` |
| **Screamer Rally `ST.EXE`** | `297,736` | `e6083738a0130e77d8403f2948d70f4c554481405be8197ab70123ca0d57393d` |

---

## 2. Screamer 1 `STARTH.EXE` Startup & SVGA Bugs

1. **No Speed-Benchmark Divide-by-Zero in `STARTH.EXE` / `STARTL.EXE`**:
   - Disassembly of every `div`/`idiv` instruction in `STARTH.EXE` and `STARTL.EXE` confirms that the VRAM/CPU speed benchmark (`idiv ecx`) exists **only** in `SETUP.EXE` / `ST.EXE`.
2. **DPMI Free Memory Probe (`0x02F8CA` -> `0x02F758`, `FileOff 0x039F58`) — Stack Overflow & Signed 32-Bit Overflow**:
   - At startup (`VA 0x010045`, `FileOff 0x01A845`), `main_` calls `0x02F8CA` to determine available DPMI RAM, stores it in `[0x1062AC]`, checks `cmp eax, 0x0061A800; jge 0x010084` (signed comparison against 6.4 MB!), and pre-allocates a single master heap of size `[0x1062AC]`.
   - Because DOSBox/DOS4GW returns `-1` for `PagingFileSizeInPages` in `INT 31h, AX=0500h` (`0x02F952`), `0x02F78B..0x02F80F` enters a loop allocating chunks via `malloc` into a **fixed 100-element stack array `[ebp - 0x1A4 + count*4]`** with no bounds check on `count`!
   - If DPMI memory is fragmented into `>100` chunks, it overflows the stack; if `>= 2 GB` is reported, the signed `jge` fails with `"Insufficient memory available"`.
   - **Fix**: Clamping the maximum probed block in `0x02F5AE` (`VA 0x02F602`, `FileOff 0x039E02`) or returning a fixed safe heap size (e.g. `16 MB` = `0x01000000`) from `0x02F8CA` prevents both overflows.
3. **VESA `WinGranularity & 7` Bank Shift Bug (`VA 0x08BE5B`, `FileOff 0x09665B`)**:
   - `66 8b 46 04 83 e0 07 a3 8c c1 07 00` masks `WinGranularity` with `7`.
4. **SVGA Asset Path (`<gfx_path>\GFX\MAP%dH.DAT`)**:
   - `STARTH.EXE` loads `*H.DAT` / `*H.PCX` from `<gfx_path>\GFX\`. In GOG's Screamer 1 release, all `GFX\MAP*H.DAT` and `GFX\MAP*L.DAT` files are extracted directly into `<game_dir>\GFX\` on the hard drive! However, GOG's default `SCREAMER.CFG` says `C:\ D: D: c` (pointing `gfx_path` to `D:`), so if `D:` is not mounted properly or if a user runs without the CD image mounted, `STARTH.EXE` fails to open `D:\GFX\MAP1H.DAT`. Setting `SCREAMER.CFG` to `C:\ D: C:\ c` loads all `GFX\*H.DAT` directly from the local folder!

---

## 3. 3D Rendering Pipeline, Resolution & Hor+ Widescreen Projection

### 3.1 Screamer 1 (`STARTH.EXE`)
- **Offscreen Framebuffer**:
  - Allocates `308,480` bytes (`0x4B500`) at `[0x121528]` (`VA 0x04C8F0`, `FileOff 0x0570F0`).
  - Software rasterizer draws `640x400` 8-bit paletted pixels into `[0x121528]`.
  - At the end of each frame (`VA 0x02AA96`, `FileOff 0x035296`), `call 0x08BEB8` blits the `640x400` buffer to VESA mode `0x101` (`640x480x256`) at `0xA0000` by duplicating every 5th scanline (`400 -> 480`).
  - Notably, right after `0x08BEB8` at **`VA 0x08C08C..0x08C0FB`** (`FileOff 0x09688C`) is a complete unreferenced 1:1 `640x480` VESA bank copier!
- **3D Perspective Projection (`VA 0x0835C0..0x0836A0`, `FileOff 0x08DDC0..0x08DEA0`)**:
  - Camera rotation matrix (`3x3` fixed-point `int32`) lives at **`0x082BB4..0x082BD4`**, computed once per camera setup in **`VA 0x083520..0x0835B0`** (`FileOff 0x08DD20..0x08DDB0`).
  - Projects `(x_cam, y_cam, z_cam)` using `PERSP01.TAB`:
    ```x86asm
    0x083660: 2e 8b 04 85 dc ba 03 00  mov  eax, cs:[ebx*4 + PERSP01]
    0x083668: 0f af d0                 imul edx, eax        ; x_cam * persp
    0x08366b: 0f af c8                 imul ecx, eax        ; y_cam * persp
    0x08366e: c1 f9 10                 sar  ecx, 16
    0x083671: 03 d2                    add  edx, edx        ; 2x SVGA X scale
    0x083673: 81 c2 00 00 3f 01        add  edx, 0x013F0000 ; + center_x (319 << 16)
    0x083679: 03 c9                    add  ecx, ecx        ; 2x SVGA Y scale
    0x08367b: 03 0d dc a7 07 00        add  ecx, [0x08A7DC] ; + center_y (260)
    0x083681: 66 8b d1                 mov  dx, cx
    ```
  - **Hor+ 16:9 Widescreen Fix for Screamer 1**:
    - Scaling the X-row of the 3x3 camera matrix (`[0x082BB4]`, `[0x082BB8]`, `[0x082BBC]`) by `3/4` (`0.75`, via `(val * 3) >> 2`) scales `x_cam` by `0.75` prior to vertex projection (`0x083660`), near-plane clipping (`0x0897AC`), and `VIS.TAB` frustum culling (`0x083930`). When displayed at `16:9` aspect ratio, the central 4:3 region has the exact original proportions while expanding horizontal FOV from 90° to ~106.26° with zero frustum pop-in at screen edges!

### 3.2 Screamer 2 (`S2_3DFX.EXE`, `START65H.EXE`) & Screamer Rally (`STRT3FX.EXE`, `START65H.EXE`)
- **3dfx Glide 2.x Pipeline (`S2_3DFX.EXE` & `STRT3FX.EXE`)**:
  - Dynamically loads `GLIDE2X.OVL` and populates 130 `jmp rel32` thunks (`0x0F064C..0x0F08D5` in `S2_3DFX.EXE`; `0x0EE92C..0x0EEBB5` in `STRT3FX.EXE`).
  - Calls `_grSstWinOpen@28` (`0x056AA3` in `S2_3DFX`, `0x055077` in `STRT3FX`) with resolution ID `7` (`GR_RESOLUTION_640x480`, stored at `[0x133778]` / `[0x123880]`).
  - Converts projected integer coordinates `(x_int, y_int)` in `640x400` space into floating-point `GrVertex.x` / `GrVertex.y` via `X_table[0..640]` (`0x161E50` / `0x151B10`) and `Y_table[0..400]` (`0x161800` / `0x1514C0`, multiplied by `1.2f`), then draws triangles via `_grDrawTriangle@12`.
  - Draws 2D menus and HUD layers using `_grLfbLock@24` / `_grLfbUnlock@8` and textured 2D quads.
- **Hor+ 16:9 Widescreen Fix for Screamer 2 & Screamer Rally**:
  - Camera-space transformation uses a floating-point 3x3 rotation matrix + translation vector (`0x07C3F4..0x07C420` in `S2_3DFX.EXE`, `0x07A9C4..0x07A9F0` in `STRT3FX.EXE`), populated by `0x07C350..0x07C3F0` (`S2_3DFX`, `FileOff 0x081F50`) / `0x07A920..0x07A9C0` (`STRT3FX`, `FileOff 0x07FB20`).
  - Scaling the X-row of the float camera matrix (`[0x07C3F4]`, `[0x07C3F8]`, `[0x07C3FC]`, `[0x07C41C]`) by `0.75f` (`3/4`) widens the 3D horizontal FOV for 16:9 while keeping near-plane clipping and `VIS.TAB` frustum culling perfectly aligned.

---

## 4. Frame Rate Cap, Delta-Time (`dt`) & True 60 FPS Unlock

### 4.1 How the Engine Measures Frame Time
1. The HMI SOS timer callback runs at **100 Hz (`10 ms` per tick)** and increments `frame_ticks` (`[0x0107F9]` in S1, `[0x0519EA]` in `S2_3DFX`, `[0x04D24A]` in `S2_START65H`, `[0x050512]` in `SR_STRT3FX`, `[0x040302]` in `SR_START65H`).
2. Every physics step, car movement, steering filter, and animation multiplies by `smoothed_dt` (`[0x118E04]` in S1, `[0x153D88]` in `S2_3DFX`, `[0x1534D8]` in `S2_START65H`, `[0x1469C4]` in `SR_STRT3FX`, `[0x146494]` in `SR_START65H`), computed as an exponential moving average of `raw_dt = frame_ticks * 0.01f`.

### 4.2 Why Framerate Is Capped at 50 FPS (or Drops to 30 FPS on 60 Hz VSync) & Why Unpatched 60 FPS Runs 20% Too Fast
There are **two hardcoded 50 FPS (`20 ms`) limits** in the binaries:
1. **Spin-Wait for `frame_ticks >= 2` (`20 ms` = 50 FPS max)** in Screamer 1 and Screamer 2:
   - **S1 `STARTH.EXE`** (`VA 0x01FC46`, `FileOff 0x02A446`): `83 fa 02 72 f5` (`cmp edx, 2; jb 0x01fc40`).
   - **S1 `STARTL.EXE`** (`VA 0x01FC36`, `FileOff 0x02A436`): `83 fa 02 72 f5` (`cmp edx, 2; jb 0x01fc30`).
   - **S2 `S2_3DFX.EXE`** (`VA 0x0519B9`, `FileOff 0x0575B9`): `83 f8 02 7c f6` (`cmp eax, 2; jl 0x0519b4`).
   - **S2 `START65H.EXE`** (`VA 0x04D219`, `FileOff 0x061A19`): `83 f8 02 7c f6` (`cmp eax, 2; jl 0x04d214`).
   - *(In Screamer Rally, Milestone already changed this byte from `02` to `01` at `0x0504E3` / `0x0402D3`!)*
2. **Hardcoded `0.02f` (`20 ms`) Minimum `smoothed_dt` Clamp in ALL 6 Binaries**:
   - After updating `smoothed_dt`, all 6 binaries compare `smoothed_dt` against `double 0.02` and, if smaller, overwrite `smoothed_dt` with `float 0.02f` (`0x3CA3D70A`)!
   - Consequently, if the game renders at 60 FPS (`16.67 ms`/frame) without patching this clamp, `smoothed_dt` is forced to `0.020s` every frame, causing **all car physics and animations to run `60 * 0.02 = 1.20x` (20% too fast)**!
   - Furthermore, in Screamer 2 and Screamer Rally's in-race timer function (`0x050C5A` in `S2_3DFX`, `0x04F780` in `SR_STRT3FX`), `raw_dt` is multiplied by `1.1` before averaging (`(9.0 * smoothed_dt + 1.1 * raw_dt) / 10.0`).

### 4.3 Exact Patch Table for True 60 FPS (and 100+ FPS) with 1.0x Physics Speed

| Binary | Spin-Wait `cmp reg, 2` -> `1` (`FileOff`) | `double 0.02` Constant (`FileOff`) | `float 0.02f` (`0A D7 A3 3C`) Immediate (`FileOff`) |
| :--- | :--- | :--- | :--- |
| **S1 `STARTH.EXE`** | `0x02A448`: `02` -> `01` | `0x0F7470` (`7B 14 AE 47 E1 7A 94 3F`) | `0x02A4D0` (`0A D7 A3 3C`) |
| **S1 `STARTL.EXE`** | `0x02A438`: `02` -> `01` | `0x0F7470` (`7B 14 AE 47 E1 7A 94 3F`) | `0x02A4C0` (`0A D7 A3 3C`) |
| **S2 `S2_3DFX.EXE`** | `0x0575BC`: `02` -> `01` | Menu: `0x0FA850`<br>Race: `0x0FA88C` | Menu: `0x056848`<br>Race: `0x056930` |
| **S2 `START65H.EXE`** | `0x061A1C`: `02` -> `01` | Menu: `0x11E75C`<br>Race: `0x11E798` | Menu: `0x060CA8`<br>Race: `0x060D90` |
| **S2 `STARTH.EXE`** | `0x06008C`: `02` -> `01` | Menu: `0x0FAB8C`<br>Race: `0x0FABC8` | Menu: `0x05E13C`<br>Race: `0x05E224` |
| **SR `STRT3FX.EXE`** | *(Already `01` at `0x0556E3`)* | Menu: `0x0F7E58`<br>Race: `0x0F7E94` | Menu: `0x05496E`<br>Race: `0x054A56` |
| **SR `START65H.EXE`** | *(Already `01` at `0x0532D3`)* | Menu: `0x110C58`<br>Race: `0x110C94` | Menu: `0x05255E`<br>Race: `0x052646` |
| **SR `STARTH.EXE`** | *(Already `01`)* | Menu: `0x0F8288`<br>Race: `0x0F82C4` | Menu: `0x0512CE`<br>Race: `0x0513B6` |

- **Bonus Precision Timer Upgrade for 60 FPS**:
  - By increasing the HMI SOS timer registration argument from `100 Hz` (`0x64`) to `120 Hz` (`0x78`) (or `240 Hz` = `0xF0`) and scaling the `0.01` tick-to-seconds constant (`[0x100C58]` in S1, `[0x131C38]`/`[0x131C68]` in S2, `[0x121C40]`/`[0x121C70]` in SR) proportionally (`1/120` or `1/240`), a `2`-tick (at 120 Hz) or `4`-tick (at 240 Hz) wait paces software-rendered modes (`STARTH.EXE`, `START65H.EXE`) to an **exact 60.0 FPS (`16.667 ms`)** with zero jitter!

---

## 5. In-Race Input, Analog Calibration & Deadzone Removal

### 5.1 Master Input Normalization Function & Hardcoded 10% Deadzone
All three games poll input once per frame and normalize the 6 race actions (`0: Accelerate`, `1: Brake`, `2: Steer Left / Full Analog Steer`, `3: Steer Right`, `4: Gear Up`, `5: Gear Down`) into signed 10-bit integers in `[-1023, +1023]` (`-0x3FF..+0x3FF`):
- **Screamer 1 `STARTH.EXE`**: `VA 0x024DA0..0x025015` (`FileOff 0x02F5A0`), outputs at `VA 0x119880..0x119897`.
  - Hardcoded **10% (`0x66 = 102 / 1024`) center deadzone** at **`VA 0x024F3C`** (`FileOff 0x02F73C`): `83 f8 66 7d 05 31 f6` (`cmp eax, 0x66; jge +5; xor esi, esi`).
- **Screamer 2 `S2_3DFX.EXE`**: `VA 0x03A0A0..0x03A271` (`FileOff 0x03FCA0`), outputs at `VA 0x148950..0x148967`.
  - Hardcoded **10% (`0x66`) center deadzone** at **`VA 0x03A202`** (`FileOff 0x03FE02`): `83 f8 66 7d 0e 31 db`.
- **Screamer Rally `STRT3FX.EXE`**: `VA 0x03A2E0..0x03A4BF` (`FileOff 0x03F4E0`), outputs at `VA 0x13A560..0x13A577`.
  - Hardcoded **10% (`0x66`) center deadzone** at **`VA 0x03A442`** (`FileOff 0x03F642`): `83 f8 66 7d 0e 31 db`.

### 5.2 Complete `CONTROLS.CFG` Structure (`1,328`B in S1; `1,496`B in S2/SR)
1. **Control Binding Array** (`6 * 28B = 168B` for P1 in S1; `12 * 28B = 336B` for P1+P2 in S2/SR):
   - `+0x00`: `enabled` (`1`)
   - `+0x04`: `device_type` (`0` = Keyboard, `1` = Joystick Port 0, `2` = Joystick Port 1)
   - `+0x08`: `mode` (`0` = Full analog axis [Steer Left+Right combined], `1` = Digital/Split, `2` = Half-axis)
   - `+0x0C`: `is_button` (`0` = Analog Axis, `1` = Digital Button)
   - `+0x10`: `index_or_key` (Scancode `0..127` or Joystick axis/button index)
   - `+0x14`: `direction` (`+1` or `-1`)
   - `+0x18`: `partner_idx`
2. **Joystick Axis Calibration Table** (`40 * 28B = 1,120B` at `0x0A8` in S1, `0x150` in S2/SR):
   - `+0x00`: `min_val` (`int32`)
   - `+0x04`: `center_val` (`int32`)
   - `+0x08`: `max_val` (`int32`)
   - `+0x0C`: `divisor` (`int32`)
   - `+0x10`: `range_span` (`int32`)
   - `+0x14`: `scale_pos` (`float`, `1024.0f / (max_val - center_val)`)
   - `+0x18`: `scale_neg` (`float`, `1024.0f / (center_val - min_val)`)
3. **Trailer Config Words** (`10 * 4B = 40B` at `0x508` in S1, `0x5B0` in S2/SR).
