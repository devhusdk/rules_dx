use std::path::Path;

use dx_nested as nested;

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn bazel(root: &Path, args: &[&str]) -> (Option<i32>, String, String) {
    let run = nested::bazel_in(root, args);
    (run.status.code(), run.stdout, run.stderr)
}

#[test]
fn dx_init_with_a_erlang_fixture_builds_runs_and_tests() {
    let case = nested::case("starter");
    let scratch = case.root().to_path_buf();
    let root = nested::scaffold_fixture(&scratch, "erlang", "demo-app");
    for file in [
        "MODULE.bazel",
        "BUILD.bazel",
        "src/hello.erl",
        "src/hello_cli.erl",
        "test/hello_tests.erl",
    ] {
        assert!(root.join(file).is_file(), "{file}");
    }

    let rules = dx_testing::rules_dx_source();
    let module = read(&root.join("MODULE.bazel"));
    assert!(module.contains("bazel_dep(name = \"rules_dx\""), "{module}");
    std::fs::write(
        root.join("MODULE.bazel"),
        format!(
            "{module}local_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    )
    .unwrap_or_else(|error| panic!("write MODULE.bazel: {error}"));
    let bazelrc = read(&root.join(".bazelrc"));
    std::fs::write(
        root.join(".bazelrc"),
        format!("{bazelrc}{}", dx_testing::preset_rc(&rules)),
    )
    .unwrap_or_else(|error| panic!("write .bazelrc: {error}"));

    let (code, out, err) = bazel(&root, &["test", "--color=no", "--curses=no", "//..."]);
    assert_eq!(code, Some(0), "{out}{err}");
    assert!(out.contains("//:demo-app_test"), "{out}{err}");
    assert!(out.contains("PASSED"), "{out}{err}");

    let (code, out, err) = bazel(
        &root,
        &["run", "--color=no", "--curses=no", ":demo-app_bin"],
    );
    assert_eq!(code, Some(0), "{out}{err}");
    assert_eq!(out, "hello world\n", "{err}");
}
