//! Command line and the settings file (`launcher.cfg`).
//!
//! Every option is declared once in [`OPTIONS`] (key, flag, values, label,
//! description); `--help`, the commented `launcher.cfg` template and the
//! checks of file values and flags come from that table
//! ([`rea_launcher::cfg::spec`]), and one [`Settings::apply`] takes the
//! checked values from both. Unknown keys are reported and kept, so older
//! launchers can read newer files; values it cannot read are reported and
//! ignored. Old spellings (`fps_limit`, `renderer = glide`, `which = 2`,
//! `deadzone = off`) are still read: see [`normalize`].

use std::borrow::Cow;
use std::path::PathBuf;

use rea_launcher::cfg::spec::{self, Kind, OptionSpec, Value};
use rea_launcher::cfg::{self, ArgError, KeyStyle, Rejected, Resolution};

use crate::edition::Game;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    pub which: Option<Game>,
    pub game: Option<PathBuf>,
    pub runtime: Option<PathBuf>,
    pub saves: Option<PathBuf>,
    pub dosbox: Option<PathBuf>,
    pub renderer: Option<RendererChoice>,
    pub resolution: Option<(u32, u32)>,
    pub widescreen: Option<bool>,
    pub extended_draw_distance: Option<bool>,
    pub frame_generation: Option<bool>,
    pub vsync: Option<bool>,
    pub texture_filter: Option<TextureFilter>,
    pub gamepad_axes: Option<bool>,
    pub deadzone: Option<u32>,
    pub rumble: Option<u32>,
    pub buttons: Option<[u8; 8]>,
    pub scaler: Option<Scaler>,
}

/// Renderer selection (`renderer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererChoice {
    Auto,
    Voodoo,
    Svga16Bit,
    Svga8Bit,
    Vga,
}

impl RendererChoice {
    pub const ALL: [RendererChoice; 5] = [
        RendererChoice::Auto,
        RendererChoice::Voodoo,
        RendererChoice::Svga16Bit,
        RendererChoice::Svga8Bit,
        RendererChoice::Vga,
    ];

    pub fn key(self) -> &'static str {
        match self {
            RendererChoice::Auto => "auto",
            RendererChoice::Voodoo => "3dfx",
            RendererChoice::Svga16Bit => "svga16",
            RendererChoice::Svga8Bit => "svga8",
            RendererChoice::Vga => "vga",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RendererChoice::Auto => "Auto",
            RendererChoice::Voodoo => "3dfx Voodoo",
            RendererChoice::Svga16Bit => "SVGA 16-bit",
            RendererChoice::Svga8Bit => "SVGA 8-bit",
            RendererChoice::Vga => "VGA 320×200",
        }
    }

    /// Returns `Some(reason)` when `self` is not supported by `game` (so the UI
    /// grays out the chip and explains why).
    pub fn disabled_reason(self, game: Game, has_dos_glide: bool) -> Option<&'static str> {
        match (game, self) {
            (Game::Screamer, RendererChoice::Voodoo | RendererChoice::Svga16Bit) => Some(
                "Screamer (1995) only shipped with 8-bit SVGA (640×480) and VGA (320×200) renderers.",
            ),
            (Game::Screamer2 | Game::ScreamerRally, RendererChoice::Vga) => Some(
                "Screamer 2 and Screamer Rally use 640×480 3dfx Voodoo, 16-bit SVGA, or 8-bit SVGA.",
            ),
            (Game::Screamer2, RendererChoice::Voodoo) if !has_dos_glide => Some(
                "3dfx Voodoo in Screamer 2 requires DOS GLIDE2X.OVL (automatically linked when Screamer Rally is in the same trilogy folder).",
            ),
            _ => None,
        }
    }

    pub fn is_supported_for(self, game: Game, has_dos_glide: bool) -> bool {
        self.disabled_reason(game, has_dos_glide).is_none()
    }

    /// The key or one of the older spellings (`voodoo`, `glide`, the
    /// executable names).
    pub fn parse(v: &str) -> Option<RendererChoice> {
        match v.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(RendererChoice::Auto),
            "3dfx" | "voodoo" | "glide" => Some(RendererChoice::Voodoo),
            "svga16" | "start65h" | "16bit" => Some(RendererChoice::Svga16Bit),
            "svga8" | "starth" | "svga" => Some(RendererChoice::Svga8Bit),
            "vga" | "startl" => Some(RendererChoice::Vga),
            _ => None,
        }
    }
}

/// The game's own pad layout: every button presses its own.
pub const ORIGINAL_BUTTONS: [u8; 8] = [0; 8];

/// Modern layout for Xbox-style pads:
/// 0: Accelerate (RT=10; A=1 also works in race & menus), 1: Back/Cancel (B=2),
/// 2: Camera (Y=4), 3: Gear Down (LB=5), 4: Brake (LT=9; X=3 also works in race & menus),
/// 5: Gear Up (RB=6), 6: Back/Select (7), 7: Start/Confirm (8).
pub const MODERN_BUTTONS: [u8; 8] = [10, 2, 4, 5, 9, 6, 7, 8];

/// Legacy Modern layout (A=1 for Accelerate, X=3 for Brake).
pub const LEGACY_MODERN_BUTTONS: [u8; 8] = [1, 2, 4, 5, 3, 6, 7, 8];

pub const DEFAULT_BUTTONS: [u8; 8] = MODERN_BUTTONS;

fn parse_buttons(v: &str) -> Option<[u8; 8]> {
    match v.trim().to_ascii_lowercase().as_str() {
        "original" => return Some(ORIGINAL_BUTTONS),
        "modern" => return Some(MODERN_BUTTONS),
        _ => {}
    }
    let numbers: Vec<u8> = v
        .split(',')
        .map(|n| n.trim().parse().ok().filter(|n| *n <= 32))
        .collect::<Option<_>>()?;
    numbers.try_into().ok()
}

pub fn buttons_value(b: &[u8; 8]) -> String {
    match *b {
        ORIGINAL_BUTTONS => "original".into(),
        MODERN_BUTTONS | LEGACY_MODERN_BUTTONS => "modern".into(),
        _ => b.map(|n| n.to_string()).join(","),
    }
}

/// `scaler` (Linux, gamescope's filter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scaler {
    Nearest,
    Pixel,
    Fsr,
    Linear,
}

impl Scaler {
    pub const ALL: [Scaler; 4] = [Scaler::Nearest, Scaler::Pixel, Scaler::Fsr, Scaler::Linear];

    pub fn key(self) -> &'static str {
        match self {
            Scaler::Nearest => "nearest",
            Scaler::Pixel => "pixel",
            Scaler::Fsr => "fsr",
            Scaler::Linear => "linear",
        }
    }

    fn parse(v: &str) -> Option<Scaler> {
        let v = v.trim().to_ascii_lowercase();
        Scaler::ALL.into_iter().find(|s| s.key() == v)
    }
}

/// `texture_filter`: original (CRT), fsr (AMD FSR 1.0 EASU+RCAS), smooth (bilinear), or sharp (nearest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilter {
    Original,
    Fsr,
    Smooth,
    Sharp,
}

impl TextureFilter {
    pub fn key(self) -> &'static str {
        match self {
            TextureFilter::Original => "original",
            TextureFilter::Fsr => "fsr",
            TextureFilter::Smooth => "smooth",
            TextureFilter::Sharp => "sharp",
        }
    }

    fn parse(v: &str) -> Option<TextureFilter> {
        [
            TextureFilter::Original,
            TextureFilter::Fsr,
            TextureFilter::Smooth,
            TextureFilter::Sharp,
        ]
        .into_iter()
        .find(|f| f.key().eq_ignore_ascii_case(v.trim()))
    }
}

pub const FILE_NAME: &str = "launcher.cfg";

/// `which`: the games, by their `launcher.cfg` name.
pub const GAMES: &[(&str, &str)] = &[
    ("screamer", "Screamer"),
    ("screamer2", "Screamer 2"),
    ("screamer-rally", "Screamer Rally"),
];
pub const RENDERERS: &[(&str, &str)] = &[
    ("auto", "Auto"),
    ("3dfx", "3dfx Voodoo"),
    ("svga16", "SVGA 16-bit"),
    ("svga8", "SVGA 8-bit"),
    ("vga", "VGA 320×200"),
];
pub const FILTERS: &[(&str, &str)] = &[
    ("original", "CRT Monitor"),
    ("fsr", "FSR (Super Res)"),
    ("sharp", "Sharp (Pixel)"),
    ("smooth", "Smooth (Bilinear)"),
];
pub const SCALERS: &[(&str, &str)] = &[
    ("nearest", "Sharp"),
    ("pixel", "Even pixels"),
    ("fsr", "FSR"),
    ("linear", "Smooth"),
];

const SIZES: Kind = Kind::Size {
    min: (320, 200),
    max: (7680, 4320),
    aliases: cfg::ORIGINAL_OR_FULLSCREEN,
};

/// Every option, once: the window, the command line, `--help` and the
/// template all come from these rows.
pub const OPTIONS: &[OptionSpec] = &[
    OptionSpec::new(
        "which",
        Kind::Choice(GAMES),
        "Game",
        "Which game of the trilogy Play starts. On the command line also 1, 2 or rally.",
    )
    .with_aliases(&["play"])
    .with_example("screamer"),
    OptionSpec::new(
        "game",
        Kind::Path,
        "Game folder",
        "The folder with your Screamer game, or a folder holding two or three of them. It is only read, never changed.",
    ),
    OptionSpec::new(
        "saves",
        Kind::Path,
        "Saves folder",
        "Where your saves, choices and replays are kept between sessions.",
    ),
    OptionSpec::new(
        "runtime",
        Kind::Path,
        "Runtime folder",
        "Where the launcher keeps its private, writable copy of the game.",
    )
    .launcher_only(),
    OptionSpec::new(
        "dosbox",
        Kind::Path,
        "DOSBox",
        "A different dosbox-staging program to run the game with, instead of the bundled one.",
    )
    .with_placeholder("<program>")
    .with_needs("a dosbox-staging program")
    .launcher_only(),
    OptionSpec::new(
        "renderer",
        Kind::Choice(RENDERERS),
        "Renderer",
        "Which of the game's own graphics programs runs. Auto picks the best one the game has.",
    )
    .with_example("auto"),
    OptionSpec::new(
        "resolution",
        SIZES,
        "Display",
        "Window size (WIDTHxHEIGHT), fullscreen, or original for the 4:3 1280×960 window.",
    )
    .with_example("1920x1080"),
    OptionSpec::new(
        "widescreen",
        Kind::Toggle,
        "Widescreen",
        "On a 16:9 screen, shows more to the sides instead of stretching the picture.",
    )
    .with_example("on"),
    OptionSpec::new(
        "extended_draw_distance",
        Kind::Toggle,
        "Extended draw distance",
        "Scenery and cars appear farther ahead, so less pops in.",
    )
    .with_example("on"),
    OptionSpec::new(
        "frame_generation",
        Kind::Toggle,
        "Frame generation (100 FPS)",
        "Shows 100 frames per second by adding a blended frame between each of the game's 50; the game itself runs as always. Experimental.",
    )
    .with_aliases(&["fps_limit"])
    .with_example("off"),
    OptionSpec::new(
        "vsync",
        Kind::Toggle,
        "Vsync",
        "Waits for the screen's refresh, so the picture never tears.",
    )
    .with_example("on"),
    OptionSpec::new(
        "texture_filter",
        Kind::Choice(FILTERS),
        "Shader / Upscaler",
        "How the game's picture is enlarged: CRT look, FSR, crisp pixels or smooth.",
    )
    .with_example("sharp"),
    OptionSpec::new(
        "gamepad_axes",
        Kind::Toggle,
        "Gamepad",
        "Analog steering, trigger pedals and rumble from a modern gamepad.",
    )
    .with_example("on"),
    OptionSpec::new(
        "deadzone",
        Kind::Range { min: 0, max: 30 },
        "Deadzone",
        "How far the stick moves before the car steers, in percent (the game had 8).",
    )
    .with_example("0"),
    OptionSpec::new(
        "rumble",
        Kind::Range { min: 0, max: 100 },
        "Rumble",
        "How strongly the gamepad rumbles when you brake hard, 0 to 100.",
    )
    .with_example("60"),
    OptionSpec::new(
        "buttons",
        Kind::Text,
        "Buttons",
        "Gamepad buttons: modern, original, or 8 button numbers (accelerate, back, camera, gear down, brake, gear up, select, start).",
    )
    .with_example("modern"),
    OptionSpec::new(
        "scaler",
        Kind::Choice(SCALERS),
        "Scaler",
        "Linux: how gamescope enlarges the game to the screen (read by screamer-play).",
    )
    .helper_owned()
    .with_example("fsr"),
];

/// The keys a command-line flag also saves in `launcher.cfg` (every other
/// flag lasts one run; `screamer-play` reads a one-run `--scaler` and
/// `--resolution` itself through `REA_HELPER_KEYS`).
pub const REMEMBERED: &[&str] = &["game", "which"];

/// The row of `key` (canonical key or alias).
pub fn option(key: &str) -> &'static OptionSpec {
    spec::find(OPTIONS, key).unwrap_or_else(|| panic!("no option row {key}"))
}

pub fn template() -> String {
    spec::template(
        "Screamer Trilogy (Screamer / Screamer 2 / Screamer Rally) launcher settings.\n\n\
         One setting per line as key = value; lines starting with # are comments. \
         A setting left out uses the launcher's default. Command-line flags \
         (--game, --which, --resolution, ...) override this file for one run; \
         --game and --which are remembered.",
        OPTIONS,
    )
}

/// The first `launcher.cfg`: the template with the recommended look for
/// `screen` saved explicitly (the Steam Deck look on a Deck, else
/// Enhanced), so the defaults are the recommended experience.
pub fn first_run(screen: Option<(u32, u32)>) -> String {
    let mut form = crate::form::Form::new(&Settings::default(), screen);
    form.set_look(crate::form::recommended(screen), screen);
    let mut s = form.settings();
    // Folders stay unset until the player picks them.
    s.game = None;
    s.saves = None;
    save_keys(&template(), &s, crate::form::WINDOW_KEYS)
}

/// The `--help` text.
pub fn usage() -> String {
    let lines = [
        "Without options the settings window opens first.",
        "",
        "  --no-window              play with the saved settings, no window",
        "  --settings [--linux]     only the settings window; exit 0: Play pressed,",
        "                           3: closed without Play, 4: no window possible",
        "                           (play with the saved settings)",
        "  --check [--game <folder>] [--which <1|2|rally>]",
        "                           check every game found in the folder and every",
        "                           binary fix, without playing",
        "  --controllers            list the connected gamepads",
        "  --help                   this text",
        "",
        "resolution takes WIDTHxHEIGHT, fullscreen (the screen's size) or original.",
        "Exit codes: 0 played or checked, 2 an error (see launcher.log next to",
        "launcher.cfg), 5 the launcher or its game is already running (nothing",
        "started), otherwise the game's own.",
    ];
    format!(
        "ScreamerLauncher {}\n\nScreamerLauncher [1|2|rally] [options]\n\n\
         Options (the same keys in launcher.cfg; they override it for one run,\n\
         --game and --which are remembered):\n{}\n{}",
        env!("CARGO_PKG_VERSION"),
        spec::usage(OPTIONS, KeyStyle::Underscore),
        lines.join("\n")
    )
}

pub const ORIGINAL: (u32, u32) = (0, 0);
pub const FULLSCREEN: (u32, u32) = (1, 1);

/// The race size to give DOSBox on a screen of `screen` pixels. DOSBox
/// cannot open a window larger than the screen (it rejects the setting), so
/// a size above 1920x1080 that does not fit becomes fullscreen, with a
/// warning saying so. Sizes up to 1080p are passed on as before (e.g.
/// 1920x1080 on a Steam Deck, scaled by gamescope).
pub fn fit_to_screen(
    race: Option<(u32, u32)>,
    screen: (u32, u32),
) -> (Option<(u32, u32)>, Option<String>) {
    match race {
        Some((w, h))
            if (w, h) != FULLSCREEN && above_1080p((w, h)) && (w > screen.0 || h > screen.1) =>
        {
            (
                Some(FULLSCREEN),
                Some(format!(
                    "resolution {w}x{h} is larger than the {}x{} screen; playing fullscreen instead",
                    screen.0, screen.1
                )),
            )
        }
        other => (other, None),
    }
}

fn above_1080p((w, h): (u32, u32)) -> bool {
    w > 1920 || h > 1080
}

/// Older spellings, read as today's: a game number (`which = 2`), a
/// renderer's executable name, `deadzone`/`rumble` `off`, and the old
/// `fps_limit` (30, 50 or 100 frames per second; only 60 and above ever
/// changed anything: frame generation) as `frame_generation`. An
/// `fps_limit` that version never took (a number below 10 or above 1000)
/// is refused, as it was then, instead of read as a switch (`1` = on).
/// The error is what the key takes, for the message.
pub fn normalize<'a>(key: &str, value: &'a str) -> Result<Cow<'a, str>, &'static str> {
    let owned = |s: &str| Ok(Cow::Owned(s.to_string()));
    match key {
        "which" | "play" => {
            Game::parse(value).map_or(Ok(Cow::Borrowed(value)), |g| owned(g.slug()))
        }
        "renderer" => {
            RendererChoice::parse(value).map_or(Ok(Cow::Borrowed(value)), |r| owned(r.key()))
        }
        "deadzone" | "rumble" if value.trim().eq_ignore_ascii_case("off") => owned("0"),
        "fps_limit" => {
            let v = value.trim();
            let fps = if v.eq_ignore_ascii_case("off") {
                Some(0)
            } else {
                v.parse::<u32>().ok()
            };
            match fps {
                Some(f) if f == 0 || (10..=1000).contains(&f) => owned(cfg::on_off(f >= 60)),
                Some(_) => Err("a number from 10 to 1000 or off"),
                None => Ok(Cow::Borrowed(value)),
            }
        }
        _ => Ok(Cow::Borrowed(value)),
    }
}

/// Why the command line was not taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgsError {
    /// `--help`, `-h` or `/?`.
    Help,
    /// A bad flag or value: the message, without the usage.
    Bad(String),
}

impl Settings {
    pub fn parse_file(text: &str) -> (Settings, Vec<String>) {
        let mut s = Settings::default();
        let warnings = cfg::parse(text, FILE_NAME, |key, value| {
            let value = normalize(key, value).map_err(Rejected::needs)?;
            spec::dispatch(OPTIONS, key, &value, &mut |key, value| s.apply(key, value))
        });
        (s, warnings)
    }

    /// Takes one checked value from `launcher.cfg` or a flag.
    pub fn apply(&mut self, key: &str, value: Value) -> Result<(), Rejected> {
        let path = |t: &str| Some(PathBuf::from(t));
        match (key, value) {
            ("which", Value::Choice(v)) => self.which = Game::parse(v),
            ("game", Value::Text(t)) => self.game = path(t),
            ("saves", Value::Text(t)) => self.saves = path(t),
            ("runtime", Value::Text(t)) => self.runtime = path(t),
            ("dosbox", Value::Text(t)) => self.dosbox = path(t),
            ("renderer", Value::Choice(v)) => self.renderer = RendererChoice::parse(v),
            ("resolution", Value::Size(r)) => {
                self.resolution = Some(match r {
                    Resolution::Original => ORIGINAL,
                    Resolution::Fullscreen => FULLSCREEN,
                    Resolution::Size(w, h) => (w, h),
                })
            }
            ("widescreen", Value::Toggle(on)) => self.widescreen = Some(on),
            ("extended_draw_distance", Value::Toggle(on)) => self.extended_draw_distance = Some(on),
            ("frame_generation", Value::Toggle(on)) => self.frame_generation = Some(on),
            ("vsync", Value::Toggle(on)) => self.vsync = Some(on),
            ("texture_filter", Value::Choice(v)) => self.texture_filter = TextureFilter::parse(v),
            ("gamepad_axes", Value::Toggle(on)) => self.gamepad_axes = Some(on),
            ("deadzone", Value::Number(n)) => self.deadzone = Some(n),
            ("rumble", Value::Number(n)) => self.rumble = Some(n),
            ("buttons", Value::Text(t)) => {
                self.buttons = Some(parse_buttons(t).ok_or_else(|| {
                    Rejected::needs("modern, original or 8 button numbers from 0 to 32")
                })?)
            }
            ("scaler", Value::Choice(v)) => self.scaler = Scaler::parse(v),
            _ => return Err(Rejected::Unknown),
        }
        Ok(())
    }

    pub fn merged(self, over: Settings) -> Settings {
        Settings {
            which: over.which.or(self.which),
            game: over.game.or(self.game),
            runtime: over.runtime.or(self.runtime),
            saves: over.saves.or(self.saves),
            dosbox: over.dosbox.or(self.dosbox),
            renderer: over.renderer.or(self.renderer),
            resolution: over.resolution.or(self.resolution),
            widescreen: over.widescreen.or(self.widescreen),
            extended_draw_distance: over.extended_draw_distance.or(self.extended_draw_distance),
            frame_generation: over.frame_generation.or(self.frame_generation),
            vsync: over.vsync.or(self.vsync),
            texture_filter: over.texture_filter.or(self.texture_filter),
            gamepad_axes: over.gamepad_axes.or(self.gamepad_axes),
            deadzone: over.deadzone.or(self.deadzone),
            rumble: over.rumble.or(self.rumble),
            buttons: over.buttons.or(self.buttons),
            scaler: over.scaler.or(self.scaler),
        }
    }

    pub fn which_game(&self) -> Game {
        self.which.unwrap_or(Game::Screamer)
    }

    pub fn rumble_strength(&self) -> u32 {
        self.rumble.unwrap_or(60).min(100)
    }

    pub fn frame_generation_enabled(&self) -> bool {
        self.frame_generation.unwrap_or(false)
    }

    pub fn vsync_enabled(&self) -> bool {
        self.vsync.unwrap_or(true)
    }

    pub fn race_resolution(&self) -> Option<(u32, u32)> {
        match self.resolution {
            Some(ORIGINAL) => None,
            Some(r) => Some(r),
            None => Some((1920, 1080)),
        }
    }

    pub fn widescreen_enabled(&self) -> bool {
        self.widescreen.unwrap_or(true)
    }

    pub fn extended_draw_distance_enabled(&self) -> bool {
        self.extended_draw_distance.unwrap_or(true)
    }

    pub fn effective_filter(&self) -> TextureFilter {
        self.texture_filter.unwrap_or(TextureFilter::Sharp)
    }

    pub fn button_map(&self) -> [u8; 8] {
        self.buttons.unwrap_or(DEFAULT_BUTTONS)
    }

    pub fn renderer_choice(&self) -> RendererChoice {
        self.renderer.unwrap_or(RendererChoice::Auto)
    }

    /// Optional binary fixes (`E2`, `E5`, `E6`) that these settings turn
    /// off. (`E1`, the frame pacing, has no bytes: every unlock broke the
    /// physics; see docs/04.)
    pub fn skipped_fixes(&self) -> Vec<&'static str> {
        let mut skip = Vec::new();
        // E2 scales horizontal camera basis M00, M01, M02 by 3/4 for 16:9 Hor+.
        if !self.widescreen_enabled() {
            skip.push("E2");
        }
        // E5 zeroes the hardcoded 20/256 (7.8%) analog steering deadzone.
        if self.gamepad_axes == Some(false) || self.deadzone.unwrap_or(0) > 0 {
            skip.push("E5");
        }
        if !self.extended_draw_distance_enabled() {
            skip.push("E6");
        }
        skip
    }

    /// The command line's settings: `--key value` (or `--key=value`) pairs
    /// and a bare game name (`1`, `2`, `rally`, ...), which selects the game
    /// as `--which`.
    pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Settings, ArgsError> {
        let mut words = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            // `--key=value` is read as `--key value`, so old spellings
            // (`--which=2`, `--fps-limit=100`) are normalized either way.
            let (arg, inline) = match arg.split_once('=') {
                Some((flag, value)) if flag.starts_with("--") => {
                    (flag.to_string(), Some(value.to_string()))
                }
                _ => (arg, None),
            };
            let row =
                cfg::flag_key(&arg).and_then(|key| spec::find(OPTIONS, &key).map(|r| (key, r)));
            if let Some((key, _)) = row {
                if let Some(v) = inline.or_else(|| args.next()) {
                    let value = normalize(&key, &v)
                        .map_err(|what| ArgsError::Bad(format!("{arg} needs {what}, not {v}")))?;
                    words.extend([arg, value.into_owned()]);
                } else {
                    words.push(arg);
                }
            } else if let Some(game) = Game::parse(&arg) {
                words.extend(["--which".to_string(), game.slug().to_string()]);
            } else {
                words.push(match inline {
                    Some(value) => format!("{arg}={value}"),
                    None => arg,
                });
            }
        }
        let mut s = Settings::default();
        spec::parse_args(words, KeyStyle::Underscore, OPTIONS, &[], |key, value| {
            s.apply(key, value)
        })
        .map_err(|e| match e {
            ArgError::Help(_) => ArgsError::Help,
            e => ArgsError::Bad(e.to_string()),
        })?;
        Ok(s)
    }
}

/// The value `launcher.cfg` holds for `key` in `s`, as the window and the
/// first run save it (`None`: the key stays unset).
pub fn cfg_value(s: &Settings, key: &str) -> Option<String> {
    let path = |p: &Option<PathBuf>| {
        p.as_ref()
            .map(|p| p.display().to_string())
            .filter(|p| !p.trim().is_empty())
    };
    let toggle = |b: Option<bool>| b.map(|b| cfg::on_off(b).to_string());
    match key {
        "which" => s.which.map(|g| g.slug().to_string()),
        "game" => path(&s.game),
        "saves" => path(&s.saves),
        "runtime" => path(&s.runtime),
        "dosbox" => path(&s.dosbox),
        "renderer" => s.renderer.map(|r| r.key().to_string()),
        "resolution" => s.resolution.map(|r| match r {
            ORIGINAL => "original".to_string(),
            FULLSCREEN => "fullscreen".to_string(),
            (w, h) => format!("{w}x{h}"),
        }),
        "widescreen" => toggle(s.widescreen),
        "extended_draw_distance" => toggle(s.extended_draw_distance),
        "frame_generation" => toggle(s.frame_generation),
        "vsync" => toggle(s.vsync),
        "texture_filter" => s.texture_filter.map(|f| f.key().to_string()),
        "gamepad_axes" => toggle(s.gamepad_axes),
        "deadzone" => s.deadzone.map(|d| d.to_string()),
        "rumble" => s.rumble.map(|r| r.to_string()),
        "buttons" => s.buttons.map(|b| buttons_value(&b)),
        "scaler" => s.scaler.map(|k| k.key().to_string()),
        _ => None,
    }
}

/// `text` with every key of `keys` written from `s` (set, or cleared when
/// `s` has no value); renamed keys' old lines are commented out.
pub fn save_keys(text: &str, s: &Settings, keys: &[&str]) -> String {
    let mut t = text.to_string();
    for key in keys {
        let row = option(key);
        t = match cfg_value(s, key) {
            Some(v) => spec::set_key(&t, row, &v),
            None => {
                let mut cleared = cfg::clear_key(&t, row.key);
                for alias in row.aliases {
                    cleared = cfg::clear_key(&cleared, alias);
                }
                cleared
            }
        };
    }
    t
}
/// Hardware capabilities auto-detected from the user's machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCapabilities {
    pub gpu_name: String,
    pub has_nvidia_dgpu: bool,
    pub has_amd_dgpu: bool,
    pub display_res: (u32, u32),
    pub refresh_hz: u32,
    pub is_steam_deck: bool,
}

impl MachineCapabilities {
    pub fn detect() -> Self {
        let (gpu_name, has_nvidia_dgpu, has_amd_dgpu) = detect_gpu_info();
        let (display_res, refresh_hz) = detect_display_mode();
        let is_steam_deck = display_res == (1280, 800) || is_steam_deck_dmi();
        Self {
            gpu_name,
            has_nvidia_dgpu,
            has_amd_dgpu,
            display_res,
            refresh_hz,
            is_steam_deck,
        }
    }
}

fn is_steam_deck_dmi() -> bool {
    #[cfg(target_os = "linux")]
    {
        if let Ok(board) = std::fs::read_to_string("/sys/class/dmi/id/board_name") {
            let b = board.trim();
            if b.eq_ignore_ascii_case("Jupiter") || b.eq_ignore_ascii_case("Galileo") {
                return true;
            }
        }
    }
    false
}

fn detect_gpu_info() -> (String, bool, bool) {
    #[cfg(target_os = "linux")]
    {
        let mut has_nvidia = false;
        let mut has_amd = false;
        let mut nvidia_model: Option<String> = None;

        if let Ok(entries) = std::fs::read_dir("/proc/driver/nvidia/gpus") {
            for e in entries.flatten() {
                if let Ok(info) = std::fs::read_to_string(e.path().join("information")) {
                    for line in info.lines() {
                        if let Some(rest) = line.strip_prefix("Model:") {
                            let m = rest.trim();
                            if !m.is_empty() {
                                has_nvidia = true;
                                nvidia_model = Some(if m.starts_with("NVIDIA") {
                                    m.to_string()
                                } else {
                                    format!("NVIDIA {m}")
                                });
                            }
                        }
                    }
                }
            }
        }

        if let Ok(out) = std::process::Command::new("lspci").output()
            && out.status.success()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut best_name: Option<String> = None;
            for line in text.lines() {
                let lower = line.to_ascii_lowercase();
                if !(lower.contains("vga compatible controller")
                    || lower.contains("3d controller")
                    || lower.contains("display controller"))
                {
                    continue;
                }
                if lower.contains("nvidia") {
                    has_nvidia = true;
                    if let Some(bracket) = extract_last_bracket(line) {
                        best_name = Some(format!("NVIDIA {bracket}"));
                    } else if best_name.is_none() {
                        best_name = Some("NVIDIA GeForce GPU".to_string());
                    }
                } else if lower.contains("amd") || lower.contains("ati") {
                    // Distinguish discrete Radeon RX from integrated Radeon 610M / Raphael
                    if !lower.contains("610m") && !lower.contains("raphael") {
                        has_amd = true;
                    }
                    if best_name.is_none() {
                        if let Some(bracket) = extract_last_bracket(line) {
                            best_name = Some(format!("AMD {bracket}"));
                        } else {
                            best_name = Some("AMD Radeon GPU".to_string());
                        }
                    }
                } else if lower.contains("intel") && best_name.is_none() {
                    if let Some(bracket) = extract_last_bracket(line) {
                        best_name = Some(format!("Intel {bracket}"));
                    } else {
                        best_name = Some("Intel Graphics".to_string());
                    }
                }
            }
            if let Some(nv) = nvidia_model.clone().or(best_name) {
                return (nv, has_nvidia, has_amd);
            }
        }

        if let Some(nv) = nvidia_model {
            return (nv, has_nvidia, has_amd);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some((gpu, nv, amd)) = win32_detect_gpu() {
            return (gpu, nv, amd);
        }
    }
    ("OpenGL 2.0+ GPU".to_string(), false, false)
}

#[cfg(target_os = "linux")]
fn extract_last_bracket(line: &str) -> Option<String> {
    let end = line.rfind(']')?;
    let start = line[..end].rfind('[')?;
    let inside = line[start + 1..end].trim();
    (!inside.is_empty()).then(|| inside.to_string())
}

fn detect_display_mode() -> ((u32, u32), u32) {
    #[cfg(target_os = "linux")]
    {
        if let Ok(out) = std::process::Command::new("xrandr")
            .arg("--current")
            .output()
            && out.status.success()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            let mut best_res = (1920u32, 1080u32);
            let mut best_hz = 60u32;
            let mut best_score = 0u64;

            for line in text.lines() {
                if !line.contains('*') {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let Some(res_tok) = parts.next() else {
                    continue;
                };
                let Some((w_str, h_str)) = res_tok.split_once('x') else {
                    continue;
                };
                let (Ok(w), Ok(h)) = (w_str.parse::<u32>(), h_str.parse::<u32>()) else {
                    continue;
                };
                for tok in parts {
                    if tok.contains('*') {
                        let cleaned: String = tok
                            .chars()
                            .take_while(|c| c.is_ascii_digit() || *c == '.')
                            .collect();
                        if let Ok(hz_f) = cleaned.parse::<f32>() {
                            let hz = hz_f.round() as u32;
                            let is_landscape = if w >= h { 1_000_000u64 } else { 0 };
                            let score = is_landscape + (hz as u64) * 1000 + (w as u64);
                            if score > best_score {
                                best_score = score;
                                best_res = (w, h);
                                best_hz = hz.max(30);
                            }
                        }
                    }
                }
            }
            if best_score > 0 {
                return (best_res, best_hz);
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(mode) = win32_detect_display_mode() {
            return mode;
        }
    }
    ((1920, 1080), 60)
}

/// The attached adapters' names (NVIDIA/AMD detection for the GPU choice).
#[cfg(target_os = "windows")]
fn win32_detect_gpu() -> Option<(String, bool, bool)> {
    let names: Vec<String> = rea_launcher_win32::display::adapters()
        .into_iter()
        .filter(|a| a.attached && !a.name.trim().is_empty())
        .map(|a| a.name.trim().to_string())
        .collect();
    gpu_from_names(&names)
}

/// The best GPU name among `names` and whether an NVIDIA or a discrete AMD
/// GPU is among them (an NVIDIA name wins; integrated Radeon 610M is not
/// discrete).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn gpu_from_names(names: &[String]) -> Option<(String, bool, bool)> {
    let mut has_nvidia = false;
    let mut has_amd = false;
    let mut best_gpu: Option<String> = None;
    for name in names {
        let lower = name.to_ascii_lowercase();
        if lower.contains("nvidia") {
            has_nvidia = true;
            best_gpu = Some(name.clone());
        } else if lower.contains("amd") || lower.contains("radeon") {
            if !lower.contains("610m") {
                has_amd = true;
            }
            best_gpu.get_or_insert_with(|| name.clone());
        } else {
            best_gpu.get_or_insert_with(|| name.clone());
        }
    }
    best_gpu.map(|g| (g, has_nvidia, has_amd))
}

/// The landscape display with the highest refresh rate, then the widest.
#[cfg(target_os = "windows")]
fn win32_detect_display_mode() -> Option<((u32, u32), u32)> {
    let modes: Vec<((u32, u32), u32)> = rea_launcher_win32::display::displays()
        .into_iter()
        .map(|d| ((d.monitor.width, d.monitor.height), d.refresh_hz))
        .collect();
    best_mode(&modes)
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn best_mode(modes: &[((u32, u32), u32)]) -> Option<((u32, u32), u32)> {
    modes
        .iter()
        .filter(|((w, h), _)| *w >= 640 && *h >= 480)
        .map(|&(res, hz)| (res, hz.max(60)))
        .max_by_key(|&((w, h), hz)| (w >= h, hz, w))
}

#[cfg(test)]
mod display_tests {
    use super::*;

    #[test]
    fn the_gpu_and_the_screen_are_chosen_from_the_lists() {
        let names = ["AMD Radeon 610M", "NVIDIA GeForce RTX 4090"].map(String::from);
        assert_eq!(
            gpu_from_names(&names),
            Some(("NVIDIA GeForce RTX 4090".to_string(), true, false))
        );
        let amd = ["AMD Radeon RX 7900 XTX".to_string()];
        assert_eq!(gpu_from_names(&amd).map(|g| g.2), Some(true));
        assert_eq!(gpu_from_names(&[]), None);
        // A landscape screen beats a portrait one; then refresh, then width.
        let modes = [((2160, 3840), 60), ((2560, 1440), 144), ((3840, 2160), 60)];
        assert_eq!(best_mode(&modes), Some(((2560, 1440), 144)));
        assert_eq!(best_mode(&[((1920, 1080), 0)]), Some(((1920, 1080), 60)));
        assert_eq!(best_mode(&[((320, 200), 60)]), None);
    }
}
