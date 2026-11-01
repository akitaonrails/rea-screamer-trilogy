//! What the settings window shows and saves, kept apart from `egui` drawing
//! so it can be unit-tested on any host without a display server.

use std::path::PathBuf;

use crate::config::{self, RendererChoice, Scaler, Settings, TextureFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picture {
    pub renderer: RendererChoice,
    pub resolution: Option<(u32, u32)>,
    pub widescreen: bool,
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
        texture_filter: TextureFilter::Original,
        fps_limit: None,
        vsync: None,
    };

    pub fn enhanced(screen: Option<(u32, u32)>) -> Picture {
        let resolution = screen.and_then(landscape).or(Some((1920, 1080)));
        Picture {
            renderer: RendererChoice::Auto,
            resolution,
            widescreen: resolution.is_some_and(|(w, h)| w * 3 > h * 4),
            texture_filter: TextureFilter::Smooth,
            fps_limit: Some(60),
            vsync: Some(true),
        }
    }

    pub const STEAM_DECK: Picture = Picture {
        renderer: RendererChoice::Auto,
        resolution: Some(STEAM_DECK_SCREEN),
        widescreen: true,
        texture_filter: TextureFilter::Smooth,
        fps_limit: Some(60),
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
    (4, "Handbrake / Alt"),
    (5, "Gear up"),
    (3, "Gear down"),
    (2, "Change view"),
    (7, "Pause"),
    (0, "Select (menus)"),
    (1, "Back (menus)"),
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
    (9, "LS"),
    (10, "RS"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Original,
    Enhanced,
    SteamDeck,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
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

pub const FRAME_RATES: [(Option<u32>, &str); 4] = [
    (None, "Original (30)"),
    (Some(60), "60 FPS"),
    (Some(120), "120 FPS"),
    (Some(144), "144 FPS"),
];

pub const VSYNCS: [(Option<bool>, &str); 3] = [
    (None, "As the game"),
    (Some(true), "On"),
    (Some(false), "Off"),
];

pub const FILTERS: [(TextureFilter, &str); 3] = [
    (TextureFilter::Original, "Original"),
    (TextureFilter::Smooth, "Smooth"),
    (TextureFilter::Sharp, "Sharp"),
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

pub const WINDOW: (f32, f32) = (880.0, 640.0);

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

pub fn resolutions(screen: Option<(u32, u32)>) -> Vec<(Option<(u32, u32)>, String)> {
    let common = [(1920, 1080), (2560, 1440), (3840, 2160)];
    let mut out = vec![(None, "Original".to_string())];
    if let Some(s) = screen.and_then(landscape)
        && !common.contains(&s)
    {
        out.push((Some(s), format!("Screen {}×{}", s.0, s.1)));
    }
    for (w, h) in common {
        let label = match h {
            1080 => "1080p",
            1440 => "1440p",
            _ => "4K",
        };
        out.push((Some((w, h)), label.to_string()));
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
        let picture = Picture {
            renderer: s.renderer_choice(),
            resolution: s.race_resolution(),
            widescreen: s.widescreen == Some(true),
            texture_filter: s.texture_filter.unwrap_or(TextureFilter::Original),
            fps_limit: s.fps_cap(),
            vsync: s.vsync,
        };
        let buttons = s.buttons.unwrap_or(config::DEFAULT_BUTTONS);
        Form {
            game: path(&s.game),
            saves: path(&s.saves),
            picture,
            look: Form::look_of(&picture, screen),
            rumble: s.rumble_strength(),
            deadzone: s.deadzone.unwrap_or(0),
            gamepad_axes: s.gamepad_axes != Some(false),
            scaler: s.scaler.unwrap_or(Scaler::Nearest),
            buttons,
            layout: Form::layout_of(&buttons),
        }
    }

    pub fn layout_of(buttons: &[u8; 8]) -> Layout {
        match *buttons {
            config::ORIGINAL_BUTTONS => Layout::Original,
            config::MODERN_BUTTONS => Layout::Modern,
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
        put("game", path(&self.game));
        put("saves", path(&self.saves));
        put("renderer", Some(p.renderer.key().into()));
        put("resolution", p.resolution.map(|(w, h)| format!("{w}x{h}")));
        put("widescreen", p.widescreen.then(|| "true".into()));
        put("fps_limit", p.fps_limit.map(|f| f.to_string()));
        put("vsync", p.vsync.map(|v| v.to_string()));
        put(
            "texture_filter",
            match p.texture_filter {
                TextureFilter::Original => None,
                TextureFilter::Smooth => Some("smooth".into()),
                TextureFilter::Sharp => Some("sharp".into()),
            },
        );
        put("rumble", (self.rumble > 0).then(|| self.rumble.to_string()));
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
    fn default_template_starts_as_original_look() {
        let f = form(config::TEMPLATE);
        assert_eq!(f.look, Look::Original);
        assert_eq!(f.picture, Picture::ORIGINAL);
        assert!(f.gamepad_axes);
    }

    #[test]
    fn enhanced_look_roundtrips_through_save() {
        let mut f = form(config::TEMPLATE);
        f.set_look(Look::Enhanced, SCREEN);
        let saved = f.save(config::TEMPLATE);
        let f2 = form(&saved);
        assert_eq!(f2.look, Look::Enhanced);
        assert_eq!(f2.picture.resolution, Some((2560, 1440)));
        assert!(f2.picture.widescreen);
        assert_eq!(f2.picture.fps_limit, Some(60));
    }
}
