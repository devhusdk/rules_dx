#![cfg(unix)]

use std::process::Command;

use dx_testing::runfiles_root;

fn table_of(lines: &[&str], platforms: &[&str]) -> (String, String) {
    let dir = dx_test_scratch::scratch("compiler-table");
    let input = dir.path().join("in");
    std::fs::create_dir_all(&input).expect("create evidence dir");
    std::fs::write(input.join("a.jsonl"), lines.join("\n") + "\n").expect("write evidence");
    let out = dir.path().join("all.jsonl");
    let done = Command::new("bash")
        .arg(runfiles_root().join("tools/ci/compiler_table.sh"))
        .arg(&input)
        .arg(&out)
        .args(platforms)
        .output()
        .expect("run compiler_table.sh");
    assert!(done.status.success(), "{done:?}");
    (
        String::from_utf8_lossy(&done.stdout).into_owned(),
        std::fs::read_to_string(out).expect("read merged evidence"),
    )
}

#[test]
fn compiler_table_keeps_passed_failed_skipped_and_missing_apart() {
    let (table, merged) = table_of(
        &[
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"passed"}"#,
            r#"{"platform":"linux_x86_64","row":"llvm","outcome":"failed"}"#,
            r#"{"platform":"macos_arm64","row":"llvm","outcome":"skipped"}"#,
        ],
        &["windows_x86_64"],
    );
    assert!(
        table.contains("| row | linux_x86_64 | macos_arm64 | windows_x86_64 |"),
        "{table}"
    );
    assert!(
        table.contains("| gcc | passed | no evidence | no evidence |"),
        "{table}"
    );
    assert!(
        table.contains("| llvm | failed | skipped | no evidence |"),
        "{table}"
    );
    assert_eq!(merged.lines().count(), 3);
}

#[test]
fn compiler_table_never_upgrades_a_skipped_cell() {
    let (table, _) = table_of(
        &[
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"skipped"}"#,
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"skipped"}"#,
        ],
        &[],
    );
    assert!(table.contains("| gcc | skipped |"), "{table}");
}

#[test]
fn compiler_table_reports_mismatch_over_passed() {
    let (table, _) = table_of(
        &[
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"passed"}"#,
            r#"{"platform":"linux_x86_64","row":"gcc","outcome":"mismatch"}"#,
        ],
        &[],
    );
    assert!(table.contains("| gcc | mismatch |"), "{table}");
}

#[test]
fn compiler_table_ranks_blocked_below_mismatch_and_above_passed() {
    let (table, _) = table_of(
        &[
            r#"{"platform":"windows_arm64","row":"gcc","outcome":"blocked","tested_outcome":"skipped"}"#,
            r#"{"platform":"windows_arm64","row":"gcc","outcome":"passed"}"#,
            r#"{"platform":"windows_arm64","row":"llvm","outcome":"blocked","tested_outcome":"skipped"}"#,
            r#"{"platform":"windows_arm64","row":"llvm","outcome":"mismatch"}"#,
            r#"{"platform":"windows_arm64","row":"msvc","outcome":"blocked","tested_outcome":"passed"}"#,
        ],
        &[],
    );
    assert!(table.contains("| gcc | blocked |"), "{table}");
    assert!(table.contains("| llvm | mismatch |"), "{table}");
    assert!(table.contains("| msvc | blocked, passed |"), "{table}");
}

#[test]
fn compiler_table_keeps_a_lone_mismatch() {
    let (table, _) = table_of(
        &[r#"{"platform":"windows_x86_64","row":"msvc","outcome":"mismatch"}"#],
        &[],
    );
    assert!(table.contains("| msvc | mismatch |"), "{table}");
}

#[test]
fn compiler_table_keeps_the_command_and_lockfile_digest() {
    let (_, merged) = table_of(
        &[
            r#"{"platform":"windows_x86_64","row":"msvc","outcome":"passed","command":"bazel test //p:a","lockfile_sha256":"abc"}"#,
        ],
        &[],
    );
    assert!(
        merged.contains(r#""command":"bazel test //p:a""#),
        "{merged}"
    );
    assert!(merged.contains(r#""lockfile_sha256":"abc""#), "{merged}");
}
