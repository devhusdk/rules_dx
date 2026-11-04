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

fn macho(cputype: u32, filetype: u32, uuid: Option<[u8; 16]>) -> Vec<u8> {
    let mut cmds = Vec::new();
    if let Some(uuid) = uuid {
        cmds.extend_from_slice(&0x1bu32.to_le_bytes());
        cmds.extend_from_slice(&24u32.to_le_bytes());
        cmds.extend_from_slice(&uuid);
    }
    let mut b = Vec::new();
    b.extend_from_slice(&0xfeed_facfu32.to_le_bytes());
    b.extend_from_slice(&cputype.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&filetype.to_le_bytes());
    b.extend_from_slice(&u32::from(uuid.is_some()).to_le_bytes());
    b.extend_from_slice(&(cmds.len() as u32).to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&cmds);
    b
}

const MH_EXECUTE: u32 = 2;
const MH_DSYM: u32 = 0xa;
const ARM64: u32 = 0x0100_000c;
const X86_64: u32 = 0x0100_0007;

fn scratch(name: &str) -> PathBuf {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn dsym_with_same_uuid_and_arch_pairs() {
    let uuid = [7u8; 16];
    let pair = check_dsym(
        &macho(ARM64, MH_EXECUTE, Some(uuid)),
        &macho(ARM64, MH_DSYM, Some(uuid)),
    )
    .expect("pair");
    assert_eq!(pair.uuid, "07".repeat(16));
    assert_eq!(pair.arch, "aarch64");
}

#[test]
fn stale_dsym_with_other_uuid_is_rejected() {
    let bin = macho(ARM64, MH_EXECUTE, Some([1; 16]));
    let err = check_dsym(&bin, &macho(ARM64, MH_DSYM, Some([2; 16]))).unwrap_err();
    assert!(err.contains("uuid mismatch"), "{err}");
}

#[test]
fn dsym_for_other_arch_is_rejected() {
    let bin = macho(ARM64, MH_EXECUTE, Some([3; 16]));
    let err = check_dsym(&bin, &macho(X86_64, MH_DSYM, Some([3; 16]))).unwrap_err();
    assert!(err.contains("arch mismatch"), "{err}");
}

#[test]
fn dsym_check_rejects_missing_uuid_and_non_macho() {
    let bin = macho(ARM64, MH_EXECUTE, Some([4; 16]));
    let err = check_dsym(&bin, &macho(ARM64, MH_DSYM, None)).unwrap_err();
    assert!(err.contains("has no LC_UUID"), "{err}");
    let err = check_dsym(&bin, &elf(X86, Some(&[1]))).unwrap_err();
    assert!(err.contains("dSYM is not a"), "{err}");
}

#[test]
fn dwarf_file_needs_exactly_one_file_in_the_bundle() {
    let bundle = scratch("dsym-dwarf-one");
    let err = dwarf_file(&bundle).unwrap_err();
    assert!(err.contains("cannot read dSYM"), "{err}");
    let dir = bundle.join("Contents/Resources/DWARF");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(dwarf_file(&bundle).unwrap_err().contains("found 0"));
    std::fs::write(dir.join("app"), b"x").unwrap();
    assert_eq!(dwarf_file(&bundle).unwrap(), dir.join("app"));
    std::fs::write(dir.join("other"), b"y").unwrap();
    assert!(dwarf_file(&bundle).unwrap_err().contains("found 2"));
}

#[test]
fn manifest_records_uuid_arch_digest_and_tool() {
    let uuid = [9u8; 16];
    let root = scratch("dsym-manifest");
    let bin = macho(ARM64, MH_EXECUTE, Some(uuid));
    std::fs::write(root.join("app"), &bin).unwrap();
    let bundle = root.join("app.dSYM");
    let dir = bundle.join("Contents/Resources/DWARF");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app"), macho(ARM64, MH_DSYM, Some(uuid))).unwrap();
    let json = dsym_manifest(&root.join("app"), &bundle, "LLVM version 22").unwrap();
    assert_eq!(json["uuid"], "09".repeat(16));
    assert_eq!(json["arch"], "aarch64");
    assert_eq!(json["dwarf"], "Contents/Resources/DWARF/app");
    assert_eq!(json["dsymutil"], "LLVM version 22");
    assert_eq!(json["sha256"], format!("{:x}", Sha256::digest(&bin)));
}

fn msf(info: Option<(&[u8; 16], u32)>, dbi: Option<(u32, u16)>) -> Vec<u8> {
    const BLOCK: usize = 512;
    let mut streams: Vec<Vec<u8>> = vec![Vec::new(); 4];
    if let Some((guid, age)) = info {
        let mut s = vec![0u8; 12];
        s.extend_from_slice(guid);
        s[8..12].copy_from_slice(&age.to_le_bytes());
        streams[1] = s;
    }
    if let Some((age, machine)) = dbi {
        let mut s = vec![0u8; 64];
        s[8..12].copy_from_slice(&age.to_le_bytes());
        s[58..60].copy_from_slice(&machine.to_le_bytes());
        streams[3] = s;
    }
    let mut dir = (streams.len() as u32).to_le_bytes().to_vec();
    for s in &streams {
        dir.extend_from_slice(&(s.len() as u32).to_le_bytes());
    }
    let mut data = Vec::new();
    let mut next = 3u32;
    for s in &streams {
        if !s.is_empty() {
            dir.extend_from_slice(&next.to_le_bytes());
            data.extend_from_slice(s);
            data.resize(data.len().next_multiple_of(BLOCK), 0);
            next += 1;
        }
    }
    let mut b = vec![0u8; BLOCK * 3];
    b[..MSF_MAGIC.len()].copy_from_slice(MSF_MAGIC);
    b[32..36].copy_from_slice(&(BLOCK as u32).to_le_bytes());
    b[40..44].copy_from_slice(&next.to_le_bytes());
    b[44..48].copy_from_slice(&(dir.len() as u32).to_le_bytes());
    b[52..56].copy_from_slice(&1u32.to_le_bytes());
    b[BLOCK..BLOCK + 4].copy_from_slice(&2u32.to_le_bytes());
    b[BLOCK * 2..BLOCK * 2 + dir.len()].copy_from_slice(&dir);
    b.extend_from_slice(&data);
    b
}

const AMD64: u16 = 0x8664;
const GUID: [u8; 16] = [7; 16];

fn pe_id(guid: [u8; 16], age: u32, machine: u16) -> PeId {
    PeId {
        guid,
        age,
        path: "app.pdb".to_string(),
        machine,
        linker_version: "14.0".to_string(),
    }
}

#[test]
fn pdb_identity_reads_guid_age_and_machine() {
    let id = pdb_identity(&msf(Some((&GUID, 3)), Some((3, AMD64)))).expect("pdb");
    assert_eq!(
        id,
        PdbId {
            guid: GUID,
            age: 3,
            machine: AMD64
        }
    );
}

#[test]
fn matching_pdb_pairs() {
    let id = pdb_identity(&msf(Some((&GUID, 3)), Some((3, AMD64)))).unwrap();
    let pair = compare_pdb(&pe_id(GUID, 3, AMD64), &id).expect("pair");
    assert_eq!(pair.guid, "07".repeat(16));
    assert_eq!(pair.age, 3);
    assert_eq!(pair.arch, "x86_64");
}

#[test]
fn pdb_guid_mismatch_is_rejected() {
    let id = pdb_identity(&msf(Some((&[9; 16], 3)), Some((3, AMD64)))).unwrap();
    let err = compare_pdb(&pe_id(GUID, 3, AMD64), &id).unwrap_err();
    assert!(err.contains("guid mismatch"), "{err}");
}

#[test]
fn stale_pdb_age_is_rejected() {
    let id = pdb_identity(&msf(Some((&GUID, 3)), Some((2, AMD64)))).unwrap();
    let err = compare_pdb(&pe_id(GUID, 3, AMD64), &id).unwrap_err();
    assert!(err.contains("age mismatch"), "{err}");
}

#[test]
fn pdb_machine_mismatch_is_rejected() {
    let id = pdb_identity(&msf(Some((&GUID, 3)), Some((3, 0xaa64)))).unwrap();
    let err = compare_pdb(&pe_id(GUID, 3, AMD64), &id).unwrap_err();
    assert!(err.contains("arch mismatch"), "{err}");
}

#[test]
fn truncated_pdb_is_rejected() {
    let mut bytes = msf(Some((&GUID, 3)), Some((3, AMD64)));
    bytes.truncate(bytes.len() - 512);
    let err = pdb_identity(&bytes).unwrap_err();
    assert!(err.contains("truncated or corrupt"), "{err}");
}

#[test]
fn pdb_with_bad_magic_is_rejected() {
    let err = pdb_identity(b"hello world").unwrap_err();
    assert!(err.contains("not a PDB"), "{err}");
}

#[test]
fn pdb_without_dbi_stream_is_rejected() {
    let err = pdb_identity(&msf(Some((&GUID, 3)), None)).unwrap_err();
    assert!(err.contains("no DBI stream"), "{err}");
}

#[test]
fn pe_codeview_rejects_elf() {
    let err = pe_codeview(&elf(X86, Some(&[1]))).unwrap_err();
    assert!(err.contains("not a PE file"), "{err}");
}
