//! The unified settings window (`egui` / `eframe`) for the Screamer trilogy:
//! **Screamer (1995)**, **Screamer 2 (1996)**, and **Screamer Rally (1997)**.
//!
//! The chosen game's box art fills the left strip and fades into the navy
//! chassis; slanted gold livery stripes top right, the game's name as a
//! condensed italic decal, underline tabs, look cards with a gold
//! `Recommended` badge. Choices a game does not have are greyed out with
//! the reason. Every control comes from an option row
//! ([`screamer_launcher::config::OPTIONS`]): its label names it, its
//! description is the note under it, and the window records the keys it
//! drew for the parity test.
//!
//! Keyboard: Tab moves between controls, ←/→ change a chip row, Space
//! toggles, Enter plays. Gamepad: d-pad up/down moves between rows,
//! left/right changes the row, A toggles, LB/RB switch tabs, Start plays.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use rea_launcher::cfg::spec::OptionSpec;
use rea_launcher::pad::{self as bits, Nav, PadNav};
use rea_launcher_ui::eframe;
use rea_launcher_ui::egui::{
    self, Align2, Color32, Key, Rect, Response, TextureHandle, Vec2, pos2, vec2,
};
use rea_launcher_ui::spec as spec_ui;
use rea_launcher_ui::theme::{self, Metrics, Palette, body, decal, semibold};
use rea_launcher_ui::widgets::{self, Chip, ChipSize, Fade, GameCard, LookCard};
use rea_launcher_ui::window::WindowFit;

use screamer_launcher::config::{self, MachineCapabilities, RendererChoice, Scaler, Settings};
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

/// The credit under the box art (owner decision 2026-11-02; THIRD_PARTY.md).
const ART_CREDIT: &str = "Box art: Virgin Interactive. Scans: MobyGames";

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
    let outcome = Rc::new(Cell::new(Outcome::Closed));
    let mut opened = false;
    // Native on Linux and Windows (never Wine): created at 1x, fitted to
    // the screen and its scale on the first frame.
    let fit = WindowFit::new(SIZE, false, None, 1.0).with_native_fit(true);
    let options = fit.native_options("Screamer Trilogy");
    let path = settings_path.to_path_buf();
    let text = text.to_string();
    let shared = outcome.clone();
    let result = eframe::run_native(
        "Screamer Trilogy",
        options,
        Box::new(|cc| {
            opened = true;
            theme::install_with(&cc.egui_ctx, &PALETTE, &METRICS);
            let machine = MachineCapabilities::detect();
            let mut window = Window::new(
                path,
                text,
                linux,
                machine.display_res,
                detect_controllers(),
                shared,
            );
            window.tab = initial_tab();
            Ok(Box::new(App { fit, window }))
        }),
    )
    .map_err(|e| e.to_string());
    finish(opened, result, outcome.get())
}

/// `SCREAMER_TAB` (picture, controls, buttons) opens that tab first (for
/// screenshots).
fn initial_tab() -> Tab {
    match std::env::var("SCREAMER_TAB")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "picture" => Tab::Picture,
        "controls" => Tab::Controls,
        "buttons" => Tab::Buttons,
        _ => Tab::Play,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Play,
    Picture,
    Controls,
    Buttons,
}

const TABS: [(Tab, &str); 4] = [
    (Tab::Play, "Play"),
    (Tab::Picture, "Picture"),
    (Tab::Controls, "Controls"),
    (Tab::Buttons, "Buttons"),
];

/// A row the gamepad can focus (d-pad up/down), in the order drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Game,
    Look,
    Renderer,
    Display,
    Shader,
    Scaler,
    Widescreen,
    DrawDistance,
    Vsync,
    FrameGeneration,
    Gamepad,
    Deadzone,
    Rumble,
    Layout,
    /// One of [`form::ACTIONS`].
    Action(usize),
}

/// The eframe app: the window plus its sizing.
struct App {
    fit: WindowFit,
    window: Window,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.fit.first_frame(ui.ctx());
        for nav in self.window.pad.poll() {
            self.window.handle_pad_action(nav, &ui.ctx().clone());
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
        self.window.draw(ui);
    }
}

struct Window {
    form: Form,
    tab: Tab,
    focus_row: usize,
    pad: PadPoller,
    trilogy: DiscoveredTrilogy,
    checked: String,
    screen: Option<(u32, u32)>,
    resolutions: Vec<(Option<(u32, u32)>, String)>,
    controllers: Vec<String>,
    linux: bool,
    covers: Option<[Option<TextureHandle>; 3]>,
    path: PathBuf,
    text: String,
    error: Option<String>,
    sized: bool,
    outcome: Rc<Cell<Outcome>>,
}

impl Window {
    fn new(
        path: PathBuf,
        text: String,
        linux: bool,
        display: (u32, u32),
        controllers: Vec<String>,
        outcome: Rc<Cell<Outcome>>,
    ) -> Self {
        let (settings, _) = Settings::parse_file(&text);
        let screen = Some(display);
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

        let resolutions = form::resolutions(screen, form.picture.resolution);
        Self {
            checked: form.game.clone(),
            trilogy,
            form,
            tab: Tab::Play,
            focus_row: 0,
            pad: PadPoller::default(),
            screen,
            resolutions,
            controllers,
            linux,
            covers: None,
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
        match rea_launcher::fsx::write_atomic(&self.path, saved) {
            Ok(()) => {
                self.outcome.set(Outcome::Play);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(e) => self.error = Some(format!("Can't save settings: {e}")),
        }
    }

    /// The rows of the current tab the gamepad steps through.
    fn rows(&self) -> Vec<Row> {
        match self.tab {
            Tab::Play => vec![Row::Game, Row::Look],
            Tab::Picture => {
                let mut rows = vec![Row::Renderer, Row::Display, Row::Shader];
                if self.linux {
                    rows.push(Row::Scaler);
                }
                rows.extend([
                    Row::Widescreen,
                    Row::DrawDistance,
                    Row::Vsync,
                    Row::FrameGeneration,
                ]);
                rows
            }
            Tab::Controls => vec![Row::Gamepad, Row::Deadzone, Row::Rumble],
            Tab::Buttons => std::iter::once(Row::Layout)
                .chain((0..form::ACTIONS.len()).map(Row::Action))
                .collect(),
        }
    }

    fn focused(&self, row: Row) -> bool {
        self.rows().get(self.focus_row) == Some(&row)
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.sized {
            self.measure_screen(&ctx);
        }
        if self.covers.is_none() {
            self.covers = Some(load_covers(&ctx));
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
            if widgets::tabs_bar(ui, &TABS, &mut self.tab) {
                self.focus_row = 0;
            }
            ui.add_space(12.0);
            let tab = self.tab;
            widgets::tab_body(ui, format!("{tab:?}"), |ui| match tab {
                Tab::Play => self.play_tab(ui),
                Tab::Picture => self.picture_tab(ui),
                Tab::Controls => self.controls_tab(ui),
                Tab::Buttons => self.buttons_tab(ui),
            });
        });

        let footer = Rect::from_min_max(
            pos2(content.left(), content.bottom() - footer_h),
            content.max,
        );
        let hint = format!(
            "Settings are saved when you press Play ({}).",
            self.form.which.display_name()
        );
        if widgets::footer(ui, footer, &hint, self.ready()).clicked() {
            self.play(&ctx);
        }

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

    fn draw_box_art(&self, ui: &egui::Ui, full: Rect) {
        let rect = Rect::from_min_size(full.min, vec2(PHOTO_W, full.height()));
        let p = ui.painter().with_clip_rect(rect);
        if let Some(Some(tex)) = self.covers.as_ref().map(|c| &c[self.form.which.index()]) {
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
        // Fades into the chassis: the text below sits on solid navy.
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
            Rect::from_min_max(pos2(rect.left(), rect.bottom() - 190.0), rect.max),
            CHASSIS,
            Fade::Down,
        );
        let x = rect.left() + 20.0;
        let y = rect.bottom() - 20.0;
        p.text(
            pos2(x, y),
            Align2::LEFT_BOTTOM,
            ART_CREDIT,
            body(11.0),
            MUTED,
        );
        p.text(
            pos2(x, y - 18.0),
            Align2::LEFT_BOTTOM,
            self.form.which.studio_credit(),
            body(13.0),
            MUTED,
        );
        p.text(
            pos2(x, y - 36.0),
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
            (format!("Ready: {exe_summary}. Press Play or Enter."), MUTED)
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

    fn play_tab(&mut self, ui: &mut egui::Ui) {
        let gap = 10.0;
        widgets::heading_focused(ui, "Game in the trilogy", self.focused(Row::Game));
        ui.add_space(2.0);
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
                        format!("{}, {mode}", game.year())
                    }
                    None if self.form.game.trim().is_empty() => {
                        format!("{}, set the folder below", game.year())
                    }
                    None => format!("{}, not in folder", game.year()),
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
        spec_ui::mark_rendered(ui.ctx(), "which");
        widgets::focus_bar(ui, r0.response.rect, self.focused(Row::Game));
        if let Some(g) = chosen_game {
            self.choose_game(g);
        }

        ui.add_space(8.0);
        let row = config::option("game");
        widgets::heading(ui, row.label);
        let hint = "Folder with Screamer, Screamer 2, Screamer Rally, or all three";
        if widgets::folder_field(ui, &mut self.form.game, hint, || {
            pick_folder_dialog("Choose your Screamer or trilogy folder")
        }) {
            self.error = None;
        }
        spec_ui::mark_rendered(ui.ctx(), "game");
        let (note, color) = if self.form.game.trim().is_empty() {
            (row.description.to_string(), MUTED)
        } else if self.trilogy.any_found() {
            let found_names: Vec<&str> = Game::ALL
                .into_iter()
                .filter(|&g| self.trilogy.get(g).is_some())
                .map(|g| g.display_name())
                .collect();
            (format!("Found: {}", found_names.join(", ")), GOLD)
        } else if let Some(unknown) = self.trilogy.unknown.first() {
            (unknown.message(), ERROR)
        } else {
            (
                "No Screamer, Screamer 2 or Screamer Rally in this folder.".to_string(),
                ERROR,
            )
        };
        ui.label(egui::RichText::new(note).font(body(13.0)).color(color));
        ui.add_space(8.0);

        widgets::heading_focused(ui, "Look", self.focused(Row::Look));
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
                "1080p widescreen, longer view, crisp pixels.",
            ),
            (
                Look::SteamDeck,
                "Steam Deck",
                "1280×800 widescreen, longer view, crisp pixels.",
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
                    self.choose_look(look);
                }
            }
        });
        widgets::focus_bar(ui, r1.response.rect, self.focused(Row::Look));
        ui.add_space(8.0);

        let row = config::option("saves");
        widgets::heading(ui, row.label);
        widgets::folder_field(
            ui,
            &mut self.form.saves,
            "Default: the launcher's own saves folder",
            || pick_folder_dialog("Choose where your saves are kept"),
        );
        spec_ui::mark_rendered(ui.ctx(), "saves");
        widgets::note(ui, row.description);
    }

    fn picture_tab(&mut self, ui: &mut egui::Ui) {
        let before = self.form.picture;
        let which = self.form.which;
        let has_glide = self.has_dos_glide_for_current();

        let renderers: Vec<Chip<'_, RendererChoice>> = RendererChoice::ALL
            .into_iter()
            .map(|r| Chip::new(r, r.label()).disabled(r.disabled_reason(which, has_glide)))
            .collect();
        let focus = [
            self.focused(Row::Renderer),
            self.focused(Row::Display),
            self.focused(Row::Shader),
            self.focused(Row::Scaler),
            self.focused(Row::Widescreen),
            self.focused(Row::DrawDistance),
            self.focused(Row::Vsync),
            self.focused(Row::FrameGeneration),
        ];
        let mut help = None;
        let p = &mut self.form.picture;
        help = help.or(chip_row(ui, config::option("renderer"), focus[0], |ui| {
            widgets::chips_with(ui, &renderers, &mut p.renderer)
        }));
        let resolutions = &self.resolutions;
        help = help.or(chip_row(ui, config::option("resolution"), focus[1], |ui| {
            widgets::chips(ui, resolutions, &mut p.resolution, None)
        }));
        help = help.or(chip_row(
            ui,
            config::option("texture_filter"),
            focus[2],
            |ui| widgets::chips(ui, &form::FILTERS, &mut p.texture_filter, None),
        ));
        if self.linux {
            let scalers: Vec<(Scaler, &str)> =
                form::SCALERS.iter().map(|(s, l, _)| (*s, *l)).collect();
            let scaler = &mut self.form.scaler;
            help = help.or(chip_row(ui, config::option("scaler"), focus[3], |ui| {
                widgets::chips(ui, &scalers, scaler, None)
            }));
        }

        // Two pairs of switches: the shape of the picture, then its motion.
        ui.add_space(4.0);
        let p = &mut self.form.picture;
        let mut cells = [None; 4];
        ui.columns(2, |cols| {
            cells[0] = toggle_cell(&mut cols[0], "widescreen", &mut p.widescreen, focus[4]);
            cells[1] = toggle_cell(
                &mut cols[1],
                "extended_draw_distance",
                &mut p.extended_draw_distance,
                focus[5],
            );
        });
        ui.columns(2, |cols| {
            cells[2] = toggle_cell(&mut cols[0], "vsync", &mut p.vsync, focus[6]);
            cells[3] = toggle_cell(
                &mut cols[1],
                "frame_generation",
                &mut p.frame_generation,
                focus[7],
            );
        });
        help = help.or(cells.into_iter().flatten().next());
        ui.add_space(10.0);
        help_board(
            ui,
            help,
            "The game draws at 640×480 (3dfx Voodoo: 1280×960); the shader enlarges it to the window. Point at a setting, or move to it, to read what it changes.",
        );

        if self.form.picture != before {
            self.form.look = Form::look_of(&self.form.picture, self.screen);
        }
    }

    fn controls_tab(&mut self, ui: &mut egui::Ui) {
        let focused = self.focused(Row::Gamepad);
        let mut help = toggle_cell(ui, "gamepad_axes", &mut self.form.gamepad_axes, focused);
        ui.add_space(6.0);

        let pad_off_reason = (!self.form.gamepad_axes)
            .then_some("Turn on Gamepad above to change the deadzone and rumble.");
        let deadzones = form::with_saved(&form::DEADZONES, self.form.deadzone);
        let focused = self.focused(Row::Deadzone);
        let deadzone = &mut self.form.deadzone;
        help = help.or(chip_row(ui, config::option("deadzone"), focused, |ui| {
            widgets::chips(ui, &deadzones, deadzone, pad_off_reason)
        }));
        let rumbles = form::with_saved(&form::RUMBLES, self.form.rumble);
        let focused = self.focused(Row::Rumble);
        let rumble = &mut self.form.rumble;
        help = help.or(chip_row(ui, config::option("rumble"), focused, |ui| {
            widgets::chips(ui, &rumbles, rumble, pad_off_reason)
        }));
        if let Some(reason) = pad_off_reason {
            widgets::note(ui, reason);
        }

        ui.add_space(8.0);
        let pad_status = if self.controllers.is_empty() {
            "No controller connected right now. Turn one on at any time, even mid-race, and it connects by itself.".to_string()
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
        help_board(
            ui,
            help,
            "Keyboard arrows and the gamepad work together in menus and races.",
        );
    }

    fn buttons_tab(&mut self, ui: &mut egui::Ui) {
        let orig_title = format!("Original {}", self.form.which.year());
        let cards = [
            (
                Layout::Original,
                orig_title.as_str(),
                "The 2-button DOS gameport layout.",
                false,
            ),
            (
                Layout::Modern,
                "Modern",
                "Trigger pedals, Xbox and Steam Deck layout.",
                true,
            ),
            (Layout::Custom, "Custom", "Your own button choices.", false),
        ];
        let pad_off_reason =
            (!self.form.gamepad_axes).then_some("Turn on Gamepad in the Controls tab first.");
        if let Some(reason) = pad_off_reason {
            ui.label(egui::RichText::new(reason).font(body(13.0)).color(ERROR));
        }
        let gap = 12.0;
        let w = (ui.available_width() - 2.0 * gap) / 3.0;
        ui.add_space(8.0);
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
                if widgets::look_card(ui, vec2(w, 68.0), &card).clicked()
                    && pad_off_reason.is_none()
                {
                    self.form.set_layout(layout);
                }
            }
        });
        spec_ui::mark_rendered(ui.ctx(), "buttons");
        widgets::focus_bar(ui, r0.response.rect, self.focused(Row::Layout));
        widgets::note(
            ui,
            "RT/LT and A/X both drive the pedals in Modern. The d-pad and left stick move through menus.",
        );
        ui.spacing_mut().item_spacing.y = 4.0;
        let options: Vec<Chip<'_, u8>> = form::PAD_BUTTONS
            .iter()
            .map(|&(b, l)| Chip::new(b, l).disabled(pad_off_reason))
            .collect();
        for (row_idx, (k, name)) in form::ACTIONS.into_iter().enumerate() {
            let row_focused = self.focused(Row::Action(row_idx));
            let r = ui.horizontal_top(|ui| {
                widgets::field_focused(ui, name, row_focused);
                let mut button = self.form.pad_button(k);
                let resp = widgets::chips_sized(ui, &options, &mut button, ChipSize::Compact);
                name_control(ui, &resp, name, "The gamepad button for this action.");
                if pad_off_reason.is_none() {
                    self.form.set_pad_button(k, button);
                }
            });
            widgets::focus_bar(ui, r.response.rect, row_focused);
        }
    }

    fn choose_game(&mut self, g: Game) {
        let has_glide = self.trilogy.get(g).is_none_or(|d| d.can_use_dos_glide());
        self.form.set_which(g, has_glide, self.screen);
        self.error = None;
    }

    fn choose_look(&mut self, look: Look) {
        self.form.set_look(look, self.screen);
        let has_glide = self.has_dos_glide_for_current();
        self.form.set_which(self.form.which, has_glide, self.screen);
        if look == Look::Custom {
            self.tab = Tab::Picture;
            self.focus_row = 0;
        }
    }

    fn handle_pad_action(&mut self, nav: Nav, ctx: &egui::Context) {
        let rows = self.rows().len();
        match nav {
            Nav::Play => self.play(ctx),
            // B has no step of its own in this window.
            Nav::Back => {}
            Nav::PreviousTab | Nav::NextTab => {
                let idx = TABS.iter().position(|(t, _)| *t == self.tab).unwrap_or(0);
                let by = if nav == Nav::NextTab {
                    1
                } else {
                    TABS.len() - 1
                };
                self.tab = TABS[(idx + by) % TABS.len()].0;
                self.focus_row = 0;
            }
            Nav::Up => self.focus_row = (self.focus_row + rows - 1) % rows,
            Nav::Down => self.focus_row = (self.focus_row + 1) % rows,
            Nav::Left => self.step_focused_option(-1),
            Nav::Right => self.step_focused_option(1),
            Nav::Press => match self.rows().get(self.focus_row) {
                Some(Row::Game) => self.play(ctx),
                Some(Row::Look) if self.form.look == Look::Custom => {
                    self.tab = Tab::Picture;
                    self.focus_row = 0;
                }
                _ => self.step_focused_option(1),
            },
        }
    }

    fn step_focused_option(&mut self, dir: isize) {
        let Some(&row) = self.rows().get(self.focus_row) else {
            return;
        };
        let pad_on = self.form.gamepad_axes;
        let screen = self.screen;
        let p = &mut self.form.picture;
        let mut picture_changed = true;
        match row {
            Row::Game => {
                let g = step(&Game::ALL, self.form.which, dir);
                self.choose_game(g);
                picture_changed = false;
            }
            Row::Look => {
                let looks = [
                    Look::Original,
                    Look::Enhanced,
                    Look::SteamDeck,
                    Look::Custom,
                ];
                let look = step(&looks, self.form.look, dir);
                self.form.set_look(look, screen);
                let has_glide = self.has_dos_glide_for_current();
                self.form.set_which(self.form.which, has_glide, screen);
                picture_changed = false;
            }
            Row::Renderer => {
                let which = self.form.which;
                let has_glide = self
                    .trilogy
                    .get(which)
                    .is_none_or(|d| d.can_use_dos_glide());
                let supported: Vec<RendererChoice> = RendererChoice::ALL
                    .into_iter()
                    .filter(|r| r.is_supported_for(which, has_glide))
                    .collect();
                if !supported.is_empty() {
                    p.renderer = step(&supported, p.renderer, dir);
                }
            }
            Row::Display => {
                let sizes: Vec<_> = self.resolutions.iter().map(|(r, _)| *r).collect();
                p.resolution = step(&sizes, p.resolution, dir);
            }
            Row::Shader => {
                let filters: Vec<_> = form::FILTERS.iter().map(|(f, _)| *f).collect();
                p.texture_filter = step(&filters, p.texture_filter, dir);
            }
            Row::Scaler => {
                self.form.scaler = step(&Scaler::ALL, self.form.scaler, dir);
                picture_changed = false;
            }
            Row::Widescreen => p.widescreen = !p.widescreen,
            Row::DrawDistance => p.extended_draw_distance = !p.extended_draw_distance,
            Row::Vsync => p.vsync = !p.vsync,
            Row::FrameGeneration => p.frame_generation = !p.frame_generation,
            Row::Gamepad => {
                self.form.gamepad_axes = !self.form.gamepad_axes;
                picture_changed = false;
            }
            Row::Deadzone if pad_on => {
                let values: Vec<u32> = form::with_saved(&form::DEADZONES, self.form.deadzone)
                    .into_iter()
                    .map(|(v, _)| v)
                    .collect();
                self.form.deadzone = step(&values, self.form.deadzone, dir);
                picture_changed = false;
            }
            Row::Rumble if pad_on => {
                let values: Vec<u32> = form::with_saved(&form::RUMBLES, self.form.rumble)
                    .into_iter()
                    .map(|(v, _)| v)
                    .collect();
                self.form.rumble = step(&values, self.form.rumble, dir);
                picture_changed = false;
            }
            Row::Layout if pad_on => {
                let layouts = [Layout::Original, Layout::Modern, Layout::Custom];
                let layout = step(&layouts, self.form.layout, dir);
                self.form.set_layout(layout);
                picture_changed = false;
            }
            Row::Action(r) if pad_on => {
                let (k, _) = form::ACTIONS[r];
                let buttons: Vec<u8> = form::PAD_BUTTONS.iter().map(|(b, _)| *b).collect();
                let next = step(&buttons, self.form.pad_button(k), dir);
                self.form.set_pad_button(k, next);
                picture_changed = false;
            }
            _ => picture_changed = false,
        }
        if picture_changed {
            self.form.look = Form::look_of(&self.form.picture, self.screen);
        }
    }
}

/// The value `dir` steps from `current` in `values` (wrapping; the first
/// when `current` is not there).
fn step<T: PartialEq + Copy>(values: &[T], current: T, dir: isize) -> T {
    let idx = values.iter().position(|v| *v == current).unwrap_or(0) as isize;
    values[(idx + dir).rem_euclid(values.len() as isize) as usize]
}

/// Names a control for screen readers (the chip row is a radio group).
fn name_control(ui: &egui::Ui, control: &Response, label: &str, description: &str) {
    let (label, description) = (label.to_owned(), description.to_owned());
    ui.ctx().accesskit_node_builder(control.id, |node| {
        node.set_label(label);
        node.set_description(description);
    });
}

/// A labelled chip row from an option row (recorded for the parity
/// test); the row when the pointer or the focus is on it, for the
/// [`help_board`].
fn chip_row(
    ui: &mut egui::Ui,
    row: &'static OptionSpec,
    focused: bool,
    chips: impl FnOnce(&mut egui::Ui) -> Response,
) -> Option<&'static OptionSpec> {
    spec_ui::mark_rendered(ui.ctx(), row.key);
    let mut active = focused;
    let r = ui.horizontal_top(|ui| {
        widgets::field_focused(ui, row.label, focused);
        let resp = chips(ui);
        name_control(ui, &resp, row.label, row.description);
        active |= resp.has_focus();
    });
    active |= ui.rect_contains_pointer(r.response.rect);
    widgets::focus_bar(ui, r.response.rect, focused);
    ui.add_space(4.0);
    active.then_some(row)
}

/// A switch from an option row (recorded for the parity test); the row
/// when the pointer or the focus is on it.
fn toggle_cell(
    ui: &mut egui::Ui,
    key: &str,
    on: &mut bool,
    focused: bool,
) -> Option<&'static OptionSpec> {
    let row = config::option(key);
    spec_ui::mark_rendered(ui.ctx(), row.key);
    let resp = widgets::toggle(ui, on, row.label, None);
    name_control(ui, &resp, row.label, row.description);
    widgets::focus_bar(ui, resp.rect, focused);
    (focused || resp.has_focus() || resp.hovered()).then_some(row)
}

/// The board under a tab's settings: what the setting under the pointer or
/// the focus changes (its label and description), else `idle`. Fixed
/// height, so the layout never jumps.
fn help_board(ui: &mut egui::Ui, row: Option<&OptionSpec>, idle: &str) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 58.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(6), PANEL);
    painter.rect_filled(
        Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
        egui::CornerRadius::same(1),
        GOLD,
    );
    let inner = rect.shrink2(vec2(14.0, 8.0));
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = inner.width();
    if let Some(row) = row {
        job.append(
            &format!("{}: ", row.label),
            0.0,
            egui::TextFormat::simple(semibold(theme::size::NOTE), INK),
        );
    }
    job.append(
        row.map_or(idle, |r| r.description),
        0.0,
        egui::TextFormat::simple(body(theme::size::NOTE), INK),
    );
    let galley = painter.layout_job(job);
    painter.galley(inner.min, galley, INK);
}

fn load_covers(ctx: &egui::Context) -> [Option<TextureHandle>; 3] {
    [
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
    ]
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
                out.push(format!("Controller #{}", idx + 1));
            }
        }
    }
    out
}

/// The folder picker: the SDK's native dialog on Windows (Unicode paths),
/// zenity or kdialog on Linux.
fn pick_folder_dialog(title: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        rea_launcher_win32::dialog::pick_folder(title)
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::process::Command;
        for (program, args) in [
            (
                "zenity",
                vec!["--file-selection", "--directory", "--title", title],
            ),
            (
                "kdialog",
                vec!["--getexistingdirectory", ".", "--title", title],
            ),
        ] {
            if let Ok(out) = Command::new(program).args(&args).output()
                && out.status.success()
            {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Some(PathBuf::from(s));
                }
            }
        }
        None
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
    use rea_launcher::cfg::spec as core_spec;
    use rea_launcher_ui::egui::accesskit::Role;
    use rea_launcher_ui::testing::{self, Harness, Queryable};

    /// A unique scratch folder.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("screamer-ui-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The window over `text`, headless, on a 2560×1440 screen.
    fn open(text: &str, linux: bool, name: &str) -> Harness<'static, Window> {
        let path = scratch(name).join("launcher.cfg");
        let window = Window::new(
            path,
            text.to_string(),
            linux,
            (2560, 1440),
            Vec::new(),
            Rc::new(Cell::new(Outcome::Closed)),
        );
        let mut ui = testing::harness(&PALETTE, &METRICS, window, |ui, w: &mut Window| {
            w.sized = true; // the harness has no monitor: keep the given screen
            w.draw(ui)
        });
        ui.run();
        ui
    }

    fn tab(ui: &mut Harness<'static, Window>, name: &str) {
        ui.get_by_role_and_label(Role::Tab, name).click();
        ui.run();
    }

    #[test]
    fn the_window_has_a_control_for_every_option_and_the_reverse() {
        let mut ui = open("", true, "parity");
        for name in ["Picture", "Controls", "Buttons", "Play"] {
            tab(&mut ui, name);
        }
        core_spec::assert_parity(config::OPTIONS, &spec_ui::rendered_keys(&ui.ctx));
        // The keys the window saves are the ones it shows.
        let mut shown = spec_ui::rendered_keys(&ui.ctx);
        shown.sort_unstable();
        let mut saved = form::WINDOW_KEYS.to_vec();
        saved.sort_unstable();
        assert_eq!(shown, saved);
    }

    /// Screamer 2 "installed" (fake paths, the real edition table), so the
    /// window is ready to play.
    fn install_screamer2(w: &mut Window) {
        use screamer_launcher::edition::EDITIONS;
        use screamer_launcher::runtime::{DiscoveredExe, DiscoveredGame};
        let executables = EDITIONS
            .iter()
            .filter(|e| e.game == Game::Screamer2)
            .map(|e| DiscoveredExe {
                path: PathBuf::from("/g").join(e.exe_name),
                file_name: e.exe_name.to_string(),
                edition: e,
                sha256: e.sha256,
            })
            .collect();
        w.trilogy.games[1] = Some(DiscoveredGame {
            root_dir: "/g".into(),
            exe_dir: "/g".into(),
            game: Game::Screamer2,
            executables,
            cd_image: None,
            glide2x_ovl_source: None,
            has_genuine_dos_glide2x: true,
            has_windows_stub_glide2x: false,
            unknown: Vec::new(),
        });
        w.form.game = "/g".into();
        w.checked = "/g".into();
        w.form.which = Game::Screamer2;
    }

    fn saved(ui: &Harness<'static, Window>) -> Settings {
        let text = std::fs::read_to_string(&ui.state().path).unwrap();
        let (s, warnings) = Settings::parse_file(&text);
        assert!(warnings.is_empty(), "{warnings:?}");
        s
    }

    fn play(ui: &mut Harness<'static, Window>) {
        ui.get_by_role_and_label(Role::Button, "Play").click();
        ui.run();
        assert_eq!(ui.state().outcome.get(), Outcome::Play);
    }

    #[test]
    fn every_tab_renders_with_named_controls() {
        let mut ui = open("", true, "tabs");
        for (name, controls) in [
            ("Play", &["Enhanced", "Steam Deck", "Custom"][..]),
            (
                "Picture",
                &[
                    "Renderer",
                    "Display",
                    "Shader / Upscaler",
                    "Scaler",
                    "Widescreen",
                    "Extended draw distance",
                    "Vsync",
                    "Frame generation (100 FPS)",
                ],
            ),
            ("Controls", &["Gamepad", "Deadzone", "Rumble"]),
            ("Buttons", &["Modern", "Accelerate", "Brake / Delete"]),
        ] {
            tab(&mut ui, name);
            for control in controls {
                assert!(
                    ui.query_all_by_label(control).next().is_some(),
                    "{name}: no control named {control}"
                );
            }
        }
        // A row's description is its accessible description.
        tab(&mut ui, "Picture");
        let vsync = ui.get_by_role_and_label(Role::Switch, "Vsync");
        assert_eq!(
            testing::description(&vsync).as_deref(),
            Some(config::option("vsync").description)
        );
    }

    #[test]
    fn clicking_controls_changes_what_play_saves() {
        let mut ui = open("", true, "clicks");
        install_screamer2(ui.state_mut());
        ui.run();
        tab(&mut ui, "Picture");
        ui.get_by_role_and_label(Role::Switch, "Frame generation (100 FPS)")
            .click();
        ui.run();
        ui.get_by_role_and_label(Role::Switch, "Vsync").click();
        ui.run();
        ui.get_by_label("SVGA 16-bit").click();
        ui.run();
        ui.get_by_label("Even pixels").click();
        ui.run();
        tab(&mut ui, "Controls");
        ui.get_by_label("Strong").click();
        ui.run();
        play(&mut ui);
        let s = saved(&ui);
        assert_eq!(s.frame_generation, Some(true));
        assert_eq!(s.vsync, Some(false));
        assert_eq!(s.renderer, Some(RendererChoice::Svga16Bit));
        assert_eq!(s.scaler, Some(Scaler::Pixel));
        assert_eq!(s.rumble, Some(100));
        assert_eq!(s.which, Some(Game::Screamer2));
        // Picking each setting makes the look Custom.
        assert_eq!(ui.state().form.look, Look::Custom);
    }

    #[test]
    fn looks_set_their_values() {
        let mut ui = open("", false, "looks");
        install_screamer2(ui.state_mut());
        ui.run();
        ui.get_by_label("Original 1996").click();
        ui.run();
        play(&mut ui);
        let s = saved(&ui);
        assert_eq!(s.resolution, Some(config::ORIGINAL));
        assert_eq!(
            (s.widescreen, s.frame_generation),
            (Some(false), Some(false))
        );
    }

    #[test]
    fn disabled_controls_say_why() {
        let mut ui = open("which = screamer\n", false, "disabled");
        tab(&mut ui, "Picture");
        let voodoo = ui.get_by_label("3dfx Voodoo");
        assert!(testing::disabled(&voodoo));
        assert!(
            testing::description(&voodoo)
                .unwrap()
                .contains("only shipped with 8-bit SVGA")
        );
        tab(&mut ui, "Controls");
        ui.get_by_role_and_label(Role::Switch, "Gamepad").click();
        ui.run();
        let strong = ui.get_by_label("Strong");
        assert!(testing::disabled(&strong));
        assert!(
            ui.query_by_label_contains("Turn on Gamepad above")
                .is_some()
        );
        tab(&mut ui, "Buttons");
        assert!(
            ui.query_by_label_contains("Turn on Gamepad in the Controls tab")
                .is_some()
        );
        // Play waits for a game folder.
        assert!(testing::disabled(
            &ui.get_by_role_and_label(Role::Button, "Play")
        ));
    }

    #[test]
    fn a_saved_value_outside_the_chips_is_shown_and_kept() {
        let mut ui = open("deadzone = 12\nresolution = 2560x1440\n", false, "custom");
        install_screamer2(ui.state_mut());
        ui.run();
        tab(&mut ui, "Controls");
        assert!(testing::checked(&ui.get_by_label("12%")));
        tab(&mut ui, "Picture");
        assert!(testing::checked(&ui.get_by_label("2560×1440")));
        play(&mut ui);
        let s = saved(&ui);
        assert_eq!((s.deadzone, s.resolution), (Some(12), Some((2560, 1440))));
    }

    #[test]
    fn the_pad_steps_through_the_picture_rows() {
        let mut ui = open("", false, "pad");
        let ctx = ui.ctx.clone();
        let w = ui.state_mut();
        w.handle_pad_action(Nav::NextTab, &ctx);
        assert_eq!(w.tab, Tab::Picture);
        // Renderer, Display, Shader, then the four switches.
        for _ in 0..6 {
            w.handle_pad_action(Nav::Down, &ctx);
        }
        assert!(w.focused(Row::FrameGeneration));
        w.handle_pad_action(Nav::Press, &ctx);
        assert!(w.form.picture.frame_generation);
        w.handle_pad_action(Nav::Down, &ctx);
        assert!(w.focused(Row::Renderer), "wraps to the top");
    }

    #[test]
    fn the_scaler_row_is_linux_only() {
        let mut ui = open("", false, "no-scaler");
        tab(&mut ui, "Picture");
        assert!(
            ui.query_by_role_and_label(Role::RadioGroup, "Scaler")
                .is_none()
        );
        let mut ui = open("", true, "scaler");
        tab(&mut ui, "Picture");
        assert!(
            ui.query_by_role_and_label(Role::RadioGroup, "Scaler")
                .is_some()
        );
    }

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
