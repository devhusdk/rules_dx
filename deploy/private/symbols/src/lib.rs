//! Checks that a binary and its debug info belong together: ELF build-ids and
//! macOS dSYM UUIDs and Windows PDB GUIDs.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

pub mod cli;
pub mod entry;

use dx_atomic_fs::path_io::{self, PathIoError};
use object::{Architecture, Object};
use std::fmt;
use std::path::{Path, PathBuf};

/// Why a binary and its debug info cannot be read or do not belong together.
#[derive(Debug)]
pub enum SymbolsError {
    WrongFormat {
        what: &'static str,
        format: &'static str,
    },
    Unreadable {
        what: &'static str,
        format: &'static str,
        source: object::Error,
    },
    Corrupt {
        what: &'static str,
        source: object::Error,
    },
    BadHeader {
        what: &'static str,
    },
    Truncated {
        what: &'static str,
    },
    NoBuildId {
        what: &'static str,
    },
    NoUuid {
        what: &'static str,
    },
    NoCodeView,
    BuildIdMismatch {
        binary: String,
        debug: String,
    },
    UuidMismatch {
        binary: String,
        dsym: String,
    },
    ArchMismatch {
        binary: String,
        other: &'static str,
        debug: String,
    },
    GuidMismatch {
        binary: String,
        pdb: String,
    },
    AgeMismatch {
        binary: u32,
        pdb: u32,
    },
    MachineMismatch {
        binary: u16,
        pdb: u16,
    },
    PdbCorrupt,
    NoInfoStream,
    NoDbiStream,
    NoPdbName,
    DsymUnreadable {
        bundle: PathBuf,
        source: PathIoError,
    },
    DwarfCount {
        bundle: PathBuf,
        found: usize,
    },
    DwarfOutsideBundle {
        dwarf: PathBuf,
        bundle: PathBuf,
        source: std::path::StripPrefixError,
    },
    Io(PathIoError),
}

fn article(format: &str) -> &'static str {
    if format.starts_with('E') { "an" } else { "a" }
}

impl fmt::Display for SymbolsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFormat { what, format } => {
                write!(f, "{what} is not {} {format} file", article(format))
            }
            Self::Unreadable {
                what,
                format,
                source,
            } => write!(f, "{what} is not a readable {format} file: {source}"),
            Self::Corrupt { what, source } => write!(f, "{what} is corrupt: {source}"),
            Self::BadHeader { what } => write!(f, "{what} is corrupt"),
            Self::Truncated { what } => write!(f, "{what} is truncated"),
            Self::NoBuildId { what } => write!(f, "{what} has no build-id"),
            Self::NoUuid { what } => write!(f, "{what} has no LC_UUID"),
            Self::NoCodeView => f.write_str("binary has no CodeView record"),
            Self::BuildIdMismatch { binary, debug } => {
                write!(f, "build-id mismatch: binary {binary} debug file {debug}")
            }
            Self::UuidMismatch { binary, dsym } => {
                write!(
                    f,
                    "uuid mismatch: binary {binary} dSYM {dsym} (stale dSYM?)"
                )
            }
            Self::ArchMismatch {
                binary,
                other,
                debug,
            } => write!(f, "arch mismatch: binary {binary} {other} {debug}"),
            Self::GuidMismatch { binary, pdb } => write!(
                f,
                "guid mismatch: binary {binary} pdb {pdb} (stale or wrong PDB?)"
            ),
            Self::AgeMismatch { binary, pdb } => {
                write!(f, "age mismatch: binary {binary} pdb {pdb} (stale PDB?)")
            }
            Self::MachineMismatch { binary, pdb } => write!(
                f,
                "arch mismatch: binary machine {binary:#x} pdb machine {pdb:#x}"
            ),
            Self::PdbCorrupt => f.write_str("pdb is truncated or corrupt"),
            Self::NoInfoStream => f.write_str("pdb has no info stream"),
            Self::NoDbiStream => f.write_str("pdb has no DBI stream"),
            Self::NoPdbName => f.write_str("pdb has no file name"),
            Self::DsymUnreadable { bundle, source } => {
                write!(f, "{}: cannot read dSYM: {source}", bundle.display())
            }
            Self::DwarfCount { bundle, found } => write!(
                f,
                "{}: want one DWARF file, found {found}",
                bundle.display()
            ),
            Self::DwarfOutsideBundle { dwarf, bundle, .. } => {
                write!(f, "{} is not inside {}", dwarf.display(), bundle.display())
            }
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SymbolsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable { source, .. } | Self::Corrupt { source, .. } => Some(source),
            Self::DsymUnreadable { source, .. } | Self::Io(source) => Some(source),
            Self::WrongFormat { .. }
            | Self::BadHeader { .. }
            | Self::Truncated { .. }
            | Self::NoBuildId { .. }
            | Self::NoUuid { .. }
            | Self::NoCodeView
            | Self::BuildIdMismatch { .. }
            | Self::UuidMismatch { .. }
            | Self::ArchMismatch { .. }
            | Self::GuidMismatch { .. }
            | Self::AgeMismatch { .. }
            | Self::MachineMismatch { .. }
            | Self::PdbCorrupt
            | Self::NoInfoStream
            | Self::NoDbiStream
            | Self::NoPdbName
            | Self::DwarfCount { .. } => None,
            Self::DwarfOutsideBundle { source, .. } => Some(source),
        }
    }
}

impl From<PathIoError> for SymbolsError {
    fn from(error: PathIoError) -> Self {
        Self::Io(error)
    }
}

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

fn read(bytes: &[u8], what: &'static str) -> Result<Elf, SymbolsError> {
    let not_elf = SymbolsError::WrongFormat {
        what,
        format: "ELF",
    };
    let file = match object::File::parse(bytes) {
        Ok(file) => file,
        Err(source) if bytes.starts_with(b"\x7fELF") => {
            return Err(SymbolsError::Corrupt { what, source });
        }
        Err(_) => return Err(not_elf),
    };
    if file.format() != object::BinaryFormat::Elf {
        return Err(not_elf);
    }
    let build_id = file
        .build_id()
        .map_err(|source| SymbolsError::Corrupt { what, source })?
        .ok_or(SymbolsError::NoBuildId { what })?
        .to_vec();
    let raw: [u8; 2] = bytes
        .get(18..20)
        .and_then(|s| s.try_into().ok())
        .ok_or(SymbolsError::BadHeader { what })?;
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

/// Returns the shared build-id and arch, or says why the two files do not match.
///
/// # Errors
///
/// Returns an error when the files cannot be parsed or their build ids or architectures differ.
pub fn check_pair(binary: &[u8], debug: &[u8]) -> Result<Pair, SymbolsError> {
    let bin = read(binary, "binary")?;
    let dbg = read(debug, "debug file")?;
    if bin.build_id != dbg.build_id {
        return Err(SymbolsError::BuildIdMismatch {
            binary: hex::encode(&bin.build_id),
            debug: hex::encode(&dbg.build_id),
        });
    }
    if bin.machine != dbg.machine || bin.arch != dbg.arch {
        return Err(SymbolsError::ArchMismatch {
            binary: format!("{:?}", bin.arch),
            other: "debug file",
            debug: format!("{:?}", dbg.arch),
        });
    }
    Ok(Pair {
        build_id: hex::encode(&bin.build_id),
        arch: format!("{:?}", bin.arch).to_lowercase(),
    })
}

/// The UUID and arch shared by a Mach-O binary and its dSYM.
#[derive(Debug, PartialEq, Eq)]
pub struct DsymPair {
    pub uuid: String,
    pub arch: String,
}

fn macho_uuid(bytes: &[u8], what: &'static str) -> Result<([u8; 16], String), SymbolsError> {
    let format = "Mach-O";
    let file = object::File::parse(bytes).map_err(|source| SymbolsError::Unreadable {
        what,
        format,
        source,
    })?;
    if file.format() != object::BinaryFormat::MachO {
        return Err(SymbolsError::WrongFormat { what, format });
    }
    let uuid = file
        .mach_uuid()
        .map_err(|source| SymbolsError::Corrupt { what, source })?
        .ok_or(SymbolsError::NoUuid { what })?;
    Ok((uuid, format!("{:?}", file.architecture()).to_lowercase()))
}

/// Returns the shared UUID and arch, or says why the binary and dSYM do not match.
///
/// # Errors
///
/// Returns an error when either file is not a Mach-O file or the UUIDs or architectures differ.
pub fn check_dsym(binary: &[u8], dwarf: &[u8]) -> Result<DsymPair, SymbolsError> {
    let (bin_uuid, bin_arch) = macho_uuid(binary, "binary")?;
    let (dsym_uuid, dsym_arch) = macho_uuid(dwarf, "dSYM")?;
    if bin_uuid != dsym_uuid {
        return Err(SymbolsError::UuidMismatch {
            binary: hex::encode(bin_uuid),
            dsym: hex::encode(dsym_uuid),
        });
    }
    if bin_arch != dsym_arch {
        return Err(SymbolsError::ArchMismatch {
            binary: bin_arch,
            other: "dSYM",
            debug: dsym_arch,
        });
    }
    Ok(DsymPair {
        uuid: hex::encode(bin_uuid),
        arch: bin_arch,
    })
}

/// The one DWARF file inside a dSYM bundle, at Contents/Resources/DWARF/.
///
/// # Errors
///
/// Returns an error when the bundle has no DWARF file or more than one.
pub fn dwarf_file(bundle: &Path) -> Result<PathBuf, SymbolsError> {
    let dir = bundle.join("Contents/Resources/DWARF");
    let files = path_io::read_dir(&dir).map_err(|source| SymbolsError::DsymUnreadable {
        bundle: bundle.to_path_buf(),
        source,
    })?;
    match <[PathBuf; 1]>::try_from(files) {
        Ok([file]) => Ok(file),
        Err(files) => Err(SymbolsError::DwarfCount {
            bundle: bundle.to_path_buf(),
            found: files.len(),
        }),
    }
}

/// The manifest for a binary and its dSYM bundle: UUID, arch, binary digest and the dsymutil version.
///
/// # Errors
///
/// Returns an error when a file cannot be read or the pair does not match.
pub fn dsym_manifest(
    binary: &Path,
    bundle: &Path,
    tool: &str,
) -> Result<serde_json::Value, SymbolsError> {
    let bin = path_io::read(binary)?;
    let dwarf = dwarf_file(bundle)?;
    let dbg = path_io::read(&dwarf)?;
    let pair = check_dsym(&bin, &dbg)?;
    let rel = dwarf
        .strip_prefix(bundle)
        .map_err(|source| SymbolsError::DwarfOutsideBundle {
            dwarf: dwarf.clone(),
            bundle: bundle.to_path_buf(),
            source,
        })?;
    Ok(serde_json::json!({
        "uuid": pair.uuid,
        "arch": pair.arch,
        "sha256": dx_digest::sha256_hex(&bin),
        "dwarf": rel.to_string_lossy().replace('\\', "/"),
        "dsymutil": tool,
    }))
}

/// What a PE file says about its PDB: the `CodeView` record and the machine.
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

/// Reads the `CodeView` GUID, age and path, the machine and the linker version of a PE file.
///
/// # Errors
///
/// Returns an error when the bytes are not a PE file with a `CodeView` record.
pub fn pe_codeview(bytes: &[u8]) -> Result<PeId, SymbolsError> {
    let (what, format) = ("binary", "PE");
    let file = object::File::parse(bytes).map_err(|source| SymbolsError::Unreadable {
        what,
        format,
        source,
    })?;
    if file.format() != object::BinaryFormat::Pe {
        return Err(SymbolsError::WrongFormat { what, format });
    }
    let cv = file
        .pdb_info()
        .map_err(|source| SymbolsError::Corrupt { what, source })?
        .ok_or(SymbolsError::NoCodeView)?;
    let truncated = SymbolsError::Truncated { what };
    let Some(nt) = u32_at(bytes, 0x3c) else {
        return Err(truncated);
    };
    let nt = nt as usize;
    let (Some(machine), Some(&[major, minor])) =
        (u16_at(bytes, nt + 4), bytes.get(nt + 26..nt + 28))
    else {
        return Err(truncated);
    };
    Ok(PeId {
        guid: cv.guid(),
        age: cv.age(),
        path: String::from_utf8_lossy(cv.path()).into_owned(),
        machine,
        linker_version: format!("{major}.{minor}"),
    })
}

const MSF_MAGIC: &[u8] = b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0";

fn msf_streams(bytes: &[u8]) -> Result<Vec<Vec<u8>>, SymbolsError> {
    if !bytes.starts_with(MSF_MAGIC) {
        return Err(SymbolsError::WrongFormat {
            what: "pdb",
            format: "PDB",
        });
    }
    let corrupt = || SymbolsError::PdbCorrupt;
    let field = |at: usize| u32_at(bytes, at).ok_or_else(corrupt);
    let block = field(32)? as usize;
    let dir_bytes = field(44)? as usize;
    let map_block = field(52)? as usize;
    if block == 0 || !block.is_power_of_two() || !bytes.len().is_multiple_of(block) {
        return Err(corrupt());
    }
    let read_blocks = |list: &[u32], len: usize| -> Result<Vec<u8>, SymbolsError> {
        let mut out = Vec::with_capacity(len);
        for &n in list {
            let start = (n as usize).checked_mul(block).ok_or_else(corrupt)?;
            out.extend_from_slice(bytes.get(start..start + block).ok_or_else(corrupt)?);
        }
        out.truncate(len);
        Ok(out)
    };
    let numbers = |data: &[u8], count: usize| -> Result<Vec<u32>, SymbolsError> {
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
///
/// # Errors
///
/// Returns an error when the bytes are not a valid MSF file.
pub fn pdb_identity(bytes: &[u8]) -> Result<PdbId, SymbolsError> {
    let streams = msf_streams(bytes)?;
    let info = streams.get(1).filter(|s| s.len() >= 28);
    let info = info.ok_or(SymbolsError::NoInfoStream)?;
    let dbi = streams.get(3).filter(|s| s.len() >= 64);
    let dbi = dbi.ok_or(SymbolsError::NoDbiStream)?;
    Ok(PdbId {
        guid: info
            .get(12..28)
            .and_then(|s| s.try_into().ok())
            .ok_or(SymbolsError::NoInfoStream)?,
        age: u32_at(dbi, 8).ok_or(SymbolsError::NoDbiStream)?,
        machine: u16_at(dbi, 58).ok_or(SymbolsError::NoDbiStream)?,
    })
}

/// Returns the shared GUID, age and arch, or says why the PE file and PDB do not match.
///
/// # Errors
///
/// Returns an error when the GUIDs, ages or machines differ.
pub fn compare_pdb(pe: &PeId, pdb: &PdbId) -> Result<PdbPair, SymbolsError> {
    if pe.guid != pdb.guid {
        return Err(SymbolsError::GuidMismatch {
            binary: hex::encode(pe.guid),
            pdb: hex::encode(pdb.guid),
        });
    }
    if pe.age != pdb.age {
        return Err(SymbolsError::AgeMismatch {
            binary: pe.age,
            pdb: pdb.age,
        });
    }
    if pe.machine != pdb.machine {
        return Err(SymbolsError::MachineMismatch {
            binary: pe.machine,
            pdb: pdb.machine,
        });
    }
    let arch = match pe.machine {
        0x8664 => "x86_64".to_string(),
        0xaa64 => "aarch64".to_string(),
        0x14c => "i386".to_string(),
        other => format!("machine_{other:#x}"),
    };
    Ok(PdbPair {
        guid: hex::encode(pe.guid),
        age: pe.age,
        arch,
    })
}

/// Checks a PE file against its PDB.
///
/// # Errors
///
/// Returns an error when either file cannot be parsed or the pair does not match.
pub fn check_pdb(binary: &[u8], pdb: &[u8]) -> Result<PdbPair, SymbolsError> {
    compare_pdb(&pe_codeview(binary)?, &pdb_identity(pdb)?)
}

/// The manifest for a PE file and its PDB: GUID, age, arch, binary digest, PDB name, `CodeView` path and linker version.
///
/// # Errors
///
/// Returns an error when a file cannot be read or the pair does not match.
pub fn pdb_manifest(binary: &Path, pdb: &Path) -> Result<serde_json::Value, SymbolsError> {
    let bin = path_io::read(binary)?;
    let dbg = path_io::read(pdb)?;
    let pe = pe_codeview(&bin)?;
    let pair = compare_pdb(&pe, &pdb_identity(&dbg)?)?;
    let name = pdb.file_name().ok_or(SymbolsError::NoPdbName)?;
    Ok(serde_json::json!({
        "guid": pair.guid,
        "age": pair.age,
        "arch": pair.arch,
        "sha256": dx_digest::sha256_hex(&bin),
        "pdb": name.to_string_lossy(),
        "codeview_path": pe.path,
        "linker_version": pe.linker_version,
    }))
}

#[cfg(test)]
mod tests;
