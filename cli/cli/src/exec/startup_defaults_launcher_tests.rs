//! Startup-defaults tests that run the built `dx` binary, so they live in the
//! launcher layer next to the binary's other process tests.

use dx_adopt::defaults::{BOOL_SPELLINGS, DX_OUTPUT_ENV, DX_QUIET_ENV, DX_WORKSPACE_ENV};

#[test]
fn the_dx_process_turns_injected_defaults_on_and_off() {
    let scratch = dx_test_scratch::scratch("dx-defaults-process-");
    let root = scratch.path().to_path_buf();
    std::fs::create_dir(root.join(".dx")).expect("dx");
    std::fs::write(root.join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
    let config = root.join("dx.toml");
    std::fs::write(&config, "[dx]\nquiet = true\nverbose = true\n").expect("defaults");
    let run = |words: &[&str], envs: &[(&str, &str)]| {
        let mut command = assert_cmd::Command::new(dx_binary());
        command.current_dir(&root);
        for (name, value) in envs {
            command.env(name, value);
        }
        let output = command.args(words).output().expect("dx runs");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };
    let (code, _, err) = run(&["status", "--output=json"], &[]);
    assert_eq!(code, Some(0), "the config file defaults apply: {err}");
    assert!(
        err.contains("dx invocation parsed"),
        "the config file turns verbose on: {err}"
    );
    let (code, out, err) = run(&["completion", "bash"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("COMPLETE"), "{out}");
    for flag in ["--verbose=false", "-v=false"] {
        let (code, _, err) = run(&["status", flag], &[]);
        assert_eq!(code, Some(0), "{flag}: {err}");
        assert!(
            !err.contains("dx invocation parsed"),
            "{flag} turns the inherited verbose off: {err}"
        );
    }
    let (code, _, err) = run(&["status", "--output=json"], &[(DX_QUIET_ENV, "tru")]);
    assert_eq!(code, Some(2), "a typo is a usage error: {err}");
    assert!(
        err.contains("DX_QUIET=\"tru\"") && err.contains(BOOL_SPELLINGS),
        "the diagnostic names the variable and the spellings: {err}"
    );
    assert!(err.contains("Usage: dx"), "the usage line follows: {err}");
    std::fs::write(&config, "[dx]\nqiet = true\n").expect("typo");
    let (code, out, err) = run(&["status", "--output=json"], &[]);
    assert_eq!(code, Some(2), "a misspelled key is a usage error: {err}");
    assert_startup_outcome(&out, "invalid_defaults", 2);
    assert!(
        err.contains("dx.toml"),
        "the diagnostic names the file: {err}"
    );
    std::fs::write(&config, "[dx]\nquiet = true\n").expect("valid");
    let (code, out, err) = run(&["status", "--output=json"], &[(DX_QUIET_ENV, "true")]);
    assert_eq!(code, Some(0), "a documented spelling still parses: {err}");
    assert!(out.contains("\"command_finished\""), "{out}");
    let (code, _, err) = run(&["status", "--quiet=maybe"], &[]);
    assert_eq!(code, Some(2), "a bad flag value stays a usage error");
    assert!(
        err.contains("invalid value 'maybe'"),
        "a bad flag value stays a clap usage error: {err}"
    );
    scratch.close().expect("cleanup");
}

const SAMPLE_MODULE: &str = "module(name = \"sample\", version = \"0.0.0\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

fn dx_binary() -> std::path::PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    std::path::Path::new(&root)
        .join(workspace)
        .join("cli/cli/dx")
}

/// Runs the built binary in `dir` with extra environment, returning exit code,
// stdout, and stderr.
fn run_dx(
    dir: &std::path::Path,
    words: &[&str],
    envs: &[(&str, &str)],
) -> (Option<i32>, String, String) {
    let mut command = assert_cmd::Command::new(dx_binary());
    command.current_dir(dir);
    for (name, value) in envs {
        command.env(name, value);
    }
    let output = command.args(words).output().expect("dx runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Checks one pre-execution failure stream: an `error` event naming the
/// machine-readable code, then the terminal `command_finished` with the
/// process exit and incomplete results, and nothing else on stdout.
fn assert_startup_outcome(out: &str, code: &str, exit: i32) {
    let mut lines = out.lines();
    let first: serde_json::Value = serde_json::from_str(lines.next().expect("an error event"))
        .expect("every stdout line is json");
    assert_eq!(first["event"], "error", "{out}");
    assert_eq!(first["code"], code, "{out}");
    assert!(
        !first["message"].as_str().unwrap_or_default().is_empty(),
        "{out}"
    );
    let second: serde_json::Value = serde_json::from_str(lines.next().expect("a finished event"))
        .expect("every stdout line is json");
    assert_eq!(second["event"], "command_finished", "{out}");
    assert_eq!(second["exit_code"], exit, "{out}");
    assert_eq!(second["results_complete"], false, "{out}");
    assert!(
        lines.next().is_none(),
        "the terminal event ends the stream: {out}"
    );
}

fn workspace_with_output(name: &str, output: &str) -> dx_test_scratch::TempDir {
    let scratch = dx_test_scratch::scratch(name);
    std::fs::write(scratch.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(scratch.path().join(".dx")).expect("dx");
    std::fs::write(scratch.path().join(".dx/version"), "0.0.0\n").expect("pin");
    std::fs::write(
        scratch.path().join("dx.toml"),
        format!("[dx]\noutput = \"{output}\"\n"),
    )
    .expect("defaults");
    scratch
}

#[test]
fn unsupported_file_defaults_do_not_break_completion() {
    let scratch = workspace_with_output("dx-unsupported-defaults-", "json");
    std::fs::write(
        scratch.path().join("dx.toml"),
        "[dx]\noutput = \"json\"\nfail-on = \"error\"\n",
    )
    .expect("defaults");
    let (code, out, err) = run_dx(scratch.path(), &["completion", "bash"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("complete"), "{out}");
    scratch.close().expect("cleanup");
}

#[test]
fn help_works_with_a_malformed_config_but_operations_fail_closed() {
    let scratch = dx_test_scratch::scratch("dx-startup-malformed-help-");
    std::fs::write(scratch.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(scratch.path().join(".dx")).expect("dx");
    std::fs::write(scratch.path().join("dx.toml"), "not toml = [").expect("bad config");
    let root = scratch.path();
    let (code, out, _) = run_dx(root, &["--help"], &[]);
    assert_eq!(code, Some(0), "--help works with a malformed config");
    assert!(out.contains("Run Bazel workflows"), "{out}");
    let (code, out, _) = run_dx(root, &["lint", "--help"], &[]);
    assert_eq!(code, Some(0), "command help works with a malformed config");
    assert!(out.contains("dx lint"), "{out}");
    let (code, out, _) = run_dx(root, &["help", "lint"], &[]);
    assert_eq!(code, Some(0), "the help verb works with a malformed config");
    assert!(out.contains("dx lint"), "{out}");
    let (code, out, err) = run_dx(root, &["status"], &[]);
    assert_eq!(code, Some(2), "an operation still fails closed: {err}");
    assert!(out.is_empty(), "no events for a usage error: {out}");
    assert!(
        err.contains("dx.toml"),
        "the diagnostic names the file: {err}"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn startup_argument_failures_report_structured_json() {
    let scratch = dx_test_scratch::scratch("dx-startup-json-usage-");
    std::fs::write(scratch.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    let root = scratch.path();
    let (code, out, err) = run_dx(root, &["lint", ":oops", "--output=json"], &[]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    assert!(
        err.contains("Usage: dx"),
        "stderr keeps the usage line: {err}"
    );
    let (code, out, err) = run_dx(root, &["lint", ":oops"], &[(DX_OUTPUT_ENV, "json")]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    let (code, out, _) = run_dx(root, &["lint", ":oops"], &[]);
    assert_eq!(code, Some(2));
    assert!(out.is_empty(), "text mode stays event-free: {out}");
    let (code, out, _) = run_dx(root, &["lint", ":oops", "--output=xml"], &[]);
    assert_eq!(code, Some(2));
    assert!(out.is_empty(), "a junk output value stays text-only: {out}");
    for short in [
        &["lint", ":oops", "-o", "json"][..],
        &["lint", ":oops", "-ojson"],
        &["lint", ":oops", "-qojson"],
        &["lint", ":oops", "-vo", "json"],
    ] {
        let (code, out, err) = run_dx(root, short, &[]);
        assert_eq!(code, Some(2), "{err}");
        assert_startup_outcome(&out, "invalid_arguments", 2);
    }
    let (code, out, err) = run_dx(root, &["lint", ":oops", "-o", "json"], &[("CI", "true")]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    let (code, out, err) = run_dx(root, &["lint", ":oops", "-qojson"], &[("CI", "true")]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    let (code, out, _) = run_dx(
        root,
        &["lint", ":oops", "-o", "text"],
        &[(DX_OUTPUT_ENV, "json")],
    );
    assert_eq!(code, Some(2));
    assert!(
        out.is_empty(),
        "a short text flag beats the environment: {out}"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn startup_file_and_workspace_failures_report_structured_json() {
    let scratch = workspace_with_output("dx-startup-json-file-", "json");
    let (code, out, err) = run_dx(scratch.path(), &["lint", ":oops"], &[]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_arguments", 2);
    let missing = scratch
        .path()
        .join("no-such-dir")
        .to_string_lossy()
        .into_owned();
    let (code, out, err) = run_dx(scratch.path(), &["version", "--workspace", &missing], &[]);
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "unresolved_workspace", 2);
    assert!(err.contains("cannot resolve workspace"), "{err}");
}

#[test]
fn explicit_workspace_uses_the_target_workspace_defaults() {
    let a = workspace_with_output("dx-startup-ws-a-", "json");
    let b = workspace_with_output("dx-startup-ws-b-", "text");
    let b_dir = b.path().to_string_lossy().into_owned();
    let (code, out, err) = run_dx(a.path(), &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "the start directory is json: {out}"
    );
    let (code, out, err) = run_dx(a.path(), &["status", "--workspace", &b_dir], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the flag workspace supplies text defaults: {out}"
    );
    let (code, out, err) = run_dx(a.path(), &["status"], &[(DX_WORKSPACE_ENV, &b_dir)]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the environment workspace supplies text defaults: {out}"
    );
    let nested = a.path().join("sub/dir");
    std::fs::create_dir_all(&nested).expect("nested");
    let (code, out, err) = run_dx(&nested, &["status", "--workspace", &b_dir], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "a nested start still reads the flag workspace: {out}"
    );
}

#[test]
fn ci_ignores_the_environment_workspace_for_defaults() {
    let a = workspace_with_output("dx-startup-ci-a-", "json");
    let b = workspace_with_output("dx-startup-ci-b-", "text");
    let a_dir = a.path().to_string_lossy().into_owned();
    let ci = (("CI"), "true");
    let (code, out, err) = run_dx(b.path(), &["status"], &[ci, (DX_WORKSPACE_ENV, &a_dir)]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: ") && !out.contains("command_started"),
        "the ignored environment workspace leaves b's text defaults: {out}"
    );
    std::fs::write(a.path().join("dx.toml"), "[dx\noutput = \n").expect("malformed");
    let (code, out, err) = run_dx(b.path(), &["status"], &[ci, (DX_WORKSPACE_ENV, &a_dir)]);
    assert_eq!(
        code,
        Some(0),
        "a malformed ignored workspace is not read: {err}"
    );
    assert!(out.contains("toolchain: "), "{out}");
    let redirect = format!("[dx]\noutput = \"json\"\nworkspace = '{a_dir}'\n");
    let c = workspace_with_output("dx-startup-ci-c-", "json");
    std::fs::write(c.path().join("dx.toml"), redirect).expect("redirect");
    std::fs::write(a.path().join("dx.toml"), "[dx\noutput = \n").expect("malformed");
    let (code, _, err) = run_dx(b.path(), &["status"], &[ci, (DX_WORKSPACE_ENV, &a_dir)]);
    assert_eq!(code, Some(0), "{err}");
    let (code, _, err) = run_dx(
        b.path(),
        &["status", "--workspace", &a_dir],
        &[ci, (DX_WORKSPACE_ENV, &a_dir)],
    );
    assert_ne!(code, Some(0), "an explicit flag still selects a: {err}");
    std::fs::write(a.path().join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("json");
    let (code, out, err) = run_dx(
        b.path(),
        &["status", "--workspace", &a_dir],
        &[ci, (DX_WORKSPACE_ENV, &a_dir)],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("command_started"), "the flag selects a: {out}");
}

#[test]
fn repeated_workspace_flags_load_the_last_workspace_defaults() {
    let a = workspace_with_output("dx-startup-last-a-", "json");
    let b = workspace_with_output("dx-startup-last-b-", "text");
    let a_dir = a.path().to_string_lossy().into_owned();
    let b_dir = b.path().to_string_lossy().into_owned();
    let inline_a = format!("--workspace={a_dir}");
    let inline_b = format!("--workspace={b_dir}");
    let cases: Vec<Vec<&str>> = vec![
        vec!["status", "--workspace", &a_dir, "--workspace", &b_dir],
        vec!["status", &inline_a, &inline_b],
        vec!["status", "--workspace", &a_dir, &inline_b],
        vec![
            "status",
            "--output",
            "text",
            "--workspace",
            &a_dir,
            "--workspace",
            &b_dir,
        ],
    ];
    for words in cases {
        let (code, out, err) = run_dx(a.path(), &words, &[]);
        assert_eq!(code, Some(0), "{words:?}: {err}");
        assert!(
            out.contains("toolchain: ") && !out.contains("command_started"),
            "{words:?}: the last workspace supplies text defaults: {out}"
        );
    }
    let (code, out, err) = run_dx(
        b.path(),
        &["status", "--workspace", &b_dir, "--workspace", &a_dir],
        &[],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "the last workspace supplies json defaults: {out}"
    );
}

#[test]
fn overridden_workspace_with_malformed_defaults_does_not_block_the_last() {
    let a = workspace_with_output("dx-startup-bad-a-", "json");
    std::fs::write(a.path().join("dx.toml"), "[dx\noutput = \n").expect("malformed");
    let b = workspace_with_output("dx-startup-bad-b-", "text");
    let a_dir = a.path().to_string_lossy().into_owned();
    let b_dir = b.path().to_string_lossy().into_owned();
    let inline_a = format!("--workspace={a_dir}");
    let inline_b = format!("--workspace={b_dir}");
    let cases: Vec<Vec<&str>> = vec![
        vec!["status", "--workspace", &a_dir, "--workspace", &b_dir],
        vec!["status", &inline_a, &inline_b],
    ];
    for words in cases {
        let (code, out, err) = run_dx(a.path(), &words, &[]);
        assert_eq!(code, Some(0), "{words:?}: {err}");
        assert!(out.contains("toolchain: "), "{words:?}: {out}");
    }
    let (code, _, err) = run_dx(
        b.path(),
        &["status", "--workspace", &b_dir, "--workspace", &a_dir],
        &[],
    );
    assert_ne!(code, Some(0), "the malformed last workspace fails: {err}");
}

#[test]
fn bazel_run_start_selects_the_workspace_root() {
    let a = workspace_with_output("dx-startup-bwd-a-", "json");
    let b = workspace_with_output("dx-startup-bwd-b-", "text");
    let b_dir = b.path().to_string_lossy().into_owned();
    let (code, out, err) = run_dx(
        a.path(),
        &["status"],
        &[("BUILD_WORKSPACE_DIRECTORY", &b_dir)],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the Bazel start supplies text defaults: {out}"
    );
}

#[test]
fn bazel_run_here_uses_the_working_directory() {
    let ws = workspace_with_output("dx-startup-bwd-here-ws-", "json");
    let runfiles = dx_test_scratch::scratch("dx-startup-bwd-here-runfiles-");
    let outside = dx_test_scratch::scratch("dx-startup-bwd-here-outside-");
    let ws_dir = ws.path().to_string_lossy().into_owned();
    let outside_dir = outside.path().to_string_lossy().into_owned();
    let canonical_outside = outside.path().canonicalize().expect("canonical outside");
    let canonical_runfiles = runfiles.path().canonicalize().expect("canonical runfiles");
    let (code, out, err) = run_dx(
        runfiles.path(),
        &["lint", "--here", "--output=json"],
        &[
            ("BUILD_WORKSPACE_DIRECTORY", &ws_dir),
            ("BUILD_WORKING_DIRECTORY", &outside_dir),
        ],
    );
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_scope", 2);
    let json_text = |path: std::path::PathBuf| {
        serde_json::to_string(&path.display().to_string())
            .expect("path")
            .trim_matches('"')
            .to_owned()
    };
    let canonical_outside = json_text(canonical_outside);
    let canonical_runfiles = json_text(canonical_runfiles);
    assert!(
        out.contains(&canonical_outside),
        "names the working directory: {out}"
    );
    assert!(
        !out.contains(&canonical_runfiles),
        "ignores the runfiles cwd: {out}"
    );
    let (code, out, err) = run_dx(
        runfiles.path(),
        &["lint", "--here", "--output=json"],
        &[
            ("BUILD_WORKSPACE_DIRECTORY", &ws_dir),
            ("BUILD_WORKING_DIRECTORY", "relative/dir"),
        ],
    );
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "invalid_scope", 2);
    assert!(
        out.contains(&canonical_runfiles),
        "a relative value falls back to the cwd: {out}"
    );
}

#[test]
fn bazel_run_relative_workspace_uses_the_working_directory() {
    let a = workspace_with_output("dx-startup-bwd-rel-a-", "json");
    let b = workspace_with_output("dx-startup-bwd-rel-b-", "text");
    let runfiles = dx_test_scratch::scratch("dx-startup-bwd-rel-runfiles-");
    let a_dir = a.path().to_string_lossy().into_owned();
    let b_parent = b
        .path()
        .parent()
        .expect("b parent")
        .to_string_lossy()
        .into_owned();
    let b_name = b
        .path()
        .file_name()
        .expect("b name")
        .to_string_lossy()
        .into_owned();
    let bazel_env = [
        ("BUILD_WORKSPACE_DIRECTORY", a_dir.as_str()),
        ("BUILD_WORKING_DIRECTORY", b_parent.as_str()),
    ];
    let flag = format!("--workspace={b_name}");
    let (code, out, err) = run_dx(runfiles.path(), &["status", &flag], &bazel_env);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the flag starts from the working directory: {out}"
    );
    let env = [
        bazel_env[0],
        bazel_env[1],
        ("DX_WORKSPACE", b_name.as_str()),
    ];
    let (code, out, err) = run_dx(runfiles.path(), &["status"], &env);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the environment starts from the working directory: {out}"
    );
    let inner = a.path().join("inner");
    std::fs::create_dir_all(inner.join(".dx")).expect("inner dx");
    std::fs::write(inner.join("MODULE.bazel"), SAMPLE_MODULE).expect("inner module");
    std::fs::write(inner.join(".dx/version"), "0.0.0\n").expect("inner pin");
    std::fs::write(inner.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("inner defaults");
    std::fs::write(
        a.path().join("dx.toml"),
        "[dx]\noutput = \"json\"\nworkspace = \"inner\"\n",
    )
    .expect("redirect");
    let (code, out, err) = run_dx(runfiles.path(), &["status"], &bazel_env);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("toolchain: "),
        "the file key starts from the workspace start: {out}"
    );
}

#[test]
fn new_and_completion_run_without_a_workspace() {
    let scratch = dx_test_scratch::scratch("dx-startup-outside-");
    let root = scratch.path();
    let (code, out, err) = run_dx(root, &["new", "--apply", "rust", "demo"], &[]);
    assert_eq!(
        code,
        Some(0),
        "standalone new works outside a workspace: {err}"
    );
    assert!(out.contains("demo/Cargo.toml"), "{out}");
    assert!(root.join("demo/Cargo.toml").exists());
    let (code, out, err) = run_dx(root, &["completion", "bash"], &[]);
    assert_eq!(code, Some(0), "completion works outside a workspace: {err}");
    assert!(out.contains("COMPLETE"), "{out}");
    scratch.close().expect("cleanup");
}

#[test]
fn removed_new_spellings_exit_2_with_canonical_name() {
    let scratch = dx_test_scratch::scratch("dx-new-removed-");
    let root = scratch.path();
    for (spelling, canonical) in [
        ("cc", "cpp"),
        ("c#", "csharp"),
        ("c-sharp", "csharp"),
        ("f#", "fsharp"),
        ("f-sharp", "fsharp"),
    ] {
        for apply in [false, true] {
            let mut words = vec!["new"];
            if apply {
                words.push("--apply");
            }
            words.extend([spelling, "demo"]);
            let (code, out, err) = run_dx(root, &words, &[]);
            assert_eq!(code, Some(2), "{words:?}: {err}");
            assert!(out.is_empty(), "{words:?}: {out}");
            assert!(
                err.contains(&format!(
                    "language spelling for dx new was removed: {spelling} (use {canonical})"
                )),
                "{words:?}: {err}"
            );
            assert!(!root.join("demo").exists(), "{words:?} wrote demo");
        }
    }
    let (code, out, err) = run_dx(root, &["new", "--apply", "c", "demo"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("wrote demo/hello.c\n"), "{out}");
    assert!(root.join("demo/hello.c").is_file());
    assert!(!root.join("demo/hello.cc").exists());
    scratch.close().expect("cleanup");
}

#[test]
fn config_directed_workspace_loads_the_target_defaults_once() {
    let b = workspace_with_output("dx-startup-redir-b-", "json");
    let b_dir = b.path().to_string_lossy().into_owned();
    let a = dx_test_scratch::scratch("dx-startup-redir-a-");
    std::fs::write(a.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(a.path().join(".dx")).expect("dx");
    std::fs::write(a.path().join(".dx/version"), "0.0.0\n").expect("pin");
    std::fs::write(
        a.path().join("dx.toml"),
        format!("[dx]\nworkspace = '{b_dir}'\n"),
    )
    .expect("redirect");
    let (code, out, err) = run_dx(a.path(), &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "the redirect target supplies json defaults: {out}"
    );
    let a_dir = a.path().to_string_lossy().into_owned();
    std::fs::write(
        b.path().join("dx.toml"),
        format!("[dx]\noutput = \"text\"\nworkspace = '{a_dir}'\n"),
    )
    .expect("cycle");
    let (code, out, err) = run_dx(a.path(), &["status"], &[]);
    assert_eq!(code, Some(0), "a redirect cycle still terminates: {err}");
    assert!(
        out.contains("toolchain: "),
        "the frozen target supplies text defaults: {out}"
    );
}

#[test]
fn relative_config_redirects_resolve_once_against_the_start_directory() {
    let outer = dx_test_scratch::scratch("dx-startup-once-");
    let root = outer.path();
    std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("outer");
    let repo = root.join("repo");
    std::fs::create_dir(&repo).expect("repo");
    std::fs::write(repo.join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(repo.join(".dx")).expect(".dx");
    std::fs::write(repo.join(".dx/version"), "0.0.0\n").expect("pin");
    std::fs::write(repo.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("repo");
    let sub = repo.join("sub");
    std::fs::create_dir(&sub).expect("sub");
    std::fs::write(sub.join("dx.toml"), "[dx]\nworkspace = \"..\"\n").expect("sub");
    let (code, out, err) = run_dx(&sub, &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("command_started"), "repo json defaults: {out}");

    let repo_dir = repo.to_string_lossy().into_owned();
    std::fs::write(
        sub.join("dx.toml"),
        format!("[dx]\nworkspace = '{repo_dir}'\n"),
    )
    .expect("absolute");
    let (code, out, err) = run_dx(&sub, &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("command_started"), "absolute redirect: {out}");

    std::fs::write(sub.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("text");
    std::fs::write(repo.join("dx.toml"), "[dx]\nworkspace = \"nested\"\n").expect("hop");
    let nested = repo.join("nested");
    std::fs::create_dir(&nested).expect("nested");
    std::fs::write(nested.join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(nested.join(".dx")).expect(".dx");
    std::fs::write(nested.join(".dx/version"), "0.0.0\n").expect("pin");
    std::fs::write(
        nested.join("dx.toml"),
        "[dx]\noutput = \"json\"\nworkspace = \"..\"\n",
    )
    .expect("nested");
    let (code, out, err) = run_dx(&repo, &["status"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains("command_started"),
        "a relative child redirect loads the child defaults, one hop only: {out}"
    );
    outer.close().expect("cleanup");
}

#[test]
fn removed_dry_run_is_rejected_from_every_source() {
    let scratch = workspace_with_output("dx-removed-dry-run-", "text");
    let root = scratch.path();
    for words in [
        vec!["lint", "--dry-run"],
        vec!["lint", "--dry-run=true"],
        vec!["lint", "--dry-run=false"],
        vec!["fix", "--dry-run", "--apply"],
    ] {
        let (code, out, err) = run_dx(root, &words, &[]);
        assert_eq!(code, Some(2), "{words:?}: {err}");
        assert!(out.is_empty(), "{words:?}: {out}");
        assert!(err.contains("--dry-run"), "{words:?}: {err}");
    }
    for value in ["true", "false", "1"] {
        let (code, _, err) = run_dx(root, &["lint"], &[("DX_DRY_RUN", value)]);
        assert_eq!(code, Some(2), "DX_DRY_RUN={value}: {err}");
        assert!(err.contains("DX_DRY_RUN was removed"), "{err}");
    }
    for key in ["dry_run", "dry-run"] {
        for value in ["true", "false"] {
            std::fs::write(root.join("dx.toml"), format!("[dx]\n{key} = {value}\n"))
                .expect("config");
            let (code, _, err) = run_dx(root, &["lint"], &[]);
            assert_eq!(code, Some(2), "{key}={value}: {err}");
            assert!(err.contains("was removed"), "{err}");
        }
    }
    scratch.close().expect("cleanup");
}

#[test]
fn log_level_config_keys_are_rejected() {
    let scratch = workspace_with_output("dx-log-level-config-", "text");
    let root = scratch.path();
    for key in ["log_level", "log-level"] {
        std::fs::write(root.join("dx.toml"), format!("[dx]\n{key} = \"debug\"\n")).expect("config");
        let (code, out, err) = run_dx(root, &["lint"], &[]);
        assert_eq!(code, Some(2), "{key}: {err}");
        assert!(out.is_empty(), "{key}: {out}");
        assert!(err.contains(key.split('-').next().unwrap()), "{key}: {err}");
    }
    scratch.close().expect("cleanup");
}

#[test]
fn log_level_flag_exits_two_before_running() {
    let scratch = workspace_with_output("dx-log-level-flag-", "text");
    let root = scratch.path();
    for words in [
        vec!["lint", "--log-level=debug"],
        vec!["lint", "-v", "--log-level", "trace"],
    ] {
        let (code, out, err) = run_dx(root, &words, &[]);
        assert_eq!(code, Some(2), "{words:?}: {err}");
        assert!(out.is_empty(), "{words:?}: {out}");
        assert!(err.contains("--log-level"), "{words:?}: {err}");
    }
    scratch.close().expect("cleanup");
}

#[test]
fn rust_log_does_not_change_dx_diagnostics() {
    let scratch = dx_test_scratch::scratch("dx-rust-log-");
    let root = scratch.path();
    let marker = "dx invocation parsed";
    for filter in ["trace", "info", "dx=trace", "off", "not a filter [", ""] {
        let (code, _, err) = run_dx(root, &["completion", "bash"], &[("RUST_LOG", filter)]);
        assert_eq!(code, Some(0), "RUST_LOG={filter}: {err}");
        assert!(
            !err.contains(marker),
            "default ignores RUST_LOG={filter}: {err}"
        );
        let (code, _, err) = run_dx(
            root,
            &["completion", "bash", "--verbose"],
            &[("RUST_LOG", filter)],
        );
        assert_eq!(code, Some(0), "RUST_LOG={filter}: {err}");
        assert!(
            err.contains(marker),
            "verbose ignores RUST_LOG={filter}: {err}"
        );
    }
    let (code, _, err) = run_dx(root, &["completion", "bash", "--verbose"], &[]);
    assert_eq!(code, Some(0), "{err}");
    assert!(err.contains(marker), "{err}");
    scratch.close().expect("cleanup");
}

#[test]
fn no_color_controls_auto_color_and_explicit_modes_win() {
    let scratch = dx_test_scratch::scratch("dx-no-color-");
    let root = scratch.path();
    let verbose = ["completion", "bash", "--verbose"];
    let escape = '\u{1b}';
    for (no_color, color, ansi) in [
        (None, "auto", false),
        (Some("1"), "auto", false),
        (Some(""), "auto", false),
        (Some("1"), "never", false),
        (Some(""), "always", true),
        (Some("1"), "always", true),
        (None, "always", true),
    ] {
        let color_arg = format!("--color={color}");
        let mut words = verbose.to_vec();
        words.push(&color_arg);
        let mut envs = Vec::new();
        if let Some(value) = no_color {
            envs.push(("NO_COLOR", value));
        }
        let (code, out, err) = run_dx(root, &words, &envs);
        assert_eq!(code, Some(0), "{no_color:?} {color}: {err}");
        assert_eq!(
            err.contains(escape),
            ansi,
            "NO_COLOR={no_color:?} --color={color}: {err:?}"
        );
        assert!(!out.contains(escape), "stdout stays plain: {out:?}");
    }
    scratch.close().expect("cleanup");
}

#[test]
fn bad_build_slots_exit_two_from_the_binary() {
    let scratch = workspace_with_output("dx-bad-build-slots-", "text");
    let root = scratch.path();
    let (code, out, err) = run_dx(root, &["status", "--build-slots=0"], &[]);
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty(), "{out}");
    assert!(err.contains("--build-slots"), "{err}");
    let (code, _, err) = run_dx(root, &["status"], &[("DX_BUILD_SLOTS", "many")]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("DX_BUILD_SLOTS"), "{err}");
}
