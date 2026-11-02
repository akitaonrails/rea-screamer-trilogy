# Third-party components

Everything bundled with or compiled into the package, with its licence and
source. Add a row before bundling anything new.

## Compiled into `screamer-launcher` / `ScreamerLauncher.exe`

| Component | Licence | Source |
| --- | --- | --- |
| `rea-launcher`, `rea-launcher-ui`, `rea-launcher-win32` (Windows) from rea-launcher-sdk (the tag in `sdk/VERSION`) | MIT OR Apache-2.0 | https://github.com/akitaonrails/rea-launcher-sdk |
| `sha2` and its RustCrypto dependencies | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| `eframe`, `egui` and their dependencies (`winit`, `glutin`, `glow`, `ab_glyph`, …): the settings window | MIT OR Apache-2.0 (dependencies: MIT, Apache-2.0, Zlib, BSD; see `cargo tree`) | https://github.com/emilk/egui |
| `egui_kittest` (tests only, not shipped) | MIT OR Apache-2.0 | https://github.com/emilk/egui |
| `jpeg-decoder` | MIT OR Apache-2.0 | https://github.com/image-rs/jpeg-decoder |
| `image`, `png` and their dependencies | MIT OR Apache-2.0 | https://github.com/image-rs/image |
| `libc` (Linux target) / `windows-sys` (Windows target) | MIT OR Apache-2.0 | https://github.com/rust-lang/libc , https://github.com/microsoft/windows-rs |
| Rust standard library | MIT OR Apache-2.0 | https://github.com/rust-lang/rust |

## Statically compiled into the bundled `dosbox` / `dosbox.exe`

| Component | Licence | Source |
| --- | --- | --- |
| Our own code (`crates/screamer-hooks`) | MIT OR Apache-2.0 | this repository |
| `dosbox-staging` v0.82.2 plus the source patch under `patches/` | GPL-2.0-or-later | https://github.com/dosbox-staging/dosbox-staging/tree/v0.82.2 |
| SDL2, Opus/Opusfile, SpeexDSP, libpng, zlib and their transitive runtime libraries | Zlib, BSD-3-Clause, Xiph BSD, BSD-2-Clause, Libpng, Zlib | https://github.com/libsdl-org/SDL , https://opus-codec.org/ , https://www.speex.org/ , https://libpng.org/ , https://zlib.net/ |

## Bundled in the Linux AppImage (built by `sdk/packaging/appimage-wine.sh --native`)

| Component | Licence | Source |
| --- | --- | --- |
| The project-built `dosbox-staging` v0.82.2 executable and selected shared libraries listed above | GPL-2.0-or-later and the libraries' respective licences | recursive corresponding-source archive attached to each release |
| AppImage runtime (added by `appimagetool`) | MIT | https://github.com/AppImage/type2-runtime |
| `rea-play.sh` (beside `screamer-play`, also in the zip's `linux/`), vendored from rea-launcher-sdk (the tag in `sdk/VERSION`) as `sdk/rea-play.sh` | MIT OR Apache-2.0 | https://github.com/akitaonrails/rea-launcher-sdk |
| The AppImage icon `packaging/appimage/screamer-launcher.png` | ours (MIT OR Apache-2.0), set in Barlow (OFL) | this repository |

## Settings window fonts (compiled into `screamer-launcher` through `rea-launcher-ui`)

| File | Licence | Source |
| --- | --- | --- |
| Barlow Regular and SemiBold, Barlow Condensed SemiBold and Black Italic (`rea-launcher-ui`'s `assets/fonts/`) | SIL OFL 1.1 (`rea-launcher-ui`'s `assets/fonts/OFL.txt`, also `rea_launcher_ui::theme::OFL`) | https://github.com/jpt/barlow |

## Images in the settings window and this repository

| File | Author / rights holder | Licence | Source |
| --- | --- | --- | --- |
| `crates/screamer-launcher/assets/cover_screamer1.jpg` (compiled into the launcher) | Box art of *Screamer* (1995) © Virgin Interactive, the publisher (developed by Graffiti / Milestone); the publisher's original packaging | All rights reserved; used with the owner's recorded decision of 2026-11-02 to identify the game in the settings window | Scan from MobyGames.com (the JPEG's comment says `MobyGames.com`) |
| `crates/screamer-launcher/assets/cover_screamer2.jpg` (compiled into the launcher) | Box art of *Screamer 2* (1996) © Virgin Interactive, the publisher (developed by Milestone); the publisher's original packaging | As above (owner decision 2026-11-02) | Scan from MobyGames.com (the JPEG's comment says `MobyGames.com`) |
| `crates/screamer-launcher/assets/cover_screamer_rally.jpg` (compiled into the launcher) | Box art of *Screamer Rally* (1997) © Virgin Interactive, the publisher (developed by Milestone); the publisher's original packaging | As above (owner decision 2026-11-02) | Believed to be a MobyGames.com scan like the other two, but this file carries no comment and its source was not recorded: to be confirmed |
| Gameplay video and its poster, linked from the README (hosted outside the repository, at `new-uploads-akitaonrails.s3.us-east-2.amazonaws.com/2026/11/02/rea-screamer-trilogy/`) | The repository owner's recording of *Screamer* (1995) running through the launcher | Game footage © the game's rights holders; recording the owner's | Recorded by the owner |
| `docs/images/*.jpg` (README only, not shipped) | Screenshots of this launcher's window, which shows the box art above | Ours for the window; the box art as above | Taken from the launcher |

The window shows the credit "Box art: Virgin Interactive. Scans: MobyGames" under the art.

## Shaders added to the bundled DOSBox (through `patches/`)

| File | Licence | Source |
| --- | --- | --- |
| `contrib/resources/glshaders/interpolation/fsr.glsl` (our GLSL implementation of AMD FidelityFX Super Resolution 1.0, EASU + RCAS, with frame generation) | GPL-2.0-or-later (as dosbox-staging); FSR 1.0 is AMD's algorithm, published by AMD under the MIT licence. Whether the shader copies code from AMD's `ffx_fsr1.h` was not established, so AMD's notice is reproduced below | https://github.com/GPUOpen-Effects/FidelityFX-FSR |

> Copyright (c) 2021 Advanced Micro Devices, Inc. All rights reserved.
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Used by the game at runtime but not shipped in this repository

Provided by the user's game folder (`STARTH.EXE`, `STARTL.EXE`, `START65H.EXE`,
`S2_3DFX.EXE`, `STRT3FX.EXE`, `DOS4GW.EXE`, `GLIDE2X.OVL`, `*.DAT`, `*.GOG`,
`*.ISO`) or by the host environment (`dosbox-staging`, `gamescope`, SDL2/SDL3,
OpenGL, PipeWire/PulseAudio).
