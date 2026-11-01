use crate::edition::Game;
use std::path::Path;

/// DOS keyboard scancodes used in `CONFIG/CONTROLS.CFG`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBindings {
    pub accelerate: i32,
    pub brake: i32,
    pub steer_left: i32,
    pub steer_right: i32,
    pub gear_up: i32,
    pub gear_down: i32,
}

impl KeyBindings {
    pub const fn default_for(game: Game) -> Self {
        match game {
            Game::Screamer => Self {
                accelerate: 72,  // Up Arrow
                brake: 80,       // Down Arrow
                steer_left: 75,  // Left Arrow
                steer_right: 77, // Right Arrow
                gear_up: 56,     // Alt
                gear_down: 29,   // LCtrl
            },
            Game::Screamer2 => Self {
                accelerate: 72,
                brake: 80,
                steer_left: 75,
                steer_right: 77,
                gear_up: 54,   // RShift
                gear_down: 29, // LCtrl
            },
            Game::ScreamerRally => Self {
                accelerate: 72,
                brake: 80,
                steer_left: 75,
                steer_right: 77,
                gear_up: 29,   // LCtrl
                gear_down: 56, // Alt
            },
        }
    }
}

/// Generates the contents of `SCREAMER.CFG` (`C:` for HDD data and saves, `D:` for CD-ROM).
pub fn build_screamer_cfg(game: Game) -> Vec<u8> {
    match game {
        Game::Screamer => b"C:\tD:\tC:\tc".to_vec(),
        Game::Screamer2 | Game::ScreamerRally => b"C:\\\tD:\tC:\\\tc".to_vec(),
    }
}

/// Generates a pre-configured `CONFIG/SETUP.CFG` with Sound Blaster 16 (`Port=0x220`,
/// `IRQ=7`, `DMA=1`) and AdLib/OPL3 (`Port=0x388`) matching `dosbox-staging`'s defaults.
pub fn build_setup_cfg(game: Game, sound_irq: u32) -> Vec<u8> {
    let put_u32 = |buf: &mut [u8], off: usize, val: u32| {
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };
    match game {
        Game::Screamer => {
            let mut buf = vec![0u8; 144];
            put_u32(&mut buf, 0x00, 0x0000_E016); // Sound Blaster 16 Digital ID
            put_u32(&mut buf, 0x04, 0x0000_0012); // Capabilities / rate
            put_u32(&mut buf, 0x08, 0x0000_0220); // Port 0x220
            put_u32(&mut buf, 0x0C, sound_irq); // IRQ 7 (matches dosbox default)
            put_u32(&mut buf, 0x10, 0x0000_0001); // DMA 1
            put_u32(&mut buf, 0x20, 0x0000_0388); // AdLib / OPL3 port 0x388
            put_u32(&mut buf, 0x40, 0x0000_0009); // Serial port setting
            buf[0x44..0x4C].copy_from_slice(b"AT Z\\N0\0");
            put_u32(&mut buf, 0x84, 0x0000_0001); // Audio/video configured
            put_u32(&mut buf, 0x8C, 0x0000_0001); // Setup complete
            buf
        }
        Game::Screamer2 => {
            let mut buf = vec![0u8; 160];
            put_u32(&mut buf, 0x00, 0x0000_0009);
            put_u32(&mut buf, 0x04, 0x00A4_01EB);
            put_u32(&mut buf, 0x08, 0x0000_E016); // Sound Blaster 16 Digital ID
            put_u32(&mut buf, 0x0C, 0x0000_0012);
            put_u32(&mut buf, 0x10, 0x0000_0220); // Port 0x220
            put_u32(&mut buf, 0x14, sound_irq); // IRQ 7
            put_u32(&mut buf, 0x18, 0x0000_0001); // DMA 1
            put_u32(&mut buf, 0x1C, 0x0000_0000);
            put_u32(&mut buf, 0x3C, 0x0000_2580); // 9600 baud
            put_u32(&mut buf, 0x40, 0x0000_0001);
            put_u32(&mut buf, 0x44, 0x0000_0000);
            put_u32(&mut buf, 0x4C, 0x0000_0009);
            buf[0x50..0x58].copy_from_slice(b"AT Z\\N0\0");
            put_u32(&mut buf, 0x98, 0x0000_0001);
            put_u32(&mut buf, 0x9C, 0x0000_0001);
            buf
        }
        Game::ScreamerRally => {
            let mut buf = vec![0u8; 160];
            put_u32(&mut buf, 0x00, 0x0000_0009);
            put_u32(&mut buf, 0x04, 0x00A3_01EA);
            put_u32(&mut buf, 0x08, 0x0000_E016); // Sound Blaster 16 Digital ID
            put_u32(&mut buf, 0x0C, 0x0000_0012);
            put_u32(&mut buf, 0x10, 0x0000_0220); // Port 0x220
            put_u32(&mut buf, 0x14, sound_irq); // IRQ 7
            put_u32(&mut buf, 0x18, 0x0000_0001); // DMA 1
            put_u32(&mut buf, 0x1C, 0x0000_0225);
            put_u32(&mut buf, 0x3C, 0x0000_2580); // 9600 baud
            put_u32(&mut buf, 0x40, 0x0000_0001);
            put_u32(&mut buf, 0x44, 0x0000_0002);
            put_u32(&mut buf, 0x4C, 0x0000_0009);
            buf[0x50..0x58].copy_from_slice(b"AT Z\\N0\0");
            put_u32(&mut buf, 0x98, 0x0000_0001);
            put_u32(&mut buf, 0x9C, 0x0000_0001);
            buf
        }
    }
}

/// Generates a pre-configured `CONFIG/CONTROLS.CFG` (`1328` bytes for Screamer 1,
/// `1496` bytes for Screamer 2 and Screamer Rally) with keyboard bindings and
/// pre-calibrated analog joystick records.
pub fn build_controls_cfg(game: Game, keys: KeyBindings) -> Vec<u8> {
    let put_rec = |buf: &mut [u8], rec_idx: usize, words: [i32; 7]| {
        let base = rec_idx * 28;
        for (i, w) in words.iter().enumerate() {
            buf[base + i * 4..base + (i + 1) * 4].copy_from_slice(&w.to_le_bytes());
        }
    };
    let put_i32 = |buf: &mut [u8], off: usize, val: i32| {
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };

    match game {
        Game::Screamer => {
            let mut buf = vec![0u8; 1328];
            // Preset 0: Keyboard 1 (6 records)
            put_rec(&mut buf, 0, [1, 0, 1, 0, 1, 1, keys.accelerate]);
            put_rec(&mut buf, 1, [1, 0, 1, 0, 1, 1, keys.brake]);
            put_rec(&mut buf, 2, [1, 0, 1, 0, 0, 1, keys.steer_left]);
            put_rec(&mut buf, 3, [1, 0, 1, 0, 0, 1, keys.steer_right]);
            put_rec(&mut buf, 4, [1, 0, 1, 0, 0, 1, keys.gear_up]);
            put_rec(&mut buf, 5, [1, 0, 1, 0, 0, 1, keys.gear_down]);

            // Device calibration & button records (records 6..45)
            put_rec(
                &mut buf,
                6,
                [
                    0,
                    12,
                    333,
                    5,
                    513,
                    0x42CC_999Au32 as i32,
                    0x42CC_999Au32 as i32,
                ],
            );
            put_rec(&mut buf, 7, [1, 0, 333, 4, 515, 0, 0]);
            put_rec(&mut buf, 8, [1, 0, 333, 4, 517, 0, 0]);
            put_rec(&mut buf, 9, [1, 0, 333, 4, 519, 0, 0]);

            put_rec(
                &mut buf,
                16,
                [
                    0,
                    142,
                    227,
                    4,
                    513,
                    0x40F6_22A6u32 as i32,
                    0x40F6_22A6u32 as i32,
                ],
            );
            put_rec(&mut buf, 17, [1, 0, 226, 3, 515, 0, 0]);
            put_rec(&mut buf, 18, [1, 0, 226, 3, 517, 0, 0]);
            put_rec(&mut buf, 19, [1, 0, 226, 3, 519, 0, 0]);

            put_rec(&mut buf, 36, [0, 0, 442, 4, 513, 0, 0]);
            put_rec(&mut buf, 37, [1, 0, 441, 3, 515, 0, 0]);
            put_rec(&mut buf, 38, [1, 0, 441, 4, 517, 0, 0]);
            put_rec(&mut buf, 39, [1, 0, 441, 4, 519, 0, 0]);

            // Tail 10 u32 words at +0x508
            put_i32(&mut buf, 0x508, 513);
            put_i32(&mut buf, 0x50C, 515);
            put_i32(&mut buf, 0x510, 517);
            put_i32(&mut buf, 0x514, 519);
            buf
        }
        Game::Screamer2 | Game::ScreamerRally => {
            let mut buf = vec![0u8; 1496];
            // Preset 0: Keyboard 1 (6 records)
            put_rec(&mut buf, 0, [1, 0, 1, 0, 1, 1, keys.accelerate]);
            put_rec(&mut buf, 1, [1, 0, 1, 0, 1, 1, keys.brake]);
            put_rec(&mut buf, 2, [1, 0, 1, 0, 0, 1, keys.steer_left]);
            put_rec(&mut buf, 3, [1, 0, 1, 0, 0, 1, keys.steer_right]);
            put_rec(&mut buf, 4, [1, 0, 1, 0, 0, 1, keys.gear_up]);
            put_rec(&mut buf, 5, [1, 0, 1, 0, 0, 1, keys.gear_down]);

            // Preset 1: Keyboard 2 (6 records: W, X, A, D, 2, 1)
            put_rec(&mut buf, 6, [1, 0, 1, 0, 2, 1, 17]);
            put_rec(&mut buf, 7, [1, 0, 1, 0, 2, 1, 45]);
            put_rec(&mut buf, 8, [1, 0, 1, 0, 2, 1, 30]);
            put_rec(&mut buf, 9, [1, 0, 1, 0, 2, 1, 32]);
            put_rec(&mut buf, 10, [1, 0, 1, 0, 0, 1, 3]);
            put_rec(&mut buf, 11, [1, 0, 1, 0, 0, 1, 2]);

            // Device calibration & button records (records 12..51)
            if game == Game::Screamer2 {
                put_rec(&mut buf, 12, [0, 0, 200, 5, 513, 0, 0]);
            } else {
                put_rec(&mut buf, 12, [1, 0, 62, 1, 513, 0, 0]);
                put_rec(&mut buf, 22, [1, 0, 62, 1, 513, 0, 0]);
            }
            put_rec(
                &mut buf,
                16,
                [
                    0,
                    341,
                    187,
                    13,
                    513,
                    0xC3FF_C000u32 as i32,
                    0xC3FF_C000u32 as i32,
                ],
            );
            put_rec(&mut buf, 32, [0, 0, 388, 13, 513, 0, 0]);
            put_rec(&mut buf, 36, [0, 0, 322, 7, 513, 0, 0]);
            put_rec(&mut buf, 42, [0, 0, 123, 5, 513, 0, 0]);
            put_rec(&mut buf, 46, [513, 0, 0, 0, 0, 0, 0]);

            // Tail 10 u32 words at +0x5B0
            put_i32(&mut buf, 0x5B0, 513);
            buf
        }
    }
}

/// Options for generating `dosbox-screamer.conf`.
#[derive(Debug, Clone)]
pub struct DosboxConfOptions<'a> {
    pub fullscreen: bool,
    pub resolution: (u32, u32),
    pub widescreen: bool,
    pub vsync: bool,
    pub voodoo_enabled: bool,
    pub texture_filtering: bool,
    pub glshader: &'a str,
    pub c_drive_dir: &'a Path,
    pub cd_image_path: Option<&'a Path>,
    pub exe_name: &'a str,
    pub exit_after_run: bool,
}

/// Generates a clean `dosbox-staging` configuration file for running Screamer 1, 2, or Rally.
pub fn build_dosbox_conf(opts: &DosboxConfOptions<'_>) -> String {
    let win_res_str = if opts.resolution.0 <= 1 || opts.resolution.1 <= 1 {
        "1280x960".to_string()
    } else {
        format!("{}x{}", opts.resolution.0, opts.resolution.1)
    };
    let aspect_str = if opts.widescreen { "stretch" } else { "auto" };
    // Always use standard 4 MB Voodoo 1 (2 MB FBI + 2 MB TMU0, single TMU).
    // 12 MB selects VOODOO_1_DTMU (4 MB TMU0), which overflows S2/SR's 1024-slot
    // lazy texture cache table before reaching grTexMaxAddress(0).
    let voodoo_mem = "4";
    let voodoo_bilinear = if opts.texture_filtering {
        "true"
    } else {
        "false"
    };
    let glshader = if opts.glshader.is_empty() {
        if opts.texture_filtering {
            "bilinear"
        } else {
            "sharp"
        }
    } else {
        opts.glshader
    };

    let mut autoexec = String::new();
    autoexec.push_str("@echo off\n");
    autoexec.push_str(&format!(
        "mount c \"{}\" -freesize 250\n",
        opts.c_drive_dir.display()
    ));
    if let Some(cd) = opts.cd_image_path {
        autoexec.push_str(&format!("imgmount d \"{}\" -t iso -fs iso\n", cd.display()));
    } else {
        autoexec.push_str(&format!(
            "mount d \"{}\" -t cdrom\n",
            opts.c_drive_dir.display()
        ));
    }
    autoexec.push_str("c:\n");
    autoexec.push_str(&format!("{}\n", opts.exe_name));
    if opts.exit_after_run {
        autoexec.push_str("exit\n");
    }

    format!(
        r#"# Generated by ScreamerLauncher — do not edit manually; regenerated on launch.
[sdl]
fullscreen          = {fullscreen}
display             = 0
fullresolution      = desktop
windowresolution    = {win_res_str}
output              = opengl
vsync               = {vsync}

[dosbox]
machine             = svga_s3
memsize             = 31

[render]
aspect              = {aspect_str}
glshader            = {glshader}

[cpu]
core                = auto
cputype             = pentium
cpu_cycles          = max
cpu_cycles_protected = max

[voodoo]
voodoo              = {voodoo_enabled}
voodoo_memsize      = {voodoo_mem}
voodoo_threads      = auto
voodoo_bilinear_filtering = {voodoo_bilinear}

[mixer]
nosound             = false
rate                = 48000
blocksize           = 1024
prebuffer           = 25

[sblaster]
sbtype              = sb16
sbbase              = 220
irq                 = 7
dma                 = 1
hdma                = 5
sbmixer             = true
oplmode             = auto

[gus]
gus                 = false

[speaker]
pcspeaker           = discrete

[joystick]
joysticktype        = 2axis
timed               = true
autofire            = false
swap34              = false
buttonwrap          = false

[autoexec]
{autoexec}"#,
        fullscreen = opts.fullscreen,
        win_res_str = win_res_str,
        vsync = opts.vsync,
        aspect_str = aspect_str,
        voodoo_enabled = opts.voodoo_enabled,
        voodoo_mem = voodoo_mem,
        voodoo_bilinear = voodoo_bilinear,
        autoexec = autoexec,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screamer_cfg_bytes_match_expected() {
        assert_eq!(build_screamer_cfg(Game::Screamer), b"C:\tD:\tC:\tc");
        assert_eq!(build_screamer_cfg(Game::Screamer2), b"C:\\\tD:\tC:\\\tc");
        assert_eq!(
            build_screamer_cfg(Game::ScreamerRally),
            b"C:\\\tD:\tC:\\\tc"
        );
    }

    #[test]
    fn setup_cfg_sizes_and_sb16_settings() {
        let s1 = build_setup_cfg(Game::Screamer, 7);
        assert_eq!(s1.len(), 144);
        assert_eq!(
            u32::from_le_bytes(s1[0x00..0x04].try_into().unwrap()),
            0xE016
        );
        assert_eq!(
            u32::from_le_bytes(s1[0x08..0x0C].try_into().unwrap()),
            0x220
        );
        assert_eq!(u32::from_le_bytes(s1[0x0C..0x10].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(s1[0x10..0x14].try_into().unwrap()), 1);

        let s2 = build_setup_cfg(Game::Screamer2, 7);
        assert_eq!(s2.len(), 160);
        assert_eq!(
            u32::from_le_bytes(s2[0x08..0x0C].try_into().unwrap()),
            0xE016
        );
        assert_eq!(
            u32::from_le_bytes(s2[0x10..0x14].try_into().unwrap()),
            0x220
        );
        assert_eq!(u32::from_le_bytes(s2[0x14..0x18].try_into().unwrap()), 7);

        let sr = build_setup_cfg(Game::ScreamerRally, 7);
        assert_eq!(sr.len(), 160);
        assert_eq!(
            u32::from_le_bytes(sr[0x04..0x08].try_into().unwrap()),
            0x00A3_01EA
        );
    }

    #[test]
    fn controls_cfg_matches_on_disk_sizes_and_records() {
        let s1 = build_controls_cfg(Game::Screamer, KeyBindings::default_for(Game::Screamer));
        assert_eq!(s1.len(), 1328);
        assert_eq!(i32::from_le_bytes(s1[0x18..0x1C].try_into().unwrap()), 72);
        assert_eq!(
            i32::from_le_bytes(s1[0x508..0x50C].try_into().unwrap()),
            513
        );

        let s2 = build_controls_cfg(Game::Screamer2, KeyBindings::default_for(Game::Screamer2));
        assert_eq!(s2.len(), 1496);
        assert_eq!(
            i32::from_le_bytes(s2[0x5B0..0x5B4].try_into().unwrap()),
            513
        );

        let sr = build_controls_cfg(
            Game::ScreamerRally,
            KeyBindings::default_for(Game::ScreamerRally),
        );
        assert_eq!(sr.len(), 1496);
        assert_eq!(
            i32::from_le_bytes(sr[0x5B0..0x5B4].try_into().unwrap()),
            513
        );
    }

    #[test]
    fn dosbox_conf_renders_expected_sections() {
        let conf = build_dosbox_conf(&DosboxConfOptions {
            fullscreen: false,
            resolution: (1920, 1080),
            widescreen: true,
            vsync: true,
            voodoo_enabled: true,
            texture_filtering: true,
            glshader: "bilinear",
            c_drive_dir: Path::new("/tmp/runtime"),
            cd_image_path: Some(Path::new("/tmp/game/game.ins")),
            exe_name: "STARTH.EXE",
            exit_after_run: true,
        });
        assert!(conf.contains("windowresolution    = 1920x1080"));
        assert!(conf.contains("fullresolution      = desktop"));
        assert!(conf.contains("aspect              = stretch"));
        assert!(conf.contains("irq                 = 7"));
        assert!(conf.contains("imgmount d \"/tmp/game/game.ins\" -t iso -fs iso"));
        assert!(conf.contains("STARTH.EXE"));
    }
}
