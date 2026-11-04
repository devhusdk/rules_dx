//! Starts a project with `dx init`, adds the C hello-world fixture, and builds and tests it as a
//! consumer of this checkout.

use std::path::Path;

use dx_nested as nested;

fn bazel_ok(root: &Path, args: &[&str]) -> String {
    let mut argv = vec![args[0], "--color=no", "--curses=no"];
    argv.extend_from_slice(&args[1..]);
    let run = nested::bazel_in(root, &argv);
    assert_eq!(
        run.status.code(),
        Some(0),
        "bazel {args:?}\n{}\n{}",
        run.stdout,
        run.stderr
    );
    run.stdout
}

fn assert_llvm_compiles(root: &Path, ext: &str, compiler: &str) {
    let out = bazel_ok(
        root,
        &[
            "aquery",
            "--output=text",
            "mnemonic(\"CppCompile\", deps(//:demo-app))",
        ],
    );
    let head = format!("action 'Compiling hello.{ext}'");
    let action = out
        .split("\n\n")
        .find(|action| action.contains(&head))
        .unwrap_or_else(|| panic!("no compile of hello.{ext}:\n{out}"));
    assert!(
        action.lines().any(|line| {
            let line = line.replace('\\', "/");
            let line = line.trim_end().trim_end_matches('\\').trim_end();
            let line = line.strip_suffix(".exe").unwrap_or(line);
            line.contains("external/llvm+") && line.ends_with(&format!("/bin/{compiler}"))
        }),
        "{action}"
    );
    if ext == "c" {
        assert!(!action.contains("-std=c++"), "{action}");
    }
}

#[test]
fn starter_builds_and_tests_as_a_consumer() {
    let case = nested::case("starter_c");
    let root = nested::scaffold_consumer(case.root(), "c", "demo-app");
    let run = nested::bazel_in(&root, &["test", "--color=no", "--curses=no", "//..."]);
    let text = format!("{}\n{}", run.stdout, run.stderr);
    assert_eq!(run.status.code(), Some(0), "{text}");
    assert!(
        text.lines()
            .any(|line| line.starts_with("//:demo-app_test") && line.contains("PASSED")),
        "//:demo-app_test did not pass:\n{text}"
    );
    let out = bazel_ok(&root, &["run", ":demo-app_bin"]);
    assert!(out.contains("hello world"), "printed {out:?}");
    assert_llvm_compiles(&root, "c", "clang");
}
