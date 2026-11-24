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
fn a_leftover_dx_primary_in_the_main_checkout_is_ignored() {
    for mode in ["protected", "open", "protectd"] {
        let harness = Harness::new("wt_main_leftover");
        set_primary(&harness, mode);
        for words in [&["build"][..], &["check"][..], &["clean"][..]] {
            let (_, _, stderr) = harness.run(words);
            assert!(!stderr.contains("refused"), "{mode} {words:?}: {stderr}");
        }
        let invocation = super::test_support::invocation(&["run", "//:app"]);
        let runner = harness.runner();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let env = harness.env(&runner, &mut out, &mut err);
        super::execute_until(&invocation, env, &|| false);
        let stderr = String::from_utf8(err).expect("stderr");
        assert!(!stderr.contains("refused"), "{mode} run: {stderr}");
        assert!(
            !harness.build_argv.borrow().is_empty(),
            "{mode} run: {stderr}"
        );
    }
}
