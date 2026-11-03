# Screamer Trilogy on Linux

The launcher is a native Linux program (`ScreamerLauncher`) with its own
patched DOSBox Staging (`dosbox`); no Wine is involved. Point it at your
own copy of **Screamer**, **Screamer 2** or **Screamer Rally** (a single
game folder, or a folder holding all three). Your game folder is only
read: the launcher works on its own copy. There are three ways to play:
the AppImage, the zip, or either one added to Steam.

## The AppImage

```sh
chmod +x ScreamerLauncher-v<version>-x86_64.AppImage
./ScreamerLauncher-v<version>-x86_64.AppImage
```

It contains the launcher, the bundled DOSBox, `screamer-play` (with
`rea-play.sh`, the shared play library it uses) and the libraries they
need. The settings window opens: pick your game folder, choose a game and
a look, and press **Play**. If
[gamescope](https://github.com/ValveSoftware/gamescope) is installed, the
game runs in it at the chosen size, scaled to the screen; otherwise in a
plain DOSBox window.

## The zip

Unpack it, for example into `~/Games/screamer-launcher`, and run:

```sh
~/Games/screamer-launcher/linux/screamer-play
```

`screamer-play` shows the settings window, then starts the game (in
gamescope when it is installed). It also understands:

| Command | What it does |
| --- | --- |
| `screamer-play 1` / `2` / `rally` | settings window with that game selected, then play |
| `screamer-play --play [1\|2\|rally] [options]` | play with the saved settings, no window (`start` is an alias); options such as `--scaler pixel` or `--resolution 1280x720` last that one run |
| `screamer-play stop` | end the game this script started (gamescope, launcher and DOSBox) |
| `screamer-play status` | `running` or `idle` |
| anything else | passed to `ScreamerLauncher` (e.g. `--check`, `--controllers`) |

Every launch waits until the game ends, so the launcher can copy your
saves back. A second start while the game runs says `already running`.
`ScreamerLauncher` can also be run directly; `--help` lists its options.

## Steam

In Desktop Mode: **Games → Add a Non-Steam Game to My Library… →
Browse**, and pick the AppImage or `linux/screamer-play`. No Proton: it is
a native program. In Steam's Game Mode (or any other gamescope session)
`screamer-play` does not start a second gamescope; the game runs in
Steam's.

### Steam Deck

Pick the **Steam Deck** look (chosen by default on the Deck's screen):
the race at the panel's 1280×800 in 16:9 widescreen. The touchscreen and
the pad drive the settings window. Game Mode has not been tested with the
real games yet.

## Environment

`screamer-play` reads these keys from `launcher.cfg` itself:

- `resolution`: the game's size in gamescope: `WIDTHxHEIGHT`,
  `fullscreen` (the screen's size, from `xrandr`; set `SCREAMER_SCREEN`
  to override it) or `original` (a 1920×1080 gamescope screen for the
  1280×960 window).
- `scaler`: gamescope's filter: `fsr` (default), `nearest`, `pixel` or
  `linear` (anything else is `fsr`).

`--resolution`, `--look` and `--scaler` after `--play` override them for
that run only; `launcher.cfg` is not changed. gamescope's size follows the
look as the launcher reads it (`REA_LOOK_SIZES`): Steam Deck is 1280×800,
Enhanced and Original 1920×1080, unless a `resolution` says otherwise.

| Variable | What it does |
| --- | --- |
| `SCREAMER_LAUNCHER` | the `ScreamerLauncher` to run |
| `SCREAMER_DOSBOX` | the DOSBox to run |
| `SCREAMER_GAME_DIR` | the game folder when none is saved |
| `SCREAMER_SCREEN` | `WIDTHxHEIGHT` for `resolution = fullscreen` |

## Where things are

| What | Where |
| --- | --- |
| Settings (`launcher.cfg`) | `~/.config/screamer-launcher/` (or next to `ScreamerLauncher`, if a `launcher.cfg` is there) |
| Log (`launcher.log`) | the same folder |
| Game output (`play.log`), settings-window output (`settings.log`) and the running-game lock (`running.pid`) | `~/.config/screamer-launcher/` |
| The launcher's copy of the game | `~/.local/share/screamer-launcher/runtime/<game>` (code files hard-linked when possible, the rest copied; CD images stay in your folder) |
| Your saves (`CHOICE.DAT`, `*.SAV`, replays) | `~/.local/share/screamer-launcher/saves/<game>` |

`XDG_CONFIG_HOME` and `XDG_DATA_HOME` move these as usual; `runtime =` and
`saves =` in `launcher.cfg` choose other folders (never inside the game
folder).

## Troubleshooting

- **Nothing happens, or the window doesn't open:** with no display or no
  OpenGL the launcher plays with the saved settings instead; see
  `launcher.log` and `play.log`.
- **The game is the wrong size in gamescope:** check `resolution` in
  `launcher.cfg`; for `fullscreen`, see what `xrandr --current` reports
  or set `SCREAMER_SCREEN=2560x1440`.
- **The pad isn't seen:** run `screamer-play --controllers` to list the
  controllers the launcher finds.
- **Anything else:** run `screamer-play --check` (it checks your game
  files and the fixes) and look at `launcher.log`.
