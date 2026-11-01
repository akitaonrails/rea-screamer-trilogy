//! Core library for `ScreamerLauncher`: `LE` executable parser, edition
//! table, declarative binary patches (`F1`–`F8`, `E1`–`E5`), config generators,
//! runtime staging, and settings form model.

pub mod config;
pub mod edition;
pub mod fixes;
pub mod form;
pub mod game_cfg;
pub mod le;
pub mod runtime;

use fixes::{FixSet, Write};
use le::Image;

/// A verified write plus the original bytes read from the image before patching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedWrite {
    pub write: Write,
    pub file_offset: usize,
    pub original: Vec<u8>,
}

/// Assembles the active fixes for `image`, verifies that every target site
/// lies inside file-backed `LE` object pages, has zero overlap with `LE`
/// relocation fixups, and matches `write.expected` (or `write.bytes` if
/// already patched).
pub fn prepare(
    image: &Image,
    fixes: &FixSet,
    skip: &[&str],
    cave_base: u32,
) -> Result<Vec<CheckedWrite>, String> {
    fixes
        .assemble(cave_base, skip)
        .into_iter()
        .map(|write| {
            let (file_offset, original) = if write.in_image {
                let foff = image
                    .va_to_file_offset(write.address, write.bytes.len())
                    .ok_or_else(|| {
                        format!(
                            "{}: 0x{:08X} ({} bytes) is not file-backed in this LE executable",
                            write.fix,
                            write.address,
                            write.bytes.len()
                        )
                    })?;
                let slice = image
                    .bytes_at(write.address, write.bytes.len())
                    .ok_or_else(|| {
                        format!(
                            "{}: 0x{:08X} ({} bytes) is not file-backed in this LE executable",
                            write.fix,
                            write.address,
                            write.bytes.len()
                        )
                    })?;
                if image
                    .has_fixup_overlap(write.address, write.bytes.len())
                    .map_err(|e| format!("{}: {e}", write.fix))?
                {
                    return Err(format!(
                        "{}: 0x{:08X} ({} bytes) overlaps an LE relocation fixup record",
                        write.fix,
                        write.address,
                        write.bytes.len()
                    ));
                }
                if !write.expected.is_empty()
                    && slice != write.expected
                    && slice != write.bytes.as_slice()
                {
                    return Err(format!(
                        "{}: unexpected bytes at 0x{:08X} (expected {:02X?}, found {:02X?})",
                        write.fix, write.address, write.expected, slice
                    ));
                }
                (foff, slice.to_vec())
            } else {
                (0, Vec::new())
            };
            Ok(CheckedWrite {
                write,
                file_offset,
                original,
            })
        })
        .collect()
}

/// Applies all `CheckedWrite` patches to `file_bytes` in-place and returns the
/// number of writes applied.
pub fn apply_to_image(file_bytes: &mut [u8], writes: &[CheckedWrite]) -> Result<usize, String> {
    let mut count = 0;
    for cw in writes {
        if !cw.write.in_image {
            continue;
        }
        let foff = cw.file_offset;
        let end = foff + cw.write.bytes.len();
        if end > file_bytes.len() {
            return Err(format!(
                "{}: file offset 0x{:X}..0x{:X} out of bounds",
                cw.write.fix, foff, end
            ));
        }
        file_bytes[foff..end].copy_from_slice(&cw.write.bytes);
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fixes::{At, Fix, Op, Piece};

    static TEST_FIXES: FixSet = FixSet {
        fixes: &[Fix {
            id: "T1",
            what: "test patch",
            optional: false,
            code: &[Piece {
                at: At::Image(0x10002),
                expected: &[0xec],
                ops: &[Op::Raw(&[0xc3])],
            }],
        }],
    };

    #[test]
    fn prepare_and_apply_work_and_are_idempotent() {
        let mut file = le::tests::synthetic(&[0x55, 0x8b, 0xec, 0x90], &[0u8; 4], None);
        let writes = {
            let img = Image::parse(&file).unwrap();
            prepare(&img, &TEST_FIXES, &[], 0).unwrap()
        };
        assert_eq!(writes[0].original, [0xec]);
        let applied = apply_to_image(&mut file, &writes).unwrap();
        assert_eq!(applied, 1);

        // Re-running prepare on already-patched image succeeds idempotently.
        let img2 = Image::parse(&file).unwrap();
        let writes2 = prepare(&img2, &TEST_FIXES, &[], 0).unwrap();
        assert_eq!(writes2[0].original, [0xc3]);
    }

    #[test]
    fn prepare_rejects_unexpected_bytes() {
        let file = le::tests::synthetic(&[0x55, 0x8b, 0x00, 0x90], &[0u8; 4], None);
        let img = Image::parse(&file).unwrap();
        let err = prepare(&img, &TEST_FIXES, &[], 0).unwrap_err();
        assert!(err.contains("unexpected bytes"));
    }

    #[test]
    fn verify_real_game_dirs_from_env_if_present() {
        for var in [
            "SCREAMER_GAME_DIR",
            "SCREAMER1_DIR",
            "SCREAMER2_DIR",
            "SCREAMER_RALLY_DIR",
        ] {
            let Ok(dir) = std::env::var(var) else {
                continue;
            };
            let disc = runtime::scan_game_dir(std::path::Path::new(&dir))
                .unwrap_or_else(|e| panic!("{var} ({dir}): {e}"));
            let verified = runtime::verify_discovered_game(&disc, &[])
                .unwrap_or_else(|e| panic!("{var} ({dir}): {e}"));
            assert!(!verified.is_empty());
        }
    }
}
