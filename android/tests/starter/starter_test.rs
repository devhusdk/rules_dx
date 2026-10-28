//! `dx init` plus the rust-android fixture makes a consumer whose host targets build and test, also when the
//! consumer renames rules_dx, and whose adb launchers analyze without an SDK. With
//! `ANDROID_HOME`, `ANDROID_SDK_ROOT` and `ANDROID_NDK_REVISION` set it also builds the APK.

use std::path::{Path, PathBuf};

use dx_nested as nested;

const NAME: &str = "demo-app";

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn bazel(root: &Path, args: &[&str]) -> (Option<i32>, String) {
    let run = nested::bazel_in(root, args);
    (run.status.code(), format!("{}{}", run.stdout, run.stderr))
}

fn scaffold(scratch: &Path) -> PathBuf {
    let root = nested::scaffold_fixture(scratch, "rust-android", NAME);
    for file in [
        "Cargo.toml",
        "src/lib.rs",
        "activity.c",
        "res/values/strings.xml",
        "BUILD.bazel",
        "MODULE.bazel",
        ".bazelrc",
    ] {
        assert!(root.join(file).is_file(), "{file}");
    }
    let rules = dx_testing::rules_dx_source();
    let module = read(&root.join("MODULE.bazel"));
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "{module}local_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    let bazelrc = read(&root.join(".bazelrc"));
    write(
        &root.join(".bazelrc"),
        &format!("{bazelrc}{}", dx_testing::preset_rc(&rules)),
    );
    root
}

fn rename_rules_dx(root: &Path) {
    let module = read(&root.join("MODULE.bazel"));
    let dep = "bazel_dep(name = \"rules_dx\", version = ";
    let at = module.find(dep).expect("module depends on rules_dx");
    let end = at + module[at..].find(")\n").expect("bazel_dep closes");
    let renamed = format!("{}, repo_name = \"dxr\"{}", &module[..end], &module[end..])
        .replace("@rules_dx//", "@dxr//");
    write(&root.join("MODULE.bazel"), &renamed);
    let build = read(&root.join("BUILD.bazel"));
    assert!(build.contains("@rules_dx//"), "{build}");
    write(
        &root.join("BUILD.bazel"),
        &build.replace("@rules_dx//", "@dxr//"),
    );
}

fn sdk_present() -> bool {
    ["ANDROID_HOME", "ANDROID_SDK_ROOT", "ANDROID_NDK_REVISION"]
        .iter()
        .all(|var| std::env::var(var).is_ok_and(|value| !value.is_empty()))
}

fn assert_host_and_launchers(root: &Path) {
    let (code, text) = bazel(root, &["test", "--color=no", "--curses=no", "//..."]);
    assert_eq!(code, Some(0), "{text}");
    for test in ["test", "fmt_test", "clippy_test"] {
        assert!(
            text.lines()
                .any(|line| line.starts_with(&format!("//:{NAME}_{test}"))
                    && line.contains("PASSED")),
            "{test}\n{text}"
        );
    }
    let start = format!(":{NAME}_start");
    let log = format!(":{NAME}_log");
    let (code, text) = bazel(root, &["build", "--nobuild", &start, &log]);
    assert_eq!(code, Some(0), "{text}");
    if sdk_present() {
        let apk = format!(":{NAME}_apk_file");
        let install = format!(":{NAME}_install");
        let check = format!(":{NAME}_check_test");
        let (code, text) = bazel(root, &["build", "--config=android", &apk, &install]);
        assert_eq!(code, Some(0), "{text}");
        let (code, text) = bazel(root, &["test", "--config=android", &check]);
        assert_eq!(code, Some(0), "{text}");
    }
}

#[test]
fn dx_init_with_the_rust_android_fixture_builds_and_tests_with_plain_and_renamed_rules_dx() {
    let case = nested::case("starter_rust_android");
    let root = scaffold(case.root());
    assert_host_and_launchers(&root);
    rename_rules_dx(&root);
    assert_host_and_launchers(&root);
}
