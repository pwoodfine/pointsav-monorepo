fn align4(n: usize) -> usize {
    (n + 3) & !3
}

fn make_entry(name: &str, data: &[u8]) -> Vec<u8> {
    let name_bytes: Vec<u8> = name.bytes().chain(std::iter::once(0u8)).collect();
    let namesize = name_bytes.len();
    let filesize = data.len();
    let hdr = format!(
        "070701{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        0u32,
        0x81a4u32, // mode: regular file 0644
        0u32,
        0u32,
        1u32, // nlink
        0u32,
        filesize as u32,
        8u32, // devmajor
        1u32, // devminor
        0u32,
        0u32,
        namesize as u32,
        0u32, // check
    );
    debug_assert_eq!(hdr.len(), 110, "CPIO newc header must be exactly 110 bytes");
    let name_pad = align4(110 + namesize) - 110 - namesize;
    let data_pad = align4(filesize) - filesize;
    let mut out = Vec::with_capacity(align4(110 + namesize) + align4(filesize));
    out.extend_from_slice(hdr.as_bytes());
    out.extend_from_slice(&name_bytes);
    out.resize(out.len() + name_pad, 0u8);
    out.extend_from_slice(data);
    out.resize(out.len() + data_pad, 0u8);
    out
}

/// Write a CPIO "newc" (070701) archive.
///
/// Appends a `TRAILER!!!` sentinel automatically.
/// Padding: header+name aligned to 4 bytes; data aligned to 4 bytes.
pub fn write_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for &(name, data) in entries {
        out.extend_from_slice(&make_entry(name, data));
    }
    out.extend_from_slice(&make_entry("TRAILER!!!", &[]));
    out
}

#[cfg(test)]
mod tests {
    use super::write_archive;

    #[test]
    fn archive_starts_with_cpio_magic() {
        let bytes = write_archive(&[("hello", b"world")]);
        assert_eq!(&bytes[..6], b"070701");
    }

    #[test]
    fn archive_contains_trailer_sentinel() {
        let bytes = write_archive(&[]);
        let trailer = b"TRAILER!!!\0";
        assert!(
            bytes.windows(trailer.len()).any(|w| w == trailer),
            "CPIO archive must contain TRAILER!!! sentinel"
        );
    }

    #[test]
    fn namesize_field_correct_at_header_offset_94() {
        // name "ab\0" = 3 bytes → namesize field at header offset 94
        let bytes = write_archive(&[("ab", b"")]);
        let ns = u32::from_str_radix(std::str::from_utf8(&bytes[94..102]).unwrap(), 16).unwrap();
        assert_eq!(
            ns, 3,
            "namesize for 'ab' should be 3 (including null terminator)"
        );
    }

    #[test]
    fn entries_are_4_byte_aligned() {
        // name "x\0"=2 bytes; align4(110+2)=112; data "abc"=3 bytes; align4(3)=4
        // first entry = 112 + 4 = 116 bytes; TRAILER starts at offset 116
        let bytes = write_archive(&[("x", b"abc")]);
        assert_eq!(
            &bytes[116..122],
            b"070701",
            "TRAILER entry must start at 4-byte-aligned offset 116"
        );
    }
}
