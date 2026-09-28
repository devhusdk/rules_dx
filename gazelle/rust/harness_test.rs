use std::path::{Path, PathBuf};

fn gazelle_bin() -> PathBuf {
    let rel = std::env::var("DX_GAZELLE_BIN").expect("DX_GAZELLE_BIN must name the gazelle binary");
    dx_testing::resolve_runfiles(&rel)
}

fn workspace(prefix: &str) -> PathBuf {
    dx_testing::mkscratch(prefix).expect("scratch workspace")
}

fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parents");
    }
    std::fs::write(&path, text.as_bytes()).expect("write");
    path
}

fn run_gazelle(root: &Path, extra_env: &[(&str, &str)]) -> dx_testing::Run {
    let bin = gazelle_bin();
    let repo = format!("-repo_root={}", root.display());
    std::process::Command::new(&bin)
        .args([repo.as_str()])
        .envs(extra_env.iter().copied())
        .current_dir(root)
        .output()
        .map(|output| dx_testing::Run {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
        .expect("gazelle must execute")
}

fn generated_builds(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).expect("read workspace dir");
        for entry in entries {
            let entry = entry.expect("workspace entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|name| name.to_str()) == Some("BUILD.bazel") {
                let rel = path.strip_prefix(root).expect("rel").to_path_buf();
                let bytes = std::fs::read(&path).expect("read build file");
                out.push((rel, bytes));
            }
        }
    }
    out.sort();
    out
}

fn failure_case() {
    let root = workspace("gazelle-failure-");
    write(&root, "WORKSPACE", "");
    write(&root, "crate/src/lib.rs", "use missing_crate::Thing;\n");

    let first = run_gazelle(&root, &[]);
    assert!(!first.status.success(), "unresolved import must fail");
    assert!(
        first
            .combined()
            .contains("unresolved import \"missing_crate\""),
        "missing actionable unresolved-import diagnostic\n{}",
        first.combined()
    );
    assert!(
        !first.combined().contains("panic") && !first.combined().contains("goroutine"),
        "failure printed Go panic text\n{}",
        first.combined()
    );
    assert!(
        !root.join("crate/BUILD.bazel").exists() && !root.join("crate/BUILD").exists(),
        "failed generation wrote a BUILD file"
    );

    write(
        &root,
        "BUILD.bazel",
        "# gazelle:dx_ignore_import rust missing_crate\n",
    );
    let allowed = run_gazelle(&root, &[]);
    assert!(
        allowed.status.success(),
        "exact inherited ignore must permit generation\n{}",
        allowed.combined()
    );
    assert!(
        root.join("crate/BUILD.bazel").is_file(),
        "exact inherited ignore did not permit generation"
    );

    std::fs::remove_file(root.join("crate/src/lib.rs")).expect("remove lib");
    std::fs::remove_file(root.join("crate/BUILD.bazel")).expect("remove build");
    let stale = run_gazelle(&root, &[]);
    assert!(
        !stale.status.success()
            && stale
                .combined()
                .contains("stale # gazelle:dx_ignore_import"),
        "stale ignore did not fail\n{}",
        stale.combined()
    );
    assert!(
        !stale.combined().contains("panic") && !stale.combined().contains("goroutine"),
        "failure printed Go panic text\n{}",
        stale.combined()
    );

    std::fs::remove_file(root.join("BUILD.bazel")).expect("remove root build");
    let intended = root.join("intended.json");
    let bad_mode = run_gazelle(
        &root,
        &[
            ("DX_GENERATE_MODE", "print"),
            ("DX_GENERATE_INTENDED", intended.to_string_lossy().as_ref()),
        ],
    );
    assert!(!bad_mode.status.success(), "bad DX_GENERATE_MODE must fail");
    assert!(
        bad_mode
            .combined()
            .contains("must be \"check\" or \"default\""),
        "missing actionable mode diagnostic\n{}",
        bad_mode.combined()
    );
    assert!(
        !bad_mode.combined().contains("panic") && !bad_mode.combined().contains("goroutine"),
        "failure printed Go panic text\n{}",
        bad_mode.combined()
    );
    assert!(
        !root.join("crate/BUILD.bazel").exists()
            && !root.join("crate/BUILD").exists()
            && !intended.exists(),
        "failed generation wrote output"
    );
}

fn idempotent_case() {
    let root = workspace("gazelle-idempotent-");
    write(&root, "WORKSPACE", "");
    write(
        &root,
        "crate/src/lib.rs",
        "pub fn current() {}\n\n#[test]\nfn works() {}\n",
    );
    write(&root, "crate/src/main.rs", "fn main() {}\n");
    write(&root, "crate/tests/smoke.rs", "#[test]\nfn smoke() {}\n");

    let first = run_gazelle(&root, &[]);
    assert!(
        first.status.success(),
        "first generation must pass\n{}",
        first.combined()
    );
    let first_builds = generated_builds(&root);
    let second = run_gazelle(&root, &[]);
    assert!(
        second.status.success(),
        "second generation must pass\n{}",
        second.combined()
    );
    assert_eq!(
        first_builds,
        generated_builds(&root),
        "repository generation is not idempotent"
    );
}

fn native_config_case() {
    let root = workspace("gazelle-native-");
    write(&root, "WORKSPACE", "");
    write(&root, "crate/src/lib.rs", "pub fn current() {}\n");
    write(&root, "crate/rustfmt.toml", "edition = \"2021\"\n");
    write(&root, "crate/taplo.toml", "[formatting]\n");
    write(&root, "crate/.vale.ini", "StylesPath = styles\n");
    write(
        &root,
        "crate/styles/org/Example.yml",
        "extends: existence\n",
    );

    let fresh = run_gazelle(&root, &[]);
    assert!(
        fresh.status.success(),
        "fresh generation must pass\n{}",
        fresh.combined()
    );
    let build = root.join("crate/BUILD.bazel");
    dx_testing::expect_contains(
        &build,
        &[
            "name = \"rustfmt_config\"",
            "name = \"taplo_config\"",
            "name = \"vale_config\"",
            "aspect_hints = [",
            "\":rustfmt_config\"",
            "data = [\"crate/styles/org/Example.yml\"]",
            "//crate:__subpackages__",
        ],
    )
    .expect("fresh generation pins");
    dx_testing::expect_absent(&build, &["\":taplo_config\""])
        .expect("non-binding tool leaked into aspect_hints");

    let before = std::fs::read(&build).expect("read build");
    let rerun = run_gazelle(&root, &[]);
    assert!(
        rerun.status.success(),
        "rerun must pass\n{}",
        rerun.combined()
    );
    assert_eq!(
        before,
        std::fs::read(&build).expect("read build"),
        "rerun was not idempotent"
    );

    std::fs::remove_file(root.join("crate/rustfmt.toml")).expect("remove config");
    let pruned = run_gazelle(&root, &[]);
    assert!(
        pruned.status.success(),
        "prune must pass\n{}",
        pruned.combined()
    );
    dx_testing::expect_absent(&build, &["rustfmt_config"]).expect("removed config target");
    dx_testing::expect_absent(&build, &["rustfmt"]).expect("stale rustfmt hint");
    dx_testing::expect_absent(&build, &["aspect_hints"]).expect("stale hint");
    dx_testing::expect_contains(&build, &["name = \"taplo_config\""]).expect("unrelated target");

    std::fs::remove_dir_all(root.join("crate")).expect("remove crate");
    write(
        &root,
        "broken/BUILD.bazel",
        "# gazelle:dx_native_tools rustfmt bogus\n",
    );
    let bogus = run_gazelle(&root, &[]);
    assert!(
        !bogus.status.success() && bogus.combined().contains("unknown native tool \"bogus\""),
        "unknown tool did not fail closed\n{}",
        bogus.combined()
    );
    assert!(
        !root.join("broken/BUILD").exists(),
        "failed generation wrote a BUILD file"
    );

    std::fs::remove_dir_all(root.join("broken")).expect("remove broken");
    write(
        &root,
        "ambiguous/BUILD.bazel",
        "load(\"@rules_dx//quality:native_config.bzl\", \"rustfmt_config\")\n\nrustfmt_config(\n    name = \"rustfmt_cfg\",\n    src = \"custom.toml\",\n)\n\nrustfmt_config(\n    name = \"rustfmt_extra\",\n    src = \"extra.toml\",\n)\n",
    );
    let ambiguous = run_gazelle(&root, &[]);
    assert!(
        !ambiguous.status.success() && ambiguous.combined().contains("2 config targets"),
        "ambiguous configs did not fail closed\n{}",
        ambiguous.combined()
    );
}

#[test]
fn gazelle_rust_harness() {
    let case = std::env::var("DX_GAZELLE_CASE").expect("DX_GAZELLE_CASE must select the harness");
    match case.as_str() {
        "failure" => failure_case(),
        "idempotent" => idempotent_case(),
        "native_config" => native_config_case(),
        other => panic!("unknown DX_GAZELLE_CASE: {other}"),
    }
}
