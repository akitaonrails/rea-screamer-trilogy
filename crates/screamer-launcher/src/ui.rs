//! The unified settings window (`egui` / `eframe`) for the Screamer trilogy:
//! **Screamer (1995)**, **Screamer 2 (1996)**, and **Screamer Rally (1997)**.
//!
//! Styled to match `cmr-launcher`: left-hand full-height box art hero for the
//! chosen game with smooth gradient fades into the dark tarmac chassis, top-right
//! slanted livery stripes, condensed italic decal header, underline tabs, look
//! cards with a gold `Recommended` tab, and per-game option capability checks
//! that automatically gray out choices unsupported by the selected game.

use std::path::{Path, PathBuf};
use std::process::Command;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Key, Mesh,
    Rect, Sense, Shape, Stroke, StrokeKind, TextureHandle, Vec2, pos2, vec2,
};

use screamer_launcher::config::{RendererChoice, Settings};
use screamer_launcher::edition::Game;
use screamer_launcher::form::{self, Form, Layout, Look};
use screamer_launcher::runtime::{self, DiscoveredTrilogy};

const CHASSIS: Color32 = Color32::from_rgb(0x0e, 0x17, 0x2a);
const PANEL: Color32 = Color32::from_rgb(0x16, 0x24, 0x42);
const LINE: Color32 = Color32::from_rgb(0x28, 0x3e, 0x6b);
const GOLD: Color32 = Color32::from_rgb(0xf5, 0xc4, 0x00);
const INK: Color32 = Color32::from_rgb(0xee, 0xf2, 0xfb);
const MUTED: Color32 = Color32::from_rgb(0x9f, 0xb0, 0xd4);
const DISABLED_INK: Color32 = Color32::from_rgb(0x4f, 0x60, 0x85);
const DISABLED_LINE: Color32 = Color32::from_rgb(0x1b, 0x29, 0x47);
const ON_GOLD: Color32 = Color32::from_rgb(0x0e, 0x17, 0x2a);
const ERROR: Color32 = Color32::from_rgb(0xff, 0x8a, 0x80);

const SIZE: Vec2 = vec2(form::WINDOW.0, form::WINDOW.1);
const PHOTO_W: f32 = 284.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Play,
    Closed,
}

pub fn show(settings_path: &Path, text: &str, linux: bool) -> Result<Outcome, String> {
    let mut outcome = Outcome::Closed;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Screamer Trilogy")
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
        "Screamer Trilogy",
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
        v.panel_fill = CHASSIS;
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

struct Window<'a> {
    form: Form,
    tab: Tab,
    trilogy: DiscoveredTrilogy,
    checked: String,
    screen: Option<(u32, u32)>,
    resolutions: Vec<(Option<(u32, u32)>, String)>,
    controllers: Vec<String>,
    linux: bool,
    covers: [Option<TextureHandle>; 3],
    path: PathBuf,
    text: String,
    error: Option<String>,
    sized: bool,
    outcome: &'a mut Outcome,
}

impl<'a> Window<'a> {
    fn new(
        ctx: &egui::Context,
        path: PathBuf,
        text: String,
        linux: bool,
        outcome: &'a mut Outcome,
    ) -> Self {
        let (settings, _) = Settings::parse_file(&text);
        let screen = Some((1920, 1080));
        let mut form = Form::new(&settings, screen);
        let trilogy = check_trilogy(&form.game);

        // If the user hasn't explicitly picked `which` in the config yet and the
        // folder points directly at one game (or only one game is installed),
        // select that game automatically.
        if settings.which.is_none() {
            if let Some(direct) = trilogy.direct_game {
                form.which = direct;
            } else if trilogy.get(form.which).is_none()
                && let Some(first) = Game::ALL.into_iter().find(|&g| trilogy.get(g).is_some())
            {
                form.which = first;
            }
        }
        let has_glide = trilogy
            .get(form.which)
            .is_none_or(|d| d.can_use_dos_glide());
        form.set_which(form.which, has_glide, screen);

        let initial_tab = match std::env::var("SCREAMER_TAB")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "picture" => Tab::Picture,
            "controls" => Tab::Controls,
            "buttons" => Tab::Buttons,
            _ => Tab::Play,
        };

        Self {
            checked: form.game.clone(),
            trilogy,
            form,
            tab: initial_tab,
            screen,
            resolutions: form::resolutions(screen),
            controllers: detect_controllers(),
            linux,
            covers: [
                load_jpeg(
                    ctx,
                    "cover_s1",
                    include_bytes!("../assets/cover_screamer1.jpg"),
                ),
                load_jpeg(
                    ctx,
                    "cover_s2",
                    include_bytes!("../assets/cover_screamer2.jpg"),
                ),
                load_jpeg(
                    ctx,
                    "cover_sr",
                    include_bytes!("../assets/cover_screamer_rally.jpg"),
                ),
            ],
            path,
            text,
            error: None,
            sized: false,
            outcome,
        }
    }

    fn fit_screen(&mut self, ctx: &egui::Context) {
        self.sized = true;
        if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
            let mon = (monitor.x as u32, monitor.y as u32);
            if mon.0 >= 640 && mon.1 >= 480 {
                self.screen = Some(mon);
                self.resolutions = form::resolutions(self.screen);
            }
        }
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        let zoom = form::window_zoom(self.screen, native);
        if zoom == 1.0 {
            return;
        }
        ctx.set_zoom_factor(zoom);
        let size = SIZE * zoom;
        for cmd in [
            egui::ViewportCommand::MinInnerSize(size),
            egui::ViewportCommand::MaxInnerSize(size),
            egui::ViewportCommand::InnerSize(size),
        ] {
            ctx.send_viewport_cmd(cmd);
        }
    }

    fn has_dos_glide_for_current(&self) -> bool {
        self.trilogy
            .get(self.form.which)
            .is_none_or(|d| d.can_use_dos_glide())
    }

    fn ready(&self) -> bool {
        self.trilogy.get(self.form.which).is_some()
    }

    fn play(&mut self, ctx: &egui::Context) {
        if !self.ready() {
            self.tab = Tab::Play;
            return;
        }
        let saved = self.form.save(&self.text);
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::write(&self.path, saved) {
            Ok(()) => {
                *self.outcome = Outcome::Play;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(e) => self.error = Some(format!("Can't save settings: {e}")),
        }
    }
}

fn check_trilogy(folder: &str) -> DiscoveredTrilogy {
    let trimmed = folder.trim();
    if trimmed.is_empty() {
        return DiscoveredTrilogy::default();
    }
    runtime::discover_trilogy(Path::new(trimmed))
}

fn detect_controllers() -> Vec<String> {
    let mut out = Vec::new();
    let sys_input = Path::new("/sys/class/input");
    if sys_input.is_dir()
        && let Ok(entries) = std::fs::read_dir(sys_input)
    {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("js")
                && let Ok(dev) = std::fs::read_to_string(e.path().join("device/name"))
            {
                let trimmed = dev.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
            }
        }
    }
    out
}

fn pick_folder_dialog(title: &str) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(out) = Command::new("zenity")
            .args(["--file-selection", "--directory", "--title", title])
            .output()
            && out.status.success()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
        if let Ok(out) = Command::new("kdialog")
            .args(["--getexistingdirectory", ".", "--title", title])
            .output()
            && out.status.success()
        {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    let _ = title;
    None
}

fn load_jpeg(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<TextureHandle> {
    let mut decoder = jpeg_decoder::Decoder::new(bytes);
    let rgb = decoder.decode().ok()?;
    let info = decoder.info()?;
    let size = [usize::from(info.width), usize::from(info.height)];
    (rgb.len() == size[0] * size[1] * 3).then(|| {
        ctx.load_texture(
            name,
            egui::ColorImage::from_rgb(size, &rgb),
            egui::TextureOptions::LINEAR,
        )
    })
}

impl eframe::App for Window<'_> {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.sized {
            self.fit_screen(&ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, CHASSIS);
        self.draw_box_art(ui, full);
        draw_decal(ui, full);

        let content = Rect::from_min_max(
            pos2(full.left() + PHOTO_W + 16.0, full.top() + 22.0),
            pos2(full.right() - 28.0, full.bottom() - 22.0),
        );
        let footer_h = 50.0;
        let body = Rect::from_min_max(
            content.min,
            pos2(content.right(), content.bottom() - footer_h - 12.0),
        );

        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
            ui.label(
                egui::RichText::new(self.form.which.short_title())
                    .font(named("decal", 44.0))
                    .color(INK),
            );
            ui.add_space(-6.0);
            self.status_line(ui);
            ui.add_space(4.0);
            self.tabs(ui);
            ui.add_space(12.0);
            match self.tab {
                Tab::Play => self.play_tab(ui),
                Tab::Picture => self.picture_tab(ui),
                Tab::Controls => self.controls_tab(ui),
                Tab::Buttons => self.buttons_tab(ui),
            }
        });

        let footer = Rect::from_min_max(
            pos2(content.left(), content.bottom() - footer_h),
            content.max,
        );
        self.footer(ui, footer, &ctx);

        if ctx.input(|i| i.key_pressed(Key::Enter)) && !ctx.egui_wants_keyboard_input() {
            self.play(&ctx);
        }
        if self.form.game != self.checked {
            self.checked = self.form.game.clone();
            self.trilogy = check_trilogy(&self.form.game);
            if let Some(direct) = self.trilogy.direct_game {
                self.form.which = direct;
            } else if self.trilogy.get(self.form.which).is_none()
                && let Some(first) = Game::ALL
                    .into_iter()
                    .find(|&g| self.trilogy.get(g).is_some())
            {
                self.form.which = first;
            }
            let has_glide = self.has_dos_glide_for_current();
            self.form
                .set_which(self.form.which, has_glide, self.screen);
        }
    }
}

impl Window<'_> {
    fn draw_box_art(&self, ui: &egui::Ui, full: Rect) {
        let rect = Rect::from_min_size(full.min, vec2(PHOTO_W, full.height()));
        let p = ui.painter().with_clip_rect(rect);
        if let Some(tex) = &self.covers[self.form.which.index()] {
            let [w, h] = tex.size();
            let img_aspect = w as f32 / h as f32;
            let rect_aspect = rect.width() / rect.height();
            let uv = if img_aspect > rect_aspect {
                let u_span = (rect_aspect / img_aspect).min(1.0);
                let u_pad = (1.0 - u_span) * 0.5;
                Rect::from_min_max(pos2(u_pad, 0.0), pos2(1.0 - u_pad, 1.0))
            } else {
                let v_span = (img_aspect / rect_aspect).min(1.0);
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, v_span))
            };
            p.image(tex.id(), rect, uv, Color32::WHITE);
        }
        // Smooth horizontal and bottom fades into the chassis background
        gradient(
            &p,
            Rect::from_min_max(
                pos2(rect.left() + rect.width() * 0.52, rect.top()),
                rect.max,
            ),
            true,
        );
        gradient(
            &p,
            Rect::from_min_max(pos2(rect.left(), rect.bottom() - 150.0), rect.max),
            false,
        );
        let x = rect.left() + 20.0;
        let y = rect.bottom() - 20.0;
        p.text(
            pos2(x, y),
            Align2::LEFT_BOTTOM,
            "Cover art: Virgin Interactive / Milestone",
            font(11.0),
            MUTED,
        );
        p.text(
            pos2(x, y - 16.0),
            Align2::LEFT_BOTTOM,
            self.form.which.studio_credit(),
            font(13.0),
            MUTED,
        );
        p.text(
            pos2(x, y - 34.0),
            Align2::LEFT_BOTTOM,
            self.form.which.display_name(),
            named("barlow-semibold", 19.0),
            INK,
        );
    }

    fn status_line(&self, ui: &mut egui::Ui) {
        let (text, color) = if let Some(err) = &self.error {
            (err.clone(), ERROR)
        } else if let Some(disc) = self.trilogy.get(self.form.which) {
            let exe_summary = disc
                .select_executable(self.form.picture.renderer)
                .map(|e| format!("{} ({})", e.file_name, e.edition.role_label()))
                .unwrap_or_else(|_| "Ready".into());
            (
                format!("Ready · {exe_summary} · Press Play or Enter."),
                MUTED,
            )
        } else if self.form.game.trim().is_empty() {
            (
                "First, show the launcher where your Screamer folder is.".to_string(),
                MUTED,
            )
        } else {
            (
                format!(
                    "{} was not found in this folder.",
                    self.form.which.display_name()
                ),
                ERROR,
            )
        };
        ui.label(egui::RichText::new(text).font(font(14.0)).color(color));
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 26.0;
            for (tab, name) in [
                (Tab::Play, "Play"),
                (Tab::Picture, "Picture"),
                (Tab::Controls, "Controls"),
                (Tab::Buttons, "Buttons"),
            ] {
                let on = self.tab == tab;
                let galley = ui.painter().layout_no_wrap(
                    name.to_string(),
                    named("condensed", 22.0),
                    if on { INK } else { MUTED },
                );
                let (rect, resp) =
                    ui.allocate_exact_size(galley.size() + vec2(0.0, 8.0), Sense::click());
                let color = if on || resp.hovered() { INK } else { MUTED };
                ui.painter().galley(rect.min, galley, color);
                if on {
                    let bar = Rect::from_min_size(
                        pos2(rect.left(), rect.bottom() - 3.0),
                        vec2(rect.width(), 3.0),
                    );
                    ui.painter().rect_filled(bar, 0.0, GOLD);
                }
                if resp
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.tab = tab;
                }
            }
        });
        let y = ui.cursor().top() - 4.0;
        let r = ui.max_rect();
        ui.painter()
            .hline(r.left()..=r.right(), y, Stroke::new(1.0, LINE));
    }

    fn play_tab(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Game in the trilogy");
        ui.add_space(2.0);
        let gap = 10.0;
        let card_w = (ui.available_width() - 2.0 * gap) / 3.0;
        let mut chosen_game = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for game in Game::ALL {
                let installed = self.trilogy.get(game).is_some();
                let subtitle = match self.trilogy.get(game) {
                    Some(disc) => {
                        let mode = disc
                            .select_executable(self.form.picture.renderer)
                            .map(|e| e.edition.role_label())
                            .unwrap_or("Ready");
                        format!("{} · {mode}", game.year())
                    }
                    None if self.form.game.trim().is_empty() => {
                        format!("{} · Set folder below", game.year())
                    }
                    None => format!("{} · Not in folder", game.year()),
                };
                if game_card(
                    ui,
                    vec2(card_w, 64.0),
                    self.form.which == game,
                    game.short_title(),
                    &subtitle,
                    installed,
                ) {
                    chosen_game = Some(game);
                }
            }
        });
        if let Some(g) = chosen_game {
            let has_glide = self.trilogy.get(g).is_none_or(|d| d.can_use_dos_glide());
            self.form.set_which(g, has_glide, self.screen);
            self.error = None;
        }

        ui.add_space(8.0);
        heading(ui, "Game or trilogy folder");
        let hint = "Folder with Screamer, Screamer 2, Screamer Rally, or all three (e.g. .../screamer/games)";
        if folder_field(
            ui,
            &mut self.form.game,
            hint,
            "Choose your Screamer or trilogy folder",
        ) {
            self.error = None;
        }

        let (note, color) = if self.form.game.trim().is_empty() {
            (
                "Point at a single game folder or a parent folder containing all three games. Never modified.".to_string(),
                MUTED,
            )
        } else if self.trilogy.any_found() {
            let found_names: Vec<&str> = Game::ALL
                .into_iter()
                .filter(|&g| self.trilogy.get(g).is_some())
                .map(|g| g.display_name())
                .collect();
            (format!("Discovered: {}", found_names.join(" · ")), GOLD)
        } else {
            (
                "No recognized Screamer, Screamer 2, or Screamer Rally executables in this folder."
                    .to_string(),
                ERROR,
            )
        };
        ui.label(egui::RichText::new(note).font(font(13.0)).color(color));
        ui.add_space(8.0);

        heading(ui, "Look");
        ui.add_space(4.0);
        let best = form::recommended(self.screen);
        let orig_title = format!("Original {}", self.form.which.year());
        let cards = [
            (
                Look::Original,
                orig_title.as_str(),
                "4:3 window, CRT shader, as shipped.",
            ),
            (
                Look::Enhanced,
                "Enhanced",
                "1080p Hor+ widescreen, smooth filter.",
            ),
            (
                Look::SteamDeck,
                "Steam Deck",
                "1280×800 Hor+ widescreen, smooth filter.",
            ),
            (Look::Custom, "Custom", "Pick each setting in Picture."),
        ];
        let look_w = (ui.available_width() - 3.0 * gap) / 4.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (look, title, desc) in cards {
                if look_card(
                    ui,
                    vec2(look_w, 96.0),
                    self.form.look == look,
                    title,
                    desc,
                    look == best,
                ) {
                    self.form.set_look(look, self.screen);
                    let has_glide = self.has_dos_glide_for_current();
                    self.form
                        .set_which(self.form.which, has_glide, self.screen);
                    if look == Look::Custom {
                        self.tab = Tab::Picture;
                    }
                }
            }
        });
        ui.add_space(8.0);

        heading(ui, "Saves");
        folder_field(
            ui,
            &mut self.form.saves,
            "Default per-game saves folder (~/.local/share/screamer-launcher/saves)",
            "Choose where your saves are kept",
        );
    }

    fn picture_tab(&mut self, ui: &mut egui::Ui) {
        let before = self.form.picture;
        let which = self.form.which;
        let has_glide = self.has_dos_glide_for_current();
        let resolutions = &self.resolutions;
        let p = &mut self.form.picture;

        let row = |ui: &mut egui::Ui, name: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
            ui.horizontal_top(|ui| {
                field(ui, name);
                ui.vertical(|ui| add(ui));
            });
        };

        row(ui, "Renderer", &mut |ui| {
            let opts: Vec<(RendererChoice, &str, Option<&str>)> = RendererChoice::ALL
                .into_iter()
                .map(|r| (r, r.label(), r.disabled_reason(which, has_glide)))
                .collect();
            chips_with_disabled(ui, &opts, &mut p.renderer);
        });

        row(ui, "Display", &mut |ui| {
            chips(ui, resolutions, &mut p.resolution);
        });

        row(ui, "Shader / Filter", &mut |ui| {
            chips(ui, &form::FILTERS, &mut p.texture_filter);
        });

        row(ui, "Vsync", &mut |ui| {
            chips(ui, &form::VSYNCS, &mut p.vsync);
        });

        row(ui, "", &mut |ui| {
            toggle(
                ui,
                &mut p.widescreen,
                "Widescreen: 16:9 Hor+ 3D camera FOV (shows more to the sides)",
                None,
            );
        });

        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(
                "Note: All three DOS games render internally at 640×480 (3dfx Voodoo / SVGA) or 320×200 (VGA), upscaled to your display by OpenGL, and pace physics via the 50 Hz SOS timer table.",
            )
            .font(font(13.0))
            .color(MUTED),
        );
        let _ = self.linux;

        if self.form.picture != before {
            self.form.look = Form::look_of(&self.form.picture, self.screen);
        }
    }

    fn controls_tab(&mut self, ui: &mut egui::Ui) {
        toggle(
            ui,
            &mut self.form.gamepad_axes,
            "Gamepad: proportional analog steering, L2/R2 trigger pedals, D-pad menus",
            None,
        );
        ui.add_space(6.0);

        let pad_off_reason = (!self.form.gamepad_axes)
            .then_some("Turn on Gamepad above to configure steering deadzone and rumble.");

        ui.horizontal_top(|ui| {
            field(ui, "Deadzone");
            let opts: Vec<_> = form::DEADZONES
                .into_iter()
                .map(|(dz, label)| (dz, label, pad_off_reason))
                .collect();
            chips_with_disabled(ui, &opts, &mut self.form.deadzone);
        });

        ui.horizontal_top(|ui| {
            field(ui, "Rumble");
            let opts: Vec<_> = form::RUMBLES
                .into_iter()
                .map(|(r, label)| (r, label, pad_off_reason))
                .collect();
            chips_with_disabled(ui, &opts, &mut self.form.rumble);
        });

        ui.add_space(8.0);
        let pad_status = if self.controllers.is_empty() {
            "No controller detected right now. You can turn on your controller at any time (even mid-race) and it will connect automatically.".to_string()
        } else {
            format!(
                "Connected ({}): {}",
                self.controllers.len(),
                self.controllers.join(", ")
            )
        };
        ui.label(
            egui::RichText::new(pad_status)
                .font(font(13.0))
                .color(if self.controllers.is_empty() {
                    MUTED
                } else {
                    GOLD
                }),
        );

        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(
                "Keyboard arrows (Up/Down/Left/Right) and gamepad work simultaneously in menus and races.",
            )
            .font(font(13.0))
            .color(MUTED),
        );
    }

    fn buttons_tab(&mut self, ui: &mut egui::Ui) {
        let orig_title = format!("Original {}", self.form.which.year());
        let cards = [
            (
                Layout::Original,
                orig_title.as_str(),
                "2-button DOS gameport layout.",
                false,
            ),
            (
                Layout::Modern,
                "Modern",
                "Xbox / Steam Deck racing layout.",
                true,
            ),
            (Layout::Custom, "Custom", "Your own button choices.", false),
        ];
        let gap = 12.0;
        let w = (ui.available_width() - 2.0 * gap) / 3.0;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (layout, title, note, recommended) in cards {
                if look_card(
                    ui,
                    vec2(w, 68.0),
                    self.form.layout == layout,
                    title,
                    note,
                    recommended,
                ) && self.form.gamepad_axes
                {
                    self.form.set_layout(layout);
                }
            }
        });
        ui.label(
            egui::RichText::new(
                "Names follow Xbox / Steam Deck pads. D-pad and Left Stick navigate menus; A confirms, B goes back.",
            )
            .font(font(13.0))
            .color(MUTED),
        );
        ui.spacing_mut().item_spacing.y = 4.0;
        let pad_off_reason =
            (!self.form.gamepad_axes).then_some("Turn on Gamepad in the Controls tab first.");
        let options: Vec<_> = form::PAD_BUTTONS[..8]
            .iter()
            .map(|&(b, l)| (b, l, pad_off_reason))
            .collect();
        for (k, name) in form::ACTIONS {
            ui.horizontal_top(|ui| {
                field(ui, name);
                let mut button = self.form.pad_button(k);
                chips_with_disabled(ui, &options, &mut button);
                if self.form.gamepad_axes {
                    self.form.set_pad_button(k, button);
                }
            });
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui, rect: Rect, ctx: &egui::Context) {
        let button = Rect::from_min_size(
            pos2(rect.right() - 170.0, rect.top()),
            vec2(170.0, rect.height()),
        );
        let hint = format!(
            "Settings are saved when you press Play ({}).",
            self.form.which.display_name()
        );
        let galley = ui.painter().layout(
            hint,
            font(13.0),
            MUTED,
            button.left() - rect.left() - 20.0,
        );
        let y = rect.center().y - galley.size().y / 2.0;
        ui.painter().galley(pos2(rect.left(), y), galley, MUTED);

        let resp = ui.interact(button, ui.scope_id().with("play"), Sense::click());
        let ready = self.ready();
        let fill = match (ready, resp.hovered()) {
            (false, _) => LINE,
            (true, true) => Color32::from_rgb(0xff, 0xd3, 0x2a),
            (true, false) => GOLD,
        };
        let s = 10.0;
        let plate = vec![
            pos2(button.left() + s, button.top()),
            button.right_top(),
            pos2(button.right() - s, button.bottom()),
            button.left_bottom(),
        ];
        ui.painter()
            .add(Shape::convex_polygon(plate, fill, Stroke::NONE));
        ui.painter().text(
            button.center(),
            Align2::CENTER_CENTER,
            "Play",
            named("decal", 30.0),
            if ready { ON_GOLD } else { MUTED },
        );
        if resp
            .on_hover_cursor(if ready {
                egui::CursorIcon::PointingHand
            } else {
                egui::CursorIcon::Default
            })
            .clicked()
        {
            self.play(ctx);
        }
    }
}

trait RoleLabel {
    fn role_label(&self) -> &'static str;
}

impl RoleLabel for screamer_launcher::edition::Edition {
    fn role_label(&self) -> &'static str {
        use screamer_launcher::edition::ExeRole;
        match self.role {
            ExeRole::Glide3dfx => "3dfx Voodoo",
            ExeRole::Svga16Bit => "SVGA 16-bit",
            ExeRole::Svga8Bit => "SVGA 8-bit",
            ExeRole::VgaLow => "VGA 320×200",
            ExeRole::Setup => "Setup",
        }
    }
}

fn gradient(p: &egui::Painter, rect: Rect, horizontal: bool) {
    let mut mesh = Mesh::default();
    let clear = Color32::from_rgba_unmultiplied(CHASSIS.r(), CHASSIS.g(), CHASSIS.b(), 0);
    let (a, b, c, d) = if horizontal {
        (clear, CHASSIS, CHASSIS, clear)
    } else {
        (clear, clear, CHASSIS, CHASSIS)
    };
    mesh.colored_vertex(rect.left_top(), a);
    mesh.colored_vertex(rect.right_top(), b);
    mesh.colored_vertex(rect.right_bottom(), c);
    mesh.colored_vertex(rect.left_bottom(), d);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
}

fn draw_decal(ui: &egui::Ui, full: Rect) {
    let p = ui.painter().with_clip_rect(full);
    for (i, (w, alpha)) in [(30.0, 1.0), (16.0, 0.65)].into_iter().enumerate() {
        let x = full.right() - 120.0 + i as f32 * 40.0;
        let slant = 60.0;
        let pts = vec![
            pos2(x, full.top()),
            pos2(x + w, full.top()),
            pos2(x + w - slant, full.top() + 112.0),
            pos2(x - slant, full.top() + 112.0),
        ];
        p.add(Shape::convex_polygon(
            pts,
            GOLD.gamma_multiply(alpha),
            Stroke::NONE,
        ));
    }
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(named("barlow-semibold", 14.0))
            .color(MUTED),
    );
}

fn field(ui: &mut egui::Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(128.0, 30.0), Sense::hover());
    ui.painter().text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        text,
        font(15.0),
        INK,
    );
}

fn folder_field(ui: &mut egui::Ui, value: &mut String, hint: &str, title: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let w = ui.available_width() - 100.0;
        let edit = egui::TextEdit::singleline(value)
            .hint_text(egui::RichText::new(hint).color(MUTED))
            .font(font(15.0))
            .margin(vec2(10.0, 8.0))
            .desired_width(w);
        changed |= ui.add(edit).changed();
        if outline_button(ui, "Browse…").clicked()
            && let Some(dir) = pick_folder_dialog(title)
        {
            *value = dir.display().to_string();
            changed = true;
        }
    });
    changed
}

fn outline_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(92.0, 34.0), Sense::click());
    let hover = resp.hovered();
    ui.painter().rect(
        rect,
        CornerRadius::same(6),
        if hover { LINE } else { Color32::TRANSPARENT },
        Stroke::new(1.5, GOLD),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        named("barlow-semibold", 15.0),
        GOLD,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn chips<T: PartialEq + Copy, S: AsRef<str>>(
    ui: &mut egui::Ui,
    options: &[(T, S)],
    current: &mut T,
) {
    let mapped: Vec<(T, &str, Option<&str>)> = options
        .iter()
        .map(|(v, s)| (*v, s.as_ref(), None))
        .collect();
    chips_with_disabled(ui, &mapped, current);
}

/// Renders a horizontal row of choice chips where options with `Some(reason)`
/// are visibly grayed out (disabled) and explain why on hover.
fn chips_with_disabled<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    options: &[(T, &str, Option<&str>)],
    current: &mut T,
) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for &(value, label, disabled_reason) in options {
            let enabled = disabled_reason.is_none();
            let on = enabled && *current == value;
            let text_color = if !enabled {
                DISABLED_INK
            } else if on {
                ON_GOLD
            } else {
                INK
            };
            let galley = ui.painter().layout_no_wrap(
                label.to_string(),
                if on {
                    named("barlow-semibold", 14.0)
                } else {
                    font(14.0)
                },
                text_color,
            );
            let size = vec2(galley.size().x + 22.0, 30.0);
            let sense = if enabled {
                Sense::click()
            } else {
                Sense::hover()
            };
            let (rect, resp) = ui.allocate_exact_size(size, sense);
            let fill = if !enabled {
                Color32::TRANSPARENT
            } else if on {
                GOLD
            } else if resp.hovered() {
                LINE
            } else {
                Color32::TRANSPARENT
            };
            let border = if !enabled {
                DISABLED_LINE
            } else if on {
                GOLD
            } else {
                LINE
            };
            ui.painter().rect(
                rect,
                CornerRadius::same(6),
                fill,
                Stroke::new(1.0, border),
                StrokeKind::Inside,
            );
            let at = rect.center() - galley.size() / 2.0;
            ui.painter().galley(at, galley, text_color);

            if let Some(why) = disabled_reason {
                resp.on_hover_text(why);
            } else if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *current = value;
            }
        }
    });
}

fn toggle(ui: &mut egui::Ui, on: &mut bool, text: &str, disabled_reason: Option<&str>) {
    let enabled = disabled_reason.is_none();
    let label_color = if enabled { INK } else { DISABLED_INK };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font(15.0), label_color);
    let size = vec2(46.0 + 12.0 + galley.size().x, 30.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if enabled && resp.clicked() {
        *on = !*on;
    }
    let active = enabled && *on;
    let t = ui.ctx().animate_bool(resp.id, active);
    let track = Rect::from_min_size(pos2(rect.left(), rect.center().y - 13.0), vec2(46.0, 26.0));
    let p = ui.painter();
    p.rect(
        track,
        CornerRadius::same(13),
        if active { GOLD } else { Color32::TRANSPARENT },
        Stroke::new(
            1.5,
            if !enabled {
                DISABLED_LINE
            } else if active {
                GOLD
            } else {
                MUTED
            },
        ),
        StrokeKind::Inside,
    );
    let x = egui::lerp(track.left() + 13.0..=track.right() - 13.0, t);
    p.circle_filled(
        pos2(x, track.center().y),
        9.0,
        if !enabled {
            DISABLED_INK
        } else if active {
            ON_GOLD
        } else {
            MUTED
        },
    );
    p.galley(
        pos2(
            track.right() + 12.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        label_color,
    );
    if let Some(why) = disabled_reason {
        resp.on_hover_text(why);
    } else {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }
}

fn game_card(
    ui: &mut egui::Ui,
    size: Vec2,
    on: bool,
    title: &str,
    subtitle: &str,
    installed: bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let fill = if on {
        PANEL
    } else if resp.hovered() {
        PANEL.gamma_multiply(0.6)
    } else {
        Color32::TRANSPARENT
    };
    p.rect(
        rect,
        CornerRadius::same(8),
        fill,
        Stroke::new(if on { 2.0 } else { 1.0 }, if on { GOLD } else { LINE }),
        StrokeKind::Inside,
    );
    let pad = 12.0;
    p.text(
        rect.left_top() + vec2(pad, 10.0),
        Align2::LEFT_TOP,
        title,
        named("condensed", 20.0),
        if on { GOLD } else { INK },
    );
    let dot_color = if installed { GOLD } else { DISABLED_INK };
    p.circle_filled(pos2(rect.right() - pad, rect.top() + 18.0), 4.0, dot_color);
    p.text(
        rect.left_top() + vec2(pad, 36.0),
        Align2::LEFT_TOP,
        subtitle,
        font(12.5),
        if installed { MUTED } else { DISABLED_INK },
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn look_card(
    ui: &mut egui::Ui,
    size: Vec2,
    on: bool,
    title: &str,
    note: &str,
    recommended: bool,
) -> bool {
    let w = size.x;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let fill = if on {
        PANEL
    } else if resp.hovered() {
        PANEL.gamma_multiply(0.6)
    } else {
        Color32::TRANSPARENT
    };
    p.rect(
        rect,
        CornerRadius::same(8),
        fill,
        Stroke::new(if on { 2.0 } else { 1.0 }, if on { GOLD } else { LINE }),
        StrokeKind::Inside,
    );
    let pad = 12.0;
    p.text(
        rect.left_top() + vec2(pad, pad - 2.0),
        Align2::LEFT_TOP,
        title,
        named("condensed", 20.0),
        if on { GOLD } else { INK },
    );
    if recommended {
        let galley = p.layout_no_wrap(
            "Recommended".into(),
            named("barlow-semibold", 11.5),
            ON_GOLD,
        );
        let tab = Rect::from_min_size(
            pos2(
                rect.right() - pad - galley.size().x - 10.0,
                rect.top() - 8.0,
            ),
            galley.size() + vec2(10.0, 2.0),
        );
        p.rect_filled(tab, CornerRadius::same(4), GOLD);
        p.galley(tab.min + vec2(5.0, 1.0), galley, ON_GOLD);
    }
    let galley = p.layout(note.to_string(), font(12.5), MUTED, w - 2.0 * pad);
    p.galley(rect.left_top() + vec2(pad, pad + 28.0), galley, MUTED);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}
