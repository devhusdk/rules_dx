use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_jvm_external", version = "7.1")

maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(
    name = "maven",
    artifacts = ["javax.inject:javax.inject:1"],
    lock_file = "//third_party:maven_install.json",
    repositories = ["https://repo1.maven.org/maven2"],
)
maven.install(
    name = "tool_maven",
    artifacts = ["com.google.code.findbugs:jsr305:3.0.2"],
    lock_file = "//tools/jvm:tool_install.json",
    repositories = ["https://repo1.maven.org/maven2"],
)
use_repo(maven, "maven", "tool_maven")
"#;

const DX_TOML: &str = r#"schema_version = 1

[[dependency_set]]
name = "jvm"
ecosystem = "maven"
manifests = ["MODULE.bazel"]
locks = ["third_party/maven_install.json"]
scopes = ["third_party"]
repository = "maven"

[[dependency_set]]
name = "tools"
ecosystem = "maven"
manifests = ["MODULE.bazel"]
locks = ["tools/jvm/tool_install.json"]
scopes = ["tools/jvm"]
repository = "tool_maven"
"#;

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("cli/cli/tests/maven_sets/tests/maven_sets.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(6)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn consumer() -> Case {
    let root = nested::case("maven-sets");
    let version = std::fs::read_to_string(rules_dx_root().join(".bazelversion"))
        .unwrap_or_else(|error| panic!(".bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(&root.join("MODULE.bazel"), MODULE);
    write(&root.join("BUILD.bazel"), "");
    write(
        &root.join(".bazelrc"),
        "build:dx_dev --compilation_mode=fastbuild\n",
    );
    write(&root.join(".dx/version"), "0.0.0\n");
    write(&root.join("dx.toml"), DX_TOML);
    for (dir, lock) in [
        ("third_party", "maven_install.json"),
        ("tools/jvm", "tool_install.json"),
    ] {
        write(
            &root.join(dir).join("BUILD.bazel"),
            &format!("exports_files([\"{lock}\"])\n"),
        );
        write(&root.join(dir).join(lock), "");
    }
    root
}

fn dx_in(root: &Case, args: &[&str], env: &[(&str, &str)]) -> Run {
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = args.to_vec();
    argv.push("--workspace");
    argv.push(workspace.as_str());
    root.run(&dx(), &argv, env)
}

fn report(run: &Run) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        run.status.code(),
        run.stdout,
        run.stderr
    )
}

#[test]
fn maven_sets_pin_and_check_one_hub_at_a_time() {
    let root = consumer();
    let app_lock = root.join("third_party/maven_install.json");
    let tool_lock = root.join("tools/jvm/tool_install.json");

    let run = dx_in(&root, &["update", "--apply"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    let app_text = String::from_utf8(read(&app_lock)).expect("utf-8 lock");
    assert!(app_text.contains("javax.inject:javax.inject"), "{app_text}");
    let tool_text = String::from_utf8(read(&tool_lock)).expect("utf-8 lock");
    assert!(tool_text.contains("jsr305"), "{tool_text}");

    let run = dx_in(&root, &["update", "--check"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    assert!(
        run.stdout.contains("jvm lockfile current"),
        "{}",
        report(&run)
    );
    assert!(
        run.stdout.contains("tools lockfile current"),
        "{}",
        report(&run)
    );

    let before = (read(&app_lock), read(&tool_lock));
    for (key, value) in [
        ("REPIN", "1"),
        ("REPIN", "false"),
        ("RULES_JVM_EXTERNAL_REPIN", "1"),
        ("COURSIER_MIRRORS", "/nonexistent"),
    ] {
        let run = dx_in(&root, &["update", "--check"], &[(key, value)]);
        assert_ne!(run.status.code(), Some(0), "{key}: {}", report(&run));
        assert!(
            !run.stdout.contains("lockfile current"),
            "{key}: {}",
            report(&run)
        );
        assert_eq!((read(&app_lock), read(&tool_lock)), before, "{key} wrote");
    }

    let edited = MODULE.replace(
        "artifacts = [\"javax.inject:javax.inject:1\"]",
        "artifacts = [\"javax.inject:javax.inject:1\", \"org.apache.commons:commons-lang3:3.14.0\"]",
    );
    write(&root.join("MODULE.bazel"), &edited);
    let run = dx_in(&root, &["update", "--check"], &[]);
    assert_ne!(run.status.code(), Some(0), "{}", report(&run));
    assert!(
        !run.stdout.contains("jvm lockfile current"),
        "{}",
        report(&run)
    );
    assert_eq!((read(&app_lock), read(&tool_lock)), before, "--check wrote");

    let run = dx_in(&root, &["update", "--apply", "jvm"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
    let app_text = String::from_utf8(read(&app_lock)).expect("utf-8 lock");
    assert!(app_text.contains("commons-lang3"), "{app_text}");
    assert_eq!(read(&tool_lock), before.1, "tools lock changed");

    let run = dx_in(&root, &["update", "--check"], &[]);
    assert_eq!(run.status.code(), Some(0), "{}", report(&run));
}
