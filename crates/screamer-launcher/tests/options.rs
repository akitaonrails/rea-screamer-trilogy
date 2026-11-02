//! Every option end to end: the `launcher.cfg` key and its flag reach the
//! settings, and the settings reach what the launcher writes and passes on
//! (the DOSBox configuration, DOSBox's environment, the binary fixes, the
//! executable). Plus `--help`, bad values, old `launcher.cfg` files and
//! hostile paths.

use std::path::{Path, PathBuf};

use screamer_launcher::config::{self, ArgsError, OPTIONS, RendererChoice, Settings};
use screamer_launcher::edition::{EDITIONS, ExeRole, Game};
use screamer_launcher::form::{self, Form, Look};
use screamer_launcher::runtime::{self, DiscoveredExe, DiscoveredGame, Gpu, LaunchPlan};

fn args(a: &[&str]) -> Result<Settings, ArgsError> {
    Settings::parse_args(a.iter().map(|s| s.to_string()))
}

fn file(text: &str) -> Settings {
    let (s, warnings) = Settings::parse_file(text);
    assert!(warnings.is_empty(), "{warnings:?}");
    s
}

/// Screamer 2's 3dfx build, run from `/rt`.
fn plan(s: &Settings) -> LaunchPlan {
    runtime::plan(
        s,
        Game::Screamer2,
        "S2_3DFX.EXE",
        ExeRole::Glide3dfx,
        Path::new("/rt"),
        None,
        Gpu::Default,
    )
}

fn env<'a>(p: &'a LaunchPlan, key: &str) -> &'a str {
    p.env
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("no {key}"))
}

fn conf_line(p: &LaunchPlan, key: &str) -> String {
    p.dosbox_conf
        .lines()
        .find(|l| l.split('=').next().is_some_and(|k| k.trim() == key))
        .unwrap_or_else(|| panic!("no {key} in the DOSBox conf"))
        .split('=')
        .nth(1)
        .unwrap()
        .trim()
        .to_string()
}

/// The same `value` for `key` from the file and from its flag gives the
/// same settings, and `check` holds for the plan.
fn both(key: &str, value: &str, check: impl Fn(&Settings, &LaunchPlan)) {
    let from_file = file(&format!("{key} = {value}\n"));
    let flag = format!("--{}", key.replace('_', "-"));
    let from_flag = args(&[&flag, value]).unwrap();
    assert_eq!(from_file, from_flag, "{key} = {value}");
    check(&from_file, &plan(&from_file));
}

#[test]
fn which_selects_the_game() {
    both("which", "screamer2", |s, _| {
        assert_eq!(s.which_game(), Game::Screamer2)
    });
    for (word, game) in [
        ("1", Game::Screamer),
        ("2", Game::Screamer2),
        ("rally", Game::ScreamerRally),
    ] {
        assert_eq!(args(&[word]).unwrap().which, Some(game));
        assert_eq!(args(&["--which", word]).unwrap().which, Some(game));
        assert_eq!(args(&["--play", word]).unwrap().which, Some(game));
        assert_eq!(file(&format!("which = {word}\n")).which, Some(game));
    }
    let p = runtime::plan(
        &Settings::default(),
        Game::ScreamerRally,
        "STARTH.EXE",
        ExeRole::Svga8Bit,
        Path::new("/rt"),
        None,
        Gpu::Default,
    );
    assert_eq!(env(&p, "SCREAMER_GAME"), "screamer-rally");
    assert_eq!(env(&p, "SCREAMER_EXE"), "STARTH.EXE");
}

#[test]
fn folders_are_taken_as_written() {
    for key in ["game", "saves", "runtime", "dosbox"] {
        both(key, "/games/Jogos d'Ação/José", |s, _| {
            let got = match key {
                "game" => &s.game,
                "saves" => &s.saves,
                "runtime" => &s.runtime,
                _ => &s.dosbox,
            };
            assert_eq!(got, &Some(PathBuf::from("/games/Jogos d'Ação/José")));
        });
    }
}

/// A Screamer 2 folder holding every renderer (fake paths; the editions
/// are the real table's).
fn screamer2() -> DiscoveredGame {
    let exes = EDITIONS
        .iter()
        .filter(|e| e.game == Game::Screamer2)
        .map(|e| DiscoveredExe {
            path: PathBuf::from("/g").join(e.exe_name),
            file_name: e.exe_name.to_string(),
            edition: e,
            sha256: e.sha256,
        })
        .collect();
    DiscoveredGame {
        root_dir: "/g".into(),
        exe_dir: "/g".into(),
        game: Game::Screamer2,
        executables: exes,
        cd_image: None,
        glide2x_ovl_source: None,
        has_genuine_dos_glide2x: true,
        has_windows_stub_glide2x: false,
        unknown: Vec::new(),
    }
}

#[test]
fn renderer_picks_the_executable() {
    let game = screamer2();
    for (value, exe) in [
        ("auto", "S2_3DFX.EXE"),
        ("3dfx", "S2_3DFX.EXE"),
        ("svga16", "START65H.EXE"),
        ("svga8", "STARTH.EXE"),
        // Screamer 2 has no VGA build: the best one it has.
        ("vga", "S2_3DFX.EXE"),
    ] {
        both("renderer", value, |s, _| {
            let selected = game.select_executable(s.renderer_choice()).unwrap();
            assert_eq!(selected.file_name, exe, "{value}");
        });
    }
    // Older spellings.
    assert_eq!(
        file("renderer = glide\n").renderer,
        Some(RendererChoice::Voodoo)
    );
    assert_eq!(
        args(&["--renderer", "START65H"]).unwrap().renderer,
        Some(RendererChoice::Svga16Bit)
    );
}

#[test]
fn resolution_sizes_the_dosbox_window() {
    both("resolution", "2560x1440", |_, p| {
        assert_eq!(conf_line(p, "windowresolution"), "2560x1440");
        assert_eq!(conf_line(p, "fullscreen"), "false");
        assert_eq!(env(p, "SCREAMER_VOODOO_SCALE"), "2");
    });
    both("resolution", "fullscreen", |_, p| {
        assert_eq!(conf_line(p, "fullscreen"), "true");
    });
    both("resolution", "original", |_, p| {
        assert_eq!(conf_line(p, "fullscreen"), "false");
        // The original 4:3 window; the Voodoo renders at its own 640×480.
        assert_eq!(env(p, "SCREAMER_VOODOO_SCALE"), "1");
    });
    // Unset: 1080p.
    let p = plan(&Settings::default());
    assert_eq!(conf_line(&p, "windowresolution"), "1920x1080");
}

#[test]
fn widescreen_patches_the_camera_and_stretches() {
    both("widescreen", "on", |_, p| {
        assert!(!p.skipped_fixes.contains(&"E2"));
        assert_eq!(conf_line(p, "aspect"), "stretch");
    });
    both("widescreen", "off", |_, p| {
        assert!(p.skipped_fixes.contains(&"E2"));
        assert_eq!(conf_line(p, "aspect"), "auto");
    });
}

#[test]
fn extended_draw_distance_is_fix_e6() {
    both("extended_draw_distance", "on", |_, p| {
        assert!(!p.skipped_fixes.contains(&"E6"))
    });
    both("extended_draw_distance", "off", |_, p| {
        assert!(p.skipped_fixes.contains(&"E6"))
    });
}

#[test]
fn frame_generation_reaches_dosbox() {
    both("frame_generation", "on", |_, p| {
        assert_eq!(env(p, "SCREAMER_FRAME_GEN"), "1")
    });
    both("frame_generation", "off", |_, p| {
        assert_eq!(env(p, "SCREAMER_FRAME_GEN"), "0")
    });
    assert_eq!(env(&plan(&Settings::default()), "SCREAMER_FRAME_GEN"), "0");
}

#[test]
fn vsync_reaches_the_dosbox_conf() {
    both("vsync", "on", |_, p| {
        assert_eq!(conf_line(p, "vsync"), "auto")
    });
    both("vsync", "off", |_, p| {
        assert_eq!(conf_line(p, "vsync"), "false")
    });
    assert_eq!(conf_line(&plan(&Settings::default()), "vsync"), "auto");
}

#[test]
fn texture_filter_picks_the_shader() {
    for (value, shader, bilinear) in [
        ("original", "crt-auto", "false"),
        ("fsr", "interpolation/fsr", "true"),
        ("smooth", "bilinear", "true"),
        ("sharp", "sharp", "false"),
    ] {
        both("texture_filter", value, |_, p| {
            assert_eq!(conf_line(p, "glshader"), shader);
            assert_eq!(conf_line(p, "voodoo_bilinear_filtering"), bilinear);
        });
    }
}

#[test]
fn gamepad_options_reach_the_hooks() {
    both("gamepad_axes", "off", |_, p| {
        assert_eq!(env(p, "SCREAMER_GAMEPAD_AXES"), "0");
        assert!(p.skipped_fixes.contains(&"E5"));
    });
    both("gamepad_axes", "on", |_, p| {
        assert_eq!(env(p, "SCREAMER_GAMEPAD_AXES"), "1");
        assert!(!p.skipped_fixes.contains(&"E5"));
    });
    both("deadzone", "15", |_, p| {
        assert_eq!(env(p, "SCREAMER_DEADZONE"), "15");
        // The game's own 7.8% deadzone stays when one is asked for.
        assert!(p.skipped_fixes.contains(&"E5"));
    });
    both("deadzone", "0", |_, p| {
        assert!(!p.skipped_fixes.contains(&"E5"))
    });
    both("rumble", "30", |_, p| {
        assert_eq!(env(p, "SCREAMER_RUMBLE"), "30")
    });
    both("buttons", "original", |_, p| {
        assert_eq!(env(p, "SCREAMER_BUTTONS"), "original")
    });
    both("buttons", "1,2,3,4,5,6,7,8", |_, p| {
        assert_eq!(env(p, "SCREAMER_BUTTONS"), "1,2,3,4,5,6,7,8")
    });
    let p = plan(&Settings::default());
    assert_eq!(env(&p, "SCREAMER_BUTTONS"), "modern");
    assert_eq!(env(&p, "SCREAMER_RUMBLE"), "60");
    assert_eq!(env(&p, "SCREAMER_DEADZONE"), "0");
    assert_eq!(file("deadzone = off\n").deadzone, Some(0));
    assert_eq!(args(&["--rumble", "off"]).unwrap().rumble, Some(0));
}

#[test]
fn scaler_is_a_key_and_a_one_run_flag() {
    both("scaler", "pixel", |s, _| {
        assert_eq!(config::cfg_value(s, "scaler").as_deref(), Some("pixel"));
        let saved = config::save_keys("", s, &["scaler"]);
        assert_eq!(saved, "scaler = pixel\n");
    });
    // --scaler lasts one run: screamer-play reads it (REA_HELPER_KEYS),
    // the launcher never writes it into launcher.cfg.
    assert_eq!(config::REMEMBERED, ["game", "which"]);
}

#[test]
fn the_gpu_choice_reaches_dosbox() {
    let with = |gpu| {
        runtime::plan(
            &Settings::default(),
            Game::Screamer,
            "STARTH.EXE",
            ExeRole::Svga8Bit,
            Path::new("/rt"),
            None,
            gpu,
        )
    };
    assert_eq!(env(&with(Gpu::Nvidia), "__NV_PRIME_RENDER_OFFLOAD"), "1");
    assert_eq!(env(&with(Gpu::Amd), "DRI_PRIME"), "1");
    assert!(
        !with(Gpu::Default)
            .env
            .iter()
            .any(|(k, _)| k.contains("PRIME"))
    );
}

#[test]
fn help_lists_every_flag_and_goes_to_stdout() {
    for flag in ["--help", "-h", "/?"] {
        assert_eq!(args(&[flag]), Err(ArgsError::Help));
    }
    let usage = config::usage();
    for row in OPTIONS {
        assert!(
            usage.contains(&format!("--{} ", row.key.replace('_', "-"))),
            "{} missing from --help",
            row.key
        );
    }
    for switch in ["--no-window", "--settings", "--check", "--controllers"] {
        assert!(usage.contains(switch), "{switch}");
    }
    assert!(usage.contains("3: closed without Play"));
    assert!(usage.contains("4: no window possible"));
    assert!(usage.contains("fullscreen"));
}

#[test]
fn bad_values_say_what_they_need() {
    let bad = |a: &[&str]| match args(a) {
        Err(ArgsError::Bad(m)) => m,
        other => panic!("{a:?}: {other:?}"),
    };
    assert_eq!(
        bad(&["--deadzone", "50"]),
        "--deadzone needs 0 to 30, not 50"
    );
    assert_eq!(
        bad(&["--widescreen", "maybe"]),
        "--widescreen needs on or off, not maybe"
    );
    assert_eq!(
        bad(&["--which", "4"]),
        "--which needs screamer, screamer2 or screamer-rally, not 4"
    );
    assert_eq!(bad(&["--game"]), "--game needs a value");
    assert_eq!(bad(&["--bogus", "1"]), "unknown argument --bogus");
    assert!(bad(&["--resolution", "100x100"]).starts_with("--resolution needs WIDTHxHEIGHT"));
    assert!(bad(&["--buttons", "1,2"]).contains("8 button numbers"));
    assert!(bad(&["--scaler", "sharpest"]).contains("nearest, pixel, fsr or linear"));

    let (_, warnings) = Settings::parse_file("rumble = loud\nbogus = 1\n");
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("rumble needs 0 to 100"));
    assert!(warnings[1].contains("unknown setting \"bogus\""));
}

#[test]
fn the_template_lists_every_option_and_sets_nothing() {
    let template = config::template();
    for row in OPTIONS {
        assert!(template.contains(&format!("#{} =", row.key)), "{}", row.key);
    }
    assert_eq!(file(&template), Settings::default());
}

/// The template 0.6.0 wrote, as players have it.
const TEMPLATE_0_6: &str = "# Screamer Trilogy launcher settings.
which = screamer
game = /games/screamer
#runtime =
renderer = auto
resolution = 1920x1080
widescreen = true
extended_draw_distance = true
# Frame-rate target: 30 is the unchanged original timing, 50 is the smooth native
fps_limit = 50
vsync = true
texture_filter = sharp
gamepad_axes = true
deadzone = 0
rumble = 60
buttons = modern
scaler = fsr
";

#[test]
fn a_0_6_launcher_cfg_still_reads_without_warnings() {
    let s = file(TEMPLATE_0_6);
    assert_eq!(s.frame_generation, Some(false));
    assert_eq!(s.vsync, Some(true));
    let screen = Some((2560, 1440));
    assert_eq!(Form::new(&s, screen).look, Look::Enhanced);
    for (old, frame_gen) in [("30", false), ("50", false), ("100", true), ("off", false)] {
        let s = file(&format!("fps_limit = {old}\n"));
        assert_eq!(s.frame_generation, Some(frame_gen), "fps_limit = {old}");
        let flag = args(&["--fps-limit", old]).unwrap();
        assert_eq!(flag.frame_generation, Some(frame_gen));
    }
    // Saving writes the new key and retires the old line.
    let saved = Form::new(&s, screen).save(TEMPLATE_0_6);
    assert!(saved.contains("#fps_limit =\n"), "{saved}");
    assert!(saved.contains("frame_generation = off\n"), "{saved}");
    assert_eq!(file(&saved).frame_generation, Some(false));
}

#[test]
fn an_fps_limit_0_6_never_took_is_still_ignored() {
    // 0.6.0 took 0, off and 10 to 1000; `fps_limit = 1` was refused, not
    // read as the switch's `1` (on).
    for bad in ["1", "5", "9", "1001"] {
        let (s, warnings) = Settings::parse_file(&format!("fps_limit = {bad}\n"));
        assert_eq!(s.frame_generation, None, "fps_limit = {bad}");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("a number from 10 to 1000 or off"),
            "{warnings:?}"
        );
        let joined = format!("--fps-limit={bad}");
        for flag in [&["--fps-limit", bad][..], &[joined.as_str()]] {
            match args(flag) {
                Err(ArgsError::Bad(msg)) => assert_eq!(
                    msg,
                    format!("--fps-limit needs a number from 10 to 1000 or off, not {bad}")
                ),
                other => panic!("{flag:?}: {other:?}"),
            }
        }
    }
    // The old spellings read the same as `--key=value`.
    let s = args(&["--fps-limit=100", "--which=2", "--rumble=off"]).unwrap();
    assert_eq!(s.frame_generation, Some(true));
    assert_eq!(s.which, Some(Game::Screamer2));
    assert_eq!(s.rumble, Some(0));
    assert_eq!(file("fps_limit = 10\n").frame_generation, Some(false));
    assert_eq!(file("fps_limit = 0\n").frame_generation, Some(false));
}

#[test]
fn a_0_6_original_look_is_still_the_original_look() {
    // What 0.6.0's window saved for the Original look (vsync "as the game"
    // left unset, which DOSBox ran with vsync on).
    let old = "renderer = auto\nresolution = original\nwidescreen = false\n\
               extended_draw_distance = false\nfps_limit = 30\ntexture_filter = original\n";
    let f = Form::new(&file(old), Some((1920, 1080)));
    assert_eq!(f.look, Look::Original);
    let p = plan(&file(old));
    assert_eq!(conf_line(&p, "vsync"), "auto");
    assert_eq!(env(&p, "SCREAMER_FRAME_GEN"), "0");
}

#[test]
fn a_launcher_cfg_in_a_windows_code_page_is_read() {
    // "game = C:\Jogos d'Ação" saved by Notepad in Windows-1252.
    let bytes = b"game = C:\\Jogos d'A\xe7\xe3o\nwhich = 2\n".to_vec();
    let s = file(&rea_launcher::cfg::decode(bytes));
    assert_eq!(s.game, Some(PathBuf::from("C:\\Jogos d'Ação")));
    assert_eq!(s.which, Some(Game::Screamer2));
}

#[test]
fn hostile_paths_reach_dosbox_quoted() {
    let rt = Path::new("/home/José/Jogos d'Ação/runtime");
    let cd = Path::new("/home/José/Jogos d'Ação/SCREAM.DAT");
    let p = runtime::plan(
        &Settings::default(),
        Game::Screamer,
        "STARTH.EXE",
        ExeRole::Svga8Bit,
        rt,
        Some(cd),
        Gpu::Default,
    );
    assert!(
        p.dosbox_conf
            .contains("mount c \"/home/José/Jogos d'Ação/runtime\" -freesize 250\n")
    );
    assert!(
        p.dosbox_conf
            .contains("imgmount d \"/home/José/Jogos d'Ação/SCREAM.DAT\" -t iso -fs iso\n")
    );
    let conf = p.dosbox_args.last().unwrap();
    assert!(
        conf.starts_with("/home/José/Jogos d'Ação/runtime"),
        "{conf}"
    );
    assert!(conf.ends_with("dosbox-screamer.conf"), "{conf}");
}

#[test]
fn the_first_run_saves_the_recommended_look() {
    let deck = config::first_run(Some(form::STEAM_DECK_SCREEN));
    let f = Form::new(&file(&deck), Some(form::STEAM_DECK_SCREEN));
    assert_eq!(f.look, Look::SteamDeck);
    assert!(deck.contains("resolution = 1280x800\n"));
    let desk = config::first_run(Some((2560, 1440)));
    assert_eq!(
        Form::new(&file(&desk), Some((2560, 1440))).look,
        Look::Enhanced
    );
    assert!(desk.contains("#game =\n") && desk.contains("#saves =\n"));
}
