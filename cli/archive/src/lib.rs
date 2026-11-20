#![deny(clippy::indexing_slicing, clippy::string_slice)]

//! Safe extraction of tar, tar.gz and zip archives into a fresh directory, plus read-only
//! member visits with name, kind and mode ([`visit_entries`], [`visit`], [`visit_tar`],
//! [`visit_tar_entries`]), zip member reads ([`read_zip_member`]), listings ([`zip_names`]),
//! integrity checks ([`check_zip`]), and deterministic writers ([`TarWriter`], [`TarStream`],
//! [`ZipWriter`]).

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read, Seek, Write};
use std::path::Path;

mod write;
pub use write::{TarStream, TarWriter, ZipWriter, fits_ustar, gzip};

/// The archive container, named by suffix and confirmed by magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Tar,
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
        let name = name.to_ascii_lowercase();
        if name.ends_with(".tar.gz") {
            Some(Format::TarGz)
        } else if name.ends_with(".tar") {
            Some(Format::Tar)
        } else if name.ends_with(".zip") {
            Some(Format::Zip)
        } else {
            None
        }
    }

    /// The suffix release assets of this format carry.
    #[must_use]
    pub fn suffix(self) -> &'static str {
        match self {
            Format::Tar => ".tar",
            Format::TarGz => ".tar.gz",
            Format::Zip => ".zip",
        }
    }

    fn magic_matches(self, head: &[u8]) -> bool {
        match self {
            Format::Tar => true,
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

fn invalid_input(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn context(what: impl std::fmt::Display) -> impl FnOnce(io::Error) -> io::Error {
    move |e| io::Error::new(e.kind(), format!("{what}: {e}"))
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

/// An archive symlink waiting to be created.
struct Link {
    name: String,
    target: String,
}

/// Most links followed while resolving one link target, like Linux's `ELOOP` limit.
const MAX_LINK_HOPS: usize = 40;

struct Extraction<'a> {
    dest: &'a Path,
    keep: &'a dyn Fn(&str) -> bool,
    limits: Limits,
    entries: usize,
    seen: HashSet<String>,
    /// Lowercased link name to its index in `pending`.
    links: HashMap<String, usize>,
    pending: Vec<Link>,
    total: u64,
    members: Vec<Member>,
}

impl<'a> Extraction<'a> {
    fn new(dest: &'a Path, keep: &'a dyn Fn(&str) -> bool, limits: Limits) -> Self {
        Extraction {
            dest,
            keep,
            limits,
            entries: 0,
            seen: HashSet::new(),
            links: HashMap::new(),
            pending: Vec::new(),
            total: 0,
            members: Vec::new(),
        }
    }

    fn add(&mut self, name: String, kind: Kind, body: &mut dyn Read) -> io::Result<()> {
        if !(self.keep)(&name) {
            return Ok(());
        }
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
            Kind::Dir => {
                std::fs::create_dir_all(&path).map_err(context(format!("create {name:?}")))
            }
            Kind::Symlink { target } => {
                if let Some(reason) = link_target_error(&name, &target) {
                    return Err(invalid(reason));
                }
                // Created after the other members, so a Windows link knows whether its target is a directory.
                self.links.insert(key, self.pending.len());
                self.pending.push(Link { name, target });
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
                .is_some_and(|parent| self.links.contains_key(parent))
        })
    }

    /// Creates the archive symlinks in archive order, once every other member exists.
    fn create_links(&self) -> io::Result<()> {
        for link in &self.pending {
            let what = format!("create {:?}", link.name);
            let path = self.dest.join(&link.name);
            create_parent(&path).map_err(context(&what))?;
            let dir = self
                .resolve(&link.name, &link.target)
                .is_some_and(|member| self.dest.join(member).is_dir());
            symlink(&link.target, &path, dir).map_err(context(&what))?;
        }
        Ok(())
    }

    /// Follows the link `name -> target` through the archive's own links to a link-free member path.
    ///
    /// Returns `None` for a loop or a chain over [`MAX_LINK_HOPS`] links.
    fn resolve(&self, name: &str, target: &str) -> Option<String> {
        let mut parts: Vec<&str> = name.split('/').collect();
        parts.pop();
        let mut hops = 0;
        self.follow(&mut parts, target, &mut hops)?;
        Some(parts.join("/"))
    }

    fn follow<'s>(
        &'s self,
        parts: &mut Vec<&'s str>,
        target: &'s str,
        hops: &mut usize,
    ) -> Option<()> {
        for segment in target.split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    parts.pop()?;
                }
                _ => {
                    parts.push(segment);
                    let key = parts.join("/").to_lowercase();
                    if let Some(link) = self.links.get(&key).and_then(|&at| self.pending.get(at)) {
                        *hops += 1;
                        if *hops > MAX_LINK_HOPS {
                            return None;
                        }
                        parts.pop();
                        self.follow(parts, &link.target, hops)?;
                    }
                }
            }
        }
        Some(())
    }

    fn write_file(
        &mut self,
        name: String,
        path: &Path,
        mode: Option<u32>,
        body: &mut dyn Read,
    ) -> io::Result<()> {
        let what = format!("write {name:?}");
        create_parent(path).map_err(context(&what))?;
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(context(&what))?;
        let cap = self
            .limits
            .max_entry_bytes
            .min(self.limits.max_total_bytes.saturating_sub(self.total));
        let written =
            io::copy(&mut body.take(cap.saturating_add(1)), &mut out).map_err(context(&what))?;
        if written > cap {
            return Err(invalid(format!(
                "archive member {name:?} exceeds the extraction size cap"
            )));
        }
        out.flush().map_err(context(&what))?;
        self.total += written;
        let executable = mode.is_some_and(|mode| mode & 0o111 != 0);
        set_mode(&out, executable).map_err(context(&what))?;
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
fn symlink(target: &str, path: &Path, _dir: bool) -> io::Result<()> {
    std::os::unix::fs::symlink(target, path)
}

/// `CreateSymbolicLinkW` fails with this without Developer Mode or an elevated shell.
#[cfg(windows)]
const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

/// Windows needs the target kind up front: a file link to a directory does not open as one.
#[cfg(windows)]
fn symlink(target: &str, path: &Path, dir: bool) -> io::Result<()> {
    let target = target.replace('/', "\\");
    let made = if dir {
        std::os::windows::fs::symlink_dir(&target, path)
    } else {
        std::os::windows::fs::symlink_file(&target, path)
    };
    made.map_err(|e| {
        if e.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD) {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Windows may not create symlinks for this user; turn on Developer Mode (Settings > System > For developers)",
            )
        } else {
            e
        }
    })
}

#[cfg(not(any(unix, windows)))]
fn symlink(_target: &str, path: &Path, _dir: bool) -> io::Result<()> {
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
/// On Windows, symlinks need Developer Mode or an elevated shell.
pub fn extract(
    archive: &Path,
    format: Format,
    dest: &Path,
    limits: Limits,
) -> io::Result<Vec<Member>> {
    extract_matching(archive, format, dest, limits, |_| true)
}

/// Extracts only the members whose archive name satisfies `keep`, like [`extract`].
///
/// `keep` sees the raw member name, with the trailing `/` of a directory; caps count kept members only.
pub fn extract_matching(
    archive: &Path,
    format: Format,
    dest: &Path,
    limits: Limits,
    keep: impl Fn(&str) -> bool,
) -> io::Result<Vec<Member>> {
    let file = File::open(archive).map_err(context(archive.display()))?;
    unpack(
        file,
        &archive.display().to_string(),
        format,
        dest,
        limits,
        &keep,
    )
}

/// Extracts an archive held in memory, like [`extract`].
pub fn extract_bytes(
    bytes: &[u8],
    format: Format,
    dest: &Path,
    limits: Limits,
) -> io::Result<Vec<Member>> {
    unpack(
        io::Cursor::new(bytes),
        "the archive",
        format,
        dest,
        limits,
        &|_| true,
    )
}

fn unpack(
    mut source: impl Read + Seek,
    label: &str,
    format: Format,
    dest: &Path,
    limits: Limits,
    keep: &dyn Fn(&str) -> bool,
) -> io::Result<Vec<Member>> {
    check_magic(&mut source, label, format)?;
    prepare_dest(dest)?;
    let mut run = Extraction::new(dest, keep, limits);
    match format {
        Format::Tar => extract_tar(source, &mut run)?,
        Format::TarGz => {
            let mut gz = flate2::read::GzDecoder::new(source);
            extract_tar(&mut gz, &mut run)?;
            io::copy(
                &mut gz.by_ref().take(limits.max_total_bytes),
                &mut io::sink(),
            )?;
        }
        Format::Zip => extract_zip(source, &mut run)?,
    }
    run.create_links()?;
    Ok(run.members)
}

/// Confirms the magic bytes of `source` and rewinds it.
fn check_magic(source: &mut (impl Read + Seek), label: &str, format: Format) -> io::Result<()> {
    let mut head = [0u8; 4];
    let read = read_head(source, &mut head)?;
    if !format.magic_matches(head.get(..read).unwrap_or_default()) {
        return Err(invalid(format!(
            "{label} does not look like a {} archive",
            format.suffix()
        )));
    }
    source.rewind()?;
    Ok(())
}

/// What an archive entry holds, as [`visit`] reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
    HardLink,
    Other,
}

/// One archive entry as [`visit_entries`] reports it.
///
/// `mode` holds the permission bits (`& 0o7777`); it is `None` for a zip entry not made on Unix and
/// for a tar header whose mode field is not a number. `link_target` is the raw, unchecked target of a tar
/// symlink or hard link; it is `None` for every other entry and for zip entries.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
    pub name: &'a str,
    pub kind: EntryKind,
    pub mode: Option<u32>,
    pub link_target: Option<&'a str>,
}

/// Calls `f` with the name, kind and body of every entry of an archive, in archive order, without extracting.
///
/// Like [`visit_entries`] without the mode.
pub fn visit(
    source: impl Read + Seek,
    format: Format,
    limits: Limits,
    mut f: impl FnMut(&str, EntryKind, &mut dyn Read) -> io::Result<()>,
) -> io::Result<()> {
    visit_entries(source, format, limits, |entry, body| {
        f(entry.name, entry.kind, body)
    })
}

/// Calls `f` with every [`Entry`] of an archive and its body, in archive order, without extracting.
///
/// Names lose a leading `./`, a bare `.` directory is skipped, directory names lose their trailing `/`, the
/// result passes [`member_name_error`], and repeated names are allowed.
/// Tar pax global headers are skipped. `limits` caps the entries and the body bytes `f` reads; a body read
/// past a cap fails. A tar.gz is read to its gzip trailer, so a cut or corrupt trailer fails.
pub fn visit_entries(
    mut source: impl Read + Seek,
    format: Format,
    limits: Limits,
    f: impl FnMut(&Entry, &mut dyn Read) -> io::Result<()>,
) -> io::Result<()> {
    check_magic(&mut source, "the archive", format)?;
    let mut run = Visit {
        limits,
        entries: 0,
        total: 0,
        f,
    };
    match format {
        Format::Tar => walk_tar(source, &mut run),
        Format::TarGz => {
            let mut gz = flate2::read::GzDecoder::new(source);
            walk_tar(&mut gz, &mut run)?;
            io::copy(
                &mut gz.by_ref().take(limits.max_total_bytes),
                &mut io::sink(),
            )?;
            Ok(())
        }
        Format::Zip => walk_zip(source, &mut run),
    }
}

/// Visits a plain tar stream like [`visit`]; for streams that cannot seek or that the caller decompressed.
pub fn visit_tar(
    source: impl Read,
    limits: Limits,
    mut f: impl FnMut(&str, EntryKind, &mut dyn Read) -> io::Result<()>,
) -> io::Result<()> {
    visit_tar_entries(source, limits, |entry, body| {
        f(entry.name, entry.kind, body)
    })
}

/// Visits a plain tar stream like [`visit_entries`]; for streams that cannot seek or that the caller decompressed.
pub fn visit_tar_entries(
    source: impl Read,
    limits: Limits,
    f: impl FnMut(&Entry, &mut dyn Read) -> io::Result<()>,
) -> io::Result<()> {
    let mut run = Visit {
        limits,
        entries: 0,
        total: 0,
        f,
    };
    walk_tar(source, &mut run)
}

/// Decompresses a gzip stream of at most `max_bytes` bytes.
pub fn gunzip(source: impl Read, max_bytes: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(source)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(invalid(format!("gzip stream exceeds {max_bytes} bytes")));
    }
    Ok(bytes)
}

struct Visit<F> {
    limits: Limits,
    entries: usize,
    total: u64,
    f: F,
}

impl<F: FnMut(&Entry, &mut dyn Read) -> io::Result<()>> Visit<F> {
    fn entry(
        &mut self,
        name: String,
        kind: EntryKind,
        mode: Option<u32>,
        link_target: Option<&str>,
        body: &mut dyn Read,
    ) -> io::Result<()> {
        if self.entries >= self.limits.max_entries {
            return Err(invalid(format!(
                "archive has more than {} entries",
                self.limits.max_entries
            )));
        }
        self.entries += 1;
        // `tar -C dir .` names every member `./…` and adds a `./` entry for the root itself.
        let name = name.strip_prefix("./").unwrap_or(&name);
        let name = match (kind, name.strip_suffix('/')) {
            (EntryKind::Dir, Some(dir)) => dir,
            _ => name,
        };
        if kind == EntryKind::Dir && (name.is_empty() || name == ".") {
            return Ok(());
        }
        let name = name.to_owned();
        if let Some(reason) = member_name_error(&name) {
            return Err(invalid(reason));
        }
        let mut capped = Capped {
            inner: body,
            left: self
                .limits
                .max_entry_bytes
                .min(self.limits.max_total_bytes.saturating_sub(self.total)),
            read: 0,
            name: &name,
        };
        let entry = Entry {
            name: &name,
            kind,
            mode,
            link_target,
        };
        (self.f)(&entry, &mut capped)?;
        self.total += capped.read;
        Ok(())
    }
}

/// A member body that fails once a read would pass `left` bytes.
struct Capped<'a> {
    inner: &'a mut dyn Read,
    left: u64,
    read: u64,
    name: &'a str,
}

impl Read for Capped<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let want = usize::try_from(self.left.saturating_add(1))
            .unwrap_or(usize::MAX)
            .min(buf.len());
        let n = self.inner.read(buf.get_mut(..want).unwrap_or_default())?;
        let n64 = n as u64;
        if n64 > self.left {
            return Err(invalid(format!(
                "archive member {:?} exceeds the read size cap",
                self.name
            )));
        }
        self.left -= n64;
        self.read += n64;
        Ok(n)
    }
}

fn walk_tar<F: FnMut(&Entry, &mut dyn Read) -> io::Result<()>>(
    source: impl Read,
    run: &mut Visit<F>,
) -> io::Result<()> {
    let mut tar = tar::Archive::new(source);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_pax_global_extensions() {
            continue;
        }
        let name = utf8(entry.path_bytes().into_owned(), "member name")?;
        let kind = if entry_type.is_file() {
            EntryKind::File
        } else if entry_type.is_dir() {
            EntryKind::Dir
        } else if entry_type.is_symlink() {
            EntryKind::Symlink
        } else if entry_type.is_hard_link() {
            EntryKind::HardLink
        } else {
            EntryKind::Other
        };
        let mode = entry.header().mode().ok().map(|mode| mode & 0o7777);
        let link_target = match kind {
            EntryKind::Symlink | EntryKind::HardLink => Some(utf8(
                entry.link_name_bytes().unwrap_or_default().into_owned(),
                "link target",
            )?),
            EntryKind::File | EntryKind::Dir | EntryKind::Other => None,
        };
        run.entry(name, kind, mode, link_target.as_deref(), &mut entry)?;
    }
    Ok(())
}

fn walk_zip<F: FnMut(&Entry, &mut dyn Read) -> io::Result<()>>(
    source: impl Read + Seek,
    run: &mut Visit<F>,
) -> io::Result<()> {
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
            EntryKind::Symlink
        } else if entry.is_dir() {
            EntryKind::Dir
        } else if entry.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        };
        let mode = entry
            .unix_mode()
            .filter(|_| entry.system() == zip::System::Unix)
            .map(|mode| mode & 0o7777);
        run.entry(name, kind, mode, None, &mut entry)?;
    }
    Ok(())
}

/// Reads one named member of a zip without extracting the rest; `None` when it is absent.
pub fn read_zip_member(archive: &Path, name: &str, max_bytes: u64) -> io::Result<Option<Vec<u8>>> {
    let file = File::open(archive).map_err(context(archive.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| invalid(format!("zip: {e}")))?;
    let entry = match zip.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(invalid(format!("zip: {e}"))),
    };
    let mut bytes = Vec::new();
    entry
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
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

/// Reads every member of a zip through its checksum via [`visit`] without writing anything.
///
/// Fails on an unsafe member name, a CRC mismatch, or a member or archive over the caps.
pub fn check_zip(source: impl Read + Seek, limits: Limits) -> io::Result<()> {
    visit(source, Format::Zip, limits, |_, _, body| {
        io::copy(body, &mut io::sink()).map(drop)
    })
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
    let mut tar = tar::Archive::new(source);
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
