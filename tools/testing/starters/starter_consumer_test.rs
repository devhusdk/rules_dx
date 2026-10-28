//! Starts a project with `dx init`, adds one hello-world fixture, and builds and tests it as a
//! consumer of this checkout.

use dx_nested as nested;

fn assert_ok(code: Option<i32>, out: &str, err: &str) {
    assert_eq!(code, Some(0), "{out}\n{err}");
}

#[test]
fn starter_builds_and_tests_as_a_consumer() {
    let fixture = std::env::var("DX_FIXTURE").unwrap_or_else(|_| panic!("DX_FIXTURE must be set"));
    let case = nested::case(&format!("starter_{fixture}"));
    let root = nested::scaffold_consumer(case.root(), &fixture, "demo-app");
    if fixture == "mdx" {
        // The mdx fixture ships a static-site check, not a test target.
        let run = nested::bazel_in(&root, &["build", "--color=no", "--curses=no", "//..."]);
        assert_ok(run.status.code(), &run.stdout, &run.stderr);
        return;
    }
    let run = nested::bazel_in(&root, &["test", "--color=no", "--curses=no", "//..."]);
    assert_ok(run.status.code(), &run.stdout, &run.stderr);
    let text = format!("{}\n{}", run.stdout, run.stderr);
    assert!(
        text.lines()
            .any(|line| line.starts_with("//") && line.contains("PASSED")),
        "no test target passed:\n{text}"
    );
}
