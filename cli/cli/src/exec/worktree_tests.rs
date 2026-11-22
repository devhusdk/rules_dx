use super::test_support::Harness;

#[test]
fn disk_on_prune_fails_with_usage() {
    let harness = Harness::new("wt_disk");
    let (code, _, stderr) = harness.run(&["worktree", "prune", "--disk"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(
        stderr.contains("option \"--disk\" is not supported by dx worktree prune"),
        "{stderr}"
    );
}

#[test]
fn misplaced_flag_under_json_reports_invalid_arguments() {
    let harness = Harness::new("wt_misplaced_json");
    let (code, stdout, stderr) = harness.run(&["worktree", "list", "--apply", "--output=json"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(
        stderr.contains("option \"--apply\" is not supported by dx worktree list"),
        "{stderr}"
    );
    let events: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("json line"))
        .collect();
    assert!(
        events
            .iter()
            .any(|event| event["event"] == "error" && event["code"] == "invalid_arguments"),
        "{stdout}"
    );
    let last = events.last().expect("events");
    assert_eq!(last["event"], "command_finished", "{stdout}");
    assert_eq!(last["exit_code"], 2, "{stdout}");
    assert!(
        !events
            .iter()
            .any(|event| event["event"] == "command_started"),
        "{stdout}"
    );
}

fn set_primary(harness: &Harness, mode: &str) {
    std::fs::write(
        harness.workspace.join(".git").join("config"),
        format!("[core]\n\trepositoryformatversion = 0\n[dx]\n\tprimary = {mode}\n"),
    )
    .expect("git config");
}

#[test]
fn a_protected_main_checkout_refuses_a_build_with_exit_2() {
    let harness = Harness::new("wt_main_refused");
    set_primary(&harness, "protected");
    let (code, stdout, stderr) = harness.run(&["build", "--output=json"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(
        stderr.starts_with("dx: refused: no Bazel in the main checkout "),
        "{stderr}"
    );
    assert!(
        stderr.contains(". Work in a slot: tools/dx worktree add <branch>\n"),
        "{stderr}"
    );
    let error = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json line"))
        .find(|event| event["event"] == "error")
        .expect("error event");
    assert_eq!(error["code"], "main_checkout", "{stdout}");
}

#[test]
fn ci_and_worktree_commands_pass_a_protected_main_checkout() {
    let harness = Harness::new("wt_main_exempt");
    set_primary(&harness, "protected");
    let (_, _, stderr) = harness.run_with_ci(&["build"], true);
    assert!(!stderr.contains("refused"), "{stderr}");
    let (_, _, stderr) = harness.run(&["worktree", "list"]);
    assert!(!stderr.contains("refused"), "{stderr}");
}

#[test]
fn an_unset_or_open_main_checkout_lets_a_plain_shell_build() {
    for mode in [None, Some("open")] {
        let harness = Harness::new("wt_main_open");
        if let Some(mode) = mode {
            set_primary(&harness, mode);
        }
        let (_, _, stderr) = harness.run(&["build"]);
        assert!(!stderr.contains("refused"), "{mode:?}: {stderr}");
    }
}

#[test]
fn a_protected_main_checkout_refuses_dx_run_before_it_builds() {
    let harness = Harness::new("wt_main_run");
    set_primary(&harness, "protected");
    let invocation = super::test_support::invocation(&["run", "//:app"]);
    let runner = harness.runner();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let env = harness.env(&runner, &mut out, &mut err);
    let code = super::execute_until(&invocation, env, &|| false);
    let stderr = String::from_utf8(err).expect("stderr");
    assert_eq!(code, 2, "{stderr}");
    assert!(
        stderr.starts_with("dx: refused: no Bazel in the main checkout "),
        "{stderr}"
    );
    assert!(harness.build_argv.borrow().is_empty());
}
