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
/// 1: Select/Accelerate (A), 2: Back/Brake (B), 3: View (Y), 4: Gear Down (LB),
/// 5: Handbrake/Alt (X), 6: Gear Up (RB), 7: Back/Select, 8: Pause (Start).
pub const MODERN_BUTTONS: [u8; 8] = [1, 2, 4, 5, 3, 6, 7, 8];

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
        MODERN_BUTTONS => "modern".into(),
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

/// `texture_filter`: original, smooth (bilinear), or sharp (nearest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilter {
    Original,
    Smooth,
    Sharp,
}

fn parse_filter(v: &str) -> Option<TextureFilter> {
    match v.trim().to_ascii_lowercase().as_str() {
        "original" => Some(TextureFilter::Original),
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

# Display resolution, e.g. 1920x1080, 2560x1440, 3840x2160, or original.
#resolution = 1920x1080

# True 16:9 Hor+ widescreen 3D camera projection (expands horizontal FOV by
# 4/3 inside SetCamera3D instead of stretching the 4:3 image).
#widescreen = true

# Frame-rate cap in frames per second (e.g. 60 unlocks the 30 FPS PIT timer
# lock while keeping car physics at 1.0x speed); off or unset keeps 30 FPS.
#fps_limit = 60

# Wait for vertical refresh before presenting each frame.
#vsync = true

# Texture filtering: original, smooth (bilinear), or sharp (nearest-neighbor).
#texture_filter = smooth

# Modern gamepad analog steering & independent L2/R2 analog triggers.
gamepad_axes = true

# Gamepad steering deadzone in percent (0 removes the hardcoded 7.8% deadzone).
#deadzone = 0

# Gamepad rumble strength on kerbs, gravel, skids, and wall impacts (0-100).
#rumble = 60

# Pad buttons: modern, original, or 8 comma-separated button numbers.
#buttons = modern

# Linux (screamer-play) only: gamescope upscaling filter (nearest, pixel, fsr, linear).
#scaler = nearest
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
  --fps-limit <n>|off      frame-rate cap, e.g. 60 (unlocks 30 FPS lock)
  --vsync on|off           wait for the screen's refresh
  --texture-filter <f>     original, smooth, or sharp
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
        self.rumble.unwrap_or(0).min(100)
    }

    pub fn fps_cap(&self) -> Option<u32> {
        self.fps_limit.filter(|&f| f > 0)
    }

    pub fn race_resolution(&self) -> Option<(u32, u32)> {
        self.resolution.filter(|&r| r != ORIGINAL)
    }

    pub fn button_map(&self) -> [u8; 8] {
        self.buttons.unwrap_or(DEFAULT_BUTTONS)
    }

    /// Optional binary fixes (`E1`, `E2`, `E5`) that these settings turn off.
    pub fn skipped_fixes(&self) -> Vec<&'static str> {
        let mut skip = Vec::new();
        // E1 unlocks the 30 FPS PIT timer wait loop (`fdiv 2.0 -> 1.0`).
        // Only enable E1 when fps_limit > 30.
        if self.fps_cap().unwrap_or(30) <= 30 {
            skip.push("E1");
        }
        // E2 scales horizontal camera basis M00, M01, M02 by 3/4 for 16:9 Hor+.
        if self.widescreen != Some(true) {
            skip.push("E2");
        }
        // E5 zeroes the hardcoded 20/256 (7.8%) analog steering deadzone.
        if self.gamepad_axes == Some(false) || self.deadzone.unwrap_or(0) > 0 {
            skip.push("E5");
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
                            "--texture-filter needs original, smooth, or sharp, not {v}\n\n{USAGE}"
                        )
                    })?);
                }
                "--fps-limit" => {
                    let v = value("--fps-limit")?;
                    s.fps_limit = Some(parse_fps(&v).ok_or_else(|| {
                        format!("--fps-limit needs 10 to 1000 or off, not {v}\n\n{USAGE}")
                    })?);
                }
                flag @ ("--widescreen" | "--gamepad-axes" | "--vsync") => {
                    let v = value(flag)?;
                    let b = parse_bool(&v)
                        .ok_or_else(|| format!("{flag} needs on or off, not {v}\n\n{USAGE}"))?;
                    match flag {
                        "--widescreen" => s.widescreen = Some(b),
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
        let def = Settings::default();
        assert_eq!(def.skipped_fixes(), vec!["E1", "E2"]);

        let enhanced = Settings {
            fps_limit: Some(60),
            widescreen: Some(true),
            gamepad_axes: Some(true),
            deadzone: Some(0),
            ..Settings::default()
        };
        assert!(enhanced.skipped_fixes().is_empty());

        let orig = Settings {
            fps_limit: Some(30),
            widescreen: Some(false),
            gamepad_axes: Some(false),
            ..Settings::default()
        };
        assert_eq!(orig.skipped_fixes(), vec!["E1", "E2", "E5"]);
    }

    #[test]
    fn parses_file_and_args() {
        let (s, warn) = Settings::parse_file(
            "game = /games/screamer\nrenderer = 3dfx\nresolution = 1920x1080\nwidescreen = true\nfps_limit = 60\n",
        );
        assert!(warn.is_empty());
        assert_eq!(s.renderer_choice(), RendererChoice::Voodoo);
        assert_eq!(s.resolution, Some((1920, 1080)));
        assert_eq!(s.widescreen, Some(true));
        assert_eq!(s.fps_limit, Some(60));

        // Legacy 4K/1440p values map cleanly to FULLSCREEN ("desktop")
        let (s_legacy, _) = Settings::parse_file("resolution = 3840x2160\n");
        assert_eq!(s_legacy.resolution, Some(FULLSCREEN));
    }
}

impl Settings {
    pub fn renderer_choice(&self) -> RendererChoice {
        self.renderer.unwrap_or(RendererChoice::Auto)
    }
}
