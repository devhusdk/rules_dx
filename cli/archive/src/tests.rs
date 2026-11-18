use super::*;
use std::path::PathBuf;

type RawMember<'a> = (&'a str, &'a [u8], u32);
type BuildArchive = fn(&Path, &[RawMember<'_>]);

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
    ("dx", b"dx executable data", 0o755),
    ("NOTICE", b"notice", 0o644),
    ("lib/x.dll", b"dll", 0o644),
];

fn round_trip(format: Format, build: BuildArchive) {
    let dir = scratch();
    let archive = dir.path().join(format!("dx{}", format.suffix()));
    build(&archive, MEMBERS);
    let dest = dir.path().join("out γ dir");
    let members = extract(&archive, format, &dest, Limits::default()).expect("extract");
    check_members(&dest, &members);
    let bytes = std::fs::read(&archive).expect("read");
    let from_bytes = dir.path().join("nested/from bytes");
    let members = extract_bytes(&bytes, format, &from_bytes, Limits::default()).expect("bytes");
    check_members(&from_bytes, &members);
}

fn check_members(dest: &Path, members: &[Member]) {
    assert_eq!(
        members,
        [
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
fn windows_reserved_names_are_refused_everywhere() {
    for name in [
        "bin/CON.exe",
        "NUL",
        "aux.txt",
        "COM1",
        "lpt9.log",
        "CONOUT$",
        "tool.",
        "tool ",
        "a/b.",
    ] {
        assert!(member_name_error(name).is_some(), "{name} must be refused");
    }
}

#[test]
fn names_that_only_look_like_reserved_ones_are_accepted() {
    for name in ["console.exe", "bin/tool", "com10", "tool.v2"] {
        assert!(member_name_error(name).is_none(), "{name} must be accepted");
    }
}

#[test]
fn duplicates_are_refused_case_insensitively() {
    let dir = scratch();
    let archive = dir.path().join("dup.tar.gz");
    tar_gz(&archive, &[("dx", b"1", 0o755), ("DX", b"2", 0o755)]);
    assert!(expect_refused(Format::TarGz, &archive).contains("duplicate"));
}

#[test]
fn absolute_names_are_refused_in_both_formats() {
    let dir = scratch();
    let tar = dir.path().join("abs.tar.gz");
    tar_gz(&tar, &[("/dx-archive-abs", b"x", 0o644)]);
    assert!(expect_refused(Format::TarGz, &tar).contains("must be relative"));
    let zip = dir.path().join("abs.zip");
    zip_file(&zip, &[("/dx-archive-abs", b"x", 0o644)]);
    assert!(expect_refused(Format::Zip, &zip).contains("must be relative"));
}

#[test]
fn dot_dot_names_are_refused_in_both_formats() {
    for name in ["../escape", "lib/../../escape"] {
        let dir = scratch();
        let tar = dir.path().join("up.tar.gz");
        tar_gz(&tar, &[(name, b"x", 0o644)]);
        assert!(
            expect_refused(Format::TarGz, &tar).contains("'..'"),
            "{name}"
        );
        let zip = dir.path().join("up.zip");
        zip_file(&zip, &[(name, b"x", 0o644)]);
        assert!(expect_refused(Format::Zip, &zip).contains("'..'"), "{name}");
        assert!(!dir.path().join("escape").exists(), "{name}");
    }
}

/// One tar entry of any type; `link` is the symlink or hardlink target.
type TarEntry<'a> = (&'a str, tar::EntryType, &'a str, &'a [u8], u32);

fn tar_entries(path: &Path, entries: &[TarEntry<'_>]) {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, kind, link, data, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(*kind);
        header.set_size(data.len() as u64);
        header.set_mode(*mode);
        if !link.is_empty() {
            builder
                .append_link(&mut header, name, link)
                .expect("append link");
        } else {
            builder
                .append_data(&mut header, name, *data)
                .expect("append");
        }
    }
    write_gz(path, &builder.into_inner().expect("tar"));
}

enum ZipEntry<'a> {
    File(&'a str, &'a [u8], u32),
    Dir(&'a str),
    Link(&'a str, &'a str),
}

fn zip_entries(path: &Path, entries: &[ZipEntry<'_>]) {
    let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    for entry in entries {
        match entry {
            ZipEntry::File(name, data, mode) => {
                let options = zip::write::SimpleFileOptions::default().unix_permissions(*mode);
                writer.start_file(*name, options).expect("start");
                writer.write_all(data).expect("data");
            }
            ZipEntry::Dir(name) => writer
                .add_directory(*name, zip::write::SimpleFileOptions::default())
                .expect("dir"),
            ZipEntry::Link(name, target) => writer
                .add_symlink(*name, *target, zip::write::SimpleFileOptions::default())
                .expect("symlink"),
        }
    }
    std::fs::write(path, writer.finish().expect("zip").into_inner()).expect("write");
}

#[test]
fn hardlinks_and_devices_are_refused() {
    let dir = scratch();
    for (kind, link) in [
        (tar::EntryType::Link, "/etc/passwd"),
        (tar::EntryType::Link, "other"),
        (tar::EntryType::Char, ""),
        (tar::EntryType::Fifo, ""),
    ] {
        let archive = dir.path().join(format!("{kind:?}.tar.gz"));
        tar_entries(&archive, &[("entry", kind, link, b"", 0o644)]);
        let error = expect_refused(Format::TarGz, &archive);
        assert!(error.contains("not a regular file"), "{kind:?}: {error}");
    }
}

#[test]
fn directories_and_inner_symlinks_are_extracted_in_both_formats() {
    let dir = scratch();
    let tar = dir.path().join("tree.tar.gz");
    tar_entries(
        &tar,
        &[
            ("pkg/", tar::EntryType::Directory, "", b"", 0o755),
            ("pkg/bin/", tar::EntryType::Directory, "", b"", 0o755),
            (
                "pkg/lib/tool",
                tar::EntryType::Regular,
                "",
                b"tool body\n",
                0o755,
            ),
            (
                "pkg/bin/tool",
                tar::EntryType::Symlink,
                "../lib/tool",
                b"",
                0o777,
            ),
            ("pkg/current", tar::EntryType::Symlink, ".", b"", 0o777),
            ("pkg/empty/", tar::EntryType::Directory, "", b"", 0o755),
        ],
    );
    let zip = dir.path().join("tree.zip");
    zip_entries(
        &zip,
        &[
            ZipEntry::Dir("pkg/"),
            ZipEntry::Dir("pkg/bin/"),
            ZipEntry::File("pkg/lib/tool", b"tool body\n", 0o755),
            ZipEntry::Link("pkg/bin/tool", "../lib/tool"),
            ZipEntry::Link("pkg/current", "."),
            ZipEntry::Dir("pkg/empty/"),
        ],
    );
    for (format, archive) in [(Format::TarGz, &tar), (Format::Zip, &zip)] {
        let dest = dir.path().join(format!("out{}", format.suffix()));
        let members = extract(archive, format, &dest, Limits::default()).expect("extract");
        assert_eq!(
            members,
            [Member {
                name: "pkg/lib/tool".to_owned(),
                size: 10,
                executable: true,
            }],
            "{format:?}"
        );
        assert!(dest.join("pkg/empty").is_dir(), "{format:?}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let link = dest.join("pkg/bin/tool");
            assert!(
                std::fs::symlink_metadata(&link)
                    .expect("link")
                    .file_type()
                    .is_symlink(),
                "{format:?}"
            );
            assert_eq!(std::fs::read(&link).expect("read"), b"tool body\n");
            let mode = std::fs::metadata(&link).expect("meta").permissions().mode();
            assert_eq!(mode & 0o777, 0o755, "{format:?}");
            assert_eq!(
                std::fs::read(dest.join("pkg/current/lib/tool")).expect("read"),
                b"tool body\n"
            );
        }
    }
}

#[test]
fn pax_global_headers_are_skipped() {
    let dir = scratch();
    let archive = dir.path().join("git.tar.gz");
    tar_entries(
        &archive,
        &[
            (
                "pax_global_header",
                tar::EntryType::XGlobalHeader,
                "",
                b"52 comment=0123456789abcdef0123456789abcdef01234567\n",
                0o666,
            ),
            ("repo/README", tar::EntryType::Regular, "", b"hi", 0o644),
        ],
    );
    let dest = dir.path().join("out");
    let members = extract(&archive, Format::TarGz, &dest, Limits::default()).expect("extract");
    assert_eq!(members.len(), 1);
    assert!(!dest.join("pax_global_header").exists());
}

const ESCAPING_LINKS: &[(&str, &str)] = &[
    ("link", "/etc/passwd"),
    ("link", ".."),
    ("link", "../outside"),
    ("a/link", "../.."),
    ("a/b/link", "../../../outside"),
    ("link", "a/../.."),
    ("link", "a/../b"),
    ("link", "C:/Windows"),
    ("link", "..\\outside"),
    ("link", ""),
];

#[test]
fn escaping_symlink_targets_are_named() {
    for &(name, target) in ESCAPING_LINKS {
        assert!(
            link_target_error(name, target).is_some(),
            "{name} -> {target} must be refused"
        );
    }
    for (name, target) in [
        ("link", "."),
        ("link", "./a/b"),
        ("a/link", ".."),
        ("a/b/link", "../../c/d"),
        ("a/link", "../a/./b/"),
    ] {
        assert!(
            link_target_error(name, target).is_none(),
            "{name} -> {target} must be accepted"
        );
    }
}

#[test]
fn escaping_symlinks_are_refused_in_both_formats() {
    for &(name, target) in ESCAPING_LINKS {
        let dir = scratch();
        let tar = dir.path().join("link.tar.gz");
        tar_entries(&tar, &[(name, tar::EntryType::Symlink, target, b"", 0o777)]);
        let zip = dir.path().join("link.zip");
        zip_entries(&zip, &[ZipEntry::Link(name, target)]);
        for (format, archive) in [(Format::TarGz, &tar), (Format::Zip, &zip)] {
            let error = expect_refused(format, archive);
            assert!(
                error.contains("points outside"),
                "{format:?} {name} -> {target}: {error}"
            );
            assert!(
                std::fs::symlink_metadata(archive.with_extension("out").join(name)).is_err(),
                "{format:?} {name} -> {target} was created"
            );
        }
    }
}

#[test]
fn members_below_a_symlink_are_refused_in_both_formats() {
    let dir = scratch();
    let tar = dir.path().join("through.tar.gz");
    tar_entries(
        &tar,
        &[
            ("sub/", tar::EntryType::Directory, "", b"", 0o755),
            ("link", tar::EntryType::Symlink, "sub", b"", 0o777),
            ("link/file", tar::EntryType::Regular, "", b"x", 0o644),
        ],
    );
    let zip = dir.path().join("through.zip");
    zip_entries(
        &zip,
        &[
            ZipEntry::Dir("sub/"),
            ZipEntry::Link("link", "sub"),
            ZipEntry::File("link/file", b"x", 0o644),
        ],
    );
    for (format, archive) in [(Format::TarGz, &tar), (Format::Zip, &zip)] {
        let error = expect_refused(format, archive);
        assert!(
            error.contains("below an archive symlink"),
            "{format:?}: {error}"
        );
        assert!(!archive.with_extension("out").join("sub/file").exists());
    }
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
    std::fs::create_dir_all(&dest).expect("dest");
    std::fs::write(dest.join("keep"), b"keep").expect("keep");
    let error = extract(&archive, Format::TarGz, &dest, Limits::default()).expect_err("busy");
    assert!(error.to_string().contains("not empty"), "{error}");
    let file_dest = dir.path().join("file");
    std::fs::write(&file_dest, b"").expect("file");
    let error = extract(&archive, Format::TarGz, &file_dest, Limits::default()).expect_err("file");
    assert!(error.to_string().contains("not a directory"), "{error}");
    let empty = dir.path().join("empty");
    std::fs::create_dir_all(&empty).expect("empty");
    extract(&archive, Format::TarGz, &empty, Limits::default()).expect("empty dir works");
}

#[test]
fn zip_names_lists_the_central_directory_in_order() {
    let dir = scratch();
    let archive = dir.path().join("dx.zip");
    zip_entries(
        &archive,
        &[
            ZipEntry::Dir("pkg/"),
            ZipEntry::File("pkg/a.txt", b"a", 0o644),
            ZipEntry::Link("pkg/link", "a.txt"),
        ],
    );
    let names = zip_names(File::open(&archive).expect("open")).expect("names");
    assert_eq!(names, ["pkg/", "pkg/a.txt", "pkg/link"]);
    let error = zip_names(io::Cursor::new(b"not a zip")).expect_err("not a zip");
    assert!(error.to_string().starts_with("zip: "), "{error}");
}

fn written(format: Format, build: impl FnOnce(&mut dyn FnMut(&str, &[u8], u32))) -> Vec<u8> {
    match format {
        Format::Tar => {
            let mut writer = TarWriter::new();
            build(&mut |name, data, mode| writer.add_file(name, data, mode).expect("add"));
            writer.finish_tar().expect("finish")
        }
        Format::TarGz => {
            let mut writer = TarWriter::new();
            build(&mut |name, data, mode| writer.add_file(name, data, mode).expect("add"));
            writer.finish().expect("finish")
        }
        Format::Zip => {
            let mut writer = ZipWriter::new();
            build(&mut |name, data, mode| writer.add_file(name, data, mode).expect("add"));
            writer.finish().expect("finish")
        }
    }
}

fn write_members(path: &Path, format: Format, members: &[RawMember<'_>]) {
    let bytes = written(format, |add| {
        for (name, data, mode) in members {
            add(name, data, *mode);
        }
    });
    std::fs::write(path, bytes).expect("write");
}

#[test]
fn tar_writer_round_trips() {
    round_trip(Format::TarGz, |path, members| {
        write_members(path, Format::TarGz, members);
    });
}

#[test]
fn zip_writer_round_trips() {
    round_trip(Format::Zip, |path, members| {
        write_members(path, Format::Zip, members);
    });
}

const PINNED: &[RawMember<'_>] = &[
    ("bin/tool", b"#!/bin/sh\necho hi\n", 0o755),
    ("README.md", b"hello\n", 0o644),
];

fn pinned_long_name() -> String {
    format!("{}/{}", "p".repeat(120), "n".repeat(90))
}

#[test]
fn writers_keep_release_archive_digests() {
    let long = pinned_long_name();
    for (format, digest) in [
        (
            Format::TarGz,
            "9bb4f7acd2ab2a5ef51a01cd9bb05ac99e70e06606057068d6d8fbcf7cb79fa3",
        ),
        (
            Format::Zip,
            "e8852d59fca6e9956e3447451ce1f9b5bc9b096b57986ade1937a8d5691c2660",
        ),
    ] {
        let bytes = written(format, |add| {
            for (name, data, mode) in PINNED {
                add(name, data, *mode);
            }
            add(&long, b"deep\n", 0o600);
        });
        assert_eq!(dx_digest::sha256_hex(&bytes), digest, "{format:?}");
    }
}

#[test]
fn gzip_header_has_no_name_or_time() {
    let bytes = TarWriter::new().finish().expect("finish");
    let header = bytes.get(..10).expect("header");
    assert_eq!(header, [0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 2, 255]);
    assert_eq!(gzip(b"x").expect("gzip").get(..10), Some(header));
}

#[test]
fn tar_writer_pads_to_a_whole_record() {
    let mut writer = TarWriter::new();
    writer.add_file("a", b"a", 0o644).expect("add");
    assert_eq!(writer.finish_tar().expect("tar").len(), 20 * 512);
}

#[test]
fn writers_refuse_unsafe_and_duplicate_names() {
    let mut tar = TarWriter::new();
    let mut zip = ZipWriter::new();
    for name in ["../x", "/x", "a\\b", "a//b", "c:x", "a/./b"] {
        assert!(tar.add_file(name, b"", 0o644).is_err(), "{name}");
        assert!(zip.add_file(name, b"", 0o644).is_err(), "{name}");
    }
    tar.add_file("a", b"", 0o644).expect("first");
    zip.add_file("a", b"", 0o644).expect("first");
    assert!(tar.add_dir("a", 0o755).is_err());
    assert!(zip.add_dir("a", 0o755).is_err());
    assert!(tar.add_symlink("l", "../../etc").is_err());
    assert!(zip.add_symlink("l", "/etc").is_err());
    let long = format!("{}/x", "p".repeat(200));
    assert!(tar.add_file(&long, b"", 0o644).is_err());
}

#[test]
fn zip_writer_deflates_unix_members_at_the_dos_epoch() {
    let mut writer = ZipWriter::new();
    writer
        .add_file("NOTICE", b"notice\n", 0o644)
        .expect("notice");
    writer.add_file("dx.exe", b"MZ app\n", 0o755).expect("exe");
    let bytes = writer.finish().expect("zip");
    let mut archive = zip::ZipArchive::new(io::Cursor::new(bytes)).expect("open zip");
    assert_eq!(archive.len(), 2);
    for index in 0..archive.len() {
        let entry = archive.by_index(index).expect("entry");
        assert_eq!(entry.compression(), zip::CompressionMethod::Deflated);
        assert_eq!(entry.system(), zip::System::Unix);
        let stamp = entry.last_modified().expect("time");
        assert_eq!((stamp.year(), stamp.month(), stamp.day()), (1980, 1, 1));
    }
}

#[test]
fn writers_add_dirs_and_inner_symlinks() {
    let dir = scratch();
    let mut tar = TarWriter::new();
    let mut zip = ZipWriter::new();
    tar.add_dir("d", 0o755).expect("dir");
    zip.add_dir("d", 0o755).expect("dir");
    tar.add_file("d/f", b"f", 0o644).expect("file");
    zip.add_file("d/f", b"f", 0o644).expect("file");
    tar.add_symlink("l", "d/f").expect("link");
    zip.add_symlink("l", "d/f").expect("link");
    for (format, bytes) in [
        (Format::TarGz, tar.finish().expect("tar")),
        (Format::Zip, zip.finish().expect("zip")),
    ] {
        let dest = dir.path().join(format.suffix().trim_start_matches('.'));
        extract_bytes(&bytes, format, &dest, Limits::default()).expect("extract");
        assert!(dest.join("d").is_dir());
        assert_eq!(std::fs::read(dest.join("d/f")).expect("read"), b"f");
        #[cfg(unix)]
        assert_eq!(std::fs::read(dest.join("l")).expect("read link"), b"f");
    }
}

#[test]
fn tar_header_matches_python_ustar() {
    let mut writer = TarWriter::new();
    writer
        .add_file("hello", b"hello world\n", 0o755)
        .expect("add");
    let tar = writer.finish_tar().expect("tar");
    let bytes = tar.get(..512).expect("header");
    assert_eq!(bytes.get(0..6), Some(&b"hello\0"[..]));
    assert!(
        bytes
            .get(6..100)
            .expect("name")
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(bytes.get(100..108), Some(&b"0000755\0"[..]));
    assert_eq!(bytes.get(108..116), Some(&b"0000000\0"[..]));
    assert_eq!(bytes.get(116..124), Some(&b"0000000\0"[..]));
    assert_eq!(bytes.get(124..136), Some(&b"00000000014\0"[..]));
    assert_eq!(bytes.get(136..148), Some(&b"00000000000\0"[..]));
    assert_eq!(bytes.get(148..156), Some(&b"006771\0 "[..]));
    assert_eq!(bytes.get(156), Some(&b'0'));
    assert!(
        bytes
            .get(157..257)
            .expect("link")
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(bytes.get(257..265), Some(&b"ustar\x0000"[..]));
    assert!(
        bytes
            .get(265..512)
            .expect("rest")
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(tar.get(512..524), Some(&b"hello world\n"[..]));
    let mut big = TarWriter::new();
    big.add_file("f", &[b'x'; 10240], 0o644).expect("add");
    assert_eq!(big.finish_tar().expect("tar").len(), 2 * 20 * 512);
}

#[test]
fn ustar_boundaries() {
    let tail = "n".repeat(100);
    let prefix = "p".repeat(155);
    assert!(fits_ustar(&"n".repeat(100)));
    assert!(!fits_ustar(&"n".repeat(101)));
    assert!(fits_ustar(&format!("{prefix}/{tail}")));
    assert!(!fits_ustar(&format!("{prefix}p/{tail}")));
    assert!(!fits_ustar(&format!("{prefix}/{tail}n")));
    assert!(!fits_ustar(&"n".repeat(300)));
    let wide = "\u{e9}".repeat(60);
    assert!(!fits_ustar(&wide));
    assert!(fits_ustar(&format!("{wide}/x")));
    let mut writer = TarWriter::new();
    for name in ["", "a\0b", &"n".repeat(101), &"n".repeat(300)] {
        assert!(writer.add_file(name, b"", 0o644).is_err(), "{name:?}");
    }
}

fn plain_tar(members: &[(&str, &[u8], u32)]) -> Vec<u8> {
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
    builder.into_inner().expect("tar")
}

fn visit_all(format: Format, bytes: &[u8], limits: Limits) -> io::Result<Vec<(String, Vec<u8>)>> {
    let mut seen = Vec::new();
    visit(
        io::Cursor::new(bytes),
        format,
        limits,
        |name, kind, body| {
            assert_eq!(kind, EntryKind::File);
            let mut data = Vec::new();
            body.read_to_end(&mut data)?;
            seen.push((name.to_owned(), data));
            Ok(())
        },
    )?;
    Ok(seen)
}

#[test]
fn visit_reads_every_format_and_keeps_repeated_names() {
    let dir = scratch();
    let members: &[RawMember<'_>] = &[("a/x", b"one", 0o644), ("a/x", b"two", 0o644)];
    let tar = plain_tar(members);
    let expected = vec![
        ("a/x".to_owned(), b"one".to_vec()),
        ("a/x".to_owned(), b"two".to_vec()),
    ];
    assert_eq!(
        visit_all(Format::Tar, &tar, Limits::default()).expect("tar"),
        expected
    );
    let mut streamed = Vec::new();
    visit_tar(tar.as_slice(), Limits::default(), |name, _, _| {
        streamed.push(name.to_owned());
        Ok(())
    })
    .expect("stream");
    assert_eq!(streamed, ["a/x", "a/x"]);
    let gz = dir.path().join("a.tar.gz");
    tar_gz(&gz, members);
    let gz = std::fs::read(&gz).expect("read");
    assert_eq!(
        visit_all(Format::TarGz, &gz, Limits::default()).expect("gz"),
        expected
    );
    let zip = dir.path().join("a.zip");
    zip_file(&zip, &[("a/x", b"one", 0o644)]);
    let zip = std::fs::read(&zip).expect("read");
    assert_eq!(
        visit_all(Format::Zip, &zip, Limits::default()).expect("zip"),
        [("a/x".to_owned(), b"one".to_vec())]
    );
    assert_eq!(Format::from_name("01-index.tar"), Some(Format::Tar));
}

#[test]
fn visit_refuses_unsafe_names_and_oversized_bodies() {
    let tar = plain_tar(&[("../x", b"x", 0o644)]);
    let error = visit_all(Format::Tar, &tar, Limits::default()).expect_err("dot dot");
    assert!(error.to_string().contains("'..'"), "{error}");
    let tar = plain_tar(&[("x", b"12345", 0o644)]);
    let limits = Limits {
        max_entry_bytes: 4,
        ..Limits::default()
    };
    let error = visit_all(Format::Tar, &tar, limits).expect_err("oversized");
    assert!(error.to_string().contains("size cap"), "{error}");
    let tar = plain_tar(&[("x", b"12", 0o644), ("y", b"34", 0o644)]);
    let limits = Limits {
        max_total_bytes: 3,
        ..Limits::default()
    };
    assert!(visit_all(Format::Tar, &tar, limits).is_err());
    let limits = Limits {
        max_entries: 1,
        ..Limits::default()
    };
    assert!(visit_all(Format::Tar, &tar, limits).is_err());
}

#[test]
fn gunzip_caps_the_output() {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(b"12345").expect("gz");
    let gz = encoder.finish().expect("gz");
    assert_eq!(gunzip(gz.as_slice(), 5).expect("fits"), b"12345");
    assert!(gunzip(gz.as_slice(), 4).is_err());
}

fn visit_modes(format: Format, bytes: &[u8]) -> io::Result<Vec<(String, EntryKind, Option<u32>)>> {
    let mut seen = Vec::new();
    visit_entries(
        io::Cursor::new(bytes),
        format,
        Limits::default(),
        |entry, _| {
            seen.push((entry.name.to_owned(), entry.kind, entry.mode));
            Ok(())
        },
    )?;
    Ok(seen)
}

#[test]
fn visit_entries_reports_modes() {
    for format in [Format::Tar, Format::TarGz, Format::Zip] {
        let bytes = written(format, |add| {
            add("bin/run", b"#!", 0o755);
            add("doc.txt", b"text", 0o644);
        });
        assert_eq!(
            visit_modes(format, &bytes).expect("visit"),
            [
                ("bin/run".to_owned(), EntryKind::File, Some(0o755)),
                ("doc.txt".to_owned(), EntryKind::File, Some(0o644)),
            ],
            "{format:?}"
        );
    }
    let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    writer
        .start_file(
            "x",
            zip::write::SimpleFileOptions::default().system(zip::System::Dos),
        )
        .expect("start");
    let dos = writer.finish().expect("finish").into_inner();
    assert_eq!(
        visit_modes(Format::Zip, &dos).expect("visit"),
        [("x".to_owned(), EntryKind::File, None)]
    );
}

#[test]
fn tar_stream_matches_tar_writer() {
    let long = pinned_long_name();
    let block = vec![7u8; 512];
    let large = vec![9u8; 3 * 20 * 512 + 1];
    let members: Vec<RawMember<'_>> = vec![
        ("bin/tool", b"tool body\n", 0o755),
        ("empty", b"", 0o644),
        ("block", &block, 0o600),
        ("large", &large, 0o644),
        (&long, b"x", 0o644),
    ];
    let mut writer = TarWriter::new();
    let mut stream = TarStream::new(Vec::new());
    for (name, data, mode) in &members {
        writer.add_file(name, data, *mode).expect("add");
        stream
            .add_file_from(name, data.len() as u64, &mut &data[..], *mode)
            .expect("stream");
    }
    let streamed = stream.finish().expect("finish");
    assert_eq!(streamed, writer.finish_tar().expect("finish_tar"));
    assert_eq!(
        TarStream::new(Vec::new()).finish().expect("empty"),
        TarWriter::new().finish_tar().expect("empty")
    );
}

#[test]
fn visit_entries_refuses_a_cut_gzip_trailer() {
    let whole = written(Format::TarGz, |add| add("x", &[7u8; 4096], 0o644));
    assert!(visit_modes(Format::TarGz, &whole).is_ok());
    let cut = whole.get(..whole.len() - 4).expect("cut");
    assert!(visit_modes(Format::TarGz, cut).is_err());
}

#[test]
fn tar_stream_refuses_a_reader_of_the_wrong_size() {
    for (size, data) in [(5u64, &b"abc"[..]), (2, &b"abc"[..])] {
        let mut stream = TarStream::new(Vec::new());
        let error = stream
            .add_file_from("a.txt", size, &mut &data[..], 0o644)
            .expect_err("wrong size");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("exactly"), "{error}");
    }
}

#[test]
fn tar_stream_checks_member_names() {
    let mut stream = TarStream::new(Vec::new());
    stream
        .add_file_from("a.txt", 0, &mut io::empty(), 0o644)
        .expect("first");
    for name in ["a.txt", "../evil", "/abs", &"n".repeat(101)] {
        let error = stream
            .add_file_from(name, 0, &mut io::empty(), 0o644)
            .expect_err(name);
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{name}");
    }
}

/// A ustar stream whose names and link targets go into the header verbatim, so `./` prefixes survive.
fn raw_tar(entries: &[(&str, tar::EntryType, &str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, kind, link, data) in entries {
        let mut header = tar::Header::new_ustar();
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(*kind);
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.as_mut_bytes()[157..157 + link.len()].copy_from_slice(link.as_bytes());
        header.set_cksum();
        builder.append(&header, *data).expect("append");
    }
    builder.into_inner().expect("tar")
}

type Seen = (String, EntryKind, Option<String>, Vec<u8>);

fn visit_tar_all(bytes: &[u8]) -> io::Result<Vec<Seen>> {
    let mut seen = Vec::new();
    visit_tar_entries(bytes, Limits::default(), |entry, body| {
        let mut data = Vec::new();
        body.read_to_end(&mut data)?;
        seen.push((
            entry.name.to_owned(),
            entry.kind,
            entry.link_target.map(str::to_owned),
            data,
        ));
        Ok(())
    })?;
    Ok(seen)
}

#[test]
fn visit_strips_dot_slash_and_skips_the_root_dir() {
    let tar = raw_tar(&[
        ("./", tar::EntryType::Directory, "", b""),
        (".", tar::EntryType::Directory, "", b""),
        ("./blobs/", tar::EntryType::Directory, "", b""),
        ("./index.json", tar::EntryType::Regular, "", b"{}"),
    ]);
    assert_eq!(
        visit_tar_all(&tar).expect("visit"),
        [
            ("blobs".to_owned(), EntryKind::Dir, None, Vec::new()),
            (
                "index.json".to_owned(),
                EntryKind::File,
                None,
                b"{}".to_vec()
            ),
        ]
    );
    let dir = scratch();
    let gz = dir.path().join("a.tar.gz");
    write_gz(&gz, &tar);
    let gz = std::fs::read(&gz).expect("read");
    let mut names = Vec::new();
    visit(
        io::Cursor::new(gz),
        Format::TarGz,
        Limits::default(),
        |name, _, _| {
            names.push(name.to_owned());
            Ok(())
        },
    )
    .expect("gz");
    assert_eq!(names, ["blobs", "index.json"]);
    let escaping = raw_tar(&[("./../x", tar::EntryType::Regular, "", b"x")]);
    let error = visit_tar_all(&escaping).expect_err("escaping name");
    assert!(error.to_string().contains("'..'"), "{error}");
}

#[test]
fn visit_reports_link_targets() {
    let tar = raw_tar(&[
        ("a/file", tar::EntryType::Regular, "", b"x"),
        ("a/soft", tar::EntryType::Symlink, "../b/c", b""),
        ("./hard", tar::EntryType::Link, "./a/file", b""),
    ]);
    assert_eq!(
        visit_tar_all(&tar).expect("visit"),
        [
            ("a/file".to_owned(), EntryKind::File, None, b"x".to_vec()),
            (
                "a/soft".to_owned(),
                EntryKind::Symlink,
                Some("../b/c".to_owned()),
                Vec::new()
            ),
            (
                "hard".to_owned(),
                EntryKind::HardLink,
                Some("./a/file".to_owned()),
                Vec::new()
            ),
        ]
    );
}

#[test]
fn extract_matching_writes_only_the_kept_members() {
    let dir = scratch();
    let archive = dir.path().join("fat.zip");
    zip_file(
        &archive,
        &[
            ("META-INF/LICENSE", b"l", 0o644),
            ("META-INF/license/x.txt", b"x", 0o644),
            ("META-INF/semanticdb/a/A.scala.semanticdb", b"a", 0o644),
        ],
    );
    let dest = dir.path().join("out");
    let keep = |name: &str| name.starts_with("META-INF/semanticdb/");
    let members =
        extract_matching(&archive, Format::Zip, &dest, Limits::default(), keep).expect("extract");
    let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["META-INF/semanticdb/a/A.scala.semanticdb"]);
    assert!(!dest.join("META-INF/LICENSE").exists());
    let count_cap = Limits {
        max_entries: 1,
        ..Limits::default()
    };
    extract_matching(
        &archive,
        Format::Zip,
        &dir.path().join("o2"),
        count_cap,
        keep,
    )
    .expect("caps count kept members only");
}

#[test]
fn extract_matching_refuses_a_kept_dot_dot_member() {
    let dir = scratch();
    let archive = dir.path().join("up.zip");
    zip_file(&archive, &[("keep/../../escape", b"x", 0o644)]);
    let error = extract_matching(
        &archive,
        Format::Zip,
        &dir.path().join("out"),
        Limits::default(),
        |name| name.starts_with("keep/"),
    )
    .expect_err("refused");
    assert!(error.to_string().contains("'..'"), "{error}");
    assert!(!dir.path().join("escape").exists());
}

#[test]
fn check_zip_reads_every_member_under_the_caps() {
    let dir = scratch();
    let archive = dir.path().join("ok.zip");
    zip_entries(
        &archive,
        &[
            ZipEntry::Dir("pkg/"),
            ZipEntry::File("pkg/a.txt", &[7u8; 64], 0o644),
        ],
    );
    let open = || File::open(&archive).expect("open");
    check_zip(open(), Limits::default()).expect("intact");
    let entry_cap = Limits {
        max_entry_bytes: 10,
        ..Limits::default()
    };
    let error = check_zip(open(), entry_cap).expect_err("capped");
    assert!(error.to_string().contains("size cap"), "{error}");
    let count_cap = Limits {
        max_entries: 1,
        ..Limits::default()
    };
    let error = check_zip(open(), count_cap).expect_err("capped");
    assert!(error.to_string().contains("more than 1 entries"), "{error}");
}

#[test]
fn check_zip_fails_on_a_crc_mismatch() {
    let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("a.txt", stored).expect("start");
    writer.write_all(b"payload bytes").expect("write");
    let mut bytes = writer.finish().expect("finish").into_inner();
    let at = bytes
        .windows(7)
        .position(|window| window == b"payload")
        .expect("payload");
    bytes[at] ^= 0xff;
    check_zip(io::Cursor::new(bytes), Limits::default()).expect_err("corrupt");
}

#[test]
fn check_zip_refuses_a_dot_dot_member() {
    let dir = scratch();
    let archive = dir.path().join("up.zip");
    zip_file(&archive, &[("../escape", b"x", 0o644)]);
    let error =
        check_zip(File::open(&archive).expect("open"), Limits::default()).expect_err("refused");
    assert!(error.to_string().contains("'..'"), "{error}");
}
