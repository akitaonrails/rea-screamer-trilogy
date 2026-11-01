//! Declarative binary fixes for Screamer, Screamer 2, and Screamer Rally.
//!
//! Every fix replaces a few instructions or constants at a known Virtual
//! Address (`VA`) of one exact executable (see `edition`). Before writing to
//! the private runtime executable, `prepare()` verifies both that the file
//! bytes at `at` match `expected` and that no `DOS4GW` LE relocation fixup
//! overlaps the patch range.

/// One fix: a list of code/data pieces written at Virtual Addresses.
#[derive(Debug, PartialEq, Eq)]
pub struct Fix {
    pub id: &'static str,
    pub what: &'static str,
    /// True if a launcher setting can turn it off (`E1`, `E2`, `E5`).
    pub optional: bool,
    pub code: &'static [Piece],
}

#[derive(Debug, PartialEq, Eq)]
pub struct FixSet {
    pub fixes: &'static [Fix],
}

/// Where a piece of code or data goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum At {
    /// A Virtual Address inside the game image.
    Image(u32),
    /// An offset inside a dedicated cave area.
    Cave(u32),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Piece {
    pub at: At,
    /// Expected original bytes at `at` in the unpatched executable.
    pub expected: &'static [u8],
    pub ops: &'static [Op],
}

/// Instructions and literals our patches assemble.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Raw(&'static [u8]),
    /// `jmp rel32` to an address.
    Jmp(At),
    /// `jcc rel32`; condition code `0x4 = je`, `0x5 = jne`.
    Jcc(u8, At),
    /// A 32-bit absolute address.
    Addr(At),
    /// `call rel32` to an address.
    Call(At),
}

pub const JE: u8 = 0x4;
pub const JNE: u8 = 0x5;

impl Op {
    fn len(self) -> usize {
        match self {
            Op::Raw(b) => b.len(),
            Op::Jmp(_) => 5,
            Op::Jcc(..) => 6,
            Op::Addr(_) => 4,
            Op::Call(_) => 5,
        }
    }
}

impl Piece {
    fn len(&self) -> usize {
        self.ops.iter().map(|op| op.len()).sum()
    }
}

/// A concrete write: bytes at a Virtual Address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Write {
    pub fix: &'static str,
    pub address: u32,
    pub expected: Vec<u8>,
    pub bytes: Vec<u8>,
    pub in_image: bool,
}

fn rel32(from: u32, instr_len: u32, to: u32) -> [u8; 4] {
    let next = from.wrapping_add(instr_len);
    let delta = to.wrapping_sub(next) as i32;
    delta.to_le_bytes()
}

impl FixSet {
    /// Active fixes: all required fixes plus optional fixes not listed in `skip`.
    pub fn active<'a>(&'a self, skip: &'a [&str]) -> impl Iterator<Item = &'a Fix> {
        self.fixes
            .iter()
            .filter(move |f| !(f.optional && skip.contains(&f.id)))
    }

    /// Bytes the cave needs; 0 means no external cave is needed.
    pub fn cave_size(&self, skip: &[&str]) -> u32 {
        self.active(skip)
            .flat_map(|f| f.code.iter())
            .filter_map(|p| match p.at {
                At::Cave(off) => Some(off + p.len() as u32),
                At::Image(_) => None,
            })
            .max()
            .unwrap_or(0)
    }

    /// Resolves every active piece to absolute bytes.
    pub fn assemble(&self, cave_base: u32, skip: &[&str]) -> Vec<Write> {
        let resolve = |at: At| match at {
            At::Image(a) => a,
            At::Cave(off) => cave_base.wrapping_add(off),
        };
        let mut writes = Vec::new();
        for fix in self.active(skip) {
            for piece in fix.code {
                let start = resolve(piece.at);
                let mut bytes = Vec::with_capacity(piece.len());
                for op in piece.ops {
                    let here = start.wrapping_add(bytes.len() as u32);
                    match *op {
                        Op::Raw(b) => bytes.extend_from_slice(b),
                        Op::Jmp(to) => {
                            bytes.push(0xe9);
                            bytes.extend_from_slice(&rel32(here, 5, resolve(to)));
                        }
                        Op::Jcc(cc, to) => {
                            bytes.extend_from_slice(&[0x0f, 0x80 | cc]);
                            bytes.extend_from_slice(&rel32(here, 6, resolve(to)));
                        }
                        Op::Addr(to) => bytes.extend_from_slice(&resolve(to).to_le_bytes()),
                        Op::Call(to) => {
                            bytes.push(0xe8);
                            bytes.extend_from_slice(&rel32(here, 5, resolve(to)));
                        }
                    }
                }
                writes.push(Write {
                    fix: fix.id,
                    address: start,
                    expected: piece.expected.to_vec(),
                    bytes,
                    in_image: matches!(piece.at, At::Image(_)),
                });
            }
        }
        writes
    }
}

/// 31-byte Hor+ 16:9 widescreen stub that scales the X-row (`M00`, `M01`, `M02`)
/// of `camera->view_matrix` (`[esi+0x00]`, `[esi+0x0c]`, `[esi+0x18]`) by `3/4`
/// (`0.75`) and executes the displaced `sub eax, eax; mov 0x24(%esi), %eax; ret`.
const WIDESCREEN_16_9_STUB: &[u8; 31] = &[
    0x8b, 0x06, // mov (%esi), %eax
    0xc1, 0xf8, 0x02, // sar $2, %eax
    0x29, 0x06, // sub %eax, (%esi)
    0x8b, 0x46, 0x0c, // mov 0xc(%esi), %eax
    0xc1, 0xf8, 0x02, // sar $2, %eax
    0x29, 0x46, 0x0c, // sub %eax, 0xc(%esi)
    0x8b, 0x46, 0x18, // mov 0x18(%esi), %eax
    0xc1, 0xf8, 0x02, // sar $2, %eax
    0x29, 0x46, 0x18, // sub %eax, 0x18(%esi)
    0x2b, 0xc0, // sub %eax, %eax
    0x89, 0x46, 0x24, // mov %eax, 0x24(%esi)
    0xc3, // ret
];

/// IEEE-754 double `0.02` (`20 ms` = 50 FPS floor).
const DBL_0_02: &[u8; 8] = &[0x7b, 0x14, 0xae, 0x47, 0xe1, 0x7a, 0x94, 0x3f];
/// IEEE-754 double `0.004` (`4 ms` = 250 FPS floor).
const DBL_0_004: &[u8; 8] = &[0x62, 0x10, 0x58, 0x39, 0xb4, 0xc8, 0x70, 0x3f];
/// IEEE-754 float `0.02f` (`20 ms` = 50 FPS floor).
const FLT_0_02: &[u8; 4] = &[0x0a, 0xd7, 0xa3, 0x3c];
/// IEEE-754 float `0.004f` (`4 ms` = 250 FPS floor).
const FLT_0_004: &[u8; 4] = &[0x0b, 0xd7, 0x83, 0x3b];

// ============================================================================
// Screamer (1995) — STARTH.EXE (SVGA 640x480)
// ============================================================================
pub static S1_STARTH_FIXES: FixSet = FixSet {
    fixes: &[
        Fix {
            id: "F1",
            what: "Bypass VerifyCD optical disc file check (xor eax, eax; ret 8)",
            optional: false,
            code: &[Piece {
                at: At::Image(0x0001_5460),
                expected: &[0x56, 0x57, 0x55, 0x89, 0xe5],
                ops: &[Op::Raw(&[0x31, 0xc0, 0xc2, 0x08, 0x00])],
            }],
        },
        Fix {
            id: "E1",
            what: "Unlock 60+ FPS spin-wait and lower smoothed_dt 0.02s floor to 0.004s",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0001_fc48),
                    expected: &[0x02],
                    ops: &[Op::Raw(&[0x01])],
                },
                Piece {
                    at: At::Image(0x0010_0c70),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0001_fcd0),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
            ],
        },
        Fix {
            id: "E2",
            what: "Hor+ 16:9 widescreen 3D camera projection",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0001_54a6),
                    expected: &[
                        0x8d, 0x5d, 0xf2, 0xe8, 0x7e, 0x7e, 0x03, 0x00, 0x83, 0xc4, 0x0c, 0x8d,
                        0x55, 0x8e, 0x8d, 0x85, 0xda, 0xfe, 0xff, 0xff, 0xe8, 0xce, 0x6b, 0x03,
                        0x00, 0x85, 0xc0, 0x74, 0x15, 0x8d, 0x85,
                    ],
                    ops: &[Op::Raw(WIDESCREEN_16_9_STUB)],
                },
                Piece {
                    at: At::Image(0x0008_acc2),
                    expected: &[0x2b, 0xc0, 0x89, 0x46, 0x24],
                    ops: &[Op::Call(At::Image(0x0001_54a6))],
                },
            ],
        },
        Fix {
            id: "E5",
            what: "Remove hardcoded 10% (0x66) center joystick deadzone in PollControls",
            optional: true,
            code: &[Piece {
                at: At::Image(0x0002_4f3e),
                expected: &[0x66],
                ops: &[Op::Raw(&[0x00])],
            }],
        },
    ],
};

// ============================================================================
// Screamer (1995) — STARTL.EXE (VGA 320x200)
// ============================================================================
pub static S1_STARTL_FIXES: FixSet = FixSet {
    fixes: &[
        Fix {
            id: "F1",
            what: "Bypass VerifyCD optical disc file check (xor eax, eax; ret 8)",
            optional: false,
            code: &[Piece {
                at: At::Image(0x0001_545c),
                expected: &[0x56, 0x57, 0x55, 0x89, 0xe5],
                ops: &[Op::Raw(&[0x31, 0xc0, 0xc2, 0x08, 0x00])],
            }],
        },
        Fix {
            id: "E1",
            what: "Unlock 60+ FPS spin-wait and lower smoothed_dt 0.02s floor to 0.004s",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0001_fc38),
                    expected: &[0x02],
                    ops: &[Op::Raw(&[0x01])],
                },
                Piece {
                    at: At::Image(0x0010_0c70),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0001_fcc0),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
            ],
        },
        Fix {
            id: "E2",
            what: "Hor+ 16:9 widescreen 3D camera projection",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0001_54a2),
                    expected: &[
                        0x8d, 0x5d, 0xf2, 0xe8, 0x3a, 0x7c, 0x03, 0x00, 0x83, 0xc4, 0x0c, 0x8d,
                        0x55, 0x8e, 0x8d, 0x85, 0xda, 0xfe, 0xff, 0xff, 0xe8, 0x92, 0x69, 0x03,
                        0x00, 0x85, 0xc0, 0x74, 0x15, 0x8d, 0x85,
                    ],
                    ops: &[Op::Raw(WIDESCREEN_16_9_STUB)],
                },
                Piece {
                    at: At::Image(0x0008_aa12),
                    expected: &[0x2b, 0xc0, 0x89, 0x46, 0x24],
                    ops: &[Op::Call(At::Image(0x0001_54a2))],
                },
            ],
        },
        Fix {
            id: "E5",
            what: "Remove hardcoded 10% (0x66) center joystick deadzone in PollControls",
            optional: true,
            code: &[Piece {
                at: At::Image(0x0002_4ede),
                expected: &[0x66],
                ops: &[Op::Raw(&[0x00])],
            }],
        },
    ],
};

// ============================================================================
// Screamer (1995) — SETUP.EXE
// ============================================================================
pub static S1_SETUP_FIXES: FixSet = FixSet {
    fixes: &[Fix {
        id: "F6",
        what: "Prevent divide-by-zero in VRAM/CPU speed benchmark at high cycles",
        optional: false,
        code: &[
            Piece {
                at: At::Image(0x0003_5e4b),
                expected: &[0xf7, 0xf9],
                ops: &[Op::Raw(&[0x90, 0x90])],
            },
            Piece {
                at: At::Image(0x0003_5ef2),
                expected: &[0xf7, 0xf9],
                ops: &[Op::Raw(&[0x90, 0x90])],
            },
        ],
    }],
};

// ============================================================================
// Screamer 2 (1996) — S2_3DFX.EXE (3dfx Voodoo Glide 640x480)
// ============================================================================
pub static S2_3DFX_FIXES: FixSet = FixSet {
    fixes: &[
        Fix {
            id: "F1",
            what: "Bypass VerifyCD optical disc file check (xor eax, eax; ret 8)",
            optional: false,
            code: &[Piece {
                at: At::Image(0x0003_9b40),
                expected: &[0x56, 0x57, 0x55, 0x81, 0xec],
                ops: &[Op::Raw(&[0x31, 0xc0, 0xc2, 0x08, 0x00])],
            }],
        },
        Fix {
            id: "E1",
            what: "Unlock 60+ FPS spin-wait and lower smoothed_dt 0.02s floor to 0.004s",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0005_19bc),
                    expected: &[0x02],
                    ops: &[Op::Raw(&[0x01])],
                },
                Piece {
                    at: At::Image(0x0013_1c50),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0013_1c8c),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0005_0c48),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
                Piece {
                    at: At::Image(0x0005_0d30),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
            ],
        },
        Fix {
            id: "E2",
            what: "Hor+ 16:9 widescreen 3D camera projection",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0003_9b82),
                    expected: &[
                        0x8d, 0x9c, 0x24, 0x24, 0x05, 0x00, 0x00, 0xe8, 0x44, 0xa8, 0x01, 0x00,
                        0x83, 0xc4, 0x0c, 0x8d, 0x94, 0x24, 0xb4, 0x04, 0x00, 0x00, 0x8d, 0x84,
                        0x24, 0x00, 0x04, 0x00, 0x00, 0xe8, 0x71,
                    ],
                    ops: &[Op::Raw(WIDESCREEN_16_9_STUB)],
                },
                Piece {
                    at: At::Image(0x0008_3db2),
                    expected: &[0x2b, 0xc0, 0x89, 0x46, 0x24],
                    ops: &[Op::Call(At::Image(0x0003_9b82))],
                },
            ],
        },
        Fix {
            id: "E5",
            what: "Remove hardcoded 10% (0x66) center joystick deadzone in PollControls",
            optional: true,
            code: &[Piece {
                at: At::Image(0x0003_a204),
                expected: &[0x66],
                ops: &[Op::Raw(&[0x00])],
            }],
        },
    ],
};

// ============================================================================
// Screamer 2 (1996) — START65H.EXE (SVGA 640x480x16bpp)
// ============================================================================
pub static S2_START65H_FIXES: FixSet = FixSet {
    fixes: &[
        Fix {
            id: "F1",
            what: "Bypass VerifyCD optical disc file check (xor eax, eax; ret 8)",
            optional: false,
            code: &[Piece {
                at: At::Image(0x0003_237c),
                expected: &[0x56, 0x57, 0x55, 0x89, 0xe5],
                ops: &[Op::Raw(&[0x31, 0xc0, 0xc2, 0x08, 0x00])],
            }],
        },
        Fix {
            id: "F5",
            what: "Force standard VESA mode 0x111 (640x480x16bpp) on VBE 2.0+ cards",
            optional: false,
            code: &[Piece {
                at: At::Image(0x000d_782f),
                expected: &[0x7c],
                ops: &[Op::Raw(&[0xeb])],
            }],
        },
        Fix {
            id: "E1",
            what: "Unlock 60+ FPS spin-wait and lower smoothed_dt 0.02s floor to 0.004s",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0004_d21c),
                    expected: &[0x02],
                    ops: &[Op::Raw(&[0x01])],
                },
                Piece {
                    at: At::Image(0x0013_1f5c),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0013_1f98),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0004_c4a8),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
                Piece {
                    at: At::Image(0x0004_c590),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
            ],
        },
        Fix {
            id: "E2",
            what: "Hor+ 16:9 widescreen 3D camera projection",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0003_23ce),
                    expected: &[
                        0x8d, 0x9d, 0x10, 0xff, 0xff, 0xff, 0x8d, 0x95, 0x74, 0xff, 0xff, 0xff,
                        0x8d, 0x85, 0x5c, 0xfe, 0xff, 0xff, 0xe8, 0xd0, 0xd2, 0x01, 0x00, 0x85,
                        0xc0, 0x74, 0x15, 0x8d, 0x85, 0x5c, 0xfe,
                    ],
                    ops: &[Op::Raw(WIDESCREEN_16_9_STUB)],
                },
                Piece {
                    at: At::Image(0x0007_e0f2),
                    expected: &[0x2b, 0xc0, 0x89, 0x46, 0x24],
                    ops: &[Op::Call(At::Image(0x0003_23ce))],
                },
            ],
        },
        Fix {
            id: "E5",
            what: "Remove hardcoded 10% (0x66) center joystick deadzone in PollControls",
            optional: true,
            code: &[Piece {
                at: At::Image(0x0003_2bb3),
                expected: &[0x66],
                ops: &[Op::Raw(&[0x00])],
            }],
        },
    ],
};

// ============================================================================
// Screamer 2 (1996) — STARTH.EXE (SVGA 640x480x8bpp)
// ============================================================================
pub static S2_STARTH_FIXES: FixSet = FixSet {
    fixes: &[
        Fix {
            id: "F1",
            what: "Bypass VerifyCD optical disc file check (xor eax, eax; ret 8)",
            optional: false,
            code: &[Piece {
                at: At::Image(0x0003_11a4),
                expected: &[0x56, 0x57, 0x55, 0x89, 0xe5],
                ops: &[Op::Raw(&[0x31, 0xc0, 0xc2, 0x08, 0x00])],
            }],
        },
        Fix {
            id: "E1",
            what: "Unlock 60+ FPS spin-wait and lower smoothed_dt 0.02s floor to 0.004s",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0004_d08c),
                    expected: &[0x02],
                    ops: &[Op::Raw(&[0x01])],
                },
                Piece {
                    at: At::Image(0x0011_1b8c),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0011_1bc8),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0004_b13c),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
                Piece {
                    at: At::Image(0x0004_b224),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
            ],
        },
        Fix {
            id: "E2",
            what: "Hor+ 16:9 widescreen 3D camera projection",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0003_11f6),
                    expected: &[
                        0x8d, 0x9d, 0x10, 0xff, 0xff, 0xff, 0x8d, 0x95, 0x74, 0xff, 0xff, 0xff,
                        0x8d, 0x85, 0x5c, 0xfe, 0xff, 0xff, 0xe8, 0x58, 0xdc, 0x01, 0x00, 0x85,
                        0xc0, 0x74, 0x15, 0x8d, 0x85, 0x5c, 0xfe,
                    ],
                    ops: &[Op::Raw(WIDESCREEN_16_9_STUB)],
                },
                Piece {
                    at: At::Image(0x0007_ecb2),
                    expected: &[0x2b, 0xc0, 0x89, 0x46, 0x24],
                    ops: &[Op::Call(At::Image(0x0003_11f6))],
                },
            ],
        },
        Fix {
            id: "E5",
            what: "Remove hardcoded 10% (0x66) center joystick deadzone in PollControls",
            optional: true,
            code: &[Piece {
                at: At::Image(0x0003_19db),
                expected: &[0x66],
                ops: &[Op::Raw(&[0x00])],
            }],
        },
    ],
};

// ============================================================================
// Screamer 2 (1996) — SETUP.EXE
// ============================================================================
pub static S2_SETUP_FIXES: FixSet = FixSet {
    fixes: &[Fix {
        id: "F6",
        what: "Prevent divide-by-zero in VRAM/CPU speed benchmark at high cycles",
        optional: false,
        code: &[
            Piece {
                at: At::Image(0x0003_9b53),
                expected: &[0xf7, 0xf9],
                ops: &[Op::Raw(&[0x90, 0x90])],
            },
            Piece {
                at: At::Image(0x0003_9c06),
                expected: &[0xf7, 0xf9],
                ops: &[Op::Raw(&[0x90, 0x90])],
            },
        ],
    }],
};

// ============================================================================
// Screamer Rally (1997) — STRT3FX.EXE (3dfx Voodoo Glide 640x480)
// ============================================================================
pub static SR_STRT3FX_FIXES: FixSet = FixSet {
    fixes: &[
        Fix {
            id: "F1",
            what: "Bypass VerifyCD optical disc file check (xor eax, eax; ret 8)",
            optional: false,
            code: &[Piece {
                at: At::Image(0x0003_9e80),
                expected: &[0x56, 0x57, 0x55, 0x81, 0xec],
                ops: &[Op::Raw(&[0x31, 0xc0, 0xc2, 0x08, 0x00])],
            }],
        },
        Fix {
            id: "E1",
            what: "Lower smoothed_dt 0.02s floor to 0.004s for true 60+ FPS physics",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0012_1c58),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0012_1c94),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0004_f76e),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
                Piece {
                    at: At::Image(0x0004_f856),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
            ],
        },
        Fix {
            id: "E2",
            what: "Hor+ 16:9 widescreen 3D camera projection",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0003_9ec2),
                    expected: &[
                        0x8d, 0x9c, 0x24, 0x24, 0x05, 0x00, 0x00, 0xe8, 0x44, 0x8f, 0x01, 0x00,
                        0x83, 0xc4, 0x0c, 0x8d, 0x94, 0x24, 0xb4, 0x04, 0x00, 0x00, 0x8d, 0x84,
                        0x24, 0x00, 0x04, 0x00, 0x00, 0xe8, 0xe5,
                    ],
                    ops: &[Op::Raw(WIDESCREEN_16_9_STUB)],
                },
                Piece {
                    at: At::Image(0x0008_2382),
                    expected: &[0x2b, 0xc0, 0x89, 0x46, 0x24],
                    ops: &[Op::Call(At::Image(0x0003_9ec2))],
                },
            ],
        },
        Fix {
            id: "E5",
            what: "Remove hardcoded 10% (0x66) center joystick deadzone in PollControls",
            optional: true,
            code: &[Piece {
                at: At::Image(0x0003_a444),
                expected: &[0x66],
                ops: &[Op::Raw(&[0x00])],
            }],
        },
    ],
};

// ============================================================================
// Screamer Rally (1997) — START65H.EXE (SVGA 640x480x16bpp)
// ============================================================================
pub static SR_START65H_FIXES: FixSet = FixSet {
    fixes: &[
        Fix {
            id: "F1",
            what: "Bypass VerifyCD optical disc file check (xor eax, eax; ret 8)",
            optional: false,
            code: &[Piece {
                at: At::Image(0x0002_9cec),
                expected: &[0x56, 0x57, 0x55, 0x81, 0xec],
                ops: &[Op::Raw(&[0x31, 0xc0, 0xc2, 0x08, 0x00])],
            }],
        },
        Fix {
            id: "E1",
            what: "Lower smoothed_dt 0.02s floor to 0.004s for true 60+ FPS physics",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0012_1c58),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0012_1c94),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0003_f55e),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
                Piece {
                    at: At::Image(0x0003_f646),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
            ],
        },
        Fix {
            id: "E2",
            what: "Hor+ 16:9 widescreen 3D camera projection",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0002_9d2e),
                    expected: &[
                        0x8d, 0x9c, 0x24, 0x24, 0x05, 0x00, 0x00, 0xe8, 0xc8, 0x8e, 0x01, 0x00,
                        0x83, 0xc4, 0x0c, 0x8d, 0x94, 0x24, 0xb4, 0x04, 0x00, 0x00, 0x8d, 0x84,
                        0x24, 0x00, 0x04, 0x00, 0x00, 0xe8, 0x69,
                    ],
                    ops: &[Op::Raw(WIDESCREEN_16_9_STUB)],
                },
                Piece {
                    at: At::Image(0x0007_1bf2),
                    expected: &[0x2b, 0xc0, 0x89, 0x46, 0x24],
                    ops: &[Op::Call(At::Image(0x0002_9d2e))],
                },
            ],
        },
        Fix {
            id: "E5",
            what: "Remove hardcoded 10% (0x66) center joystick deadzone in PollControls",
            optional: true,
            code: &[Piece {
                at: At::Image(0x0002_a2b4),
                expected: &[0x66],
                ops: &[Op::Raw(&[0x00])],
            }],
        },
    ],
};

// ============================================================================
// Screamer Rally (1997) — STARTH.EXE (SVGA 640x480x8bpp)
// ============================================================================
pub static SR_STARTH_FIXES: FixSet = FixSet {
    fixes: &[
        Fix {
            id: "F1",
            what: "Bypass VerifyCD optical disc file check (xor eax, eax; ret 8)",
            optional: false,
            code: &[Piece {
                at: At::Image(0x0002_8ec0),
                expected: &[0x56, 0x57, 0x55, 0x81, 0xec],
                ops: &[Op::Raw(&[0x31, 0xc0, 0xc2, 0x08, 0x00])],
            }],
        },
        Fix {
            id: "E1",
            what: "Lower smoothed_dt 0.02s floor to 0.004s for true 60+ FPS physics",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0011_1888),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0011_18c4),
                    expected: DBL_0_02,
                    ops: &[Op::Raw(DBL_0_004)],
                },
                Piece {
                    at: At::Image(0x0003_e8ce),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
                Piece {
                    at: At::Image(0x0003_e9b6),
                    expected: FLT_0_02,
                    ops: &[Op::Raw(FLT_0_004)],
                },
            ],
        },
        Fix {
            id: "E2",
            what: "Hor+ 16:9 widescreen 3D camera projection",
            optional: true,
            code: &[
                Piece {
                    at: At::Image(0x0002_8f02),
                    expected: &[
                        0x8d, 0x9c, 0x24, 0x24, 0x05, 0x00, 0x00, 0xe8, 0x2f, 0x93, 0x01, 0x00,
                        0x83, 0xc4, 0x0c, 0x8d, 0x94, 0x24, 0xb4, 0x04, 0x00, 0x00, 0x8d, 0x84,
                        0x24, 0x00, 0x04, 0x00, 0x00, 0xe8, 0xd1,
                    ],
                    ops: &[Op::Raw(WIDESCREEN_16_9_STUB)],
                },
                Piece {
                    at: At::Image(0x0007_70a2),
                    expected: &[0x2b, 0xc0, 0x89, 0x46, 0x24],
                    ops: &[Op::Call(At::Image(0x0002_8f02))],
                },
            ],
        },
        Fix {
            id: "E5",
            what: "Remove hardcoded 10% (0x66) center joystick deadzone in PollControls",
            optional: true,
            code: &[Piece {
                at: At::Image(0x0002_9484),
                expected: &[0x66],
                ops: &[Op::Raw(&[0x00])],
            }],
        },
    ],
};

// ============================================================================
// Screamer Rally (1997) — ST.EXE
// ============================================================================
pub static SR_ST_FIXES: FixSet = FixSet {
    fixes: &[Fix {
        id: "F6",
        what: "Prevent divide-by-zero in VRAM/CPU speed benchmark at high cycles",
        optional: false,
        code: &[
            Piece {
                at: At::Image(0x0003_d0a3),
                expected: &[0xf7, 0xf9],
                ops: &[Op::Raw(&[0x90, 0x90])],
            },
            Piece {
                at: At::Image(0x0003_d156),
                expected: &[0xf7, 0xf9],
                ops: &[Op::Raw(&[0x90, 0x90])],
            },
        ],
    }],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_pieces_match_expected_len() {
        for set in [
            &S1_STARTH_FIXES,
            &S1_STARTL_FIXES,
            &S1_SETUP_FIXES,
            &S2_3DFX_FIXES,
            &S2_START65H_FIXES,
            &S2_STARTH_FIXES,
            &S2_SETUP_FIXES,
            &SR_STRT3FX_FIXES,
            &SR_START65H_FIXES,
            &SR_STARTH_FIXES,
            &SR_ST_FIXES,
        ] {
            for fix in set.fixes {
                for piece in fix.code {
                    assert_eq!(
                        piece.expected.len(),
                        piece.len(),
                        "fix {} piece at {:?} expected len {} != ops len {}",
                        fix.id,
                        piece.at,
                        piece.expected.len(),
                        piece.len()
                    );
                }
            }
        }
    }

    #[test]
    fn skip_filters_only_optional_fixes() {
        let writes = S1_STARTH_FIXES.assemble(0, &["F1", "E1", "E2", "E5"]);
        assert_eq!(writes.len(), 1, "required F1 cannot be skipped");
        assert_eq!(writes[0].fix, "F1");
    }
}
