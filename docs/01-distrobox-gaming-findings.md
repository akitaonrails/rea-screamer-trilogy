# Distrobox-Gaming Playbook & Pain Points Analysis

This document records the research from `~/Projects/distrobox-gaming` (`docs/screamer.md`, `ansible/group_vars/all/screamer.yml`, `ansible/roles/install_screamer/`, and the repository's commit log) regarding how the **Screamer** trilogy was previously set up on Linux and why that setup was cumbersome, fragile, and limited.

---

## 1. Games Inventory & Source Formats

In `distrobox-gaming`, three MS-DOS games by Graffiti / Milestone were configured under `install_screamer`:

| Game | Year | Source Artifact in `ROMS_FINAL/PC/ports/Screamer/` | Extracted Layout | Executables | CD / Audio Image |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Screamer** (`screamer`) | 1995 | `setup_screamer_1.01_gog_v2_(86677).exe` (GOG InnoSetup installer, already patched to v1.01) | Root dir (`DATA/`, `GFX/`, `CONFIG/`, `TABS/`, `REPLAYS/`) | `STARTL.EXE` (VGA 320x200), `STARTH.EXE` (SVGA 640x480), `SETUP.EXE`, `DOS4GW.EXE` | `SCREAM.DAT` (CUE sheet) + `SCREAM.gog` (raw Mode1/Audio BIN image, ~382 MB) |
| **Screamer 2** (`screamer2`) | 1996 | `setup_screamer2_2.0.0.1.exe` (GOG InnoSetup installer) | `app/` subdir (`ANIMS/`, `CONFIG/`, `DATA/`, `GFX/`, `REPLAYS/`, `TABS/`) | `STARTL.EXE`, `STARTH.EXE`, `START65L.EXE`, `START65H.EXE`, `S2_3DFX.EXE` (Glide), `SETUP.EXE`, `DOS4GW.EXE` | `GAME.DAT` (CUE sheet) + `GAME.GOG` (raw BIN image, ~569 MB) |
| **Screamer Rally** (`screamer-rally`) | 1997 | `Screamer-Rally_DOS_EN_ISO-Version.zip` (CloneCD `.img/.ccd/.sub/.cue` rip) + `Screamer-Rally_Fix_DOS_EN_NoCD.zip` | Required manual sector-stripping (`2352 -> 2048` bytes) + interactive DOS installation into `hdd/SR/` | `STARTL.EXE`, `STARTH.EXE`, `START65L.EXE`, `START65H.EXE`, `STRT3FX.EXE` (3Dfx Glide), `STRT3DBL.EXE`/`STRT3DBH.EXE` (Creative 3D Blaster), `STRTMSI.EXE` (Matrox Mystique), `ST.EXE` (Setup) | `Screamer_Rally_ISO/srally.iso` (note: stripping 2352->2048 destroyed RedBook CD audio tracks if any existed!) |

---

## 2. How `distrobox-gaming` Ran Screamer

1. **Container Dependency (`distrobox`)**:
   - Required an Arch Linux container (`distrobox-enter -n gaming`) with `pacman` packages (`sdl2`, `libglvnd`, `opusfile`, `fluidsynth`, `libslirp`, `innoextract`).
   - Downloaded a pinned tarball of `dosbox-staging` (`v0.82.2`) into `{{ screamer_install_dir }}/dosbox-staging`.
2. **Shell Wrapper Generation (`screamer-launch.sh.j2`)**:
   - Generated a temporary `/tmp/<slug>-dosbox.XXXXXX.conf` on every run.
   - Hardcoded Hyprland IPC socket calls (`focusmonitor DP-1`) and NVIDIA PRIME offload variables (`__NV_PRIME_RENDER_OFFLOAD=1`, `__GLX_VENDOR_LIBRARY_NAME=nvidia`).
   - Required separate CLI subcommands (`play`, `setup`, `svga`, `install`) because the game's `SETUP.EXE` and main executables required completely different CPU cycle limits and mount configurations.

---

## 3. Major Pain Points, Bugs & Limitations in `distrobox-gaming`

### A. SVGA Mode (`STARTH.EXE`) Crashes
- According to the playbook and PCGamingWiki notes:
  - Running `STARTH.EXE` (640x480 SVGA mode) in Screamer 1 hard-crashes or hangs.
  - Because of this, `distrobox-gaming` forced `STARTL.EXE` (320x200 low-res VGA) as the default for Screamer 1 and Screamer 2!
  - In Screamer 2 and Screamer Rally, `START65L.EXE` rendered into a squished band at the top of the screen, while `START65H.EXE` (VESA 640x480 16-bit 65k colors) worked in software mode but required tremendous emulated CPU cycles.

### B. 3Dfx Glide Mode (`S2_3DFX.EXE` / `STRT3FX.EXE`) Unusable
- Both **Screamer 2** (`S2_3DFX.EXE` + `glide2x.ovl`) and **Screamer Rally** (`STRT3FX.EXE` + `GLIDE2X.OVL`) have hardware-accelerated 3Dfx Glide renderers with bilinear texture filtering, perspective correction, fog/transparency, and smooth framerates.
- However, `dosbox-staging v0.82.2` had **no 3Dfx Glide emulation or passthrough**, so selecting 3Dfx immediately crashed with a missing Glide/Voodoo error.
- Even when Glide wrappers (like nGlide on Windows) are used, DOS Glide runs via an OVL (`GLIDE2X.OVL`) that communicates over a virtual/I/O bridge or Voodoo LFB/FIFO hardware registers, which standard DOSBox builds without Glide patches cannot accelerate or upscale natively.

### C. `SETUP.EXE` Crashes at Normal CPU Speeds (Divide Error / Timing Loop)
- `SETUP.EXE` (and `ST.EXE` in Screamer Rally) crashes at high CPU cycles (`60000` or `max`).
- The user had to run a separate command (`screamer-launch setup`) that throttled `cpu_cycles_protected = 20000` just to change audio or control settings, and then relaunch in `play` mode.

### D. CPU Cycle Goldilocks Problem & Framerate Instability
- Commit history (`6de255a` -> `c75c445` -> `147ec55` -> `1e43597`) shows repeated trial-and-error trying to find a `cpu_cycles_protected` value:
  - `60000` (~486 speed) starved 640x480 software rendering (laggy, low FPS).
  - `max` ran menu loops or physics too fast in certain sections or caused audio/timer stutter.
  - `120000` and `200000` were tried back and forth.

### E. Broken / Cumbersome Gamepad Support
- Screamer's DOS code only polls a legacy **2-axis, 2-button DOS gameport joystick** (`port 0x201`) or keyboard scancodes (`INT 9` / `port 0x60`).
- There is **no joystick menu** in `SETUP.EXE` if a 4-axis stick is presented (it fails detection).
- `dosbox-staging` recognized modern controllers as SDL GameControllers and did **not** auto-map them to the emulated 2-axis joystick or keyboard.
- Workaround required manually opening DOSBox's `Ctrl+F1` keymapper GUI, binding a specific controller (`stick_0`, 8BitDo) to keyboard arrow keys (`key_up`, `key_down`, `key_left`, `key_right`, `key_m`, `key_lctrl`, `key_lalt`, `F1-F4`), and committing a brittle `screamer.map` file:
  - Mapping an analog stick (`stick_0 axis 0`) and analog triggers (`axis 2`, `axis 5`) to **digital arrow keys** meant **zero analog steering or throttle precision** (unless the 2-axis gameport calibration worked, which conflicted with simultaneous keyboard arrow bindings in `screamer.map`!).
  - Controller hotplugging, different controller GUIDs/button layouts, deadzones, sensitivity curves, and force-feedback/rumble were completely unsupported.

### F. Interactive DOS Install & Destructive ISO Conversion (Screamer Rally)
- For Screamer Rally, the Ansible playbook ran a Python script that stripped 2352-byte raw sectors down to 2048-byte ISO sectors (`s[16:2064]`) and deleted `.img/.ccd/.sub/.cue`, which destroys any RedBook CD audio tracks on mixed-mode CDs and required the user to manually step through `INSTALL.BAT` inside DOS (`screamer-rally-launch install`).

---

## 4. Goals for `rea-screamer`

By following the architecture of `~/Projects/rea-colin-mcrae` and performing binary-level reverse engineering on Screamer:
1. **Zero Distrobox / Zero Manual Setup**: Point the launcher to a local Screamer installation (or GOG installer / extracted directory / ISO) and click Play.
2. **Fix Binary Bugs Directly**:
   - Diagnose and patch/fix the `SETUP.EXE` high-cycle crash and `STARTH.EXE` SVGA crash so separate low-cycle setup modes and broken SVGA modes are a thing of the past.
   - Auto-configure sound (HMI SOS `SETUP.CFG`), paths (`SCREAMER.CFG`), and controls (`CONTROLS.CFG` / `CHOICE.DAT`) directly from the launcher GUI/CLI without needing to run DOS `SETUP.EXE` at all!
3. **Modern Graphics Pipeline (4K Upscaling, Widescreen, 60 FPS)**:
   - Understand how Screamer's software SVGA/VGA/16-bit VESA renderer and 3Dfx Glide renderer work at the binary level, and provide crisp 4K upscaling, aspect ratio / widescreen projection (or clean 4:3 pillarbox with widescreen 3D FOV patch), and smooth 60 FPS framerate pacing.
4. **Native Modern SDL3 Input & Audio**:
   - True analog steering & throttle/brake with customizable deadzones, linearity, gear shifting, camera views, and UI navigation mapped cleanly from any modern gamepad/wheel without `Ctrl+F1` mapper files.
   - Full RedBook CD audio (`SCREAM.DAT`/`SCREAM.gog`, `GAME.DAT`/`GAME.GOG`) + HMI Sound Operating System PCM/MIDI audio working out of the box.
5. **Cross-Platform (Linux & Windows)**:
   - Same clean Rust workspace structure, GUI launcher, CLI options, automated tests, and packaging as `rea-colin-mcrae`.
