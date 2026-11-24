use std::io::{self, Write};

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

    pub fn add_file(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        self.append(name, tar::EntryType::Regular, None, data)
    }

    pub fn add_dir(&mut self, name: &str) -> io::Result<()> {
        self.append(name, tar::EntryType::Directory, None, &[])
    }

    pub fn add_symlink(&mut self, name: &str, target: &str) -> io::Result<()> {
        self.append(name, tar::EntryType::Symlink, Some(target), &[])
    }

    pub fn finish(self) -> io::Result<Vec<u8>> {
        self.builder.into_inner()
    }
}

pub struct RawZip {
    writer: zip::ZipWriter<io::Cursor<Vec<u8>>>,
}

impl Default for RawZip {
    fn default() -> Self {
        RawZip::new()
    }
}

fn stored() -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .system(zip::System::Unix)
}

impl RawZip {
    #[must_use]
    pub fn new() -> Self {
        RawZip {
            writer: zip::ZipWriter::new(io::Cursor::new(Vec::new())),
        }
    }

    pub fn add_file(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        self.writer
            .start_file(name, stored())
            .map_err(io::Error::other)?;
        self.writer.write_all(data)
    }

    pub fn add_dos_file(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        self.writer
            .start_file(name, stored().system(zip::System::Dos))
            .map_err(io::Error::other)?;
        self.writer.write_all(data)
    }

    pub fn add_dir(&mut self, name: &str) -> io::Result<()> {
        self.writer
            .add_directory(name, stored())
            .map_err(io::Error::other)
    }

    pub fn add_symlink(&mut self, name: &str, target: &str) -> io::Result<()> {
        self.writer
            .add_symlink(name, target, stored())
            .map_err(io::Error::other)
    }

    pub fn finish(self) -> io::Result<Vec<u8>> {
        Ok(self.writer.finish().map_err(io::Error::other)?.into_inner())
    }
}
