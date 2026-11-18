//! Deterministic tar.gz and zip writers.
//!
//! Every member gets mtime 0 and uid/gid 0, tar output is padded to a whole 10 KiB record with ustar
//! headers whose checksum field matches python's tarfile, and the gzip header carries no name or time.

use std::collections::HashSet;
use std::io::{self, Write};

use crate::{invalid_input, link_target_error, member_name_error};

const RECORDSIZE: usize = 20 * 512;

/// Returns whether `name` fits the ustar name field, alone or split at a '/' into prefix and name.
#[must_use]
pub fn fits_ustar(name: &str) -> bool {
    name.len() <= 100
        || name
            .match_indices('/')
            .any(|(at, _)| at <= 155 && name.len() - at - 1 <= 100)
}

/// Gzips `data` at the best level with a zero mtime, no name and an unknown OS byte.
pub fn gzip(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut encoder = flate2::GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(Vec::new(), flate2::Compression::best());
    encoder.write_all(data)?;
    encoder.finish()
}

fn check_new(seen: &mut HashSet<String>, name: &str) -> io::Result<()> {
    if let Some(reason) = member_name_error(name) {
        return Err(invalid_input(reason));
    }
    if !seen.insert(name.to_owned()) {
        return Err(invalid_input(format!(
            "archive member {name:?} is added twice"
        )));
    }
    Ok(())
}

fn check_link(name: &str, target: &str) -> io::Result<()> {
    match link_target_error(name, target) {
        Some(reason) => Err(invalid_input(reason)),
        None => Ok(()),
    }
}

/// Builds a ustar image in memory; finish it as plain tar or tar.gz.
pub struct TarWriter {
    builder: tar::Builder<Vec<u8>>,
    seen: HashSet<String>,
}

impl Default for TarWriter {
    fn default() -> Self {
        TarWriter::new()
    }
}

impl TarWriter {
    #[must_use]
    pub fn new() -> Self {
        TarWriter {
            builder: tar::Builder::new(Vec::new()),
            seen: HashSet::new(),
        }
    }

    fn header(
        &mut self,
        name: &str,
        path: &str,
        kind: tar::EntryType,
        size: u64,
        mode: u32,
    ) -> io::Result<tar::Header> {
        check_new(&mut self.seen, name)?;
        if !fits_ustar(path) {
            return Err(invalid_input(format!(
                "tar member name {name:?} must be at most 100 bytes, or split at a '/' into a prefix of at most 155 bytes and a name of at most 100 bytes"
            )));
        }
        let mut header = tar::Header::new_ustar();
        header.set_entry_type(kind);
        header.set_mode(mode & 0o7777);
        header.set_uid(0);
        header.set_gid(0);
        header.set_size(size);
        header.set_mtime(0);
        header.set_path(path)?;
        Ok(header)
    }

    fn append(&mut self, mut header: tar::Header, data: &[u8]) -> io::Result<()> {
        header.set_cksum();
        let field = format!("{:06o}\0 ", header.cksum()?);
        header.as_old_mut().cksum.copy_from_slice(field.as_bytes());
        self.builder.append(&header, data)
    }

    /// Adds a regular file; only the permission bits of `mode` are kept.
    pub fn add_file(&mut self, name: &str, data: &[u8], mode: u32) -> io::Result<()> {
        let header = self.header(name, name, tar::EntryType::Regular, data.len() as u64, mode)?;
        self.append(header, data)
    }

    /// Adds a directory; `name` has no trailing '/'.
    pub fn add_dir(&mut self, name: &str, mode: u32) -> io::Result<()> {
        let header = self.header(
            name,
            &format!("{name}/"),
            tar::EntryType::Directory,
            0,
            mode,
        )?;
        self.append(header, &[])
    }

    /// Adds a symlink whose target stays inside the archive root.
    pub fn add_symlink(&mut self, name: &str, target: &str) -> io::Result<()> {
        check_link(name, target)?;
        let mut header = self.header(name, name, tar::EntryType::Symlink, 0, 0o777)?;
        header.set_link_name(target)?;
        self.append(header, &[])
    }

    /// Returns the tar image padded to a whole record.
    pub fn finish_tar(self) -> io::Result<Vec<u8>> {
        let mut image = self.builder.into_inner()?;
        image.resize(image.len().div_ceil(RECORDSIZE) * RECORDSIZE, 0);
        Ok(image)
    }

    /// Returns the gzipped tar image.
    pub fn finish(self) -> io::Result<Vec<u8>> {
        gzip(&self.finish_tar()?)
    }
}

/// Builds a zip image in memory with a fixed DOS time, unix modes on every host and Deflate level 9.
pub struct ZipWriter {
    writer: zip::ZipWriter<io::Cursor<Vec<u8>>>,
    seen: HashSet<String>,
}

impl Default for ZipWriter {
    fn default() -> Self {
        ZipWriter::new()
    }
}

fn zip_options(mode: u32) -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9))
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(mode & 0o7777)
        .system(zip::System::Unix)
        .large_file(false)
}

impl ZipWriter {
    #[must_use]
    pub fn new() -> Self {
        ZipWriter {
            writer: zip::ZipWriter::new(io::Cursor::new(Vec::new())),
            seen: HashSet::new(),
        }
    }

    /// Adds a regular file; only the permission bits of `mode` are kept.
    pub fn add_file(&mut self, name: &str, data: &[u8], mode: u32) -> io::Result<()> {
        check_new(&mut self.seen, name)?;
        self.writer
            .start_file(name, zip_options(mode))
            .map_err(io::Error::other)?;
        self.writer.write_all(data)
    }

    /// Adds a directory; `name` has no trailing '/'.
    pub fn add_dir(&mut self, name: &str, mode: u32) -> io::Result<()> {
        check_new(&mut self.seen, name)?;
        self.writer
            .add_directory(name, zip_options(mode))
            .map_err(io::Error::other)
    }

    /// Adds a symlink whose target stays inside the archive root.
    pub fn add_symlink(&mut self, name: &str, target: &str) -> io::Result<()> {
        check_link(name, target)?;
        check_new(&mut self.seen, name)?;
        self.writer
            .add_symlink(name, target, zip_options(0o777))
            .map_err(io::Error::other)
    }

    /// Returns the zip image.
    pub fn finish(self) -> io::Result<Vec<u8>> {
        Ok(self.writer.finish().map_err(io::Error::other)?.into_inner())
    }
}
