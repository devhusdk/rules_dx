use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

const BUILD: &str = r#"load("@my_dx//deploy/rules:archive.bzl", "archive_deploy")
load("@my_dx//deploy/rules:github.bzl", "github_deploy")
load("@my_dx//rust/rules:defs.bzl", "rust_binary")

rust_binary(
    name = "app",
    srcs = ["main.rs"],
)

archive_deploy(
    name = "release",
    app = ":app",
)

github_deploy(
    name = "bundle",
    artifacts = [
        "@assets//:notice",
        "@assets//:notes",
    ],
)
"#;

const ASSETS_BUILD: &str = r#"filegroup(
    name = "notice",
    srcs = ["NOTICE.txt"],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "notes",
    srcs = ["release notes.txt"],
    visibility = ["//visibility:public"],
)
"#;

const NOTICE: &str = "Assets notice\n";

const NOTES: &str = "Release notes with spaces\n";

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let source = resolve_runfiles("deploy/tests/runfiles/renamed_consumer_test.rs");
    let real = std::fs::canonicalize(&source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(write_consumer)
}

fn write_consumer() -> PathBuf {
    let rules = rules_dx_root();
    let root = mkscratch("renamed-consumer").unwrap_or_else(|error| panic!("scratch: {error}"));
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"my_dx\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n\nlocal_repository = use_repo_rule(\"@bazel_tools//tools/build_defs/repo:local.bzl\", \"local_repository\")\n\nlocal_repository(\n    name = \"assets\",\n    path = \"{}\",\n)\n",
            rules.display(),
            root.join("third_party/assets").display()
        ),
    );
    write(
        &root.join(".bazelrc"),
        &format!(
            "{}startup --max_idle_secs=120\n",
            read(&rules.join("tools/bazelrc/preset.bazelrc"))
        ),
    );
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("main.rs"), "fn main() {}\n");
    write(&root.join("third_party/assets/REPO.bazel"), "");
    write(&root.join("third_party/assets/BUILD.bazel"), ASSETS_BUILD);
    write(&root.join("third_party/assets/NOTICE.txt"), NOTICE);
    write(&root.join("third_party/assets/release notes.txt"), NOTES);
    root
}

fn dx(root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec!["deploy", "--workspace", workspace.as_str()];
    argv.extend_from_slice(args);
    run(&resolve_runfiles(&rel), &argv, envs).unwrap_or_else(|error| panic!("run dx: {error}"))
}

#[test]
fn a_renamed_rules_dx_deploys_inputs_from_a_renamed_repository() {
    let root = consumer();
    let out = root.join("github-out");
    let target = out.to_string_lossy().into_owned();

    let checked = dx(root, &["//:bundle"], &[]);
    assert_eq!(checked.status.code(), Some(0), "{}", checked.combined());
    assert!(
        !out.exists(),
        "a check must not stage: {}",
        checked.combined()
    );

    let applied = dx(
        root,
        &["--apply", "//:bundle", "--", target.as_str()],
        &[("GH_RELEASE_DRY_RUN", "1")],
    );
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    assert!(
        applied.stdout.contains("publishing nothing"),
        "{}",
        applied.combined()
    );
    let staging = out.join("bundle-release");
    assert_eq!(read(&staging.join("NOTICE.txt")), NOTICE);
    assert_eq!(read(&staging.join("release notes.txt")), NOTES);
}

#[test]
fn a_renamed_rules_dx_archives_and_checksums_a_consumer_app() {
    let root = consumer();
    let out = root.join("archive-out");
    let target = out.to_string_lossy().into_owned();

    let applied = dx(root, &["--apply", "//:release", "--", target.as_str()], &[]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    let tarball = out.join("release.tar.gz");
    let digest = dx_digest::sha256_file_hex(&tarball)
        .unwrap_or_else(|error| panic!("{}: {error}", tarball.display()));
    assert_eq!(
        read(&out.join("release.tar.gz.sha256")),
        format!("{digest}  release.tar.gz\n")
    );
}
