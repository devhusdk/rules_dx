//! Writers for malformed archive fixtures.
//!
//! Members are written exactly as given, without name, link-target or duplicate checks.

use std::io::{self, Write};

/// Builds a ustar image whose header name bytes are copied verbatim.
pub struct RawTar {
    builder: tar::Builder<Vec<u8>>,
}

impl Default for RawTar {
    fn default() -> Self {
        RawTar::new()
    }
}

impl RawTar {
    #[must_use]
    pub fn new() -> Self {
        RawTar {
            builder: tar::Builder::new(Vec::new()),
        }
    }

    fn append(
        &mut self,
        name: &str,
        kind: tar::EntryType,
        link: Option<&str>,
        data: &[u8],
    ) -> io::Result<()> {
        let mut header = tar::Header::new_ustar();
        let field = &mut header.as_old_mut().name;
        if name.len() > field.len() {
            return Err(io::Error::other(format!(
                "tar member name {name:?} exceeds 100 bytes"
            )));
        }
        field[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(kind);
        header.set_size(data.len() as u64);
        header.set_mode(if kind == tar::EntryType::Symlink {
            0o777
        } else {
            0o644
        });
        header.set_mtime(0);
        if let Some(target) = link {
            header.set_link_name_literal(target)?;
        }
        header.set_cksum();
        self.builder.append(&header, data)
    }

    /// Adds a regular file named `name` verbatim.
    pub fn add_file(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        self.append(name, tar::EntryType::Regular, None, data)
    }

    /// Adds a symlink with `target` verbatim.
    pub fn add_symlink(&mut self, name: &str, target: &str) -> io::Result<()> {
        self.append(name, tar::EntryType::Symlink, Some(target), &[])
    }

    /// Returns the tar image.
    pub fn finish(self) -> io::Result<Vec<u8>> {
        self.builder.into_inner()
    }
}

/// Builds a zip image of stored members whose names and link targets are written verbatim.
pub struct RawZip {
    writer: zip::ZipWriter<io::Cursor<Vec<u8>>>,
}

impl Default for RawZip {
    fn default() -> Self {
        RawZip::new()
    }
}

fn stored() -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)
}

impl RawZip {
    #[must_use]
    pub fn new() -> Self {
        RawZip {
            writer: zip::ZipWriter::new(io::Cursor::new(Vec::new())),
        }
    }

    /// Adds a stored regular file.
    pub fn add_file(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        self.writer
            .start_file(name, stored())
            .map_err(io::Error::other)?;
        self.writer.write_all(data)
    }

    /// Adds a directory; `name` has no trailing '/'.
    pub fn add_dir(&mut self, name: &str) -> io::Result<()> {
        self.writer
            .add_directory(name, stored())
            .map_err(io::Error::other)
    }

    /// Adds a symlink with `target` verbatim.
    pub fn add_symlink(&mut self, name: &str, target: &str) -> io::Result<()> {
        self.writer
            .add_symlink(name, target, stored())
            .map_err(io::Error::other)
    }

    /// Returns the zip image.
    pub fn finish(self) -> io::Result<Vec<u8>> {
        Ok(self.writer.finish().map_err(io::Error::other)?.into_inner())
    }
}
