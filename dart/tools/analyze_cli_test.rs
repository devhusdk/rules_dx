//! Pins the `dart analyze --format=json` contract the quality parser relies on. dart.dev documents
//! neither the exit codes nor the JSON shape, so an SDK bump that changes them fails here first.
use dx_testing::resolve_runfiles;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CLEAN: &str = "void main() {\n  print(1);\n}\n";
const WARNING: &str = "void main() {\n  var unused = 1;\n}\n";
const ERROR: &str = "void main() {\n  int x = 'a';\n  print(x);\n}\n";
const ASTRAL: &str = "void main() {\n  var s = '\u{1F600}'; int x = 'a'; print(s + x);\n}\n";

struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str, options: &str) -> Self {
        let tmp = std::env::var("TEST_TMPDIR").unwrap_or_else(|_| "/tmp".into());
        let root = PathBuf::from(tmp).join(format!("dart_analyze_{}_{name}", std::process::id()));
        std::fs::create_dir_all(root.join("lib")).expect("mkdir");
        let root = std::fs::canonicalize(root).expect("canonicalize");
        let project = Self { root };
        project.write("analysis_options.yaml", options);
        project
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.root.join(rel);
        std::fs::write(&path, text).expect("write");
        path
    }

    /// Runs the analyzer with no HOME, as the quality runner does.
    fn analyze(&self, paths: &[&Path]) -> Output {
        let bin = resolve_runfiles(&std::env::var("DART").expect("DART"));
        Command::new(bin)
            .env_clear()
            .env("DART_SUPPRESS_ANALYTICS", "true")
            .arg("--suppress-analytics")
            .arg("analyze")
            .arg("--format=json")
            .arg(format!("--cache={}", self.root.join(".cache").display()))
            .args(paths)
            .output()
            .expect("spawn")
    }
}

fn report(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|error| {
        panic!(
            "json: {error}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn diagnostics(out: &Output) -> Vec<serde_json::Value> {
    let value = report(out);
    assert_eq!(value["version"], 1);
    value["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .clone()
}

#[test]
fn clean_exits_0() {
    let p = Project::new("clean", "");
    let file = p.write("lib/a.dart", CLEAN);
    let out = p.analyze(&[&file]);
    assert_eq!(out.status.code(), Some(0));
    assert!(diagnostics(&out).is_empty());
}

#[test]
fn warning_exits_2_and_error_exits_3() {
    let p = Project::new("severity", "");
    let warn = p.write("lib/w.dart", WARNING);
    let out = p.analyze(&[&warn]);
    assert_eq!(out.status.code(), Some(2));
    let found = diagnostics(&out);
    assert_eq!(found[0]["severity"], "WARNING");
    assert_eq!(found[0]["code"], "unused_local_variable");
    assert_eq!(found[0]["type"], "STATIC_WARNING");
    assert!(found[0]["problemMessage"].is_string());
    assert_eq!(found[0]["location"]["file"], warn.to_str().unwrap());
    assert_eq!(found[0]["location"]["range"]["start"]["line"], 2);
    let bad = p.write("lib/e.dart", ERROR);
    let out = p.analyze(&[&bad]);
    assert_eq!(out.status.code(), Some(3));
    let found = diagnostics(&out);
    assert_eq!(found[0]["severity"], "ERROR");
    assert_eq!(found[0]["type"], "COMPILE_TIME_ERROR");
}

#[test]
fn missing_path_exits_64() {
    let p = Project::new("missing", "");
    let out = p.analyze(&[&p.root.join("lib/none.dart")]);
    assert_eq!(out.status.code(), Some(64));
}

#[test]
fn unlisted_files_are_not_reported() {
    let p = Project::new("unlisted", "");
    let clean = p.write("lib/a.dart", CLEAN);
    p.write("lib/b.dart", ERROR);
    let out = p.analyze(&[&clean]);
    assert_eq!(out.status.code(), Some(0));
    assert!(diagnostics(&out).is_empty());
}

#[test]
fn listed_options_file_reports_its_own_problems() {
    let p = Project::new("options", "linter:\n  rules:\n    - no_such_lint_rule\n");
    let file = p.write("lib/a.dart", CLEAN);
    let options = p.root.join("analysis_options.yaml");
    let out = p.analyze(&[&options, &file]);
    let found = diagnostics(&out);
    assert!(found.iter().any(|d| {
        d["code"] == "undefined_lint" && d["location"]["file"] == options.to_str().unwrap()
    }));
    let unlisted = p.analyze(&[&file]);
    assert!(diagnostics(&unlisted).is_empty());
}

#[test]
fn symlinked_scratch_reports_canonical_paths() {
    let p = Project::new("link", "");
    let bad = p.write("lib/e.dart", ERROR);
    let link = p.root.join("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&p.root, &link).expect("symlink");
    #[cfg(unix)]
    {
        let out = p.analyze(&[&link.join("lib/e.dart")]);
        let found = diagnostics(&out);
        assert_eq!(found[0]["location"]["file"], bad.to_str().unwrap());
    }
}

#[test]
fn columns_count_utf16_units() {
    let p = Project::new("utf16", "");
    let file = p.write("lib/a.dart", ASTRAL);
    let out = p.analyze(&[&file]);
    let found = diagnostics(&out);
    let invalid = found
        .iter()
        .find(|d| d["code"] == "invalid_assignment")
        .expect("invalid_assignment");
    // `  var s = '<U+1F600>'; int x = ` puts the literal after one astral char (2 UTF-16 units).
    let line = ASTRAL.lines().nth(1).expect("line");
    let scalars = line.find("'a'").expect("literal");
    let before = &line[..scalars];
    let utf16 = before.encode_utf16().count() as u64 + 1;
    assert_eq!(invalid["location"]["range"]["start"]["column"], utf16);
}
