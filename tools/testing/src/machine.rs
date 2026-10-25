/// Names the machine of an ELF image, or None for any other file.
pub fn elf_machine(head: &[u8]) -> Option<String> {
    if head.len() < 20 || &head[..4] != b"\x7fELF" {
        return None;
    }
    let raw = match head[5] {
        1 => u16::from_le_bytes([head[18], head[19]]),
        2 => u16::from_be_bytes([head[18], head[19]]),
        _ => return None,
    };
    Some(
        match raw {
            0x03 => "x86",
            0x28 => "arm",
            0x3e => "x86_64",
            0xb7 => "aarch64",
            0xf3 => "riscv",
            _ => return Some(format!("elf-0x{raw:x}")),
        }
        .to_owned(),
    )
}

/// Names the machine of a PE image, or None for any other file.
pub fn pe_machine(image: &[u8]) -> Option<String> {
    if image.len() < 0x40 || &image[..2] != b"MZ" {
        return None;
    }
    let offset = u32::from_le_bytes([image[0x3c], image[0x3d], image[0x3e], image[0x3f]]) as usize;
    let header = image.get(offset..offset.checked_add(6)?)?;
    if &header[..4] != b"PE\0\0" {
        return None;
    }
    let raw = u16::from_le_bytes([header[4], header[5]]);
    Some(
        match raw {
            0x14c => "x86",
            0x8664 => "x86_64",
            0xaa64 => "aarch64",
            0x1c4 => "arm",
            _ => return Some(format!("pe-0x{raw:x}")),
        }
        .to_owned(),
    )
}

/// Names the machine of an ELF or PE image, or None for any other file.
pub fn image_machine(head: &[u8]) -> Option<String> {
    elf_machine(head).or_else(|| pe_machine(head))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elf_machine_reads_the_header_bytes() {
        let mut x86 = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];
        x86.resize(18, 0);
        x86.extend_from_slice(&[0x3e, 0x00]);
        assert_eq!(elf_machine(&x86).as_deref(), Some("x86_64"));
        let mut arm = x86.clone();
        arm[18] = 0xb7;
        assert_eq!(elf_machine(&arm).as_deref(), Some("aarch64"));
        let mut big = x86.clone();
        big[5] = 2;
        big[18] = 0x00;
        big[19] = 0x28;
        assert_eq!(elf_machine(&big).as_deref(), Some("arm"));
        let mut odd = x86.clone();
        odd[18] = 0x99;
        assert_eq!(elf_machine(&odd).as_deref(), Some("elf-0x99"));
        let mut i386 = x86.clone();
        i386[18] = 0x03;
        assert_eq!(elf_machine(&i386).as_deref(), Some("x86"));
        let mut riscv = x86.clone();
        riscv[18] = 0xf3;
        assert_eq!(elf_machine(&riscv).as_deref(), Some("riscv"));
        let mut unknown_order = x86.clone();
        unknown_order[5] = 0;
        assert_eq!(elf_machine(&unknown_order), None);
        assert_eq!(elf_machine(b"!<arch>\nlibsolo.a padding"), None);
        assert_eq!(elf_machine(&x86[..10]), None);
    }

    #[test]
    fn pe_machine_reads_the_coff_header() {
        let mut pe = vec![0u8; 0x40];
        pe[..2].copy_from_slice(b"MZ");
        pe[0x3c] = 0x40;
        pe.extend_from_slice(b"PE\0\0");
        pe.extend_from_slice(&[0x64, 0x86]);
        assert_eq!(pe_machine(&pe).as_deref(), Some("x86_64"));
        let last = pe.len() - 2;
        pe[last] = 0x64;
        pe[last + 1] = 0xaa;
        assert_eq!(pe_machine(&pe).as_deref(), Some("aarch64"));
        pe[last] = 0x99;
        pe[last + 1] = 0x00;
        assert_eq!(pe_machine(&pe).as_deref(), Some("pe-0x99"));
        assert_eq!(pe_machine(&pe[..0x20]), None);
        assert_eq!(pe_machine(b"\x7fELF"), None);
    }

    #[test]
    fn image_machine_reads_elf_and_pe() {
        let mut elf = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];
        elf.resize(18, 0);
        elf.extend_from_slice(&[0xb7, 0x00]);
        assert_eq!(image_machine(&elf).as_deref(), Some("aarch64"));
        let mut pe = vec![0u8; 0x40];
        pe[..2].copy_from_slice(b"MZ");
        pe[0x3c] = 0x40;
        pe.extend_from_slice(b"PE\0\0");
        pe.extend_from_slice(&[0x64, 0x86]);
        assert_eq!(image_machine(&pe).as_deref(), Some("x86_64"));
        assert_eq!(image_machine(b"#!/bin/sh\necho hi\n"), None);
        assert_eq!(image_machine(b""), None);
    }
}
