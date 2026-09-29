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

fn usage_blocks(page: &str) -> Vec<Vec<String>> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut open = false;
    for line in page.lines() {
        if !open && line.trim() == "```text" {
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
    assert!(!open, "unterminated ```text block");
    blocks
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

#[test]
fn docs_page_names_every_workflow_input() {
    let page = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");
    for workflow in ["reusable-consumer.yml", "reusable-docs.yml"] {
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
