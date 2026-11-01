//! The settings window (`egui` / `eframe`): edits and saves `form::Form`.

use std::path::Path;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Key, Stroke, Vec2,
    vec2,
};

use screamer_launcher::config::{RendererChoice, Settings};
use screamer_launcher::form::{self, Form, Layout, Look};
use screamer_launcher::runtime;

const NAVY: Color32 = Color32::from_rgb(0x10, 0x16, 0x26);
const PANEL: Color32 = Color32::from_rgb(0x18, 0x22, 0x38);
const LINE: Color32 = Color32::from_rgb(0x2c, 0x3e, 0x66);
const GOLD: Color32 = Color32::from_rgb(0xff, 0xc8, 0x00);
const INK: Color32 = Color32::from_rgb(0xf2, 0xf5, 0xfc);
const MUTED: Color32 = Color32::from_rgb(0xa2, 0xb2, 0xd4);
const ON_GOLD: Color32 = Color32::from_rgb(0x10, 0x16, 0x26);
const ERROR: Color32 = Color32::from_rgb(0xff, 0x80, 0x80);
const OK_GREEN: Color32 = Color32::from_rgb(0x69, 0xf0, 0xae);

const SIZE: Vec2 = vec2(form::WINDOW.0, form::WINDOW.1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Play,
    Closed,
}

pub fn show(settings_path: &Path, text: &str, linux: bool) -> Result<Outcome, String> {
    let mut outcome = Outcome::Closed;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Screamer Trilogy Launcher")
            .with_inner_size(SIZE)
            .with_min_inner_size(SIZE)
            .with_max_inner_size(SIZE)
            .with_resizable(false)
            .with_maximize_button(false),
        centered: true,
        ..Default::default()
    };
    let path = settings_path.to_path_buf();
    let text = text.to_string();
    eframe::run_native(
        "Screamer Trilogy Launcher",
        options,
        Box::new(|cc| {
            set_style(&cc.egui_ctx);
            Ok(Box::new(Window::new(
                &cc.egui_ctx,
                path,
                text,
                linux,
                &mut outcome,
            )))
        }),
    )
    .map_err(|e| e.to_string())?;
    Ok(outcome)
}

fn set_style(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let font = |bytes: &'static [u8]| std::sync::Arc::new(FontData::from_static(bytes));
    fonts.font_data.insert(
        "barlow".into(),
        font(include_bytes!("../assets/fonts/Barlow-Regular.ttf")),
    );
    fonts.font_data.insert(
        "barlow-semibold".into(),
        font(include_bytes!("../assets/fonts/Barlow-SemiBold.ttf")),
    );
    fonts.font_data.insert(
        "condensed".into(),
        font(include_bytes!(
            "../assets/fonts/BarlowCondensed-SemiBold.ttf"
        )),
    );
    fonts.font_data.insert(
        "decal".into(),
        font(include_bytes!(
            "../assets/fonts/BarlowCondensed-BlackItalic.ttf"
        )),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.insert(family, vec!["barlow".into()]);
    }
    for name in ["barlow-semibold", "condensed", "decal"] {
        fonts.families.insert(
            FontFamily::Name(name.into()),
            vec![name.into(), "barlow".into()],
        );
    }
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        *v = egui::Visuals::dark();
        v.panel_fill = NAVY;
        v.extreme_bg_color = PANEL;
        v.text_edit_bg_color = Some(PANEL);
        v.override_text_color = Some(INK);
        v.selection.bg_fill = GOLD.gamma_multiply(0.5);
        v.selection.stroke = Stroke::new(1.0, GOLD);
        v.text_cursor.stroke = Stroke::new(2.0, GOLD);
        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.bg_stroke = Stroke::new(1.0, LINE);
            w.corner_radius = CornerRadius::same(6);
        }
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, MUTED);
        v.widgets.active.bg_stroke = Stroke::new(1.0, GOLD);
        style.spacing.item_spacing = vec2(8.0, 8.0);
    });
}

fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

fn named(name: &str, size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(name.into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Play,
    Picture,
    Controls,
    Buttons,
}

#[derive(Debug, Clone)]
enum FolderStatus {
    Empty,
    Valid(String),
    Invalid(String),
}

struct Window<'a> {
    settings_path: std::path::PathBuf,
    raw_text: String,
    form: Form,
    tab: Tab,
    linux: bool,
    screen: Option<(u32, u32)>,
    status: FolderStatus,
    outcome: &'a mut Outcome,
}

impl<'a> Window<'a> {
    fn new(
        _ctx: &egui::Context,
        settings_path: std::path::PathBuf,
        raw_text: String,
        linux: bool,
        outcome: &'a mut Outcome,
    ) -> Self {
        let (settings, _) = Settings::parse_file(&raw_text);
        let screen = Some((1920, 1080));
        let form = Form::new(&settings, screen);
        let status = Self::check_folder(&form.game);
        Self {
            settings_path,
            raw_text,
            form,
            tab: Tab::Play,
            linux,
            screen,
            status,
            outcome,
        }
    }

    fn check_folder(game_str: &str) -> FolderStatus {
        let trimmed = game_str.trim();
        if trimmed.is_empty() {
            return FolderStatus::Empty;
        }
        match runtime::scan_game_dir(Path::new(trimmed)) {
            Ok(disc) => {
                let exes: Vec<&str> = disc
                    .executables
                    .iter()
                    .map(|e| e.file_name.as_str())
                    .collect();
                FolderStatus::Valid(format!("{} ({})", disc.game.title(), exes.join(", ")))
            }
            Err(e) => FolderStatus::Invalid(e),
        }
    }

    fn save_and_play(&mut self, ctx: &egui::Context) {
        let updated = self.form.save(&self.raw_text);
        let _ = std::fs::write(&self.settings_path, updated);
        *self.outcome = Outcome::Play;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for Window<'_> {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("SCREAMER TRILOGY")
                        .font(named("decal", 30.0))
                        .color(GOLD),
                );
                ui.add_space(16.0);
                for (t, label) in [
                    (Tab::Play, "Play"),
                    (Tab::Picture, "Picture"),
                    (Tab::Controls, "Controls"),
                    (Tab::Buttons, "Pad Buttons"),
                ] {
                    let selected = self.tab == t;
                    let btn = egui::Button::new(
                        egui::RichText::new(label)
                            .font(named("barlow-semibold", 16.0))
                            .color(if selected { ON_GOLD } else { INK }),
                    )
                    .fill(if selected { GOLD } else { PANEL });
                    if ui.add(btn).clicked() {
                        self.tab = t;
                    }
                }
            });
            ui.separator();

            match self.tab {
                Tab::Play => self.draw_play_tab(ui),
                Tab::Picture => self.draw_picture_tab(ui),
                Tab::Controls => self.draw_controls_tab(ui),
                Tab::Buttons => self.draw_buttons_tab(ui),
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                ui.add_space(8.0);
                let can_play = matches!(self.status, FolderStatus::Valid(_));
                let play_btn = egui::Button::new(
                    egui::RichText::new("PLAY")
                        .font(named("decal", 22.0))
                        .color(if can_play { ON_GOLD } else { MUTED }),
                )
                .fill(if can_play { GOLD } else { PANEL })
                .min_size(vec2(140.0, 42.0));
                if ui.add_enabled(can_play, play_btn).clicked() {
                    self.save_and_play(&ctx);
                }
            });
        });
    }
}

impl Window<'_> {
    fn draw_play_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Game Folder (Screamer, Screamer 2, or Screamer Rally)")
                .font(named("barlow-semibold", 16.0)),
        );
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.form.game)
                .desired_width(f32::INFINITY)
                .font(font(15.0)),
        );
        if resp.changed() {
            self.status = Self::check_folder(&self.form.game);
        }
        match &self.status {
            FolderStatus::Empty => {
                ui.label(
                    egui::RichText::new("Enter the path to your installed Screamer game folder.")
                        .color(MUTED),
                );
            }
            FolderStatus::Valid(msg) => {
                ui.label(egui::RichText::new(format!("Ready: {msg}")).color(OK_GREEN));
            }
            FolderStatus::Invalid(err) => {
                ui.label(egui::RichText::new(err).color(ERROR));
            }
        }

        ui.add_space(16.0);
        ui.label(
            egui::RichText::new("Saves Folder (optional)").font(named("barlow-semibold", 16.0)),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.form.saves)
                .desired_width(f32::INFINITY)
                .font(font(15.0)),
        );

        ui.add_space(20.0);
        ui.label(
            egui::RichText::new("Graphics & Gameplay Preset").font(named("barlow-semibold", 16.0)),
        );
        ui.horizontal(|ui| {
            for (look, title, desc) in [
                (
                    Look::Original,
                    "Original 1995",
                    "4:3 aspect, 30 FPS timer lock, software/3dfx defaults.",
                ),
                (
                    Look::Enhanced,
                    "Enhanced (Recommended)",
                    "Hor+ 16:9 widescreen, unlocked 60 FPS with 1.0x physics, smooth filtering.",
                ),
                (
                    Look::SteamDeck,
                    "Steam Deck",
                    "1280x800 widescreen, 60 FPS, VSync, low-latency gamepad controls.",
                ),
                (
                    Look::Custom,
                    "Custom",
                    "Configure resolution, renderer, frame rate, and widescreen individually.",
                ),
            ] {
                let selected = self.form.look == look;
                let btn = egui::Button::new(
                    egui::RichText::new(title)
                        .font(named("barlow-semibold", 15.0))
                        .color(if selected { ON_GOLD } else { INK }),
                )
                .fill(if selected { GOLD } else { PANEL });
                if ui.add(btn).clicked() {
                    self.form.set_look(look, self.screen);
                }
                let _ = desc;
            }
        });
    }

    fn draw_picture_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Renderer").font(named("barlow-semibold", 16.0)));
        ui.horizontal_wrapped(|ui| {
            for r in RendererChoice::ALL {
                let sel = self.form.picture.renderer == r;
                let btn = egui::Button::new(egui::RichText::new(r.label()).color(if sel {
                    ON_GOLD
                } else {
                    INK
                }))
                .fill(if sel { GOLD } else { PANEL });
                if ui.add(btn).clicked() {
                    self.form.picture.renderer = r;
                    self.form.look = Form::look_of(&self.form.picture, self.screen);
                }
            }
        });

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Resolution").font(named("barlow-semibold", 16.0)));
        ui.horizontal_wrapped(|ui| {
            for (res, label) in form::resolutions(self.screen) {
                let sel = self.form.picture.resolution == res;
                let btn = egui::Button::new(egui::RichText::new(label).color(if sel {
                    ON_GOLD
                } else {
                    INK
                }))
                .fill(if sel { GOLD } else { PANEL });
                if ui.add(btn).clicked() {
                    self.form.picture.resolution = res;
                    self.form.look = Form::look_of(&self.form.picture, self.screen);
                }
            }
        });

        ui.add_space(12.0);
        if ui
            .checkbox(
                &mut self.form.picture.widescreen,
                "True 16:9 Hor+ Widescreen (scales 3D camera horizontal FOV by 4/3)",
            )
            .changed()
        {
            self.form.look = Form::look_of(&self.form.picture, self.screen);
        }

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Frame-Rate Cap").font(named("barlow-semibold", 16.0)));
        ui.horizontal(|ui| {
            for (fps, label) in form::FRAME_RATES {
                let sel = self.form.picture.fps_limit == fps;
                let btn = egui::Button::new(egui::RichText::new(label).color(if sel {
                    ON_GOLD
                } else {
                    INK
                }))
                .fill(if sel { GOLD } else { PANEL });
                if ui.add(btn).clicked() {
                    self.form.picture.fps_limit = fps;
                    self.form.look = Form::look_of(&self.form.picture, self.screen);
                }
            }
        });

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Texture Filtering").font(named("barlow-semibold", 16.0)));
        ui.horizontal(|ui| {
            for (flt, label) in form::FILTERS {
                let sel = self.form.picture.texture_filter == flt;
                let btn = egui::Button::new(egui::RichText::new(label).color(if sel {
                    ON_GOLD
                } else {
                    INK
                }))
                .fill(if sel { GOLD } else { PANEL });
                if ui.add(btn).clicked() {
                    self.form.picture.texture_filter = flt;
                    self.form.look = Form::look_of(&self.form.picture, self.screen);
                }
            }
        });

        if self.linux {
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("Gamescope Upscaler").font(named("barlow-semibold", 16.0)),
            );
            ui.horizontal(|ui| {
                for (sc, label, _) in form::SCALERS {
                    let sel = self.form.scaler == sc;
                    let btn = egui::Button::new(egui::RichText::new(label).color(if sel {
                        ON_GOLD
                    } else {
                        INK
                    }))
                    .fill(if sel { GOLD } else { PANEL });
                    if ui.add(btn).clicked() {
                        self.form.scaler = sc;
                    }
                }
            });
        }
    }

    fn draw_controls_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.checkbox(
            &mut self.form.gamepad_axes,
            "Proportional analog steering & independent L2/R2 analog triggers",
        );

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Steering Deadzone").font(named("barlow-semibold", 16.0)));
        ui.horizontal(|ui| {
            for (dz, label) in form::DEADZONES {
                let sel = self.form.deadzone == dz;
                let btn = egui::Button::new(egui::RichText::new(label).color(if sel {
                    ON_GOLD
                } else {
                    INK
                }))
                .fill(if sel { GOLD } else { PANEL });
                if ui.add(btn).clicked() {
                    self.form.deadzone = dz;
                }
            }
        });

        ui.add_space(12.0);
        ui.label(egui::RichText::new("Controller Rumble").font(named("barlow-semibold", 16.0)));
        ui.horizontal(|ui| {
            for (r, label) in form::RUMBLES {
                let sel = self.form.rumble == r;
                let btn = egui::Button::new(egui::RichText::new(label).color(if sel {
                    ON_GOLD
                } else {
                    INK
                }))
                .fill(if sel { GOLD } else { PANEL });
                if ui.add(btn).clicked() {
                    self.form.rumble = r;
                }
            }
        });
    }

    fn draw_buttons_tab(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            for (lay, label) in [
                (Layout::Modern, "Modern (Xbox / Steam Deck)"),
                (Layout::Original, "Original 1995"),
                (Layout::Custom, "Custom"),
            ] {
                let sel = self.form.layout == lay;
                let btn = egui::Button::new(egui::RichText::new(label).color(if sel {
                    ON_GOLD
                } else {
                    INK
                }))
                .fill(if sel { GOLD } else { PANEL });
                if ui.add(btn).clicked() {
                    self.form.set_layout(lay);
                }
            }
        });

        ui.add_space(12.0);
        for (action_idx, action_name) in form::ACTIONS {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{action_name:18}"))
                        .font(named("barlow-semibold", 15.0)),
                );
                let current = self.form.pad_button(action_idx);
                for (btn_num, btn_label) in form::PAD_BUTTONS {
                    let sel = current == btn_num;
                    let b = egui::Button::new(egui::RichText::new(btn_label).color(if sel {
                        ON_GOLD
                    } else {
                        INK
                    }))
                    .fill(if sel { GOLD } else { PANEL });
                    if ui.add(b).clicked() {
                        self.form.set_pad_button(action_idx, btn_num);
                    }
                }
            });
        }
    }
}
