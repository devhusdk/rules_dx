//! Runs the rustdoc JSON converter on the fixture crate and on broken inputs.

use std::path::{Path, PathBuf};
use std::process::Command;

use documentation_ir::decode_shard;

struct Run {
    code: i32,
    stderr: String,
}

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    dx_testing::resolve_runfiles(&rel)
}

fn fixture_json() -> String {
    let dir = runfile("DX_FIXTURE_RUSTDOC");
    std::fs::read_to_string(dir.join("docs_fixture.json"))
        .unwrap_or_else(|error| panic!("read fixture JSON: {error}"))
}

fn fixture_srcs() -> String {
    std::fs::read_to_string(runfile("DX_FIXTURE_SRCS"))
        .unwrap_or_else(|error| panic!("read fixture srcs: {error}"))
}

fn scratch(name: &str, json: &str, srcs: &str) -> dx_test_scratch::TempDir {
    let dir = dx_test_scratch::scratch(name);
    std::fs::write(dir.path().join("crate.json"), json).expect("write JSON");
    std::fs::write(dir.path().join("srcs.tsv"), srcs).expect("write srcs");
    dir
}

fn extract(dir: &Path, args: &[&str]) -> Run {
    let output = Command::new(runfile("DX_RUST_EXTRACT"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|error| panic!("run rust_extract: {error}"));
    Run {
        code: output.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn standard(dir: &Path) -> Run {
    extract(
        dir,
        &[
            "--json",
            "crate.json",
            "--package",
            "docs_fixture",
            "--crate",
            "docs_fixture",
            "--srcs-file",
            "srcs.tsv",
            "--out",
            "shard.ir.pb",
        ],
    )
}

fn expect_failure(dir: &Path, run: &Run, detail: &str) {
    assert_eq!(
        run.code, 1,
        "expected a failure naming {detail:?}: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains(detail),
        "stderr must name {detail:?}, got {}",
        run.stderr
    );
    assert!(
        !dir.join("shard.ir.pb").exists(),
        "a failed run left a shard"
    );
    assert!(
        !dir.join("shard.ir.pb.partial").exists(),
        "a failed run left a partial shard"
    );
}

#[test]
fn the_fixture_json_becomes_a_valid_shard() {
    let dir_tmp = scratch("extract-ok", &fixture_json(), &fixture_srcs());
    let dir = dir_tmp.path();
    let run = standard(&dir);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(!dir.join("shard.ir.pb.partial").exists());
    let bytes = std::fs::read(dir.join("shard.ir.pb")).expect("read shard");
    let shard = decode_shard(&bytes).unwrap_or_else(|error| panic!("decode: {error:?}"));
    assert_eq!(shard.package, "docs_fixture");
    assert!(shard
        .symbols
        .iter()
        .any(|s| s.id == "rust:docs_fixture:add"));
}

#[test]
fn malformed_json_fails_without_a_shard() {
    let dir_tmp = scratch("extract-malformed", "{", &fixture_srcs());
    let dir = dir_tmp.path();
    let run = standard(&dir);
    expect_failure(&dir, &run, "docs_rust: malformed rustdoc JSON");
}

#[test]
fn another_format_version_fails_without_a_shard() {
    let json = fixture_json();
    let found = json
        .split("\"format_version\":")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .unwrap_or_else(|| panic!("fixture JSON has no format_version"))
        .to_string();
    let other = (found.parse::<u32>().expect("number") - 1).to_string();
    let changed = json.replace(
        &format!("\"format_version\":{found}"),
        &format!("\"format_version\":{other}"),
    );
    let dir_tmp = scratch("extract-version", &changed, &fixture_srcs());
    let dir = dir_tmp.path();
    let run = standard(&dir);
    expect_failure(
        &dir,
        &run,
        &format!("rustdoc JSON format_version {other} is not the supported {found}"),
    );
}

#[test]
fn a_span_outside_the_crate_sources_fails_without_a_shard() {
    let srcs: String = fixture_srcs()
        .lines()
        .filter(|line| !line.contains("shapes.rs"))
        .map(|line| format!("{line}\n"))
        .collect();
    let dir_tmp = scratch("extract-source", &fixture_json(), &srcs);
    let dir = dir_tmp.path();
    let run = standard(&dir);
    expect_failure(
        &dir,
        &run,
        "source 'tools/docs/rust/fixture/src/shapes.rs' of",
    );
}

#[test]
fn a_crate_without_public_items_fails_without_a_shard() {
    let json = fixture_json();
    let marker = "\"is_crate\":true,\"items\":[";
    let start = json.find(marker).expect("crate root module") + marker.len();
    let end = start + json[start..].find(']').expect("item list end");
    let empty = format!("{}{}", &json[..start], &json[end..]);
    let dir_tmp = scratch("extract-empty", &empty, &fixture_srcs());
    let dir = dir_tmp.path();
    let run = standard(&dir);
    expect_failure(&dir, &run, "crate 'docs_fixture' has no public API");
}

#[test]
fn a_missing_flag_is_a_usage_error() {
    let dir_tmp = scratch("extract-usage", &fixture_json(), &fixture_srcs());
    let dir = dir_tmp.path();
    let run = extract(
        &dir,
        &[
            "--package",
            "docs_fixture",
            "--crate",
            "docs_fixture",
            "--srcs-file",
            "srcs.tsv",
            "--out",
            "shard.ir.pb",
        ],
    );
    assert_eq!(run.code, 2, "{}", run.stderr);
    assert!(run.stderr.contains("--json is required"), "{}", run.stderr);
    assert!(!dir.join("shard.ir.pb").exists());
}

#[test]
fn a_nonexistent_json_file_is_a_failure() {
    let dir_tmp = scratch("extract-nojson", &fixture_json(), &fixture_srcs());
    let dir = dir_tmp.path();
    let run = extract(
        &dir,
        &[
            "--package",
            "docs_fixture",
            "--crate",
            "docs_fixture",
            "--json",
            "missing.json",
            "--srcs-file",
            "srcs.tsv",
            "--out",
            "shard.ir.pb",
        ],
    );
    expect_failure(&dir, &run, "cannot read rustdoc JSON 'missing.json'");
}
