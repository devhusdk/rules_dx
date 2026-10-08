use super::{falls_back, flag_workspace, resolve, scans_help, Invocation, StartupError};
use crate::args::Command;
use crate::test_support::strings;
use dx_output::OutputMode;

fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

fn workspace_at(root: &std::path::Path, module: &str) {
    std::fs::write(root.join("MODULE.bazel"), module).expect("module");
}

fn config_at(root: &std::path::Path, text: &str) {
    std::fs::create_dir_all(root.join(".dx")).expect("dx");
    std::fs::write(root.join(".dx/config.toml"), text).expect("config");
}

fn canonical(root: &std::path::Path) -> std::path::PathBuf {
    dx_process::canonicalize_or_keep(root)
}

fn ok_workspace(
    words: &[&str],
    env: &[(&str, &str)],
    cwd: &std::path::Path,
) -> (Invocation, std::path::PathBuf) {
    let env = env_of(env);
    match resolve(&strings(words), &env, cwd) {
        Ok(resolved) => (resolved.invocation, resolved.workspace),
        Err(error) => panic!("{words:?}: want Ok, got {}", describe(error)),
    }
}

fn describe(error: StartupError) -> String {
    match error {
        StartupError::Help { text } => format!("Help({text:?})"),
        StartupError::Usage { message } => format!("Usage({message:?})"),
        StartupError::Workspace { message } => format!("Workspace({message:?})"),
    }
}

fn usage_message(words: &[&str], env: &[(&str, &str)], cwd: &std::path::Path) -> String {
    let env = env_of(env);
    match resolve(&strings(words), &env, cwd) {
        Err(StartupError::Usage { message }) => message,
        other => panic!(
            "{words:?}: want Usage, got {}",
            other.map_or_else(
                |error| describe(error),
                |resolved| format!("Ok({:?})", resolved.invocation.command)
            )
        ),
    }
}

fn workspace_message(words: &[&str], env: &[(&str, &str)], cwd: &std::path::Path) -> String {
    let env = env_of(env);
    match resolve(&strings(words), &env, cwd) {
        Err(StartupError::Workspace { message }) => message,
        other => panic!(
            "{words:?}: want Workspace, got {}",
            other.map_or_else(
                |error| describe(error),
                |resolved| format!("Ok({:?})", resolved.invocation.command)
            )
        ),
    }
}

fn help_text(words: &[&str], env: &[(&str, &str)], cwd: &std::path::Path) -> String {
    let env = env_of(env);
    match resolve(&strings(words), &env, cwd) {
        Err(StartupError::Help { text }) => text,
        other => panic!(
            "{words:?}: want Help, got {}",
            other.map_or_else(
                |error| describe(error),
                |resolved| format!("Ok({:?})", resolved.invocation.command)
            )
        ),
    }
}

#[test]
fn help_scan_finds_help_and_version_before_any_filesystem_read() {
    for words in [
        vec!["--help"],
        vec!["-h"],
        vec!["--version"],
        vec!["-V"],
        vec!["help"],
        vec!["help", "lint"],
        vec!["lint", "--help"],
        vec!["lint", "--output", "json", "--help"],
        vec!["lint", "--workspace", "/repo", "--help"],
        vec!["version", "--pin=0.0.0", "--help"],
    ] {
        assert!(scans_help(&words), "{words:?} asks for help");
    }
    for words in [
        vec!["lint"],
        vec!["version"],
        vec!["status"],
        vec!["new", "rust", "demo"],
        vec!["bazel", "--help"],
        vec!["build", "--", "--help"],
        Vec::new(),
    ] {
        assert!(!scans_help(&words), "{words:?} is not a help request");
    }
}

#[test]
fn flag_scan_reads_the_last_workspace_and_skips_payloads() {
    assert_eq!(
        flag_workspace(&["lint", "--workspace", "/b"]),
        Some("/b".to_owned())
    );
    assert_eq!(
        flag_workspace(&["lint", "--workspace=/b"]),
        Some("/b".to_owned())
    );
    assert_eq!(
        flag_workspace(&["lint", "--workspace", "/a", "--workspace", "/b"]),
        Some("/b".to_owned())
    );
    assert_eq!(
        flag_workspace(&["lint", "--output", "json", "--workspace", "/b"]),
        Some("/b".to_owned())
    );
    assert_eq!(flag_workspace(&["lint", "--output", "json"]), None);
    assert_eq!(flag_workspace(&["lint", "--workspace"]), None);
    assert_eq!(flag_workspace(&["lint"]), None);
    assert_eq!(flag_workspace(&["bazel", "--workspace", "/b"]), None);
    assert_eq!(flag_workspace(&["lint", "--", "--workspace", "/b"]), None);
    assert_eq!(
        flag_workspace(&["lint", "--workspace", "--quiet"]),
        None,
        "a flag is never a workspace value"
    );
}

#[test]
fn help_ignores_a_malformed_config() {
    let scratch = dx_test_scratch::scratch("startup-help-malformed-");
    let root = scratch.path().to_path_buf();
    workspace_at(&root, "module(name = \"help\", version = \"0.0.0\")\n");
    config_at(&root, "not toml = [\n");
    for words in [
        vec!["--help"],
        vec!["help"],
        vec!["help", "lint"],
        vec!["lint", "--help"],
        vec!["--version"],
    ] {
        let text = help_text(&words, &[], &root);
        assert!(!text.is_empty(), "{words:?}");
    }
    let text = help_text(&["--help"], &[], &root);
    assert!(text.contains("lint"), "top help lists commands");
    scratch.close().expect("cleanup");
}

#[test]
fn help_ignores_a_bogus_workspace_selection() {
    let scratch = dx_test_scratch::scratch("startup-help-bogus-ws-");
    let root = scratch.path().to_path_buf();
    workspace_at(&root, "module(name = \"help\", version = \"0.0.0\")\n");
    let text = help_text(
        &["lint", "--help", "--workspace", "/nonexistent-zzz"],
        &[],
        &root,
    );
    assert!(text.contains("--workspace"), "command help names the flag");
    scratch.close().expect("cleanup");
}

#[test]
fn operational_commands_report_a_malformed_config() {
    let scratch = dx_test_scratch::scratch("startup-malformed-");
    let root = scratch.path().to_path_buf();
    workspace_at(&root, "module(name = \"ops\", version = \"0.0.0\")\n");
    config_at(&root, "[dx]\nqiet = true\n");
    let message = usage_message(&["status"], &[], &root);
    assert!(
        message.contains("qiet") && message.contains("config.toml"),
        "the diagnostic names the key and the file: {message}"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn explicit_workspace_uses_that_workspaces_defaults() {
    let scratch = dx_test_scratch::scratch("startup-explicit-");
    let root = scratch.path().to_path_buf();
    let first = root.join("first");
    let second = root.join("second");
    std::fs::create_dir_all(&first).expect("first");
    std::fs::create_dir_all(&second).expect("second");
    workspace_at(&first, "module(name = \"first\", version = \"0.0.0\")\n");
    workspace_at(&second, "module(name = \"second\", version = \"0.0.0\")\n");
    config_at(&first, "[dx]\noutput = \"json\"\n");
    config_at(&second, "[dx]\noutput = \"text\"\n");
    let (invocation, workspace) = ok_workspace(
        &["version", "--workspace", &second.display().to_string()],
        &[],
        &first,
    );
    assert_eq!(workspace, canonical(&second));
    assert_eq!(
        invocation.output,
        OutputMode::Text { quiet: false },
        "the selected workspace supplies the defaults, not the cwd tree"
    );
    let (invocation, workspace) = ok_workspace(
        &["version"],
        &[("DX_WORKSPACE", &second.display().to_string())],
        &first,
    );
    assert_eq!(workspace, canonical(&second));
    assert_eq!(invocation.output, OutputMode::Text { quiet: false });
    let (invocation, workspace) = ok_workspace(&["version"], &[], &first);
    assert_eq!(workspace, canonical(&first));
    assert_eq!(invocation.output, OutputMode::Json);
    scratch.close().expect("cleanup");
}

#[test]
fn flag_beats_env_beats_file_for_workspace_selection() {
    let scratch = dx_test_scratch::scratch("startup-precedence-");
    let root = scratch.path().to_path_buf();
    let mut dirs = std::collections::BTreeMap::new();
    for name in ["cwd", "file", "env", "flag"] {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).expect(name);
        workspace_at(
            &dir,
            &format!("module(name = \"{name}\", version = \"0.0.0\")\n"),
        );
        dirs.insert(name, dir);
    }
    config_at(
        &dirs["cwd"],
        &format!("[dx]\nworkspace = \"{}\"\n", dirs["file"].display()),
    );
    let (_, workspace) = ok_workspace(&["status"], &[], &dirs["cwd"]);
    assert_eq!(workspace, canonical(&dirs["file"]), "the file redirects");
    let (_, workspace) = ok_workspace(
        &["status"],
        &[("DX_WORKSPACE", &dirs["env"].display().to_string())],
        &dirs["cwd"],
    );
    assert_eq!(
        workspace,
        canonical(&dirs["env"]),
        "the environment beats the file"
    );
    let (_, workspace) = ok_workspace(
        &["status", "--workspace", &dirs["flag"].display().to_string()],
        &[("DX_WORKSPACE", &dirs["env"].display().to_string())],
        &dirs["cwd"],
    );
    assert_eq!(
        workspace,
        canonical(&dirs["flag"]),
        "the flag beats the environment"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn config_redirect_loads_the_target_workspaces_defaults() {
    let scratch = dx_test_scratch::scratch("startup-redirect-");
    let root = scratch.path().to_path_buf();
    let here = root.join("here");
    let target = root.join("target");
    std::fs::create_dir_all(&here).expect("here");
    std::fs::create_dir_all(&target).expect("target");
    workspace_at(&here, "module(name = \"here\", version = \"0.0.0\")\n");
    workspace_at(&target, "module(name = \"target\", version = \"0.0.0\")\n");
    config_at(
        &here,
        &format!(
            "[dx]\nworkspace = \"{}\"\noutput = \"json\"\n",
            target.display()
        ),
    );
    config_at(&target, "[dx]\noutput = \"text\"\n");
    let (invocation, workspace) = ok_workspace(&["version"], &[], &here);
    assert_eq!(workspace, canonical(&target));
    assert_eq!(
        invocation.output,
        OutputMode::Text { quiet: false },
        "the redirect target supplies the defaults"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn workspace_redirect_cycles_are_a_usage_error() {
    let scratch = dx_test_scratch::scratch("startup-cycle-");
    let root = scratch.path().to_path_buf();
    let left = root.join("left");
    let right = root.join("right");
    std::fs::create_dir_all(&left).expect("left");
    std::fs::create_dir_all(&right).expect("right");
    workspace_at(&left, "module(name = \"left\", version = \"0.0.0\")\n");
    workspace_at(&right, "module(name = \"right\", version = \"0.0.0\")\n");
    config_at(
        &left,
        &format!("[dx]\nworkspace = \"{}\"\n", right.display()),
    );
    config_at(
        &right,
        &format!("[dx]\nworkspace = \"{}\"\n", left.display()),
    );
    let message = usage_message(&["status"], &[], &left);
    assert!(
        message.contains("workspace redirect cycle"),
        "the diagnostic names the cycle: {message}"
    );
    assert!(
        message.contains(&left.display().to_string())
            && message.contains(&right.display().to_string()),
        "the diagnostic names both workspaces: {message}"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn self_redirect_converges() {
    let scratch = dx_test_scratch::scratch("startup-self-");
    let root = scratch.path().to_path_buf();
    workspace_at(&root, "module(name = \"own\", version = \"0.0.0\")\n");
    config_at(
        &root,
        &format!("[dx]\nworkspace = \"{}\"\n", canonical(&root).display()),
    );
    let (_, workspace) = ok_workspace(&["status"], &[], &root);
    assert_eq!(workspace, canonical(&root));
    scratch.close().expect("cleanup");
}

#[test]
fn discovery_free_commands_work_outside_a_workspace() {
    let scratch = dx_test_scratch::scratch("startup-outside-");
    let root = scratch.path().to_path_buf();
    let start = canonical(&root);
    for words in [
        vec!["new", "rust", "demo", "--dry-run"],
        vec!["completion", "bash"],
        vec!["completion", "--check"],
        vec!["version"],
        vec!["version", "--dry-run"],
        vec!["version", "--pin=0.0.0", "--dry-run"],
        vec!["init", "--dry-run"],
    ] {
        let (invocation, workspace) = ok_workspace(&words, &[], &root);
        assert_eq!(
            workspace, start,
            "{words:?} falls back to the start directory"
        );
        assert!(falls_back(&invocation), "{words:?} needs no workspace");
    }
    scratch.close().expect("cleanup");
}

#[test]
fn workspace_operations_fail_clearly_outside_a_workspace() {
    let scratch = dx_test_scratch::scratch("startup-needs-ws-");
    let root = scratch.path().to_path_buf();
    for words in [
        vec!["status"],
        vec!["build"],
        vec!["version", "--check"],
        vec!["version", "--pin=0.0.0"],
        vec!["version", "--rollback"],
    ] {
        let message = workspace_message(&words, &[], &root);
        assert!(
            message.contains("workspace_not_found"),
            "{words:?} names the missing workspace: {message}"
        );
    }
    scratch.close().expect("cleanup");
}

#[test]
fn init_prefers_an_explicit_dir_without_a_module() {
    let scratch = dx_test_scratch::scratch("startup-init-dir-");
    let root = scratch.path().to_path_buf();
    let fresh = root.join("fresh");
    let (invocation, workspace) = ok_workspace(
        &["init", "--workspace", &fresh.display().to_string()],
        &[],
        &root,
    );
    assert_eq!(invocation.command, Command::Init);
    assert_eq!(workspace, canonical(&fresh));
    scratch.close().expect("cleanup");
}

#[test]
fn bazel_payload_workspace_stays_a_payload() {
    let scratch = dx_test_scratch::scratch("startup-bazel-ws-");
    let root = scratch.path().to_path_buf();
    let message = workspace_message(&["bazel", "--workspace", "/repo"], &[], &root);
    assert!(
        message.contains("no MODULE.bazel") && !message.contains("/repo"),
        "the flag reaches Bazel, it never selects a workspace: {message}"
    );
    scratch.close().expect("cleanup");
}

fn dx_binary() -> std::path::PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    std::path::Path::new(&root)
        .join(workspace)
        .join("cli/cli/dx")
}

fn run_dx(
    words: &[&str],
    cwd: &std::path::Path,
    envs: &[(&str, &str)],
    remove_env: &[&str],
) -> (Option<i32>, String, String) {
    let mut command = assert_cmd::Command::new(dx_binary());
    command.current_dir(cwd);
    for name in remove_env {
        command.env_remove(name);
    }
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

fn hermetic_env() -> Vec<&'static str> {
    vec!["BUILD_WORKSPACE_DIRECTORY", "DX_WORKSPACE"]
}

#[test]
fn help_process_works_with_a_malformed_config() {
    let scratch = dx_test_scratch::scratch("startup-proc-help-");
    let root = scratch.path().to_path_buf();
    workspace_at(&root, "module(name = \"help\", version = \"0.0.0\")\n");
    config_at(&root, "not toml = [\n");
    for words in [vec!["--help"], vec!["help", "lint"], vec!["lint", "--help"]] {
        let (code, out, err) = run_dx(&words, &root, &[], &hermetic_env());
        assert_eq!(code, Some(0), "{words:?}: {err}");
        assert!(out.contains("dx"), "{words:?}");
    }
    scratch.close().expect("cleanup");
}

#[test]
fn workspace_selection_process_uses_the_target_defaults() {
    let scratch = dx_test_scratch::scratch("startup-proc-ws-");
    let root = scratch.path().to_path_buf();
    let first = root.join("first");
    let second = root.join("second");
    std::fs::create_dir_all(&first).expect("first");
    std::fs::create_dir_all(&second).expect("second");
    for dir in [&first, &second] {
        workspace_at(dir, "module(name = \"ws\", version = \"0.0.0\")\n");
        std::fs::create_dir_all(dir.join(".dx")).expect("dx");
        std::fs::write(dir.join(".dx/version"), "0.0.0\n").expect("pin");
    }
    config_at(&first, "[dx]\noutput = \"json\"\n");
    config_at(&second, "[dx]\noutput = \"text\"\n");
    let flag = second.display().to_string();
    let (code, out, err) = run_dx(
        &["version", "--workspace", &flag],
        &first,
        &[],
        &hermetic_env(),
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.starts_with("dx "),
        "the target workspace selects text output: {out}"
    );
    let (code, out, err) = run_dx(
        &["version"],
        &first,
        &[("DX_WORKSPACE", &flag)],
        &hermetic_env(),
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.starts_with("dx "),
        "the environment selection does too: {out}"
    );
    let (code, out, _) = run_dx(&["version"], &first, &[], &hermetic_env());
    assert_eq!(code, Some(0));
    assert!(
        out.contains("\"event\":\"command_started\""),
        "the cwd workspace keeps json output: {out}"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn standalone_new_process_works_from_an_empty_directory() {
    let scratch = dx_test_scratch::scratch("startup-proc-new-");
    let root = scratch.path().to_path_buf();
    let (code, out, err) = run_dx(
        &["new", "rust", "demo", "--dry-run"],
        &root,
        &[],
        &hermetic_env(),
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("demo/Cargo.toml"), "{out}");
    assert!(
        !root.join("demo/Cargo.toml").exists(),
        "dry run writes nothing"
    );
    let (code, out, err) = run_dx(&["new", "rust", "demo"], &root, &[], &hermetic_env());
    assert_eq!(code, Some(0), "{err}: {out}");
    assert!(
        root.join("demo/Cargo.toml").exists(),
        "standalone new scaffolds"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn version_and_completion_process_work_outside_a_workspace() {
    let scratch = dx_test_scratch::scratch("startup-proc-outside-");
    let root = scratch.path().to_path_buf();
    let (code, out, err) = run_dx(&["version"], &root, &[], &hermetic_env());
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("dx 0.0.0"), "{out}");
    assert!(
        !out.contains("pin "),
        "no pin line without a workspace: {out}"
    );
    let (code, out, err) = run_dx(&["completion", "bash"], &root, &[], &hermetic_env());
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("COMPLETE"), "{out}");
    for words in [vec!["version", "--check"], vec!["status"]] {
        let (code, _, err) = run_dx(&words, &root, &[], &hermetic_env());
        assert_eq!(code, Some(2), "{words:?}");
        assert!(err.contains("cannot resolve workspace"), "{words:?}: {err}");
    }
    scratch.close().expect("cleanup");
}

#[test]
fn bazel_workspace_start_is_honored_for_discovery() {
    let scratch = dx_test_scratch::scratch("startup-proc-start-");
    let root = scratch.path().to_path_buf();
    let inside = root.join("inside");
    std::fs::create_dir_all(&inside).expect("inside");
    workspace_at(&root, "module(name = \"ws\", version = \"0.0.0\")\n");
    std::fs::create_dir_all(root.join(".dx")).expect("dx");
    std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
    let start = root.display().to_string();
    let (code, out, err) = run_dx(
        &["version", "--check"],
        &inside,
        &[("BUILD_WORKSPACE_DIRECTORY", &start)],
        &["DX_WORKSPACE"],
    );
    assert_eq!(code, Some(0), "{err}");
    assert!(out.contains("version ok"), "{out}");
    scratch.close().expect("cleanup");
}
