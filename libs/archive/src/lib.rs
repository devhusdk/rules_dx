//! Safe extraction of release archives (tar.gz and zip) into a fresh directory.
#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

/// The archive container, named by suffix and confirmed by magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    TarGz,
    Zip,
}

const GZIP_MAGIC: &[u8] = &[0x1f, 0x8b];
const ZIP_MAGIC: &[u8] = b"PK\x03\x04";
const ZIP_EMPTY_MAGIC: &[u8] = b"PK\x05\x06";

impl Format {
    /// Names the format from a file name suffix.
    pub fn from_name(name: &str) -> Option<Format> {
        if name.ends_with(".tar.gz") {
            Some(Format::TarGz)
        } else if name.ends_with(".zip") {
            Some(Format::Zip)
        } else {
            None
        }
    }

    /// The suffix release assets of this format carry.
    pub fn suffix(self) -> &'static str {
        match self {
            Format::TarGz => ".tar.gz",
            Format::Zip => ".zip",
        }
    }

    fn magic_matches(self, head: &[u8]) -> bool {
        match self {
            Format::TarGz => head.starts_with(GZIP_MAGIC),
            Format::Zip => head.starts_with(ZIP_MAGIC) || head.starts_with(ZIP_EMPTY_MAGIC),
        }
    }
}

/// Caps applied while extracting, measured on the bytes actually written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_entries: 4096,
            max_entry_bytes: 512 << 20,
            max_total_bytes: 1 << 30,
        }
    }
}

/// One extracted regular file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub size: u64,
    pub executable: bool,
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Returns why an archive member name is unsafe to extract, if it is.
pub fn member_name_error(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("archive member name must not be empty".to_owned());
    }
    if name.contains('\0') {
        return Some(format!("archive member name {name:?} contains NUL"));
    }
    if name.starts_with('/') {
        return Some(format!("archive member name {name:?} must be relative"));
    }
    if name.contains('\\') {
        return Some(format!(
            "archive member name {name:?} must use '/' separators"
        ));
    }
    if name.contains(':') {
        return Some(format!("archive member name {name:?} must not contain ':'"));
    }
    for segment in name.split('/') {
        if segment.is_empty() {
            return Some(format!(
                "archive member name {name:?} must not contain empty segments"
            ));
        }
        if segment == "." || segment == ".." {
            return Some(format!(
                "archive member name {name:?} must not contain '.' or '..'"
            ));
        }
    }
    None
}

struct Extraction<'a> {
    dest: &'a Path,
    limits: Limits,
    seen: HashSet<String>,
    total: u64,
    members: Vec<Member>,
}

impl Extraction<'_> {
    fn add(
        &mut self,
        name: String,
        regular: bool,
        mode: Option<u32>,
        body: &mut dyn Read,
    ) -> io::Result<()> {
        if self.members.len() >= self.limits.max_entries {
            return Err(invalid(format!(
                "archive has more than {} entries",
                self.limits.max_entries
            )));
        }
        if let Some(reason) = member_name_error(&name) {
            return Err(invalid(reason));
        }
        if !regular {
            return Err(invalid(format!(
                "archive member {name:?} is not a regular file"
            )));
        }
        if !self.seen.insert(name.to_lowercase()) {
            return Err(invalid(format!("archive has duplicate member {name:?}")));
        }
        let path = self.dest.join(&name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let cap = self
            .limits
            .max_entry_bytes
            .min(self.limits.max_total_bytes - self.total);
        let written = io::copy(&mut body.take(cap + 1), &mut out)?;
        if written > cap {
            return Err(invalid(format!(
                "archive member {name:?} exceeds the extraction size cap"
            )));
        }
        out.flush()?;
        self.total += written;
        let executable = mode.is_some_and(|mode| mode & 0o111 != 0);
        set_mode(&out, executable)?;
        self.members.push(Member {
            name,
            size: written,
            executable,
        });
        Ok(())
    }
}

#[cfg(unix)]
fn set_mode(file: &File, executable: bool) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = if executable { 0o755 } else { 0o644 };
    file.set_permissions(std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_file: &File, _executable: bool) -> io::Result<()> {
    Ok(())
}

fn prepare_dest(dest: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(dest) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => std::fs::create_dir(dest),
        Err(e) => Err(e),
        Ok(meta) if !meta.file_type().is_dir() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("extraction target {} is not a directory", dest.display()),
        )),
        Ok(_) => {
            if std::fs::read_dir(dest)?.next().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("extraction target {} is not empty", dest.display()),
                ));
            }
            Ok(())
        }
    }
}

/// Extracts `archive` into the new or empty directory `dest`; callers discard `dest` on error.
pub fn extract(
    archive: &Path,
    format: Format,
    dest: &Path,
    limits: Limits,
) -> io::Result<Vec<Member>> {
    let mut file = File::open(archive)?;
    let mut head = [0u8; 4];
    let read = read_head(&mut file, &mut head)?;
    if !format.magic_matches(&head[..read]) {
        return Err(invalid(format!(
            "{} does not look like a {} archive",
            archive.display(),
            format.suffix()
        )));
    }
    drop(file);
    prepare_dest(dest)?;
    let mut run = Extraction {
        dest,
        limits,
        seen: HashSet::new(),
        total: 0,
        members: Vec::new(),
    };
    match format {
        Format::TarGz => extract_tar(archive, &mut run)?,
        Format::Zip => extract_zip(archive, &mut run)?,
    }
    Ok(run.members)
}

fn read_head(file: &mut File, head: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < head.len() {
        let n = file.read(&mut head[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

fn extract_tar(archive: &Path, run: &mut Extraction<'_>) -> io::Result<()> {
    let file = File::open(archive)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in tar.entries()? {
        let mut entry = entry?;
        let name = String::from_utf8(entry.path_bytes().into_owned())
            .map_err(|_| invalid("archive member name is not UTF-8".to_owned()))?;
        let regular = entry.header().entry_type().is_file();
        let mode = if regular {
            Some(entry.header().mode()?)
        } else {
            None
        };
        run.add(name, regular, mode, &mut entry)?;
    }
    Ok(())
}

fn extract_zip(archive: &Path, run: &mut Extraction<'_>) -> io::Result<()> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| invalid(format!("zip: {e}")))?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|e| invalid(format!("zip: {e}")))?;
        let name = entry
            .name()
            .map_err(|e| invalid(format!("zip: {e}")))?
            .into_owned();
        let regular = entry.is_file() && !entry.is_symlink();
        let mode = entry
            .unix_mode()
            .filter(|_| entry.system() == zip::System::Unix);
        run.add(name, regular, mode, &mut entry)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
