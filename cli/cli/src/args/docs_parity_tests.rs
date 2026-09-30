use std::path::{Path, PathBuf};

use clap::{CommandFactory, ValueEnum};

use super::super::grammar::Cli;
use super::super::{parse, ArgsError, Command};

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

fn workspace_root() -> PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    Path::new(&root).join(workspace)
}

fn docs_dir() -> PathBuf {
    workspace_root().join("docs/cli/commands")
}

fn pages() -> Vec<(String, String)> {
    let mut got: Vec<(String, String)> = std::fs::read_dir(docs_dir())
        .expect("docs/cli/commands must ship as test data")
        .map(|entry| entry.expect("readable dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| {
            (
                path.file_stem()
                    .expect("docs file stem")
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read_to_string(&path).expect("readable docs page"),
            )
        })
        .collect();
    got.sort();
    assert!(!got.is_empty(), "no command docs found");
    got
}

fn flag_tokens(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut flags = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '-' || chars.get(index + 1) != Some(&'-') {
            index += 1;
            continue;
        }
        let start = index;
        let mut end = index + 2;
        while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '-') {
            end += 1;
        }
        if end > start + 2 {
            flags.push(chars[start..end].iter().collect());
        }
        index = end;
    }
    flags.sort();
    flags.dedup();
    flags
}

fn rejects(command: Command, flag: &str, payload: Option<&str>) -> bool {
    let mut words = vec![command.name()];
    if command == Command::Docs {
        words.push("--serve");
    }
    words.push(flag);
    if let Some(payload) = payload {
        words.push(payload);
    }
    match parse(&args(&words)) {
        Err(ArgsError::UnsupportedOption { option, .. }) => option == flag,
        Err(ArgsError::UnknownOption { option, .. }) => option == flag,
        _ => false,
    }
}

fn global_flags() -> Vec<String> {
    Cli::command()
        .get_arguments()
        .filter_map(|arg| {
            arg.get_long_and_visible_aliases().map(|names| {
                names
                    .iter()
                    .map(|name| format!("--{name}"))
                    .collect::<Vec<_>>()
            })
        })
        .flatten()
        .collect()
}

fn shell_blocks(page: &str) -> Vec<Vec<String>> {
    fenced_blocks(page, "```sh")
}

fn usage_blocks(page: &str) -> Vec<Vec<String>> {
    fenced_blocks(page, "```text")
        .into_iter()
        .filter(|block| {
            block
                .iter()
                .any(|line| line.trim_start().starts_with("dx "))
        })
        .map(|block| {
            block
                .into_iter()
                .map(|line| line.trim().to_owned())
                .filter(|line| !line.is_empty())
                .collect()
        })
        .collect()
}

fn fenced_blocks(page: &str, fence: &str) -> Vec<Vec<String>> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut open = false;
    for line in page.lines() {
        if !open && line.trim() == fence {
            open = true;
            current.clear();
            continue;
        }
        if open && line.trim() == "```" {
            open = false;
            blocks.push(std::mem::take(&mut current));
            continue;
        }
        if open {
            current.push(line.to_owned());
        }
    }
    assert!(!open, "unterminated {fence} block");
    blocks
}

fn sections(page: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut heading = String::new();
    let mut body: Vec<&str> = Vec::new();
    for line in page.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            out.push((std::mem::take(&mut heading), body.join("\n")));
            heading = rest.trim().to_owned();
            body.clear();
            continue;
        }
        body.push(line);
    }
    out.push((heading, body.join("\n")));
    out
}

fn documents_exit_codes(text: &str) -> bool {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.to_lowercase().contains("exit code")
}

fn slug(heading: &str) -> String {
    heading
        .trim_start_matches('#')
        .trim()
        .trim_matches('`')
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

fn anchors(page: &str) -> Vec<String> {
    page.lines()
        .filter(|line| line.starts_with('#'))
        .map(|line| slug(line))
        .collect()
}

#[test]
fn command_usage_only_advertises_accepted_flags() {
    for command in Command::value_variants() {
        let command = *command;
        let usage = command.usage();
        for flag in flag_tokens(usage) {
            assert!(
                !rejects(command, &flag, None),
                "dx {} advertises {flag} in its usage but rejects it:\n{usage}",
                command.name()
            );
        }
        if usage.contains("[-- bazel-options") || usage.contains("[-- bazel-args") {
            assert!(
                !rejects(command, "--", Some("--jobs=1")),
                "dx {} advertises `--` in its usage but rejects it:\n{usage}",
                command.name()
            );
        }
    }
}

#[test]
fn docs_usage_blocks_only_use_accepted_flags() {
    let globals = global_flags();
    for (name, page) in pages() {
        for block in usage_blocks(&page) {
            for line in &block {
                let words: Vec<&str> = line.split_whitespace().collect();
                assert_eq!(
                    words[0], "dx",
                    "{name}: usage line must start with dx: {line}"
                );
                let command = Command::parse(words[1]).unwrap_or_else(|| {
                    panic!("{name}: usage line names an unknown command: {line}")
                });
                let mut accepted = globals.clone();
                accepted.extend(flag_tokens(command.usage()));
                accepted.extend(flag_tokens(command.flags()));
                for flag in flag_tokens(line) {
                    assert!(
                        accepted.contains(&flag),
                        "{name}: dx {} usage block uses {flag} but dx {command:?} does not accept it: {line}",
                        command.name()
                    );
                }
            }
        }
    }
}

#[test]
fn docs_shell_examples_parse() {
    const DX_PREFIX: &str = "bazel run //cli/cli:dx --";
    const ENV_LAUNCHER: &str = "bazel run //dx:env";
    for (name, page) in pages() {
        for block in shell_blocks(&page) {
            for line in &block {
                let trimmed = line.trim();
                assert!(
                    trimmed.starts_with(DX_PREFIX) || trimmed == ENV_LAUNCHER,
                    "{name}: shell example must run the dx launcher: {line}"
                );
                if trimmed == ENV_LAUNCHER {
                    continue;
                }
                let words: Vec<String> = trimmed
                    .strip_prefix(DX_PREFIX)
                    .expect("checked above")
                    .split_whitespace()
                    .take_while(|word| *word != ">")
                    .map(ToOwned::to_owned)
                    .collect();
                assert!(!words.is_empty(), "{name}: empty example: {line}");
                if words.first().is_some_and(|word| word == "--help") {
                    continue;
                }
                let parsed = parse(&words);
                assert!(
                    parsed.is_ok(),
                    "{name}: example does not parse: {line}\n{parsed:?}"
                );
            }
        }
    }
}

fn report_formats(line: &str) -> Vec<String> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut formats = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let token = token.trim_start_matches('[');
        let payload = match token.strip_prefix("--report=") {
            Some(payload) => payload.to_owned(),
            None if token == "--report" => match tokens.get(index + 1) {
                Some(next) => (*next).to_owned(),
                None => continue,
            },
            None => continue,
        };
        for choice in payload.split('|') {
            let format = choice
                .split(['<', '='])
                .next()
                .unwrap_or_default()
                .trim_end_matches(']');
            if !format.is_empty() {
                formats.push(format.to_owned());
            }
        }
    }
    formats
}

#[test]
fn docs_report_formats_match_the_command_registry() {
    for (name, page) in pages() {
        for block in usage_blocks(&page) {
            for line in &block {
                let words: Vec<&str> = line.split_whitespace().collect();
                let command = Command::parse(words[1]).unwrap_or_else(|| {
                    panic!("{name}: usage line names an unknown command: {line}")
                });
                let supported = crate::plan::spec(command).reports;
                for format in report_formats(line) {
                    assert!(
                        supported.contains(&format.as_str()),
                        "{name}: dx {command:?} documents --report {format} but the registry allows only {supported:?}: {line}"
                    );
                }
            }
        }
    }
}

fn flag_bullet(page: &str, flag: &str) -> String {
    let head = format!("- `--{flag} ");
    let mut bullet: Option<String> = None;
    let mut continued = String::new();
    for line in page.lines() {
        if line.starts_with(&head) {
            bullet = Some(line.to_owned());
            continued.clear();
            continue;
        }
        if bullet.is_none() {
            continue;
        }
        if line.starts_with("  ") {
            continued.push_str(line);
            continue;
        }
        break;
    }
    format!("{}{continued}", bullet.expect("global flag bullet"))
}

fn backticked(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let tail = &rest[start + 1..];
        let Some(end) = tail.find('`') else {
            break;
        };
        out.push(tail[..end].to_owned());
        rest = &tail[end + 1..];
    }
    out
}

fn registry_reports() -> Vec<String> {
    let mut formats: Vec<String> = Command::value_variants()
        .iter()
        .flat_map(|command| {
            crate::plan::spec(*command)
                .reports
                .iter()
                .map(|format| (*format).to_owned())
        })
        .collect();
    formats.sort();
    formats.dedup();
    formats
}

#[test]
fn global_flags_page_names_every_reporting_command_and_format() {
    let page =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    let bullet = flag_bullet(&page, "report");
    let mut documented: Vec<String> = backticked(&bullet)
        .into_iter()
        .filter(|token| Command::parse(token).is_some())
        .collect();
    documented.sort();
    documented.dedup();
    let mut reporting: Vec<String> = Command::value_variants()
        .iter()
        .copied()
        .filter(|command| !crate::plan::spec(*command).reports.is_empty())
        .map(|command| command.name().to_owned())
        .collect();
    reporting.sort();
    assert_eq!(
        documented, reporting,
        "docs/cli/commands/README.md --report bullet must name every command with a report format"
    );
    for format in registry_reports() {
        assert!(
            bullet.contains(&format!("`{format}`")),
            "docs/cli/commands/README.md --report bullet never names the {format} format"
        );
    }
}

#[test]
fn global_flags_page_names_exactly_the_narrow_output_commands() {
    let page =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    let bullet = flag_bullet(&page, "output");
    let mut documented: Vec<String> = backticked(&bullet)
        .into_iter()
        .filter(|token| Command::parse(token).is_some())
        .collect();
    documented.sort();
    documented.dedup();
    let mut narrow: Vec<String> = Command::value_variants()
        .iter()
        .copied()
        .filter(|command| command.supports_diff() || !command.supports_json())
        .map(|command| command.name().to_owned())
        .collect();
    narrow.sort();
    assert_eq!(
        documented, narrow,
        "docs/cli/commands/README.md --output bullet must name every command that accepts `diff` or rejects `json`"
    );
}

#[test]
fn docs_sections_with_usage_blocks_document_output_modes() {
    for (name, page) in pages() {
        for (heading, body) in sections(&page) {
            for line in usage_lines(&body) {
                let command = Command::parse(&line[1]).unwrap_or_else(|| {
                    panic!("{name}: usage line names an unknown command: {line:?}")
                });
                let modes = super::super::help::output_modes(command);
                let where_ = if heading.is_empty() {
                    "the page intro".to_owned()
                } else {
                    format!("section {heading:?}")
                };
                assert!(
                    body.contains(&format!("`--output {modes}`")),
                    "{name}: {where_} documents dx {} usage but never states `--output {modes}`",
                    command.name()
                );
            }
        }
    }
}

fn usage_lines(body: &str) -> Vec<Vec<String>> {
    usage_blocks(body)
        .into_iter()
        .flat_map(|block| {
            block
                .into_iter()
                .filter(|line| line.starts_with("dx "))
                .map(|line| {
                    line.split_whitespace()
                        .map(ToOwned::to_owned)
                        .collect::<Vec<String>>()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn docs_sections_with_usage_blocks_document_exit_codes() {
    for (name, page) in pages() {
        for (heading, body) in sections(&page) {
            if usage_blocks(&body).is_empty() {
                continue;
            }
            assert!(
                documents_exit_codes(&body),
                "{name}: section {heading:?} has a usage block but no exit codes"
            );
        }
    }
}

#[test]
fn every_command_has_a_docs_page() {
    let all: String = pages()
        .iter()
        .map(|(_, page)| page.clone())
        .collect::<Vec<_>>()
        .join("\n");
    for command in Command::value_variants() {
        let needle = format!("dx {}", command.name());
        assert!(
            all.contains(&needle),
            "no docs/cli/commands page mentions {needle}"
        );
    }
}

#[test]
fn help_doc_anchors_resolve() {
    for command in Command::value_variants() {
        let texts = [
            command.describe(),
            command.usage(),
            command.flags(),
            command.scopes_text(),
        ];
        for text in texts {
            let mut rest = text;
            while let Some(start) = rest.find("docs/") {
                let tail = &rest[start..];
                rest = &tail[1..];
                let end = tail
                    .find(|c: char| c.is_whitespace() || c == ')' || c == '`')
                    .unwrap_or(tail.len());
                let reference = &tail[..end];
                let Some((file, anchor)) = reference.split_once('#') else {
                    continue;
                };
                let path = workspace_root().join(file);
                let page = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                assert!(
                    anchors(&page).iter().any(|slug| slug == anchor),
                    "dx {} help points at {reference} but {file} has no matching heading",
                    command.name()
                );
            }
        }
    }
}

fn gated_check_ids(workflow: &str) -> Vec<String> {
    let mut ids: Vec<String> = workflow
        .lines()
        .filter_map(|line| {
            line.split_once("inputs.disabled_checks), ',")
                .and_then(|(_, tail)| tail.split_once(",')"))
                .map(|(id, _)| id.to_owned())
        })
        .collect();
    ids.sort();
    ids
}

fn platform_labels(workflow: &str) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    for line in workflow.lines() {
        let Some((_, table)) = line.split_once("fromJSON('{") else {
            continue;
        };
        let Some((body, _)) = table.split_once('}') else {
            continue;
        };
        for entry in body.split(',') {
            if let Some((label, _)) = entry.split_once(':') {
                let label = label.trim().trim_matches('"');
                if !label.is_empty() && !label.contains(char::is_whitespace) {
                    labels.push(label.to_owned());
                }
            }
        }
    }
    labels.sort();
    labels.dedup();
    labels
}

#[test]
fn docs_page_names_every_platform_label() {
    let path = workspace_root().join(".github/workflows/reusable-consumer.yml");
    let workflow = std::fs::read_to_string(&path).expect("workflow ships as test data");
    let labels = platform_labels(&workflow);
    assert_eq!(labels.len(), 5, "consumer workflow maps five platforms");
    let page = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");
    for label in &labels {
        assert!(
            page.contains(&format!("`{label}`")),
            "docs/github-ci.md never names the {label} platform"
        );
    }
}

#[test]
fn caller_passes_platforms_as_a_json_array() {
    let template =
        std::fs::read_to_string(workspace_root().join("examples/consumer-ci/caller.yml"))
            .expect("caller ships as test data");
    let value = template
        .lines()
        .skip_while(|line| line.trim() != "with:")
        .find_map(|line| line.trim().strip_prefix("platforms:"))
        .expect("caller sets platforms");
    let value = value.trim();
    assert!(
        value.starts_with("'[\"") && value.ends_with("]'"),
        "caller platforms must be a quoted JSON array, got {value}"
    );
    let requested = value
        .trim_matches('\'')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|label| label.trim().trim_matches('"').to_owned())
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    assert!(!requested.is_empty(), "caller requests no platforms");
    let path = workspace_root().join(".github/workflows/reusable-consumer.yml");
    let workflow = std::fs::read_to_string(&path).expect("workflow ships as test data");
    for label in requested {
        assert!(
            platform_labels(&workflow).contains(&label),
            "caller platform {label} is not in the workflow label table"
        );
    }
}

fn workflow_call_inputs(workflow: &str) -> Vec<String> {
    let mut inputs: Vec<String> = Vec::new();
    let mut inside = false;
    for line in workflow.lines() {
        if line == "    inputs:" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if !line.starts_with("      ") {
            break;
        }
        let Some(name) = line
            .strip_prefix("      ")
            .and_then(|rest| rest.strip_suffix(':'))
        else {
            continue;
        };
        if !name.is_empty() && !name.contains(char::is_whitespace) {
            inputs.push(name.to_owned());
        }
    }
    inputs.sort();
    inputs
}

#[test]
fn github_ci_page_matches_the_consumer_workflow() {
    let workflow =
        std::fs::read_to_string(workspace_root().join(".github/workflows/reusable-consumer.yml"))
            .expect("reusable-consumer.yml ships as test data");
    let page = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");

    let ids = gated_check_ids(&workflow);
    assert_eq!(ids.len(), 9, "consumer workflow gates nine checks");
    for id in &ids {
        assert!(
            page.contains(&format!("`{id}`")),
            "docs/github-ci.md never names the {id} check"
        );
    }

    for command in Command::value_variants() {
        for line in page.lines() {
            if !line.contains(&format!("dx {} ", command.name())) || !line.contains("--check") {
                continue;
            }
            assert!(
                !rejects(*command, "--check", None),
                "docs/github-ci.md pairs dx {} with --check but dx rejects it: {line}",
                command.name()
            );
        }
    }
}

fn workflow_call_secrets(workflow: &str) -> Vec<String> {
    let mut secrets: Vec<String> = Vec::new();
    let mut inside = false;
    for line in workflow.lines() {
        if line == "    secrets:" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if !line.starts_with("      ") {
            break;
        }
        let Some(name) = line
            .strip_prefix("      ")
            .and_then(|rest| rest.strip_suffix(':'))
        else {
            continue;
        };
        if !name.is_empty() && !name.contains(char::is_whitespace) {
            secrets.push(name.to_owned());
        }
    }
    secrets.sort();
    secrets
}

fn workflow_check_names(workflow: &str) -> Vec<String> {
    let mut jobs: Vec<(String, Option<String>)> = Vec::new();
    let mut in_jobs = false;
    for line in workflow.lines() {
        if line == "jobs:" {
            in_jobs = true;
            continue;
        }
        if !in_jobs {
            continue;
        }
        if let Some(id) = line
            .strip_prefix("  ")
            .and_then(|rest| rest.strip_suffix(':'))
        {
            if !id.is_empty() && !id.contains(char::is_whitespace) {
                jobs.push((id.to_owned(), None));
            }
        } else if let Some(name) = line.strip_prefix("    name: ") {
            if let Some(job) = jobs.last_mut() {
                job.1 = Some(name.trim_matches('"').to_owned());
            }
        }
    }
    jobs.into_iter()
        .map(|(id, name)| name.unwrap_or(id))
        .collect()
}

fn workflow_callers() -> [(&'static str, &'static str); 2] {
    [
        ("reusable-consumer.yml", "examples/consumer-ci/caller.yml"),
        ("reusable-docs.yml", "examples/docs-ci/caller.yml"),
    ]
}

#[test]
fn callers_forward_every_workflow_secret() {
    let page = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");
    for (workflow, caller) in workflow_callers() {
        let root = workspace_root();
        let text = std::fs::read_to_string(root.join(".github/workflows").join(workflow))
            .expect("workflow ships as test data");
        let secrets = workflow_call_secrets(&text);
        assert!(!secrets.is_empty(), "{workflow} declares no secrets");
        let template =
            std::fs::read_to_string(root.join(caller)).expect("caller ships as test data");
        for secret in secrets {
            assert!(
                page.contains(&secret),
                "docs/github-ci.md never names the {workflow} secret {secret}"
            );
            assert!(
                template.contains("secrets: inherit") || template.contains(&secret),
                "{caller} never forwards the {workflow} secret {secret}"
            );
        }
    }
}

#[test]
fn docs_page_names_every_workflow_check() {
    let page = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");
    for (workflow, _) in workflow_callers() {
        let path = workspace_root().join(".github/workflows").join(workflow);
        let text = std::fs::read_to_string(&path).expect("workflow ships as test data");
        let checks = workflow_check_names(&text);
        assert!(!checks.is_empty(), "{workflow} declares no jobs");
        for check in checks {
            assert!(
                page.contains(&format!("`{check}`")),
                "docs/github-ci.md never names the {workflow} check {check}"
            );
        }
    }
}

#[test]
fn docs_page_names_every_workflow_input() {
    let page = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");
    for (workflow, _) in workflow_callers() {
        let path = workspace_root().join(".github/workflows").join(workflow);
        let inputs = workflow_call_inputs(
            &std::fs::read_to_string(&path).expect("workflow ships as test data"),
        );
        assert!(!inputs.is_empty(), "{workflow} declares no inputs");
        for input in inputs {
            assert!(
                page.contains(&format!("`{input}`")),
                "docs/github-ci.md never names the {workflow} input {input}"
            );
        }
    }
}

fn ignored_dir(text: &str, dir: &str) -> bool {
    text.lines()
        .map(str::trim)
        .any(|line| line == format!("{dir}/"))
}

fn bazelignored_dirs(text: &str) -> Vec<String> {
    let mut dirs: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('.') && !line.starts_with(".."))
        .map(|line| line.trim_end_matches('/').to_owned())
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

#[test]
fn dx_state_dir_is_ignored_by_git_bazel_and_editors() {
    let root = workspace_root();
    let dir = dx_env::DX_DIR_NAME;
    let gitignore = std::fs::read_to_string(root.join(".gitignore")).expect("readable .gitignore");
    let bazelignore =
        std::fs::read_to_string(root.join(".bazelignore")).expect("readable .bazelignore");
    let biome = std::fs::read_to_string(root.join("biome.json")).expect("readable biome.json");
    assert!(
        ignored_dir(&gitignore, dir),
        ".gitignore never ignores {dir}/"
    );
    assert!(
        ignored_dir(&bazelignore, dir),
        ".bazelignore never ignores {dir}/"
    );
    assert!(
        biome.contains(&format!("\"!!**/{dir}\"")),
        "biome.json files.includes never force-ignores {dir}"
    );
}

#[test]
fn every_dot_dir_bazel_ignores_is_ignored_by_git_and_editors() {
    let root = workspace_root();
    let bazelignore =
        std::fs::read_to_string(root.join(".bazelignore")).expect("readable .bazelignore");
    let gitignore = std::fs::read_to_string(root.join(".gitignore")).expect("readable .gitignore");
    let biome = std::fs::read_to_string(root.join("biome.json")).expect("readable biome.json");
    let dirs = bazelignored_dirs(&bazelignore);
    assert!(!dirs.is_empty(), ".bazelignore lists no dot dirs");
    for dir in dirs {
        assert!(
            ignored_dir(&gitignore, &dir),
            ".gitignore never ignores {dir}/ listed in .bazelignore"
        );
        assert!(
            biome.contains(&format!("\"!!**/{dir}\"")),
            "biome.json files.includes never force-ignores {dir} listed in .bazelignore"
        );
    }
}

fn documented_env_names(page: &str) -> Vec<String> {
    let (_, body) = sections(page)
        .into_iter()
        .find(|(heading, _)| heading == "Environment")
        .expect("docs/cli/commands/README.md has an Environment section");
    let mut names: Vec<String> = body
        .lines()
        .filter_map(|line| line.strip_prefix("- `"))
        .filter_map(|line| line.split(['=', '`']).next())
        .filter(|name| name.chars().any(|c| c.is_ascii_uppercase()))
        .map(|name| name.trim().to_owned())
        .collect();
    names.sort();
    names.dedup();
    names
}

#[test]
fn environment_section_documents_every_parsed_env_default() {
    let page =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "Environment")
        .map(|(_, body)| body)
        .expect("Environment section");
    let expected: Vec<String> = dx_adopt::defaults::ENV_DEFAULTS
        .iter()
        .map(|(env, _, _)| (*env).to_owned())
        .chain([
            "RUST_LOG".to_owned(),
            "NO_COLOR".to_owned(),
            "BUILD_WORKSPACE_DIRECTORY".to_owned(),
        ])
        .collect();
    assert_eq!(
        documented_env_names(&page),
        {
            let mut sorted = expected.clone();
            sorted.sort();
            sorted
        },
        "docs/cli/commands/README.md Environment must name every env default dx reads, and nothing else"
    );
    for (_, flag, _) in dx_adopt::defaults::ENV_DEFAULTS {
        assert!(
            body.contains(&format!("default for `{flag}`")),
            "docs/cli/commands/README.md never states which flag {flag} comes from"
        );
    }
}

#[test]
fn every_env_default_names_a_real_global_flag() {
    for (env, flag, _) in dx_adopt::defaults::ENV_DEFAULTS {
        assert!(
            global_flags().contains(&flag.to_owned()),
            "{env} defaults {flag}, which is not a global flag"
        );
        assert_eq!(
            dx_adopt::defaults::config_key(env),
            Some(flag.trim_start_matches("--")),
            "{env} has no matching .dx/config.toml key"
        );
    }
}

#[test]
fn config_file_keys_are_the_underscore_spellings_the_parser_reads() {
    let text = "[dx]\nworkspace = \"/w\"\ncolor = \"never\"\ndry_run = true\nfail_on = \"error\"\n";
    let parsed = dx_adopt::defaults::parse_file_text(text).expect("keys parse");
    assert_eq!(parsed.workspace, Some("/w".to_owned()));
    assert_eq!(parsed.color, Some("never".to_owned()));
    assert_eq!(parsed.dry_run, Some(true));
    assert_eq!(parsed.fail_on, Some("error".to_owned()));
    for (env, _, _) in dx_adopt::defaults::ENV_DEFAULTS {
        let key = dx_adopt::defaults::config_key(env).expect("every default has a key");
        let spelled = key.replace('-', "_");
        let body = format!("[dx]\n{spelled} = {}\n", literal_for(&key));
        dx_adopt::defaults::parse_file_text(&body)
            .unwrap_or_else(|error| panic!("docs promise {key} works in a config file: {error}"));
    }
}

fn literal_for(key: &str) -> &'static str {
    match key {
        "dry-run" | "quiet" | "verbose" => "true",
        "fail-on" => "\"error\"",
        "color" => "\"never\"",
        "output" => "\"json\"",
        _ => "\"/w\"",
    }
}

fn banner() -> String {
    super::super::help::usage_banner()
}

fn set_names_in_list(text: &str) -> Vec<String> {
    let start = text
        .find("Sets: ")
        .unwrap_or_else(|| panic!("no `Sets:` list in:\n{text}"));
    let rest = &text[start + "Sets: ".len()..];
    let end = rest.find('.').expect("the Sets: list ends a sentence");
    let mut names: Vec<String> = rest[..end]
        .split(',')
        .map(|name| name.trim().trim_matches('`').to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    names
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

#[test]
fn update_docs_page_names_exactly_the_update_sets() {
    let page = std::fs::read_to_string(docs_dir().join("audit-update-bazel.md"))
        .expect("audit page ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "`dx update`")
        .map(|(_, body)| body)
        .expect("`dx update` section");
    assert_eq!(
        set_names_in_list(&body),
        sorted(
            dx_update::sets::SetId::names()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect()
        ),
        "docs/cli/commands/audit-update-bazel.md must name every dx update set, and nothing else"
    );
}

#[test]
fn bump_docs_page_names_exactly_the_bump_sets() {
    let page = std::fs::read_to_string(docs_dir().join("audit-update-bazel.md"))
        .expect("audit page ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "`dx bump`")
        .map(|(_, body)| body)
        .expect("`dx bump` section");
    assert_eq!(
        set_names_in_list(&body),
        sorted(
            dx_bump::BumpSet::ALL
                .iter()
                .map(|set| (*set).name().to_owned())
                .collect()
        ),
        "docs/cli/commands/audit-update-bazel.md must name every dx bump set, and nothing else"
    );
}

#[test]
fn update_help_scopes_name_exactly_the_update_sets() {
    let sets: Vec<String> = dx_update::sets::SetId::names()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
    let update = scoped_set_list(Command::Update.scopes_text());
    assert_eq!(
        sorted(update),
        sorted(sets.clone()),
        "dx update --help must name every update set, and nothing else"
    );
    let bump = scoped_set_list(Command::Bump.scopes_text());
    assert_eq!(
        sorted(bump),
        sorted(
            dx_bump::BumpSet::ALL
                .iter()
                .map(|set| (*set).name().to_owned())
                .collect()
        ),
        "dx bump --help must name every bump set, and nothing else"
    );
}

fn scoped_set_list(scopes: &str) -> Vec<String> {
    let open = scopes
        .rfind('(')
        .unwrap_or_else(|| panic!("Scopes: names no parenthesised set list:\n{scopes}"));
    let rest = &scopes[open + 1..];
    let end = rest
        .find(')')
        .unwrap_or_else(|| panic!("Scopes: set list is unterminated:\n{scopes}"));
    rest[..end]
        .split('|')
        .map(|name| name.split(',').next().unwrap_or(name).trim().to_owned())
        .collect()
}

#[test]
fn unknown_set_and_selector_errors_name_every_set() {
    let unknown_set = dx_update::manifest::ManifestError::UnknownSet {
        set: "nope".to_owned(),
    }
    .to_string();
    assert!(
        unknown_set.contains(&dx_update::sets::SetId::name_list()),
        "the unknown-set error never names every set: {unknown_set}"
    );
    let unknown_selector = dx_update::selector::SelectorError::UnknownSelector {
        selector: "nope".to_owned(),
    }
    .to_string();
    assert!(
        unknown_selector.contains(&dx_update::sets::SetId::pipe_list()),
        "the unknown-selector error never names every set: {unknown_selector}"
    );
    let unknown_bump = dx_bump::BumpError::UnknownSelector {
        selector: "nope:thing".to_owned(),
    }
    .to_string();
    assert!(
        unknown_bump.contains(&dx_bump::BumpSet::pipe_list()),
        "the unknown-bump-selector error never names every set: {unknown_bump}"
    );
    let unknown_discovery = dx_bump::discovery::DiscoveryError::UnknownSelector {
        selector: "nope:thing".to_owned(),
    }
    .to_string();
    assert!(
        unknown_discovery.contains(&dx_bump::BumpSet::pipe_list()),
        "the unknown-discovery-selector error never names every set: {unknown_discovery}"
    );
}

#[test]
fn every_usage_error_prints_the_same_banner() {
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
    let pre_exec_error = rendered(&["lint", "--dry-run", "--report=sarif=out.sarif"]);
    let expected = banner();
    for (label, text) in [("parse", &parse_error), ("pre-exec", &pre_exec_error)] {
        assert!(
            text.contains(&expected),
            "the {label} usage error does not print usage_banner():\n{text}"
        );
        assert_eq!(
            text.matches("usage: dx").count(),
            1,
            "the {label} usage error prints the banner more than once:\n{text}"
        );
    }
}

#[test]
fn the_usage_banner_names_every_command() {
    let text = banner();
    let marker = format!("<{}>", Command::pipe_list());
    assert!(
        text.contains(&marker),
        "the usage banner must list the registry commands as {marker}"
    );
}

const SCOPE_PAGE_POLICIES: [(&str, &[&str]); 7] = [
    ("No Scope Means `//...`", &["default-//..."]),
    ("No Scope Means The Repository", &["default-repo"]),
    (
        "Repository-Wide Or One Exact Label",
        &["default-repo|exact-label"],
    ),
    (
        "Set Selectors",
        &["selector-default-all", "require-selector+version"],
    ),
    (
        "Required Arguments",
        &[
            "require",
            "require-label",
            "require-file+label",
            "optional-name",
        ],
    ),
    ("No Scopes", &["reject"]),
    ("Raw Bazel Args", &["passthrough"]),
];

fn scope_page() -> String {
    std::fs::read_to_string(docs_dir().join("scope-defaults.md"))
        .expect("scope-defaults.md ships as test data")
}

fn named_commands(text: &str) -> Vec<String> {
    backticked(text)
        .into_iter()
        .filter_map(|token| token.strip_prefix("dx ").map(ToOwned::to_owned))
        .collect()
}

fn scope_page_section(page: &str, heading: &str) -> String {
    sections(page)
        .into_iter()
        .find(|(name, _)| name == heading)
        .map(|(_, body)| body)
        .unwrap_or_else(|| panic!("scope-defaults.md has no `{heading}` section"))
}

#[test]
fn scope_page_sections_match_the_registry_policies() {
    let page = scope_page();
    for (heading, policies) in SCOPE_PAGE_POLICIES {
        let mut expected: Vec<String> = Command::value_variants()
            .iter()
            .copied()
            .filter(|command| policies.contains(&command.scope_policy()))
            .map(|command| command.name().to_owned())
            .collect();
        expected.sort();
        let mut found = named_commands(&scope_page_section(&page, heading));
        found.sort();
        assert_eq!(
            found, expected,
            "docs/cli/commands/scope-defaults.md section `{heading}` must name exactly the commands whose scope policy is {policies:?}"
        );
    }
}

#[test]
fn scope_page_names_every_command_exactly_once() {
    let page = scope_page();
    let mut seen: Vec<String> = Vec::new();
    for (heading, _) in SCOPE_PAGE_POLICIES {
        seen.extend(named_commands(&scope_page_section(&page, heading)));
    }
    let mut expected: Vec<String> = Command::value_variants()
        .iter()
        .map(|command| command.name().to_owned())
        .collect();
    expected.sort();
    seen.sort();
    assert_eq!(
        seen, expected,
        "the scope-defaults.md policy sections must classify every command once"
    );
    let shapes = scope_page_section(&page, "Scope Shapes");
    for name in named_commands(&shapes) {
        assert!(
            Command::parse(&name).is_some(),
            "scope-defaults.md Scope Shapes names an unknown command `dx {name}`"
        );
    }
}

#[test]
fn scope_page_here_bullet_names_exactly_the_here_commands() {
    let page = scope_page();
    let body = scope_page_section(&page, "Scope Shapes");
    let mut bullets = body.lines().filter(|line| line.starts_with("- `--here`"));
    let mut text = bullets
        .next()
        .expect("Scope Shapes documents `--here` in a bullet")
        .to_owned();
    assert!(
        bullets.next().is_none(),
        "Scope Shapes has two `--here` bullets"
    );
    let rest = body
        .lines()
        .skip_while(|line| !line.starts_with("- `--here`"))
        .skip(1)
        .take_while(|line| line.starts_with("  "));
    for line in rest {
        text.push_str(line);
    }
    let mut found = named_commands(&text);
    found.sort();
    let mut expected: Vec<String> = Command::value_variants()
        .iter()
        .copied()
        .filter(|command| command.supports_here())
        .map(|command| command.name().to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        found, expected,
        "the scope-defaults.md `--here` bullet must name every command that accepts `--here`, and nothing else"
    );
}
