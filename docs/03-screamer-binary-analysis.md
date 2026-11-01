# Screamer Trilogy Reverse-Engineering & Binary Analysis

This document records the reverse-engineering findings, Linear Executable (`LE`) structures, hardware I/O routines, configuration file binary layouts, and exact byte offsets of bugs across **Screamer** (1995), **Screamer 2** (1996), and **Screamer Rally** (1997).

---

## 1. Executables & File Inventory

### 1.1 Screamer 1 (`screamer`)
- **Compiler & Extender**: Watcom C/C++32 v10.0 (`1988-1994`), `wstub` loader stub (`0x2A50` = `10,832` bytes), Rational Systems `DOS4GW.EXE` v1.95 (`254,532` bytes).
- **Executables**:
  - `STARTH.EXE` (`1,034,923` bytes): SVGA High-Resolution (`640x480`, 256 colors, VESA mode `0x101` with `640x400 -> 640x480` vertical scaling).
  - `STARTL.EXE` (`1,034,919` bytes): VGA Low-Resolution (`320x200`, 256 colors, mode `0x13`).
  - `SETUP.EXE` (`251,079` bytes): Hardware setup, sound/music card configuration, and CPU/VRAM speed benchmark utility.
- **Audio Drivers**: HMI Sound Operating System (SOS) v4.0 (`HMIDRV.386` `317,317`B, `HMIMDRV.386` `117,852`B, `HMIDET.386` `83,774`B).
- **CD Image**: `SCREAM.DAT` (CDRWin CUE sheet) + `SCREAM.gog` (`382,548,096`B or `545,606,832`B raw 2352-byte/sector mixed-mode image: Track 01 `MODE1/2352` data + Tracks 02–11 RedBook `AUDIO`).

### 1.2 Screamer 2 (`screamer2`)
- **Compiler & Extender**: Watcom C/C++32 v10.5/10.6 (`1988-1995`), `wstub` loader stub (`0x2C90` = `11,408` bytes), Tenberry Software `DOS4GW.EXE` v1.97 (`265,396` bytes).
- **Executables**:
  - `STARTH.EXE` (`1,055,490` bytes): SVGA 256-color (`640x400` mode `0x100` on VBE 2.0+ or `640x480` mode `0x101` on VBE 1.2).
  - `STARTL.EXE` (`1,055,478` bytes): VGA 256-color (`320x200` mode `0x13`).
  - `START65H.EXE` (`1,201,082` bytes): SVGA 65,536-color 16-bit (`640x480` mode `0x111`; has VBE 2.0 mode `0x120` bug — see Section 4.1).
  - `START65L.EXE` (`1,205,090` bytes): Low-res 65,536-color 16-bit (`320x200` VESA mode `0x10E`).
  - `S2_3DFX.EXE` (`1,055,262` bytes): 3dfx Voodoo Glide 2.x hardware-accelerated executable (dynamically loads `glide2x.ovl`).
  - `SETUP.EXE` (`274,089` bytes): Sound, video timing, and hardware benchmark setup utility.
- **CD Image**: `GAME.DAT` (CUE sheet) + `GAME.GOG` (raw 2352-byte/sector mixed-mode image: Track 01 `MODE1/2352` + Tracks 02–09 RedBook `AUDIO`).

### 1.3 Screamer Rally (`screamer-rally`)
- **Compiler & Extender**: Watcom C/C++32 v10.5/10.6 (`1988-1995`), `wstub` loader stub (`0x2C90` = `11,408` bytes), `DOS4GW.EXE` v1.97 (`265,396` bytes). Internal LE module name is `s2` (built directly on the Screamer 2 engine).
- **Executables**:
  - `STARTH.EXE` (`1,045,098`B in `/START0/` small install; `1,037,782`B in `/START1/` full No-CD install): 2D menu in `640x480x16bpp` (`0x111`), 3D race in `640x400x8bpp` (`0x100`) or `640x480x8bpp` (`0x101`).
  - `STARTL.EXE` (`1,049,186`B / `1,042,382`B): 2D menu in `640x480x16bpp` (`0x111`), 3D race in `320x200x8bpp` (`0x13`).
  - `START65H.EXE` (`1,145,058`B / `1,138,254`B): High-Res 65K-color (`640x480x16bpp` mode `0x111`).
  - `START65L.EXE` (`1,149,630`B / `1,142,826`B): Low-Res 65K-color (`320x200x16bpp` mode `0x10E`).
  - `STRT3FX.EXE` (`1,044,502`B / `1,037,186`B): 3dfx Voodoo Glide 2.x hardware-accelerated executable (`GLIDE2X.OVL`).
  - `STRT3DBH.EXE` / `STRT3DBL.EXE`: Creative Labs 3D Blaster PCI (Rendition Vérité V1000 CGL).
  - `STRTMSI.EXE`: Matrox Mystique (`msi*.dll` / Matrox Simple Interface).
  - `ST.EXE` (`297,736` bytes): Sound/video setup & hardware benchmark utility.
- **Discovery on `srally.iso`**:
  - `/START0/` on the CD contains the small-install executables (which perform the MSCDEX PVD check at `0x028CD8`).
  - `/START1/` on the CD contains the full-install executables (`FILESALL.NTF`), which Milestone compiled **without any CD-ROM check**!
  - `/S2/` on the CD contains Milestone's official 3D card patch v1.00 for Screamer 2 (`S2_3DFX.EXE`, `S2_3DBH.EXE`, `S2_3DBL.EXE`, `S2_MYST.EXE`).

---

## 2. Linear Executable (`LE`) Object Layout

| Game & Binary | LE Header Offset | `DataPagesOff` | Entry (`Obj:Off`) | `Obj 1` (32-bit Code) | `Obj 2` (16-bit Code / BSS) | Data + BSS + Stack Object |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **S1 `STARTH.EXE`** | `0x2A50` | `0x1A800` | `1:0x0200F8` (VA `0x300F8`) | `Base=0x10000`, `VSize=0xDBA93` (220p) | `Obj 2`: `Base=0xF0000`, `VSize=0x82` (BSS) | `Obj 3`: `Base=0x100000`, `VSize=0x472C0` (`FileOff=0xF6800`) |
| **S1 `STARTL.EXE`** | `0x2A50` | `0x1A800` | `1:0x01FEB8` (VA `0x2FEB8`) | `Base=0x10000`, `VSize=0xDB7A3` (220p) | `Obj 2`: `Base=0xF0000`, `VSize=0x82` (BSS) | `Obj 3`: `Base=0x100000`, `VSize=0x472C0` (`FileOff=0xF6800`) |
| **S1 `SETUP.EXE`** | `0x2A50` | `0x0DA00` | `1:0x012C28` (VA `0x22C28`) | `Base=0x10000`, `VSize=0x261C1` (39p) | `Obj 2`: `Base=0x40000`, `VSize=0x82` (BSS) | `Obj 3`: `Base=0x50000`, `VSize=0x29460` (`FileOff=0x34A00`) |
| **S2 `STARTH.EXE`** | `0x2C90` | `0x23000` | `1:0x0A40C0` (VA `0xB40C0`) | `Base=0x10000`, `VSize=0xD4857` (213p) | `Obj 2`: `Base=0xF0000`, `VSize=0x9AC` (16-bit) | `Obj 4`: `Base=0x110000`, `VSize=0x29C90` (`FileOff=0xF9000`) |
| **S2 `START65H.EXE`** | `0x2C90` | `0x24800` | `1:0x0C2EF4` (VA `0xD2EF4`) | `Base=0x10000`, `VSize=0xF6E39` (247p) | `Obj 2`: `Base=0x110000`, `VSize=0x9AC` (16-bit) | `Obj 4`: `Base=0x130000`, `VSize=0x298E0` (`FileOff=0x11C800`) |
| **S2 `S2_3DFX.EXE`** | `0x2C90` | `0x24C00` | `2:0x0A0BB4` (VA `0xC0BB4`) | `Obj 2`: `Base=0x20000`, `VSize=0xD0AE0` (209p) | `Obj 3`: `Base=0x100000`, `VSize=0x9AC` (16-bit)<br>`Obj 4`: `Base=0x110000`, `VSize=0x48` (Thunk) | `Obj 6`: `Base=0x130000`, `VSize=0x74EE0` (`FileOff=0xF8C00`) |
| **S2 `SETUP.EXE`** | `0x2C90` | `0x0EA00` | `1:0x0185C8` (VA `0x285C8`) | `Base=0x10000`, `VSize=0x2ACA1` (43p) | `Obj 2`: `Base=0x40000`, `VSize=0x9AC` (16-bit) | `Obj 4`: `Base=0x60000`, `VSize=0x5D620` (`FileOff=0x39A00`) |
| **SR `STARTH.EXE`** | `0x2C90` | `0x22800` | `1:0x098D9A` (VA `0xA8D9A`) | `Base=0x10000`, `VSize=0xD2D57` (211p) | `Obj 2`: `Base=0xF0000`, `VSize=0x9AC` (16-bit) | `Obj 4`: `Base=0x110000`, `VSize=0x2DFD0` (`FileOff=0xF6800`) |
| **SR `START65H.EXE`** | `0x2C90` | `0x23E00` | `1:0x0A33CA` (VA `0xB33CA`) | `Base=0x10000`, `VSize=0xE9E39` (234p) | `Obj 2`: `Base=0x100000`, `VSize=0x9AC` (16-bit) | `Obj 4`: `Base=0x120000`, `VSize=0x2D5F0` (`FileOff=0x10EE00`) |
| **SR `STRT3FX.EXE`** | `0x2C90` | `0x24400` | `2:0x094CA2` (VA `0xB4CA2`) | `Obj 2`: `Base=0x20000`, `VSize=0xCEE30` (207p) | `Obj 3`: `Base=0xF0000`, `VSize=0x9AC` (16-bit)<br>`Obj 4`: `Base=0x100000`, `VSize=0x48` (Thunk) | `Obj 6`: `Base=0x120000`, `VSize=0x785C0` (`FileOff=0xF5400`) |
| **SR `ST.EXE`** | `0x2C90` | `0x0E200` | `1:0x01BB18` (VA `0x2BB18`) | `Base=0x10000`, `VSize=0x2E1F1` (47p) | `Obj 2`: `Base=0x40000`, `VSize=0x9AC` (16-bit) | `Obj 4`: `Base=0x60000`, `VSize=0x5D6B0` (`FileOff=0x3E200`) |

---

## 3. Low-Level Hardware I/O Architecture

All three games share a custom 32-bit assembly hardware layer (`0x0100E4..0x0102B0` in S1 `STARTH.EXE`; `0x04CF59..0x04D2B4` in S2 `STARTH.EXE`; `0x040388..0x040C34` in SR `STARTH.EXE`):

### 3.1 Keyboard Interrupt Handler (`INT 9` / IRQ 1)
- **Addresses**: S1 VA `0x0100E4` | S2 VA `0x04CED9` | SR VA `0x040388`.
- Reads raw scancode from 8042 port `0x60`, strips bit 7 to get a 7-bit scancode index (`0..127`), and writes `1` (make) or `0` (break) into a 128-byte `key_table` (`0x010113` in S1, `0x04CED9` in S2, `0x040308` in SR).
- **Quirk**: Ignores `0xE0` extended prefixes, collapsing extended arrow/numpad keys and Left/Right Ctrl/Alt (`29`, `56`, `72`, `75`, `77`, `80`) into the same 7-bit slot.

### 3.2 PIT Timer (`0x43`/`0x40`) & HMI SOS 100 Hz Timer
- Direct PIT reprogram helper at S1 `0x0101F6`, S2 `0x04CFBC`, SR `0x0403EB`.
- Registers a **100 Hz (`0x64`)** timer callback with HMI SOS (`sosTIMERRegisterEvent`).
- Callback (S1 `0x01020D`, S2 `0x04CFD3`, SR `0x040402`) increments two 32-bit monotonic tick counters (`inc dword [tick_a]; inc dword [tick_b]`).

### 3.3 Gameport Joystick Polling (Port `0x201`)
- **Button Read** (S1 `0x010257`, S2 `0x04D01D`, SR `0x04044C`):
  `in al, 0x201; shr eax, 4; xor eax, 0x0f; ret` (4 active-high button bits `0..3`).
- **Axis Read** (S1 `0x010263`, S2 `0x04D02B`, SR `0x04045A`):
  - Spins up to `0x1000` (`4096`) `in al, 0x201` reads waiting for residual charge to clear.
  - Disables interrupts (`cli`), writes `out 0x201, al`, and counts `in al, 0x201` loops in `ECX` up to **`0x2000` (`8192`) in S1** or **`0x1800` (`6144`) in S2/SR**, then `sti`.
  - Returns `((in(0x201) & 0xF0) << 12) | (ecx & 0xFFF)`.
  - Because the loop is capped at `0x1800`/`0x2000` and masked with `0xFFF` (`4095`), wall-clock-timed gameport emulation overflows `0xFFF` at high CPU cycles unless `timed=false` (per-read decrement) is used or the polling routine is hooked/patched.

### 3.4 3dfx Glide Dynamic Loader (`S2_3DFX.EXE` & `STRT3FX.EXE`)
- Uses 3dfx's `LINEXE_LOADER` to dynamically load `GLIDE2X.OVL` and resolve 129 uppercase stdcall Glide 2.x symbols (`_GRGLIDEINIT@0`, `_GRSSTWINOPEN@28`, `_GRBUFFERSWAP@4`, `_GRDRAWTRIANGLE@12`, `_GRLFBLOCK@24`, `_GRTEXDOWNLOADMIPMAP@16`, etc.).

---

## 4. Exact Root Causes & Byte Offsets of Game Bugs

### 4.1 `SETUP.EXE` / `ST.EXE` High-Cycle Divide-By-Zero Crash (`idiv ecx`)
Before drawing the main menu, `SETUP.EXE` (S1/S2) and `ST.EXE` (SR) run a VRAM bandwidth benchmark (`160` frames of `64,000` bytes copied to `0xA0000`) and a CPU MHz benchmark (`xchg eax, edx` loop) timed with `INT 1Ah, AH=00h` BIOS ticks in `ECX`, then divide using `idiv ecx` (`F7 F9`). At high CPU speeds, both loops finish in `< 1` tick (`ECX = 0`), causing an immediate CPU Divide-By-Zero Exception (`INT 00h`).

| Binary | Benchmark Routine VA | VRAM Benchmark `idiv ecx` (VA / File Offset) | CPU MHz Benchmark `idiv ecx` (VA / File Offset) |
| :--- | :--- | :--- | :--- |
| **Screamer 1 `SETUP.EXE`** | `0x035C80` (`0x033680`) | VA **`0x035E4B`** / File **`0x03384B`** | VA **`0x035EF2`** / File **`0x0338F2`** |
| **Screamer 2 `SETUP.EXE`** | `0x039988` (`0x038388`) | VA **`0x039B53`** / File **`0x038553`** | VA **`0x039C06`** / File **`0x038606`** |
| **Screamer Rally `ST.EXE`** | `0x03CED8` (`0x03B0D8`) | VA **`0x03D0A3`** / File **`0x03B2A3`** | VA **`0x03D156`** / File **`0x03B356`** |

### 4.2 VESA / SVGA Bugs Across `STARTH.EXE` and `START65H.EXE`
1. **Bug A — Screamer 2 `START65H.EXE` VBE 2.0 Mode `0x120` Hijack**:
   - At VA `0x0D7825..0x0D7852` (file offset **`0x0EB025..0x0EB052`**), `START65H.EXE` checks `cmp eax, 0x200` (`VbeVersion >= 2.0`). If true, it requests VESA mode `0x120` (an old UniVBE `640x400x16bpp` mode), which S3 Trio64 VBE 2.0 defines as **`1600x1200x8bpp`**!
   - **Fix**: Patching `7c 16` (`jl 0xd7847`) at file offset **`0x0EB02F`** to `eb 16` (`jmp 0xd7847`) forces standard VESA mode `0x111` (`640x480x16bpp`).
2. **Bug B — Broken `WinGranularity & 7` Bank-Switching Shift**:
   - S1 `STARTH.EXE` (`0x08BE5B` / file `0x09665B`), S2 `STARTH.EXE` (`0x07FA9B` / file `0x092A9B`), S2 `START65H.EXE` (`0x0D78A6` / file `0x0EB0A6`), SR `STARTH.EXE` (`0x0AE6ED` / file `0x0C0EED` & `0x0B1678` / file `0x0C3E78`), SR `START65H.EXE` (`0x0CB398` / file `0x0DF198`).
   - Masks `ModeInfoBlock.WinGranularity` with `and eax, 7` and uses it as a shift count (`shl al, cl`) for `INT 10h, AX=4F05h`, which fails whenever `WinGranularity` is 8, 16, or 32 KB instead of 64 KB or 4 KB.
3. **Bug C — Direct VGA CRTC (`0x3D4`/`0x3D5`) & Misc Output (`0x3C2`) Register Overwrites**:
   - S2 `START65H.EXE` (`0x0D78B7` / file `0x0EB0B7`), S2 `START65L.EXE` (`0x0D76AB` / file `0x0EAEAB`), SR `STARTH.EXE` (`0x0AE70B` / file `0x0C0F0B`), SR `START65H.EXE` (`0x0CB3A4` / file `0x0DF1A4`).
   - Overwrites VGA CRTC vertical/horizontal retrace and scanline-doubling registers after `INT 10h, AX=4F02h` unless the video preset in `SETUP.CFG` offset `+0x00` is `9` (or `7`).

### 4.3 CD-ROM Volume Check Offsets (No-CD / ISO Mounting)
- **Screamer 1**:
  - `STARTH.EXE` (`1,034,923`B): VA `0x0279EC` (file offset **`0x0321EC`**): `75 10` (`jne`) -> `EB 10` (`jmp`) bypasses `call 0x15874` (PVD check for `"PC"`, `"SCREAMER"`, `"VIE"`).
  - `STARTL.EXE` (`1,034,919`B): VA `0x02798C` (file offset **`0x03218C`**): `75 10` (`jne`) -> `EB 10` (`jmp`).
- **Screamer Rally (`/START0/` Small-Install Executables)**:
  - `STARTH.EXE` (`1,045,098`B): PVD check function at VA **`0x028CD8`** (file offset **`0x03B4D8`**) checks `"PC"`, `"SRALLY"`, `"VIE"`, `"MLSTN"`. Patching `0x03B4D8` to `b8 01 00 00 00 c3` (`mov eax, 1; ret`) or using `/START1/*.EXE` from `srally.iso` eliminates the check.

---

## 5. Configuration File Binary Layouts

### 5.1 `SCREAMER.CFG`
ASCII text: `<hdd_path> <cd_path> <gfx_path> <install_flag>` (e.g. `C:\ D: D: c` or `C: D: C: c` or `C:\SR D: C:\SR c`). Setting `<install_flag>` to `'n'` (`0x6E`) sets `[no_cd_flag] = 1`.

### 5.2 `CONFIG/SETUP.CFG` (`144` bytes in S1; `160` bytes in S2/SR)
- `S2/SR +0x00` (`int32`): Video timing preset (`9` = standard VESA).
- `S2/SR +0x04` (`int32`): Packed VGA CRTC retrace timings (`0x00A401EB` / `0x00A301EA`).
- `S1 +0x00` / `S2/SR +0x08` (`int32`): HMI SOS Digital Driver ID (`0xE016` = Sound Blaster 16 Stereo; `0` = None).
- `S1 +0x04` / `S2/SR +0x0C` (`int32`): HMI SOS MIDI Driver ID (`0x0012` = Sound Blaster / OPL; `0` = None).
- `S1 +0x08..0x10` / `S2/SR +0x10..0x18` (`3x int32`): Digital Port (`0x220`), IRQ (`5` or `7`), DMA (`1`).
- `S1 +0x20` / `S2/SR +0x28` (`int32`): MIDI Port (`0x388` or `0`).
- `S1 +0x84..0x8F` / `S2/SR +0x90..0x9F` (`int32[]`): Audio/Hardware flags (Digital SFX enabled, MIDI enabled, CD-Audio enabled, Benchmark passed).

### 5.3 `CONFIG/CONTROLS.CFG` & `DEFCTRL.CFG` (`1,328` bytes in S1; `1,496` bytes in S2/SR)
- **Digital Action Bindings**:
  - S1 (`0x000..0x0A7`): 6 actions x 7 `int32` (`[enabled, dev_type, axis_or_key, invert, group, submode, scancode]`) for Accelerate (`72` Up), Brake (`80` Down), Steer Left (`75` Left), Steer Right (`77` Right), Gear Up (`56` Alt), Gear Down (`29` Ctrl).
  - S2/SR (`0x000..0x14F`): 12 actions x 7 `int32` (6 for Player 1 + 6 for Player 2 split-screen).
- **Analog Joystick Calibration (`0x0A8..0x52F` in S1; `0x150..0x5D7` in S2/SR)**:
  - `1,160` bytes (`290` dwords) of analog axis min/center/max bounds and button/axis mappings.
