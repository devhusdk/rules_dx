//! Scaffolds one `dx new` starter and builds and tests it as a consumer of this checkout.

use dx_nested as nested;

fn assert_ok(code: Option<i32>, out: &str, err: &str) {
    assert_eq!(code, Some(0), "{out}\n{err}");
}

#[test]
fn starter_builds_and_tests_as_a_consumer() {
    let starter = std::env::var("DX_STARTER").unwrap_or_else(|_| panic!("DX_STARTER must be set"));
    let case = nested::case(&format!("starter_{starter}"));
    let root = nested::scaffold_starter(case.root(), &starter, "demo-app");
    if starter == "mdx" {
        // The mdx starter ships a static-site check, not a test target.
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
