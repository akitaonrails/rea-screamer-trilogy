# Third-party components

Everything bundled with or compiled into the package, with its licence and
source. Add a row before bundling anything new.

## Compiled into `screamer-launcher` / `ScreamerLauncher.exe`

| Component | Licence | Source |
| --- | --- | --- |
| `rea-launcher`, `rea-launcher-ui`, `rea-launcher-win32` (Windows) from rea-launcher-sdk v0.1.0 | MIT OR Apache-2.0 | https://github.com/akitaonrails/rea-launcher-sdk/tree/v0.1.0 |
| `sha2` and its RustCrypto dependencies | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| `eframe`, `egui` and their dependencies (`winit`, `glutin`, `glow`, `ab_glyph`, …): the settings window | MIT OR Apache-2.0 (dependencies: MIT, Apache-2.0, Zlib, BSD; see `cargo tree`) | https://github.com/emilk/egui |
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
| `rea-play.sh` (beside `screamer-play`, also in the zip's `linux/`), vendored from rea-launcher-sdk v0.1.0 as `sdk/rea-play.sh` | MIT OR Apache-2.0 | https://github.com/akitaonrails/rea-launcher-sdk/tree/v0.1.0 |
| The AppImage icon `packaging/appimage/screamer-launcher.png` | ours (MIT OR Apache-2.0), set in Barlow (OFL) | this repository |

## Settings window fonts (compiled into `screamer-launcher` through `rea-launcher-ui`)

| File | Licence | Source |
| --- | --- | --- |
| Barlow Regular and SemiBold, Barlow Condensed SemiBold and Black Italic (`rea-launcher-ui`'s `assets/fonts/`) | SIL OFL 1.1 (`rea-launcher-ui`'s `assets/fonts/OFL.txt`, also `rea_launcher_ui::theme::OFL`) | https://github.com/jpt/barlow |

## Used by the game at runtime but not shipped in this repository

Provided by the user's game folder (`STARTH.EXE`, `STARTL.EXE`, `START65H.EXE`,
`S2_3DFX.EXE`, `STRT3FX.EXE`, `DOS4GW.EXE`, `GLIDE2X.OVL`, `*.DAT`, `*.GOG`,
`*.ISO`) or by the host environment (`dosbox-staging`, `gamescope`, SDL2/SDL3,
OpenGL, PipeWire/PulseAudio).
