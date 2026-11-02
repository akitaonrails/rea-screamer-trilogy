//! Golden snapshots of everything the launcher produces, so an SDK upgrade
//! or a refactor can't change it silently: `--help`, the `launcher.cfg`
//! template, and for each look and a few option combinations the files it
//! writes into the runtime folder, DOSBox's arguments and environment, and
//! the binary fixes it applies (by fix, address and the bytes the launcher
//! writes; never the game's own bytes).
//!
//! A difference fails the test. Review it like code: it must match an
//! intended change or an SDK Migration item. To accept one, run
//! `UPDATE_SNAPSHOTS=1 cargo test --test snapshots` and review the diff.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use screamer_launcher::config::{self, Settings};
use screamer_launcher::edition::{EDITIONS, Game};
use screamer_launcher::form::{self, Form, Look};
use screamer_launcher::runtime::{self, Gpu};

fn snapshot_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn check(name: &str, actual: &str) {
    let path = snapshot_dir().join(name);
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(snapshot_dir()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (UPDATE_SNAPSHOTS=1 creates it)", path.display()));
    // Line endings as checked out (Git may turn them into CRLF on Windows).
    assert!(
        expected.replace("\r\n", "\n") == actual,
        "{name} differs from its snapshot; review, then UPDATE_SNAPSHOTS=1 to accept"
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .chunks(32)
        .map(|row| {
            row.iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Everything one launch produces for `exe` of `game` with `settings`.
fn describe(settings: &Settings, game: Game, exe: &str, gpu: Gpu) -> String {
    let edition = EDITIONS
        .iter()
        .find(|e| e.game == game && e.exe_name == exe)
        .unwrap();
    // A fixed, platform-neutral runtime path.
    let plan = runtime::plan(
        settings,
        game,
        exe,
        edition.role,
        Path::new("/runtime"),
        Some(Path::new("/game/GAME.DAT")),
        gpu,
    );
    let mut out = String::new();
    let _ = writeln!(out, "# {} {exe}\n", game.display_name());
    let _ = writeln!(out, "## Fixes left out\n{:?}\n", plan.skipped_fixes);
    let _ = writeln!(out, "## Fixes the launcher writes into {exe} (VA: bytes)");
    for write in edition.fixes.assemble(0, &plan.skipped_fixes) {
        if write.in_image {
            let _ = writeln!(
                out,
                "{} 0x{:08X}: {}",
                write.fix,
                write.address,
                hex(&write.bytes)
            );
        }
    }
    let _ = writeln!(
        out,
        "\n## DOSBox arguments\n{}\n",
        plan.dosbox_args.join(" ").replace('\\', "/")
    );
    let _ = writeln!(out, "## DOSBox environment");
    for (k, v) in &plan.env {
        let _ = writeln!(out, "{k}={v}");
    }
    for (rel, bytes) in plan.files() {
        if rel.ends_with(".conf") {
            let _ = writeln!(out, "\n## {rel}\n{}", String::from_utf8_lossy(bytes));
        } else {
            let _ = writeln!(out, "\n## {rel} ({} bytes)\n{}", bytes.len(), hex(bytes));
        }
    }
    out
}

fn look(look: Look, screen: (u32, u32)) -> Settings {
    let mut f = Form::new(&Settings::default(), Some(screen));
    f.set_look(look, Some(screen));
    f.settings()
}

fn file(text: &str) -> Settings {
    let (s, warnings) = Settings::parse_file(text);
    assert!(warnings.is_empty(), "{warnings:?}");
    s
}

#[test]
fn help_and_template() {
    let help = config::usage().replace(env!("CARGO_PKG_VERSION"), "<version>");
    check("help.txt", &help);
    check("template.cfg", &config::template());
    check(
        "first-run-desktop.cfg",
        &config::first_run(Some((2560, 1440))),
    );
    check(
        "first-run-steam-deck.cfg",
        &config::first_run(Some(form::STEAM_DECK_SCREEN)),
    );
}

#[test]
fn the_looks() {
    let desk = (2560, 1440);
    check(
        "look-original-screamer.txt",
        &describe(
            &look(Look::Original, desk),
            Game::Screamer,
            "STARTH.EXE",
            Gpu::Default,
        ),
    );
    check(
        "look-enhanced-screamer2-3dfx.txt",
        &describe(
            &look(Look::Enhanced, desk),
            Game::Screamer2,
            "S2_3DFX.EXE",
            Gpu::Default,
        ),
    );
    check(
        "look-steam-deck-rally-3dfx.txt",
        &describe(
            &look(Look::SteamDeck, form::STEAM_DECK_SCREEN),
            Game::ScreamerRally,
            "STRT3FX.EXE",
            Gpu::Amd,
        ),
    );
}

#[test]
fn option_combinations() {
    check(
        "combo-svga16-fsr-framegen-novsync.txt",
        &describe(
            &file(
                "renderer = svga16\ntexture_filter = fsr\nframe_generation = on\nvsync = off\nresolution = 2560x1440\n",
            ),
            Game::Screamer2,
            "START65H.EXE",
            Gpu::Default,
        ),
    );
    check(
        "combo-pad-off-custom-buttons-fullscreen-nvidia.txt",
        &describe(
            &file(
                "gamepad_axes = off\ndeadzone = 12\nrumble = 0\nbuttons = 1,2,3,4,5,6,7,8\nresolution = fullscreen\nwidescreen = off\nextended_draw_distance = off\n",
            ),
            Game::ScreamerRally,
            "STARTH.EXE",
            Gpu::Nvidia,
        ),
    );
    check(
        "combo-0.6-launcher-cfg-fps-limit-100.txt",
        &describe(
            &file("fps_limit = 100\nvsync = true\ntexture_filter = smooth\nrenderer = vga\n"),
            Game::Screamer,
            "STARTL.EXE",
            Gpu::Default,
        ),
    );
}
