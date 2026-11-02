mod ui;
#[cfg(windows)]
mod win32;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use rea_launcher::cfg;
use rea_launcher::fsx;
use rea_launcher::log::Log;

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

/// Set once the launcher runs the way a player starts it (double-click or
/// a shortcut: the settings window, then the game). Only then are fatal
/// errors also shown in a message box; scripts (`--check`, `--no-window`,
/// `--settings`, `--controllers`, bad arguments) get stderr and the log.
static INTERACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn is_interactive(mode: &Mode) -> bool {
    *mode == Mode::Play { show_window: true }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(msg) => {
            log().error(&msg);
            if INTERACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
                show_error(&msg);
            }
            ExitCode::from(2)
        }
    }
}

/// `launcher.log`, next to `launcher.cfg` (1 MiB at most, then started
/// afresh); logging never fails the launch.
fn log() -> Log {
    Log::new(locate_config_file().with_file_name("launcher.log"))
}

/// Logs and prints a warning.
fn warn(message: &str) {
    log().warn(message);
}

/// On Windows the launcher is often started without a console: show fatal
/// errors of an interactive launch in a message box as well.
#[cfg(windows)]
fn show_error(msg: &str) {
    use rea_launcher_win32::dialog::{self, Icon};
    let text = format!("{msg}\n\nDetails: {}", log().path().display());
    dialog::message("Screamer Trilogy", &text, Icon::Error);
}

#[cfg(not(windows))]
fn show_error(_msg: &str) {}

fn run() -> Result<ExitCode, String> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let (mode, filtered_args) = extract_mode(&raw_args)?;
    INTERACTIVE.store(is_interactive(&mode), std::sync::atomic::Ordering::Relaxed);
    let cli_settings = match Settings::parse_args(filtered_args) {
        Ok(s) => s,
        Err(ArgsError::Help) => {
            println!("{}", config::usage());
            return Ok(ExitCode::SUCCESS);
        }
        Err(ArgsError::Bad(message)) => {
            return Err(format!("{message}\n\n{}", config::usage()));
        }
    };

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
    let mut settings = file_settings.merged(cli_settings.clone());
    if settings.game.is_none()
        && let Ok(env_dir) = std::env::var("SCREAMER_GAME_DIR")
        && !env_dir.trim().is_empty()
    {
        settings.game = Some(PathBuf::from(env_dir));
    }

    match mode {
        Mode::Check => run_check(&settings, cli_settings.which),
        Mode::Controllers { seconds } => run_controllers(seconds),
        Mode::SettingsOnly { linux } => match ui::show(&cfg_path, &cfg_text, linux) {
            Ok(ui::Outcome::Play) => Ok(ExitCode::SUCCESS),
            Ok(ui::Outcome::Closed) => Ok(ExitCode::from(WINDOW_CLOSED)),
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
                        Settings::parse_file(&reloaded).0.merged(cli_settings)
                    }
                    Ok(ui::Outcome::Closed) => return Ok(ExitCode::from(WINDOW_CLOSED)),
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
            launch_game(&final_settings)
        }
    }
}

fn extract_mode(args: &[String]) -> Result<(Mode, Vec<String>), String> {
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
    Ok((resolved, out))
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

fn run_check(settings: &Settings, filter_game: Option<Game>) -> Result<ExitCode, String> {
    let log = log();
    let machine = MachineCapabilities::detect();
    let say = |line: String| {
        println!("{line}");
        log.line(&line);
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
        println!();
        checked_any = true;
        say(format!("Game:       {}", disc.game.title()));
        say(format!("Source dir: {}", disc.exe_dir.display()));
        match &disc.cd_image {
            Some(cd) => say(format!("CD image:   {}", cd.display())),
            None => say("CD image:   none (directory mount fallback)".into()),
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

fn run_controllers(_seconds: u32) -> Result<ExitCode, String> {
    println!("Scanning connected controllers...");
    let controllers = ui::detect_controllers();
    if controllers.is_empty() {
        println!("  No controllers currently connected.");
    } else {
        for (idx, name) in controllers.iter().enumerate() {
            println!("  Controller #{}: {name}", idx + 1);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn launch_game(settings: &Settings) -> Result<ExitCode, String> {
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

    let machine = MachineCapabilities::detect();
    let mut settings = settings.clone();
    let (race, warning) = config::fit_to_screen(settings.race_resolution(), machine.display_res);
    if let Some(warning) = warning {
        warn(&warning);
        settings.resolution = race;
    }
    let settings = &settings;

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
    let dosbox =
        runtime::find_dosbox_with_hint(settings.dosbox.as_deref(), Some(game_dir)).ok_or_else(
            || {
                "Could not find bundled dosbox (or dosbox-staging). Run ./scripts/build-dosbox.sh or pass --dosbox <path>."
                    .to_string()
            },
        )?;

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

    #[test]
    fn only_the_double_click_path_is_interactive() {
        let mode = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            extract_mode(&args).unwrap().0
        };
        assert!(is_interactive(&mode(&[])));
        assert!(is_interactive(&mode(&["rally"])));
        for scripted in [
            &["--check"][..],
            &["--no-window"],
            &["--settings", "--linux"],
            &["--controllers"],
        ] {
            assert!(!is_interactive(&mode(scripted)), "{scripted:?}");
        }
    }
}
