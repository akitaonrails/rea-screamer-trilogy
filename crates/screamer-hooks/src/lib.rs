//! `libscreamer_hooks`: Runtime `dosbox-staging` bridge for Screamer (1995),
//! Screamer 2 (1996), and Screamer Rally (1997).
//!
//! Resolves `MemBase`, `KEYBOARD_AddKey`, `JOYSTICK_*`, `SDL_GameController*`,
//! and `SDL_Joystick*` symbols from the host `dosbox-staging` process to provide:
//! - Automatic support for both SDL GameController DB controllers and unmapped
//!   Linux/Windows XInput joysticks (e.g., 8BitDo Ultimate 2 Wireless)
//! - Direct guest RAM (`MemBase`) injection into the Screamer Data Object
//!   (`record[0..5]` + `normalized_controls[0..5]`), bypassing DOS port `0x201`
//!   timing loops for true proportional `-1023..+1023` steering and `0..1023`
//!   independent L2/R2 analog triggers
//! - Automatic in-race vs menu heartbeat detection via `normalized_controls[3]`
//!   so D-Pad, Left Stick, A/B, and Start/Select navigate all game menus and
//!   skip intros out of the box without manual mapper setup
//! - Simultaneous native keyboard + gamepad operation
//! - Telemetry/load-driven controller rumble (`SDL_GameControllerRumble` /
//!   `SDL_JoystickRumble`)

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// Exact Data Object layout specification for each of the 8 racing executables
/// across Screamer (1995), Screamer 2 (1996), and Screamer Rally (1997).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinaryControlSpec {
    pub id: &'static str,
    pub game_env: &'static str,
    pub exe_env: &'static str,
    /// Offset of `"%s\\config\\controls.cfg\0"` inside the Data Object.
    pub anchor_offset: usize,
    /// Offset of `record[0..5]` (`6 * 28 = 0xA8` bytes) followed immediately by
    /// `normalized_controls[0..5]` (`6 * 4 = 0x18` bytes) at `+0xA8`.
    pub controls_base_offset: usize,
    /// Number of `0xC0`-byte control blocks (`1` in Screamer 1, `4` in Screamer 2/Rally).
    pub block_count: usize,
}

pub const BINARY_CONTROL_SPECS: &[BinaryControlSpec] = &[
    BinaryControlSpec {
        id: "S1_STARTH",
        game_env: "screamer",
        exe_env: "STARTH.EXE",
        anchor_offset: 0x09AF,
        controls_base_offset: 0x197D8,
        block_count: 1,
    },
    BinaryControlSpec {
        id: "S1_STARTL",
        game_env: "screamer",
        exe_env: "STARTL.EXE",
        anchor_offset: 0x09AF,
        controls_base_offset: 0x197D4,
        block_count: 1,
    },
    BinaryControlSpec {
        id: "S2_3DFX",
        game_env: "screamer2",
        exe_env: "S2_3DFX.EXE",
        anchor_offset: 0x104A,
        controls_base_offset: 0x18140,
        block_count: 4,
    },
    BinaryControlSpec {
        id: "S2_START65H",
        game_env: "screamer2",
        exe_env: "START65H.EXE",
        anchor_offset: 0x1206,
        controls_base_offset: 0x17890,
        block_count: 4,
    },
    BinaryControlSpec {
        id: "S2_STARTH",
        game_env: "screamer2",
        exe_env: "STARTH.EXE",
        anchor_offset: 0x0E46,
        controls_base_offset: 0x17714,
        block_count: 4,
    },
    BinaryControlSpec {
        id: "SR_STRT3FX",
        game_env: "screamer-rally",
        exe_env: "STRT3FX.EXE",
        anchor_offset: 0x10E2,
        controls_base_offset: 0x19BD0,
        block_count: 4,
    },
    BinaryControlSpec {
        id: "SR_START65H",
        game_env: "screamer-rally",
        exe_env: "START65H.EXE",
        anchor_offset: 0x10E2,
        controls_base_offset: 0x196A0,
        block_count: 4,
    },
    BinaryControlSpec {
        id: "SR_STARTH",
        game_env: "screamer-rally",
        exe_env: "STARTH.EXE",
        anchor_offset: 0x0D22,
        controls_base_offset: 0x18F24,
        block_count: 4,
    },
];

/// Compatibility alias for anchor lookup tests.
pub const DATA_OBJECT_ANCHORS: &[(&str, usize)] = &[
    ("S1_STARTH", 0x09AF),
    ("S1_STARTL", 0x09AF),
    ("S2_3DFX", 0x104A),
    ("S2_START65H", 0x1206),
    ("S2_STARTH", 0x0E46),
    ("SR_STRT3FX", 0x10E2),
    ("SR_START65H", 0x10E2),
    ("SR_STARTH", 0x0D22),
];

pub const ANCHOR_SIGNATURE: &[u8] = b"%s\\config\\controls.cfg\0";

/// `dosbox-staging` `KBD_KEYS` enum values verified from `_Z21KEYBOARD_GetScanCode18KBD_KEYSb`.
pub mod kbd_keys {
    pub const KBD_ESC: u32 = 49;
    pub const KBD_BACKSPACE: u32 = 51;
    pub const KBD_ENTER: u32 = 52;
    pub const KBD_SPACE: u32 = 53;
    pub const KBD_C: u32 = 32;
    pub const KBD_M: u32 = 36;
    pub const KBD_LEFT: u32 = 86;
    pub const KBD_UP: u32 = 87;
    pub const KBD_DOWN: u32 = 88;
    pub const KBD_RIGHT: u32 = 89;
}

/// Synthetic key bitmask flags used by [`inject_race_controls_with_synth_mask`]
/// so synthetic arrow keys emitted for menu navigation never get mistaken for
/// physical keyboard overrides during a race.
pub const SYNTH_KEY_BIT_UP: usize = 1 << 0;
pub const SYNTH_KEY_BIT_DOWN: usize = 1 << 1;
pub const SYNTH_KEY_BIT_LEFT: usize = 1 << 2;
pub const SYNTH_KEY_BIT_RIGHT: usize = 1 << 3;
pub const SYNTH_KEY_BIT_ENTER: usize = 1 << 4;
pub const SYNTH_KEY_BIT_ESC: usize = 1 << 5;
pub const SYNTH_KEY_BIT_CAMERA: usize = 1 << 6;
pub const SYNTH_KEY_BIT_BACKSPACE: usize = 1 << 7;

/// Normalized race controls ready to inject into `normalized_controls[0..5]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GamepadRaceState {
    /// Accelerate (`0..=1023`).
    pub throttle: i32,
    /// Brake (`0..=1023`).
    pub brake: i32,
    /// Proportional steering (`-1023..=1023`, negative = left, positive = right).
    pub steer: i32,
    /// Shift gear up.
    pub gear_up: bool,
    /// Shift gear down.
    pub gear_down: bool,
}

fn read_i32_le(buf: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn write_i32_le(buf: &mut [u8], off: usize, val: i32) {
    buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
}

/// Checks whether a candidate `controls_base_offset` inside `data_obj` looks
/// like an initialized `CONTROLS.CFG` table (`record[0].enabled` in `1..=2` and
/// `record[0].scancode` in `1..=88`).
pub fn is_valid_control_block(data_obj: &[u8], controls_base_offset: usize) -> bool {
    if controls_base_offset + 0xC0 > data_obj.len() {
        return false;
    }
    let enabled = read_i32_le(data_obj, controls_base_offset);
    let scancode = read_i32_le(data_obj, controls_base_offset + 0x18);
    (1..=2).contains(&enabled) && (1..=88).contains(&scancode)
}

/// Scans guest RAM (`mem`) on 4096-byte page boundaries for the Screamer Data
/// Object anchor string `"%s\\config\\controls.cfg\0"` and returns the byte
/// offset of the start of the Data Object.
pub fn locate_data_object(mem: &[u8]) -> Option<usize> {
    locate_binary_data_object(mem, None, None).map(|(base, _)| base)
}

/// Locates both the Data Object base offset in `mem` and the matching
/// [`BinaryControlSpec`] on 4096-byte page boundaries, disambiguating
/// binaries that share an anchor offset (`S1_STARTH` vs `S1_STARTL`,
/// `SR_STRT3FX` vs `SR_START65H`) via optional `game_hint` / `exe_hint` or by
/// inspecting `record[0]` at `controls_base_offset`.
pub fn locate_binary_data_object(
    mem: &[u8],
    game_hint: Option<&str>,
    exe_hint: Option<&str>,
) -> Option<(usize, &'static BinaryControlSpec)> {
    let page_size = 4096usize;
    let sig_len = ANCHOR_SIGNATURE.len();
    let mut page_base = 0usize;
    let mut fallback: Option<(usize, &'static BinaryControlSpec)> = None;

    while page_base + page_size <= mem.len() {
        for spec in BINARY_CONTROL_SPECS {
            let pos = page_base + spec.anchor_offset;
            if pos + sig_len <= mem.len()
                && mem[pos..pos + sig_len].eq_ignore_ascii_case(ANCHOR_SIGNATURE)
            {
                if let Some(exe) = exe_hint
                    && (!spec.exe_env.eq_ignore_ascii_case(exe)
                        || game_hint.is_some_and(|g| !spec.game_env.eq_ignore_ascii_case(g)))
                {
                    continue;
                }

                let rem = &mem[page_base..];
                let is_live = is_valid_control_block(rem, spec.controls_base_offset)
                    || (spec.block_count == 4
                        && spec.controls_base_offset >= 0x180
                        && is_valid_control_block(rem, spec.controls_base_offset - 0x180));
                if is_live {
                    return Some((page_base, spec));
                }
                if fallback.is_none() {
                    fallback = Some((page_base, spec));
                }
            }
        }
        page_base += page_size;
    }
    fallback
}

fn is_synth_scancode(scancode: i32, synth_key_mask: usize) -> bool {
    match scancode {
        72 => (synth_key_mask & SYNTH_KEY_BIT_UP) != 0,
        80 => (synth_key_mask & SYNTH_KEY_BIT_DOWN) != 0,
        75 => (synth_key_mask & SYNTH_KEY_BIT_LEFT) != 0,
        77 => (synth_key_mask & SYNTH_KEY_BIT_RIGHT) != 0,
        28 => (synth_key_mask & SYNTH_KEY_BIT_ENTER) != 0,
        1 => (synth_key_mask & SYNTH_KEY_BIT_ESC) != 0,
        14 => (synth_key_mask & SYNTH_KEY_BIT_BACKSPACE) != 0,
        _ => false,
    }
}

fn inject_single_control_block(
    data_obj: &mut [u8],
    base: usize,
    gp: &GamepadRaceState,
    synth_key_mask: usize,
) -> bool {
    if base + 0xC0 > data_obj.len() || !is_valid_control_block(data_obj, base) {
        return false;
    }

    // In the real DOS/4GW binary, the 130-byte keyboard state array (`0` or `1` per
    // scancode) lives in its own preceding 0x82-byte BSS LE object, while `data_obj[0..130]`
    // contains initialized ASCII strings (`> 1`). Guard `has_clean_kbd_array` so ASCII
    // string bytes in `data_obj[0..130]` are never mistaken for held keyboard keys.
    let has_clean_kbd_array = data_obj
        .get(2..90)
        .is_some_and(|b| b.iter().all(|&x| x <= 1));

    let is_key_down = |buf: &[u8], scancode: i32| -> bool {
        if !has_clean_kbd_array || is_synth_scancode(scancode, synth_key_mask) {
            return false;
        }
        if (1..90).contains(&scancode) {
            let idx = 2 + scancode as usize;
            idx < buf.len() && buf[idx] == 1
        } else {
            false
        }
    };

    let rec_off = |r: usize| base + r * 28;
    let norm_off = |r: usize| base + 0xA8 + r * 4;

    // 0: Accelerate (0..=1023, ctrl_type=1, mode=0 so MenuPollInput ignores norm[0] in menus)
    let sc0 = read_i32_le(data_obj, rec_off(0) + 0x18);
    if gp.throttle == 0 && is_key_down(data_obj, sc0) {
        write_i32_le(data_obj, rec_off(0), 1);
        write_i32_le(data_obj, rec_off(0) + 0x04, 0);
        write_i32_le(data_obj, rec_off(0) + 0x08, 1);
    } else {
        write_i32_le(data_obj, rec_off(0), 2);
        write_i32_le(data_obj, rec_off(0) + 0x04, 1);
        write_i32_le(data_obj, rec_off(0) + 0x08, 0);
        write_i32_le(data_obj, rec_off(0) + 0x14, 1);
        write_i32_le(data_obj, norm_off(0), gp.throttle.clamp(0, 1023));
    }

    // 1: Brake (0..=1023, ctrl_type=1, mode=0 for direct proportional analog brake)
    let sc1 = read_i32_le(data_obj, rec_off(1) + 0x18);
    if gp.brake == 0 && is_key_down(data_obj, sc1) {
        write_i32_le(data_obj, rec_off(1), 1);
        write_i32_le(data_obj, rec_off(1) + 0x04, 0);
        write_i32_le(data_obj, rec_off(1) + 0x08, 1);
    } else {
        write_i32_le(data_obj, rec_off(1), 2);
        write_i32_le(data_obj, rec_off(1) + 0x04, 1);
        write_i32_le(data_obj, rec_off(1) + 0x08, 0);
        write_i32_le(data_obj, rec_off(1) + 0x14, 1);
        write_i32_le(data_obj, norm_off(1), gp.brake.clamp(0, 1023));
    }

    // 2 & 3: Steering (-1023..=1023 in norm[2] with ctrl_type=1, mode=0 for full analog steering)
    let sc2 = read_i32_le(data_obj, rec_off(2) + 0x18);
    let sc3 = read_i32_le(data_obj, rec_off(3) + 0x18);
    if gp.steer == 0 && (is_key_down(data_obj, sc2) || is_key_down(data_obj, sc3)) {
        write_i32_le(data_obj, rec_off(2), 1);
        write_i32_le(data_obj, rec_off(2) + 0x04, 0);
        write_i32_le(data_obj, rec_off(2) + 0x08, 1);
        write_i32_le(data_obj, rec_off(3), 1);
        write_i32_le(data_obj, rec_off(3) + 0x04, 0);
        write_i32_le(data_obj, rec_off(3) + 0x08, 1);
    } else {
        write_i32_le(data_obj, rec_off(2), 2);
        write_i32_le(data_obj, rec_off(2) + 0x04, 1);
        write_i32_le(data_obj, rec_off(2) + 0x08, 0);
        write_i32_le(data_obj, rec_off(2) + 0x14, 1);
        write_i32_le(data_obj, norm_off(2), gp.steer.clamp(-1023, 1023));

        write_i32_le(data_obj, rec_off(3), 2);
        write_i32_le(data_obj, rec_off(3) + 0x04, 1);
        write_i32_le(data_obj, rec_off(3) + 0x08, 0);
        write_i32_le(data_obj, rec_off(3) + 0x14, 1);
        write_i32_le(data_obj, norm_off(3), 0);
    }

    // 4: Gear Up (0 or 1023, ctrl_type=1, mode=0)
    let sc4 = read_i32_le(data_obj, rec_off(4) + 0x18);
    if !gp.gear_up && is_key_down(data_obj, sc4) {
        write_i32_le(data_obj, rec_off(4), 1);
        write_i32_le(data_obj, rec_off(4) + 0x04, 0);
        write_i32_le(data_obj, rec_off(4) + 0x08, 1);
    } else {
        write_i32_le(data_obj, rec_off(4), 2);
        write_i32_le(data_obj, rec_off(4) + 0x04, 1);
        write_i32_le(data_obj, rec_off(4) + 0x08, 0);
        write_i32_le(data_obj, rec_off(4) + 0x14, 1);
        write_i32_le(data_obj, norm_off(4), if gp.gear_up { 1023 } else { 0 });
    }

    // 5: Gear Down (0 or 1023, ctrl_type=1, mode=0)
    let sc5 = read_i32_le(data_obj, rec_off(5) + 0x18);
    if !gp.gear_down && is_key_down(data_obj, sc5) {
        write_i32_le(data_obj, rec_off(5), 1);
        write_i32_le(data_obj, rec_off(5) + 0x04, 0);
        write_i32_le(data_obj, rec_off(5) + 0x08, 1);
    } else {
        write_i32_le(data_obj, rec_off(5), 2);
        write_i32_le(data_obj, rec_off(5) + 0x04, 1);
        write_i32_le(data_obj, rec_off(5) + 0x08, 0);
        write_i32_le(data_obj, rec_off(5) + 0x14, 1);
        write_i32_le(data_obj, norm_off(5), if gp.gear_down { 1023 } else { 0 });
    }

    true
}

/// Injects `gp` into Player 1's control block inside `data_obj` (starting at
/// `spec.controls_base_offset`), yielding individual controls to the physical
/// keyboard whenever `data_obj[2 + scancode] == 1` in a clean keyboard array
/// (excluding synthetic keys in `synth_key_mask`).
pub fn inject_race_controls_with_synth_mask(
    data_obj: &mut [u8],
    spec: &BinaryControlSpec,
    gp: &GamepadRaceState,
    synth_key_mask: usize,
) -> bool {
    let base = spec.controls_base_offset;
    let mut injected = inject_single_control_block(data_obj, base, gp, synth_key_mask);
    if spec.block_count == 4 && base >= 0x180 {
        injected |= inject_single_control_block(data_obj, base - 0x180, gp, synth_key_mask);
        injected |= inject_single_control_block(data_obj, base - 0x0C0, gp, synth_key_mask);
        injected |= inject_single_control_block(data_obj, base + 0x0C0, gp, synth_key_mask);
    }
    injected
}

/// Injects `gp` into Player 1's control block inside `data_obj` with no synthetic
/// key exclusions.
pub fn inject_race_controls(
    data_obj: &mut [u8],
    spec: &BinaryControlSpec,
    gp: &GamepadRaceState,
) -> bool {
    inject_race_controls_with_synth_mask(data_obj, spec, gp, 0)
}

/// Applies a radial/axial deadzone (`deadzone_pct` in `0..=30`) to a raw SDL
/// axis value (`-32768..=32767`) and rescales the remaining range smoothly to
/// `-32767..=32767`.
pub fn apply_deadzone(raw: i16, deadzone_pct: u32) -> i16 {
    let v = (raw as i32).clamp(-32767, 32767);
    let dz = ((deadzone_pct.min(30) as i32) * 32767) / 100;
    if v.abs() <= dz {
        return 0;
    }
    let sign = v.signum();
    let mag = v.abs() - dz;
    let span = (32767 - dz).max(1);
    ((mag * 32767) / span * sign).clamp(-32767, 32767) as i16
}

/// Normalizes a raw `SDL_Joystick` trigger axis (`-32768..=32767`, where `-32768`
/// is released and `+32767` is full pull) into `0..=32767`, guarding against
/// Linux `evdev` drivers that report `0` before the trigger is touched for the
/// first time (`seen_nonzero`).
pub fn normalize_joystick_trigger(raw: i16, seen_nonzero: &mut bool) -> i16 {
    if raw != 0 {
        *seen_nonzero = true;
    }
    if !*seen_nonzero {
        return 0;
    }
    (((raw as i32) + 32768) / 2).clamp(0, 32767) as i16
}

/// Combines independent L2 (`0..=32767`) and R2 (`0..=32767`) triggers with
/// the right stick vertical axis (`-32768..=32767`, where negative is up/throttle)
/// into DOSBox's Y-axis (`-32767..=32767`, where negative is accelerate and
/// positive is brake).
pub fn combine_triggers(trigger_left: i16, trigger_right: i16, right_stick_y: i16) -> i16 {
    let l2 = (trigger_left as i32).clamp(0, 32767);
    let r2 = (trigger_right as i32).clamp(0, 32767);
    let trigger_net = l2 - r2; // -32767 (full throttle) .. +32767 (full brake)
    let stick_y = apply_deadzone(right_stick_y, 10) as i32;
    if trigger_net.abs() >= stick_y.abs() {
        trigger_net.clamp(-32767, 32767) as i16
    } else {
        stick_y.clamp(-32767, 32767) as i16
    }
}

/// Computes low-frequency and high-frequency rumble intensities (`0..=65535`)
/// from car speed/collision delta and configured `rumble_pct` (`0..=100`).
pub fn compute_rumble(impact_delta: u32, skid_level: u32, rumble_pct: u32) -> (u16, u16) {
    if rumble_pct == 0 {
        return (0, 0);
    }
    let scale = rumble_pct.min(100);
    let low = ((impact_delta.min(1000) * 65535 / 1000) * scale / 100).min(65535) as u16;
    let high = ((skid_level.min(1000) * 65535 / 1000) * scale / 100).min(65535) as u16;
    (low, high)
}

#[cfg(unix)]
mod unix_hook {
    use super::*;
    use std::ffi::{CStr, c_char, c_int, c_long, c_short, c_uint, c_ushort, c_void};

    #[repr(C)]
    pub struct Timespec {
        pub tv_sec: c_long,
        pub tv_nsec: c_long,
    }

    const RTLD_DEFAULT: *mut c_void = std::ptr::null_mut();
    const RTLD_NEXT: *mut c_void = (-1isize) as *mut c_void;

    /// Size of guest DOS RAM allocated by `memsize = 31` (`31 MiB`).
    const GUEST_MEM_SCAN_BYTES: usize = 31 * 1024 * 1024;

    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    type ClockGettimeFn = unsafe extern "C" fn(c_int, *mut Timespec) -> c_int;
    type SdlSwapWindowFn = unsafe extern "C" fn(*mut c_void);
    type SdlWasInitFn = unsafe extern "C" fn(c_uint) -> c_uint;
    type SdlInitSubSystemFn = unsafe extern "C" fn(c_uint) -> c_int;
    type SdlNumJoysticksFn = unsafe extern "C" fn() -> c_int;
    type SdlGameControllerOpenFn = unsafe extern "C" fn(c_int) -> *mut c_void;
    type SdlGameControllerGetAttachedFn = unsafe extern "C" fn(*mut c_void) -> c_int;
    type SdlGameControllerGetAxisFn = unsafe extern "C" fn(*mut c_void, c_int) -> c_short;
    type SdlGameControllerGetButtonFn = unsafe extern "C" fn(*mut c_void, c_int) -> u8;
    type SdlGameControllerRumbleFn =
        unsafe extern "C" fn(*mut c_void, c_ushort, c_ushort, c_uint) -> c_int;
    type SdlJoystickOpenFn = unsafe extern "C" fn(c_int) -> *mut c_void;
    type SdlJoystickGetAttachedFn = unsafe extern "C" fn(*mut c_void) -> c_int;
    type SdlJoystickGetAxisFn = unsafe extern "C" fn(*mut c_void, c_int) -> c_short;
    type SdlJoystickGetButtonFn = unsafe extern "C" fn(*mut c_void, c_int) -> u8;
    type SdlJoystickGetHatFn = unsafe extern "C" fn(*mut c_void, c_int) -> u8;
    type SdlJoystickRumbleFn =
        unsafe extern "C" fn(*mut c_void, c_ushort, c_ushort, c_uint) -> c_int;
    type JoystickEnableFn = unsafe extern "C" fn(u8, bool);
    type JoystickMoveFn = unsafe extern "C" fn(u8, i16);
    type JoystickButtonFn = unsafe extern "C" fn(u8, c_int, bool);
    type KeyboardAddKeyFn = unsafe extern "C" fn(u32, bool);

    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    static SUBSYSTEM_INIT_DONE: AtomicBool = AtomicBool::new(false);
    static ENABLED: AtomicBool = AtomicBool::new(true);
    static IS_SCREAMER_1: AtomicBool = AtomicBool::new(false);
    static DEADZONE_PCT: AtomicUsize = AtomicUsize::new(0);
    static RUMBLE_PCT: AtomicUsize = AtomicUsize::new(60);
    static LAST_POLL_NS: AtomicU64 = AtomicU64::new(0);
    static LAST_OPEN_ATTEMPT_NS: AtomicU64 = AtomicU64::new(0);
    static LAST_SCAN_ATTEMPT_NS: AtomicU64 = AtomicU64::new(0);
    static LAST_RACE_HEARTBEAT_NS: AtomicU64 = AtomicU64::new(0);
    static IN_POLL: AtomicBool = AtomicBool::new(false);

    static CONTROLLER_PTR: AtomicUsize = AtomicUsize::new(0);
    static JOYSTICK_PTR: AtomicUsize = AtomicUsize::new(0);
    static SEEN_L2_NONZERO: AtomicBool = AtomicBool::new(false);
    static SEEN_R2_NONZERO: AtomicBool = AtomicBool::new(false);

    /// Cached page base offset and `BINARY_CONTROL_SPECS` index (`usize::MAX` if uncached).
    static CACHED_DATA_OBJ_BASE: AtomicUsize = AtomicUsize::new(usize::MAX);
    static CACHED_SPEC_IDX: AtomicUsize = AtomicUsize::new(usize::MAX);

    /// Bitmask of synthetic keys currently held via `KEYBOARD_AddKey`.
    static HELD_KEYS_MASK: AtomicUsize = AtomicUsize::new(0);

    const KEY_BIT_UP: usize = SYNTH_KEY_BIT_UP;
    const KEY_BIT_DOWN: usize = SYNTH_KEY_BIT_DOWN;
    const KEY_BIT_LEFT: usize = SYNTH_KEY_BIT_LEFT;
    const KEY_BIT_RIGHT: usize = SYNTH_KEY_BIT_RIGHT;
    const KEY_BIT_ENTER: usize = SYNTH_KEY_BIT_ENTER;
    const KEY_BIT_ESC: usize = SYNTH_KEY_BIT_ESC;
    const KEY_BIT_CAMERA: usize = SYNTH_KEY_BIT_CAMERA;
    const KEY_BIT_BACKSPACE: usize = SYNTH_KEY_BIT_BACKSPACE;

    unsafe fn dlsym_default(name: &[u8]) -> *mut c_void {
        let Ok(cstr) = CStr::from_bytes_with_nul(name) else {
            return std::ptr::null_mut();
        };
        unsafe { dlsym(RTLD_DEFAULT, cstr.as_ptr() as *const c_char) }
    }

    unsafe fn dlsym_next(name: &[u8]) -> *mut c_void {
        let Ok(cstr) = CStr::from_bytes_with_nul(name) else {
            return std::ptr::null_mut();
        };
        unsafe { dlsym(RTLD_NEXT, cstr.as_ptr() as *const c_char) }
    }

    fn init_once() {
        if INITIALIZED.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Ok(v) = std::env::var("SCREAMER_GAMEPAD_AXES")
            && (v == "0" || v.eq_ignore_ascii_case("false"))
        {
            ENABLED.store(false, Ordering::Relaxed);
        }
        if let Ok(v) = std::env::var("SCREAMER_DEADZONE")
            && let Ok(dz) = v.parse::<usize>()
        {
            DEADZONE_PCT.store(dz.min(30), Ordering::Relaxed);
        }
        if let Ok(v) = std::env::var("SCREAMER_RUMBLE")
            && let Ok(r) = v.parse::<usize>()
        {
            RUMBLE_PCT.store(r.min(100), Ordering::Relaxed);
        }
        if let Ok(game) = std::env::var("SCREAMER_GAME")
            && game.eq_ignore_ascii_case("screamer")
        {
            IS_SCREAMER_1.store(true, Ordering::Relaxed);
        }
    }

    #[derive(Default)]
    struct RawPadState {
        lx: i16,
        ly: i16,
        ry: i16,
        l2: i16,
        r2: i16,
        a: bool,
        b: bool,
        x: bool,
        y: bool,
        lb: bool,
        rb: bool,
        back: bool,
        start: bool,
        dpad_up: bool,
        dpad_down: bool,
        dpad_left: bool,
        dpad_right: bool,
    }

    unsafe fn read_controller_or_joystick(now_ns: u64) -> Option<RawPadState> {
        let mut gc = CONTROLLER_PTR.load(Ordering::Relaxed) as *mut c_void;
        let mut js = JOYSTICK_PTR.load(Ordering::Relaxed) as *mut c_void;

        // Verify existing handle is still attached
        if !gc.is_null() {
            let att_ptr = unsafe { dlsym_default(b"SDL_GameControllerGetAttached\0") };
            if !att_ptr.is_null() {
                let att_fn: SdlGameControllerGetAttachedFn = unsafe { std::mem::transmute(att_ptr) };
                if unsafe { att_fn(gc) } == 0 {
                    CONTROLLER_PTR.store(0, Ordering::Relaxed);
                    gc = std::ptr::null_mut();
                }
            }
        }
        if !js.is_null() {
            let att_ptr = unsafe { dlsym_default(b"SDL_JoystickGetAttached\0") };
            if !att_ptr.is_null() {
                let att_fn: SdlJoystickGetAttachedFn = unsafe { std::mem::transmute(att_ptr) };
                if unsafe { att_fn(js) } == 0 {
                    JOYSTICK_PTR.store(0, Ordering::Relaxed);
                    js = std::ptr::null_mut();
                }
            }
        }

        // Try opening a controller or joystick at most once every 250 ms if none is open
        if gc.is_null() && js.is_null() {
            let last_try = LAST_OPEN_ATTEMPT_NS.load(Ordering::Relaxed);
            if now_ns != 0 && now_ns.wrapping_sub(last_try) < 250_000_000 {
                return None;
            }
            LAST_OPEN_ATTEMPT_NS.store(now_ns, Ordering::Relaxed);

            let num_ptr = unsafe { dlsym_default(b"SDL_NumJoysticks\0") };
            let count = if !num_ptr.is_null() {
                let num_fn: SdlNumJoysticksFn = unsafe { std::mem::transmute(num_ptr) };
                unsafe { num_fn() }.max(1)
            } else {
                1
            };

            let gc_open_ptr = unsafe { dlsym_default(b"SDL_GameControllerOpen\0") };
            if !gc_open_ptr.is_null() {
                let gc_open: SdlGameControllerOpenFn = unsafe { std::mem::transmute(gc_open_ptr) };
                for idx in 0..count {
                    let h = unsafe { gc_open(idx) };
                    if !h.is_null() {
                        gc = h;
                        CONTROLLER_PTR.store(gc as usize, Ordering::Relaxed);
                        break;
                    }
                }
            }

            if gc.is_null() {
                let js_open_ptr = unsafe { dlsym_default(b"SDL_JoystickOpen\0") };
                if !js_open_ptr.is_null() {
                    let js_open: SdlJoystickOpenFn = unsafe { std::mem::transmute(js_open_ptr) };
                    for idx in 0..count {
                        let h = unsafe { js_open(idx) };
                        if !h.is_null() {
                            js = h;
                            SEEN_L2_NONZERO.store(false, Ordering::Relaxed);
                            SEEN_R2_NONZERO.store(false, Ordering::Relaxed);
                            JOYSTICK_PTR.store(js as usize, Ordering::Relaxed);
                            break;
                        }
                    }
                }
            }

            if !gc.is_null() || !js.is_null() {
                let joy_en_ptr = unsafe { dlsym_default(b"_Z15JOYSTICK_Enablehb\0") };
                if !joy_en_ptr.is_null() {
                    let en_fn: JoystickEnableFn = unsafe { std::mem::transmute(joy_en_ptr) };
                    unsafe { en_fn(0, true) };
                }
            }
        }

        if !gc.is_null() {
            let axis_ptr = unsafe { dlsym_default(b"SDL_GameControllerGetAxis\0") };
            let btn_ptr = unsafe { dlsym_default(b"SDL_GameControllerGetButton\0") };
            if axis_ptr.is_null() || btn_ptr.is_null() {
                return None;
            }
            let axis_fn: SdlGameControllerGetAxisFn = unsafe { std::mem::transmute(axis_ptr) };
            let btn_fn: SdlGameControllerGetButtonFn = unsafe { std::mem::transmute(btn_ptr) };
            Some(RawPadState {
                lx: unsafe { axis_fn(gc, 0) },
                ly: unsafe { axis_fn(gc, 1) },
                ry: unsafe { axis_fn(gc, 3) },
                l2: unsafe { axis_fn(gc, 4) },
                r2: unsafe { axis_fn(gc, 5) },
                a: unsafe { btn_fn(gc, 0) } != 0,
                b: unsafe { btn_fn(gc, 1) } != 0,
                x: unsafe { btn_fn(gc, 2) } != 0,
                y: unsafe { btn_fn(gc, 3) } != 0,
                back: unsafe { btn_fn(gc, 4) } != 0,
                start: unsafe { btn_fn(gc, 6) } != 0,
                lb: unsafe { btn_fn(gc, 9) } != 0,
                rb: unsafe { btn_fn(gc, 10) } != 0,
                dpad_up: unsafe { btn_fn(gc, 11) } != 0,
                dpad_down: unsafe { btn_fn(gc, 12) } != 0,
                dpad_left: unsafe { btn_fn(gc, 13) } != 0,
                dpad_right: unsafe { btn_fn(gc, 14) } != 0,
            })
        } else if !js.is_null() {
            let axis_ptr = unsafe { dlsym_default(b"SDL_JoystickGetAxis\0") };
            let btn_ptr = unsafe { dlsym_default(b"SDL_JoystickGetButton\0") };
            let hat_ptr = unsafe { dlsym_default(b"SDL_JoystickGetHat\0") };
            if axis_ptr.is_null() || btn_ptr.is_null() {
                return None;
            }
            let axis_fn: SdlJoystickGetAxisFn = unsafe { std::mem::transmute(axis_ptr) };
            let btn_fn: SdlJoystickGetButtonFn = unsafe { std::mem::transmute(btn_ptr) };
            let hat = if !hat_ptr.is_null() {
                let hat_fn: SdlJoystickGetHatFn = unsafe { std::mem::transmute(hat_ptr) };
                unsafe { hat_fn(js, 0) }
            } else {
                0
            };

            let raw_l2 = unsafe { axis_fn(js, 2) };
            let raw_r2 = unsafe { axis_fn(js, 5) };
            let mut seen_l2 = SEEN_L2_NONZERO.load(Ordering::Relaxed);
            let mut seen_r2 = SEEN_R2_NONZERO.load(Ordering::Relaxed);
            let l2 = normalize_joystick_trigger(raw_l2, &mut seen_l2);
            let r2 = normalize_joystick_trigger(raw_r2, &mut seen_r2);
            SEEN_L2_NONZERO.store(seen_l2, Ordering::Relaxed);
            SEEN_R2_NONZERO.store(seen_r2, Ordering::Relaxed);

            // Standard Linux XInput / 8BitDo / Xbox / Steam Deck evdev layout
            Some(RawPadState {
                lx: unsafe { axis_fn(js, 0) },
                ly: unsafe { axis_fn(js, 1) },
                ry: unsafe { axis_fn(js, 4) },
                l2,
                r2,
                a: unsafe { btn_fn(js, 0) } != 0,
                b: unsafe { btn_fn(js, 1) } != 0,
                x: unsafe { btn_fn(js, 2) } != 0,
                y: unsafe { btn_fn(js, 3) } != 0,
                lb: unsafe { btn_fn(js, 4) } != 0,
                rb: unsafe { btn_fn(js, 5) } != 0,
                back: unsafe { btn_fn(js, 6) } != 0,
                start: unsafe { btn_fn(js, 7) } != 0,
                dpad_up: (hat & 0x01) != 0,
                dpad_right: (hat & 0x02) != 0,
                dpad_down: (hat & 0x04) != 0,
                dpad_left: (hat & 0x08) != 0,
            })
        } else {
            None
        }
    }

    unsafe fn update_guest_ram_and_check_in_race(
        gp: &GamepadRaceState,
        synth_key_mask: usize,
        now_ns: u64,
    ) -> bool {
        let membase_sym = unsafe { dlsym_default(b"MemBase\0") as *const *mut u8 };
        if membase_sym.is_null() {
            return false;
        }
        let mem_ptr = unsafe { *membase_sym };
        if mem_ptr.is_null() {
            return false;
        }
        let guest_ram = unsafe { std::slice::from_raw_parts_mut(mem_ptr, GUEST_MEM_SCAN_BYTES) };

        let mut base = CACHED_DATA_OBJ_BASE.load(Ordering::Relaxed);
        let mut spec_idx = CACHED_SPEC_IDX.load(Ordering::Relaxed);

        let cache_valid = if base < GUEST_MEM_SCAN_BYTES && spec_idx < BINARY_CONTROL_SPECS.len() {
            let spec = &BINARY_CONTROL_SPECS[spec_idx];
            let anchor_pos = base + spec.anchor_offset;
            anchor_pos + ANCHOR_SIGNATURE.len() <= guest_ram.len()
                && guest_ram[anchor_pos..anchor_pos + ANCHOR_SIGNATURE.len()]
                    .eq_ignore_ascii_case(ANCHOR_SIGNATURE)
                && (is_valid_control_block(&guest_ram[base..], spec.controls_base_offset)
                    || (spec.block_count == 4
                        && spec.controls_base_offset >= 0x180
                        && is_valid_control_block(
                            &guest_ram[base..],
                            spec.controls_base_offset - 0x180,
                        )))
        } else {
            false
        };

        if !cache_valid {
            let last_scan = LAST_SCAN_ATTEMPT_NS.load(Ordering::Relaxed);
            if now_ns != 0 && now_ns.wrapping_sub(last_scan) < 100_000_000 {
                return false;
            }
            LAST_SCAN_ATTEMPT_NS.store(now_ns, Ordering::Relaxed);

            let game_env = std::env::var("SCREAMER_GAME").ok();
            let exe_env = std::env::var("SCREAMER_EXE").ok();
            if let Some((found_base, found_spec)) = locate_binary_data_object(
                guest_ram,
                game_env.as_deref(),
                exe_env.as_deref(),
            ) {
                base = found_base;
                spec_idx = BINARY_CONTROL_SPECS
                    .iter()
                    .position(|s| s.id == found_spec.id)
                    .unwrap_or(0);
                CACHED_DATA_OBJ_BASE.store(base, Ordering::Relaxed);
                CACHED_SPEC_IDX.store(spec_idx, Ordering::Relaxed);
                if found_spec.game_env == "screamer" {
                    IS_SCREAMER_1.store(true, Ordering::Relaxed);
                }
            } else {
                return false;
            }
        }

        let spec = &BINARY_CONTROL_SPECS[spec_idx];
        let injected = inject_race_controls_with_synth_mask(
            &mut guest_ram[base..],
            spec,
            gp,
            synth_key_mask,
        );
        if injected {
            LAST_RACE_HEARTBEAT_NS.store(now_ns, Ordering::Relaxed);
        }
        injected
    }

    unsafe fn emit_edge_keys(new_mask: usize) {
        let prev_mask = HELD_KEYS_MASK.swap(new_mask, Ordering::Relaxed);
        let diff = prev_mask ^ new_mask;
        if diff == 0 {
            return;
        }
        let kbd_ptr = unsafe { dlsym_default(b"_Z15KEYBOARD_AddKey8KBD_KEYSb\0") };
        if kbd_ptr.is_null() {
            return;
        }
        let add_key: KeyboardAddKeyFn = unsafe { std::mem::transmute(kbd_ptr) };
        let cam_key = if IS_SCREAMER_1.load(Ordering::Relaxed) {
            kbd_keys::KBD_M
        } else {
            kbd_keys::KBD_C
        };

        let bindings: &[(usize, u32)] = &[
            (KEY_BIT_UP, kbd_keys::KBD_UP),
            (KEY_BIT_DOWN, kbd_keys::KBD_DOWN),
            (KEY_BIT_LEFT, kbd_keys::KBD_LEFT),
            (KEY_BIT_RIGHT, kbd_keys::KBD_RIGHT),
            (KEY_BIT_ENTER, kbd_keys::KBD_ENTER),
            (KEY_BIT_ESC, kbd_keys::KBD_ESC),
            (KEY_BIT_CAMERA, cam_key),
            (KEY_BIT_BACKSPACE, kbd_keys::KBD_BACKSPACE),
        ];

        for &(bit, key) in bindings {
            if (diff & bit) != 0 {
                let pressed = (new_mask & bit) != 0;
                unsafe { add_key(key, pressed) };
            }
        }
    }

    unsafe fn poll_gamepad_and_inject(now_ns: u64) {
        if IN_POLL
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        struct PollGuard;
        impl Drop for PollGuard {
            fn drop(&mut self) {
                IN_POLL.store(false, Ordering::Release);
            }
        }
        let _guard = PollGuard;

        init_once();
        if !ENABLED.load(Ordering::Relaxed) {
            return;
        }

        let was_init_ptr = unsafe { dlsym_default(b"SDL_WasInit\0") };
        if !was_init_ptr.is_null() {
            let was_init_fn: SdlWasInitFn = unsafe { std::mem::transmute(was_init_ptr) };
            // Wait until SDL_INIT_VIDEO (0x20) is initialized
            if unsafe { was_init_fn(0x20) } == 0 {
                return;
            }
            if !SUBSYSTEM_INIT_DONE.swap(true, Ordering::Relaxed) {
                let sub_ptr = unsafe { dlsym_default(b"SDL_InitSubSystem\0") };
                if !sub_ptr.is_null() {
                    let sub_fn: SdlInitSubSystemFn = unsafe { std::mem::transmute(sub_ptr) };
                    // Ensure both SDL_INIT_JOYSTICK (0x200) and SDL_INIT_GAMECONTROLLER (0x2000) are active
                    unsafe {
                        let _ = sub_fn(0x2200);
                    }
                }
            }
        }

        let Some(pad) = (unsafe { read_controller_or_joystick(now_ns) }) else {
            return;
        };

        let dz = DEADZONE_PCT.load(Ordering::Relaxed) as u32;
        let steer_x = apply_deadzone(pad.lx, dz);
        let pedals_y = combine_triggers(pad.l2, pad.r2, pad.ry);

        // Also keep DOSBox's internal joystick state updated
        let joy_x_ptr = unsafe { dlsym_default(b"_Z15JOYSTICK_Move_Xhs\0") };
        let joy_y_ptr = unsafe { dlsym_default(b"_Z15JOYSTICK_Move_Yhs\0") };
        let joy_btn_ptr = unsafe { dlsym_default(b"_Z15JOYSTICK_Buttonhib\0") };
        if !joy_x_ptr.is_null() && !joy_y_ptr.is_null() {
            let move_x: JoystickMoveFn = unsafe { std::mem::transmute(joy_x_ptr) };
            let move_y: JoystickMoveFn = unsafe { std::mem::transmute(joy_y_ptr) };
            unsafe {
                move_x(0, steer_x);
                move_y(0, pedals_y);
            }
        }
        if !joy_btn_ptr.is_null() {
            let joy_btn: JoystickButtonFn = unsafe { std::mem::transmute(joy_btn_ptr) };
            unsafe {
                joy_btn(0, 0, pad.a || pad.rb);
                joy_btn(0, 1, pad.b || pad.lb);
            }
        }

        // Synthesize edge-triggered keyboard keys for Menus / Name Input / Intro / Pause / Camera:
        // - D-Pad always navigates Up/Down/Left/Right; Left Stick also navigates menus when
        //   no race throttle/brake button or trigger is held and the stick is pushed along
        //   a dominant cardinal axis (preventing accidental Up/Down keystrokes while steering)
        // - A (South) and Start emit Enter (confirm menu item / confirm driver name / start race)
        // - B (East) and Back/Select emit Esc (back out of menu / pause race)
        // - X (West) emits Backspace (delete character on Screamer 2/Rally name input)
        // - Y (North) emits Camera toggle ('M' in Screamer 1, 'C' in Screamer 2/Rally)
        let pedals_idle =
            !pad.a && !pad.x && pad.r2 < 2000 && pad.l2 < 2000 && (pad.ry as i32).abs() < 8000;
        let lx_abs = (pad.lx as i32).abs();
        let ly_abs = (pad.ly as i32).abs();
        let stick_menu_up = pedals_idle && pad.ly < -22000 && lx_abs < 10000;
        let stick_menu_down = pedals_idle && pad.ly > 22000 && lx_abs < 10000;
        let stick_menu_left = pedals_idle && pad.lx < -22000 && ly_abs < 10000;
        let stick_menu_right = pedals_idle && pad.lx > 22000 && ly_abs < 10000;

        let mut key_mask = 0usize;
        if pad.dpad_up || stick_menu_up {
            key_mask |= KEY_BIT_UP;
        }
        if pad.dpad_down || stick_menu_down {
            key_mask |= KEY_BIT_DOWN;
        }
        if pad.dpad_left || stick_menu_left {
            key_mask |= KEY_BIT_LEFT;
        }
        if pad.dpad_right || stick_menu_right {
            key_mask |= KEY_BIT_RIGHT;
        }
        if pad.a || pad.start {
            key_mask |= KEY_BIT_ENTER;
        }
        if pad.b || pad.back {
            key_mask |= KEY_BIT_ESC;
        }
        if pad.x {
            key_mask |= KEY_BIT_BACKSPACE;
        }
        if pad.y {
            key_mask |= KEY_BIT_CAMERA;
        }

        // Compute normalized race controls (-1023..1023 steer, 0..1023 throttle/brake)
        let mut steer_norm = ((steer_x as i32) * 1023 / 32767).clamp(-1023, 1023);
        if steer_norm == 0 {
            if pad.dpad_left {
                steer_norm = -1023;
            } else if pad.dpad_right {
                steer_norm = 1023;
            }
        }

        let stick_y = apply_deadzone(pad.ry, 10) as i32;
        let r2_norm = ((pad.r2 as i32).clamp(0, 32767) * 1023) / 32767;
        let stick_throttle = if stick_y < 0 {
            (-stick_y * 1023) / 32767
        } else {
            0
        };
        let a_throttle = if pad.a { 1023 } else { 0 };
        let throttle_norm = r2_norm.max(stick_throttle).max(a_throttle).clamp(0, 1023);

        let l2_norm = ((pad.l2 as i32).clamp(0, 32767) * 1023) / 32767;
        let stick_brake = if stick_y > 0 {
            (stick_y * 1023) / 32767
        } else {
            0
        };
        let x_brake = if pad.x { 1023 } else { 0 };
        let brake_norm = l2_norm.max(stick_brake).max(x_brake).clamp(0, 1023);

        let gp_race = GamepadRaceState {
            throttle: throttle_norm,
            brake: brake_norm,
            steer: steer_norm,
            gear_up: pad.rb,
            gear_down: pad.lb,
        };

        let active_mask = key_mask | HELD_KEYS_MASK.load(Ordering::Relaxed);
        let in_race =
            unsafe { update_guest_ram_and_check_in_race(&gp_race, active_mask, now_ns) };

        unsafe { emit_edge_keys(key_mask) };

        // Optional subtle rumble feedback when braking hard in race
        let rumble_pct = RUMBLE_PCT.load(Ordering::Relaxed) as u32;
        if in_race && rumble_pct > 0 {
            let brake_load = if brake_norm > 500 {
                ((brake_norm as u32 - 500) * 400) / 523
            } else {
                0
            };
            let (low, high) = compute_rumble(brake_load, 0, rumble_pct);
            if low > 0 || high > 0 {
                let gc = CONTROLLER_PTR.load(Ordering::Relaxed) as *mut c_void;
                let js = JOYSTICK_PTR.load(Ordering::Relaxed) as *mut c_void;
                if !gc.is_null() {
                    let rumble_ptr = unsafe { dlsym_default(b"SDL_GameControllerRumble\0") };
                    if !rumble_ptr.is_null() {
                        let rumble_fn: SdlGameControllerRumbleFn =
                            unsafe { std::mem::transmute(rumble_ptr) };
                        unsafe {
                            let _ = rumble_fn(gc, low, high, 50);
                        }
                    }
                } else if !js.is_null() {
                    let rumble_ptr = unsafe { dlsym_default(b"SDL_JoystickRumble\0") };
                    if !rumble_ptr.is_null() {
                        let rumble_fn: SdlJoystickRumbleFn =
                            unsafe { std::mem::transmute(rumble_ptr) };
                        unsafe {
                            let _ = rumble_fn(js, low, high, 50);
                        }
                    }
                }
            }
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn SDL_GL_SwapWindow(window: *mut c_void) {
        let now_ns = LAST_POLL_NS.load(Ordering::Relaxed).max(1);
        unsafe { poll_gamepad_and_inject(now_ns) };
        let next = unsafe { dlsym_next(b"SDL_GL_SwapWindow\0") };
        if !next.is_null() {
            let real_fn: SdlSwapWindowFn = unsafe { std::mem::transmute(next) };
            unsafe { real_fn(window) };
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn clock_gettime(clk_id: c_int, tp: *mut Timespec) -> c_int {
        let next = unsafe { dlsym_next(b"clock_gettime\0") };
        if next.is_null() {
            return -1;
        }
        let real_fn: ClockGettimeFn = unsafe { std::mem::transmute(next) };
        let ret = unsafe { real_fn(clk_id, tp) };
        if ret == 0 && !tp.is_null() {
            let now_ns = unsafe {
                ((*tp).tv_sec as u64)
                    .wrapping_mul(1_000_000_000)
                    .wrapping_add((*tp).tv_nsec as u64)
            };
            let prev = LAST_POLL_NS.load(Ordering::Relaxed);
            // Poll at most once every 4 ms (250 Hz).
            if now_ns.wrapping_sub(prev) >= 4_000_000 {
                LAST_POLL_NS.store(now_ns, Ordering::Relaxed);
                unsafe { poll_gamepad_and_inject(now_ns) };
            }
        }
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadzone_scales_smoothly() {
        assert_eq!(apply_deadzone(0, 10), 0);
        assert_eq!(apply_deadzone(3000, 10), 0);
        assert_eq!(apply_deadzone(-3000, 10), 0);
        assert_eq!(apply_deadzone(32767, 10), 32767);
        assert_eq!(apply_deadzone(-32767, 10), -32767);
        assert_eq!(apply_deadzone(16384, 0), 16384);
    }

    #[test]
    fn joystick_trigger_normalizes_and_guards_initial_zero() {
        let mut seen = false;
        // Before first non-zero event from Linux evdev, 0 means released (0)
        assert_eq!(normalize_joystick_trigger(0, &mut seen), 0);
        assert!(!seen);
        // Resting value -32768 marks axis active and normalizes to 0
        assert_eq!(normalize_joystick_trigger(-32768, &mut seen), 0);
        assert!(seen);
        // Half pull (0 in -32768..32767) -> 16384
        assert_eq!(normalize_joystick_trigger(0, &mut seen), 16384);
        // Full pull (+32767) -> 32767
        assert_eq!(normalize_joystick_trigger(32767, &mut seen), 32767);
    }

    #[test]
    fn triggers_combine_independently() {
        // Full right trigger -> -32767 (full throttle)
        assert_eq!(combine_triggers(0, 32767, 0), -32767);
        // Full left trigger -> +32767 (full brake)
        assert_eq!(combine_triggers(32767, 0, 0), 32767);
        // Both triggers pulled equally -> 0 (trail-braking proportional net)
        assert_eq!(combine_triggers(20000, 20000, 0), 0);
        // Right stick up when triggers idle -> negative (throttle)
        assert!(combine_triggers(0, 0, -25000) < -20000);
    }

    #[test]
    fn locate_data_object_finds_anchor_on_page_boundary() {
        let mut guest_ram = vec![0u8; 64 * 4096];
        let target_page = 17 * 4096;
        let off = target_page + 0x104A; // S2_3DFX offset
        guest_ram[off..off + ANCHOR_SIGNATURE.len()].copy_from_slice(ANCHOR_SIGNATURE);
        assert_eq!(locate_data_object(&guest_ram), Some(target_page));
    }

    #[test]
    fn inject_race_controls_writes_analog_values_and_yields_to_keyboard() {
        let spec = &BINARY_CONTROL_SPECS[0]; // S1_STARTH (controls at 0x197D8)
        let mut data_obj = vec![0u8; 0x20000];
        let base = spec.controls_base_offset;

        // Initialize default keyboard records (Up=72, Down=80, Left=75, Right=77, GearUp=56, GearDown=29)
        let scancodes = [72i32, 80, 75, 77, 56, 29];
        for (r, &sc) in scancodes.iter().enumerate() {
            let rec = base + r * 28;
            write_i32_le(&mut data_obj, rec, 1); // enabled = 1
            write_i32_le(&mut data_obj, rec + 0x08, 1); // mode = 1
            write_i32_le(&mut data_obj, rec + 0x14, 1); // polarity = 1
            write_i32_le(&mut data_obj, rec + 0x18, sc);
        }

        let gp = GamepadRaceState {
            throttle: 900,
            brake: 450,
            steer: -600,
            gear_up: true,
            gear_down: false,
        };

        assert!(inject_race_controls(&mut data_obj, spec, &gp));
        assert_eq!(read_i32_le(&data_obj, base), 2); // record[0].enabled = 2 (PollControls skips)
        assert_eq!(read_i32_le(&data_obj, base + 0x04), 1); // record[0].ctrl_type = 1 (MenuPollInput ignores)
        assert_eq!(read_i32_le(&data_obj, base + 0x08), 0); // record[0].mode = 0
        assert_eq!(read_i32_le(&data_obj, base + 0xA8), 900); // norm[0] = throttle
        assert_eq!(read_i32_le(&data_obj, base + 0xA8 + 4), 450); // norm[1] = brake
        assert_eq!(read_i32_le(&data_obj, base + 2 * 28 + 0x08), 0); // record[2].mode = 0 (full analog steer)
        assert_eq!(read_i32_le(&data_obj, base + 0xA8 + 8), -600); // norm[2] = steer
        assert_eq!(read_i32_le(&data_obj, base + 0xA8 + 12), 0); // norm[3] = 0 (never -1, so MenuPollInput is never poisoned)
        assert_eq!(read_i32_le(&data_obj, base + 0xA8 + 16), 1023); // norm[4] = gear_up

        // If Up arrow (72) is set in data_obj[2 + 72] due to synthetic gamepad D-Pad Up,
        // inject_race_controls_with_synth_mask keeps gamepad control active (enabled = 2).
        data_obj[2 + 72] = 1;
        inject_race_controls_with_synth_mask(&mut data_obj, spec, &gp, SYNTH_KEY_BIT_UP);
        assert_eq!(read_i32_le(&data_obj, base), 2);

        // When gamepad throttle is idle (0) and physical keyboard Up arrow is pressed
        // (synth_key_mask = 0) in a clean 0/1 keyboard array, record[0].enabled is
        // restored to 1 so PollControls handles keyboard Up arrow natively.
        let gp_idle = GamepadRaceState::default();
        inject_race_controls_with_synth_mask(&mut data_obj, spec, &gp_idle, 0);
        assert_eq!(read_i32_le(&data_obj, base), 1);

        // When data_obj[2..90] contains ASCII string bytes (> 1, as in the real DOS/4GW
        // Data Object), those bytes are never mistaken for held keyboard keys.
        data_obj[2 + 72] = b'f';
        inject_race_controls_with_synth_mask(&mut data_obj, spec, &gp_idle, 0);
        assert_eq!(read_i32_le(&data_obj, base), 2);
    }

    #[test]
    fn compute_rumble_respects_strength() {
        assert_eq!(compute_rumble(1000, 1000, 0), (0, 0));
        let (low, high) = compute_rumble(1000, 500, 50);
        assert!(low > 32000 && low < 33000);
        assert!(high > 16000 && high < 17000);
    }
}
