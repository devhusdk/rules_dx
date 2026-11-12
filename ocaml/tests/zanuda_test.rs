//! The built zanuda reports findings on typed-tree files from the pinned toolchain, names the
//! listed source in each finding, passes a clean target and fails on a missing or corrupt .cmt.

use std::path::{Path, PathBuf};

use dx_testing::{Run, command_for, env_file, env_text, path_text, read, scratch_dir, write};

fn runfiles_root() -> PathBuf {
    let rel = env_text("DIRTY_LIST");
    let mut root = env_file("DIRTY_LIST");
    for _ in Path::new(&rel).components() {
        root.pop();
    }
    root
}

fn zanuda(args: &[&str]) -> Run {
    let stdlib = env_file("STDLIB_CMI");
    let ocamllib = path_text(stdlib.parent().expect("stdlib directory"));
    let mut command = command_for(
        &env_file("ZANUDA"),
        &[("OCAMLLIB", &ocamllib), ("PATH", "")],
    )
    .expect("command");
    command.current_dir(runfiles_root()).args(args);
    dx_testing::run_command(&mut command, std::time::Duration::from_secs(120)).expect("zanuda runs")
}

fn lint(list: &Path, report: &Path) -> Run {
    zanuda(&[
        "-cmt-list",
        &path_text(list),
        "-no-top_file_license",
        "-ordjsonl",
        &path_text(report),
    ])
}

fn rewrite_cmt(list: &Path, cmt: &str, out: &Path) {
    let text: String = read(list)
        .lines()
        .map(|line| {
            let mut fields = line.split('\t');
            let kind = fields.next().expect("kind");
            let source = fields.next().expect("source");
            format!("{kind}\t{source}\t{cmt}\n")
        })
        .collect();
    write(out, &text);
}

#[test]
fn reports_version() {
    let out = zanuda(&["-version"]);
    assert!(out.status.success(), "{}", out.combined());
    assert!(
        out.stdout.contains("2.0.0"),
        "unexpected version: {}",
        out.combined()
    );
}

#[test]
fn dirty_target_reports_listed_source() {
    let scratch = scratch_dir("dirty");
    let report = scratch.join("dirty.jsonl");
    let list = env_file("DIRTY_LIST");
    let out = lint(&list, &report);
    assert!(out.status.success(), "{}", out.combined());
    let findings = read(&report);
    for id in ["if_bool", "list_length_comparisons"] {
        assert!(
            findings.contains(&format!("\"value\":\"{id}\"")),
            "no {id} finding:\n{findings}"
        );
    }
    let listed = read(&list)
        .lines()
        .next()
        .and_then(|line| line.split('\t').nth(1))
        .expect("listed source")
        .to_owned();
    let paths: Vec<&str> = findings
        .split("\"path\":\"")
        .skip(1)
        .map(|rest| rest.split('"').next().expect("path"))
        .collect();
    assert!(!paths.is_empty(), "no finding names a path:\n{findings}");
    for path in paths {
        assert_eq!(path, listed, "finding names {path}, want {listed}");
    }
}

#[test]
fn clean_target_has_no_findings() {
    let scratch = scratch_dir("clean");
    let report = scratch.join("clean.jsonl");
    let out = lint(&env_file("CLEAN_LIST"), &report);
    assert!(out.status.success(), "{}", out.combined());
    let findings = std::fs::read_to_string(&report).unwrap_or_default();
    assert!(findings.is_empty(), "unexpected findings:\n{findings}");
}

#[test]
fn missing_cmt_fails() {
    let scratch = scratch_dir("missing");
    let list = scratch.join("missing.list");
    rewrite_cmt(&env_file("CLEAN_LIST"), "does-not-exist.cmt", &list);
    let out = lint(&list, &scratch.join("missing.jsonl"));
    assert!(!out.status.success(), "{}", out.combined());
}

#[test]
fn corrupt_cmt_fails() {
    let scratch = scratch_dir("corrupt");
    let cmt = scratch.join("corrupt.cmt");
    write(&cmt, "not a cmt file\n");
    let list = scratch.join("corrupt.list");
    rewrite_cmt(&env_file("CLEAN_LIST"), &path_text(&cmt), &list);
    let out = lint(&list, &scratch.join("corrupt.jsonl"));
    assert!(!out.status.success(), "{}", out.combined());
}
