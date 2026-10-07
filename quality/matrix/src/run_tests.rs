//! End-to-end cases: the harness drives the real runner and printer.

use std::path::PathBuf;

use crate::manifest::{runfiles_root, Manifest, Rlocation};
use crate::run::execute;
use crate::Error;

const SOURCE: &str = "a = 1\n";

/// The runfiles tree of this test's own workspace.
fn workspace_runfiles_root() -> PathBuf {
    let workspace = std::env::var("TEST_WORKSPACE").unwrap_or_else(|_| "_main".to_owned());
    runfiles_root().join(workspace)
}

/// Declared inputs the manifest names, resolved the way runfiles would.
struct Inputs {
    runner: PathBuf,
    printer: PathBuf,
    tool: PathBuf,
    source: PathBuf,
    expected: PathBuf,
    root: PathBuf,
}

impl Inputs {
    fn load(expected: PathBuf, source: PathBuf) -> Self {
        let root = workspace_runfiles_root();
        let named = |key: &str| -> PathBuf {
            let rel = std::env::var_os(key).unwrap_or_else(|| panic!("{key} names a runfile"));
            let direct = PathBuf::from(&rel);
            if direct.is_absolute() {
                return direct;
            }
            root.join(rel)
        };
        Inputs {
            runner: named("DX_HARNESS_RUNNER"),
            printer: named("DX_HARNESS_PRINTER"),
            tool: named("DX_HARNESS_TOOL"),
            source,
            expected,
            root,
        }
    }

    /// Resolves one declared key.
    fn resolve(&self, key: &str) -> Result<PathBuf, Error> {
        let found = match key {
            "runner" => Some(&self.runner),
            "printer" => Some(&self.printer),
            "tool" => Some(&self.tool),
            "source" => Some(&self.source),
            "expected" => Some(&self.expected),
            _ => None,
        };
        match found {
            Some(path) if path.is_file() => Ok(path.clone()),
            _ => Err(Error::MissingRunfile {
                key: key.to_owned(),
                detail: "not a declared runfile".to_owned(),
            }),
        }
    }
}

fn manifest() -> String {
    r#"{
  "schema_version": 1,
  "name": "harness_case",
  "producer": "//quality/matrix:harness_case",
  "capability": "format",
  "stages": ["taplo;toml;matrix/toml_clean.toml"],
  "runner": "runner",
  "printer": "printer",
  "expected": "expected",
  "snapshot_dir": "quality/testdata/matrix",
  "sources": [{"workspace": "matrix/toml_clean.toml", "rlocation": "source"}],
  "tools": [{"tool": "taplo", "rlocation": "tool"}]
}
"#
    .to_owned()
}

/// One case whose source, scratch and snapshot belong to this test alone.
struct Case {
    inputs: Inputs,
    manifest: Manifest,
    work: PathBuf,
    update_dir: PathBuf,
}

impl Rlocation for Inputs {
    fn path(&self, key: &str) -> Result<PathBuf, Error> {
        self.resolve(key)
    }
}

impl Case {
    fn new(name: &str) -> Self {
        let dir = std::env::var_os("TEST_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(name);
        std::fs::create_dir_all(&dir).expect("scratch");
        let expected = dir.join("pinned.expected.txt");
        std::fs::write(&expected, "").expect("empty pinned snapshot");
        let source = dir.join("toml_clean.toml");
        std::fs::write(&source, SOURCE).expect("declared source");
        let inputs = Inputs::load(expected, source);
        let work = dir.clone();
        let manifest = Manifest::parse(manifest().as_bytes()).expect("manifest parses");
        Case {
            inputs,
            manifest,
            work,
            update_dir: dir.join("updates"),
        }
    }

    fn run(&self, update: bool) -> Result<(), Error> {
        self.run_manifest(&self.manifest, update)
    }

    fn run_manifest(&self, manifest: &Manifest, update: bool) -> Result<(), Error> {
        let staged = update.then_some(self.update_dir.as_path());
        execute(
            manifest,
            &self.inputs,
            &self.inputs.root,
            &self.work,
            staged,
        )
    }

    fn pin(&self, bytes: &[u8]) {
        std::fs::write(&self.inputs.expected, bytes).expect("pin the snapshot");
    }

    fn pinned(&self) -> Vec<u8> {
        std::fs::read(&self.inputs.expected).expect("pinned snapshot")
    }

    /// Stages the fresh result into this case's own output directory.
    fn stage(&self) -> Vec<u8> {
        self.run(true).expect("stage the fresh result");
        std::fs::read(self.update_dir.join("harness_case.expected.update")).expect("staged update")
    }
}

#[test]
fn a_case_whose_snapshot_matches_the_printed_result_passes() {
    let case = Case::new("harness-pass");
    let fresh = case.stage();
    let text = String::from_utf8(fresh.clone()).expect("staged text");
    assert!(
        text.starts_with("producer //quality/matrix:harness_case\ncapability FORMAT\n"),
        "the staged result names the producer and capability: {text}"
    );
    assert!(
        text.contains("stage taplo classes=toml sources=matrix/toml_clean.toml\n"),
        "the staged result names the stage it ran: {text}"
    );
    assert!(text.contains("convergence STABLE\n"), "stable: {text}");
    assert!(fresh.ends_with(b"\n"), "the staged copy is complete text");
    case.pin(&fresh);
    case.run(false)
        .expect("matrix PASS against its own snapshot");
    assert_eq!(case.pinned(), fresh);
}

#[test]
fn a_stale_snapshot_fails_and_names_the_file_to_review() {
    let case = Case::new("harness-mismatch");
    case.pin(b"producer //someone/else\n");
    assert_eq!(
        case.run(false).expect_err("a stale snapshot fails"),
        Error::Mismatch("quality/testdata/matrix/harness_case.expected.txt".to_owned())
    );
    assert_eq!(
        case.pinned(),
        b"producer //someone/else\n",
        "a failing case leaves the pinned snapshot alone"
    );
}

#[test]
fn an_update_request_stages_the_fresh_result_and_leaves_the_pinned_one_alone() {
    let case = Case::new("harness-update");
    case.pin(b"producer //someone/else\n");
    let fresh = case.stage();
    assert_eq!(
        case.pinned(),
        b"producer //someone/else\n",
        "the pinned snapshot is never rewritten in place"
    );
    case.pin(&fresh);
    case.run(false)
        .expect("the staged copy pins without further changes");
}

#[test]
fn a_snapshot_is_compared_byte_for_byte() {
    let case = Case::new("harness-bytes");
    let fresh = case.stage();
    let text = String::from_utf8(fresh).expect("staged text");
    let altered = [
        text.replace("stages 1\n", "stages 4\n"),
        text.replace("convergence STABLE\n", "convergence STABLE"),
        format!("{text}trailing\n"),
        text.replace("initial 0\n", "initial 1\n"),
    ];
    for candidate in &altered {
        assert_ne!(candidate, &text, "every variant really differs");
    }
    for candidate in altered {
        case.pin(candidate.as_bytes());
        assert_eq!(
            case.run(false).expect_err("a differing snapshot fails"),
            Error::Mismatch("quality/testdata/matrix/harness_case.expected.txt".to_owned()),
            "a wrong count, a missing newline, a trailing line and a wrong row count all fail: {candidate:?}"
        );
    }
    case.pin(text.as_bytes());
    case.run(false)
        .expect("the untouched snapshot still passes");
}

#[test]
fn a_capability_the_harness_does_not_own_is_refused() {
    let case = Case::new("harness-capability");
    let text = manifest().replace("\"capability\": \"format\"", "\"capability\": \"audit\"");
    let manifest = Manifest::parse(text.as_bytes()).expect("manifest parses");
    let error = case
        .run_manifest(&manifest, false)
        .expect_err("an unknown capability fails");
    assert!(
        matches!(error, Error::Malformed(_)),
        "the unknown capability names itself: {error:?}"
    );
    assert_eq!(case.pinned(), Vec::<u8>::new());
}

#[test]
fn a_missing_declared_input_fails_before_anything_is_launched() {
    let case = Case::new("harness-missing");
    std::fs::remove_file(&case.inputs.source).expect("drop the declared source");
    assert_eq!(
        case.run(false).expect_err("an undeclared source fails"),
        Error::MissingRunfile {
            key: "source".to_owned(),
            detail: "not a declared runfile".to_owned(),
        }
    );
}

#[test]
fn a_runner_failure_reports_the_child_output() {
    let case = Case::new("harness-runner");
    let text = manifest().replace("taplo;toml;matrix/toml_clean.toml", "taplo;toml");
    let manifest = Manifest::parse(text.as_bytes()).expect("manifest parses");
    let error = case
        .run_manifest(&manifest, false)
        .expect_err("a broken stage fails the runner");
    assert!(
        matches!(error, Error::Runner(_)),
        "the runner reports the failure: {error:?}"
    );
}

#[test]
fn a_result_the_runner_cannot_produce_is_a_runner_failure() {
    let case = Case::new("harness-notarunner");
    let text = manifest().replace("\"runner\": \"runner\"", "\"runner\": \"source\"");
    let manifest = Manifest::parse(text.as_bytes()).expect("manifest parses");
    let error = case
        .run_manifest(&manifest, false)
        .expect_err("a source file is not a runner");
    assert!(
        matches!(error, Error::Runner(_)),
        "the failing stage names itself: {error:?}"
    );
}
