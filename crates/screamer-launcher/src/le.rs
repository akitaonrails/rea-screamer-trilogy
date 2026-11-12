//! Watcom Linear Executable (`LE`) parser and relocation fixup verifier.
//!
//! All Screamer, Screamer 2, and Screamer Rally executables (`STARTH.EXE`,
//! `STARTL.EXE`, `START65H.EXE`, `S2_3DFX.EXE`, `STRT3FX.EXE`, `SETUP.EXE`,
//! `ST.EXE`) are 32-bit DOS/4GW Linear Executables (`LE`) whose outer `MZ`
//! header (`e_lfanew` at `0x3c`) points to the `LE\0\0` header.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageEntry {
    /// 1-based physical page index in the data pages area (`0` if unbacked).
    pub physical_page: u32,
    /// `0x00` = legal file-backed page, `0x03` = zero-filled / BSS page.
    pub flags: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    pub index: u32,
    pub virtual_size: u32,
    pub reloc_base: u32,
    pub flags: u32,
    pub first_page_index: u32,
    pub page_count: u32,
    pub executable: bool,
    pub pages: Vec<PageEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image<'a> {
    pub file: &'a [u8],
    pub le_offset: usize,
    pub page_size: u32,
    pub last_page_size: u32,
    pub module_pages: u32,
    pub data_pages_offset: usize,
    pub fixup_page_table_offset: usize,
    pub fixup_record_table_offset: usize,
    pub objects: Vec<Object>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeError {
    NotLe,
    Not32Bit,
    Truncated,
}

impl std::fmt::Display for LeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LeError::NotLe => "not a DOS/4GW Linear Executable (LE)",
            LeError::Not32Bit => "not a 32-bit Linear Executable",
            LeError::Truncated => "the executable is truncated",
        })
    }
}

fn u16_at(b: &[u8], off: usize) -> Result<u16, LeError> {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or(LeError::Truncated)
}

fn i16_at(b: &[u8], off: usize) -> Result<i16, LeError> {
    b.get(off..off + 2)
        .map(|s| i16::from_le_bytes([s[0], s[1]]))
        .ok_or(LeError::Truncated)
}

fn u32_at(b: &[u8], off: usize) -> Result<u32, LeError> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or(LeError::Truncated)
}

const OBJECT_EXECUTABLE: u32 = 0x0004;
const OBJECT_BIG_32BIT: u32 = 0x2000;

impl<'a> Image<'a> {
    pub fn parse(file: &'a [u8]) -> Result<Self, LeError> {
        if file.get(0..2) != Some(b"MZ") {
            return Err(LeError::NotLe);
        }
        let le = u32_at(file, 0x3c)? as usize;
        if file.get(le..le + 4) != Some(b"LE\0\0") {
            return Err(LeError::NotLe);
        }
        let cpu_type = u16_at(file, le + 0x08)?;
        if cpu_type < 2 {
            return Err(LeError::Not32Bit);
        }
        let module_pages = u32_at(file, le + 0x14)?;
        let page_size = u32_at(file, le + 0x28)?;
        if page_size == 0 {
            return Err(LeError::Truncated);
        }
        let last_page_size = u32_at(file, le + 0x2c)?;
        let obj_table_off = le
            .checked_add(u32_at(file, le + 0x40)? as usize)
            .ok_or(LeError::Truncated)?;
        let obj_count = u32_at(file, le + 0x44)? as usize;
        let pmap_off = le
            .checked_add(u32_at(file, le + 0x48)? as usize)
            .ok_or(LeError::Truncated)?;
        let fixup_page_table_offset = le
            .checked_add(u32_at(file, le + 0x68)? as usize)
            .ok_or(LeError::Truncated)?;
        let fixup_record_table_offset = le
            .checked_add(u32_at(file, le + 0x6c)? as usize)
            .ok_or(LeError::Truncated)?;
        let data_pages_offset = u32_at(file, le + 0x80)? as usize;

        let mut objects = Vec::with_capacity(obj_count);
        for i in 0..obj_count {
            let entry = obj_table_off + i * 24;
            let virtual_size = u32_at(file, entry)?;
            let reloc_base = u32_at(file, entry + 4)?;
            let flags = u32_at(file, entry + 8)?;
            let first_page_index = u32_at(file, entry + 12)?;
            let page_count = u32_at(file, entry + 16)?;
            let mut pages = Vec::with_capacity(page_count as usize);
            for p in 0..page_count {
                let p_idx = (first_page_index.saturating_sub(1) + p) as usize;
                let p_off = pmap_off + p_idx * 4;
                let b = file.get(p_off..p_off + 4).ok_or(LeError::Truncated)?;
                let physical_page = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
                pages.push(PageEntry {
                    physical_page,
                    flags: b[3],
                });
            }
            objects.push(Object {
                index: (i + 1) as u32,
                virtual_size,
                reloc_base,
                flags,
                first_page_index,
                page_count,
                executable: (flags & OBJECT_EXECUTABLE) != 0,
                pages,
            });
        }

        if !objects.is_empty() && !objects.iter().any(|o| (o.flags & OBJECT_BIG_32BIT) != 0) {
            return Err(LeError::Not32Bit);
        }

        Ok(Image {
            file,
            le_offset: le,
            page_size,
            last_page_size,
            module_pages,
            data_pages_offset,
            fixup_page_table_offset,
            fixup_record_table_offset,
            objects,
        })
    }

    /// Maps a virtual address range `[address, address + len)` inside one object
    /// to its contiguous raw file offset, if every spanned page is file-backed.
    pub fn va_to_file_offset(&self, address: u32, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        let end_va = address.checked_add(len as u32)?;
        let obj = self.objects.iter().find(|o| {
            address >= o.reloc_base && end_va <= o.reloc_base.saturating_add(o.virtual_size)
        })?;
        let rel_start = address - obj.reloc_base;
        let rel_last = end_va - 1 - obj.reloc_base;
        let first_p = (rel_start / self.page_size) as usize;
        let last_p = (rel_last / self.page_size) as usize;
        if last_p >= obj.pages.len() {
            return None;
        }
        let first_entry = obj.pages[first_p];
        if first_entry.flags != 0 || first_entry.physical_page == 0 {
            return None;
        }
        for (step, p) in (first_p..=last_p).enumerate() {
            let entry = obj.pages[p];
            if entry.flags != 0 || entry.physical_page != first_entry.physical_page + step as u32 {
                return None;
            }
        }
        let page_rem = (rel_start % self.page_size) as usize;
        let file_off = self.data_pages_offset
            + ((first_entry.physical_page - 1) * self.page_size) as usize
            + page_rem;
        if file_off.checked_add(len)? <= self.file.len() {
            Some(file_off)
        } else {
            None
        }
    }

    /// Maps a raw file offset in the data pages area back to its runtime Virtual Address.
    pub fn file_offset_to_va(&self, file_offset: usize) -> Option<u32> {
        if file_offset < self.data_pages_offset || file_offset >= self.file.len() {
            return None;
        }
        let rel = (file_offset - self.data_pages_offset) as u32;
        let phys_page = (rel / self.page_size) + 1;
        let page_rem = rel % self.page_size;
        for obj in &self.objects {
            for (p_idx, entry) in obj.pages.iter().enumerate() {
                if entry.flags == 0 && entry.physical_page == phys_page {
                    let va = obj.reloc_base + (p_idx as u32) * self.page_size + page_rem;
                    if va < obj.reloc_base.saturating_add(obj.virtual_size) {
                        return Some(va);
                    }
                }
            }
        }
        None
    }

    /// Returns the file bytes backing `[address, address + len)` if inside an
    /// executable code object.
    pub fn code_at(&self, address: u32, len: usize) -> Option<&'a [u8]> {
        let obj = self.objects.iter().find(|o| {
            o.executable
                && address >= o.reloc_base
                && address.saturating_add(len as u32) <= o.reloc_base.saturating_add(o.virtual_size)
        })?;
        let _ = obj;
        let off = self.va_to_file_offset(address, len)?;
        self.file.get(off..off + len)
    }

    /// Returns the file bytes backing `[address, address + len)` inside any
    /// file-backed object (executable code or initialized data).
    pub fn bytes_at(&self, address: u32, len: usize) -> Option<&'a [u8]> {
        let off = self.va_to_file_offset(address, len)?;
        self.file.get(off..off + len)
    }

    /// Checks whether any `DOS4GW` LE relocation fixup record modifies any byte
    /// within `[address, address + len)`.
    pub fn has_fixup_overlap(&self, address: u32, len: usize) -> Result<bool, LeError> {
        if len == 0 {
            return Ok(false);
        }
        let end_va = address.saturating_add(len as u32);
        let Some(obj) = self.objects.iter().find(|o| {
            address >= o.reloc_base && end_va <= o.reloc_base.saturating_add(o.virtual_size)
        }) else {
            return Ok(false);
        };
        let rel_start = (address - obj.reloc_base) as i64;
        let rel_end = (end_va - obj.reloc_base) as i64;
        let first_p = (rel_start as u32 / self.page_size) as usize;
        let last_p = ((rel_end as u32).saturating_sub(1) / self.page_size) as usize;

        for (p_idx, &entry) in obj.pages.iter().enumerate().take(last_p + 1).skip(first_p) {
            if entry.flags != 0 || entry.physical_page == 0 {
                continue;
            }
            let global_page = (entry.physical_page - 1) as usize;
            if global_page >= self.module_pages as usize {
                continue;
            }
            let fp_off = self.fixup_page_table_offset + global_page * 4;
            let rec_start = u32_at(self.file, fp_off)? as usize;
            let rec_end = u32_at(self.file, fp_off + 4)? as usize;
            let mut pos = self.fixup_record_table_offset + rec_start;
            let end = self.fixup_record_table_offset + rec_end;
            let page_base_rel = (p_idx as i64) * (self.page_size as i64);

            while pos < end {
                let hdr = self.file.get(pos..pos + 2).ok_or(LeError::Truncated)?;
                let src = hdr[0];
                let flg = hdr[1];
                pos += 2;

                let fixup_len: i64 = match src & 0x0f {
                    0x00 => 1,
                    0x02 | 0x05 => 2,
                    0x03 | 0x07 | 0x08 => 4,
                    0x06 => 6,
                    _ => 4,
                };

                let mut single_src_off = 0i16;
                let count = if (src & 0x20) != 0 {
                    let c = *self.file.get(pos).ok_or(LeError::Truncated)? as usize;
                    pos += 1;
                    c
                } else {
                    single_src_off = i16_at(self.file, pos)?;
                    pos += 2;
                    1
                };

                // Target object number
                pos += if (flg & 0x40) != 0 { 2 } else { 1 };
                // Target offset (for internal reference type 0 when src_type != 2)
                if (flg & 0x03) == 0 && (src & 0x0f) != 0x02 {
                    pos += if (flg & 0x10) != 0 { 4 } else { 2 };
                }
                // Additive value
                if (flg & 0x04) != 0 {
                    pos += if (flg & 0x20) != 0 { 4 } else { 2 };
                }

                if (src & 0x20) != 0 {
                    for _ in 0..count {
                        let s_off = i16_at(self.file, pos)? as i64;
                        pos += 2;
                        let f_start = page_base_rel + s_off;
                        let f_end = f_start + fixup_len;
                        if f_start < rel_end && f_end > rel_start {
                            return Ok(true);
                        }
                    }
                } else {
                    let f_start = page_base_rel + (single_src_off as i64);
                    let f_end = f_start + fixup_len;
                    if f_start < rel_end && f_end > rel_start {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Builds a minimal synthetic 32-bit Watcom LE executable with 1 code object
    /// at `0x00010000` and 1 data object at `0x00100000`, plus one optional
    /// 32-bit internal fixup on page 1 at `fixup_src_off`.
    pub(crate) fn synthetic(code: &[u8], data: &[u8], fixup_src_off: Option<i16>) -> Vec<u8> {
        let mut f = vec![0u8; 0x300];
        f[0..2].copy_from_slice(b"MZ");
        let le = 0x80usize;
        f[0x3c..0x40].copy_from_slice(&(le as u32).to_le_bytes());
        f[le..le + 4].copy_from_slice(b"LE\0\0");
        f[le + 0x08..le + 0x0a].copy_from_slice(&2u16.to_le_bytes()); // 386+
        f[le + 0x14..le + 0x18].copy_from_slice(&2u32.to_le_bytes()); // 2 pages
        f[le + 0x28..le + 0x2c].copy_from_slice(&4096u32.to_le_bytes()); // page size
        f[le + 0x2c..le + 0x30].copy_from_slice(&(data.len() as u32).to_le_bytes());

        // Relative offsets from LE header:
        // 0x100: Object Table (2 * 24 = 48 bytes)
        // 0x130: Object Page Table (2 * 4 = 8 bytes)
        // 0x140: Fixup Page Table (3 * 4 = 12 bytes)
        // 0x150: Fixup Record Table
        f[le + 0x40..le + 0x44].copy_from_slice(&0x100u32.to_le_bytes());
        f[le + 0x44..le + 0x48].copy_from_slice(&2u32.to_le_bytes());
        f[le + 0x48..le + 0x4c].copy_from_slice(&0x130u32.to_le_bytes());
        f[le + 0x68..le + 0x6c].copy_from_slice(&0x140u32.to_le_bytes());
        f[le + 0x6c..le + 0x70].copy_from_slice(&0x150u32.to_le_bytes());
        f[le + 0x80..le + 0x84].copy_from_slice(&0x300u32.to_le_bytes());

        // Obj 1: Code at 0x00010000
        let o1 = le + 0x100;
        f[o1..o1 + 4].copy_from_slice(&(code.len().max(1) as u32).to_le_bytes());
        f[o1 + 4..o1 + 8].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        f[o1 + 8..o1 + 12].copy_from_slice(&0x2045u32.to_le_bytes());
        f[o1 + 12..o1 + 16].copy_from_slice(&1u32.to_le_bytes());
        f[o1 + 16..o1 + 20].copy_from_slice(&1u32.to_le_bytes());

        // Obj 2: Data at 0x00100000
        let o2 = o1 + 24;
        f[o2..o2 + 4].copy_from_slice(&(data.len().max(1) as u32).to_le_bytes());
        f[o2 + 4..o2 + 8].copy_from_slice(&0x0010_0000u32.to_le_bytes());
        f[o2 + 8..o2 + 12].copy_from_slice(&0x2043u32.to_le_bytes());
        f[o2 + 12..o2 + 16].copy_from_slice(&2u32.to_le_bytes());
        f[o2 + 16..o2 + 20].copy_from_slice(&1u32.to_le_bytes());

        // Object Page Table: page 1 -> [0, 0, 1, 0], page 2 -> [0, 0, 2, 0]
        let pmap = le + 0x130;
        f[pmap..pmap + 4].copy_from_slice(&[0, 0, 1, 0]);
        f[pmap + 4..pmap + 8].copy_from_slice(&[0, 0, 2, 0]);

        // Fixup Page Table & Record Table
        let fpage = le + 0x140;
        let frec = le + 0x150;
        if let Some(src_off) = fixup_src_off {
            // 7-byte internal 32-bit offset fixup: 07 00 <src_off:i16> 02 <tgt_off:u16>
            f[frec] = 0x07;
            f[frec + 1] = 0x00;
            f[frec + 2..frec + 4].copy_from_slice(&src_off.to_le_bytes());
            f[frec + 4] = 0x02;
            f[frec + 5..frec + 7].copy_from_slice(&0u16.to_le_bytes());
            f[fpage..fpage + 4].copy_from_slice(&0u32.to_le_bytes());
            f[fpage + 4..fpage + 8].copy_from_slice(&7u32.to_le_bytes());
            f[fpage + 8..fpage + 12].copy_from_slice(&7u32.to_le_bytes());
        }

        // Data pages at 0x300 (page 1 = 4096 B, page 2 = 4096 B)
        let mut page1 = vec![0u8; 4096];
        page1[..code.len()].copy_from_slice(code);
        let mut page2 = vec![0u8; 4096];
        page2[..data.len()].copy_from_slice(data);
        f.extend_from_slice(&page1);
        f.extend_from_slice(&page2);
        f
    }

    #[test]
    fn maps_code_and_data_addresses() {
        let file = synthetic(&[0x55, 0x89, 0xe5, 0xc3], &[0x11, 0x22, 0x33, 0x44], None);
        let img = Image::parse(&file).unwrap();
        assert_eq!(img.objects.len(), 2);
        assert_eq!(img.code_at(0x0001_0001, 2), Some(&[0x89, 0xe5][..]));
        assert_eq!(
            img.code_at(0x0010_0000, 2),
            None,
            "data object is not executable"
        );
        assert_eq!(img.bytes_at(0x0010_0001, 3), Some(&[0x22, 0x33, 0x44][..]));
        let f_off = img.va_to_file_offset(0x0001_0002, 2).unwrap();
        assert_eq!(img.file_offset_to_va(f_off), Some(0x0001_0002));
    }

    #[test]
    fn detects_fixup_overlap() {
        let code = [0xa1, 0x00, 0x00, 0x00, 0x00, 0xc3]; // mov eax, [reloc]; ret
        let file = synthetic(&code, &[0u8; 8], Some(1));
        let img = Image::parse(&file).unwrap();
        assert!(
            !img.has_fixup_overlap(0x0001_0000, 1).unwrap(),
            "opcode byte before fixup"
        );
        assert!(
            img.has_fixup_overlap(0x0001_0001, 4).unwrap(),
            "overlaps relocated address"
        );
        assert!(
            img.has_fixup_overlap(0x0001_0004, 2).unwrap(),
            "overlaps last byte of fixup"
        );
        assert!(
            !img.has_fixup_overlap(0x0001_0005, 1).unwrap(),
            "ret byte after fixup"
        );
    }

    #[test]
    fn rejects_non_le() {
        assert_eq!(Image::parse(b"hello").unwrap_err(), LeError::NotLe);
    }
}
