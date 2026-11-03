use super::*;

fn elf(machine: u16, id: Option<&[u8]>) -> Vec<u8> {
    let shstrtab = b"\0.shstrtab\0.note.gnu.build-id\0";
    let mut note = Vec::new();
    if let Some(id) = id {
        note.extend_from_slice(&4u32.to_le_bytes());
        note.extend_from_slice(&(id.len() as u32).to_le_bytes());
        note.extend_from_slice(&3u32.to_le_bytes());
        note.extend_from_slice(b"GNU\0");
        note.extend_from_slice(id);
    }
    let str_off = 64u64;
    let note_off = str_off + shstrtab.len() as u64;
    let sh_off = (note_off + note.len() as u64).next_multiple_of(8);
    let mut b = vec![0u8; 64];
    b[..4].copy_from_slice(b"\x7fELF");
    b[4] = 2;
    b[5] = 1;
    b[6] = 1;
    b[16..18].copy_from_slice(&2u16.to_le_bytes());
    b[18..20].copy_from_slice(&machine.to_le_bytes());
    b[20..24].copy_from_slice(&1u32.to_le_bytes());
    b[40..48].copy_from_slice(&sh_off.to_le_bytes());
    b[52..54].copy_from_slice(&64u16.to_le_bytes());
    b[58..60].copy_from_slice(&64u16.to_le_bytes());
    b[60..62].copy_from_slice(&3u16.to_le_bytes());
    b[62..64].copy_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(shstrtab);
    b.extend_from_slice(&note);
    b.resize(sh_off as usize, 0);
    b.extend_from_slice(&[0u8; 64]);
    let section = |name: u32, kind: u32, off: u64, size: u64| {
        let mut s = vec![0u8; 64];
        s[0..4].copy_from_slice(&name.to_le_bytes());
        s[4..8].copy_from_slice(&kind.to_le_bytes());
        s[24..32].copy_from_slice(&off.to_le_bytes());
        s[32..40].copy_from_slice(&size.to_le_bytes());
        s[48..56].copy_from_slice(&4u64.to_le_bytes());
        s
    };
    b.extend(section(1, 3, str_off, shstrtab.len() as u64));
    b.extend(section(11, 7, note_off, note.len() as u64));
    b
}

const X86: u16 = 62;
const ARM: u16 = 183;

#[test]
fn matching_pair_reports_id_and_arch() {
    let id = [0xab, 0xcd, 0x01];
    let pair = check_pair(&elf(X86, Some(&id)), &elf(X86, Some(&id))).expect("pair");
    assert_eq!(pair.build_id, "abcd01");
    assert_eq!(pair.arch, "x86_64");
}

#[test]
fn rejects_non_elf() {
    let err = check_pair(b"hello world", &elf(X86, Some(&[1]))).unwrap_err();
    assert!(err.contains("not an ELF"), "{err}");
}

#[test]
fn rejects_truncated_file() {
    let good = elf(X86, Some(&[1, 2]));
    let err = check_pair(&good, &good[..70]).unwrap_err();
    assert!(err.contains("corrupt"), "{err}");
}

#[test]
fn rejects_missing_build_id() {
    let err = check_pair(&elf(X86, None), &elf(X86, Some(&[1]))).unwrap_err();
    assert!(err.contains("no build-id"), "{err}");
}

#[test]
fn rejects_build_id_mismatch() {
    let err = check_pair(&elf(X86, Some(&[1])), &elf(X86, Some(&[2]))).unwrap_err();
    assert!(err.contains("build-id mismatch"), "{err}");
}

#[test]
fn rejects_arch_mismatch() {
    let err = check_pair(&elf(X86, Some(&[1])), &elf(ARM, Some(&[1]))).unwrap_err();
    assert!(err.contains("arch mismatch"), "{err}");
}
