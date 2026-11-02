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

use rea_launcher::pad::{self as bits, Nav, PadNav};
use rea_launcher_ui::eframe;
use rea_launcher_ui::egui::{self, Align2, Color32, Key, Rect, TextureHandle, Vec2, pos2, vec2};
use rea_launcher_ui::theme::{self, Metrics, Palette, body, decal, semibold};
use rea_launcher_ui::widgets::{self, Chip, ChipSize, Fade, GameCard, LookCard};
use rea_launcher_ui::window::WindowFit;

use screamer_launcher::config::{MachineCapabilities, RendererChoice, Settings};
use screamer_launcher::edition::Game;
use screamer_launcher::form::{self, Form, Layout, Look};
use screamer_launcher::runtime::{self, DiscoveredTrilogy};

const CHASSIS: Color32 = Color32::from_rgb(0x0e, 0x17, 0x2a);
const PANEL: Color32 = Color32::from_rgb(0x16, 0x24, 0x42);
/// Control borders: 3:1 against the chassis and the panel (WCAG non-text
/// contrast); the old `#283e6b` was 1.70:1.
const LINE: Color32 = Color32::from_rgb(0x5d, 0x6e, 0x90);
const GOLD: Color32 = Color32::from_rgb(0xf5, 0xc4, 0x00);
const INK: Color32 = Color32::from_rgb(0xee, 0xf2, 0xfb);
const MUTED: Color32 = Color32::from_rgb(0x9f, 0xb0, 0xd4);
const DISABLED_INK: Color32 = Color32::from_rgb(0x4f, 0x60, 0x85);
const DISABLED_LINE: Color32 = Color32::from_rgb(0x1b, 0x29, 0x47);
const ON_GOLD: Color32 = Color32::from_rgb(0x0e, 0x17, 0x2a);
const ERROR: Color32 = Color32::from_rgb(0xff, 0x8a, 0x80);

/// The settings window's colours, by the SDK's roles: gold is both the
/// action (Play, tab underline) and the selection.
const PALETTE: Palette = Palette {
    base: CHASSIS,
    panel: PANEL,
    line: LINE,
    ink: INK,
    muted: MUTED,
    action: GOLD,
    action_hover: Color32::from_rgb(0xff, 0xd3, 0x2a),
    on_action: ON_GOLD,
    select: GOLD,
    on_select: ON_GOLD,
    error: ERROR,
    disabled_ink: DISABLED_INK,
    disabled_line: DISABLED_LINE,
    focus: GOLD,
    selection: theme::tint(GOLD, 128),
};

/// Screamer's widget sizes (the values its own widgets drew before the SDK
/// had them).
const METRICS: Metrics = Metrics {
    chip_pad: 22.0,
    field_width: 128.0,
    toggle_wrap: false,
    card: theme::CardMetrics {
        pad: 12.0,
        title: 20.0,
        badge: 11.5,
        badge_pad: 5.0,
        badge_rise: 8.0,
        note: 12.5,
        note_top: 28.0,
    },
    ..Metrics::DEFAULT
};

const SIZE: Vec2 = vec2(form::WINDOW.0, form::WINDOW.1);
const PHOTO_W: f32 = 284.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Play,
    Closed,
}

/// Why the settings window did not finish normally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShowError {
    /// No window could be made (no display, no OpenGL): the caller may play
    /// with the saved settings.
    NoWindow(String),
    /// The window opened, then failed: the player's choices may be lost, so
    /// the game must not start with stale settings.
    Failed(String),
}

/// Sorts `run_native`'s result by whether the window ever opened.
fn finish(
    opened: bool,
    result: Result<(), String>,
    outcome: Outcome,
) -> Result<Outcome, ShowError> {
    match result {
        Ok(()) => Ok(outcome),
        Err(e) if opened => Err(ShowError::Failed(e)),
        Err(e) => Err(ShowError::NoWindow(e)),
    }
}

pub fn show(settings_path: &Path, text: &str, linux: bool) -> Result<Outcome, ShowError> {
    let mut outcome = Outcome::Closed;
    let mut opened = false;
    // Native on Linux and Windows (never Wine): created at 1x, fitted to
    // the screen and its scale on the first frame.
    let fit = WindowFit::new(SIZE, false, None, 1.0).with_native_fit(true);
    let options = fit.native_options("Screamer Trilogy");
    let path = settings_path.to_path_buf();
    let text = text.to_string();
    let result = eframe::run_native(
        "Screamer Trilogy",
        options,
        Box::new(|cc| {
            opened = true;
            theme::install_with(&cc.egui_ctx, &PALETTE, &METRICS);
            Ok(Box::new(Window::new(
                &cc.egui_ctx,
                fit,
                path,
                text,
                linux,
                &mut outcome,
            )))
        }),
    )
    .map_err(|e| e.to_string());
    finish(opened, result, outcome)
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
    focus_row: usize,
    pad: PadPoller,
    fit: WindowFit,
    trilogy: DiscoveredTrilogy,
    checked: String,
    machine: MachineCapabilities,
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
        fit: WindowFit,
        path: PathBuf,
        text: String,
        linux: bool,
        outcome: &'a mut Outcome,
    ) -> Self {
        let (settings, _) = Settings::parse_file(&text);
        let machine = MachineCapabilities::detect();
        let screen = Some(machine.display_res);
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

        let resolutions = form::resolutions(screen, form.picture.resolution);
        Self {
            checked: form.game.clone(),
            trilogy,
            form,
            tab: initial_tab,
            focus_row: 0,
            pad: PadPoller::default(),
            fit,
            machine,
            screen,
            resolutions,
            controllers: detect_controllers(),
            linux,
            covers: [
                widgets::load_jpeg(
                    ctx,
                    "cover_s1",
                    include_bytes!("../assets/cover_screamer1.jpg"),
                ),
                widgets::load_jpeg(
                    ctx,
                    "cover_s2",
                    include_bytes!("../assets/cover_screamer2.jpg"),
                ),
                widgets::load_jpeg(
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

    /// The first frame: the monitor's size in pixels (egui reports logical
    /// points) for the Steam Deck check and the resolution choices.
    fn measure_screen(&mut self, ctx: &egui::Context) {
        self.sized = true;
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
            let mon = form::physical_size((monitor.x, monitor.y), native);
            if mon.0 >= 640 && mon.1 >= 480 {
                self.screen = Some(mon);
                self.resolutions = form::resolutions(self.screen, self.form.picture.resolution);
            }
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

pub(crate) fn detect_controllers() -> Vec<String> {
    let mut out = Vec::new();
    #[cfg(target_os = "linux")]
    {
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
    }
    #[cfg(target_os = "windows")]
    {
        for idx in 0..4u32 {
            if win32_joy_poll(idx).is_some() {
                out.push(format!("XInput / WinMM Controller #{}", idx + 1));
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
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; \
             $d = New-Object System.Windows.Forms.FolderBrowserDialog; \
             $d.Description = '{}'; \
             if ($d.ShowDialog() -eq 'OK') {{ Write-Output $d.SelectedPath }}",
            title.replace('\'', "''")
        );
        if let Ok(out) = Command::new("powershell")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
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

impl eframe::App for Window<'_> {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.fit.first_frame(&ctx);
        if !self.sized {
            self.measure_screen(&ctx);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
        for nav in self.pad.poll() {
            self.handle_pad_action(nav, &ctx);
        }

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, CHASSIS);
        self.draw_box_art(ui, full);
        widgets::stripes(
            ui.painter(),
            full,
            &[(30.0, GOLD), (16.0, GOLD.gamma_multiply(0.65))],
        );

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
                    .font(decal(44.0))
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
            self.form.set_which(self.form.which, has_glide, self.screen);
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
        widgets::gradient(
            &p,
            Rect::from_min_max(
                pos2(rect.left() + rect.width() * 0.52, rect.top()),
                rect.max,
            ),
            CHASSIS,
            Fade::Right,
        );
        widgets::gradient(
            &p,
            Rect::from_min_max(pos2(rect.left(), rect.bottom() - 150.0), rect.max),
            CHASSIS,
            Fade::Down,
        );
        let x = rect.left() + 20.0;
        let y = rect.bottom() - 20.0;
        p.text(
            pos2(x, y),
            Align2::LEFT_BOTTOM,
            "Cover art: Virgin Interactive / Milestone",
            body(11.0),
            MUTED,
        );
        p.text(
            pos2(x, y - 16.0),
            Align2::LEFT_BOTTOM,
            self.form.which.studio_credit(),
            body(13.0),
            MUTED,
        );
        p.text(
            pos2(x, y - 34.0),
            Align2::LEFT_BOTTOM,
            self.form.which.display_name(),
            semibold(19.0),
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
        ui.label(egui::RichText::new(text).font(body(14.0)).color(color));
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        let tabs = [
            (Tab::Play, "Play"),
            (Tab::Picture, "Picture"),
            (Tab::Controls, "Controls"),
            (Tab::Buttons, "Buttons"),
        ];
        if widgets::tabs_bar(ui, &tabs, &mut self.tab) {
            self.focus_row = 0;
        }
    }

    fn play_tab(&mut self, ui: &mut egui::Ui) {
        let focus = self.focus_row;
        widgets::heading_focused(ui, "Game in the trilogy", focus == 0);
        ui.add_space(2.0);
        let gap = 10.0;
        let card_w = (ui.available_width() - 2.0 * gap) / 3.0;
        let mut chosen_game = None;
        let r0 = ui.horizontal(|ui| {
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
                let card = GameCard {
                    title: game.short_title(),
                    subtitle: &subtitle,
                    selected: self.form.which == game,
                    installed,
                };
                if widgets::game_card(ui, vec2(card_w, 64.0), &card).clicked() {
                    chosen_game = Some(game);
                }
            }
        });
        widgets::focus_bar(ui, r0.response.rect, focus == 0);
        if let Some(g) = chosen_game {
            let has_glide = self.trilogy.get(g).is_none_or(|d| d.can_use_dos_glide());
            self.form.set_which(g, has_glide, self.screen);
            self.error = None;
        }

        ui.add_space(8.0);
        widgets::heading(ui, "Game or trilogy folder");
        let hint = "Folder with Screamer, Screamer 2, Screamer Rally, or all three (e.g. .../screamer/games)";
        if widgets::folder_field(ui, &mut self.form.game, hint, || {
            pick_folder_dialog("Choose your Screamer or trilogy folder")
        }) {
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
        ui.label(egui::RichText::new(note).font(body(13.0)).color(color));
        ui.add_space(8.0);

        widgets::heading_focused(ui, "Look", focus == 1);
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
                "1080p Hor+, longer view, native 50 FPS.",
            ),
            (
                Look::SteamDeck,
                "Steam Deck",
                "1280×800 Hor+, longer view, native 50 FPS.",
            ),
            (Look::Custom, "Custom", "Pick each setting in Picture."),
        ];
        let look_w = (ui.available_width() - 3.0 * gap) / 4.0;
        let r1 = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (look, title, desc) in cards {
                let card = LookCard {
                    title,
                    note: desc,
                    selected: self.form.look == look,
                    badge: (look == best).then_some("Recommended"),
                    ..LookCard::default()
                };
                if widgets::look_card(ui, vec2(look_w, 96.0), &card).clicked() {
                    self.form.set_look(look, self.screen);
                    let has_glide = self.has_dos_glide_for_current();
                    self.form.set_which(self.form.which, has_glide, self.screen);
                    if look == Look::Custom {
                        self.tab = Tab::Picture;
                    }
                }
            }
        });
        widgets::focus_bar(ui, r1.response.rect, focus == 1);
        ui.add_space(8.0);

        widgets::heading(ui, "Saves");
        widgets::folder_field(
            ui,
            &mut self.form.saves,
            "Default per-game saves folder (~/.local/share/screamer-launcher/saves)",
            || pick_folder_dialog("Choose where your saves are kept"),
        );
    }

    fn picture_tab(&mut self, ui: &mut egui::Ui) {
        let before = self.form.picture;
        let which = self.form.which;
        let has_glide = self.has_dos_glide_for_current();
        let resolutions = &self.resolutions;
        let focus = self.focus_row;
        let p = &mut self.form.picture;

        let row =
            |ui: &mut egui::Ui, focused: bool, name: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                let resp = ui.horizontal_top(|ui| {
                    widgets::field_focused(ui, name, focused);
                    ui.vertical(|ui| add(ui));
                });
                widgets::focus_bar(ui, resp.response.rect, focused);
            };

        row(ui, focus == 0, "Renderer", &mut |ui| {
            let opts: Vec<Chip<'_, RendererChoice>> = RendererChoice::ALL
                .into_iter()
                .map(|r| Chip::new(r, r.label()).disabled(r.disabled_reason(which, has_glide)))
                .collect();
            widgets::chips_with(ui, &opts, &mut p.renderer);
        });

        row(ui, focus == 1, "Display", &mut |ui| {
            widgets::chips(ui, resolutions, &mut p.resolution, None);
        });

        row(ui, focus == 2, "Shader / Upscaler", &mut |ui| {
            widgets::chips(ui, &form::FILTERS, &mut p.texture_filter, None);
        });

        row(ui, focus == 3, "Frame rate", &mut |ui| {
            widgets::chips(ui, &form::FRAME_RATES, &mut p.fps_limit, None);
        });

        row(ui, focus == 4, "Vsync", &mut |ui| {
            widgets::chips(ui, &form::VSYNCS, &mut p.vsync, None);
        });

        row(ui, focus == 5, "", &mut |ui| {
            widgets::toggle(
                ui,
                &mut p.widescreen,
                "Widescreen: 16:9 Hor+ 3D camera FOV (shows more to the sides)",
                None,
            );
        });

        row(ui, focus == 6, "", &mut |ui| {
            widgets::toggle(
                ui,
                &mut p.extended_draw_distance,
                "Extended draw distance: reduce scenery and animated-object pop-in",
                None,
            );
        });

        ui.add_space(8.0);
        let is_3dfx = which != Game::Screamer
            && has_glide
            && matches!(p.renderer, RendererChoice::Auto | RendererChoice::Voodoo);
        ui.label(
            egui::RichText::new(self.machine.summary_line(is_3dfx))
                .font(body(13.0))
                .color(GOLD),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Note: 3dfx Voodoo renders internally at 1280×960 (2x supersampled) and SVGA at 640×480. Sharp is the stable pixel-preserving default; FSR, Smooth, and experimental 100 FPS optical-flow Frame Gen remain available.",
            )
            .font(body(13.0))
            .color(MUTED),
        );
        let _ = self.linux;

        if self.form.picture != before {
            self.form.look = Form::look_of(&self.form.picture, self.screen);
        }
    }

    fn controls_tab(&mut self, ui: &mut egui::Ui) {
        let focus = self.focus_row;
        let r0 = ui.horizontal_top(|ui| {
            widgets::toggle(
                ui,
                &mut self.form.gamepad_axes,
                "Gamepad: proportional analog steering, L2/R2 trigger pedals, D-pad menus",
                None,
            );
        });
        widgets::focus_bar(ui, r0.response.rect, focus == 0);
        ui.add_space(6.0);

        let pad_off_reason = (!self.form.gamepad_axes)
            .then_some("Turn on Gamepad above to configure steering deadzone and rumble.");

        let r1 = ui.horizontal_top(|ui| {
            widgets::field_focused(ui, "Deadzone", focus == 1);
            widgets::chips(
                ui,
                &form::DEADZONES,
                &mut self.form.deadzone,
                pad_off_reason,
            );
        });
        widgets::focus_bar(ui, r1.response.rect, focus == 1);

        let r2 = ui.horizontal_top(|ui| {
            widgets::field_focused(ui, "Rumble", focus == 2);
            widgets::chips(ui, &form::RUMBLES, &mut self.form.rumble, pad_off_reason);
        });
        widgets::focus_bar(ui, r2.response.rect, focus == 2);

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
        ui.label(egui::RichText::new(pad_status).font(body(13.0)).color(
            if self.controllers.is_empty() {
                MUTED
            } else {
                GOLD
            },
        ));

        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(
                "Keyboard arrows (Up/Down/Left/Right) and gamepad work simultaneously in menus and races.",
            )
            .font(body(13.0))
            .color(MUTED),
        );
    }

    fn buttons_tab(&mut self, ui: &mut egui::Ui) {
        let focus = self.focus_row;
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
                "RT/LT pedals + Xbox / Steam Deck layout.",
                true,
            ),
            (Layout::Custom, "Custom", "Your own button choices.", false),
        ];
        let gap = 12.0;
        let w = (ui.available_width() - 2.0 * gap) / 3.0;
        ui.add_space(4.0);
        let r0 = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (layout, title, note, recommended) in cards {
                let card = LookCard {
                    title,
                    note,
                    selected: self.form.layout == layout,
                    badge: recommended.then_some("Recommended"),
                    ..LookCard::default()
                };
                if widgets::look_card(ui, vec2(w, 68.0), &card).clicked() && self.form.gamepad_axes
                {
                    self.form.set_layout(layout);
                }
            }
        });
        widgets::focus_bar(ui, r0.response.rect, focus == 0);
        ui.label(
            egui::RichText::new(
                "RT/LT (R2/L2) analog triggers & A/X buttons work together in Modern mode. D-pad / Left Stick navigate menus.",
            )
            .font(body(13.0))
            .color(MUTED),
        );
        ui.spacing_mut().item_spacing.y = 4.0;
        let pad_off_reason =
            (!self.form.gamepad_axes).then_some("Turn on Gamepad in the Controls tab first.");
        let options: Vec<Chip<'_, u8>> = form::PAD_BUTTONS
            .iter()
            .map(|&(b, l)| Chip::new(b, l).disabled(pad_off_reason))
            .collect();
        for (row_idx, (k, name)) in form::ACTIONS.into_iter().enumerate() {
            let row_focused = focus == row_idx + 1;
            let r = ui.horizontal_top(|ui| {
                widgets::field_focused(ui, name, row_focused);
                let mut button = self.form.pad_button(k);
                widgets::chips_sized(ui, &options, &mut button, ChipSize::Compact);
                if self.form.gamepad_axes {
                    self.form.set_pad_button(k, button);
                }
            });
            widgets::focus_bar(ui, r.response.rect, row_focused);
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui, rect: Rect, ctx: &egui::Context) {
        let hint = format!(
            "Settings are saved when you press Play ({}).",
            self.form.which.display_name()
        );
        if widgets::footer(ui, rect, &hint, self.ready()).clicked() {
            self.play(ctx);
        }
    }

    fn max_focus_row(&self) -> usize {
        match self.tab {
            Tab::Play => 1,
            Tab::Picture => 6,
            Tab::Controls => 2,
            Tab::Buttons => form::ACTIONS.len(),
        }
    }

    fn handle_pad_action(&mut self, nav: Nav, ctx: &egui::Context) {
        let tabs = [Tab::Play, Tab::Picture, Tab::Controls, Tab::Buttons];
        match nav {
            Nav::Play => self.play(ctx),
            // B has no step of its own in this window.
            Nav::Back => {}
            Nav::PreviousTab => {
                let idx = tabs.iter().position(|&t| t == self.tab).unwrap_or(0);
                self.tab = tabs[(idx + tabs.len() - 1) % tabs.len()];
                self.focus_row = 0;
            }
            Nav::NextTab => {
                let idx = tabs.iter().position(|&t| t == self.tab).unwrap_or(0);
                self.tab = tabs[(idx + 1) % tabs.len()];
                self.focus_row = 0;
            }
            Nav::Up => {
                let max = self.max_focus_row();
                self.focus_row = if self.focus_row == 0 {
                    max
                } else {
                    self.focus_row - 1
                };
            }
            Nav::Down => {
                let max = self.max_focus_row();
                self.focus_row = if self.focus_row >= max {
                    0
                } else {
                    self.focus_row + 1
                };
            }
            Nav::Left => self.step_focused_option(-1),
            Nav::Right => self.step_focused_option(1),
            Nav::Press => match (self.tab, self.focus_row) {
                (Tab::Play, 0) => self.play(ctx),
                (Tab::Play, 1) if self.form.look == Look::Custom => {
                    self.tab = Tab::Picture;
                    self.focus_row = 0;
                }
                (Tab::Picture, 5) => {
                    self.form.picture.widescreen = !self.form.picture.widescreen;
                    self.form.look = Form::look_of(&self.form.picture, self.screen);
                }
                (Tab::Picture, 6) => {
                    self.form.picture.extended_draw_distance =
                        !self.form.picture.extended_draw_distance;
                    self.form.look = Form::look_of(&self.form.picture, self.screen);
                }
                (Tab::Controls, 0) => {
                    self.form.gamepad_axes = !self.form.gamepad_axes;
                }
                _ => self.step_focused_option(1),
            },
        }
    }

    fn step_focused_option(&mut self, dir: isize) {
        match (self.tab, self.focus_row) {
            (Tab::Play, 0) => {
                let idx = Game::ALL
                    .iter()
                    .position(|&g| g == self.form.which)
                    .unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(Game::ALL.len() as isize) as usize;
                let g = Game::ALL[next];
                let has_glide = self.trilogy.get(g).is_none_or(|d| d.can_use_dos_glide());
                self.form.set_which(g, has_glide, self.screen);
                self.error = None;
            }
            (Tab::Play, 1) => {
                let looks = [
                    Look::Original,
                    Look::Enhanced,
                    Look::SteamDeck,
                    Look::Custom,
                ];
                let idx = looks.iter().position(|&l| l == self.form.look).unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(looks.len() as isize) as usize;
                let look = looks[next];
                self.form.set_look(look, self.screen);
                let has_glide = self.has_dos_glide_for_current();
                self.form.set_which(self.form.which, has_glide, self.screen);
            }
            (Tab::Picture, 0) => {
                let which = self.form.which;
                let has_glide = self.has_dos_glide_for_current();
                let supported: Vec<RendererChoice> = RendererChoice::ALL
                    .into_iter()
                    .filter(|r| r.is_supported_for(which, has_glide))
                    .collect();
                if !supported.is_empty() {
                    let idx = supported
                        .iter()
                        .position(|&r| r == self.form.picture.renderer)
                        .unwrap_or(0) as isize;
                    let next = (idx + dir).rem_euclid(supported.len() as isize) as usize;
                    self.form.picture.renderer = supported[next];
                    self.form.look = Form::look_of(&self.form.picture, self.screen);
                }
            }
            (Tab::Picture, 1) if !self.resolutions.is_empty() => {
                let idx = self
                    .resolutions
                    .iter()
                    .position(|(r, _)| *r == self.form.picture.resolution)
                    .unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(self.resolutions.len() as isize) as usize;
                self.form.picture.resolution = self.resolutions[next].0;
                self.form.look = Form::look_of(&self.form.picture, self.screen);
            }
            (Tab::Picture, 2) => {
                let idx = form::FILTERS
                    .iter()
                    .position(|(f, _)| *f == self.form.picture.texture_filter)
                    .unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(form::FILTERS.len() as isize) as usize;
                self.form.picture.texture_filter = form::FILTERS[next].0;
                self.form.look = Form::look_of(&self.form.picture, self.screen);
            }
            (Tab::Picture, 3) => {
                let idx = form::FRAME_RATES
                    .iter()
                    .position(|(f, _)| *f == self.form.picture.fps_limit)
                    .unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(form::FRAME_RATES.len() as isize) as usize;
                self.form.picture.fps_limit = form::FRAME_RATES[next].0;
                self.form.look = Form::look_of(&self.form.picture, self.screen);
            }
            (Tab::Picture, 4) => {
                let idx = form::VSYNCS
                    .iter()
                    .position(|(v, _)| *v == self.form.picture.vsync)
                    .unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(form::VSYNCS.len() as isize) as usize;
                self.form.picture.vsync = form::VSYNCS[next].0;
                self.form.look = Form::look_of(&self.form.picture, self.screen);
            }
            (Tab::Picture, 5) => {
                self.form.picture.widescreen = !self.form.picture.widescreen;
                self.form.look = Form::look_of(&self.form.picture, self.screen);
            }
            (Tab::Picture, 6) => {
                self.form.picture.extended_draw_distance =
                    !self.form.picture.extended_draw_distance;
                self.form.look = Form::look_of(&self.form.picture, self.screen);
            }
            (Tab::Controls, 0) => {
                self.form.gamepad_axes = !self.form.gamepad_axes;
            }
            (Tab::Controls, 1) if self.form.gamepad_axes => {
                let idx = form::DEADZONES
                    .iter()
                    .position(|(d, _)| *d == self.form.deadzone)
                    .unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(form::DEADZONES.len() as isize) as usize;
                self.form.deadzone = form::DEADZONES[next].0;
            }
            (Tab::Controls, 2) if self.form.gamepad_axes => {
                let idx = form::RUMBLES
                    .iter()
                    .position(|(r, _)| *r == self.form.rumble)
                    .unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(form::RUMBLES.len() as isize) as usize;
                self.form.rumble = form::RUMBLES[next].0;
            }
            (Tab::Buttons, 0) if self.form.gamepad_axes => {
                let layouts = [Layout::Original, Layout::Modern, Layout::Custom];
                let idx = layouts
                    .iter()
                    .position(|&l| l == self.form.layout)
                    .unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(layouts.len() as isize) as usize;
                self.form.set_layout(layouts[next]);
            }
            (Tab::Buttons, r)
                if self.form.gamepad_axes && (1..=form::ACTIONS.len()).contains(&r) =>
            {
                let (k, _) = form::ACTIONS[r - 1];
                let cur = self.form.pad_button(k);
                let idx = form::PAD_BUTTONS
                    .iter()
                    .position(|(b, _)| *b == cur)
                    .unwrap_or(0) as isize;
                let next = (idx + dir).rem_euclid(form::PAD_BUTTONS.len() as isize) as usize;
                self.form.set_pad_button(k, form::PAD_BUTTONS[next].0);
            }
            _ => {}
        }
    }
}

/// The pad, read as XInput button bits ([`rea_launcher::pad`]): the
/// joystick device on Linux, WinMM on Windows.
#[derive(Default)]
struct PadPoller {
    nav: PadNav,
    #[cfg(target_os = "linux")]
    js_file: Option<std::fs::File>,
    #[cfg(target_os = "linux")]
    buttons: [bool; 16],
    #[cfg(target_os = "linux")]
    axes: [i16; 8],
}

impl PadPoller {
    fn poll(&mut self) -> Vec<Nav> {
        let now = self.read_mask();
        self.nav.update(now)
    }

    #[cfg(target_os = "linux")]
    fn read_mask(&mut self) -> u16 {
        use std::io::Read;
        use std::os::unix::fs::OpenOptionsExt;

        if self.js_file.is_none() {
            for idx in 0..4 {
                let path = format!("/dev/input/js{idx}");
                if let Ok(f) = std::fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(0o4000) // O_NONBLOCK
                    .open(&path)
                {
                    self.buttons = [false; 16];
                    self.axes = [0; 8];
                    self.js_file = Some(f);
                    break;
                }
            }
        }

        if let Some(f) = &mut self.js_file {
            let mut buf = [0u8; 8];
            loop {
                match f.read_exact(&mut buf) {
                    Ok(()) => {
                        let value = i16::from_le_bytes([buf[4], buf[5]]);
                        let ev_type = buf[6] & !0x80; // strip JS_EVENT_INIT
                        let number = buf[7] as usize;
                        if ev_type == 0x01 && number < self.buttons.len() {
                            self.buttons[number] = value != 0;
                        } else if ev_type == 0x02 && number < self.axes.len() {
                            self.axes[number] = value;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        self.js_file = None;
                        self.buttons = [false; 16];
                        self.axes = [0; 8];
                        break;
                    }
                }
            }
        }

        js_mask(&self.buttons, &self.axes)
    }

    #[cfg(target_os = "windows")]
    fn read_mask(&mut self) -> u16 {
        for idx in 0..4u32 {
            if let Some(mask) = win32_joy_poll(idx) {
                return mask;
            }
        }
        0
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    fn read_mask(&mut self) -> u16 {
        0
    }
}

/// One WinMM joystick's state as XInput bits. WinMM (not XInput) because
/// it also sees DirectInput/HID pads, which DOSBox (SDL) drives in the game.
#[cfg(target_os = "windows")]
fn win32_joy_poll(idx: u32) -> Option<u16> {
    crate::win32::joystick(idx).map(|j| joy_mask(j.buttons, j.x, j.y, j.pov))
}

/// WinMM buttons, stick (0–65535, centre 32767) and POV hat (hundredths
/// of a degree, 0xFFFF centred) as XInput bits.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn joy_mask(buttons: u32, x: u32, y: u32, pov: u32) -> u16 {
    let mut m = 0u16;
    for (button, bit) in PAD_BUTTON_BITS {
        if buttons & (1 << button) != 0 {
            m |= bit;
        }
    }
    if y < 16384 || (pov <= 4500 || (31500..36000).contains(&pov)) {
        m |= bits::DPAD_UP;
    }
    if y > 49152 || (13500..=22500).contains(&pov) {
        m |= bits::DPAD_DOWN;
    }
    if x < 16384 || (22500..=31500).contains(&pov) {
        m |= bits::DPAD_LEFT;
    }
    if x > 49152 || (4500..=13500).contains(&pov) {
        m |= bits::DPAD_RIGHT;
    }
    m
}

/// Joystick buttons (Linux `js` numbers, WinMM bits: an Xbox-style pad's
/// A, B, LB, RB, Start) and the XInput bits they stand for.
const PAD_BUTTON_BITS: [(usize, u16); 5] = [
    (0, bits::A),
    (1, bits::B),
    (4, bits::LEFT_SHOULDER),
    (5, bits::RIGHT_SHOULDER),
    (7, bits::START),
];

/// A Linux joystick's state as XInput bits: the buttons above, and the
/// left stick (axes 0/1) or d-pad hat (axes 6/7) past half way as d-pad.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn js_mask(buttons: &[bool; 16], axes: &[i16; 8]) -> u16 {
    let lean = bits::STICK_THRESHOLD;
    let mut m = 0u16;
    for (button, bit) in PAD_BUTTON_BITS {
        if buttons[button] {
            m |= bit;
        }
    }
    for (pressed, bit) in [
        (axes[1] < -lean || axes[7] < -lean, bits::DPAD_UP),
        (axes[1] > lean || axes[7] > lean, bits::DPAD_DOWN),
        (axes[0] < -lean || axes[6] < -lean, bits::DPAD_LEFT),
        (axes[0] > lean || axes[6] > lean, bits::DPAD_RIGHT),
    ] {
        if pressed {
            m |= bit;
        }
    }
    m
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_keeps_its_contrast() {
        assert_eq!(
            theme::contrast_problems(&PALETTE, &METRICS),
            Vec::<String>::new()
        );
        assert_eq!(theme::text_size_problems(&METRICS), Vec::<String>::new());
    }

    #[test]
    fn a_winmm_joystick_becomes_xinput_bits() {
        let centre = 32767;
        assert_eq!(joy_mask(0, centre, centre, 0xFFFF), 0);
        assert_eq!(
            joy_mask(1 | 1 << 7, centre, 0, 0xFFFF),
            bits::A | bits::START | bits::DPAD_UP
        );
        assert_eq!(joy_mask(0, centre, centre, 9000), bits::DPAD_RIGHT);
        assert_eq!(
            joy_mask(0, 65535, centre, 18000),
            bits::DPAD_RIGHT | bits::DPAD_DOWN
        );
    }

    #[test]
    fn a_linux_joystick_becomes_xinput_bits() {
        let mut buttons = [false; 16];
        let mut axes = [0i16; 8];
        assert_eq!(js_mask(&buttons, &axes), 0);
        buttons[0] = true;
        buttons[7] = true;
        axes[1] = -20_000; // stick up
        axes[6] = 32_767; // hat right
        axes[0] = 15_000; // not past half way
        assert_eq!(
            js_mask(&buttons, &axes),
            bits::A | bits::START | bits::DPAD_UP | bits::DPAD_RIGHT
        );
        // Start plays first; held buttons do nothing more.
        let mut nav = PadNav::default();
        assert_eq!(nav.update(js_mask(&buttons, &axes))[0], Nav::Play);
        assert!(nav.update(js_mask(&buttons, &axes)).is_empty());
    }

    #[test]
    fn only_a_window_that_never_opened_falls_back_to_saved_settings() {
        assert_eq!(finish(true, Ok(()), Outcome::Play), Ok(Outcome::Play));
        assert_eq!(finish(false, Ok(()), Outcome::Closed), Ok(Outcome::Closed));
        assert_eq!(
            finish(false, Err("no display".into()), Outcome::Closed),
            Err(ShowError::NoWindow("no display".into()))
        );
        // Failing after the player pressed Play or Close is not "no window".
        assert_eq!(
            finish(true, Err("lost context".into()), Outcome::Play),
            Err(ShowError::Failed("lost context".into()))
        );
    }
}
