//! Checks that a binary and its debug info belong together: ELF build-ids and
//! macOS dSYM UUIDs and Windows PDB GUIDs.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use object::{Architecture, Object};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The build-id and architecture shared by a binary and its debug file.
#[derive(Debug, PartialEq, Eq)]
pub struct Pair {
    pub build_id: String,
    pub arch: String,
}

struct Elf {
    build_id: Vec<u8>,
    arch: Architecture,
    machine: u16,
}

fn read(bytes: &[u8], what: &str) -> Result<Elf, String> {
    let file = object::File::parse(bytes).map_err(|e| {
        if bytes.starts_with(b"\x7fELF") {
            format!("{what} is corrupt: {e}")
        } else {
            format!("{what} is not an ELF file")
        }
    })?;
    if file.format() != object::BinaryFormat::Elf {
        return Err(format!("{what} is not an ELF file"));
    }
    let build_id = file
        .build_id()
        .map_err(|e| format!("{what} is corrupt: {e}"))?
        .ok_or_else(|| format!("{what} has no build-id"))?
        .to_vec();
    let raw = [bytes[18], bytes[19]];
    let machine = if file.is_little_endian() {
        u16::from_le_bytes(raw)
    } else {
        u16::from_be_bytes(raw)
    };
    Ok(Elf {
        build_id,
        arch: file.architecture(),
        machine,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Returns the shared build-id and arch, or says why the two files do not match.
pub fn check_pair(binary: &[u8], debug: &[u8]) -> Result<Pair, String> {
    let bin = read(binary, "binary")?;
    let dbg = read(debug, "debug file")?;
    if bin.build_id != dbg.build_id {
        return Err(format!(
            "build-id mismatch: binary {} debug file {}",
            hex(&bin.build_id),
            hex(&dbg.build_id)
        ));
    }
    if bin.machine != dbg.machine || bin.arch != dbg.arch {
        return Err(format!(
            "arch mismatch: binary {:?} debug file {:?}",
            bin.arch, dbg.arch
        ));
    }
    Ok(Pair {
        build_id: hex(&bin.build_id),
        arch: format!("{:?}", bin.arch).to_lowercase(),
    })
}

/// The UUID and arch shared by a Mach-O binary and its dSYM.
#[derive(Debug, PartialEq, Eq)]
pub struct DsymPair {
    pub uuid: String,
    pub arch: String,
}

fn macho_uuid(bytes: &[u8], what: &str) -> Result<([u8; 16], String), String> {
    let file = object::File::parse(bytes)
        .map_err(|e| format!("{what} is not a readable Mach-O file: {e}"))?;
    if file.format() != object::BinaryFormat::MachO {
        return Err(format!("{what} is not a Mach-O file"));
    }
    let uuid = file
        .mach_uuid()
        .map_err(|e| format!("{what} is corrupt: {e}"))?
        .ok_or_else(|| format!("{what} has no LC_UUID"))?;
    Ok((uuid, format!("{:?}", file.architecture()).to_lowercase()))
}

/// Returns the shared UUID and arch, or says why the binary and dSYM do not match.
pub fn check_dsym(binary: &[u8], dwarf: &[u8]) -> Result<DsymPair, String> {
    let (bin_uuid, bin_arch) = macho_uuid(binary, "binary")?;
    let (dsym_uuid, dsym_arch) = macho_uuid(dwarf, "dSYM")?;
    if bin_uuid != dsym_uuid {
        return Err(format!(
            "uuid mismatch: binary {} dSYM {} (stale dSYM?)",
            hex(&bin_uuid),
            hex(&dsym_uuid)
        ));
    }
    if bin_arch != dsym_arch {
        return Err(format!("arch mismatch: binary {bin_arch} dSYM {dsym_arch}"));
    }
    Ok(DsymPair {
        uuid: hex(&bin_uuid),
        arch: bin_arch,
    })
}

/// The one DWARF file inside a dSYM bundle, at Contents/Resources/DWARF/.
pub fn dwarf_file(bundle: &Path) -> Result<PathBuf, String> {
    let dir = bundle.join("Contents/Resources/DWARF");
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: cannot read dSYM: {e}", bundle.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        files.push(entry.map_err(|e| e.to_string())?.path());
    }
    if files.len() != 1 {
        return Err(format!(
            "{}: want one DWARF file, found {}",
            bundle.display(),
            files.len()
        ));
    }
    Ok(files.remove(0))
}

/// The manifest for a binary and its dSYM bundle: UUID, arch, binary digest and the dsymutil version.
pub fn dsym_manifest(
    binary: &Path,
    bundle: &Path,
    tool: &str,
) -> Result<serde_json::Value, String> {
    let bin = std::fs::read(binary).map_err(|e| format!("{}: {e}", binary.display()))?;
    let dwarf = dwarf_file(bundle)?;
    let dbg = std::fs::read(&dwarf).map_err(|e| format!("{}: {e}", dwarf.display()))?;
    let pair = check_dsym(&bin, &dbg)?;
    let rel = dwarf.strip_prefix(bundle).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "uuid": pair.uuid,
        "arch": pair.arch,
        "sha256": format!("{:x}", Sha256::digest(&bin)),
        "dwarf": rel.to_string_lossy().replace('\\', "/"),
        "dsymutil": tool,
    }))
}

/// What a PE file says about its PDB: the CodeView record and the machine.
#[derive(Debug, PartialEq, Eq)]
pub struct PeId {
    pub guid: [u8; 16],
    pub age: u32,
    pub path: String,
    pub machine: u16,
    pub linker_version: String,
}

/// What a PDB says about itself: the info stream GUID, and the DBI age and machine.
#[derive(Debug, PartialEq, Eq)]
pub struct PdbId {
    pub guid: [u8; 16],
    pub age: u32,
    pub machine: u16,
}

/// The GUID, age and arch shared by a PE file and its PDB.
#[derive(Debug, PartialEq, Eq)]
pub struct PdbPair {
    pub guid: String,
    pub age: u32,
    pub arch: String,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Reads the CodeView GUID, age and path, the machine and the linker version of a PE file.
pub fn pe_codeview(bytes: &[u8]) -> Result<PeId, String> {
    let file =
        object::File::parse(bytes).map_err(|e| format!("binary is not a readable PE file: {e}"))?;
    if file.format() != object::BinaryFormat::Pe {
        return Err("binary is not a PE file".to_string());
    }
    let cv = file
        .pdb_info()
        .map_err(|e| format!("binary is corrupt: {e}"))?
        .ok_or("binary has no CodeView record")?;
    let nt = u32_at(bytes, 0x3c).ok_or("binary is truncated")? as usize;
    let machine = u16_at(bytes, nt + 4).ok_or("binary is truncated")?;
    let linker = bytes.get(nt + 26..nt + 28).ok_or("binary is truncated")?;
    Ok(PeId {
        guid: cv.guid(),
        age: cv.age(),
        path: String::from_utf8_lossy(cv.path()).into_owned(),
        machine,
        linker_version: format!("{}.{}", linker[0], linker[1]),
    })
}

const MSF_MAGIC: &[u8] = b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0";

fn msf_streams(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if !bytes.starts_with(MSF_MAGIC) {
        return Err("pdb is not a PDB file".to_string());
    }
    let corrupt = || "pdb is truncated or corrupt".to_string();
    let field = |at: usize| u32_at(bytes, at).ok_or_else(corrupt);
    let block = field(32)? as usize;
    let dir_bytes = field(44)? as usize;
    let map_block = field(52)? as usize;
    if block == 0 || !block.is_power_of_two() || !bytes.len().is_multiple_of(block) {
        return Err(corrupt());
    }
    let read_blocks = |list: &[u32], len: usize| -> Result<Vec<u8>, String> {
        let mut out = Vec::with_capacity(len);
        for &n in list {
            let start = (n as usize).checked_mul(block).ok_or_else(corrupt)?;
            out.extend_from_slice(bytes.get(start..start + block).ok_or_else(corrupt)?);
        }
        out.truncate(len);
        Ok(out)
    };
    let numbers = |data: &[u8], count: usize| -> Result<Vec<u32>, String> {
        (0..count)
            .map(|i| u32_at(data, i * 4).ok_or_else(corrupt))
            .collect()
    };
    let map_start = map_block.checked_mul(block).ok_or_else(corrupt)?;
    let map = bytes
        .get(map_start..map_start + block)
        .ok_or_else(corrupt)?;
    let dir_blocks = numbers(map, dir_bytes.div_ceil(block))?;
    let dir = read_blocks(&dir_blocks, dir_bytes)?;
    let count = u32_at(&dir, 0).ok_or_else(corrupt)? as usize;
    let sizes = numbers(dir.get(4..).ok_or_else(corrupt)?, count)?;
    let mut at = 4 + count * 4;
    let mut streams = Vec::new();
    for size in sizes {
        let size = if size == u32::MAX { 0 } else { size as usize };
        let n = size.div_ceil(block);
        let list = numbers(dir.get(at..).ok_or_else(corrupt)?, n)?;
        at += n * 4;
        streams.push(read_blocks(&list, size)?);
    }
    Ok(streams)
}

/// Reads the GUID, age and machine of a PDB from its MSF info and DBI streams.
pub fn pdb_identity(bytes: &[u8]) -> Result<PdbId, String> {
    let streams = msf_streams(bytes)?;
    let info = streams.get(1).filter(|s| s.len() >= 28);
    let info = info.ok_or("pdb has no info stream")?;
    let dbi = streams.get(3).filter(|s| s.len() >= 64);
    let dbi = dbi.ok_or("pdb has no DBI stream")?;
    Ok(PdbId {
        guid: info[12..28]
            .try_into()
            .map_err(|_| "pdb has no info stream")?,
        age: u32_at(dbi, 8).ok_or("pdb has no DBI stream")?,
        machine: u16_at(dbi, 58).ok_or("pdb has no DBI stream")?,
    })
}

/// Returns the shared GUID, age and arch, or says why the PE file and PDB do not match.
pub fn compare_pdb(pe: &PeId, pdb: &PdbId) -> Result<PdbPair, String> {
    if pe.guid != pdb.guid {
        return Err(format!(
            "guid mismatch: binary {} pdb {} (stale or wrong PDB?)",
            hex(&pe.guid),
            hex(&pdb.guid)
        ));
    }
    if pe.age != pdb.age {
        return Err(format!(
            "age mismatch: binary {} pdb {} (stale PDB?)",
            pe.age, pdb.age
        ));
    }
    if pe.machine != pdb.machine {
        return Err(format!(
            "arch mismatch: binary machine {:#x} pdb machine {:#x}",
            pe.machine, pdb.machine
        ));
    }
    let arch = match pe.machine {
        0x8664 => "x86_64".to_string(),
        0xaa64 => "aarch64".to_string(),
        0x14c => "i386".to_string(),
        other => format!("machine_{other:#x}"),
    };
    Ok(PdbPair {
        guid: hex(&pe.guid),
        age: pe.age,
        arch,
    })
}

/// Checks a PE file against its PDB.
pub fn check_pdb(binary: &[u8], pdb: &[u8]) -> Result<PdbPair, String> {
    compare_pdb(&pe_codeview(binary)?, &pdb_identity(pdb)?)
}

/// The manifest for a PE file and its PDB: GUID, age, arch, binary digest, PDB name, CodeView path and linker version.
pub fn pdb_manifest(binary: &Path, pdb: &Path) -> Result<serde_json::Value, String> {
    let bin = std::fs::read(binary).map_err(|e| format!("{}: {e}", binary.display()))?;
    let dbg = std::fs::read(pdb).map_err(|e| format!("{}: {e}", pdb.display()))?;
    let pe = pe_codeview(&bin)?;
    let pair = compare_pdb(&pe, &pdb_identity(&dbg)?)?;
    let name = pdb.file_name().ok_or("pdb has no file name")?;
    Ok(serde_json::json!({
        "guid": pair.guid,
        "age": pair.age,
        "arch": pair.arch,
        "sha256": format!("{:x}", Sha256::digest(&bin)),
        "pdb": name.to_string_lossy(),
        "codeview_path": pe.path,
        "linker_version": pe.linker_version,
    }))
}

#[cfg(test)]
mod tests;
