//! What the settings window shows and saves, kept apart from `egui` drawing
//! so it can be unit-tested on any host without a display server.

use std::path::PathBuf;

use crate::config::{self, RendererChoice, Scaler, Settings, TextureFilter};
use crate::edition::Game;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picture {
    pub renderer: RendererChoice,
    pub resolution: Option<(u32, u32)>,
    pub widescreen: bool,
    pub extended_draw_distance: bool,
    pub texture_filter: TextureFilter,
    pub fps_limit: Option<u32>,
    pub vsync: Option<bool>,
}

pub const STEAM_DECK_SCREEN: (u32, u32) = (1280, 800);

impl Picture {
    pub const ORIGINAL: Picture = Picture {
        renderer: RendererChoice::Auto,
        resolution: None,
        widescreen: false,
        extended_draw_distance: false,
        texture_filter: TextureFilter::Original,
        fps_limit: Some(30),
        vsync: None,
    };

    pub fn enhanced(_screen: Option<(u32, u32)>) -> Picture {
        Picture {
            renderer: RendererChoice::Auto,
            resolution: Some((1920, 1080)),
            widescreen: true,
            extended_draw_distance: true,
            // The games render low-resolution pixel art and pre-filtered textures.
            // Nearest-neighbour output keeps that artwork crisp and avoids putting the
            // comparatively expensive FSR shader on the default presentation path.
            texture_filter: TextureFilter::Sharp,
            fps_limit: None,
            vsync: Some(true),
        }
    }

    pub const STEAM_DECK: Picture = Picture {
        renderer: RendererChoice::Auto,
        resolution: Some(STEAM_DECK_SCREEN),
        widescreen: true,
        extended_draw_distance: true,
        texture_filter: TextureFilter::Sharp,
        fps_limit: None,
        vsync: Some(true),
    };
}

pub fn recommended(screen: Option<(u32, u32)>) -> Look {
    if screen.and_then(landscape) == Some(STEAM_DECK_SCREEN) {
        Look::SteamDeck
    } else {
        Look::Enhanced
    }
}

fn landscape((a, b): (u32, u32)) -> Option<(u32, u32)> {
    let (w, h) = (a.max(b) & !1, a.min(b) & !1);
    (w > 640 && h > 480 && w <= 7680 && h <= 4320).then_some((w, h))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Original,
    Modern,
    Custom,
}

pub const ACTIONS: [(usize, &str); 7] = [
    (0, "Accelerate"),
    (4, "Brake / Delete"),
    (5, "Gear up"),
    (3, "Gear down"),
    (2, "Change camera"),
    (1, "Back / Pause"),
    (7, "Start / Confirm"),
];

pub const PAD_BUTTONS: [(u8, &str); 10] = [
    (1, "A"),
    (2, "B"),
    (3, "X"),
    (4, "Y"),
    (5, "LB"),
    (6, "RB"),
    (7, "Back"),
    (8, "Start"),
    (9, "LT"),
    (10, "RT"),
];

pub const PAD_A: u16 = 1 << 0;
pub const PAD_B: u16 = 1 << 1;
pub const PAD_LB: u16 = 1 << 4;
pub const PAD_RB: u16 = 1 << 5;
pub const PAD_START: u16 = 1 << 7;
pub const PAD_UP: u16 = 1 << 8;
pub const PAD_DOWN: u16 = 1 << 9;
pub const PAD_LEFT: u16 = 1 << 10;
pub const PAD_RIGHT: u16 = 1 << 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadAction {
    Play,
    Up,
    Down,
    Left,
    Right,
    Press,
    NextTab,
    PreviousTab,
}

pub fn pad_actions(before: u16, now: u16) -> Vec<PadAction> {
    let pressed = now & !before;
    let mut out = Vec::new();
    for (bit, action) in [
        (PAD_START, PadAction::Play),
        (PAD_LB, PadAction::PreviousTab),
        (PAD_RB, PadAction::NextTab),
        (PAD_UP, PadAction::Up),
        (PAD_DOWN, PadAction::Down),
        (PAD_LEFT, PadAction::Left),
        (PAD_RIGHT, PadAction::Right),
        (PAD_A, PadAction::Press),
    ] {
        if pressed & bit != 0 {
            out.push(action);
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Original,
    Enhanced,
    SteamDeck,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    pub which: Game,
    pub game: String,
    pub saves: String,
    pub picture: Picture,
    pub look: Look,
    pub rumble: u32,
    pub deadzone: u32,
    pub gamepad_axes: bool,
    pub scaler: Scaler,
    pub buttons: [u8; 8],
    pub layout: Layout,
}

pub const FRAME_RATES: [(Option<u32>, &str); 3] = [
    (Some(30), "30 FPS (Original)"),
    (None, "50 FPS (Native)"),
    (Some(100), "100 FPS (Frame Gen)"),
];

pub const VSYNCS: [(Option<bool>, &str); 3] = [
    (None, "As the game"),
    (Some(true), "On"),
    (Some(false), "Off"),
];

pub const FILTERS: [(TextureFilter, &str); 4] = [
    (TextureFilter::Original, "CRT Monitor"),
    (TextureFilter::Fsr, "FSR (Super Res)"),
    (TextureFilter::Sharp, "Sharp (Pixel)"),
    (TextureFilter::Smooth, "Smooth (Bilinear)"),
];

pub const RUMBLES: [(u32, &str); 4] = [(0, "Off"), (30, "Light"), (60, "Medium"), (100, "Strong")];

pub const DEADZONES: [(u32, &str); 4] = [
    (0, "0% (Direct)"),
    (5, "5%"),
    (8, "8% (Original)"),
    (15, "15%"),
];

pub const SCALERS: [(Scaler, &str, &str); 4] = [
    (
        Scaler::Nearest,
        "Sharp",
        "Every game pixel becomes a crisp block. Best for the 1995-1997 artwork.",
    ),
    (
        Scaler::Pixel,
        "Even pixels",
        "Crisp blocks, with only their edges blended so all are the same size.",
    ),
    (
        Scaler::Fsr,
        "FSR",
        "AMD FSR: smooth, then sharpened. Softer than Sharp.",
    ),
    (
        Scaler::Linear,
        "Smooth",
        "Plain smoothing. The softest look.",
    ),
];

pub const WINDOW: (f32, f32) = (980.0, 700.0);

/// A size in egui points as physical pixels, given the display's native
/// pixels per point (1.0 when unknown or invalid).
pub fn physical_size(points: (f32, f32), native: f32) -> (u32, u32) {
    let scale = if native.is_finite() && native > 0.0 {
        native
    } else {
        1.0
    };
    (
        (points.0 * scale).round() as u32,
        (points.1 * scale).round() as u32,
    )
}

pub fn window_zoom(screen: Option<(u32, u32)>, native: f32) -> f32 {
    let native = if native > 0.0 { native } else { 1.0 };
    let Some((w, h)) = screen else {
        return 1.0;
    };
    let (w, h) = (w as f32, h as f32);
    let fit = 0.9 * (w / WINDOW.0).min(h / WINDOW.1);
    let target = (w.min(h) / 1440.0).max(1.0).min(fit);
    let zoom = (target / native).max(1.0);
    (zoom * 20.0).round() / 20.0
}

/// The race sizes offered in the Picture tab. A size saved in
/// `launcher.cfg` that is not one of them (e.g. 2560x1440) gets its own chip,
/// so it shows as selected and is kept when saving.
pub fn resolutions(
    screen: Option<(u32, u32)>,
    current: Option<(u32, u32)>,
) -> Vec<(Option<(u32, u32)>, String)> {
    let mut out = vec![
        (None, "4:3 (1280×960)".to_string()),
        (Some((1280, 720)), "720p".to_string()),
        (Some((1600, 900)), "900p".to_string()),
        (Some((1920, 1080)), "1080p".to_string()),
    ];
    if screen.and_then(landscape) == Some(STEAM_DECK_SCREEN) {
        out.push((Some(STEAM_DECK_SCREEN), "Steam Deck".to_string()));
    }
    out.push((Some(config::FULLSCREEN), "Fullscreen".to_string()));
    if let Some((w, h)) = current
        && !out.iter().any(|(r, _)| *r == current)
    {
        out.insert(out.len() - 1, (current, format!("{w}×{h}")));
    }
    out
}

impl Form {
    pub fn new(s: &Settings, screen: Option<(u32, u32)>) -> Form {
        let path = |p: &Option<PathBuf>| {
            p.as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        };
        let which = s.which_game();
        let mut renderer = s.renderer_choice();
        if !renderer.is_supported_for(which, true) {
            renderer = RendererChoice::Auto;
        }
        let resolution =
            if s.resolution.is_none() && screen.and_then(landscape) == Some(STEAM_DECK_SCREEN) {
                Some(STEAM_DECK_SCREEN)
            } else {
                s.race_resolution()
            };
        let vsync = if s.resolution == Some(config::ORIGINAL) && s.vsync.is_none() {
            None
        } else {
            Some(s.vsync.unwrap_or(true))
        };
        let picture = Picture {
            renderer,
            resolution,
            widescreen: s.widescreen_enabled(),
            extended_draw_distance: s.extended_draw_distance_enabled(),
            texture_filter: s.effective_filter(),
            fps_limit: match s.fps_cap() {
                Some(fps) if fps <= 30 => Some(30),
                Some(fps) if fps >= 60 => Some(100),
                _ => None,
            },
            vsync,
        };
        let buttons = s.buttons.unwrap_or(config::DEFAULT_BUTTONS);
        Form {
            which,
            game: path(&s.game),
            saves: path(&s.saves),
            picture,
            look: Form::look_of(&picture, screen),
            rumble: s.rumble_strength(),
            deadzone: s.deadzone.unwrap_or(0),
            gamepad_axes: s.gamepad_axes != Some(false),
            scaler: s.scaler.unwrap_or(Scaler::Fsr),
            buttons,
            layout: Form::layout_of(&buttons),
        }
    }

    pub fn set_which(&mut self, which: Game, has_dos_glide: bool, screen: Option<(u32, u32)>) {
        self.which = which;
        if !self.picture.renderer.is_supported_for(which, has_dos_glide) {
            self.picture.renderer = RendererChoice::Auto;
            self.look = Form::look_of(&self.picture, screen);
        }
    }

    pub fn layout_of(buttons: &[u8; 8]) -> Layout {
        match *buttons {
            config::ORIGINAL_BUTTONS => Layout::Original,
            config::MODERN_BUTTONS | config::LEGACY_MODERN_BUTTONS => Layout::Modern,
            _ => Layout::Custom,
        }
    }

    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
        match layout {
            Layout::Original => self.buttons = config::ORIGINAL_BUTTONS,
            Layout::Modern => self.buttons = config::MODERN_BUTTONS,
            Layout::Custom => {}
        }
    }

    pub fn pad_button(&self, k: usize) -> u8 {
        match self.buttons[k] {
            0 => k as u8 + 1,
            b => b,
        }
    }

    pub fn set_pad_button(&mut self, k: usize, button: u8) {
        if self.pad_button(k) != button {
            for i in 0..8 {
                self.buttons[i] = self.pad_button(i);
            }
            self.buttons[k] = button;
            self.layout = Form::layout_of(&self.buttons);
        }
    }

    pub fn look_of(picture: &Picture, screen: Option<(u32, u32)>) -> Look {
        if *picture == Picture::ORIGINAL {
            Look::Original
        } else if *picture == Picture::enhanced(screen) {
            Look::Enhanced
        } else if *picture == Picture::STEAM_DECK {
            Look::SteamDeck
        } else {
            Look::Custom
        }
    }

    pub fn set_look(&mut self, look: Look, screen: Option<(u32, u32)>) {
        self.look = look;
        match look {
            Look::Original => self.picture = Picture::ORIGINAL,
            Look::Enhanced => self.picture = Picture::enhanced(screen),
            Look::SteamDeck => self.picture = Picture::STEAM_DECK,
            Look::Custom => {}
        }
    }

    pub fn save(&self, text: &str) -> String {
        let p = &self.picture;
        let mut t = text.to_string();
        let mut put = |key: &str, value: Option<String>| {
            t = match &value {
                Some(v) => config::set_key(&t, key, v),
                None => config::clear_key(&t, key),
            };
        };
        let path = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
        put("which", Some(self.which.slug().into()));
        put("game", path(&self.game));
        put("saves", path(&self.saves));
        put("renderer", Some(p.renderer.key().into()));
        put(
            "resolution",
            Some(match p.resolution {
                None => "original".to_string(),
                Some((w, h)) if (w, h) == config::FULLSCREEN => "fullscreen".to_string(),
                Some((w, h)) => format!("{w}x{h}"),
            }),
        );
        put("widescreen", Some(p.widescreen.to_string()));
        put(
            "extended_draw_distance",
            Some(p.extended_draw_distance.to_string()),
        );
        put(
            "fps_limit",
            Some(
                p.fps_limit
                    .map(|f| f.to_string())
                    .unwrap_or_else(|| "50".to_string()),
            ),
        );
        put("vsync", p.vsync.map(|v| v.to_string()));
        put(
            "texture_filter",
            Some(
                match p.texture_filter {
                    TextureFilter::Original => "original",
                    TextureFilter::Fsr => "fsr",
                    TextureFilter::Smooth => "smooth",
                    TextureFilter::Sharp => "sharp",
                }
                .into(),
            ),
        );
        put("rumble", Some(self.rumble.to_string()));
        put("deadzone", Some(self.deadzone.to_string()));
        put("gamepad_axes", Some(self.gamepad_axes.to_string()));
        put("scaler", Some(self.scaler.key().into()));
        put(
            "buttons",
            (self.buttons != config::DEFAULT_BUTTONS).then(|| config::buttons_value(&self.buttons)),
        );
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Option<(u32, u32)> = Some((2560, 1440));

    fn form(text: &str) -> Form {
        Form::new(&Settings::parse_file(text).0, SCREEN)
    }

    #[test]
    fn default_template_starts_as_smart_enhanced_look() {
        let f = form(config::TEMPLATE);
        assert_eq!(f.which, Game::Screamer);
        assert_eq!(f.look, Look::Enhanced);
        assert_eq!(f.picture, Picture::enhanced(SCREEN));
        assert!(f.gamepad_axes);
        assert_eq!(f.rumble, 60);

        // Steam Deck screen auto-selects Look::SteamDeck when resolution is unset
        let deck_f = Form::new(&Settings::default(), Some(STEAM_DECK_SCREEN));
        assert_eq!(deck_f.look, Look::SteamDeck);
        assert_eq!(deck_f.picture, Picture::STEAM_DECK);
    }

    #[test]
    fn enhanced_and_original_looks_roundtrip_through_save() {
        let mut f = form(config::TEMPLATE);
        f.set_which(Game::ScreamerRally, true, SCREEN);
        f.set_look(Look::Original, SCREEN);
        let saved_orig = f.save(config::TEMPLATE);
        let f_orig = form(&saved_orig);
        assert_eq!(f_orig.look, Look::Original);
        assert_eq!(f_orig.picture, Picture::ORIGINAL);
        assert!(!f_orig.picture.extended_draw_distance);

        f.set_look(Look::Enhanced, SCREEN);
        let saved = f.save(&saved_orig);
        let f2 = form(&saved);
        assert_eq!(f2.which, Game::ScreamerRally);
        assert_eq!(f2.look, Look::Enhanced);
        assert_eq!(f2.picture.resolution, Some((1920, 1080)));
        assert!(f2.picture.widescreen);
        assert!(f2.picture.extended_draw_distance);
        assert_eq!(f2.picture.texture_filter, TextureFilter::Sharp);
        assert_eq!(f2.picture.fps_limit, None);
    }

    #[test]
    fn switching_game_resets_unsupported_renderer() {
        let mut f = form(config::TEMPLATE);
        f.set_which(Game::ScreamerRally, true, SCREEN);
        f.picture.renderer = RendererChoice::Voodoo;
        assert_eq!(f.picture.renderer, RendererChoice::Voodoo);

        // Switching to Screamer (1995), which does not have 3dfx Voodoo, resets renderer to Auto
        f.set_which(Game::Screamer, true, SCREEN);
        assert_eq!(f.picture.renderer, RendererChoice::Auto);
    }

    #[test]
    fn pad_actions_detect_rising_edges_and_modern_triggers() {
        assert_eq!(pad_actions(0, PAD_START), vec![PadAction::Play]);
        assert_eq!(pad_actions(PAD_START, PAD_START), Vec::<PadAction>::new());
        assert_eq!(
            pad_actions(0, PAD_RB | PAD_DOWN),
            vec![PadAction::NextTab, PadAction::Down]
        );
        let f = form(config::TEMPLATE);
        assert_eq!(f.layout, Layout::Modern);
        assert_eq!(f.buttons[0], 10); // Accelerate = RT
        assert_eq!(f.buttons[4], 9); // Brake = LT
    }

    #[test]
    fn a_saved_custom_size_gets_its_own_chip() {
        let plain = resolutions(None, Some((1920, 1080)));
        assert_eq!(plain.len(), 5);
        let custom = resolutions(None, Some((2560, 1440)));
        assert_eq!(custom.len(), 6);
        assert_eq!(custom[4], (Some((2560, 1440)), "2560×1440".to_string()));
        assert_eq!(custom[5].0, Some(config::FULLSCREEN));
        assert_eq!(resolutions(None, None).len(), 5);
    }

    #[test]
    fn monitor_points_become_pixels() {
        // A Steam Deck at 1.25x scaling reports 1024x640 points.
        assert_eq!(physical_size((1024.0, 640.0), 1.25), STEAM_DECK_SCREEN);
        assert_eq!(physical_size((1920.0, 1080.0), 2.0), (3840, 2160));
        assert_eq!(physical_size((1920.0, 1080.0), 0.0), (1920, 1080));
        assert_eq!(physical_size((1366.4, 768.0), 1.0), (1366, 768));
    }
}
