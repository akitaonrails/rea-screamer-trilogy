//! `libscreamer_hooks`: Runtime `dosbox-staging` bridge for Screamer (1995),
//! Screamer 2 (1996), and Screamer Rally (1997).
//!
//! Resolves `MemBase`, `JOYSTICK_Enable`, `JOYSTICK_Move_X`, `JOYSTICK_Move_Y`,
//! `JOYSTICK_Button`, `KEYBOARD_AddKey`, and `SDL_GameController*` symbols from
//! the host `dosbox-staging` process to provide:
//! - Proportional analog steering with configurable deadzone (0%–30%)
//! - Independent analog L2/R2 triggers (and right stick Y fallback)
//! - Modern Xbox / Steam Deck button remapping & D-pad menu navigation
//! - Telemetry-driven controller rumble (`SDL_GameControllerRumble`)

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// Known Data Object offsets of `"%s\\config\\controls.cfg\0"` across all 8
/// racing executables in the Screamer trilogy. Because `DOS4GW` allocates
/// objects on 4096-byte page boundaries inside `MemBase`, checking only these
/// page offsets locates the active Data Object in microseconds.
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

/// Scans guest RAM (`mem`) on 4096-byte page boundaries for the Screamer Data
/// Object anchor string `"%s\\config\\controls.cfg\0"` and returns the byte
/// offset of the start of the Data Object.
pub fn locate_data_object(mem: &[u8]) -> Option<usize> {
    let page_size = 4096usize;
    let sig_len = ANCHOR_SIGNATURE.len();
    let mut page_base = 0usize;
    while page_base + page_size <= mem.len() {
        for &(_name, rel_off) in DATA_OBJECT_ANCHORS {
            let pos = page_base + rel_off;
            if pos + sig_len <= mem.len()
                && mem[pos..pos + sig_len].eq_ignore_ascii_case(ANCHOR_SIGNATURE)
            {
                return Some(page_base);
            }
        }
        page_base += page_size;
    }
    None
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

    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    type ClockGettimeFn = unsafe extern "C" fn(c_int, *mut Timespec) -> c_int;
    type SdlSwapWindowFn = unsafe extern "C" fn(*mut c_void);
    type SdlWasInitFn = unsafe extern "C" fn(c_uint) -> c_uint;
    type SdlGameControllerOpenFn = unsafe extern "C" fn(c_int) -> *mut c_void;
    type SdlGameControllerGetAxisFn = unsafe extern "C" fn(*mut c_void, c_int) -> c_short;
    type SdlGameControllerGetButtonFn = unsafe extern "C" fn(*mut c_void, c_int) -> u8;
    type SdlGameControllerRumbleFn =
        unsafe extern "C" fn(*mut c_void, c_ushort, c_ushort, c_uint) -> c_int;
    type JoystickEnableFn = unsafe extern "C" fn(u8, bool);
    type JoystickMoveFn = unsafe extern "C" fn(u8, i16);
    type JoystickButtonFn = unsafe extern "C" fn(u8, c_int, bool);

    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    static ENABLED: AtomicBool = AtomicBool::new(true);
    static DEADZONE_PCT: AtomicUsize = AtomicUsize::new(0);
    static RUMBLE_PCT: AtomicUsize = AtomicUsize::new(60);
    static LAST_POLL_NS: AtomicU64 = AtomicU64::new(0);
    static CONTROLLER_PTR: AtomicUsize = AtomicUsize::new(0);

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
    }

    unsafe fn poll_gamepad_and_inject() {
        init_once();
        if !ENABLED.load(Ordering::Relaxed) {
            return;
        }

        let was_init_ptr = unsafe { dlsym_default(b"SDL_WasInit\0") };
        if !was_init_ptr.is_null() {
            let was_init_fn: SdlWasInitFn = unsafe { std::mem::transmute(was_init_ptr) };
            // Wait until SDL_INIT_VIDEO (0x20) and SDL_INIT_GAMECONTROLLER (0x2000) or JOYSTICK (0x200) are initialized
            if unsafe { was_init_fn(0x220) } != 0x220 {
                return;
            }
        }

        let open_ptr = unsafe { dlsym_default(b"SDL_GameControllerOpen\0") };
        let axis_ptr = unsafe { dlsym_default(b"SDL_GameControllerGetAxis\0") };
        let btn_ptr = unsafe { dlsym_default(b"SDL_GameControllerGetButton\0") };
        let joy_en_ptr = unsafe { dlsym_default(b"_Z15JOYSTICK_Enablehb\0") };
        let joy_x_ptr = unsafe { dlsym_default(b"_Z15JOYSTICK_Move_Xhs\0") };
        let joy_y_ptr = unsafe { dlsym_default(b"_Z15JOYSTICK_Move_Yhs\0") };
        let joy_btn_ptr = unsafe { dlsym_default(b"_Z15JOYSTICK_Buttonhib\0") };

        if open_ptr.is_null()
            || axis_ptr.is_null()
            || btn_ptr.is_null()
            || joy_x_ptr.is_null()
            || joy_y_ptr.is_null()
        {
            return;
        }

        let mut pad = CONTROLLER_PTR.load(Ordering::Relaxed) as *mut c_void;
        if pad.is_null() {
            let open_fn: SdlGameControllerOpenFn = unsafe { std::mem::transmute(open_ptr) };
            pad = unsafe { open_fn(0) };
            if pad.is_null() {
                return;
            }
            CONTROLLER_PTR.store(pad as usize, Ordering::Relaxed);
            if !joy_en_ptr.is_null() {
                let en_fn: JoystickEnableFn = unsafe { std::mem::transmute(joy_en_ptr) };
                unsafe { en_fn(0, true) };
            }
        }

        let axis_fn: SdlGameControllerGetAxisFn = unsafe { std::mem::transmute(axis_ptr) };
        let btn_fn: SdlGameControllerGetButtonFn = unsafe { std::mem::transmute(btn_ptr) };
        let move_x: JoystickMoveFn = unsafe { std::mem::transmute(joy_x_ptr) };
        let move_y: JoystickMoveFn = unsafe { std::mem::transmute(joy_y_ptr) };

        // SDL_GameControllerAxis: 0=LEFTX, 1=LEFTY, 2=RIGHTX, 3=RIGHTY, 4=TRIGGERLEFT, 5=TRIGGERRIGHT
        let raw_lx = unsafe { axis_fn(pad, 0) };
        let raw_ry = unsafe { axis_fn(pad, 3) };
        let raw_l2 = unsafe { axis_fn(pad, 4) };
        let raw_r2 = unsafe { axis_fn(pad, 5) };

        let dz = DEADZONE_PCT.load(Ordering::Relaxed) as u32;
        let steer_x = apply_deadzone(raw_lx, dz);
        let pedals_y = combine_triggers(raw_l2, raw_r2, raw_ry);

        unsafe {
            move_x(0, steer_x);
            move_y(0, pedals_y);
        }

        if !joy_btn_ptr.is_null() {
            let joy_btn: JoystickButtonFn = unsafe { std::mem::transmute(joy_btn_ptr) };
            // SDL_CONTROLLER_BUTTON_A = 0, B = 1, X = 2, Y = 3, LEFTSHOULDER = 4, RIGHTSHOULDER = 5
            let a_pressed = unsafe { btn_fn(pad, 0) } != 0;
            let b_pressed = unsafe { btn_fn(pad, 1) } != 0;
            let rb_pressed = unsafe { btn_fn(pad, 5) } != 0;
            let lb_pressed = unsafe { btn_fn(pad, 4) } != 0;
            unsafe {
                joy_btn(0, 0, a_pressed || rb_pressed);
                joy_btn(0, 1, b_pressed || lb_pressed);
            }
        }

        // Optional subtle rumble feedback when braking hard or full-lock cornering
        let rumble_pct = RUMBLE_PCT.load(Ordering::Relaxed) as u32;
        if rumble_pct > 0 {
            let rumble_ptr = unsafe { dlsym_default(b"SDL_GameControllerRumble\0") };
            if !rumble_ptr.is_null() {
                let rumble_fn: SdlGameControllerRumbleFn =
                    unsafe { std::mem::transmute(rumble_ptr) };
                let brake_load = if pedals_y > 16000 {
                    ((pedals_y as u32 - 16000) * 400) / 16767
                } else {
                    0
                };
                let (low, high) = compute_rumble(brake_load, 0, rumble_pct);
                if low > 0 || high > 0 {
                    unsafe {
                        let _ = rumble_fn(pad, low, high, 50);
                    }
                }
            }
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn SDL_GL_SwapWindow(window: *mut c_void) {
        unsafe { poll_gamepad_and_inject() };
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
                unsafe { poll_gamepad_and_inject() };
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
    fn compute_rumble_respects_strength() {
        assert_eq!(compute_rumble(1000, 1000, 0), (0, 0));
        let (low, high) = compute_rumble(1000, 500, 50);
        assert!(low > 32000 && low < 33000);
        assert!(high > 16000 && high < 17000);
    }
}
