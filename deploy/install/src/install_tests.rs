use super::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write as _;

const LINUX: &Cell = &trust::CELLS[0];
const WINDOWS: &Cell = &trust::CELLS[3];

struct FakeHost {
    cell: &'static Cell,
    gh_ok: bool,
    have_gh: bool,
    version: String,
    calls: RefCell<Vec<Vec<String>>>,
}

impl FakeHost {
    fn new(cell: &'static Cell, version: &str) -> Self {
        FakeHost {
            cell,
            gh_ok: true,
            have_gh: true,
            version: format!("dx {version}\n"),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl Host for FakeHost {
    fn cell(&self) -> Result<&'static Cell, String> {
        Ok(self.cell)
    }

    fn sha256(&self, path: &Path) -> io::Result<String> {
        dx_digest::sha256_file_hex(path)
    }

    fn gh_bin(&self) -> String {
        "gh".to_owned()
    }

    fn have(&self, _bin: &str) -> bool {
        self.have_gh
    }

    fn run(&self, argv: &[String]) -> Result<String, String> {
        self.calls.borrow_mut().push(argv.to_vec());
        if argv[0] == "gh" {
            return if self.gh_ok {
                Ok(String::new())
            } else {
                Err("gh: verification failed".to_owned())
            };
        }
        assert!(Path::new(&argv[0]).is_file(), "staged exe exists");
        Ok(self.version.clone())
    }

    fn on_path(&self, _dir: &Path) -> bool {
        false
    }

    fn default_dir(&self) -> Result<PathBuf, String> {
        Err("no default in tests".to_owned())
    }
}

fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("dx install γ ")
        .tempdir()
        .expect("tempdir")
}

fn script(version: &str) -> Vec<u8> {
    format!("#!/bin/sh\necho dx {version}\n").into_bytes()
}

fn archive_bytes(cell: &Cell, members: &[(&str, Vec<u8>, u32)]) -> Vec<u8> {
    if cell.suffix == ".zip" {
        let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, data, mode) in members {
            let options = zip::write::SimpleFileOptions::default().unix_permissions(*mode);
            writer.start_file(*name, options).expect("start");
            writer.write_all(data).expect("data");
        }
        return writer.finish().expect("zip").into_inner();
    }
    let mut builder = tar::Builder::new(Vec::new());
    for (name, data, mode) in members {
        let mut header = tar::Header::new_ustar();
        header.set_path(name).expect("path");
        header.set_size(data.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        builder.append(&header, data.as_slice()).expect("append");
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&builder.into_inner().expect("tar"))
        .expect("gz");
    gz.finish().expect("gz")
}

/// Writes a release download dir for `tag` and returns the archive path.
fn release(root: &Path, cell: &Cell, tag: &str, extra: &[(&str, Vec<u8>, u32)]) -> PathBuf {
    let dir = root.join(format!("download {tag}"));
    std::fs::create_dir_all(&dir).expect("download dir");
    let mut members = vec![(cell.exe, script(&tag[1..]), 0o755)];
    members.extend(extra.iter().cloned());
    let bytes = archive_bytes(cell, &members);
    let archive = dir.join(cell.archive_name());
    std::fs::write(&archive, &bytes).expect("archive");
    std::fs::write(
        dir.join(trust::MANIFEST_NAME),
        format!(
            "{}  {}\n",
            dx_digest::sha256_hex(&bytes),
            cell.archive_name()
        ),
    )
    .expect("manifest");
    std::fs::write(dir.join(trust::BUNDLE_NAME), b"{}").expect("bundle");
    archive
}

fn args(tag: &str, archive: &Path, dir: &Path) -> InstallArgs {
    InstallArgs {
        tag: tag.to_owned(),
        archive: archive.to_path_buf(),
        dir: Some(dir.to_path_buf()),
        apply: true,
    }
}

fn tree(root: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .expect("under root")
                .to_string_lossy()
                .into_owned();
            let kind = entry.file_type().expect("type");
            if kind.is_symlink() {
                let target = std::fs::read_link(&path).expect("link");
                out.insert(rel, format!("-> {}", target.display()));
            } else if kind.is_dir() {
                out.insert(rel, "dir".to_owned());
                walk(root, &path, out);
            } else {
                out.insert(
                    rel,
                    dx_digest::sha256_hex(&std::fs::read(&path).expect("read")),
                );
            }
        }
    }
    walk(root, root, &mut out);
    out
}

#[test]
fn fresh_install_then_upgrade_keeps_previous() {
    let tmp = scratch();
    let dir = tmp.path().join("data dir");
    let v1 = release(tmp.path(), LINUX, "v1.0.0", &[]);
    let host = FakeHost::new(LINUX, "1.0.0");
    let report = install_with(&args("v1.0.0", &v1, &dir), &host, DX_PAYLOAD, false).expect("v1");
    assert_eq!(report.outcome, Outcome::Fresh);
    assert!(report.applied);
    assert_eq!(
        report.checks.iter().map(|c| c.name).collect::<Vec<_>>(),
        [
            "tag",
            "cell",
            "sha256",
            "provenance",
            "lock",
            "extract",
            "version",
            "layout",
            "commit"
        ]
    );
    assert_eq!(
        host.calls.borrow()[0],
        trust::gh_verify_argv("gh", &v1, &v1.with_file_name(trust::BUNDLE_NAME), "v1.0.0")
    );
    let receipt = read_receipt(&dir.join("versions/v1.0.0")).expect("receipt");
    assert_eq!(receipt.tag, "v1.0.0");
    assert_eq!(receipt.cell, "dx-linux-x86_64");
    assert_eq!(receipt.payload, "dx");
    assert_eq!(receipt.entries.len(), 1);
    assert_eq!(
        Receipt::parse(&receipt.to_json()).expect("round trip"),
        receipt
    );
    assert_eq!(
        std::fs::read_link(dir.join("bin/dx")).expect("link"),
        Path::new("../versions/v1.0.0/dx")
    );

    let v2 = release(tmp.path(), LINUX, "v1.1.0", &[]);
    let host = FakeHost::new(LINUX, "1.1.0");
    let report = install_with(&args("v1.1.0", &v2, &dir), &host, DX_PAYLOAD, false).expect("v2");
    assert_eq!(
        report.outcome,
        Outcome::Upgraded {
            from: "v1.0.0".to_owned()
        }
    );
    assert!(dir.join("versions/v1.0.0/dx").is_file());
    assert_eq!(
        std::fs::read(dir.join("bin/dx")).expect("read"),
        script("1.1.0")
    );

    let v3 = release(tmp.path(), LINUX, "v1.2.0", &[]);
    let host = FakeHost::new(LINUX, "1.2.0");
    install_with(&args("v1.2.0", &v3, &dir), &host, DX_PAYLOAD, false).expect("v3");
    assert!(
        !dir.join("versions/v1.0.0").exists(),
        "older versions prune"
    );
    assert!(dir.join("versions/v1.1.0").is_dir());

    let host = FakeHost::new(LINUX, "1.1.0");
    let report = install_with(&args("v1.1.0", &v2, &dir), &host, DX_PAYLOAD, false).expect("back");
    assert_eq!(
        report.outcome,
        Outcome::Switched {
            from: "v1.2.0".to_owned()
        }
    );
    let before = tree(&dir);
    let report = install_with(&args("v1.1.0", &v2, &dir), &host, DX_PAYLOAD, false).expect("again");
    assert_eq!(report.outcome, Outcome::NoOp);
    assert_eq!(tree(&dir), before);
}

#[test]
fn same_tag_with_a_different_digest_is_refused() {
    let tmp = scratch();
    let dir = tmp.path().join("dir");
    let v1 = release(tmp.path(), LINUX, "v1.0.0", &[]);
    let host = FakeHost::new(LINUX, "1.0.0");
    install_with(&args("v1.0.0", &v1, &dir), &host, DX_PAYLOAD, false).expect("v1");
    let other = tmp.path().join("other");
    let rebuilt = release(&other, LINUX, "v1.0.0", &[("NOTICE", b"x".to_vec(), 0o644)]);
    let before = tree(&dir);
    let error = install_with(&args("v1.0.0", &rebuilt, &dir), &host, DX_PAYLOAD, false)
        .expect_err("digest differs");
    assert_eq!(error.check, "layout");
    assert!(error.message.contains("refusing to replace"), "{error}");
    assert_eq!(tree(&dir), before);
}

#[test]
fn failed_checks_leave_dir_identical() {
    let tmp = scratch();
    let dir = tmp.path().join("dir");
    let v1 = release(tmp.path(), LINUX, "v1.0.0", &[]);
    install_with(
        &args("v1.0.0", &v1, &dir),
        &FakeHost::new(LINUX, "1.0.0"),
        DX_PAYLOAD,
        false,
    )
    .expect("v1");
    let before = tree(&dir);
    let v2 = release(tmp.path(), LINUX, "v2.0.0", &[]);

    let mut bad_sum = std::fs::read(&v2).expect("read");
    bad_sum.push(0);
    let tampered = tmp.path().join("tampered");
    std::fs::create_dir(&tampered).expect("dir");
    for name in [trust::MANIFEST_NAME, trust::BUNDLE_NAME] {
        std::fs::copy(v2.with_file_name(name), tampered.join(name)).expect("copy");
    }
    let tampered_archive = tampered.join(LINUX.archive_name());
    std::fs::write(&tampered_archive, bad_sum).expect("tamper");

    let gh_fails = FakeHost {
        gh_ok: false,
        ..FakeHost::new(LINUX, "2.0.0")
    };
    let no_gh = FakeHost {
        have_gh: false,
        ..FakeHost::new(LINUX, "2.0.0")
    };
    let cases: Vec<(InstallArgs, FakeHost, &str)> = vec![
        (
            args("v2.0.0", &tampered_archive, &dir),
            FakeHost::new(LINUX, "2.0.0"),
            "sha256",
        ),
        (args("v2.0.0", &v2, &dir), gh_fails, "provenance"),
        (args("v2.0.0", &v2, &dir), no_gh, "provenance"),
        (
            args("v2.0.0", &v2, &dir),
            FakeHost::new(LINUX, "9.9.9"),
            "version",
        ),
        (
            args("v2.0.0", &v2, &dir),
            FakeHost::new(&trust::CELLS[1], "2.0.0"),
            "cell",
        ),
        (
            args("v2.0.0/../x", &v2, &dir),
            FakeHost::new(LINUX, "2.0.0"),
            "tag",
        ),
    ];
    for (run_args, host, check) in cases {
        let error = install_with(&run_args, &host, DX_PAYLOAD, false).expect_err(check);
        assert_eq!(error.check, check, "{error}");
        assert_eq!(tree(&dir), before, "{check} left DIR unchanged");
    }
    std::fs::remove_file(v2.with_file_name(trust::BUNDLE_NAME)).expect("rm bundle");
    let error = install_with(
        &args("v2.0.0", &v2, &dir),
        &FakeHost::new(LINUX, "2.0.0"),
        DX_PAYLOAD,
        false,
    )
    .expect_err("no bundle");
    assert!(error.message.contains(trust::BUNDLE_NAME), "{error}");
    assert_eq!(tree(&dir), before);
}

#[test]
fn dry_run_writes_nothing_under_dir() {
    let tmp = scratch();
    let dir = tmp.path().join("never created");
    let v1 = release(tmp.path(), LINUX, "v1.0.0", &[]);
    let mut run_args = args("v1.0.0", &v1, &dir);
    run_args.apply = false;
    let report =
        install_with(&run_args, &FakeHost::new(LINUX, "1.0.0"), DX_PAYLOAD, false).expect("dry");
    assert_eq!(report.outcome, Outcome::Fresh);
    assert!(!report.applied);
    assert!(!report
        .checks
        .iter()
        .any(|c| c.name == "lock" || c.name == "commit"));
    assert!(!dir.exists());
}

#[test]
fn held_lock_reports_busy() {
    let tmp = scratch();
    let dir = tmp.path().join("dir");
    let v1 = release(tmp.path(), LINUX, "v1.0.0", &[]);
    let root = ManagedRoot::new(dir.clone());
    root.ensure_dir().expect("dir");
    let _held = acquire_commit_lock(&root, Duration::ZERO).expect("hold");
    let error = install_with(
        &args("v1.0.0", &v1, &dir),
        &FakeHost::new(LINUX, "1.0.0"),
        DX_PAYLOAD,
        false,
    )
    .expect_err("busy");
    assert_eq!(error.check, "lock");
    assert!(error.message.contains("busy"), "{error}");
    assert!(!dir.join("versions").exists());
}

#[test]
fn foreign_entry_point_is_refused() {
    let tmp = scratch();
    let dir = tmp.path().join("dir");
    std::fs::create_dir_all(dir.join("bin")).expect("bin");
    std::fs::write(dir.join("bin/dx"), b"someone else's dx").expect("foreign");
    let v1 = release(tmp.path(), LINUX, "v1.0.0", &[]);
    let error = install_with(
        &args("v1.0.0", &v1, &dir),
        &FakeHost::new(LINUX, "1.0.0"),
        DX_PAYLOAD,
        false,
    )
    .expect_err("foreign");
    assert_eq!(error.check, "layout");
    assert!(!dir.join("versions").exists());
    assert_eq!(
        std::fs::read(dir.join("bin/dx")).expect("kept"),
        b"someone else's dx"
    );
}

#[test]
fn failed_swap_rolls_back() {
    let tmp = scratch();
    let dir = tmp.path().join("dir");
    let v1 = release(tmp.path(), LINUX, "v1.0.0", &[]);
    let host = FakeHost::new(LINUX, "1.0.0");
    install_with(&args("v1.0.0", &v1, &dir), &host, DX_PAYLOAD, false).expect("v1");
    std::fs::create_dir(dir.join("bin/.dx.stage")).expect("blocking stage");
    let before = tree(&dir);
    let v2 = release(tmp.path(), LINUX, "v2.0.0", &[]);
    let error = install_with(
        &args("v2.0.0", &v2, &dir),
        &FakeHost::new(LINUX, "2.0.0"),
        DX_PAYLOAD,
        false,
    )
    .expect_err("swap fails");
    assert_eq!(error.check, "commit");
    assert_eq!(tree(&dir), before);
}

#[cfg(unix)]
#[test]
fn moved_root_still_runs() {
    let tmp = scratch();
    let dir = tmp.path().join("first place");
    let v1 = release(tmp.path(), LINUX, "v1.0.0", &[]);
    install_with(
        &args("v1.0.0", &v1, &dir),
        &FakeHost::new(LINUX, "1.0.0"),
        DX_PAYLOAD,
        false,
    )
    .expect("v1");
    let moved = tmp.path().join("moved γ place");
    std::fs::rename(&dir, &moved).expect("move");
    let out = std::process::Command::new(moved.join("bin/dx"))
        .output()
        .expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "dx 1.0.0\n");
}

#[test]
fn windows_layout_swaps_files_and_drops_stale_dlls() {
    let tmp = scratch();
    let dir = tmp.path().join("dir");
    let v1 = release(
        tmp.path(),
        WINDOWS,
        "v1.0.0",
        &[
            ("vcruntime140.dll", b"crt-1".to_vec(), 0o644),
            ("old.dll", b"old".to_vec(), 0o644),
        ],
    );
    let host = FakeHost::new(WINDOWS, "1.0.0");
    install_with(&args("v1.0.0", &v1, &dir), &host, DX_PAYLOAD, true).expect("v1");
    assert_eq!(
        std::fs::read_to_string(dir.join("bin/CURRENT")).expect("current"),
        "v1.0.0\n"
    );
    assert_eq!(std::fs::read(dir.join("bin/old.dll")).expect("dll"), b"old");
    let v2 = release(
        tmp.path(),
        WINDOWS,
        "v2.0.0",
        &[("vcruntime140.dll", b"crt-2".to_vec(), 0o644)],
    );
    let host = FakeHost::new(WINDOWS, "2.0.0");
    let report = install_with(&args("v2.0.0", &v2, &dir), &host, DX_PAYLOAD, true).expect("v2");
    assert_eq!(
        report.outcome,
        Outcome::Upgraded {
            from: "v1.0.0".to_owned()
        }
    );
    assert_eq!(
        std::fs::read(dir.join("bin/dx.exe")).expect("exe"),
        script("2.0.0")
    );
    assert_eq!(
        std::fs::read(dir.join("bin/vcruntime140.dll")).expect("dll"),
        b"crt-2"
    );
    assert!(!dir.join("bin/old.dll").exists(), "stale DLL removed");
    let names: Vec<String> = std::fs::read_dir(dir.join("bin"))
        .expect("bin")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !names
            .iter()
            .any(|n| n.contains(".old-") || n.contains(".stage")),
        "{names:?}"
    );
}

#[test]
fn swap_files_rolls_back_in_reverse() {
    let tmp = scratch();
    let bin = tmp.path().join("bin");
    let version = tmp.path().join("v2");
    std::fs::create_dir_all(&bin).expect("bin");
    std::fs::create_dir_all(&version).expect("version");
    std::fs::write(bin.join("a.dll"), b"a1").expect("a1");
    std::fs::write(version.join("a.dll"), b"a2").expect("a2");
    let entry = |name: &str| dx_archive::Member {
        name: name.to_owned(),
        size: 0,
        executable: false,
    };
    let receipt = Receipt {
        payload: "dx".to_owned(),
        tag: "v2.0.0".to_owned(),
        cell: WINDOWS.name.to_owned(),
        sha256: String::new(),
        entries: vec![entry("a.dll"), entry("doc/readme.txt"), entry("a.dll")],
    };
    let error = swap_files(&bin, &version, &receipt, None, &hard_link).expect_err("second fails");
    assert!(error.contains("a.dll"), "{error}");
    assert_eq!(std::fs::read(bin.join("a.dll")).expect("restored"), b"a1");
    let names: Vec<String> = std::fs::read_dir(&bin)
        .expect("bin")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["a.dll"]);
}

#[test]
fn default_dir_follows_platform_conventions() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| std::ffi::OsString::from(v))
        }
    };
    assert_eq!(
        default_dir_from(false, &env(&[("XDG_DATA_HOME", "/x"), ("HOME", "/h")])),
        Ok(PathBuf::from("/x/dx"))
    );
    assert_eq!(
        default_dir_from(false, &env(&[("XDG_DATA_HOME", "rel"), ("HOME", "/h")])),
        Ok(PathBuf::from("/h/.local/share/dx"))
    );
    assert!(default_dir_from(false, &env(&[])).is_err());
    assert!(default_dir_from(true, &env(&[("HOME", "/h")])).is_err());
}

#[test]
fn receipt_parse_rejects_other_schemas() {
    let error = Receipt::parse(r#"{"schema":"dx-install/v2"}"#).expect_err("schema");
    assert!(error.contains("dx-install/v1"), "{error}");
    assert!(Receipt::parse("not json").is_err());
}
