//! The one Win32 call the launcher makes itself: WinMM's `joyGetPosEx`,
//! for gamepad navigation in the settings window. The SDK has XInput only
//! (`rea_launcher_win32::xinput`), which misses DirectInput/HID pads that
//! DOSBox drives in the game.

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
