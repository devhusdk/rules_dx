//! Checks that a binary and its debug info belong together: ELF build-ids and
//! macOS dSYM UUIDs.

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
        "dwarf": rel.to_string_lossy(),
        "dsymutil": tool,
    }))
}

#[cfg(test)]
mod tests;
