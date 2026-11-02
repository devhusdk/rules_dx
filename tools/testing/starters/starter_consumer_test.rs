//! Starts a project with `dx init`, adds one hello-world fixture, and builds and tests it as a
//! consumer of this checkout.

use std::path::Path;

use dx_nested as nested;

/// What one fixture checks beyond `bazel test //...`.
struct Starter {
    fixture: &'static str,
    build_only: bool,
    test_target: Option<&'static str>,
    run: Option<(&'static str, &'static str)>,
    format_check: Option<(&'static str, &'static [&'static str])>,
    broken: Option<(&'static str, &'static str)>,
}

const PLAIN: Starter = Starter {
    fixture: "",
    build_only: false,
    test_target: None,
    run: None,
    format_check: None,
    broken: None,
};

const STARTERS: &[Starter] = &[
    Starter {
        fixture: "mdx",
        build_only: true,
        ..PLAIN
    },
    Starter {
        fixture: "elixir",
        test_target: Some("//:demo-app_test"),
        run: Some((":demo-app_bin", "hello world")),
        ..PLAIN
    },
    Starter {
        fixture: "erlang",
        test_target: Some("//:demo-app_test"),
        run: Some((":demo-app_bin", "hello world")),
        ..PLAIN
    },
    Starter {
        fixture: "haskell",
        test_target: Some("//:demo-app_test"),
        run: Some((":demo-app_bin", "hello")),
        broken: Some(("src/Hello.hs", "module Hello where\nthis is not haskell\n")),
        ..PLAIN
    },
    Starter {
        fixture: "julia",
        test_target: Some("//:DemoApp_test"),
        run: Some((":DemoApp_bin", "hello world")),
        format_check: Some((
            "@rules_dx//julia/tools:julia_format",
            &[
                "Project.toml",
                "src/DemoApp.jl",
                "main.jl",
                "test/runtests.jl",
            ],
        )),
        ..PLAIN
    },
];

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

#[test]
fn starter_builds_and_tests_as_a_consumer() {
    let fixture = std::env::var("DX_FIXTURE").unwrap_or_else(|_| panic!("DX_FIXTURE must be set"));
    let starter = STARTERS
        .iter()
        .find(|starter| starter.fixture == fixture)
        .unwrap_or(&PLAIN);
    let case = nested::case(&format!("starter_{fixture}"));
    let root = nested::scaffold_consumer(case.root(), &fixture, "demo-app");
    if starter.build_only {
        bazel_ok(&root, &["build", "//..."]);
        return;
    }

    let run = nested::bazel_in(&root, &["test", "--color=no", "--curses=no", "//..."]);
    let text = format!("{}\n{}", run.stdout, run.stderr);
    assert_eq!(run.status.code(), Some(0), "{text}");
    assert!(
        text.lines()
            .any(|line| line.starts_with("//") && line.contains("PASSED")),
        "no test target passed:\n{text}"
    );
    if let Some(target) = starter.test_target {
        assert!(text.contains(target), "{target} did not run:\n{text}");
    }

    if let Some((target, expected)) = starter.run {
        let out = bazel_ok(&root, &["run", target]);
        assert!(out.contains(expected), "{target} printed {out:?}");
    }

    if let Some((tool, files)) = starter.format_check {
        let paths: Vec<String> = files
            .iter()
            .map(|file| root.join(file).display().to_string())
            .collect();
        let mut argv = vec!["run", tool, "--", "--check"];
        argv.extend(paths.iter().map(String::as_str));
        bazel_ok(&root, &argv);
    }

    if let Some((file, body)) = starter.broken {
        std::fs::write(root.join(file), body)
            .unwrap_or_else(|error| panic!("write {file}: {error}"));
        let run = nested::bazel_in(&root, &["build", "--color=no", "--curses=no", "//..."]);
        assert_ne!(
            run.status.code(),
            Some(0),
            "broken {file} must fail the build\n{}\n{}",
            run.stdout,
            run.stderr
        );
    }
}
