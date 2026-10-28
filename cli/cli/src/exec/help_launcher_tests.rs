//! Help and usage-error tests that run the built `dx` binary, so they live
//! in the launcher layer next to the binary's other process tests.

use dx_plan::args::help::usage_banner;
use dx_plan::args::Command;

fn banner() -> String {
    usage_banner()
}

#[test]
fn help_process_exits_zero_with_brand() {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    let binary = std::path::Path::new(&root)
        .join(workspace)
        .join("cli/cli/dx");
    assert_cmd::Command::new(binary)
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Run Bazel workflows"));
}

#[test]
fn usage_errors_print_the_command_usage() {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    let binary = std::path::Path::new(&root)
        .join(workspace)
        .join("cli/cli/dx");
    let scratch = dx_test_scratch::scratch("usage-banner-");
    std::fs::write(scratch.path().join("MODULE.bazel"), "").expect("scratch workspace");
    let rendered = |argv: &[&str]| -> String {
        let output = assert_cmd::Command::new(&binary)
            .current_dir(scratch.path())
            .args(argv)
            .assert()
            .code(2)
            .get_output()
            .stderr
            .clone();
        String::from_utf8(output).expect("stderr is utf-8")
    };
    let parse_error = rendered(&["--nope", "build"]);
    let pre_exec_error = rendered(&["version", "--check", "--apply"]);
    for (label, text, command) in [
        ("parse", &parse_error, Command::Build),
        ("pre-exec", &pre_exec_error, Command::Version),
    ] {
        assert!(
            text.contains(command.usage()),
            "the {label} usage error does not print the command usage:\n{text}"
        );
        assert!(
            !text.contains("usage: dx <"),
            "the {label} usage error prints the global banner:\n{text}"
        );
        assert_eq!(
            text.matches(command.usage()).count(),
            1,
            "the {label} usage error prints the usage more than once:\n{text}"
        );
    }
    let no_command = rendered(&["--nope"]);
    assert!(
        no_command.contains(&banner()),
        "a parse error with no command prints the global banner:\n{no_command}"
    );
}

#[test]
fn clap_renders_the_error_and_supplies_the_suggestion() {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    let binary = std::path::Path::new(&root)
        .join(workspace)
        .join("cli/cli/dx");
    let scratch = dx_test_scratch::scratch("clap-error-");
    std::fs::write(scratch.path().join("MODULE.bazel"), "").expect("scratch workspace");
    let rendered = |argv: &[&str]| -> String {
        let output = assert_cmd::Command::new(&binary)
            .current_dir(scratch.path())
            .args(argv)
            .assert()
            .code(2)
            .get_output()
            .stderr
            .clone();
        String::from_utf8(output).expect("stderr is utf-8")
    };
    for (argv, needle) in [
        (
            vec!["lintt"],
            "some similar subcommands exist: 'init', 'lint'",
        ),
        (
            vec!["lint", "--ouptut=json"],
            "a similar argument exists: '--output'",
        ),
    ] {
        let text = rendered(&argv);
        assert!(
            text.contains(needle),
            "dx {argv:?} lost clap's suggestion:\n{text}"
        );
        assert!(
            !text.contains("did you mean"),
            "dx {argv:?} still renders its own suggestion:\n{text}"
        );
        assert_eq!(
            text.matches("sage: dx").count(),
            1,
            "dx {argv:?} must print one usage line:\n{text}"
        );
        assert!(
            !text.contains("Usage: dx <") && !text.contains("clap"),
            "dx {argv:?} prints the wrong usage:\n{text}"
        );
    }
    for (argv, name) in [
        (vec!["list-tests", "//..."], "list-tests"),
        (vec!["tests", "--configured", "//pkg/..."], "tests"),
    ] {
        let text = rendered(&argv);
        assert!(
            text.contains(&format!("unsupported command '{name}'")),
            "dx {argv:?} must name the removed command:\n{text}"
        );
        assert!(
            text.contains("bazel query 'tests("),
            "dx {argv:?} must carry the bazel query tip:\n{text}"
        );
    }
}
