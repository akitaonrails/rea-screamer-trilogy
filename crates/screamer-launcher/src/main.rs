mod ui;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use screamer_launcher::config::{self, Settings};
use screamer_launcher::runtime;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Play { show_window: bool },
    SettingsOnly { linux: bool },
    Check,
    Controllers { seconds: u32 },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let (mode, filtered_args) = extract_mode(&raw_args)?;
    let cli_settings = Settings::parse_args(filtered_args)?;

    let cfg_path = locate_config_file();
    let cfg_text = if cfg_path.is_file() {
        fs::read_to_string(&cfg_path).unwrap_or_else(|_| config::TEMPLATE.to_string())
    } else {
        if let Some(parent) = cfg_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&cfg_path, config::TEMPLATE);
        config::TEMPLATE.to_string()
    };

    // If --game was passed on CLI, remember it in launcher.cfg.
    let cfg_text = if let Some(g) = &cli_settings.game {
        let updated = config::set_key(&cfg_text, "game", &g.display().to_string());
        let _ = fs::write(&cfg_path, &updated);
        updated
    } else {
        cfg_text
    };

    let (file_settings, warnings) = Settings::parse_file(&cfg_text);
    for w in warnings {
        eprintln!("warning: {w}");
    }
    let settings = file_settings.merged(cli_settings.clone());

    match mode {
        Mode::Check => run_check(&settings),
        Mode::Controllers { seconds } => run_controllers(seconds),
        Mode::SettingsOnly { linux } => {
            match ui::show(&cfg_path, &cfg_text, linux)? {
                ui::Outcome::Play => Ok(ExitCode::SUCCESS),
                ui::Outcome::Closed => Ok(ExitCode::from(3)),
            }
        }
        Mode::Play { show_window } => {
            let final_settings = if show_window {
                match ui::show(&cfg_path, &cfg_text, cfg!(target_os = "linux"))? {
                    ui::Outcome::Play => {
                        let reloaded =
                            fs::read_to_string(&cfg_path).unwrap_or_else(|_| cfg_text.clone());
                        Settings::parse_file(&reloaded).0.merged(cli_settings)
                    }
                    ui::Outcome::Closed => return Ok(ExitCode::from(3)),
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

fn run_check(settings: &Settings) -> Result<ExitCode, String> {
    let game_dir = settings
        .game
        .as_deref()
        .ok_or_else(|| "No game folder set. Pass --game <folder>.".to_string())?;
    let disc = runtime::scan_game_dir(game_dir)?;
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

    let verified = runtime::verify_discovered_game(&disc, &settings.skipped_fixes())?;
    for (name, ed, write_count) in &verified {
        println!(
            "Verified:   {name:<12} -> {:<40} ({write_count} patch writes verified)",
            ed.name
        );
    }
    let selected = disc.select_executable(settings.renderer_choice())?;
    println!(
        "Selected:   {} ({})",
        selected.file_name, selected.edition.name
    );
    Ok(ExitCode::SUCCESS)
}

fn run_controllers(_seconds: u32) -> Result<ExitCode, String> {
    println!("Scanning connected controllers...");
    let sys_input = Path::new("/sys/class/input");
    let mut found = 0usize;
    if sys_input.is_dir()
        && let Ok(entries) = fs::read_dir(sys_input)
    {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("js") {
                let dev_name =
                    fs::read_to_string(e.path().join("device/name")).unwrap_or_default();
                println!("  /dev/input/{name}: {}", dev_name.trim());
                found += 1;
            }
        }
    }
    if found == 0 {
        println!("  No /dev/input/js* controllers currently connected.");
    }
    Ok(ExitCode::SUCCESS)
}

fn locate_hooks_library() -> Option<PathBuf> {
    if let Ok(env_p) = std::env::var("SCREAMER_HOOKS_LIB") {
        let p = PathBuf::from(env_p);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for cand in ["libscreamer_hooks.so", "screamer_hooks.dll"] {
            let p = dir.join(cand);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn launch_game(settings: &Settings) -> Result<ExitCode, String> {
    let game_dir = settings
        .game
        .as_deref()
        .ok_or_else(|| "No game folder set. Pass --game <folder>.".to_string())?;
    let disc = runtime::scan_game_dir(game_dir)?;
    let slug = disc.game.slug();
    let runtime_dir = settings
        .runtime
        .clone()
        .unwrap_or_else(|| default_runtime_dir(slug));
    let saves_dir = settings
        .saves
        .clone()
        .unwrap_or_else(|| default_saves_dir(slug));

    let prepared =
        runtime::prepare_runtime(&disc, settings, &runtime_dir, Some(saves_dir.as_path()))?;
    let dosbox = runtime::find_dosbox(settings.dosbox.as_deref()).ok_or_else(|| {
        "Could not find dosbox-staging (or dosbox). Install dosbox-staging or pass --dosbox <path>."
            .to_string()
    })?;

    let mut cmd = Command::new(&dosbox);
    cmd.current_dir(&prepared.runtime_dir)
        .arg("-noprimaryconf")
        .arg("-nolocalconf")
        .arg("-conf")
        .arg(&prepared.conf_path);

    // Pass settings to libscreamer_hooks via environment variables.
    cmd.env("SCREAMER_GAME", disc.game.slug())
        .env("SCREAMER_EXE", &prepared.selected_exe)
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

    if let Some(hooks_lib) = locate_hooks_library() {
        #[cfg(target_os = "linux")]
        {
            let preload = match std::env::var("LD_PRELOAD") {
                Ok(existing) if !existing.is_empty() => {
                    format!("{}:{existing}", hooks_lib.display())
                }
                _ => hooks_lib.display().to_string(),
            };
            cmd.env("LD_PRELOAD", preload);
        }
    }

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
