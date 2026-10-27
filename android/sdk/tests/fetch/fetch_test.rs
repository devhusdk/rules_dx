use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, sha256_file};

const FIXTURE: &str = "android/sdk/tests/fetch/fixtures/android-ndk-r27-linux.zip";

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|error| panic!("{path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let real = canonical(&resolve_runfiles("android/sdk/tests/fetch/fetch_test.rs"));
    real.ancestors()
        .nth(5)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn fixture_repo(name: &str, url: &str, sha256: &str) -> String {
    format!(
        "use_repo_rule(\"@rules_dx//android/sdk/tests/fetch:repos.bzl\", \"ndk_fixture_repo\")(\n    name = \"{name}\",\n    anchor = \"source.properties\",\n    archive = \"android-ndk-r27-linux.zip\",\n    canonical_id = \"dx-android-ndk-archive:{url}\",\n    directory = \"android-ndk-r27\",\n    sha256 = \"{sha256}\",\n    url = \"{url}\",\n)\n"
    )
}

fn consumer() -> Case {
    let rules = rules_dx_root();
    let fixture = canonical(&resolve_runfiles(FIXTURE));
    let digest = sha256_file(&fixture).unwrap_or_else(|error| panic!("{error}"));
    let url = format!("file://{}", fixture.display());
    let case = nested::case("fetch");
    let root = case.root().to_path_buf();
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n\n{}\n{}",
            rules.display(),
            fixture_repo("fixture_ok", &url, &digest),
            fixture_repo("fixture_bad", &url, &"0".repeat(64)),
        ),
    );
    write(
        &root.join(".bazelrc"),
        &read(&rules.join("tools/bazelrc/preset.bazelrc")),
    );
    write(&root.join("BUILD.bazel"), "");
    case
}

struct Outcome {
    code: Option<i32>,
    text: String,
}

fn build(case: &Case, target: &str) -> Outcome {
    let run = case.bazel(&["build", "--color=no", "--curses=no", target]);
    Outcome {
        code: run.status.code(),
        text: run.combined(),
    }
}

#[test]
fn a_pinned_archive_fetches_its_anchor() {
    let case = consumer();
    let built = build(&case, "@fixture_ok//:ndk_anchor");
    assert_eq!(built.code, Some(0), "{}", built.text);
}

#[test]
fn a_wrong_digest_fails_the_fetch() {
    let case = consumer();
    let built = build(&case, "@fixture_bad//:ndk_anchor");
    assert_eq!(built.code, Some(1), "{}", built.text);
    assert!(built.text.contains("Checksum was"), "{}", built.text);
}
