use dx_plan::args::{Command, HintScope, QualityRequest};

pub(crate) use dx_plan::plan::shell_quote as quote;

/// Returns the words safe to echo (safe options, then redacted stand-ins) and whether any changed.
fn echoed(options: &[String]) -> (Vec<String>, bool) {
    let (safe, withheld) = crate::plan::quality::partition_provenance_options(options);
    let changed = !withheld.is_empty() || safe != options;
    let mut words = safe;
    for entry in withheld {
        match entry.split_once(' ') {
            Some((flag, value)) => {
                words.push(flag.to_owned());
                words.push(value.to_owned());
            }
            None => words.push(entry),
        }
    }
    (words, changed)
}

pub(crate) fn command_line(request: &QualityRequest, apply: bool) -> String {
    let command = match &request.origin {
        Some(_) if apply => Command::Fix,
        Some(origin) => origin.command,
        None => request.command,
    };
    let mut words = vec!["dx".to_owned(), command.name().to_owned()];
    if apply {
        words.push("--apply".to_owned());
    }
    if let Some(workspace) = &request.workspace {
        words.push("--workspace".to_owned());
        words.push(quote(workspace));
    }
    for option in echoed(&request.bazel_startup_options).0 {
        words.push(quote(&format!("--bazel-startup-option={option}")));
    }
    let targets = match request.origin.as_ref().map(|origin| &origin.scope) {
        Some(HintScope::Affected) => {
            words.push("--affected".to_owned());
            &[][..]
        }
        Some(HintScope::Targets(targets)) => &targets[..],
        None => &request.targets[..],
    };
    words.extend(targets.iter().map(|target| quote(target)));
    let options: Vec<String> = echoed(&request.bazel_options)
        .0
        .iter()
        .map(|option| quote(option))
        .collect();
    if !options.is_empty() {
        words.push("--".to_owned());
        words.extend(options);
    }
    words.join(" ")
}

pub(crate) fn omitted_options(request: &QualityRequest) -> bool {
    echoed(&request.bazel_options).1 || echoed(&request.bazel_startup_options).1
}

pub(crate) fn apply_hint(request: &QualityRequest, files: usize) -> String {
    let mut line = format!(
        "{files} file(s) need changes. Apply them with: {}",
        command_line(request, true)
    );
    if omitted_options(request) {
        line.push_str(" (add your credential options again)");
    }
    line
}

pub(crate) fn rerun_hint(request: &QualityRequest, action: &str) -> String {
    let mut line = format!("next: {action}, then run: {}", command_line(request, false));
    if omitted_options(request) {
        line.push_str(" (add your credential options again)");
    }
    line
}

pub(crate) fn next_action(code: &str) -> Option<&'static str> {
    Some(match code {
        "bazel_failed" => "next: fix the Bazel error above and run the command again.",
        "analysis_failed" => "next: fix the Bazel analysis error above and run the command again.",
        "query_failed" => {
            "next: check the scope in the query error above and run the command again."
        }
        "invalid_scope" | "no_owner" | "no_tests" => {
            "next: check the target or path scope and run the command again."
        }
        "report_failed" => "next: make the report destination writable and run the command again.",
        "disk_full" => {
            "next: free disk space (try `dx worktree prune --apply`) and run the command again."
        }
        "invalid_bep" | "unreadable_bep" | "collection_failed" | "invalid_result" => {
            "next: results are incomplete; run the command again."
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn request() -> QualityRequest {
        request_of(&[
            "lint",
            "--workspace",
            "/repo dir/it's",
            "//a:one",
            "//b c:d",
            "--",
            "--config=ci",
        ])
    }

    pub(crate) fn request_of(words: &[&str]) -> QualityRequest {
        dx_plan::args::parse(&dx_testing::cli::strings(words))
            .expect("parse")
            .quality_request()
            .expect("quality")
    }

    #[test]
    fn quote_leaves_plain_words_and_wraps_the_rest() {
        assert_eq!(quote("//a:one"), "//a:one");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote("$(x)"), "'$(x)'");
        assert_eq!(quote(""), "''");
    }

    #[test]
    fn apply_command_keeps_workspace_scope_and_configuration() {
        assert_eq!(
            command_line(&request(), true),
            "dx lint --apply --workspace '/repo dir/it'\\''s' //a:one '//b c:d' -- --config=ci"
        );
        assert_eq!(
            apply_hint(&request(), 2),
            "2 file(s) need changes. Apply them with: dx lint --apply --workspace '/repo dir/it'\\''s' //a:one '//b c:d' -- --config=ci"
        );
    }

    fn umbrella(command: Command, scope: HintScope) -> QualityRequest {
        let mut request = request_of(&["format", "//a:x", "//a:y", "--", "--config=ci"]);
        request.origin = Some(dx_plan::args::UmbrellaOrigin { command, scope });
        request
    }

    #[test]
    fn umbrella_hints_name_the_umbrella_command_and_user_scope() {
        let affected = umbrella(Command::Check, HintScope::Affected);
        assert_eq!(
            command_line(&affected, true),
            "dx fix --apply --affected -- --config=ci"
        );
        assert_eq!(
            apply_hint(&affected, 3),
            "3 file(s) need changes. Apply them with: dx fix --apply --affected -- --config=ci"
        );
        assert_eq!(
            rerun_hint(&affected, "fix the Bazel error above"),
            "next: fix the Bazel error above, then run: dx check --affected -- --config=ci"
        );
        let patterns = umbrella(Command::Fix, HintScope::Targets(vec!["//a/...".to_owned()]));
        assert_eq!(
            command_line(&patterns, true),
            "dx fix --apply //a/... -- --config=ci"
        );
        assert_eq!(
            rerun_hint(&patterns, "fix it"),
            "next: fix it, then run: dx fix //a/... -- --config=ci"
        );
        let mut secret = umbrella(Command::Check, HintScope::Affected);
        secret.bazel_options = vec!["--remote_header=x-token=abc".to_owned()];
        assert_eq!(
            apply_hint(&secret, 1),
            "1 file(s) need changes. Apply them with: dx fix --apply --affected -- --remote_header (add your credential options again)"
        );
    }

    #[test]
    fn credential_options_never_enter_a_suggested_command() {
        let mut secret = request();
        secret.bazel_options = vec![
            "--remote_header=Authorization=Bearer abc".to_owned(),
            "--config=ci".to_owned(),
        ];
        let line = command_line(&secret, true);
        assert!(!line.contains("abc") && !line.contains("Bearer"), "{line}");
        assert!(line.ends_with("-- --config=ci --remote_header"), "{line}");
        assert!(apply_hint(&secret, 1).ends_with("(add your credential options again)"));
    }

    #[test]
    fn hints_redact_url_credentials_and_env_values() {
        let mut secret = request();
        secret.bazel_options = crate::test_support::strings(&[
            "--remote_cache=https://u:p@h",
            "--bes_backend=grpcs://u:p@h",
            "--action_env=GH_PAT=x",
            "--action_env",
            "GH_PAT=x",
            "--bes_header",
            "x=hunter2",
            "--config=ci",
        ]);
        for line in [apply_hint(&secret, 1), rerun_hint(&secret, "fix")] {
            for leaked in ["u:p", "GH_PAT=x", "hunter2"] {
                assert!(!line.contains(leaked), "{line}");
            }
            assert!(
                line.contains("--config=ci") && line.contains("GH_PAT=(value)"),
                "{line}"
            );
            assert!(
                line.ends_with("(add your credential options again)"),
                "{line}"
            );
        }
    }

    #[test]
    fn startup_option_secrets_are_withheld() {
        let mut secret = request();
        secret.bazel_startup_options =
            crate::test_support::strings(&["--host_jvm_args=-Dauth.token=hunter2"]);
        let line = apply_hint(&secret, 1);
        assert!(!line.contains("hunter2"), "{line}");
        assert!(
            line.ends_with("(add your credential options again)"),
            "{line}"
        );
    }

    #[test]
    fn safe_only_options_add_no_note() {
        let mut safe = request();
        safe.bazel_options =
            crate::test_support::strings(&["--config=ci", "--remote_cache=grpcs://cache.example"]);
        let line = apply_hint(&safe, 1);
        assert!(
            line.ends_with("-- --config=ci --remote_cache=grpcs://cache.example"),
            "{line}"
        );
        assert!(!line.contains("credential"), "{line}");
    }

    #[test]
    fn a_rerun_is_a_check_and_never_a_removed_command() {
        let line = rerun_hint(&request(), "fix the report path");
        assert!(
            line.starts_with("next: fix the report path, then run: dx lint "),
            "{line}"
        );
        assert!(!line.contains("--apply"), "{line}");
    }

    #[test]
    fn next_actions_cover_failures_with_a_supported_fix() {
        for code in [
            "bazel_failed",
            "analysis_failed",
            "query_failed",
            "report_failed",
            "invalid_bep",
            "disk_full",
        ] {
            let text = next_action(code).expect(code);
            assert!(text.starts_with("next: "), "{text}");
            for banned in [
                "dry-run",
                "dx bazel",
                "dx verify",
                "rerun",
                "hooks",
                "setup",
                "clean",
                "reset",
            ] {
                assert!(!text.contains(banned), "{text}");
            }
        }
        assert_eq!(next_action("version_skew"), None);
        assert_eq!(next_action("tests_failed"), None);
    }
}
