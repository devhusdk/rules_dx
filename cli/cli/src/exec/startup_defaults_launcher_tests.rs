//! Startup-defaults tests that run the built `dx` binary, so they live in the
//! launcher layer next to the binary's other process tests.

use dx_adopt::defaults::{BOOL_SPELLINGS, DX_OUTPUT_ENV, DX_QUIET_ENV, DX_VERBOSE_ENV};

#[test]
fn the_dx_process_turns_injected_defaults_on_and_off() {
    let scratch = dx_test_scratch::scratch("dx-defaults-process-");
    let root = scratch.path().to_path_buf();
    std::fs::create_dir(root.join(".dx")).expect("dx");
    std::fs::write(root.join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::write(root.join(".dx/pins"), "0.0.0\n").expect("pin");
    let run = |words: &[&str], envs: &[(&str, &str)]| {
        let mut command = assert_cmd::Command::new(dx_binary());
        command.current_dir(&root);
        command
            .env(DX_QUIET_ENV, "true")
            .env(DX_VERBOSE_ENV, "true");
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
    assert_eq!(code, Some(0), "the environment defaults apply: {err}");
    assert!(
        err.contains("dx invocation parsed"),
        "the environment turns verbose on: {err}"
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

fn sample_workspace(name: &str) -> dx_test_scratch::TempDir {
    let scratch = dx_test_scratch::scratch(name);
    std::fs::write(scratch.path().join("MODULE.bazel"), SAMPLE_MODULE).expect("module");
    std::fs::create_dir(scratch.path().join(".dx")).expect("dx");
    std::fs::write(scratch.path().join(".dx/pins"), "0.0.0\n").expect("pin");
    scratch
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
fn startup_workspace_failures_report_structured_json() {
    let scratch = sample_workspace("dx-startup-json-ws-");
    let missing = scratch
        .path()
        .join("no-such-dir")
        .to_string_lossy()
        .into_owned();
    let (code, out, err) = run_dx(
        scratch.path(),
        &["status", "--output=json", "--workspace", &missing],
        &[],
    );
    assert_eq!(code, Some(2), "{err}");
    assert_startup_outcome(&out, "unresolved_workspace", 2);
    assert!(err.contains("cannot resolve workspace"), "{err}");
}

#[test]
fn bazel_run_start_selects_the_workspace_root() {
    let a = dx_test_scratch::scratch("dx-startup-bwd-a-");
    let b = sample_workspace("dx-startup-bwd-b-");
    let b_dir = b.path().to_string_lossy().into_owned();
    let (code, _, err) = run_dx(
        a.path(),
        &["status"],
        &[("BUILD_WORKSPACE_DIRECTORY", &b_dir)],
    );
    assert_eq!(
        code,
        Some(0),
        "the Bazel start selects the workspace: {err}"
    );
}

#[test]
fn bazel_run_here_uses_the_working_directory() {
    let ws = sample_workspace("dx-startup-bwd-here-ws-");
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
    let a = dx_test_scratch::scratch("dx-startup-bwd-rel-a-");
    let b = sample_workspace("dx-startup-bwd-rel-b-");
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
    let (code, _, err) = run_dx(runfiles.path(), &["status", &flag], &bazel_env);
    assert_eq!(
        code,
        Some(0),
        "the flag starts from the working directory: {err}"
    );
    let env = [
        bazel_env[0],
        bazel_env[1],
        ("DX_WORKSPACE", b_name.as_str()),
    ];
    let (code, _, err) = run_dx(runfiles.path(), &["status"], &env);
    assert_eq!(
        code,
        Some(0),
        "the environment starts from the working directory: {err}"
    );
}

#[test]
fn init_and_completion_run_without_a_workspace() {
    let scratch = dx_test_scratch::scratch("dx-startup-outside-");
    let root = scratch.path();
    let start = root.to_string_lossy();
    let (code, out, err) = run_dx(
        root,
        &["init", "--workspace", start.as_ref(), "--apply", "demo"],
        &[],
    );
    assert_eq!(
        code,
        Some(0),
        "init into a new directory works outside a workspace: {err}"
    );
    assert!(out.contains("wrote demo/MODULE.bazel\n"), "{out}");
    assert!(root.join("demo/MODULE.bazel").is_file());
    assert!(root.join("demo/.dx/pins").is_file());
    let (code, out, err) = run_dx(root, &["completion", "bash"], &[]);
    assert_eq!(code, Some(0), "completion works outside a workspace: {err}");
    assert!(out.contains("COMPLETE"), "{out}");
    scratch.close().expect("cleanup");
}

#[test]
fn log_level_flag_exits_two_before_running() {
    let scratch = sample_workspace("dx-log-level-flag-");
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
    let scratch = sample_workspace("dx-bad-build-slots-");
    let root = scratch.path();
    let (code, out, err) = run_dx(root, &["status", "--build-slots=0"], &[]);
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty(), "{out}");
    assert!(err.contains("--build-slots"), "{err}");
    let (code, _, err) = run_dx(root, &["status"], &[("DX_BUILD_SLOTS", "many")]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("DX_BUILD_SLOTS"), "{err}");
}

#[test]
fn worktree_release_relative_path_uses_the_working_directory() {
    let ws = sample_workspace("dx-startup-wt-release-ws-");
    let status = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(ws.path())
        .status()
        .expect("git runs");
    assert!(status.success(), "git init the workspace");
    let runfiles = dx_test_scratch::scratch("dx-startup-wt-release-runfiles-");
    let outside = dx_test_scratch::scratch("dx-startup-wt-release-outside-");
    let cache = dx_test_scratch::scratch("dx-startup-wt-release-cache-");
    std::fs::create_dir_all(outside.path().join("slots/4")).expect("outside slot");
    let ws_dir = ws.path().to_string_lossy().into_owned();
    let outside_dir = outside.path().to_string_lossy().into_owned();
    let cache_dir = cache.path().to_string_lossy().into_owned();
    let expected = outside
        .path()
        .canonicalize()
        .expect("canonical outside")
        .join("slots/4")
        .display()
        .to_string();
    let unexpected = runfiles
        .path()
        .canonicalize()
        .expect("canonical runfiles")
        .join("slots/4")
        .display()
        .to_string();
    let (_, out, err) = run_dx(
        runfiles.path(),
        &["worktree", "release", "slots/4"],
        &[
            ("BUILD_WORKSPACE_DIRECTORY", &ws_dir),
            ("BUILD_WORKING_DIRECTORY", &outside_dir),
            ("XDG_CACHE_HOME", &cache_dir),
        ],
    );
    let text = format!("{out}{err}");
    assert!(
        text.contains("is not a dx worktree slot"),
        "release reached the slot lookup: {text}"
    );
    assert!(
        text.contains(&expected),
        "a relative release path is read from the working directory: {text}"
    );
    assert!(
        !text.contains(&unexpected),
        "a relative release path ignores the runfiles cwd: {text}"
    );
}
