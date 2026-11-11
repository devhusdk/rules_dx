//! `dx init --apply` writes the checked-in files of one `<lang>/tests/consumer` fixture, unless `tools/testing/consumer/pending.txt` holds `init <lang>`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The pending list named in failures.
const PENDING: &str = "tools/testing/consumer/pending.txt";

/// The name the fixture stores `MODULE.bazel` under.
const STORED_MODULE: &str = "consumer.MODULE.bazel";

/// The name of the scratch workspace directory; `dx init` without a directory argument names the module `my_project`.
const WORKSPACE: &str = "consumer";

fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// True when `dx init --apply` writes the file at `rel` in the fixture.
fn written(rel: &str) -> bool {
    rel == STORED_MODULE || base(rel) == "BUILD.bazel"
}

/// The workspace path of the fixture file at `rel`.
fn workspace_path(rel: &str) -> &str {
    if rel == STORED_MODULE {
        "MODULE.bazel"
    } else {
        rel
    }
}

/// The files of the `lang` fixture, keyed by their path under it.
fn fixture(lang: &str) -> BTreeMap<String, String> {
    let snapshot = dx_testing::rlocation(dx_testing::env_text("DX_CONSUMER_JSON").trim())
        .unwrap_or_else(|error| panic!("DX_CONSUMER_JSON: {error}"));
    let files: BTreeMap<String, String> =
        dx_testing::serde_json::from_str(&dx_testing::read_text(&snapshot))
            .unwrap_or_else(|error| panic!("{snapshot:?}: {error}"));
    let prefix = format!("{lang}/tests/consumer/");
    files
        .into_iter()
        .filter_map(|(path, text)| Some((path.strip_prefix(&prefix)?.to_owned(), text)))
        .collect()
}

/// Copies the fixture sources to a new workspace and names it.
fn workspace(files: &BTreeMap<String, String>) -> PathBuf {
    let root = PathBuf::from(dx_testing::env_text("TEST_TMPDIR")).join(WORKSPACE);
    for (rel, text) in files {
        if !written(rel) {
            dx_testing::write_text(&root.join(rel), text);
        }
    }
    root
}

fn init(root: &Path) -> dx_testing::Run {
    let home = root.with_file_name("home");
    std::fs::create_dir_all(&home).unwrap_or_else(|error| panic!("{home:?}: {error}"));
    let home = dx_testing::path_text(&home);
    let dx = dx_testing::env_file("DX_BIN");
    let bin = root.with_file_name("bin");
    dx_testing::fake_generate_bazel_path(&bin, &dx_testing::env_file("DX_FAKE_GENERATE"));
    let bin = dx_testing::path_text(&bin);
    let gazelle = dx_testing::resolve_runfiles(&dx_testing::env_text("DX_GAZELLE_BIN"));
    let gazelle = dx_testing::path_text(&gazelle);
    let gazelle_args = dx_testing::env_text("DX_FAKE_GAZELLE_ARGS");
    let envs = [
        ("DX_FAKE_GAZELLE", gazelle.as_str()),
        ("DX_FAKE_GAZELLE_ARGS", gazelle_args.as_str()),
        ("PATH", bin.as_str()),
        ("HOME", home.as_str()),
        ("USERPROFILE", home.as_str()),
    ];
    let mut command = dx_testing::command_for(&dx, &envs).unwrap_or_else(|error| panic!("{dx:?}: {error}"));
    command.current_dir(root).args(["init", "--apply", "--workspace"]).arg(root);
    dx_testing::run_command(&mut command, Duration::from_secs(60))
        .unwrap_or_else(|error| panic!("dx init --apply: {error}"))
}

/// The fixture files `dx init --apply` did not write as checked in, each with what differs.
fn differences(files: &BTreeMap<String, String>, root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for (rel, want) in files {
        let path = root.join(workspace_path(rel));
        match std::fs::read_to_string(&path) {
            Ok(have) if have == *want => {}
            Ok(have) => out.push(format!(
                "{rel}: differs\n--- checked in\n{want}--- written\n{have}"
            )),
            Err(error) => out.push(format!("{rel}: not written: {error}")),
        }
    }
    out
}

#[test]
fn init_writes_the_checked_in_files() {
    let lang = dx_testing::env_text("DX_CONSUMER_LANG");
    let files = fixture(&lang);
    for required in ["BUILD.bazel", STORED_MODULE] {
        assert!(
            files.contains_key(required),
            "{lang}/tests/consumer/{required} is missing"
        );
    }
    assert!(
        files.keys().any(|rel| !written(rel)),
        "{lang}/tests/consumer holds no source file"
    );
    let root = workspace(&files);
    let run = init(&root);
    let line = format!("init {lang}");
    let pending = dx_testing::read_text(&dx_testing::env_file("DX_CONSUMER_PENDING"))
        .lines()
        .any(|entry| entry == line);
    assert!(
        pending || run.status.success(),
        "dx init --apply failed in a copy of {lang}/tests/consumer\n{}",
        run.combined()
    );
    let differences = differences(&files, &root);
    if pending {
        assert!(
            !differences.is_empty(),
            "dx init --apply writes every file of {lang}/tests/consumer; delete the line {line:?} from {PENDING}"
        );
    } else {
        assert!(
            differences.is_empty(),
            "dx init --apply does not write {lang}/tests/consumer as checked in\n{}",
            differences.join("\n")
        );
    }
}
