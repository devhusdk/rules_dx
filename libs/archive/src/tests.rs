use super::*;
use std::path::PathBuf;

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("dx-archive-")
        .tempdir()
        .expect("tempdir")
}

fn tar_gz(path: &Path, members: &[(&str, &[u8], u32)]) {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, data, mode) in members {
        let mut header = tar::Header::new_ustar();
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        header.set_size(data.len() as u64);
        header.set_mode(*mode);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        builder.append(&header, *data).expect("append");
    }
    write_gz(path, &builder.into_inner().expect("tar"));
}

fn write_gz(path: &Path, tar: &[u8]) {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(tar).expect("gz");
    std::fs::write(path, encoder.finish().expect("gz")).expect("write");
}

fn zip_file(path: &Path, members: &[(&str, &[u8], u32)]) {
    let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    for (name, data, mode) in members {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(*mode);
        writer.start_file(*name, options).expect("start");
        writer.write_all(data).expect("data");
    }
    let bytes = writer.finish().expect("zip").into_inner();
    std::fs::write(path, bytes).expect("write");
}

const MEMBERS: &[(&str, &[u8], u32)] = &[
    ("dx", b"#!/bin/sh\necho dx\n", 0o755),
    ("NOTICE", b"notice", 0o644),
    ("lib/x.dll", b"dll", 0o644),
];

fn round_trip(format: Format, build: fn(&Path, &[(&str, &[u8], u32)])) {
    let dir = scratch();
    let archive = dir.path().join(format!("dx{}", format.suffix()));
    build(&archive, MEMBERS);
    let dest = dir.path().join("out γ dir");
    let members = extract(&archive, format, &dest, Limits::default()).expect("extract");
    assert_eq!(
        members,
        vec![
            Member {
                name: "dx".to_owned(),
                size: 18,
                executable: true
            },
            Member {
                name: "NOTICE".to_owned(),
                size: 6,
                executable: false
            },
            Member {
                name: "lib/x.dll".to_owned(),
                size: 3,
                executable: false
            },
        ]
    );
    assert_eq!(std::fs::read(dest.join("lib/x.dll")).expect("read"), b"dll");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |name: &str| {
            std::fs::metadata(dest.join(name))
                .expect("meta")
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode("dx"), 0o755);
        assert_eq!(mode("NOTICE"), 0o644);
    }
}

#[test]
fn tar_gz_round_trip_keeps_modes() {
    round_trip(Format::TarGz, tar_gz);
}

#[test]
fn zip_round_trip_keeps_modes() {
    round_trip(Format::Zip, zip_file);
}

#[test]
fn format_comes_from_suffix() {
    assert_eq!(Format::from_name("dx-linux.tar.gz"), Some(Format::TarGz));
    assert_eq!(Format::from_name("dx-windows.zip"), Some(Format::Zip));
    assert_eq!(Format::from_name("dx.tgz"), None);
}

fn expect_refused(format: Format, archive: &Path) -> String {
    let dest = archive.with_extension("out");
    extract(archive, format, &dest, Limits::default())
        .expect_err("refused")
        .to_string()
}

#[test]
fn unsafe_names_are_refused() {
    for name in ["/etc/passwd", "a/../b", "..", "a\\b", "a//b", "./a", "C:x"] {
        let dir = scratch();
        let archive = dir.path().join("bad.zip");
        zip_file(&archive, &[(name, b"x", 0o644)]);
        let error = expect_refused(Format::Zip, &archive);
        assert!(error.contains("archive member name"), "{name}: {error}");
    }
    let dir = scratch();
    let archive = dir.path().join("bad.tar.gz");
    tar_gz(&archive, &[("../escape", b"x", 0o644)]);
    assert!(expect_refused(Format::TarGz, &archive).contains("'..'"));
    assert!(!dir.path().join("escape").exists());
}

#[test]
fn duplicates_are_refused_case_insensitively() {
    let dir = scratch();
    let archive = dir.path().join("dup.tar.gz");
    tar_gz(&archive, &[("dx", b"1", 0o755), ("DX", b"2", 0o755)]);
    assert!(expect_refused(Format::TarGz, &archive).contains("duplicate"));
}

#[test]
fn non_regular_entries_are_refused() {
    let dir = scratch();
    for kind in [
        tar::EntryType::Symlink,
        tar::EntryType::Link,
        tar::EntryType::Char,
        tar::EntryType::Directory,
    ] {
        let archive = dir.path().join(format!("{kind:?}.tar.gz"));
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_ustar();
        header.set_path("entry").expect("path");
        header.set_size(0);
        header.set_entry_type(kind);
        if matches!(kind, tar::EntryType::Symlink | tar::EntryType::Link) {
            header.set_link_name("/etc/passwd").expect("link");
        }
        header.set_cksum();
        builder.append(&header, io::empty()).expect("append");
        write_gz(&archive, &builder.into_inner().expect("tar"));
        let error = expect_refused(Format::TarGz, &archive);
        assert!(error.contains("not a regular file"), "{kind:?}: {error}");
    }
    let archive = dir.path().join("link.zip");
    let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    writer
        .add_symlink("link", "/etc", zip::write::SimpleFileOptions::default())
        .expect("symlink");
    std::fs::write(&archive, writer.finish().expect("zip").into_inner()).expect("write");
    assert!(expect_refused(Format::Zip, &archive).contains("not a regular file"));
}

#[test]
fn caps_are_enforced() {
    let dir = scratch();
    let archive = dir.path().join("big.tar.gz");
    tar_gz(
        &archive,
        &[("a", &[0u8; 64], 0o644), ("b", &[0u8; 64], 0o644)],
    );
    let small = |limits: Limits, out: &str| {
        extract(&archive, Format::TarGz, &dir.path().join(out), limits)
            .expect_err("capped")
            .to_string()
    };
    let entry_cap = Limits {
        max_entry_bytes: 10,
        ..Limits::default()
    };
    assert!(small(entry_cap, "o1").contains("size cap"));
    let total_cap = Limits {
        max_total_bytes: 100,
        ..Limits::default()
    };
    assert!(small(total_cap, "o2").contains("size cap"));
    let count_cap = Limits {
        max_entries: 1,
        ..Limits::default()
    };
    assert!(small(count_cap, "o3").contains("more than 1 entries"));
}

#[test]
fn magic_must_match_the_format() {
    let dir = scratch();
    let archive = dir.path().join("dx.zip");
    tar_gz(&archive, MEMBERS);
    assert!(expect_refused(Format::Zip, &archive).contains("does not look like"));
    let empty = dir.path().join("empty.tar.gz");
    std::fs::write(&empty, b"").expect("write");
    assert!(expect_refused(Format::TarGz, &empty).contains("does not look like"));
}

#[test]
fn destination_must_be_new_or_empty() {
    let dir = scratch();
    let archive = dir.path().join("dx.tar.gz");
    tar_gz(&archive, MEMBERS);
    let dest: PathBuf = dir.path().join("dest");
    std::fs::create_dir(&dest).expect("dest");
    std::fs::write(dest.join("keep"), b"keep").expect("keep");
    let error = extract(&archive, Format::TarGz, &dest, Limits::default()).expect_err("busy");
    assert!(error.to_string().contains("not empty"), "{error}");
    let file_dest = dir.path().join("file");
    std::fs::write(&file_dest, b"").expect("file");
    let error = extract(&archive, Format::TarGz, &file_dest, Limits::default()).expect_err("file");
    assert!(error.to_string().contains("not a directory"), "{error}");
    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).expect("empty");
    extract(&archive, Format::TarGz, &empty, Limits::default()).expect("empty dir works");
}
