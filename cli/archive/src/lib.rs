#![deny(clippy::indexing_slicing, clippy::string_slice)]

//! Safe extraction of tar.gz and zip archives into a fresh directory, plus read-only zip
//! member reads ([`read_zip_member`]) and listings ([`zip_names`]).

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read, Seek, Write};
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
    #[must_use]
    pub fn from_name(name: &str) -> Option<Format> {
        if name.ends_with(".tar.gz") {
            Some(Format::TarGz)
        } else if std::path::Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            Some(Format::Zip)
        } else {
            None
        }
    }

    /// The suffix release assets of this format carry.
    #[must_use]
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
#[must_use]
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
        if segment.ends_with(['.', ' ']) {
            return Some(format!(
                "archive member name {name:?} must not end a segment with '.' or ' '"
            ));
        }
        if is_device_name(segment) {
            return Some(format!(
                "archive member name {name:?} uses a Windows device name"
            ));
        }
    }
    None
}

/// Returns why the symlink `name -> target` could resolve outside the extraction target, if it could.
///
/// Leading `..` segments may climb at most to the target root; once the path descends it may not climb again,
/// because a descended segment can itself be a symlink.
#[must_use]
pub fn link_target_error(name: &str, target: &str) -> Option<String> {
    let outside = || {
        Some(format!(
            "archive symlink {name:?} -> {target:?} points outside the extraction target"
        ))
    };
    if target.is_empty() || target.starts_with('/') || target.contains(['\\', ':', '\0']) {
        return outside();
    }
    let mut depth = name.matches('/').count();
    let mut descended = false;
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." if descended => return outside(),
            ".." => match depth.checked_sub(1) {
                Some(up) => depth = up,
                None => return outside(),
            },
            _ => descended = true,
        }
    }
    None
}

/// Windows maps these stems to devices, whatever the extension.
fn is_device_name(segment: &str) -> bool {
    let stem = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let numbered = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"));
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || matches!(numbered, Some(n) if n.len() == 1 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// What an archive entry holds.
enum Kind {
    File { mode: Option<u32> },
    Dir,
    Symlink { target: String },
    Other,
}

struct Extraction<'a> {
    dest: &'a Path,
    limits: Limits,
    entries: usize,
    seen: HashSet<String>,
    links: HashSet<String>,
    total: u64,
    members: Vec<Member>,
}

impl Extraction<'_> {
    fn add(&mut self, name: String, kind: Kind, body: &mut dyn Read) -> io::Result<()> {
        if self.entries >= self.limits.max_entries {
            return Err(invalid(format!(
                "archive has more than {} entries",
                self.limits.max_entries
            )));
        }
        self.entries += 1;
        let name = match (&kind, name.strip_suffix('/')) {
            (Kind::Dir, Some(dir)) => dir.to_owned(),
            _ => name,
        };
        if let Some(reason) = member_name_error(&name) {
            return Err(invalid(reason));
        }
        let key = name.to_lowercase();
        if self.below_link(&key) {
            return Err(invalid(format!(
                "archive member {name:?} lies below an archive symlink"
            )));
        }
        if !self.seen.insert(key.clone()) {
            return Err(invalid(format!("archive has duplicate member {name:?}")));
        }
        let path = self.dest.join(&name);
        match kind {
            Kind::File { mode } => self.write_file(name, &path, mode, body),
            Kind::Dir => std::fs::create_dir_all(&path),
            Kind::Symlink { target } => {
                if let Some(reason) = link_target_error(&name, &target) {
                    return Err(invalid(reason));
                }
                create_parent(&path)?;
                symlink(&target, &path)?;
                self.links.insert(key);
                Ok(())
            }
            Kind::Other => Err(invalid(format!(
                "archive member {name:?} is not a regular file, directory or symlink"
            ))),
        }
    }

    /// Whether a parent of the lowercased member `key` is an extracted symlink.
    fn below_link(&self, key: &str) -> bool {
        key.match_indices('/').any(|(at, _)| {
            key.get(..at)
                .is_some_and(|parent| self.links.contains(parent))
        })
    }

    fn write_file(
        &mut self,
        name: String,
        path: &Path,
        mode: Option<u32>,
        body: &mut dyn Read,
    ) -> io::Result<()> {
        create_parent(path)?;
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
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

fn create_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) => std::fs::create_dir_all(parent),
        None => Ok(()),
    }
}

#[cfg(unix)]
fn symlink(target: &str, path: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, path)
}

#[cfg(not(unix))]
fn symlink(_target: &str, path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!(
            "cannot extract the archive symlink {} on this platform",
            path.display()
        ),
    ))
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
        Err(e) if e.kind() == io::ErrorKind::NotFound => std::fs::create_dir_all(dest),
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

/// Extracts `archive` into the new or empty directory `dest` and returns its regular files.
///
/// Directories and symlinks that stay inside `dest` are created too; callers discard `dest` on error.
pub fn extract(
    archive: &Path,
    format: Format,
    dest: &Path,
    limits: Limits,
) -> io::Result<Vec<Member>> {
    let file = File::open(archive)?;
    unpack(file, &archive.display().to_string(), format, dest, limits)
}

/// Extracts an archive held in memory, like [`extract`].
pub fn extract_bytes(
    bytes: &[u8],
    format: Format,
    dest: &Path,
    limits: Limits,
) -> io::Result<Vec<Member>> {
    unpack(io::Cursor::new(bytes), "the archive", format, dest, limits)
}

fn unpack(
    mut source: impl Read + Seek,
    label: &str,
    format: Format,
    dest: &Path,
    limits: Limits,
) -> io::Result<Vec<Member>> {
    let mut head = [0u8; 4];
    let read = read_head(&mut source, &mut head)?;
    if !format.magic_matches(head.get(..read).unwrap_or_default()) {
        return Err(invalid(format!(
            "{label} does not look like a {} archive",
            format.suffix()
        )));
    }
    source.rewind()?;
    prepare_dest(dest)?;
    let mut run = Extraction {
        dest,
        limits,
        entries: 0,
        seen: HashSet::new(),
        links: HashSet::new(),
        total: 0,
        members: Vec::new(),
    };
    match format {
        Format::TarGz => extract_tar(source, &mut run)?,
        Format::Zip => extract_zip(source, &mut run)?,
    }
    Ok(run.members)
}

/// Reads one named member of a zip without extracting the rest; `None` when it is absent.
pub fn read_zip_member(archive: &Path, name: &str, max_bytes: u64) -> io::Result<Option<Vec<u8>>> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| invalid(format!("zip: {e}")))?;
    let entry = match zip.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(invalid(format!("zip: {e}"))),
    };
    let mut bytes = Vec::new();
    entry.take(max_bytes + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(invalid(format!(
            "archive member {name:?} exceeds {max_bytes} bytes"
        )));
    }
    Ok(Some(bytes))
}

/// Lists the central-directory names of a zip in index order, unmodified.
///
/// Directory names keep their trailing `/`; a name that is not UTF-8 is an error.
pub fn zip_names(source: impl Read + Seek) -> io::Result<Vec<String>> {
    let mut zip = zip::ZipArchive::new(source).map_err(|e| invalid(format!("zip: {e}")))?;
    (0..zip.len())
        .map(|index| {
            let entry = zip
                .by_index_raw(index)
                .map_err(|e| invalid(format!("zip: {e}")))?;
            let name = entry.name().map_err(|e| invalid(format!("zip: {e}")))?;
            Ok(name.into_owned())
        })
        .collect()
}

fn read_head(file: &mut impl Read, head: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while let Some(rest) = head.get_mut(filled..).filter(|rest| !rest.is_empty()) {
        let n = file.read(rest)?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

/// Longest symlink target read from a zip entry body.
const MAX_LINK_TARGET: u64 = 4096;

fn utf8(bytes: Vec<u8>, what: &str) -> io::Result<String> {
    String::from_utf8(bytes).map_err(|_| invalid(format!("archive {what} is not UTF-8")))
}

fn extract_tar(source: impl Read, run: &mut Extraction<'_>) -> io::Result<()> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(source));
    for entry in tar.entries()? {
        let mut entry = entry?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_pax_global_extensions() {
            continue;
        }
        let name = utf8(entry.path_bytes().into_owned(), "member name")?;
        let kind = if entry_type.is_file() {
            Kind::File {
                mode: Some(entry.header().mode()?),
            }
        } else if entry_type.is_dir() {
            Kind::Dir
        } else if entry_type.is_symlink() {
            let target = entry
                .link_name_bytes()
                .map(|bytes| bytes.into_owned())
                .unwrap_or_default();
            Kind::Symlink {
                target: utf8(target, "symlink target")?,
            }
        } else {
            Kind::Other
        };
        run.add(name, kind, &mut entry)?;
    }
    Ok(())
}

fn extract_zip(source: impl Read + Seek, run: &mut Extraction<'_>) -> io::Result<()> {
    let mut zip = zip::ZipArchive::new(source).map_err(|e| invalid(format!("zip: {e}")))?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|e| invalid(format!("zip: {e}")))?;
        let name = entry
            .name()
            .map_err(|e| invalid(format!("zip: {e}")))?
            .into_owned();
        let kind = if entry.is_symlink() {
            let mut target = Vec::new();
            (&mut entry)
                .take(MAX_LINK_TARGET + 1)
                .read_to_end(&mut target)?;
            if target.len() as u64 > MAX_LINK_TARGET {
                return Err(invalid(format!(
                    "archive symlink {name:?} has a target over {MAX_LINK_TARGET} bytes"
                )));
            }
            Kind::Symlink {
                target: utf8(target, "symlink target")?,
            }
        } else if entry.is_dir() {
            Kind::Dir
        } else if entry.is_file() {
            Kind::File {
                mode: entry
                    .unix_mode()
                    .filter(|_| entry.system() == zip::System::Unix),
            }
        } else {
            Kind::Other
        };
        run.add(name, kind, &mut entry)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
