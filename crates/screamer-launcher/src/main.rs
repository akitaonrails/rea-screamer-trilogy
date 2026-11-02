mod ui;
#[cfg(windows)]
mod win32;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use rea_launcher::cfg;
use rea_launcher::log::Log;

use screamer_launcher::config::{self, MachineCapabilities, Settings};
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
    let cli_settings = Settings::parse_args(filtered_args)?;

    let cfg_path = locate_config_file();
    // Only a missing file is a first run; one that cannot be read is never
    // replaced.
    let mut cfg_text = match cfg::read_file(&cfg_path) {
        Ok(Some(text)) => text,
        Ok(None) => {
            if let Some(parent) = cfg_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&cfg_path, config::TEMPLATE);
            config::TEMPLATE.to_string()
        }
        Err(_) => config::TEMPLATE.to_string(),
    };

    // Remember --game and --which passed on the CLI in launcher.cfg.
    let mut cfg_dirty = false;
    if let Some(g) = &cli_settings.game {
        cfg_text = cfg::set_key(&cfg_text, "game", &g.display().to_string());
        cfg_dirty = true;
    }
    if let Some(w) = cli_settings.which {
        cfg_text = cfg::set_key(&cfg_text, "which", w.slug());
        cfg_dirty = true;
    }
    if cfg_dirty {
        let _ = fs::write(&cfg_path, &cfg_text);
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
    let machine = MachineCapabilities::detect();
    println!(
        "Hardware:   {} · Display {}×{} @ {} Hz",
        machine.gpu_name, machine.display_res.0, machine.display_res.1, machine.refresh_hz
    );
    println!(
        "Defaults:   Renderer={}, Widescreen={}, Upscaler={:?}, FrameGen={}",
        settings.renderer_choice().key(),
        settings.widescreen_enabled(),
        settings.effective_filter(),
        if settings.fps_cap().is_some_and(|f| f >= 60) {
            "100 FPS (2x Optical Flow)"
        } else {
            "50 FPS (Native)"
        }
    );
    println!();

    let game_dir = settings
        .game
        .as_deref()
        .ok_or_else(|| "No game folder set. Pass --game <folder>.".to_string())?;
    let trilogy = runtime::discover_trilogy(game_dir);
    if !trilogy.any_found() {
        return Err(format!(
            "No supported Screamer, Screamer 2, or Screamer Rally executables found in {}",
            game_dir.display()
        ));
    }

    let mut checked_any = false;
    for g in Game::ALL {
        if filter_game.is_some_and(|fg| fg != g) {
            continue;
        }
        let Some(disc) = trilogy.get(g) else {
            continue;
        };
        if checked_any {
            println!();
        }
        checked_any = true;
        println!("Game:       {}", disc.game.title());
        println!("Source dir: {}", disc.exe_dir.display());
        if let Some(cd) = &disc.cd_image {
            println!("CD image:   {}", cd.display());
        } else {
            println!("CD image:   none (directory mount fallback)");
        }
        if disc.has_genuine_dos_glide2x {
            println!("Glide OVL:  Genuine DOS 3dfx GLIDE2X.OVL detected");
        } else if disc.has_windows_stub_glide2x {
            println!(
                "Glide OVL:  Windows port-0x600 stub detected (auto-selecting 16-bit SVGA START65H.EXE unless DOS GLIDE2X.OVL is supplied)"
            );
        }

        let verified = runtime::verify_discovered_game(disc, &settings.skipped_fixes())?;
        for (name, ed, write_count) in &verified {
            println!(
                "Verified:   {name:<12} -> {:<56} ({write_count} patch writes verified)",
                ed.name
            );
        }
        let selected = disc.select_executable(settings.renderer_choice())?;
        println!(
            "Selected:   {} ({})",
            selected.file_name, selected.edition.name
        );
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
    let game_dir = settings
        .game
        .as_deref()
        .ok_or_else(|| "No game folder set. Pass --game <folder>.".to_string())?;
    let disc = runtime::resolve_game_dir(game_dir, settings.which_game())?;
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

    let prepared =
        runtime::prepare_runtime(&disc, settings, &runtime_dir, Some(saves_dir.as_path()))?;
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

    let mut cmd = Command::new(&dosbox);
    cmd.current_dir(&prepared.runtime_dir)
        .arg("-noprimaryconf")
        .arg("-nolocalconf")
        .arg("-conf")
        .arg(&prepared.conf_path);

    let is_3dfx = prepared.selected_exe.eq_ignore_ascii_case("S2_3DFX.EXE")
        || prepared.selected_exe.eq_ignore_ascii_case("STRT3FX.EXE");
    let voodoo_scale = if is_3dfx && settings.race_resolution().is_some() {
        "2"
    } else {
        "1"
    };
    let frame_gen = if settings.fps_cap().is_some_and(|fps| fps >= 60) {
        "1"
    } else {
        "0"
    };

    if machine.has_nvidia_dgpu {
        cmd.env("__NV_PRIME_RENDER_OFFLOAD", "1")
            .env("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
    } else if machine.has_amd_dgpu {
        cmd.env("DRI_PRIME", "1");
    }

    // Pass settings to the statically linked screamer-hooks and Voodoo renderer inside dosbox via environment variables.
    cmd.env("SCREAMER_GAME", disc.game.slug())
        .env("SCREAMER_EXE", &prepared.selected_exe)
        .env("SCREAMER_VOODOO_SCALE", voodoo_scale)
        .env("SCREAMER_FRAME_GEN", frame_gen)
        .env(
            "SCREAMER_GAMEPAD_AXES",
            if settings.gamepad_axes != Some(false) {
                "1"
            } else {
                "0"
            },
        )
        .env(
            "SCREAMER_DEADZONE",
            settings.deadzone.unwrap_or(0).to_string(),
        )
        .env("SCREAMER_RUMBLE", settings.rumble_strength().to_string())
        .env(
            "SCREAMER_BUTTONS",
            config::buttons_value(&settings.button_map()),
        );

    let status = cmd
        .status()
        .map_err(|e| format!("Failed to start {}: {e}", dosbox.display()))?;

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
