//! Command line and the settings file (`launcher.cfg`).
//!
//! Plain `key = value` lines with `#` comments, written once from a commented
//! template so users can edit it. Unknown keys are ignored so older launchers
//! can read newer files; values it cannot read are reported and ignored.

use std::path::PathBuf;

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
    pub fps_limit: Option<u32>,
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
    /// grays out the chip and explains why on hover).
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

fn parse_filter(v: &str) -> Option<TextureFilter> {
    match v.trim().to_ascii_lowercase().as_str() {
        "original" => Some(TextureFilter::Original),
        "fsr" => Some(TextureFilter::Fsr),
        "smooth" => Some(TextureFilter::Smooth),
        "sharp" => Some(TextureFilter::Sharp),
        _ => None,
    }
}

pub const FILE_NAME: &str = "launcher.cfg";

pub const TEMPLATE: &str = r"# Screamer Trilogy (Screamer / Screamer 2 / Screamer Rally) launcher settings.
#
# One setting per line as key = value; lines starting with # are comments.
# Command-line options (--game, --which, --runtime, --saves, etc.) override this
# file for one run.

# Which game in the trilogy to launch: screamer, screamer2, or screamer-rally.
which = screamer

# The folder with your installed Screamer game(s) — either a parent trilogy
# folder containing screamer/, screamer2/, screamer-rally/ or a single game folder.
# The game folder is only read, never modified.
game =

# Where the launcher keeps its private, writable runtime copy of the game.
#runtime =

# Where your saves (*.SAV, CHOICE.DAT, REPLAYS/*.RPL) are kept.
#saves =

# Optional path to a custom dosbox-staging binary.
#dosbox =

# Graphics renderer: auto, 3dfx, svga16, svga8, or vga.
renderer = auto

# Display resolution, e.g. 1920x1080, fullscreen, or original.
resolution = 1920x1080

# True 16:9 Hor+ widescreen 3D camera projection (expands horizontal FOV by
# 4/3 inside SetCamera3D instead of stretching the 4:3 image).
widescreen = true

# Reduce scenery/object pop-in. Enhanced modes extend Screamer's managed
# animations by 20% and double Screamer 2/Rally's admission radius.
extended_draw_distance = true

# Frame-rate target: 30 is the unchanged original timing, 50 is the smooth native
# enhanced timing, and 100 enables experimental 2x GPU Optical-Flow Frame Generation.
fps_limit = 50

# Wait for vertical refresh before presenting each frame.
vsync = true

# Shader / Upscaler: sharp (nearest-neighbour pixel art), smooth (bilinear),
# fsr (AMD FSR 1.0 EASU+RCAS), or original (CRT).
texture_filter = sharp

# Modern gamepad analog steering & independent L2/R2 analog triggers.
gamepad_axes = true

# Gamepad steering deadzone in percent (0 removes the hardcoded 7.8% deadzone).
deadzone = 0

# Gamepad rumble strength on kerbs, gravel, skids, and wall impacts (0-100).
rumble = 60

# Pad buttons: modern, original, or 8 comma-separated button numbers.
buttons = modern

# Linux (screamer-play) only: gamescope upscaling filter (nearest, pixel, fsr, linear).
scaler = fsr
";

pub const USAGE: &str = concat!(
    "ScreamerLauncher ",
    env!("CARGO_PKG_VERSION"),
    "\n\n",
    "\
ScreamerLauncher [1|2|rally] [options]

  --which <1|2|rally>      select game: screamer (1), screamer2 (2), or screamer-rally (rally)
  --game <folder>          the trilogy root folder or a single game folder (remembered)
  --saves <folder>         where your saves are kept
  --runtime <folder>       where the launcher keeps its private runtime copy
  --dosbox <path>          explicit path to dosbox-staging executable
  --renderer <mode>        auto, 3dfx, svga16, svga8, or vga
  --resolution <WxH>       output resolution, e.g. 3840x2160, or original
  --widescreen on|off      Hor+ 16:9 widescreen 3D camera projection
  --extended-draw-distance on|off  reduce scenery/object pop-in
  --fps-limit <n>|off      30 original, 50 native, or 100 frame generation
  --vsync on|off           wait for the screen's refresh
  --texture-filter <f>     original, fsr, smooth, or sharp
  --gamepad-axes on|off    proportional analog steering & L2/R2 triggers
  --deadzone <0-30>        analog steering deadzone percent (0 removes 7.8% deadzone)
  --rumble <0-100>|off     gamepad rumble strength
  --buttons <layout>       pad buttons: original, modern, or 8 numbers

These override launcher.cfg for one run (--game and --which are remembered).
Without options the settings window opens first.

  --no-window              play with the saved settings, no window
  --settings [--linux]     only the settings window (exit 0: play, 3: closed)

ScreamerLauncher --check [--game <folder>] [--which <1|2|rally>]
  checks all discovered games in the trilogy folder and verifies all binary fixes

ScreamerLauncher --controllers [seconds]
  lists connected gamepads and logs axis/button events"
);

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

pub const ORIGINAL: (u32, u32) = (0, 0);
pub const FULLSCREEN: (u32, u32) = (1, 1);

fn parse_resolution(v: &str) -> Option<(u32, u32)> {
    let lower = v.trim().to_ascii_lowercase();
    if lower == "original" || lower == "off" {
        return Some(ORIGINAL);
    }
    if lower == "fullscreen" || lower == "full" || lower == "desktop" {
        return Some(FULLSCREEN);
    }
    let (w, h) = lower.split_once('x')?;
    let w: u32 = w.trim().parse().ok()?;
    let h: u32 = h.trim().parse().ok()?;
    if !(320..=7680).contains(&w) || !(200..=4320).contains(&h) {
        return None;
    }
    if w > 1920 || h > 1080 {
        return Some(FULLSCREEN);
    }
    Some((w, h))
}

fn parse_percent(v: &str) -> Option<u32> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("off") {
        return Some(0);
    }
    v.parse().ok().filter(|p| *p <= 100)
}

fn parse_fps(v: &str) -> Option<u32> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("off") || v == "0" {
        return Some(0);
    }
    let f: u32 = v.parse().ok()?;
    (10..=1000).contains(&f).then_some(f)
}

fn entry(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.starts_with('#') {
        return None;
    }
    let (key, value) = line.split_once('=')?;
    let value = value.trim();
    (!value.is_empty()).then_some((key.trim(), value))
}

impl Settings {
    pub fn parse_file(text: &str) -> (Settings, Vec<String>) {
        let mut s = Settings::default();
        let mut warnings = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let Some((key, value)) = entry(line) else {
                continue;
            };
            let mut bad = |what: &str| {
                warnings.push(format!("{FILE_NAME} line {}: {what} {value:?}", n + 1));
            };
            match key {
                "which" => match Game::parse(value) {
                    Some(g) => s.which = Some(g),
                    None => bad("which needs screamer, screamer2, or screamer-rally, not"),
                },
                "game" => s.game = Some(value.into()),
                "runtime" => s.runtime = Some(value.into()),
                "saves" => s.saves = Some(value.into()),
                "dosbox" => s.dosbox = Some(value.into()),
                "renderer" => match RendererChoice::parse(value) {
                    Some(r) => s.renderer = Some(r),
                    None => bad("renderer needs auto, 3dfx, svga16, svga8, or vga, not"),
                },
                "resolution" => match parse_resolution(value) {
                    Some(r) => s.resolution = Some(r),
                    None => bad("resolution needs WIDTHxHEIGHT (320x200 to 7680x4320), not"),
                },
                "widescreen" => match parse_bool(value) {
                    Some(b) => s.widescreen = Some(b),
                    None => bad("widescreen needs true or false, not"),
                },
                "extended_draw_distance" => match parse_bool(value) {
                    Some(b) => s.extended_draw_distance = Some(b),
                    None => bad("extended_draw_distance needs true or false, not"),
                },
                "fps_limit" => match parse_fps(value) {
                    Some(f) => s.fps_limit = Some(f),
                    None => bad("fps_limit needs a number from 10 to 1000 or off, not"),
                },
                "vsync" => match parse_bool(value) {
                    Some(b) => s.vsync = Some(b),
                    None => bad("vsync needs true or false, not"),
                },
                "texture_filter" => match parse_filter(value) {
                    Some(f) => s.texture_filter = Some(f),
                    None => bad("texture_filter needs original, smooth, or sharp, not"),
                },
                "gamepad_axes" => match parse_bool(value) {
                    Some(b) => s.gamepad_axes = Some(b),
                    None => bad("gamepad_axes needs true or false, not"),
                },
                "deadzone" => match parse_percent(value) {
                    Some(d) if d <= 30 => s.deadzone = Some(d),
                    _ => bad("deadzone needs a number from 0 to 30, not"),
                },
                "rumble" => match parse_percent(value) {
                    Some(r) => s.rumble = Some(r),
                    None => bad("rumble needs a number from 0 to 100 or off, not"),
                },
                "buttons" => match parse_buttons(value) {
                    Some(b) => s.buttons = Some(b),
                    None => bad("buttons needs original, modern, or 8 button numbers, not"),
                },
                "scaler" => match Scaler::parse(value) {
                    Some(f) => s.scaler = Some(f),
                    None => bad("scaler needs nearest, pixel, fsr, or linear, not"),
                },
                _ => {}
            }
        }
        (s, warnings)
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
            fps_limit: over.fps_limit.or(self.fps_limit),
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

    pub fn fps_cap(&self) -> Option<u32> {
        match self.fps_limit {
            Some(0) => None,
            Some(f) => Some(f),
            None => Some(50),
        }
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

    /// Optional binary fixes (`E1`, `E2`, `E5`, `E6`) that these settings turn off.
    pub fn skipped_fixes(&self) -> Vec<&'static str> {
        let mut skip = Vec::new();
        // E1 unlocks the 30 FPS PIT timer wait loop (`fdiv 2.0 -> 1.0`).
        // E1 is required for both native 50 Hz and optional 100 FPS presentation.
        // An explicit 30 (or legacy `off`/0) selects unchanged original timing.
        if self.fps_cap().unwrap_or(30) <= 30 {
            skip.push("E1");
        }
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

    pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Settings, String> {
        let mut s = Settings::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .ok_or_else(|| format!("{name} needs a value\n\n{USAGE}"))
            };
            match arg.as_str() {
                flag @ ("--which" | "--play") => {
                    let v = value(flag)?;
                    s.which = Some(Game::parse(&v).ok_or_else(|| {
                        format!(
                            "{flag} needs 1 (screamer), 2 (screamer2), or rally (screamer-rally), not {v}\n\n{USAGE}"
                        )
                    })?);
                }
                "--game" => s.game = Some(value("--game")?.into()),
                "--runtime" => s.runtime = Some(value("--runtime")?.into()),
                "--saves" => s.saves = Some(value("--saves")?.into()),
                "--dosbox" => s.dosbox = Some(value("--dosbox")?.into()),
                "--renderer" => {
                    let v = value("--renderer")?;
                    s.renderer = Some(RendererChoice::parse(&v).ok_or_else(|| {
                        format!(
                            "--renderer needs auto, 3dfx, svga16, svga8, or vga, not {v}\n\n{USAGE}"
                        )
                    })?);
                }
                "--resolution" => {
                    let v = value("--resolution")?;
                    s.resolution = Some(parse_resolution(&v).ok_or_else(|| {
                        format!("--resolution needs WIDTHxHEIGHT or original, not {v}\n\n{USAGE}")
                    })?);
                }
                "--buttons" => {
                    let v = value("--buttons")?;
                    s.buttons = Some(parse_buttons(&v).ok_or_else(|| {
                        format!(
                            "--buttons needs original, modern, or 8 button numbers, not {v}\n\n{USAGE}"
                        )
                    })?);
                }
                "--rumble" => {
                    let v = value("--rumble")?;
                    s.rumble = Some(parse_percent(&v).ok_or_else(|| {
                        format!("--rumble needs 0 to 100 or off, not {v}\n\n{USAGE}")
                    })?);
                }
                "--deadzone" => {
                    let v = value("--deadzone")?;
                    let d = parse_percent(&v)
                        .filter(|&d| d <= 30)
                        .ok_or_else(|| format!("--deadzone needs 0 to 30, not {v}\n\n{USAGE}"))?;
                    s.deadzone = Some(d);
                }
                "--texture-filter" => {
                    let v = value("--texture-filter")?;
                    s.texture_filter = Some(parse_filter(&v).ok_or_else(|| {
                        format!(
                            "--texture-filter needs original, fsr, smooth, or sharp, not {v}\n\n{USAGE}"
                        )
                    })?);
                }
                "--fps-limit" => {
                    let v = value("--fps-limit")?;
                    s.fps_limit = Some(parse_fps(&v).ok_or_else(|| {
                        format!("--fps-limit needs 10 to 1000 or off, not {v}\n\n{USAGE}")
                    })?);
                }
                flag @ ("--widescreen"
                | "--extended-draw-distance"
                | "--gamepad-axes"
                | "--vsync") => {
                    let v = value(flag)?;
                    let b = parse_bool(&v)
                        .ok_or_else(|| format!("{flag} needs on or off, not {v}\n\n{USAGE}"))?;
                    match flag {
                        "--widescreen" => s.widescreen = Some(b),
                        "--extended-draw-distance" => s.extended_draw_distance = Some(b),
                        "--gamepad-axes" => s.gamepad_axes = Some(b),
                        _ => s.vsync = Some(b),
                    }
                }
                "--help" | "-h" | "/?" => return Err(USAGE.into()),
                other => {
                    if let Some(g) = Game::parse(other) {
                        s.which = Some(g);
                    } else {
                        return Err(format!("unknown argument {other}\n\n{USAGE}"));
                    }
                }
            }
        }
        Ok(s)
    }
}

pub fn set_key(text: &str, key: &str, value: &str) -> String {
    let is_key = |line: &str| {
        let line = line.trim();
        !line.starts_with('#') && line.split_once('=').is_some_and(|(k, _)| k.trim() == key)
    };
    let mut out = String::with_capacity(text.len() + key.len() + value.len() + 4);
    let mut done = false;
    for line in text.lines() {
        if !done && is_key(line) {
            out.push_str(&format!("{key} = {value}"));
            done = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if !done {
        out.push_str(&format!("{key} = {value}\n"));
    }
    out
}

pub fn clear_key(text: &str, key: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for line in text.lines() {
        let t = line.trim();
        if !t.starts_with('#') && t.split_once('=').is_some_and(|(k, _)| k.trim() == key) {
            out.push_str(&format!("#{key} ="));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skipped_fixes_follow_settings() {
        // Smart defaults enable E1, E2, E5, and E6 out of the box.
        let def = Settings::default();
        assert!(def.skipped_fixes().is_empty());

        let enhanced = Settings {
            fps_limit: Some(100),
            widescreen: Some(true),
            gamepad_axes: Some(true),
            deadzone: Some(0),
            ..Settings::default()
        };
        assert!(enhanced.skipped_fixes().is_empty());

        let orig = Settings {
            fps_limit: Some(0),
            widescreen: Some(false),
            gamepad_axes: Some(false),
            extended_draw_distance: Some(false),
            ..Settings::default()
        };
        assert_eq!(orig.skipped_fixes(), vec!["E1", "E2", "E5", "E6"]);
    }

    #[test]
    fn parses_file_and_args() {
        let (s, warn) = Settings::parse_file(
            "game = /games/screamer\nrenderer = 3dfx\nresolution = 1920x1080\nwidescreen = true\nextended_draw_distance = false\nfps_limit = 60\n",
        );
        assert!(warn.is_empty());
        assert_eq!(s.renderer_choice(), RendererChoice::Voodoo);
        assert_eq!(s.resolution, Some((1920, 1080)));
        assert_eq!(s.widescreen, Some(true));
        assert_eq!(s.extended_draw_distance, Some(false));
        assert_eq!(s.fps_limit, Some(60));

        // Legacy 4K/1440p values map cleanly to FULLSCREEN ("desktop")
        let (s_legacy, _) = Settings::parse_file("resolution = 3840x2160\n");
        assert_eq!(s_legacy.resolution, Some(FULLSCREEN));

        let args =
            Settings::parse_args(["--extended-draw-distance".to_string(), "off".to_string()])
                .unwrap();
        assert_eq!(args.extended_draw_distance, Some(false));
    }
}

impl Settings {
    pub fn renderer_choice(&self) -> RendererChoice {
        self.renderer.unwrap_or(RendererChoice::Auto)
    }
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

    pub fn summary_line(&self, is_3dfx: bool) -> String {
        let voodoo_str = if is_3dfx {
            "2× Voodoo (1280×960) + "
        } else {
            ""
        };
        format!(
            "Auto-detected: {} · {}×{} @ {} Hz — {}FSR Super Res + 100 FPS Optical-Flow Frame Gen ready",
            self.gpu_name, self.display_res.0, self.display_res.1, self.refresh_hz, voodoo_str
        )
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

#[cfg(target_os = "windows")]
#[repr(C)]
struct DisplayDeviceW {
    cb: u32,
    device_name: [u16; 32],
    device_string: [u16; 128],
    state_flags: u32,
    device_id: [u16; 128],
    device_key: [u16; 128],
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct DevModeW {
    dm_device_name: [u16; 32],
    dm_spec_version: u16,
    dm_driver_version: u16,
    dm_size: u16,
    dm_driver_extra: u16,
    dm_fields: u32,
    dm_position_x: i32,
    dm_position_y: i32,
    dm_display_orientation: u32,
    dm_display_fixed_output: u32,
    dm_color: i16,
    dm_duplex: i16,
    dm_y_resolution: i16,
    dm_tt_option: i16,
    dm_collate: i16,
    dm_form_name: [u16; 32],
    dm_log_pixels: u16,
    dm_bits_per_pel: u32,
    dm_pels_width: u32,
    dm_pels_height: u32,
    dm_display_flags: u32,
    dm_display_frequency: u32,
    dm_padding: [u8; 64],
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn EnumDisplayDevicesW(
        lp_device: *const u16,
        i_dev_num: u32,
        lp_display_device: *mut DisplayDeviceW,
        dw_flags: u32,
    ) -> i32;
    fn EnumDisplaySettingsW(
        lpsz_device_name: *const u16,
        i_mode_num: u32,
        lp_dev_mode: *mut DevModeW,
    ) -> i32;
}

#[cfg(target_os = "windows")]
fn utf16_z_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end]).trim().to_string()
}

#[cfg(target_os = "windows")]
fn win32_detect_gpu() -> Option<(String, bool, bool)> {
    const DISPLAY_DEVICE_ATTACHED_TO_DESKTOP: u32 = 0x00000001;
    let mut has_nvidia = false;
    let mut has_amd = false;
    let mut best_gpu: Option<String> = None;

    for idx in 0..16u32 {
        let mut dev: DisplayDeviceW = unsafe { std::mem::zeroed() };
        dev.cb = std::mem::size_of::<DisplayDeviceW>() as u32;
        if unsafe { EnumDisplayDevicesW(std::ptr::null(), idx, &mut dev, 0) } == 0 {
            break;
        }
        if dev.state_flags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP == 0 {
            continue;
        }
        let name = utf16_z_to_string(&dev.device_string);
        if name.is_empty() {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if lower.contains("nvidia") {
            has_nvidia = true;
            best_gpu = Some(name);
        } else if lower.contains("amd") || lower.contains("radeon") {
            if !lower.contains("610m") {
                has_amd = true;
            }
            if best_gpu.is_none() {
                best_gpu = Some(name);
            }
        } else if best_gpu.is_none() {
            best_gpu = Some(name);
        }
    }
    best_gpu.map(|g| (g, has_nvidia, has_amd))
}

#[cfg(target_os = "windows")]
fn win32_detect_display_mode() -> Option<((u32, u32), u32)> {
    const DISPLAY_DEVICE_ATTACHED_TO_DESKTOP: u32 = 0x00000001;
    const ENUM_CURRENT_SETTINGS: u32 = u32::MAX;
    let mut best_res = (1920u32, 1080u32);
    let mut best_hz = 60u32;
    let mut best_score = 0u64;

    for idx in 0..16u32 {
        let mut dev: DisplayDeviceW = unsafe { std::mem::zeroed() };
        dev.cb = std::mem::size_of::<DisplayDeviceW>() as u32;
        if unsafe { EnumDisplayDevicesW(std::ptr::null(), idx, &mut dev, 0) } == 0 {
            break;
        }
        if dev.state_flags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP == 0 {
            continue;
        }
        let mut dm: DevModeW = unsafe { std::mem::zeroed() };
        dm.dm_size = std::mem::size_of::<DevModeW>() as u16;
        if unsafe { EnumDisplaySettingsW(dev.device_name.as_ptr(), ENUM_CURRENT_SETTINGS, &mut dm) }
            != 0
        {
            let w = dm.dm_pels_width;
            let h = dm.dm_pels_height;
            let hz = dm.dm_display_frequency.max(60);
            if w >= 640 && h >= 480 {
                let is_landscape = if w >= h { 1_000_000u64 } else { 0 };
                let score = is_landscape + (hz as u64) * 1000 + (w as u64);
                if score > best_score {
                    best_score = score;
                    best_res = (w, h);
                    best_hz = hz;
                }
            }
        }
    }
    (best_score > 0).then_some((best_res, best_hz))
}
