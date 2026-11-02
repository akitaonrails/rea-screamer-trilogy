//! Identifies Screamer, Screamer 2, and Screamer Rally executables by SHA-256.

use rea_launcher::identify::hex32;

use crate::fixes::{
    FixSet, S1_SETUP_FIXES, S1_STARTH_FIXES, S1_STARTL_FIXES, S2_3DFX_FIXES, S2_SETUP_FIXES,
    S2_START65H_FIXES, S2_STARTH_FIXES, SR_ST_FIXES, SR_START65H_FIXES, SR_STARTH_FIXES,
    SR_STRT3FX_FIXES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameTitle {
    Screamer,
    Screamer2,
    ScreamerRally,
}

pub type Game = GameTitle;

impl GameTitle {
    pub const ALL: [GameTitle; 3] = [
        GameTitle::Screamer,
        GameTitle::Screamer2,
        GameTitle::ScreamerRally,
    ];

    pub const fn index(self) -> usize {
        match self {
            GameTitle::Screamer => 0,
            GameTitle::Screamer2 => 1,
            GameTitle::ScreamerRally => 2,
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            GameTitle::Screamer => "Screamer (1995)",
            GameTitle::Screamer2 => "Screamer 2 (1996)",
            GameTitle::ScreamerRally => "Screamer Rally (1997)",
        }
    }

    pub const fn short_title(self) -> &'static str {
        match self {
            GameTitle::Screamer => "Screamer",
            GameTitle::Screamer2 => "Screamer 2",
            GameTitle::ScreamerRally => "Screamer Rally",
        }
    }

    pub const fn year(self) -> &'static str {
        match self {
            GameTitle::Screamer => "1995",
            GameTitle::Screamer2 => "1996",
            GameTitle::ScreamerRally => "1997",
        }
    }

    pub const fn studio_credit(self) -> &'static str {
        match self {
            GameTitle::Screamer => "1995 · Graffiti · Virgin Interactive",
            GameTitle::Screamer2 => "1996 · Milestone · Virgin Interactive",
            GameTitle::ScreamerRally => "1997 · Milestone · Virgin Interactive",
        }
    }

    pub const fn title(self) -> &'static str {
        self.display_name()
    }

    pub const fn slug(self) -> &'static str {
        match self {
            GameTitle::Screamer => "screamer",
            GameTitle::Screamer2 => "screamer2",
            GameTitle::ScreamerRally => "screamer-rally",
        }
    }

    pub fn parse(v: &str) -> Option<GameTitle> {
        match v.trim().to_ascii_lowercase().as_str() {
            "1" | "s1" | "screamer" | "screamer1" | "screamer-1" => Some(GameTitle::Screamer),
            "2" | "s2" | "screamer2" | "screamer-2" => Some(GameTitle::Screamer2),
            "3" | "sr" | "rally" | "screamer-rally" | "screamerrally" | "screamer_rally" => {
                Some(GameTitle::ScreamerRally)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeRole {
    /// 3dfx Voodoo Glide 640x480x16bpp (`S2_3DFX.EXE`, `STRT3FX.EXE`)
    Glide3dfx,
    /// SVGA 640x480x16bpp (`START65H.EXE`)
    Svga16Bit,
    /// SVGA 640x480x8bpp (`STARTH.EXE`)
    Svga8Bit,
    /// VGA 320x200 (`STARTL.EXE`, `START65L.EXE`)
    VgaLow,
    /// Hardware & Control Setup (`SETUP.EXE`, `ST.EXE`)
    Setup,
}

pub type Renderer = ExeRole;

/// SHA-256 of the 31,425-byte Windows-wrapper port-0x600 `glide2x.ovl` stub bundled with GOG Screamer 2.
pub const S2_GLIDE2X_WIN_STUB_SHA256: [u8; 32] =
    hex32("ad743bcad8ccaa62ca55dc9f2986173aa5a86475a6f07fa9a0b43b8c0204aa39");

/// SHA-256 of the genuine 348,791-byte 3dfx DOS `GLIDE2X.OVL` driver.
pub const S2_GLIDE2X_DOS_SHA256: [u8; 32] =
    hex32("05975df4c8ca13efcbfe7134e4cf4bfef2fc5aaad41ca9eb0f31ed83cd2be058");

/// A known executable and the binary fixes it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edition {
    pub game: GameTitle,
    pub role: ExeRole,
    pub exe_name: &'static str,
    pub name: &'static str,
    pub sha256: [u8; 32],
    pub fixes: &'static FixSet,
}

pub static EDITIONS: &[Edition] = &[
    // Screamer (1995)
    Edition {
        game: GameTitle::Screamer,
        role: ExeRole::Svga8Bit,
        exe_name: "STARTH.EXE",
        name: "Screamer (1995) — SVGA 640x480 (STARTH.EXE)",
        sha256: hex32("1246afc95463f96598d5fee764fc99c847b6d3e0bf8ca2268bd362c664bb28a3"),
        fixes: &S1_STARTH_FIXES,
    },
    Edition {
        game: GameTitle::Screamer,
        role: ExeRole::VgaLow,
        exe_name: "STARTL.EXE",
        name: "Screamer (1995) — VGA 320x200 (STARTL.EXE)",
        sha256: hex32("ad20fa3919ecf86da5931e2e0e495d4043f6be8c44e57299114528d87ee27ec7"),
        fixes: &S1_STARTL_FIXES,
    },
    Edition {
        game: GameTitle::Screamer,
        role: ExeRole::Setup,
        exe_name: "SETUP.EXE",
        name: "Screamer (1995) — Hardware Setup (SETUP.EXE)",
        sha256: hex32("33d88269a37bc67b08b2ec9f9f7be32c429619f6d946837c35b7e08f33f2d189"),
        fixes: &S1_SETUP_FIXES,
    },
    // Screamer 2 (1996)
    Edition {
        game: GameTitle::Screamer2,
        role: ExeRole::Glide3dfx,
        exe_name: "S2_3DFX.EXE",
        name: "Screamer 2 (1996) — 3dfx Voodoo Glide (S2_3DFX.EXE)",
        sha256: hex32("e6c24c84650938f2baee5f4b571baa45bb09a93ba12dd24e3e1d2cefd9b6f3ec"),
        fixes: &S2_3DFX_FIXES,
    },
    Edition {
        game: GameTitle::Screamer2,
        role: ExeRole::Svga16Bit,
        exe_name: "START65H.EXE",
        name: "Screamer 2 (1996) — SVGA 640x480 16-bit (START65H.EXE)",
        sha256: hex32("05f576866f26602be303343c5e9104132616f464b71ed6ae958e99a668e5c7c8"),
        fixes: &S2_START65H_FIXES,
    },
    Edition {
        game: GameTitle::Screamer2,
        role: ExeRole::Svga8Bit,
        exe_name: "STARTH.EXE",
        name: "Screamer 2 (1996) — SVGA 640x480 8-bit (STARTH.EXE)",
        sha256: hex32("91a49f26c7b5a26bc84552126ed337dfa733b81c61ec4ad4195d205809a2f400"),
        fixes: &S2_STARTH_FIXES,
    },
    Edition {
        game: GameTitle::Screamer2,
        role: ExeRole::Setup,
        exe_name: "SETUP.EXE",
        name: "Screamer 2 (1996) — Hardware Setup (SETUP.EXE)",
        sha256: hex32("ef8d7665f1a8979ee9f699ff0bd476b8e9679fbe423a6b53a773539ce51d178b"),
        fixes: &S2_SETUP_FIXES,
    },
    // Screamer Rally (1997)
    Edition {
        game: GameTitle::ScreamerRally,
        role: ExeRole::Glide3dfx,
        exe_name: "STRT3FX.EXE",
        name: "Screamer Rally (1997) — 3dfx Voodoo Glide (STRT3FX.EXE)",
        sha256: hex32("9eac6e12ab309389bdda019cfa695bfbbc791fbda1a4634464a444fce35165d1"),
        fixes: &SR_STRT3FX_FIXES,
    },
    Edition {
        game: GameTitle::ScreamerRally,
        role: ExeRole::Svga16Bit,
        exe_name: "START65H.EXE",
        name: "Screamer Rally (1997) — SVGA 640x480 16-bit (START65H.EXE)",
        sha256: hex32("0785e38567c475f25663e4022ec125e447a799004e34efa59758b50cfd84901d"),
        fixes: &SR_START65H_FIXES,
    },
    Edition {
        game: GameTitle::ScreamerRally,
        role: ExeRole::Svga8Bit,
        exe_name: "STARTH.EXE",
        name: "Screamer Rally (1997) — SVGA 640x480 8-bit (STARTH.EXE)",
        sha256: hex32("18d32c27d61873dc67381f87880d19774afaec1f7ee391d14ab255a9040a12d7"),
        fixes: &SR_STARTH_FIXES,
    },
    Edition {
        game: GameTitle::ScreamerRally,
        role: ExeRole::Setup,
        exe_name: "ST.EXE",
        name: "Screamer Rally (1997) — Hardware Setup (ST.EXE)",
        sha256: hex32("e6083738a0130e77d8403f2948d70f4c554481405be8197ab70123ca0d57393d"),
        fixes: &SR_ST_FIXES,
    },
];

pub use rea_launcher::identify::sha256;

pub fn identify_sha256(digest: &[u8; 32]) -> Option<&'static Edition> {
    EDITIONS.iter().find(|e| &e.sha256 == digest)
}

pub fn identify(exe: &[u8]) -> Result<&'static Edition, [u8; 32]> {
    let digest = sha256(exe);
    identify_sha256(&digest).ok_or(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editions_have_distinct_hashes() {
        for (i, a) in EDITIONS.iter().enumerate() {
            for b in &EDITIONS[i + 1..] {
                assert_ne!(a.sha256, b.sha256, "{} / {}", a.name, b.name);
            }
        }
    }

    #[test]
    fn unknown_exe_reports_its_hash() {
        let err = identify(b"not a game").unwrap_err();
        assert_eq!(err, sha256(b"not a game"));
    }
}
