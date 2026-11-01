# Third-party components

Everything bundled with or compiled into the package, with its licence and
source. Add a row before bundling anything new.

## Compiled into `screamer-launcher` / `ScreamerLauncher.exe`

| Component | Licence | Source |
| --- | --- | --- |
| `sha2` and its RustCrypto dependencies | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes |
| `eframe`, `egui` and their dependencies (`winit`, `glutin`, `glow`, `ab_glyph`, …): the settings window | MIT OR Apache-2.0 (dependencies: MIT, Apache-2.0, Zlib, BSD; see `cargo tree`) | https://github.com/emilk/egui |
| `jpeg-decoder` | MIT OR Apache-2.0 | https://github.com/image-rs/jpeg-decoder |
| `image`, `png` and their dependencies | MIT OR Apache-2.0 | https://github.com/image-rs/image |
| `libc` (Linux target) / `windows-sys` (Windows target) | MIT OR Apache-2.0 | https://github.com/rust-lang/libc , https://github.com/microsoft/windows-rs |
| Rust standard library | MIT OR Apache-2.0 | https://github.com/rust-lang/rust |

## Compiled into `libscreamer_hooks.so` / `screamerhooks.dll`

| Component | Licence | Source |
| --- | --- | --- |
| Our own code (`crates/screamer-hooks`) | MIT OR Apache-2.0 | this repository |
| Dynamic runtime symbols resolved via `dlsym` / `GetProcAddress` against `dosbox-staging` and `libSDL3` / `libSDL2` | GPL-2.0-or-later (`dosbox-staging`), Zlib (`SDL`) | https://github.com/dosbox-staging/dosbox-staging , https://github.com/libsdl-org/SDL |

## Bundled in the Linux AppImage (`packaging/appimage`)

| Component | Licence | Source |
| --- | --- | --- |
| `dosbox-staging` v0.82.2 (unmodified upstream release binary; licence and source notes under `usr/share/doc/dosbox-staging`) | GPL-2.0-or-later | https://github.com/dosbox-staging/dosbox-staging/releases/tag/v0.82.2 |
| AppImage runtime (added by `appimagetool`) | MIT | https://github.com/AppImage/type2-runtime |
| The AppImage icon `packaging/appimage/screamer-launcher.png` | ours (MIT OR Apache-2.0), set in Barlow (OFL) | this repository |

## Settings window fonts (compiled into `screamer-launcher`)

| File | Licence | Source |
| --- | --- | --- |
| `crates/screamer-launcher/assets/fonts/Barlow-*.ttf`, `BarlowCondensed-*.ttf` | SIL OFL 1.1 (`assets/fonts/OFL.txt`) | https://github.com/jpt/barlow |

## Used by the game at runtime but not shipped in this repository

Provided by the user's game folder (`STARTH.EXE`, `STARTL.EXE`, `START65H.EXE`,
`S2_3DFX.EXE`, `STRT3FX.EXE`, `DOS4GW.EXE`, `GLIDE2X.OVL`, `*.DAT`, `*.GOG`,
`*.ISO`) or by the host environment (`dosbox-staging`, `gamescope`, SDL2/SDL3,
OpenGL, PipeWire/PulseAudio).
