//! Reads the advisory JSON out of one downloaded snapshot archive.

use std::io::Read as _;

pub const MAX_ENTRY_BYTES: u64 = 512 * 1024 * 1024;

const EOCD_SIGNATURE: [u8; 4] = [b'P', b'K', 0x05, 0x06];
const CENTRAL_SIGNATURE: [u8; 4] = [b'P', b'K', 0x01, 0x02];
const LOCAL_SIGNATURE: [u8; 4] = [b'P', b'K', 0x03, 0x04];
const EOCD_FIXED: usize = 22;
const CENTRAL_FIXED: usize = 46;
const LOCAL_FIXED: usize = 30;
const MAX_COMMENT: usize = u16::MAX as usize;
const ZIP64: u32 = u32::MAX;
const STORED: u16 = 0;
const DEFLATED: u16 = 8;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ArchiveError {
    #[error("not a zip archive: no end-of-central-directory record")]
    NotAnArchive,
    #[error("zip central directory is unusable: {detail}")]
    BadCentralDirectory { detail: String },
    #[error("zip64 archives are not supported: {detail}")]
    Zip64 { detail: String },
    #[error("zip entry {name:?} uses unsupported compression method {method}")]
    UnsupportedMethod { name: String, method: u16 },
    #[error("zip entry {name:?} declares {size} bytes, over the {limit}-byte limit")]
    EntryTooLarge { name: String, size: u64, limit: u64 },
    #[error("zip entry {name:?} is unreadable: {detail}")]
    BadEntry { name: String, detail: String },
}

/// Every JSON entry one snapshot archive holds, in archive order.
pub fn json_entries(archive: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ArchiveError> {
    let mut out = Vec::new();
    for entry in central_entries(archive)? {
        if entry.name.ends_with(".json") {
            out.push((entry.name.clone(), entry_bytes(archive, &entry)?));
        }
    }
    Ok(out)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CentralEntry {
    name: String,
    method: u16,
    compressed: u64,
    uncompressed: u64,
    crc: u32,
    local_offset: u64,
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16, ArchiveError> {
    let raw = bytes
        .get(at..at + 2)
        .ok_or_else(|| ArchiveError::BadCentralDirectory {
            detail: format!("field at {at} runs past the end of the archive"),
        })?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, ArchiveError> {
    let raw = bytes
        .get(at..at + 4)
        .ok_or_else(|| ArchiveError::BadCentralDirectory {
            detail: format!("field at {at} runs past the end of the archive"),
        })?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn no_zip64(value: u32, field: &str) -> Result<u32, ArchiveError> {
    if value == ZIP64 {
        return Err(ArchiveError::Zip64 {
            detail: format!("{field} is a zip64 sentinel"),
        });
    }
    Ok(value)
}

fn end_of_central_directory(archive: &[u8]) -> Result<usize, ArchiveError> {
    if archive.len() < EOCD_FIXED {
        return Err(ArchiveError::NotAnArchive);
    }
    let lowest = archive.len().saturating_sub(EOCD_FIXED + MAX_COMMENT);
    let last = archive.len() - EOCD_FIXED;
    for at in (lowest..=last).rev() {
        if archive.get(at..at + 4) != Some(EOCD_SIGNATURE.as_slice()) {
            continue;
        }
        if usize::from(u16_at(archive, at + 20)?) == archive.len() - at - EOCD_FIXED {
            return Ok(at);
        }
    }
    Err(ArchiveError::NotAnArchive)
}

fn central_entries(archive: &[u8]) -> Result<Vec<CentralEntry>, ArchiveError> {
    let eocd = end_of_central_directory(archive)?;
    let disk = u16_at(archive, eocd + 4)?;
    let start_disk = u16_at(archive, eocd + 6)?;
    if disk != 0 || start_disk != 0 {
        return Err(ArchiveError::BadCentralDirectory {
            detail: format!("split archives are not supported: disk {disk}, start {start_disk}"),
        });
    }
    let count = usize::from(u16_at(archive, eocd + 10)?);
    let size = no_zip64(u32_at(archive, eocd + 12)?, "the directory size")? as usize;
    let offset = no_zip64(u32_at(archive, eocd + 16)?, "the directory offset")? as usize;
    if offset + size != eocd {
        return Err(ArchiveError::BadCentralDirectory {
            detail: format!(
                "directory covers {offset}..{}, want 0..{eocd}",
                offset + size
            ),
        });
    }
    let mut out = Vec::with_capacity(count);
    let mut at = offset;
    for index in 0..count {
        if archive.get(at..at + 4) != Some(CENTRAL_SIGNATURE.as_slice()) {
            return Err(ArchiveError::BadCentralDirectory {
                detail: format!("entry {index} at {at} has no central header"),
            });
        }
        let name_len = usize::from(u16_at(archive, at + 28)?);
        let extra_len = usize::from(u16_at(archive, at + 30)?);
        let comment_len = usize::from(u16_at(archive, at + 32)?);
        let name_at = at + CENTRAL_FIXED;
        let name = archive.get(name_at..name_at + name_len).ok_or_else(|| {
            ArchiveError::BadCentralDirectory {
                detail: format!("entry {index} name runs past the directory"),
            }
        })?;
        out.push(CentralEntry {
            name: String::from_utf8(name.to_vec()).map_err(|error| ArchiveError::BadEntry {
                name: format!("<entry {index}>"),
                detail: format!("name is not valid UTF-8: {error}"),
            })?,
            method: u16_at(archive, at + 10)?,
            crc: u32_at(archive, at + 16)?,
            compressed: u64::from(no_zip64(u32_at(archive, at + 20)?, "a compressed size")?),
            uncompressed: u64::from(no_zip64(u32_at(archive, at + 24)?, "an uncompressed size")?),
            local_offset: u64::from(no_zip64(
                u32_at(archive, at + 42)?,
                "a local header offset",
            )?),
        });
        at = name_at + name_len + extra_len + comment_len;
    }
    Ok(out)
}

fn entry_bytes(archive: &[u8], entry: &CentralEntry) -> Result<Vec<u8>, ArchiveError> {
    if entry.uncompressed > MAX_ENTRY_BYTES {
        return Err(ArchiveError::EntryTooLarge {
            name: entry.name.clone(),
            size: entry.uncompressed,
            limit: MAX_ENTRY_BYTES,
        });
    }
    let unreadable = |detail: String| ArchiveError::BadEntry {
        name: entry.name.clone(),
        detail,
    };
    let local = usize::try_from(entry.local_offset).map_err(|_| {
        unreadable("local header offset runs past the end of the archive".to_owned())
    })?;
    if archive.get(local..local + 4) != Some(LOCAL_SIGNATURE.as_slice()) {
        return Err(unreadable("no local file header".to_owned()));
    }
    let name_len = u16_at(archive, local + 26)
        .map_err(|_| unreadable("local header is truncated".to_owned()))?;
    let extra_len = u16_at(archive, local + 28)
        .map_err(|_| unreadable("local header is truncated".to_owned()))?;
    let data_at = local + LOCAL_FIXED + usize::from(name_len) + usize::from(extra_len);
    let data_end = data_at
        .checked_add(usize::try_from(entry.compressed).map_err(|_| {
            unreadable("compressed size runs past the end of the archive".to_owned())
        })?)
        .ok_or_else(|| unreadable("entry payload runs past the end of the archive".to_owned()))?;
    let compressed = archive
        .get(data_at..data_end)
        .ok_or_else(|| unreadable("entry payload runs past the end of the archive".to_owned()))?;
    let payload = match entry.method {
        STORED => compressed.to_vec(),
        DEFLATED => inflate(compressed, entry)?,
        method => {
            return Err(ArchiveError::UnsupportedMethod {
                name: entry.name.clone(),
                method,
            });
        }
    };
    if payload.len() as u64 != entry.uncompressed {
        return Err(unreadable(format!(
            "holds {} bytes, want {}",
            payload.len(),
            entry.uncompressed
        )));
    }
    let crc = crc32fast::hash(&payload);
    if crc != entry.crc {
        return Err(unreadable(format!(
            "crc32 {crc:#010x} does not match the archive value {:#010x}",
            entry.crc
        )));
    }
    Ok(payload)
}

fn inflate(compressed: &[u8], entry: &CentralEntry) -> Result<Vec<u8>, ArchiveError> {
    let unreadable = |detail: String| ArchiveError::BadEntry {
        name: entry.name.clone(),
        detail,
    };
    let limit = usize::try_from(entry.uncompressed.saturating_add(1)).unwrap_or(usize::MAX);
    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(compressed)
        .take(limit as u64)
        .read_to_end(&mut out)
        .map_err(|error| unreadable(format!("deflate stream is corrupt: {error}")))?;
    Ok(out)
}
