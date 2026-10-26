use super::*;
use std::cell::RefCell;

fn put(image: &mut Vec<u8>, at: usize, bytes: &[u8]) {
    if image.len() < at + bytes.len() {
        image.resize(at + bytes.len(), 0);
    }
    image[at..at + bytes.len()].copy_from_slice(bytes);
}

/// Builds one minimal 64-bit little-endian ELF executable with DT_NEEDED entries.
fn elf(machine: u16, needed: &[&str]) -> Vec<u8> {
    let mut image = vec![0u8; 64];
    put(&mut image, 0, &[0x7f, b'E', b'L', b'F', 2, 1, 1, 0]);
    put(&mut image, 16, &2u16.to_le_bytes());
    put(&mut image, 18, &machine.to_le_bytes());
    put(&mut image, 20, &1u32.to_le_bytes());
    put(&mut image, 52, &64u16.to_le_bytes());
    put(&mut image, 54, &56u16.to_le_bytes());
    put(&mut image, 58, &64u16.to_le_bytes());
    let mut strings = vec![0u8];
    let mut offsets = Vec::new();
    for name in needed {
        offsets.push(strings.len() as u64);
        strings.extend_from_slice(name.as_bytes());
        strings.push(0);
    }
    let strings_at = 256usize;
    let dynamic_at = (strings_at + strings.len()).div_ceil(8) * 8;
    put(&mut image, strings_at, &strings);
    let mut dynamic = Vec::new();
    for offset in &offsets {
        dynamic.extend_from_slice(&1u64.to_le_bytes());
        dynamic.extend_from_slice(&offset.to_le_bytes());
    }
    dynamic.extend_from_slice(&[0u8; 16]);
    put(&mut image, dynamic_at, &dynamic);
    let headers_at = (dynamic_at + dynamic.len()).div_ceil(8) * 8;
    let sections = [
        (0u32, 0usize, 0usize, 0u32, 0u64),
        (3, strings_at, strings.len(), 0, 0),
        (6, dynamic_at, dynamic.len(), 1, 16),
    ];
    for (index, (kind, at, size, link, entsize)) in sections.into_iter().enumerate() {
        let base = headers_at + index * 64;
        put(&mut image, base + 4, &kind.to_le_bytes());
        put(&mut image, base + 24, &(at as u64).to_le_bytes());
        put(&mut image, base + 32, &(size as u64).to_le_bytes());
        put(&mut image, base + 40, &link.to_le_bytes());
        put(&mut image, base + 48, &8u64.to_le_bytes());
        put(&mut image, base + 56, &entsize.to_le_bytes());
    }
    put(&mut image, 40, &(headers_at as u64).to_le_bytes());
    put(&mut image, 60, &3u16.to_le_bytes());
    put(&mut image, 62, &1u16.to_le_bytes());
    image
}

/// Builds one thin 64-bit Mach-O executable header with only LC_BUILD_VERSION.
fn macho(cputype: u32) -> Vec<u8> {
    let mut image = Vec::new();
    for word in [
        0xfeed_facfu32,
        cputype,
        0,
        2,
        1,
        24,
        0,
        0,
        0x32,
        24,
        1,
        0x000b_0000,
        0x000b_0000,
        0,
    ] {
        image.extend_from_slice(&word.to_le_bytes());
    }
    image
}

fn fat(slices: &[Vec<u8>]) -> Vec<u8> {
    let mut image = Vec::new();
    image.extend_from_slice(&0xcafe_babeu32.to_be_bytes());
    image.extend_from_slice(&(slices.len() as u32).to_be_bytes());
    for (index, slice) in slices.iter().enumerate() {
        let cputype = u32::from_le_bytes([slice[4], slice[5], slice[6], slice[7]]);
        for word in [
            cputype,
            0,
            4096 * (index as u32 + 1),
            slice.len() as u32,
            12,
        ] {
            image.extend_from_slice(&word.to_be_bytes());
        }
    }
    for (index, slice) in slices.iter().enumerate() {
        put(&mut image, 4096 * (index + 1), slice);
    }
    image
}

/// Returns a minimal PE header for `machine`, enough for `dx_binfmt::machine`.
fn pe_head(machine: u16) -> Vec<u8> {
    let mut image = vec![0u8; 0x80];
    put(&mut image, 0, b"MZ");
    put(&mut image, 0x3c, &0x40u32.to_le_bytes());
    put(&mut image, 0x40, b"PE\0\0");
    put(&mut image, 0x44, &machine.to_le_bytes());
    image
}

const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;

/// Flips the ELF e_machine, PE Machine or thin Mach-O cputype between x86_64 and aarch64.
fn patch_machine(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let to_arm = dx_binfmt::machine(bytes).expect("machine") == "x86_64";
    if bytes.starts_with(b"\x7fELF") {
        let raw = if to_arm { EM_AARCH64 } else { EM_X86_64 };
        put(&mut out, 18, &raw.to_le_bytes());
    } else if bytes.starts_with(b"MZ") {
        let at = u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
        let raw: u16 = if to_arm { 0xaa64 } else { 0x8664 };
        put(&mut out, at + 4, &raw.to_le_bytes());
    } else {
        let raw: u32 = if to_arm { 0x0100_000c } else { 0x0100_0007 };
        put(&mut out, 4, &raw.to_le_bytes());
    }
    out
}

#[test]
fn patch_machine_flips_every_container() {
    for (image, from) in [
        (elf(EM_X86_64, &[]), "x86_64"),
        (elf(EM_AARCH64, &[]), "aarch64"),
        (pe_head(0x8664), "x86_64"),
        (pe_head(0xaa64), "aarch64"),
        (macho(0x0100_0007), "x86_64"),
        (macho(0x0100_000c), "aarch64"),
    ] {
        assert_eq!(dx_binfmt::machine(&image).as_deref(), Some(from));
        let flipped = dx_binfmt::machine(&patch_machine(&image)).expect("flipped");
        assert_ne!(flipped, from);
        assert!(MACHINES.contains(&flipped.as_str()), "{flipped}");
    }
}

struct FakeRunner {
    version: String,
    fail: Vec<&'static str>,
    calls: RefCell<Vec<String>>,
}

impl FakeRunner {
    fn new() -> Self {
        FakeRunner {
            version: "dx 1.2.3\n".to_owned(),
            fail: Vec::new(),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl Runner for FakeRunner {
    fn run(&self, program: &Path, args: &[&str], cwd: &Path) -> RunOutput {
        let dir = file_name(cwd);
        self.calls.borrow_mut().push(format!(
            "{} {} in {dir}",
            file_name(program),
            args.join(" ")
        ));
        if self.fail.contains(&args[0]) {
            return RunOutput {
                success: false,
                stdout: String::new(),
                stderr: format!("{} broke\n", args[0]),
            };
        }
        let stdout = if args[0] == "version" {
            self.version.clone()
        } else {
            "ok\n".to_owned()
        };
        RunOutput {
            success: true,
            stdout,
            stderr: String::new(),
        }
    }
}

fn synthetic_cell(format: &str) -> Cell {
    Cell {
        name: "dx-linux-x86_64".to_owned(),
        os: "linux".to_owned(),
        arch: "x86_64".to_owned(),
        machine: "x86_64".to_owned(),
        format: format.to_owned(),
        exe: "dx".to_owned(),
        runtime: vec!["libextra.so.1".to_owned()],
    }
}

fn synthetic_exe() -> Vec<u8> {
    elf(EM_X86_64, &["libc.so.6", "libm.so.6", "libextra.so.1"])
}

/// Picks the host cell from the running test binary, shipping each non-system import as runtime.
fn host_cell(format: &str) -> (Cell, Vec<u8>) {
    let bytes = std::fs::read(std::env::current_exe().expect("current_exe")).expect("read exe");
    let images = dx_binfmt::inspect(&bytes).expect("inspect");
    let image = &images[0];
    let os = [("elf", "linux"), ("macho", "macos"), ("pe", "windows")]
        .into_iter()
        .find(|(format, _)| *format == image.format.name())
        .map(|(_, os)| os)
        .expect("os");
    let runtime = image
        .needed
        .iter()
        .filter(|needed| !import_allowed(os, needed, &[]))
        .map(|needed| needed.rsplit('/').next().unwrap_or_default().to_owned())
        .collect();
    let exe = if os == "windows" { "dx.exe" } else { "dx" };
    let cell = Cell {
        name: format!("dx-{os}-{}", image.machine),
        os: os.to_owned(),
        arch: image.machine.replace("aarch64", "arm64"),
        machine: image.machine.clone(),
        format: format.to_owned(),
        exe: exe.to_owned(),
        runtime,
    };
    (cell, bytes)
}

fn identity(cell: &Cell) -> Vec<u8> {
    format!(
        "name: {}\narch: {}\nos: {}\nlinkage: static\nmin_runtime: any\n",
        cell.name, cell.machine, cell.os
    )
    .into_bytes()
}

type Members = Vec<(String, Vec<u8>, u32)>;

fn members_of(cell: &Cell, exe: Vec<u8>) -> Members {
    let mut members = vec![
        (cell.exe.clone(), exe, 0o755),
        ("NOTICE".to_owned(), b"notice\n".to_vec(), 0o644),
        ("IDENTITY.txt".to_owned(), identity(cell), 0o644),
    ];
    for name in &cell.runtime {
        members.push((name.clone(), b"runtime\n".to_vec(), 0o644));
    }
    members
}

struct Fixture {
    _dir: tempfile::TempDir,
    inputs: QualifyInputs,
}

fn stage(cell: Cell, members: Members) -> Fixture {
    let dir = tempfile::TempDir::new().expect("tempdir");
    for sub in ["out", "rebuild", "consumer"] {
        std::fs::create_dir(dir.path().join(sub)).expect("mkdir");
    }
    let archive = dir.path().join("out").join(cell.archive_name());
    let refs: Vec<(&str, &[u8], u32)> = members
        .iter()
        .map(|(name, data, mode)| (name.as_str(), data.as_slice(), *mode))
        .collect();
    dx_deploy_tools::archive_tree(&refs, &archive).expect("archive");
    let bytes = std::fs::read(&archive).expect("read archive");
    let sha = dx_digest::sha256_hex(&bytes);
    let mut sidecar = archive.clone().into_os_string();
    sidecar.push(".sha256");
    std::fs::write(&sidecar, format!("{sha}  {}\n", cell.archive_name())).expect("sidecar");
    let rebuild = dir.path().join("rebuild").join(cell.archive_name());
    std::fs::write(&rebuild, &bytes).expect("rebuild");
    let inputs = QualifyInputs {
        runner_os: cell.os.clone(),
        runner_arch: cell.machine.clone(),
        cell,
        archive,
        rebuild,
        work: dir.path().join("work"),
        sha: "a".repeat(40),
        version: "1.2.3".to_owned(),
        workflow_ref: "devhusdk/rules_dx/.github/workflows/release-cells.yml@refs/tags/v1.2.3"
            .to_owned(),
        workflow_sha: "b".repeat(40),
        run_id: "77".to_owned(),
        job: "cell (dx-linux-x86_64)".to_owned(),
        consumer: Some(dir.path().join("consumer")),
    };
    Fixture { _dir: dir, inputs }
}

fn synthetic(format: &str, tweak: impl FnOnce(&mut Members)) -> Fixture {
    let cell = synthetic_cell(format);
    let mut members = members_of(&cell, synthetic_exe());
    tweak(&mut members);
    stage(cell, members)
}

fn verdict_of(record: &CellRecord, name: &str) -> (Verdict, String) {
    let check = record
        .checks
        .iter()
        .find(|check| check.name == name)
        .expect("check");
    (check.verdict, check.detail.clone())
}

fn assert_failed(record: &CellRecord, name: &str, want: &str) {
    let (verdict, detail) = verdict_of(record, name);
    assert_eq!(verdict, Verdict::Failed, "{name}: {detail}");
    assert!(detail.contains(want), "{name}: {detail:?} lacks {want:?}");
    assert_eq!(record.result, Verdict::Failed);
}

fn check_names(record: &CellRecord) -> Vec<&str> {
    record
        .checks
        .iter()
        .map(|check| check.name.as_str())
        .collect()
}

#[test]
fn qualify_passes_the_host_binary_in_tar_gz_and_zip() {
    for format in FORMATS {
        let (cell, exe) = host_cell(format);
        let members = members_of(&cell, exe.clone());
        let fixture = stage(cell.clone(), members);
        let runner = FakeRunner::new();
        let record = qualify(&fixture.inputs, &runner);
        assert_eq!(check_names(&record), CHECKS);
        assert!(
            record
                .checks
                .iter()
                .all(|check| check.verdict == Verdict::Ok),
            "{format}: {:#?}",
            record.checks
        );
        assert_eq!(record.result, Verdict::Ok);
        assert_eq!(record.schema, RECORD_SCHEMA);
        assert_eq!(record.binary.machine, cell.machine);
        assert_eq!(record.binary.sha256, dx_digest::sha256_hex(&exe));
        assert_eq!(record.archive.name, cell.archive_name());
        assert_eq!(record.members.len(), 3 + cell.runtime.len());
        let exe_name = cell.exe.clone();
        assert_eq!(
            *runner.calls.borrow(),
            vec![
                format!("{exe_name} help in {INSTALL_DIR}"),
                format!("{exe_name} version in {INSTALL_DIR}"),
                format!("{exe_name} version in {MOVED_DIR}"),
                format!("{exe_name} build //... in consumer"),
                format!("{exe_name} test //... in consumer"),
                format!("{exe_name} check //... in consumer"),
            ]
        );
        assert!(fixture
            .inputs
            .work
            .join(MOVED_DIR)
            .join(&cell.exe)
            .is_file());
        assert!(!fixture.inputs.work.join(INSTALL_DIR).exists());
    }
}

#[test]
fn qualify_passes_a_synthetic_cell_and_round_trips_the_record() {
    for format in FORMATS {
        let fixture = synthetic(format, |_| {});
        let record = qualify(&fixture.inputs, &FakeRunner::new());
        assert_eq!(record.result, Verdict::Ok, "{:#?}", record.checks);
        assert_eq!(record.binary.format, "elf");
        assert_eq!(record.binary.abi, "static");
        assert_eq!(
            record.binary.needed,
            ["libc.so.6", "libextra.so.1", "libm.so.6"]
        );
        let text = record.render();
        assert!(text.is_ascii());
        let back: CellRecord = serde_json::from_str(&text).expect("parse");
        assert_eq!(back, record);
    }
}

#[test]
fn qualify_passes_a_gnu_binary_with_only_system_imports() {
    let mut cell = synthetic_cell("tar.gz");
    cell.runtime.clear();
    let exe = elf(
        EM_X86_64,
        &["libc.so.6", "libgcc_s.so.1", "ld-linux-x86-64.so.2"],
    );
    let members = members_of(&cell, exe);
    let record = qualify(&stage(cell, members).inputs, &FakeRunner::new());
    assert_eq!(
        verdict_of(&record, "imports").0,
        Verdict::Ok,
        "{:#?}",
        record.checks
    );
    assert_eq!(record.result, Verdict::Ok, "{:#?}", record.checks);
}

#[test]
fn qualify_rejects_a_wrong_architecture_binary() {
    let (cell, exe) = host_cell("tar.gz");
    let fixture = stage(cell.clone(), members_of(&cell, patch_machine(&exe)));
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "binary", &format!("want {}", cell.machine));
    let fixture = synthetic("tar.gz", |members| {
        members[0].1 = patch_machine(&synthetic_exe());
    });
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "binary", "machine aarch64, want x86_64");
}

#[test]
fn qualify_rejects_a_binary_for_another_os() {
    let mut cell = synthetic_cell("zip");
    cell.os = "windows".to_owned();
    let members = members_of(&cell, synthetic_exe());
    let record = qualify(&stage(cell, members).inputs, &FakeRunner::new());
    assert_failed(&record, "binary", "container elf, want pe");
    assert_failed(&record, "imports", "libc.so.6");
    let mut cell = synthetic_cell("tar.gz");
    cell.os = "macos".to_owned();
    let members = members_of(&cell, synthetic_exe());
    let record = qualify(&stage(cell, members).inputs, &FakeRunner::new());
    assert_failed(&record, "binary", "container elf, want macho");
}

#[test]
fn qualify_rejects_fat_and_unreadable_binaries() {
    let fixture = synthetic("tar.gz", |members| {
        members[0].1 = fat(&[macho(0x0100_0007), macho(0x0100_000c)]);
    });
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "binary", "want one image, found 2");
    assert_failed(&record, "imports", "no image: want one image");
    let fixture = synthetic("tar.gz", |members| members[0].1 = b"#!/bin/sh\n".to_vec());
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "binary", "unknown binary format");
    let fixture = synthetic("tar.gz", |members| members[0].0 = "other".to_owned());
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(
        &record,
        "members",
        "missing [\"dx\"], unexpected [\"other\"]",
    );
    assert_failed(&record, "binary", "cannot read dx");
}

#[test]
fn qualify_rejects_a_missing_runtime_member() {
    let fixture = synthetic("zip", |members| {
        members.retain(|member| member.0 != "libextra.so.1")
    });
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(
        &record,
        "members",
        "missing [\"libextra.so.1\"], unexpected []",
    );
    let mut cell = synthetic_cell("zip");
    cell.runtime.clear();
    let members = members_of(&cell, synthetic_exe());
    let record = qualify(&stage(cell, members).inputs, &FakeRunner::new());
    assert_eq!(verdict_of(&record, "members").0, Verdict::Ok);
    assert_failed(
        &record,
        "imports",
        "not shipped and not a system import: [\"libextra.so.1\"]",
    );
}

#[test]
fn qualify_rejects_an_extra_member_bad_modes_and_identity() {
    let fixture = synthetic("tar.gz", |members| {
        members.push(("stray.txt".to_owned(), b"x".to_vec(), 0o644));
    });
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "members", "unexpected [\"stray.txt\"]");
    let fixture = synthetic("zip", |members| {
        members[0].2 = 0o644;
        members[1].2 = 0o755;
    });
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "mode", "dx has mode 644");
    assert_failed(&record, "mode", "NOTICE has mode 755");
    assert_failed(&record, "mode", "extracted dx is not mode 755");
    let fixture = synthetic("tar.gz", |members| {
        members[2].1 = b"name: dx-other\narch: aarch64\nos: linux\n".to_vec();
    });
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "identity", "IDENTITY name is Some(\"dx-other\")");
    assert_failed(&record, "identity", "IDENTITY arch is Some(\"aarch64\")");
}

#[test]
fn qualify_rejects_format_name_digest_and_rebuild_mismatches() {
    let fixture = synthetic("zip", |_| {});
    let mut inputs = fixture.inputs.clone();
    inputs.cell.format = "tar.gz".to_owned();
    let record = qualify(&inputs, &FakeRunner::new());
    assert_failed(&record, "format", "archive is zip, want tar.gz");
    let renamed = fixture.inputs.archive.with_file_name("renamed.zip");
    std::fs::copy(&fixture.inputs.archive, &renamed).expect("copy");
    let mut inputs = fixture.inputs.clone();
    inputs.archive = renamed;
    let record = qualify(&inputs, &FakeRunner::new());
    assert_failed(&record, "format", "archive is named renamed.zip");
    assert_failed(&record, "digest", "cannot read the .sha256 sidecar");
    let fixture = synthetic("tar.gz", |_| {});
    let mut sidecar = fixture.inputs.archive.clone().into_os_string();
    sidecar.push(".sha256");
    std::fs::write(&sidecar, "0000  x\n").expect("sidecar");
    std::fs::write(&fixture.inputs.rebuild, b"other bytes").expect("rebuild");
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "digest", "sidecar lists \"0000\"");
    assert_failed(&record, "determinism", "rebuild is");
    std::fs::remove_file(&fixture.inputs.rebuild).expect("rm");
    let record = qualify(
        &QualifyInputs {
            work: fixture.inputs.work.join("again"),
            ..fixture.inputs.clone()
        },
        &FakeRunner::new(),
    );
    assert_failed(&record, "determinism", "cannot read");
}

#[test]
fn qualify_blocks_every_later_check_when_extraction_fails() {
    let fixture = synthetic("tar.gz", |_| {});
    std::fs::write(&fixture.inputs.archive, b"junk").expect("junk");
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "format", "archive is unknown");
    assert_failed(&record, "extract", "cannot extract");
    for name in &CHECKS[4..] {
        assert_failed(&record, name, "blocked: extract failed");
    }
    assert_eq!(check_names(&record), CHECKS);
    assert!(record.members.is_empty());
    std::fs::remove_file(&fixture.inputs.archive).expect("rm");
    let record = qualify(
        &QualifyInputs {
            work: fixture.inputs.work.join("again"),
            ..fixture.inputs.clone()
        },
        &FakeRunner::new(),
    );
    assert_failed(&record, "format", "cannot read");
    assert_failed(&record, "digest", "cannot read");
    assert_eq!(record.archive.bytes, 0);
}

#[test]
fn qualify_rejects_failed_runs_relocation_and_consumers() {
    let fixture = synthetic("tar.gz", |_| {});
    let mut runner = FakeRunner::new();
    runner.version = "dx 0.0.1\n".to_owned();
    let record = qualify(&fixture.inputs, &runner);
    assert_failed(
        &record,
        "run",
        "version printed \"dx 0.0.1\", want it to contain \"1.2.3\"",
    );
    assert_failed(&record, "relocate", "want it to contain");
    let fixture = synthetic("tar.gz", |_| {});
    let mut runner = FakeRunner::new();
    runner.fail = vec!["test"];
    let record = qualify(&fixture.inputs, &runner);
    assert_failed(&record, "consumer", "test //... failed: test broke");
    assert_eq!(verdict_of(&record, "run").0, Verdict::Ok);
    let fixture = synthetic("tar.gz", |_| {});
    let blocker = fixture.inputs.work.join(MOVED_DIR).join("keep");
    std::fs::create_dir_all(&blocker).expect("blocker");
    let record = qualify(&fixture.inputs, &FakeRunner::new());
    assert_failed(&record, "relocate", "cannot move the install");
    assert_eq!(verdict_of(&record, "consumer").0, Verdict::Ok);
}

#[test]
fn qualify_marks_a_missing_consumer_skipped_and_not_ok() {
    let fixture = synthetic("tar.gz", |_| {});
    let inputs = QualifyInputs {
        consumer: None,
        ..fixture.inputs.clone()
    };
    let record = qualify(&inputs, &FakeRunner::new());
    assert_eq!(
        verdict_of(&record, "consumer"),
        (Verdict::Skipped, "no --consumer".to_owned())
    );
    assert_eq!(record.result, Verdict::Failed);
}

#[test]
fn import_allowed_matches_system_lists_runtime_and_windows_case() {
    let runtime = vec!["VCRUNTIME140.dll".to_owned()];
    assert!(import_allowed("windows", "KERNEL32.dll", &[]));
    assert!(import_allowed(
        "windows",
        "api-ms-win-crt-runtime-l1-1-0.dll",
        &[]
    ));
    assert!(import_allowed("windows", "vcruntime140.dll", &runtime));
    assert!(!import_allowed("windows", "vcruntime140.dll", &[]));
    assert!(import_allowed("macos", "/usr/lib/libSystem.B.dylib", &[]));
    assert!(import_allowed(
        "macos",
        "/System/Library/Frameworks/Security.framework/Versions/A/Security",
        &[]
    ));
    assert!(import_allowed(
        "macos",
        "@rpath/libfoo.dylib",
        &["libfoo.dylib".to_owned()]
    ));
    assert!(!import_allowed(
        "macos",
        "/opt/homebrew/lib/libfoo.dylib",
        &[]
    ));
    assert!(import_allowed("linux", "libc.so.6", &[]));
    assert!(import_allowed("linux", "libgcc_s.so.1", &[]));
    assert!(!import_allowed("linux", "LIBC.so.6", &[]));
    assert!(!import_allowed("linux", "libssl.so.3", &[]));
    assert!(!import_allowed("solaris", "libc.so.6", &[]));
}

fn matrix_json(cells: &[Cell]) -> String {
    serde_json::json!({"schema": MATRIX_SCHEMA, "cells": cells}).to_string()
}

#[test]
fn parse_matrix_reads_cells_and_rejects_bad_ones() {
    let cell = synthetic_cell("tar.gz");
    assert_eq!(
        parse_matrix(&matrix_json(&[cell.clone()])),
        Ok(vec![cell.clone()])
    );
    let bare = r#"{"schema":"release-matrix/v1","cells":[{"name":"a","os":"linux","arch":"x86_64","machine":"x86_64","format":"zip","exe":"dx"}]}"#;
    assert_eq!(
        parse_matrix(bare).expect("bare")[0].runtime,
        Vec::<String>::new()
    );
    let error = |cells: Vec<Cell>| parse_matrix(&matrix_json(&cells)).expect_err("bad");
    let with = |edit: fn(&mut Cell)| {
        let mut bad = cell.clone();
        edit(&mut bad);
        error(vec![bad])
    };
    assert!(error(Vec::new()).contains("no cells"));
    assert!(error(vec![cell.clone(), cell.clone()]).contains("duplicate cell"));
    assert!(with(|cell| cell.os = "solaris".to_owned()).contains("os \"solaris\""));
    assert!(with(|cell| cell.machine = "arm64".to_owned()).contains("machine \"arm64\""));
    assert!(with(|cell| cell.format = "rar".to_owned()).contains("format \"rar\""));
    assert!(with(|cell| cell.arch.clear()).contains("must be non-empty"));
    assert!(with(|cell| cell.name.clear()).contains("must be non-empty"));
    assert!(with(|cell| cell.exe = "bin/dx".to_owned()).contains("top-level name"));
    assert!(with(|cell| cell.exe = "a=b".to_owned()).contains("must not contain '='"));
    assert!(with(|cell| cell.runtime = vec!["NOTICE".to_owned()]).contains("listed twice"));
    assert!(parse_matrix("{")
        .expect_err("json")
        .starts_with("release matrix:"));
    assert!(parse_matrix(r#"{"schema":"release-matrix/v2","cells":[]}"#)
        .expect_err("schema")
        .contains("want \"release-matrix/v1\""));
    assert!(
        parse_matrix(r#"{"schema":"release-matrix/v1","cells":[],"extra":1}"#)
            .expect_err("unknown field")
            .contains("unknown field")
    );
}

struct Release {
    dir: tempfile::TempDir,
    matrix: Vec<Cell>,
    expected: Expected,
}

impl Release {
    fn records(&self) -> PathBuf {
        self.dir.path().join("records")
    }

    fn archives(&self) -> PathBuf {
        self.dir.path().join("archives")
    }

    fn verify(&self) -> Result<Vec<CellRecord>, Vec<String>> {
        verify(
            &self.matrix,
            &self.records(),
            &self.archives(),
            &self.expected,
        )
    }

    fn edit(&self, name: &str, edit: impl FnOnce(&mut serde_json::Value)) {
        let path = self.records().join(name);
        let mut value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        edit(&mut value);
        std::fs::write(&path, value.to_string()).expect("write");
    }

    fn errors(&self) -> String {
        self.verify().expect_err("must fail").join("\n")
    }
}

/// Qualifies two synthetic cells and stages their records and archives like the aggregate job.
fn release() -> Release {
    let dir = tempfile::TempDir::new().expect("tempdir");
    std::fs::create_dir(dir.path().join("records")).expect("records");
    std::fs::create_dir(dir.path().join("archives")).expect("archives");
    let mut matrix = Vec::new();
    for format in FORMATS {
        let mut cell = synthetic_cell(format);
        cell.name = format!("dx-linux-x86_64-{}", format.replace('.', "-"));
        let fixture = stage(cell.clone(), members_of(&cell, synthetic_exe()));
        let record = qualify(&fixture.inputs, &FakeRunner::new());
        assert_eq!(record.result, Verdict::Ok, "{:#?}", record.checks);
        std::fs::write(
            dir.path()
                .join("records")
                .join(format!("{}.json", cell.name)),
            record.render(),
        )
        .expect("record");
        std::fs::copy(
            &fixture.inputs.archive,
            dir.path().join("archives").join(cell.archive_name()),
        )
        .expect("archive");
        matrix.push(cell);
    }
    std::fs::write(dir.path().join("records").join("README.txt"), "ignored").expect("txt");
    Release {
        dir,
        matrix,
        expected: Expected {
            sha: "a".repeat(40),
            version: "1.2.3".to_owned(),
            workflow_ref: "devhusdk/rules_dx/.github/workflows/release-cells.yml@refs/tags/v1.2.3"
                .to_owned(),
        },
    }
}

fn assert_error(release: &Release, want: &str) {
    let errors = release.errors();
    assert!(errors.contains(want), "{errors}\nlacks {want}");
}

const TAR: &str = "dx-linux-x86_64-tar-gz.json";

#[test]
fn verify_accepts_every_qualified_cell_in_matrix_order() {
    let release = release();
    let records = release.verify().expect("verified");
    let cells: Vec<&str> = records.iter().map(|record| record.cell.as_str()).collect();
    assert_eq!(cells, ["dx-linux-x86_64-tar-gz", "dx-linux-x86_64-zip"]);
}

#[test]
fn verify_rejects_stale_identity_wrong_runner_and_machine() {
    let release = release();
    release.edit(TAR, |value| {
        value["sha"] = "c".repeat(40).into();
        value["version"] = "1.2.4".into();
        value["workflow_ref"] = "fork/.github/workflows/x.yml@refs/heads/main".into();
        value["runner"]["os"] = "macos".into();
        value["runner"]["arch"] = "aarch64".into();
        value["binary"]["machine"] = "aarch64".into();
    });
    let errors = release.errors();
    for want in [
        "cell dx-linux-x86_64-tar-gz: sha is \"cccc",
        "version is \"1.2.4\", want \"1.2.3\"",
        "workflow_ref is \"fork/",
        "runner os is \"macos\", want \"linux\"",
        "runner arch is \"aarch64\", want \"x86_64\"",
        "binary machine is \"aarch64\", want \"x86_64\"",
    ] {
        assert!(errors.contains(want), "{errors}\nlacks {want}");
    }
    assert!(!errors.contains("dx-linux-x86_64-zip"), "{errors}");
}

#[test]
fn verify_rejects_a_swapped_or_missing_archive() {
    let release = release();
    let archive = release.archives().join("dx-linux-x86_64-tar-gz.tar.gz");
    std::fs::write(&archive, b"swapped").expect("swap");
    assert_error(&release, "hashes to");
    std::fs::remove_file(&archive).expect("rm");
    assert_error(&release, "cannot read");
    release.edit(TAR, |value| {
        value["archive"]["name"] = "other.tar.gz".into()
    });
    assert!(release
        .errors()
        .contains("archive name is \"other.tar.gz\", want \"dx-linux-x86_64-tar-gz.tar.gz\""));
}

#[test]
fn verify_rejects_failed_skipped_and_missing_checks() {
    let release = release();
    release.edit(TAR, |value| {
        value["checks"][11]["verdict"] = "skipped".into();
        value["checks"][11]["detail"] = "no --consumer".into();
        value["result"] = "failed".into();
    });
    let errors = release.errors();
    assert!(
        errors.contains("check consumer is Skipped: no --consumer"),
        "{errors}"
    );
    assert!(errors.contains("result is Failed"), "{errors}");
    release.edit(TAR, |value| {
        value["checks"].as_array_mut().expect("checks").pop();
    });
    assert_error(&release, "checks are");
}

#[test]
fn verify_rejects_missing_duplicate_unknown_and_malformed_records() {
    let release = release();
    let records = release.records();
    std::fs::copy(records.join(TAR), records.join("zz-copy.json")).expect("copy");
    assert_error(&release, "duplicate record");
    std::fs::remove_file(records.join("zz-copy.json")).expect("rm");
    std::fs::rename(records.join(TAR), records.join("unknown.json")).expect("mv");
    release.edit("unknown.json", |value| {
        value["cell"] = "dx-plan9-x86_64".into()
    });
    let errors = release.errors();
    assert!(
        errors.contains("unknown cell \"dx-plan9-x86_64\""),
        "{errors}"
    );
    assert!(
        errors.contains("cell dx-linux-x86_64-tar-gz: no record"),
        "{errors}"
    );
    release.edit("unknown.json", |value| {
        value["schema"] = "release-cell/v0".into()
    });
    assert_error(&release, "want \"release-cell/v1\"");
    release.edit("unknown.json", |value| value["extra"] = 1.into());
    assert_error(&release, "unknown field");
    std::fs::create_dir(records.join("dir.json")).expect("dir");
    assert_error(&release, "cannot read");
    let missing = verify(
        &release.matrix,
        &release.dir.path().join("nope"),
        &release.archives(),
        &release.expected,
    )
    .expect_err("no dir");
    assert!(missing[0].starts_with("cannot read records"), "{missing:?}");
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(ToString::to_string).collect()
}

#[test]
fn parse_flags_rejects_unknown_missing_repeated_and_valueless_flags() {
    let parse = |args: &[&str]| parse_flags(&strings(args), &["a"], &["b"]).map(|_| ());
    assert_eq!(parse(&["--a", "1", "--b", "2"]), Ok(()));
    assert_eq!(
        parse(&["--c", "1"]),
        Err("unknown argument \"--c\"".to_owned())
    );
    assert_eq!(parse(&["a", "1"]), Err("unknown argument \"a\"".to_owned()));
    assert_eq!(parse(&["--a"]), Err("--a needs a value".to_owned()));
    assert_eq!(
        parse(&["--a", "1", "--a", "2"]),
        Err("--a given twice".to_owned())
    );
    assert_eq!(parse(&["--b", "2"]), Err("missing --a".to_owned()));
}

#[test]
fn qualify_main_and_verify_main_run_from_flags() {
    let release = release();
    let fixture = synthetic("tar.gz", |_| {});
    let matrix = release.dir.path().join("matrix.json");
    std::fs::write(&matrix, matrix_json(&[fixture.inputs.cell.clone()])).expect("matrix");
    let out = release.dir.path().join("record.json");
    let path = |path: &Path| path.to_string_lossy().into_owned();
    let mut args = strings(&["--matrix", &path(&matrix), "--cell", "dx-linux-x86_64"]);
    args.extend(strings(&[
        "--archive",
        &path(&fixture.inputs.archive),
        "--rebuild",
        &path(&fixture.inputs.rebuild),
        "--work",
        &path(&fixture.inputs.work),
        "--sha",
        &"a".repeat(40),
        "--version",
        "1.2.3",
        "--workflow-ref",
        "ref",
        "--workflow-sha",
        "wsha",
        "--run-id",
        "9",
        "--job",
        "cell",
        "--out",
        &path(&out),
        "--consumer",
        &path(fixture.inputs.consumer.as_deref().expect("consumer")),
    ]));
    let record = qualify_main(&args, &FakeRunner::new()).expect("qualify");
    assert_eq!(record.runner.os, std::env::consts::OS);
    assert_eq!(record.runner.arch, std::env::consts::ARCH);
    assert_eq!(record.run_id, "9");
    let written: CellRecord =
        serde_json::from_str(&std::fs::read_to_string(&out).expect("out")).expect("record");
    assert_eq!(written, record);
    let (_, record_out) = parse_qualify_args(&args, "linux", "x86_64").expect("parse");
    assert_eq!(record_out, out);
    let mut bad = args.clone();
    bad[3] = "dx-nope".to_owned();
    assert_eq!(
        parse_qualify_args(&bad, "linux", "x86_64").map(drop),
        Err("unknown cell \"dx-nope\"".to_owned())
    );
    let mut bad = args.clone();
    let at = bad.len() - 3;
    bad[at] = path(&release.dir.path().join("no/such/record.json"));
    assert!(qualify_main(&bad, &FakeRunner::new())
        .expect_err("unwritable")
        .starts_with("cannot write"));
    let mut bad = args.clone();
    bad[1] = path(&release.dir.path().join("missing.json"));
    assert!(qualify_main(&bad, &FakeRunner::new())
        .expect_err("no matrix")
        .starts_with("cannot read"));
    assert_eq!(
        qualify_main(&strings(&["--sha"]), &FakeRunner::new()).map(drop),
        Err("--sha needs a value".to_owned())
    );
    let full_matrix = release.dir.path().join("full.json");
    std::fs::write(&full_matrix, matrix_json(&release.matrix)).expect("matrix");
    let verify_args = |sha: &str| {
        strings(&[
            "--matrix",
            &path(&full_matrix),
            "--records",
            &path(&release.records()),
            "--archives",
            &path(&release.archives()),
            "--sha",
            sha,
            "--version",
            "1.2.3",
            "--workflow-ref",
            &release.expected.workflow_ref,
        ])
    };
    let summary = verify_main(&verify_args(&"a".repeat(40))).expect("verify");
    assert_eq!(summary.lines().count(), 2);
    assert!(summary.starts_with("dx-linux-x86_64-tar-gz dx-linux-x86_64-tar-gz.tar.gz sha256 "));
    assert!(summary.contains(" version 1.2.3 job cell (dx-linux-x86_64) run 77\n"));
    let error = verify_main(&verify_args("stale")).expect_err("stale");
    assert_eq!(error.lines().count(), 2, "{error}");
    assert_eq!(
        verify_main(&strings(&["--records", "r"])),
        Err("missing --matrix".to_owned())
    );
    let parsed = parse_verify_args(&verify_args("s")).expect("parse");
    assert_eq!(parsed.matrix, release.matrix);
    assert_eq!(parsed.expected.sha, "s");
}
