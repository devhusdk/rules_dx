use std::path::PathBuf;

fn read(key: &str) -> String {
    let rel = std::env::var(key).unwrap_or_else(|_| panic!("{key} must name a captured output"));
    let path: PathBuf = dx_testing::resolve_runfiles(&rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn candidates(text: &str) -> Vec<&str> {
    text.lines()
        .filter_map(|line| line.strip_prefix("candidate="))
        .collect()
}

fn assert_quiet(name: &str, text: &str) {
    assert!(
        text.lines().any(|line| line == "stderr_bytes=0"),
        "{name} must complete without terminal noise: {text}"
    );
}

#[test]
fn the_shell_callback_queries_the_flag_workspace_with_ordered_startup_tokens() {
    let flag = read("DX_COMPLETION_FLAG");
    assert_quiet("flag", &flag);
    assert_eq!(
        candidates(&flag),
        vec!["//pkg:alpha", "//pkg:another"],
        "the bash callback must offer the probe labels that match: {flag}"
    );
    let argv = read("DX_COMPLETION_ARGV");
    let lines: Vec<&str> = argv.lines().collect();
    assert_eq!(
        lines,
        vec![
            "ROOT/consumer ws \u{fc}",
            "--noblock_for_lock",
            "--output_base=ROOT/base \u{fc}",
            "--output_user_root=ROOT/user root",
            "query",
            "--output=label",
            "kind(rule, //pkg:*)",
        ],
        "the query must run in the selected workspace with the typed startup tokens before the verb"
    );
}

#[test]
fn the_shell_callback_follows_the_environment_workspace() {
    let env = read("DX_COMPLETION_ENV");
    assert_quiet("env", &env);
    let got = candidates(&env);
    assert!(got.contains(&"//lib/..."), "env: {env}");
    assert!(!got.contains(&"//pkg/..."), "env: {env}");
}

#[test]
fn the_shell_callback_follows_the_config_workspace() {
    let config = read("DX_COMPLETION_CONFIG");
    assert_quiet("config", &config);
    let got = candidates(&config);
    assert!(got.contains(&"//pkg/..."), "config: {config}");
    assert!(!got.contains(&"//lib/..."), "config: {config}");
}

#[test]
fn broken_defaults_and_failed_queries_degrade_quietly() {
    let broken = read("DX_COMPLETION_BROKEN");
    assert_quiet("broken", &broken);
    let labels: Vec<&str> = candidates(&broken)
        .into_iter()
        .filter(|value| value.starts_with("//"))
        .collect();
    assert_eq!(labels, vec!["//..."], "broken: {broken}");
    assert!(
        candidates(&broken).contains(&"--workspace"),
        "broken: {broken}"
    );
    let failed = read("DX_COMPLETION_FAILED_QUERY");
    assert_quiet("failed query", &failed);
    assert!(
        candidates(&failed).iter().all(|value| !value.contains(':')),
        "a failed query offers no targets: {failed}"
    );
}
