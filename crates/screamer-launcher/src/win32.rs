//! The Win32 calls the launcher makes itself:
//!
//! - WinMM's `joyGetPosEx`, for gamepad navigation in the settings window.
//!   The SDK has XInput only (`rea_launcher_win32::xinput`), which misses
//!   DirectInput/HID pads that DOSBox drives in the game.
//! - Finding the open settings window by its class *and* title, for the
//!   second start. The SDK's `instance::Window` matches one or the other:
//!   the title alone also matches an Explorer window on a folder named
//!   "Screamer Trilogy", and the class alone (winit's, the same for every
//!   eframe program) any other eframe window, such as another rea launcher.

#[repr(C)]
struct JoyInfoEx {
    dw_size: u32,
    dw_flags: u32,
    dw_xpos: u32,
    dw_ypos: u32,
    dw_zpos: u32,
    dw_rpos: u32,
    dw_upos: u32,
    dw_vpos: u32,
    dw_buttons: u32,
    dw_button_number: u32,
    dw_pov: u32,
    dw_reserved1: u32,
    dw_reserved2: u32,
}

#[link(name = "winmm")]
unsafe extern "system" {
    fn joyGetPosEx(u_joy_id: u32, pji: *mut JoyInfoEx) -> u32;
}

/// A joystick's buttons, stick and POV hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Joystick {
    pub buttons: u32,
    pub x: u32,
    pub y: u32,
    pub pov: u32,
}

/// Joystick `idx` (0–15), or `None` when none is connected there.
pub fn joystick(idx: u32) -> Option<Joystick> {
    const JOY_RETURNALL: u32 = 0x0000_00FF;
    let mut info = JoyInfoEx {
        dw_size: std::mem::size_of::<JoyInfoEx>() as u32,
        dw_flags: JOY_RETURNALL,
        dw_xpos: 32767,
        dw_ypos: 32767,
        dw_zpos: 32767,
        dw_rpos: 32767,
        dw_upos: 32767,
        dw_vpos: 32767,
        dw_buttons: 0,
        dw_button_number: 0,
        dw_pov: 0xFFFF,
        dw_reserved1: 0,
        dw_reserved2: 0,
    };
    // SAFETY: `info` is a valid, writable JOYINFOEX with `dwSize` set, as
    // joyGetPosEx requires; it does not keep the pointer.
    if unsafe { joyGetPosEx(idx, &mut info) } != 0 {
        return None;
    }
    Some(Joystick {
        buttons: info.dw_buttons,
        x: info.dw_xpos,
        y: info.dw_ypos,
        pov: info.dw_pov,
    })
}

/// The class winit 0.30 (eframe's windowing) registers every window under;
/// eframe offers no way to change it.
pub const LAUNCHER_CLASS: &str = "Window Class";

type Hwnd = *mut core::ffi::c_void;

#[link(name = "user32")]
unsafe extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> Hwnd;
    fn IsIconic(hwnd: Hwnd) -> i32;
    fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    fn SetForegroundWindow(hwnd: Hwnd) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The open settings window titled `title`, if any.
fn launcher_window(title: &str) -> Option<Hwnd> {
    let (class, title) = (wide(LAUNCHER_CLASS), wide(title));
    // SAFETY: both arguments are NUL-terminated UTF-16 strings that
    // outlive the call.
    let hwnd = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
    (!hwnd.is_null()).then_some(hwnd)
}

/// Restores and focuses the open settings window titled `title`; false
/// when there is none (the instance is held by a launcher playing).
pub fn bring_to_front(title: &str) -> bool {
    const SW_RESTORE: i32 = 9;
    let Some(hwnd) = launcher_window(title) else {
        return false;
    };
    // SAFETY: window calls on the handle FindWindowW returned.
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
    }
    true
}

#[cfg(test)]
mod tests {
    use std::ptr::null_mut;

    use super::*;

    #[repr(C)]
    struct WndClassW {
        style: u32,
        wnd_proc: unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: Hwnd,
        icon: Hwnd,
        cursor: Hwnd,
        background: Hwnd,
        menu_name: *const u16,
        class_name: *const u16,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassW(class: *const WndClassW) -> u16;
        fn DefWindowProcW(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> isize;
        #[allow(clippy::too_many_arguments)]
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            title: *const u16,
            style: u32,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            parent: Hwnd,
            menu: Hwnd,
            instance: Hwnd,
            param: *mut core::ffi::c_void,
        ) -> Hwnd;
        fn DestroyWindow(hwnd: Hwnd) -> i32;
    }

    /// A hidden window of `class` titled `title` (registering the class).
    fn window(class: &str, title: &str) -> Hwnd {
        let (class, title) = (wide(class), wide(title));
        let def = WndClassW {
            style: 0,
            wnd_proc: DefWindowProcW,
            cls_extra: 0,
            wnd_extra: 0,
            instance: null_mut(),
            icon: null_mut(),
            cursor: null_mut(),
            background: null_mut(),
            menu_name: std::ptr::null(),
            class_name: class.as_ptr(),
        };
        // SAFETY: a valid WNDCLASSW and NUL-terminated strings; the window
        // is never shown (no WS_VISIBLE) and destroyed by the caller.
        unsafe {
            RegisterClassW(&def);
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                0,
                0,
                0,
                10,
                10,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
            );
            assert!(!hwnd.is_null(), "CreateWindowExW");
            hwnd
        }
    }

    /// Windows (Wine): an Explorer window on a "Screamer Trilogy" folder is
    /// not the launcher; the title-only match took it for one.
    #[test]
    fn a_folder_window_with_the_title_is_not_the_launcher() {
        use rea_launcher_win32::instance::{AlreadyRunning, Window};
        let title = "Screamer Trilogy (win32 test)";
        let folder = window("CabinetWClass", title);
        assert!(AlreadyRunning.is_window_found(Window::Title(title)));
        assert_eq!(launcher_window(title), None);
        let other_eframe = window(LAUNCHER_CLASS, "Another launcher (win32 test)");
        assert_eq!(launcher_window(title), None);
        let launcher = window(LAUNCHER_CLASS, title);
        assert_eq!(launcher_window(title), Some(launcher));
        // SAFETY: windows this test created.
        unsafe {
            DestroyWindow(launcher);
            DestroyWindow(other_eframe);
            DestroyWindow(folder);
        }
    }
}
