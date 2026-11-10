// A GUI program on Windows: no console window beside the settings window
// on a double click. Commands reach the terminal they were started from
// through the SDK's console attach (`report::route_here`).
#![cfg_attr(windows, windows_subsystem = "windows")]

mod ui;
#[cfg(windows)]
mod win32;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use rea_launcher::cfg;
use rea_launcher::fsx;
use rea_launcher::log::Log;
use rea_launcher_win32::report::{self, Audience, Run};

use screamer_launcher::config::{self, ArgsError, MachineCapabilities, Settings};
use screamer_launcher::edition::Game;
use screamer_launcher::runtime;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Play { show_window: bool },
    SettingsOnly { linux: bool },
    Check,
    Controllers { seconds: u32 },
}

/// `--settings` exit code: the window was closed without playing.
const WINDOW_CLOSED: u8 = 3;
/// `--settings` exit code: no window could be made; play with the saved
/// settings (screamer-play treats it like Play).
const WINDOW_UNAVAILABLE: u8 = 4;

/// The command line's family flags (`--linux`, `--no-window`) beside its
/// mode, for who sees this run's messages.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Invocation {
    mode: Mode,
    linux: bool,
    no_window: bool,
}

impl Invocation {
    /// The run, for the SDK's audience rule (`report::Audience`).
    fn run(&self) -> Run {
        Run {
            command: matches!(self.mode, Mode::Check | Mode::Controllers { .. }),
            no_window: self.no_window,
            settings_only: matches!(self.mode, Mode::SettingsOnly { .. }),
            linux: self.linux,
        }
    }
}

/// Who may see this run's messages, decided by the SDK (never by hand:
/// `screamer-play` starts the game run with `--no-window --linux` for a
/// player in front of the screen). Under Wine (the Windows tests) a run
/// counts as watched like the family's.
fn audience(run: Run) -> Audience {
    #[cfg(windows)]
    {
        rea_launcher_win32::report::audience(run)
    }
    #[cfg(not(windows))]
    {
        Audience::of(run, false)
    }
}

/// What the command line asks for.
#[derive(Debug)]
enum Start {
    /// A run: its mode and family flags, and the options for this run.
    Run(Invocation, Settings),
    /// `--help`, `-h` or `/?`.
    Help,
    /// A command line that doesn't parse: the error, then the usage.
    BadCommandLine(String),
}

fn parse_command_line(args: &[String]) -> Start {
    let (invocation, rest) = match extract_mode(args) {
        Ok(parsed) => parsed,
        Err(message) => return Start::BadCommandLine(message),
    };
    match Settings::parse_args(rest) {
        Ok(settings) => Start::Run(invocation, settings),
        Err(ArgsError::Help) => Start::Help,
        Err(ArgsError::Bad(message)) => {
            Start::BadCommandLine(format!("{message}\n\n{}", config::usage()))
        }
    }
}

/// A command line that fails to parse is reported like a command (the
/// terminal; a dialog only when nobody sees it), never like a play run,
/// whose dialog would hang a script with a typo: the SDK's rule.
fn bad_command_line_audience(args: &[String]) -> Audience {
    audience(Run::bad_command_line(args))
}

/// The modes that open the window or play take the one-launcher instance,
/// and so does `--check`, an action (STANDARDS "Action flags and one
/// instance"): with the window open it is refused with exit 5 instead of
/// checking alongside. `--controllers` and `--help` only list.
fn takes_instance(mode: &Mode) -> bool {
    matches!(
        mode,
        Mode::Play { .. } | Mode::SettingsOnly { .. } | Mode::Check
    )
}

/// Whether this second start is an action (`--check`): it never brings
/// the open window forward, it is refused.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_action(args: &[String]) -> bool {
    rea_launcher_win32::instance::is_action(args, &[])
}

/// What a refused second start says: the window is open (an action), or
/// the launcher or its game is running.
#[cfg_attr(not(windows), allow(dead_code))]
fn refusal_message(second: rea_launcher_win32::instance::SecondStart) -> String {
    use rea_launcher_win32::instance::SecondStart;
    match second {
        SecondStart::WindowOpen => second.message(ui::TITLE).unwrap_or_default(),
        _ => already_running_message(),
    }
}

/// The instance name (`Local\rea-launcher-rea-screamer` on Windows, a
/// `rea-launcher-rea-screamer.lock` file in `$XDG_RUNTIME_DIR` on Linux).
const INSTANCE: &str = "rea-screamer";

/// What a refused second start says (both systems).
fn already_running_message() -> String {
    format!(
        "The {} launcher or its game is already running. Quit the game or close that window first, then start the launcher again.",
        ui::TITLE
    )
}

/// One launcher per user session (STANDARDS "Clean exit, single
/// instance"): a second plain start brings the open settings window
/// forward (exit 0); an action (`--check`) is refused while the window is
/// open, and any start while the game runs (exit 5). Keep the guard for
/// the launcher's lifetime.
#[cfg(windows)]
fn single_instance(
    args: &[String],
    says: Audience,
) -> Result<rea_launcher_win32::instance::Guard, ExitCode> {
    use rea_launcher_win32::instance::{self, SecondStart, Window};
    match instance::single(INSTANCE) {
        Ok(guard) => Ok(guard),
        Err(running) => {
            let second = running.second_start(is_action(args), Window::Own(ui::TITLE));
            log().info(second.log_line());
            if second != SecondStart::Focused {
                report_error(&refusal_message(second), says);
            }
            Err(exit_code(second.exit_code()))
        }
    }
}

/// The native Linux build's lock: the helper's session lock doesn't cover
/// a launcher started straight from a menu or the AppImage. A native
/// window can't be brought forward from here, so a second start says so
/// and exits 5.
#[cfg(not(windows))]
fn single_instance(
    _args: &[String],
    says: Audience,
) -> Result<rea_launcher::instance::Guard, ExitCode> {
    match rea_launcher::instance::single(INSTANCE) {
        Ok(guard) => Ok(guard),
        Err(running) => {
            log().info("second start: another launcher holds the instance; refused");
            report_error(&already_running_message(), says);
            Err(exit_code(running.exit_code()))
        }
    }
}

fn exit_code(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (invocation, cli_settings) = match parse_command_line(&args) {
        Start::Run(invocation, settings) => (invocation, settings),
        Start::Help => {
            command_output(
                &config::usage(),
                audience(Run::from_args(&args, &["--controllers"])),
                true,
            );
            return ExitCode::SUCCESS;
        }
        Start::BadCommandLine(message) => {
            report_error(&message, bad_command_line_audience(&args));
            return exit_code(report::BAD_COMMAND_LINE);
        }
    };
    let says = audience(invocation.run());
    let _instance = if takes_instance(&invocation.mode) {
        match single_instance(&args, says) {
            Ok(guard) => Some(guard),
            Err(code) => return code,
        }
    } else {
        None
    };
    match run(invocation.mode, cli_settings, says) {
        Ok(code) => code,
        Err(msg) => {
            report_error(&msg, says);
            ExitCode::from(2)
        }
    }
}

/// `launcher.log`, next to `launcher.cfg` (1 MiB at most, then started
/// afresh); logging never fails the launch.
fn log() -> Log {
    Log::new(locate_config_file().with_file_name("launcher.log"))
}

/// A command's output (`--help`, `--check`, `--controllers`): printed to
/// the terminal the launcher was started from or to a redirect (on
/// Windows the SDK attaches to the parent console first), and in a
/// message box when nobody sees it and a person started it (a
/// double-clicked shortcut), when `dialog` allows.
fn command_output(text: &str, says: Audience, dialog: bool) {
    #[cfg(windows)]
    {
        let route = report::route_here(says.mode());
        if route.print {
            println!("{text}");
        }
        if dialog && route.dialog {
            use rea_launcher_win32::dialog::{self, Icon};
            dialog::message(ui::TITLE, text, Icon::Info);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (says, dialog);
        println!("{text}");
    }
}

/// What the game run returns when the player closes the settings window
/// without Play: 3 for the Linux helper's `--settings`, 0 for a plain
/// window close (STANDARDS "Family consistency").
fn window_closed_code(mode: &Mode) -> u8 {
    match mode {
        Mode::SettingsOnly { .. } => WINDOW_CLOSED,
        _ => 0,
    }
}

/// Logs and prints a warning.
fn warn(message: &str) {
    log().warn(message);
}

/// A fatal error: always in `launcher.log`, then through the SDK's report
/// for this run's audience: on Windows printed and/or a message box; on
/// Linux printed to stderr (the terminal or `screamer-play`'s log) and a
/// message window whenever a person may be watching on a desktop.
fn report_error(msg: &str, says: Audience) {
    let log = log();
    log.line(&format!("error: {msg}"));
    #[cfg(windows)]
    {
        let text = format!("{msg}\n\nDetails: {}", log.path().display());
        rea_launcher_win32::report::fatal(ui::TITLE, &text, says.mode());
    }
    #[cfg(not(windows))]
    {
        let details = format!(
            "ScreamerLauncher {}\nlauncher.log: {}",
            env!("CARGO_PKG_VERSION"),
            log.path().display()
        );
        ui::fatal(msg, &details, says);
    }
}

fn run(mode: Mode, cli_settings: Settings, says: Audience) -> Result<ExitCode, String> {
    let cfg_path = locate_config_file();
    // Only a missing file is a first run; one that cannot be read is never
    // replaced. The first run saves the recommended look for this screen.
    let mut cfg_text = match cfg::read_file(&cfg_path) {
        Ok(Some(text)) => text,
        Ok(None) => {
            let screen = MachineCapabilities::detect().display_res;
            let text = config::first_run(Some(screen));
            if let Some(parent) = cfg_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fsx::write_atomic(&cfg_path, &text);
            log().info(&format!(
                "first run: wrote {} with the recommended look",
                cfg_path.display()
            ));
            text
        }
        Err(e) => {
            warn(&format!("{}: {e}; using the defaults", cfg_path.display()));
            config::template()
        }
    };

    // Remember --game and --which passed on the command line in launcher.cfg
    // (not for --check, which only looks).
    if mode != Mode::Check {
        let remembered = config::save_keys(
            &cfg_text,
            &cli_settings,
            &config::REMEMBERED
                .iter()
                .copied()
                .filter(|k| config::cfg_value(&cli_settings, k).is_some())
                .collect::<Vec<_>>(),
        );
        if remembered != cfg_text {
            cfg_text = remembered;
            let _ = fsx::write_atomic(&cfg_path, &cfg_text);
        }
    }

    let (file_settings, warnings) = Settings::parse_file(&cfg_text);
    for w in warnings {
        warn(&w);
    }
    let mut settings = file_settings.merged(cli_settings.clone()).with_look();
    if settings.game.is_none()
        && let Ok(env_dir) = std::env::var("SCREAMER_GAME_DIR")
        && !env_dir.trim().is_empty()
    {
        settings.game = Some(PathBuf::from(env_dir));
    }

    match mode {
        Mode::Check => {
            let mut report = Vec::new();
            let checked = run_check(&settings, cli_settings.which, &mut report);
            // A failed check's error goes through report_error; the lines
            // before it are only printed, never a second dialog.
            command_output(&report.join("\n"), says, checked.is_ok());
            checked.map_err(runtime::with_bug_link)
        }
        Mode::Controllers { seconds } => {
            command_output(&run_controllers(seconds), says, true);
            Ok(ExitCode::SUCCESS)
        }
        Mode::SettingsOnly { linux } => match ui::show(&cfg_path, &cfg_text, linux) {
            Ok(ui::Outcome::Play) => Ok(ExitCode::SUCCESS),
            Ok(ui::Outcome::Closed) => Ok(ExitCode::from(window_closed_code(&mode))),
            Err(ui::ShowError::NoWindow(e)) => {
                warn(&format!(
                    "settings window unavailable ({e}); playing with saved settings"
                ));
                Ok(ExitCode::from(WINDOW_UNAVAILABLE))
            }
            Err(ui::ShowError::Failed(e)) => Err(format!("settings window: {e}")),
        },
        Mode::Play { show_window } => {
            let final_settings = if show_window {
                match ui::show(&cfg_path, &cfg_text, cfg!(target_os = "linux")) {
                    Ok(ui::Outcome::Play) => {
                        let reloaded = cfg::read_file(&cfg_path)
                            .ok()
                            .flatten()
                            .unwrap_or_else(|| cfg_text.clone());
                        Settings::parse_file(&reloaded)
                            .0
                            .merged(cli_settings)
                            .with_look()
                    }
                    Ok(ui::Outcome::Closed) => {
                        return Ok(ExitCode::from(window_closed_code(&mode)));
                    }
                    // No window possible (no display or OpenGL): play with
                    // the saved settings, like the sibling launchers.
                    Err(ui::ShowError::NoWindow(e)) => {
                        warn(&format!(
                            "settings window unavailable ({e}); playing with saved settings"
                        ));
                        settings
                    }
                    // The window opened and then failed: never start the
                    // game with settings the player may have changed.
                    Err(ui::ShowError::Failed(e)) => {
                        return Err(format!("settings window: {e}"));
                    }
                }
            } else {
                settings
            };
            launch_game(&final_settings, says)
        }
    }
}

fn extract_mode(args: &[String]) -> Result<(Invocation, Vec<String>), String> {
    let mut mode = None;
    let mut linux = false;
    let mut no_window = false;
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--check" => mode = Some(Mode::Check),
            "--settings" => mode = Some(Mode::SettingsOnly { linux: false }),
            "--linux" => linux = true,
            "--no-window" => no_window = true,
            "--controllers" => {
                let mut secs = 30u32;
                if let Some(next) = args.get(i + 1)
                    && !next.starts_with('-')
                {
                    secs = next
                        .parse()
                        .map_err(|_| format!("--controllers needs seconds, not {next}"))?;
                    i += 1;
                }
                mode = Some(Mode::Controllers { seconds: secs });
            }
            other => out.push(other.to_string()),
        }
        i += 1;
    }

    let resolved = match mode {
        Some(Mode::SettingsOnly { .. }) => Mode::SettingsOnly { linux },
        Some(m) => m,
        None => Mode::Play {
            show_window: !no_window,
        },
    };
    let invocation = Invocation {
        mode: resolved,
        linux,
        no_window,
    };
    Ok((invocation, out))
}

fn locate_config_file() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let local = dir.join(config::FILE_NAME);
        if local.is_file() {
            return local;
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata)
            .join("ScreamerLauncher")
            .join(config::FILE_NAME);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg)
            .join("screamer-launcher")
            .join(config::FILE_NAME);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("screamer-launcher")
            .join(config::FILE_NAME);
    }
    PathBuf::from(config::FILE_NAME)
}

fn default_runtime_dir(game_slug: &str) -> PathBuf {
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local_appdata)
            .join("ScreamerLauncher")
            .join("runtime")
            .join(game_slug);
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata)
            .join("ScreamerLauncher")
            .join("runtime")
            .join(game_slug);
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg)
            .join("screamer-launcher")
            .join("runtime")
            .join(game_slug);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("screamer-launcher")
            .join("runtime")
            .join(game_slug);
    }
    std::env::temp_dir()
        .join("screamer-launcher-runtime")
        .join(game_slug)
}

fn default_saves_dir(game_slug: &str) -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata)
            .join("ScreamerLauncher")
            .join("saves")
            .join(game_slug);
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg)
            .join("screamer-launcher")
            .join("saves")
            .join(game_slug);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("screamer-launcher")
            .join("saves")
            .join(game_slug);
    }
    std::env::temp_dir()
        .join("screamer-launcher-saves")
        .join(game_slug)
}

/// `--check`: the report's lines go into `out` (and `launcher.log`).
fn run_check(
    settings: &Settings,
    filter_game: Option<Game>,
    out: &mut Vec<String>,
) -> Result<ExitCode, String> {
    let log = log();
    let machine = MachineCapabilities::detect();
    let mut say = |line: String| {
        if !line.is_empty() {
            log.line(&line);
        }
        out.push(line);
    };
    say(format!(
        "Hardware:   {} · Display {}×{} @ {} Hz",
        machine.gpu_name, machine.display_res.0, machine.display_res.1, machine.refresh_hz
    ));
    say(format!(
        "Settings:   renderer={}, widescreen={}, filter={}, frame generation={}, vsync={}",
        settings.renderer_choice().key(),
        cfg::on_off(settings.widescreen_enabled()),
        settings.effective_filter().key(),
        cfg::on_off(settings.frame_generation_enabled()),
        cfg::on_off(settings.vsync_enabled()),
    ));

    let game_dir = settings
        .game
        .as_deref()
        .ok_or_else(|| "No game folder set. Pass --game <folder>.".to_string())?;
    let trilogy = runtime::discover_trilogy(game_dir);
    for stranger in &trilogy.unknown {
        warn(&stranger.message());
    }
    if !trilogy.any_found() {
        return Err(match trilogy.unknown.first() {
            Some(stranger) => stranger.message(),
            None => format!(
                "No supported Screamer, Screamer 2, or Screamer Rally executables found in {}",
                game_dir.display()
            ),
        });
    }

    let mut checked_any = false;
    for g in Game::ALL {
        if filter_game.is_some_and(|fg| fg != g) {
            continue;
        }
        let Some(disc) = trilogy.get(g) else {
            continue;
        };
        say(String::new());
        checked_any = true;
        say(format!("Game:       {}", disc.game.title()));
        say(format!("Source dir: {}", disc.exe_dir.display()));
        match &disc.cd_image {
            Some(cd) => say(format!("CD image:   {}", cd.display())),
            None => say("CD image:   none found".into()),
        }
        if let Some(note) = &disc.cd_warning {
            warn(note);
        }
        if disc.has_genuine_dos_glide2x {
            say("Glide OVL:  Genuine DOS 3dfx GLIDE2X.OVL detected".into());
        } else if disc.has_windows_stub_glide2x {
            say(
                "Glide OVL:  Windows port-0x600 stub detected (auto-selecting 16-bit SVGA START65H.EXE unless DOS GLIDE2X.OVL is supplied)"
                    .into(),
            );
        }
        for stranger in &disc.unknown {
            warn(&stranger.message());
        }

        let verified = runtime::verify_discovered_game(disc, &settings.skipped_fixes())?;
        for (name, ed, write_count) in &verified {
            say(format!(
                "Verified:   {name:<12} -> {:<56} ({write_count} patch writes verified)",
                ed.name
            ));
        }
        let selected = disc.select_executable(settings.renderer_choice())?;
        say(format!(
            "Selected:   {} ({})",
            selected.file_name, selected.edition.name
        ));
    }

    if !checked_any && let Some(fg) = filter_game {
        return Err(format!(
            "{} was not found under {}",
            fg.display_name(),
            game_dir.display()
        ));
    }
    Ok(ExitCode::SUCCESS)
}

/// `--controllers`: the connected pads, as text.
fn run_controllers(_seconds: u32) -> String {
    let mut text = String::from("Scanning connected controllers...");
    let controllers = ui::detect_controllers();
    if controllers.is_empty() {
        text.push_str("\n  No controllers currently connected.");
    } else {
        for (idx, name) in controllers.iter().enumerate() {
            text.push_str(&format!("\n  Controller #{}: {name}", idx + 1));
        }
    }
    text
}

fn launch_game(settings: &Settings, says: Audience) -> Result<ExitCode, String> {
    let log = log();
    let game_dir = settings
        .game
        .as_deref()
        .ok_or_else(|| "No game folder set. Pass --game <folder>.".to_string())?;
    let disc = runtime::resolve_game_dir(game_dir, settings.which_game())?;
    for stranger in &disc.unknown {
        warn(&stranger.message());
    }
    let slug = disc.game.slug();
    let runtime_dir = settings
        .runtime
        .clone()
        .unwrap_or_else(|| default_runtime_dir(slug));
    let saves_dir = settings
        .saves
        .clone()
        .unwrap_or_else(|| default_saves_dir(slug));

    // The launcher writes into both folders; neither may overlap the
    // read-only game folder.
    runtime::check_folders(&disc.root_dir, &runtime_dir, &saves_dir)?;
    // Held until DOSBox ends and the saves are copied back.
    let _run_lock = match runtime::lock_runtime(&runtime_dir) {
        Ok(lock) => Some(lock),
        Err(busy @ runtime::RunLockError::Busy(_)) => {
            report_error(&busy.to_string(), says);
            return Ok(exit_code(rea_launcher::instance::ALREADY_RUNNING));
        }
        Err(e) => {
            warn(&format!("{e}; playing without the running-game check"));
            None
        }
    };

    let machine = MachineCapabilities::detect();
    let mut settings = settings.clone();
    let (race, warning) = config::fit_to_screen(settings.race_resolution(), machine.display_res);
    if let Some(warning) = warning {
        warn(&warning);
        settings.resolution = race;
    }
    let settings = &settings;

    let dosbox =
        runtime::find_dosbox_with_hint(settings.dosbox.as_deref(), Some(game_dir)).ok_or_else(
            || {
                "Could not find bundled dosbox (or dosbox-staging). Run ./scripts/build-dosbox.sh or pass --dosbox <path>."
                    .to_string()
            },
        )?;
    // A launcher that was killed released the instance and the runtime
    // lock, but its DOSBox may still run the game: never copy under it.
    #[cfg(windows)]
    {
        use rea_launcher_win32::instance::SecondStart;
        let image = fs::canonicalize(&dosbox).unwrap_or_else(|_| dosbox.clone());
        if !rea_launcher_win32::process::find_running(&image).is_empty() {
            let busy = SecondStart::Busy;
            log.info(busy.log_line());
            report_error(&already_running_message(), says);
            return Ok(exit_code(busy.exit_code()));
        }
    }

    let prepared = runtime::prepare_runtime(
        &disc,
        settings,
        &runtime_dir,
        Some(saves_dir.as_path()),
        runtime::Gpu::of(&machine),
    )?;
    for warning in &prepared.warnings {
        warn(warning);
    }

    let plan = &prepared.plan;
    log.info(&format!(
        "launch: {} from {}",
        disc.game.display_name(),
        disc.exe_dir.display()
    ));
    for (name, count) in &prepared.patched_exes {
        let exe = disc.executables.iter().find(|e| &e.file_name == name);
        log.info(&format!(
            "edition: {name} = {} (SHA-256 {}), {count} patch writes",
            exe.map_or("?", |e| e.edition.name),
            exe.map(|e| rea_launcher::identify::hex(&e.sha256))
                .unwrap_or_default()
        ));
    }
    log.info(&format!(
        "fixes left out: {}",
        if plan.skipped_fixes.is_empty() {
            "none".to_string()
        } else {
            plan.skipped_fixes.join(", ")
        }
    ));
    log.info(&format!(
        "exe {}; runtime {}; saves {}; dosbox {}",
        prepared.selected_exe,
        prepared.runtime_dir.display(),
        saves_dir.display(),
        dosbox.display()
    ));
    log.info(&format!("dosbox args: {}", plan.dosbox_args.join(" ")));
    log.info(&format!(
        "env: {}",
        plan.env
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ")
    ));

    let mut cmd = Command::new(&dosbox);
    cmd.current_dir(&prepared.runtime_dir)
        .args(&plan.dosbox_args);
    for (key, value) in &plan.env {
        cmd.env(key, value);
    }
    // The launcher has no console of its own (GUI subsystem): a
    // console-subsystem DOSBox would get a new console window beside the
    // game. Give it a hidden one instead.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to start {}: {e}", dosbox.display()))?;
    log.info(&format!("dosbox ended: {status}"));

    runtime::sync_saves_out(&prepared.runtime_dir, &saves_dir)?;

    if status.success() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(status.code().unwrap_or(1) as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation(args: &[&str]) -> Invocation {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        extract_mode(&args).unwrap().0
    }

    /// Who sees a message, for every way the launcher is started: the
    /// SDK's rule from the parsed flags (native Linux and Windows: no Wine).
    #[test]
    fn messages_reach_whoever_started_the_run() {
        let says = |args: &[&str]| Audience::of(invocation(args).run(), false);
        // A double-click, or a game name: a person is watching.
        for watched in [&[][..], &["rally"]] {
            assert_eq!(
                says(watched),
                Audience {
                    command: false,
                    interactive: true
                },
                "{watched:?}"
            );
        }
        // screamer-play's game run (--no-window --linux): the player is
        // in front of the screen, so a Play-time error shows a window.
        assert!(says(&["--no-window", "--linux"]).interactive);
        // screamer-play's settings window reports its own messages; when
        // it can't open, the helper starts the game run, which reports.
        assert!(!says(&["--settings", "--linux"]).interactive);
        // A script's --no-window: stderr and the log only.
        assert!(!says(&["--no-window"]).interactive);
        // Commands answer on the terminal.
        for command in [
            &["--check"][..],
            &["--controllers"],
            &["--check", "--linux"],
        ] {
            assert!(says(command).command, "{command:?}");
        }
        assert!(!says(&["--check", "--linux"]).interactive);
    }

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    /// An unknown flag or a bad value prints the error and the usage and
    /// exits 2; from a terminal it never opens a (modal) dialog, also for
    /// a run that would have opened the window (rea-launcher-sdk#1).
    #[test]
    fn a_bad_command_line_is_printed_and_exits_2() {
        use rea_launcher_win32::report::{Mode, Route, Terminal, route};
        for bad in [
            &["--bogus-flag"][..],
            &["--look", "invalid"],
            &["--no-window", "--look", "invalid"],
            &["--controllers", "soon"],
        ] {
            let args = strings(bad);
            let Start::BadCommandLine(message) = parse_command_line(&args) else {
                panic!("{bad:?} parsed");
            };
            if bad.contains(&"--look") {
                assert!(
                    message.contains("--no-window"),
                    "{bad:?}: the usage follows"
                );
            }
            let mode = Audience::of(Run::bad_command_line(&args), false).mode();
            assert!(matches!(mode, Mode::Command { .. }), "{bad:?}: {mode:?}");
            for wine in [false, true] {
                let terminal = Terminal {
                    attached: true,
                    redirected: false,
                    wine,
                };
                assert_eq!(
                    route(mode, terminal),
                    Route {
                        print: true,
                        dialog: false
                    },
                    "{bad:?}"
                );
            }
        }
        assert_eq!(report::BAD_COMMAND_LINE, 2);
        assert!(matches!(
            parse_command_line(&strings(&["--help"])),
            Start::Help
        ));
        assert!(matches!(
            parse_command_line(&strings(&["--look", "enhanced"])),
            Start::Run(..)
        ));
    }

    #[test]
    fn the_window_play_and_check_take_the_instance() {
        let mode = |args: &[&str]| invocation(args).mode;
        for one in [
            &[][..],
            &["rally"],
            &["--no-window"],
            &["--settings", "--linux"],
            &["--check"],
            &["--check", "--game", "C:\\Games\\Screamer"],
        ] {
            assert!(takes_instance(&mode(one)), "{one:?}");
        }
        assert!(!takes_instance(&mode(&["--controllers"])));
    }

    /// A second start with `--check` while the window is open is refused
    /// with exit 5 and says so; a plain second start brings the window
    /// forward (rea-launcher-sdk#1, finding 5).
    #[test]
    fn check_with_the_window_open_is_refused_with_5() {
        use rea_launcher_win32::instance::SecondStart;
        for check in [&["--check"][..], &["--check", "--which", "rally"]] {
            let args = strings(check);
            assert!(takes_instance(&invocation(check).mode), "{check:?}");
            let second = SecondStart::decide(
                is_action(&args),
                || true,
                || panic!("an action never brings the window forward"),
            );
            assert_eq!(second, SecondStart::WindowOpen, "{check:?}");
            assert_eq!(second.exit_code(), 5);
            assert!(
                refusal_message(second).contains("launcher window is open"),
                "{}",
                refusal_message(second)
            );
            // The game (no window) holds the instance: refused too.
            let busy = SecondStart::decide(is_action(&args), || false, || false);
            assert_eq!(busy.exit_code(), 5);
            assert!(refusal_message(busy).contains("already running"));
        }
        for plain in [&[][..], &["rally"], &["--no-window"]] {
            let second = SecondStart::decide(is_action(&strings(plain)), || true, || true);
            assert_eq!(second, SecondStart::Focused, "{plain:?}");
            assert_eq!(second.exit_code(), 0);
        }
    }

    /// Closing the settings window without Play: 0 for a plain start
    /// (the console build exited 3, rea-launcher-sdk#1 finding 2), 3 only
    /// for the Linux helper's `--settings`.
    #[test]
    fn a_plain_window_close_exits_0() {
        for plain in [&[][..], &["rally"], &["--linux"]] {
            assert_eq!(window_closed_code(&invocation(plain).mode), 0, "{plain:?}");
        }
        for helper in [&["--settings"][..], &["--settings", "--linux"]] {
            assert_eq!(
                window_closed_code(&invocation(helper).mode),
                3,
                "{helper:?}"
            );
        }
    }

    /// Native Linux: a second launcher can't take the first one's lock
    /// and exits 5, the family's "already running".
    #[cfg(not(windows))]
    #[test]
    fn a_second_native_launcher_is_refused_with_5() {
        let dir = std::env::temp_dir().join(format!("screamer-instance-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let lock = dir.join("rea-launcher-rea-screamer.lock");
        let first = rea_launcher::instance::single_at(&lock).expect("first");
        let second = rea_launcher::instance::single_at(&lock).unwrap_err();
        assert_eq!(second.exit_code(), 5);
        drop(first);
        assert!(rea_launcher::instance::single_at(&lock).is_ok());
        assert!(
            rea_launcher::instance::lock_path(INSTANCE)
                .to_string_lossy()
                .ends_with("rea-launcher-rea-screamer.lock")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Windows (Wine): a second launcher finds the first one's instance.
    #[cfg(windows)]
    #[test]
    fn a_second_launcher_finds_the_first() {
        let first = rea_launcher_win32::instance::single(INSTANCE).expect("first");
        assert!(rea_launcher_win32::instance::single(INSTANCE).is_err());
        drop(first);
        assert!(rea_launcher_win32::instance::single(INSTANCE).is_ok());
    }
}
