//! Behavioral cases for the manifest, the runner argv and the update request.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::manifest::{EnvValue, Manifest, Rlocation, MANIFEST_SCHEMA_VERSION};
use crate::run::update_requested;
use crate::Error;

const MANIFEST: &str = r#"{
  "schema_version": 1,
  "name": "matrix_python_lint_pass",
  "producer": "//quality/testdata:matrix_python_lint_pass",
  "capability": "lint",
  "stages": ["ruff;python,py;a.py,b.py"],
  "runner": "_main/quality/runner/quality_runner",
  "printer": "_main/quality/result/print_result",
  "expected": "_main/quality/testdata/matrix/matrix_python_lint_pass.expected.txt",
  "snapshot_dir": "quality/testdata/matrix",
  "sources": [{"workspace": "a.py", "rlocation": "_main/a.py"}],
  "siblings": [{"workspace": "ruff.toml", "rlocation": "_main/ruff.toml"}],
  "tools": [{"tool": "ruff", "rlocation": "repo/ruff"}],
  "tool_configs": [{"tool": "ruff", "config": "ruff.toml"}],
  "tool_editions": [{"tool": "rustfmt", "edition": "2021"}],
  "tool_files": [{"tool": "flake8", "rel": "setup.cfg", "rlocation": "repo/setup.cfg"}],
  "tool_env": [
    {"tool": "flake8", "key": "RUNFILES_DIR", "value": {"runfiles_root": true}},
    {"tool": "eslint", "key": "JS_BINARY__NO_CD_BINDIR", "value": {"value": "1"}}
  ],
  "upstream": [{"tool": "clippy", "rlocation": "repo/clippy.txt"}]
}"#;

fn manifest() -> Manifest {
    Manifest::parse(MANIFEST.as_bytes()).expect("manifest parses")
}

fn args(argv: &[OsString]) -> Vec<String> {
    argv.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

fn flag_value(argv: &[String], flag: &str) -> Vec<String> {
    argv.windows(2)
        .filter(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
        .collect()
}

/// Resolves every key to a path named after it, spaces included.
fn spaced(key: &str) -> Result<PathBuf, Error> {
    Ok(PathBuf::from(format!("/run files/{key}")))
}

#[test]
fn a_parsed_manifest_keeps_its_declared_inputs() {
    let manifest = manifest();
    assert_eq!(manifest.schema_version, MANIFEST_SCHEMA_VERSION);
    assert_eq!(manifest.name, "matrix_python_lint_pass");
    assert_eq!(
        manifest.snapshot_path(),
        "quality/testdata/matrix/matrix_python_lint_pass.expected.txt"
    );
    assert_eq!(
        manifest.update_name(),
        "matrix_python_lint_pass.expected.update"
    );
    assert_eq!(
        manifest.stages().expect("stages parse"),
        vec![crate::manifest::StageSpec {
            tool_id: "ruff".to_owned(),
            class_ids: vec!["python".to_owned(), "py".to_owned()],
            source_paths: vec!["a.py".to_owned(), "b.py".to_owned()],
        }]
    );
}

#[test]
fn a_manifest_rejects_an_unknown_schema_an_unknown_field_and_a_broken_stage() {
    let bumped = MANIFEST.replace("\"schema_version\": 1", "\"schema_version\": 2");
    assert_eq!(
        Manifest::parse(bumped.as_bytes()),
        Err(Error::UnsupportedSchema { found: 2 })
    );
    let extra = MANIFEST.replace("\"snapshot_dir\"", "\"unknown_key\": 1, \"snapshot_dir\"");
    assert!(matches!(
        Manifest::parse(extra.as_bytes()),
        Err(Error::Malformed(_))
    ));
    let stage = MANIFEST.replace("ruff;python,py;a.py,b.py", "ruff;python");
    assert!(matches!(
        Manifest::parse(stage.as_bytes()).and_then(|parsed| parsed.stages()),
        Err(Error::Malformed(_))
    ));
    assert!(matches!(
        Manifest::parse(b"not json"),
        Err(Error::Malformed(_))
    ));
}

#[test]
fn the_runner_argv_names_every_input_in_the_order_the_runner_reads() {
    let manifest = manifest();
    let argv = args(
        &manifest
            .runner_argv(
                Path::new("/work/out.pb"),
                Path::new("/work/scratch"),
                Path::new("/runfiles"),
                &spaced,
            )
            .expect("argv builds"),
    );
    assert_eq!(argv[0], "/run files/_main/quality/runner/quality_runner");
    assert_eq!(
        flag_value(&argv, "--producer"),
        vec!["//quality/testdata:matrix_python_lint_pass"]
    );
    assert_eq!(flag_value(&argv, "--capability"), vec!["lint"]);
    assert_eq!(flag_value(&argv, "--output"), vec!["/work/out.pb"]);
    assert_eq!(
        flag_value(&argv, "--stage"),
        vec!["ruff;python,py;a.py,b.py"]
    );
    assert_eq!(
        flag_value(&argv, "--source"),
        vec!["a.py=/run files/_main/a.py"]
    );
    assert_eq!(
        flag_value(&argv, "--sibling"),
        vec!["ruff.toml=/run files/_main/ruff.toml"]
    );
    assert!(argv.contains(&"--real".to_owned()));
    assert_eq!(flag_value(&argv, "--scratch-parent"), vec!["/work/scratch"]);
    assert_eq!(
        flag_value(&argv, "--tool-binary"),
        vec!["ruff=/run files/repo/ruff"]
    );
    assert_eq!(flag_value(&argv, "--tool-config"), vec!["ruff=ruff.toml"]);
    assert_eq!(flag_value(&argv, "--tool-edition"), vec!["rustfmt=2021"]);
    assert_eq!(
        flag_value(&argv, "--tool-file"),
        vec!["flake8=setup.cfg=/run files/repo/setup.cfg"]
    );
    assert_eq!(
        flag_value(&argv, "--tool-env"),
        vec![
            "flake8=RUNFILES_DIR=/runfiles",
            "eslint=JS_BINARY__NO_CD_BINDIR=1"
        ]
    );
    assert_eq!(
        flag_value(&argv, "--upstream-diagnostics"),
        vec!["clippy=/run files/repo/clippy.txt"]
    );
}

#[test]
fn every_runner_flag_carries_exactly_one_value_and_a_path_with_spaces_stays_one_argument() {
    let manifest = manifest();
    let argv = args(
        &manifest
            .runner_argv(
                Path::new("/work/out.pb"),
                Path::new("/work/scratch"),
                Path::new("/rf"),
                &spaced,
            )
            .expect("argv builds"),
    );
    for flag in [
        "--producer",
        "--capability",
        "--output",
        "--stage",
        "--source",
        "--sibling",
        "--scratch-parent",
        "--tool-binary",
        "--tool-config",
        "--tool-edition",
        "--tool-file",
        "--tool-env",
        "--upstream-diagnostics",
    ] {
        assert!(
            argv.iter().position(|arg| arg == flag).is_some(),
            "{flag} is passed"
        );
    }
    for (index, arg) in argv.iter().enumerate() {
        if arg.starts_with("--") && arg != "--real" {
            assert!(
                index + 1 < argv.len(),
                "{arg} carries a value instead of trailing"
            );
        }
    }
    assert!(
        !argv.iter().any(|arg| arg == "--runfiles"),
        "no runfiles flag is invented"
    );
}

#[test]
fn the_printer_argv_is_the_printer_and_the_result() {
    let manifest = manifest();
    assert_eq!(
        args(
            &manifest
                .printer_argv(Path::new("/work/out.pb"), &spaced)
                .expect("argv builds")
        ),
        vec![
            "/run files/_main/quality/result/print_result".to_owned(),
            "/work/out.pb".to_owned(),
        ]
    );
}

#[test]
fn a_missing_runfile_is_named_before_anything_is_launched() {
    let manifest = manifest();
    let missing = |key: &str| -> Result<PathBuf, Error> {
        Err(Error::MissingRunfile {
            key: key.to_owned(),
            detail: "absent".to_owned(),
        })
    };
    let error = manifest
        .runner_argv(
            Path::new("/out.pb"),
            Path::new("/scratch"),
            Path::new("/rf"),
            &missing,
        )
        .expect_err("a missing runner fails");
    assert_eq!(
        error,
        Error::MissingRunfile {
            key: "_main/quality/runner/quality_runner".to_owned(),
            detail: "absent".to_owned(),
        }
    );
}

#[test]
fn a_runfiles_value_names_the_runfiles_root_and_a_literal_is_passed_through() {
    assert_eq!(
        EnvValue::RunfilesRoot {
            runfiles_root: true
        }
        .text(Path::new("/runfiles"))
        .expect("root"),
        "/runfiles"
    );
    assert!(EnvValue::RunfilesRoot {
        runfiles_root: false
    }
    .text(Path::new("/runfiles"))
    .is_err());
    assert_eq!(
        EnvValue::Literal {
            value: "1".to_owned()
        }
        .text(Path::new("/runfiles"))
        .expect("literal"),
        "1"
    );
}

#[test]
fn only_an_explicit_update_request_stages_a_fresh_snapshot() {
    assert!(update_requested(Some("1")));
    for value in [None, Some(""), Some("0"), Some("true"), Some(" 1")] {
        assert!(!update_requested(value), "{value:?} never stages");
    }
}

#[test]
fn a_closure_resolves_a_runfile_without_a_named_source() {
    let resolve = |key: &str| -> Result<PathBuf, Error> {
        if key == "_main/a.py" {
            return Ok(PathBuf::from("/resolved/a.py"));
        }
        Err(Error::MissingRunfile {
            key: key.to_owned(),
            detail: "absent".to_owned(),
        })
    };
    assert_eq!(
        resolve.path("_main/a.py").expect("resolved"),
        PathBuf::from("/resolved/a.py")
    );
    assert!(resolve.path("_main/missing.py").is_err());
}
