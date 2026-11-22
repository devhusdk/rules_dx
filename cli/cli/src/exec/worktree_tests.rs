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
