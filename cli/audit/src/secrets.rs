pub const TOOL_ENV_VAR: &str = "DX_GITLEAKS_BIN";

pub const TOOL_LABEL: &str = "@rules_dx//quality/artifacts:gitleaks";

/// Shell command that exports the pinned Gitleaks path for a local run.
pub const RESOLVE_COMMAND: &str = "export \"$(bazel run @rules_dx//cli/tool_resolve -- --label @rules_dx//quality/artifacts:gitleaks --var DX_GITLEAKS_BIN --print)\"";

pub const SARIF_FORMAT: &str = "sarif";

pub const REDACT_FLAG: &str = "--redact";

pub const REPORT_FORMAT_FLAG: &str = "--report-format";

pub const REPORT_PATH_FLAG: &str = "--report-path";

pub const EXIT_CODE_FLAG: &str = "--exit-code";

pub const CONFIG_FLAG: &str = "--config";

pub const CONFIG_FILE_NAME: &str = ".gitleaks.toml";

/// Git arguments that list the workspace paths git ignores, NUL-separated and
/// relative to the workspace; wholly ignored directories collapse to `dir/`.
pub const IGNORED_QUERY: [&str; 6] = [
    "ls-files",
    "-z",
    "--others",
    "--ignored",
    "--exclude-standard",
    "--directory",
];

const IGNORED_ALLOWLIST_DESCRIPTION: &str = "git-ignored paths";

/// Splits `git ls-files -z` output into paths. Empty and non-UTF-8 entries are
/// dropped, so those paths stay scanned.
pub fn parse_ignored(stdout: &[u8]) -> Vec<String> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| std::str::from_utf8(entry).ok())
        .map(ToOwned::to_owned)
        .collect()
}

/// Turns ignored paths into anchored gitleaks path regexes. A directory entry
/// (trailing `/`) also matches the bare directory name, so gitleaks prunes the
/// whole tree before reading it.
pub fn ignored_path_patterns(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| {
            let path = path.replace('\\', "/");
            match path.strip_suffix('/') {
                Some("") => None,
                Some(dir) => Some(format!("^{}(?:/|$)", regex::escape(dir))),
                None => Some(format!("^{}$", regex::escape(&path))),
            }
        })
        .collect()
}

/// Builds the gitleaks config for a scan: the user's config (or the default
/// rules when there is none) plus one paths-only allowlist for `patterns`.
/// A legacy `[allowlist]` moves into `[[allowlists]]`, because gitleaks rejects
/// both at once.
pub fn scoped_config(user: Option<&str>, patterns: &[String]) -> Result<String, String> {
    let mut table = match user {
        Some(text) => toml::from_str::<toml::Table>(text)
            .map_err(|error| format!("invalid {CONFIG_FILE_NAME}: {error}"))?,
        None => {
            let mut extend = toml::Table::new();
            extend.insert("useDefault".to_owned(), toml::Value::Boolean(true));
            let mut table = toml::Table::new();
            table.insert("extend".to_owned(), toml::Value::Table(extend));
            table
        }
    };
    let mut allowlists = match table.remove("allowlists") {
        Some(toml::Value::Array(entries)) => entries,
        Some(other) => {
            return Err(format!(
                "invalid {CONFIG_FILE_NAME}: allowlists must be an array, got {}",
                other.type_str()
            ));
        }
        None => match table.remove("allowlist") {
            Some(legacy @ toml::Value::Table(_)) => vec![legacy],
            Some(other) => {
                return Err(format!(
                    "invalid {CONFIG_FILE_NAME}: allowlist must be a table, got {}",
                    other.type_str()
                ));
            }
            None => Vec::new(),
        },
    };
    let mut ignored = toml::Table::new();
    ignored.insert(
        "description".to_owned(),
        toml::Value::String(IGNORED_ALLOWLIST_DESCRIPTION.to_owned()),
    );
    ignored.insert(
        "paths".to_owned(),
        toml::Value::Array(patterns.iter().cloned().map(toml::Value::String).collect()),
    );
    allowlists.push(toml::Value::Table(ignored));
    table.insert("allowlists".to_owned(), toml::Value::Array(allowlists));
    toml::to_string(&table).map_err(|error| format!("cannot write gitleaks config: {error}"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretFinding {
    pub rule: String,
    pub message: String,
    pub path: Option<String>,
}

pub fn triage_sarif(text: &str) -> Result<Vec<SecretFinding>, String> {
    let document: serde_sarif::sarif::Sarif =
        serde_json::from_str(text).map_err(|error| format!("invalid gitleaks SARIF: {error}"))?;
    let mut findings = Vec::new();
    for run in &document.runs {
        let results = run.results.as_ref().cloned().unwrap_or_default();
        for result in &results {
            let rule = result
                .rule_id
                .as_deref()
                .unwrap_or("gitleaks/secret")
                .to_owned();
            let path = result
                .locations
                .as_ref()
                .and_then(|locations| locations.first())
                .and_then(|location| location.physical_location.as_ref())
                .and_then(|physical| physical.artifact_location.as_ref())
                .and_then(|artifact| artifact.uri.clone());
            let message = match &path {
                Some(uri) => format!("{rule} detected in {uri}"),
                None => format!("{rule} detected"),
            };
            findings.push(SecretFinding {
                rule,
                message,
                path,
            });
        }
    }
    findings.sort_by(|a, b| (&a.rule, &a.message, &a.path).cmp(&(&b.rule, &b.message, &b.path)));
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(text: &str) -> toml::Table {
        toml::from_str(text).expect("generated config parses")
    }

    fn allowlists(config: &toml::Table) -> Vec<toml::Table> {
        config["allowlists"]
            .as_array()
            .expect("allowlists array")
            .iter()
            .map(|entry| entry.as_table().expect("allowlist table").clone())
            .collect()
    }

    fn ignored_entry_paths(entry: &toml::Table) -> Vec<String> {
        assert_eq!(entry["description"].as_str(), Some("git-ignored paths"));
        assert_eq!(entry.len(), 2, "ignored entry is paths-only: {entry:?}");
        entry["paths"]
            .as_array()
            .expect("paths")
            .iter()
            .map(|path| path.as_str().expect("path string").to_owned())
            .collect()
    }

    #[test]
    fn ignored_query_output_splits_on_nul_and_drops_bad_entries() {
        let mut stdout = b"ign/\0local.env\0\0".to_vec();
        stdout.extend_from_slice(b"bad\xff\0");
        stdout.extend_from_slice(b"a b/c.txt\0");
        assert_eq!(
            parse_ignored(&stdout),
            vec![
                "ign/".to_owned(),
                "local.env".to_owned(),
                "a b/c.txt".to_owned()
            ]
        );
        assert!(parse_ignored(b"").is_empty());
    }

    #[test]
    fn ignored_patterns_are_anchored_and_escaped() {
        let patterns = ignored_path_patterns(&[
            "ign/".to_owned(),
            "a.b+c[d].env".to_owned(),
            "out/x.y/".to_owned(),
            "/".to_owned(),
        ]);
        assert_eq!(
            patterns,
            vec![
                r"^ign(?:/|$)".to_owned(),
                r"^a\.b\+c\[d\]\.env$".to_owned(),
                r"^out/x\.y(?:/|$)".to_owned(),
            ]
        );
        let dir = regex::Regex::new(&patterns[0]).expect("dir regex");
        assert!(dir.is_match("ign"));
        assert!(dir.is_match("ign/x"));
        assert!(!dir.is_match("ignx"));
        assert!(!dir.is_match("src/ign"));
        let file = regex::Regex::new(&patterns[1]).expect("file regex");
        assert!(file.is_match("a.b+c[d].env"));
        assert!(!file.is_match("aXb+c[d].env"));
        assert!(!file.is_match("a.b+c[d].env.bak"));
        let nested = regex::Regex::new(&patterns[2]).expect("nested regex");
        assert!(nested.is_match("out/x.y"));
        assert!(!nested.is_match("out/xzy/z"));
    }

    #[test]
    fn scoped_config_without_user_config_extends_the_defaults() {
        let config = table(&scoped_config(None, &["^ign(?:/|$)".to_owned()]).expect("config"));
        assert_eq!(config["extend"]["useDefault"].as_bool(), Some(true));
        let entries = allowlists(&config);
        assert_eq!(entries.len(), 1);
        assert_eq!(ignored_entry_paths(&entries[0]), vec!["^ign(?:/|$)"]);
    }

    #[test]
    fn scoped_config_migrates_a_legacy_allowlist() {
        let user = r#"
title = "mine"
[allowlist]
description = "fixtures"
regexTarget = "line"
regexes = ['''example''']
paths = ['''docs/.*\.md''']
"#;
        let config = table(&scoped_config(Some(user), &["^x$".to_owned()]).expect("config"));
        assert!(!config.contains_key("allowlist"));
        assert_eq!(config["title"].as_str(), Some("mine"));
        let entries = allowlists(&config);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["description"].as_str(), Some("fixtures"));
        assert_eq!(entries[0]["regexTarget"].as_str(), Some("line"));
        assert_eq!(entries[0]["regexes"][0].as_str(), Some("example"));
        assert_eq!(entries[0]["paths"][0].as_str(), Some(r"docs/.*\.md"));
        assert_eq!(ignored_entry_paths(&entries[1]), vec!["^x$"]);
    }

    #[test]
    fn scoped_config_keeps_user_rules_and_allowlists_and_appends_last() {
        let user = r#"
[extend]
useDefault = true
disabledRules = ["generic-api-key"]

[[rules]]
id = "custom"
regex = '''custom-[0-9]+'''

[[allowlists]]
description = "rule scoped"
targetRules = ["custom"]
regexes = ['''custom-0''']

[[allowlists]]
paths = ['''vendor/.*''']
"#;
        let config = table(&scoped_config(Some(user), &["^y$".to_owned()]).expect("config"));
        assert_eq!(config["extend"]["useDefault"].as_bool(), Some(true));
        assert_eq!(
            config["extend"]["disabledRules"][0].as_str(),
            Some("generic-api-key")
        );
        assert_eq!(config["rules"][0]["id"].as_str(), Some("custom"));
        assert_eq!(config["rules"][0]["regex"].as_str(), Some("custom-[0-9]+"));
        let entries = allowlists(&config);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0]["targetRules"][0].as_str(), Some("custom"));
        assert_eq!(entries[1]["paths"][0].as_str(), Some("vendor/.*"));
        assert_eq!(ignored_entry_paths(&entries[2]), vec!["^y$"]);
    }

    #[test]
    fn scoped_config_rejects_invalid_user_config() {
        assert!(scoped_config(Some("[allowlist"), &["^x$".to_owned()]).is_err());
        assert!(scoped_config(Some("allowlists = 3"), &["^x$".to_owned()]).is_err());
        assert!(scoped_config(Some("allowlist = 3"), &["^x$".to_owned()]).is_err());
    }

    #[test]
    fn sarif_triage_counts_results_and_never_surfaces_secrets() {
        let clean = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "gitleaks"}}, "results": []}]}"#;
        assert!(triage_sarif(clean).expect("clean").is_empty());
        let leaks = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "gitleaks"}}, "results": [{"ruleId": "gitleaks/generic-api-key", "message": {"text": "Generic API Key"}, "locations": [{"physicalLocation": {"artifactLocation": {"uri": "src/app.py"}}}]}, {"ruleId": "gitleaks/aws-key", "message": {"text": "AWS key"}, "fingerprint": "secret:AKIAIOSFODNN7EXAMPLE"}]}]}"#;
        let findings = triage_sarif(leaks).expect("leaks");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule, "gitleaks/aws-key");
        assert_eq!(findings[1].path, Some("src/app.py".to_owned()));
        for finding in &findings {
            assert!(!finding.rule.contains("AKIAIOSFODNN7EXAMPLE"));
            assert!(!finding.message.contains("AKIAIOSFODNN7EXAMPLE"));
            assert!(finding.path.as_deref() != Some("AKIAIOSFODNN7EXAMPLE"));
        }
        assert_eq!(
            findings[1].message,
            "gitleaks/generic-api-key detected in src/app.py"
        );
        assert!(triage_sarif("not json").is_err());
        assert!(triage_sarif(r#"{"version": "2.1.0"}"#).is_err());
    }

    #[test]
    fn sarif_triage_redaction_ignores_every_secret_field() {
        let aws = "AKIAIOSFODNN7EXAMPLE";
        let github = format!("{}{}", "ghp_", "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8");
        let generic = format!("{}{}", "sk-live-", "51H7x9yQ2wE4rT6yU8iO0p");
        let github: &str = &github;
        let generic: &str = &generic;
        let text = format!(
            r#"{{"version": "2.1.0", "runs": [{{"tool": {{"driver": {{"name": "gitleaks"}}}}, "results": [{{
                "ruleId": "gitleaks/aws-key",
                "message": {{"text": "leaked {aws} in src/creds.py"}},
                "fingerprints": {{"secret": "{aws}"}},
                "partialFingerprints": {{"secret/v1": "{github}"}},
                "properties": {{"secret": "{generic}"}},
                "locations": [{{
                    "physicalLocation": {{
                        "artifactLocation": {{"uri": "src/creds.py"}},
                        "region": {{"snippet": {{"text": "key = '{aws}'"}}}},
                        "contextRegion": {{"snippet": {{"text": "token {generic} here"}}}}
                    }}
                }}]}}]}}]}}"#
        );
        let findings = triage_sarif(&text).expect("unredacted triages");
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding.rule, "gitleaks/aws-key");
        assert_eq!(finding.path, Some("src/creds.py".to_owned()));
        assert_eq!(finding.message, "gitleaks/aws-key detected in src/creds.py");
        for secret in [aws, github, generic] {
            assert!(!finding.rule.contains(secret), "rule leaks {secret}");
            assert!(!finding.message.contains(secret), "message leaks {secret}");
            assert!(
                finding.path.as_deref().unwrap_or("").find(secret).is_none(),
                "path leaks {secret}"
            );
        }
        let redacted = text
            .replace(aws, "...")
            .replace(github, "...")
            .replace(generic, "...");
        let redacted_findings = triage_sarif(&redacted).expect("redacted triages");
        assert_eq!(findings, redacted_findings);
    }

    #[test]
    fn sarif_triage_message_never_copies_raw_text() {
        let secret_owned = format!("{}{}", "xoxb-", "123456789012-abcdefghijklmnopqrstuvwx");
        let secret: &str = &secret_owned;
        let text = format!(
            r#"{{"version": "2.1.0", "runs": [{{"tool": {{"driver": {{"name": "gitleaks"}}}}, "results": [{{
                "ruleId": "gitleaks/slack-token",
                "message": {{"text": "{secret}"}},
                "locations": [{{"physicalLocation": {{"artifactLocation": {{"uri": "src/chat.py"}}}}}}]}}]}}]}}"#
        );
        let findings = triage_sarif(&text).expect("secret-text triages");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].message,
            "gitleaks/slack-token detected in src/chat.py"
        );
        assert!(!findings[0].message.contains(secret));
    }

    #[test]
    fn sarif_triage_handles_empty_and_multi_run_fixtures() {
        let empty_runs = r#"{"version": "2.1.0", "$schema": "https://json.schemastore.org/sarif-2.1.0.json", "runs": []}"#;
        let typed: serde_sarif::sarif::Sarif =
            serde_json::from_str(empty_runs).expect("schema-valid empty runs");
        assert!(typed.runs.is_empty());
        assert!(triage_sarif(empty_runs).expect("empty runs").is_empty());

        let multi = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "gitleaks"}}, "results": [{"ruleId": "gitleaks/a", "message": {"text": "A"}}]}, {"tool": {"driver": {"name": "gitleaks"}}, "results": [{"ruleId": "gitleaks/b", "message": {"text": "B"}, "locations": [{"physicalLocation": {"artifactLocation": {"uri": "src/b.py"}}}]}]}]}"#;
        let typed_multi: serde_sarif::sarif::Sarif =
            serde_json::from_str(multi).expect("schema-valid multi-run");
        assert_eq!(typed_multi.runs.len(), 2);
        let findings = triage_sarif(multi).expect("multi-run");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule, "gitleaks/a");
        assert_eq!(findings[1].path, Some("src/b.py".to_owned()));
    }
}
