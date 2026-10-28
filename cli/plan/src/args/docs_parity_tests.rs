const BOOTSTRAP_SOURCE: &str = "deploy/offline/bootstrap/src/lib.rs";

use std::path::{Path, PathBuf};

use clap::{CommandFactory, ValueEnum};
use dx_process::{EXIT_OPERATIONAL, EXIT_PRE_EXEC, EXIT_SUCCESS, STARTUP_OPTIONS};

use super::super::command::SkewKind;
use super::super::grammar::Cli;
use super::super::tokenizer::REMOVED_COMMANDS;
use super::super::{parse, ArgsError, Command, Invocation};
use super::{render_command_help, render_top_help, usage_banner};
use dx_testing::cli::strings;

const DX_PREFIX: &str = "bazel run @rules_dx//:dx --";

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

fn example_readmes() -> Vec<(String, String)> {
    let dir = workspace_root().join("examples");
    let mut got: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("examples must ship as test data")
        .map(|entry| entry.expect("readable dir entry").path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("adopt-"))
                && path.join("README.md").is_file()
        })
        .map(|path| {
            let readme = path.join("README.md");
            (
                path.file_name()
                    .expect("example dir name")
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read_to_string(&readme).expect("readable example README"),
            )
        })
        .collect();
    got.sort();
    assert!(!got.is_empty(), "no adoption example READMEs found");
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
    match parse(&strings(&words)) {
        Err(ArgsError::UnsupportedOption { option, .. }) => option == flag,
        Err(ArgsError::UnknownOption { option, .. }) => option == flag,
        _ => false,
    }
}

fn required_words(command: Command) -> Vec<String> {
    let required: &[&str] = match command.name() {
        "bump" | "add" => &["cargo:demo", "1.0.0"],
        "migrate" | "upgrade" => &["--from=1.0.0", "--to=2.0.0"],
        "run" | "deploy" => &["//:demo"],
        "watch" => &["run", "//:demo"],
        "completion" => &["bash"],
        "install" => &["--tag", "v1.2.3", "--archive", "a.tar.gz"],
        _ => &[],
    };
    strings(required)
}

fn probe_rejects(command: Command, flag: &str, payload: Option<&str>) -> bool {
    if flag == "-- <bazel-options>" {
        let mut words = vec![command.name().to_owned()];
        words.extend(required_words(command));
        words.push("--".to_owned());
        words.push("--jobs=1".to_owned());
        return matches!(parse(&words), Err(ArgsError::UnsupportedOption { .. }));
    }
    let mut words = vec![command.name().to_owned()];
    words.extend(required_words(command));
    if command == Command::Docs && matches!(flag, "--port" | "--host" | "--open") {
        words.push("--serve".to_owned());
    }
    words.push(flag.to_owned());
    if let Some(payload) = payload {
        words.push(payload.to_owned());
    }
    match parse(&words) {
        Err(ArgsError::UnsupportedOption { option, .. }) => option == flag,
        Err(ArgsError::UnknownOption { option, .. }) => option == flag,
        _ => false,
    }
}

const SHARED_FLAGS: &[(&str, Option<&str>)] = &[
    ("--fail-on", Some("error")),
    ("--min-coverage", Some("80")),
    ("--check", None),
    ("--apply", None),
    ("--debug", None),
    ("--release", None),
    ("--from", Some("2.0.0")),
    ("--to", Some("2.0.0")),
    ("--here", None),
    ("--serve", None),
    ("--port", Some("1")),
    ("--host", Some("example.test")),
    ("--open", None),
    ("--offline", None),
    ("--frozen", None),
    ("--bazel-option", Some("--jobs=1")),
    ("-- <bazel-options>", None),
];

#[test]
fn help_lists_exactly_the_flags_the_parser_accepts() {
    for command in Command::value_variants() {
        let command = *command;
        let listed = super::super::grammar::advertised_flags(command);
        for (flag, payload) in SHARED_FLAGS {
            if command == Command::Watch
                && matches!(
                    *flag,
                    "--check" | "--apply" | "--fail-on" | "--report" | "--here"
                )
            {
                continue;
            }
            let refused = probe_rejects(command, flag, *payload);
            let advertised = if *flag == "-- <bazel-options>" {
                super::super::grammar::advertises_passthrough(command)
            } else {
                listed
                    .iter()
                    .any(|name| name == flag.trim_start_matches('-'))
            };
            assert_eq!(
                refused,
                !advertised,
                "dx {} help {flag}: parser refuses={refused} advertised={advertised}",
                command.name(),
            );
        }
        assert_eq!(
            !listed.iter().any(|name| name == "report") || command == Command::Watch,
            super::output_line(command).contains("no standard format") || command == Command::Watch,
            "dx {} help lists --report against what the Output line promises:\n{}",
            command.name(),
            super::output_line(command),
        );
        assert!(
            !render_command_help(command).contains("Rejected:"),
            "dx {} help carries a rejection list",
            command.name()
        );
    }
}

/// Flags a wrapped command takes that `dx watch` does not forward yet.
const WATCH_WITHHELD_FLAGS: &[&str] = &["bazel-option", "explain"];

#[test]
fn watch_advertises_exactly_the_flags_its_wrapped_commands_take() {
    let mut union: Vec<String> = dx_adopt::WATCHABLE_COMMANDS
        .iter()
        .flat_map(|name| {
            let command = Command::parse(name).expect("watchable names parse");
            super::super::grammar::advertised_flags(command)
        })
        .filter(|flag| !WATCH_WITHHELD_FLAGS.contains(&flag.as_str()))
        .collect();
    union.sort();
    union.dedup();
    let mut watch = super::super::grammar::advertised_flags(Command::Watch);
    watch.sort();
    watch.dedup();
    assert_eq!(watch, union);
}

fn global_flags() -> Vec<String> {
    Cli::command()
        .get_subcommands()
        .next()
        .expect("dx has subcommands")
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

/// The exit codes a page's `Exit codes:` clause documents, each with the words that follow it.
fn documented_exit_codes(text: &str) -> Vec<(i32, String)> {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let needle = "exit code";
    let at = flat
        .to_lowercase()
        .find(needle)
        .unwrap_or_else(|| panic!("the text has no exit-code clause:\n{flat}"));
    let mut out: Vec<(i32, String)> = Vec::new();
    let mut rest = &flat[at + needle.len()..];
    while let Some(start) = rest.find(|c: char| c.is_ascii_digit()) {
        let digits = rest[start..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len() - start);
        let code = rest[start..start + digits]
            .parse::<i32>()
            .expect("a run of ASCII digits");
        let after = &rest[start + digits..];
        let stop = after
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(after.len());
        out.push((
            code,
            after[..stop]
                .trim_matches(|c: char| !c.is_alphabetic())
                .to_owned(),
        ));
        rest = &after[stop..];
    }
    out
}

fn assert_exit_codes(where_: &str, text: &str, want: &[i32]) {
    let documented = documented_exit_codes(text);
    assert!(
        documented.len() >= want.len(),
        "{where_}: the exit-code clause names {} code(s), want {want:?}",
        documented.len()
    );
    for (index, expected) in want.iter().enumerate() {
        let (code, clause) = &documented[index];
        assert_eq!(
            code, expected,
            "{where_}: code {index} is {code}, but dx exits {expected} for that case: {clause:?}"
        );
    }
    if want.first() == Some(&EXIT_SUCCESS) {
        let success = &documented[0].1;
        assert!(
            success.contains("success") || success.contains("pass"),
            "{where_}: {success:?} never calls {EXIT_SUCCESS} success"
        );
    } else {
        assert!(
            documented.iter().all(|(code, _)| *code != EXIT_SUCCESS),
            "{where_}: documents {EXIT_SUCCESS}, but this command never exits {EXIT_SUCCESS}"
        );
    }
    if let Some(at) = want.iter().position(|code| *code == EXIT_PRE_EXEC) {
        let usage = &documented[at].1;
        assert!(
            usage.contains("usage") || usage.contains("scope"),
            "{where_}: {usage:?} never calls {EXIT_PRE_EXEC} a usage or scope error"
        );
    }
}

const EXIT_TRIPLE: [i32; 3] = [EXIT_SUCCESS, EXIT_PRE_EXEC, EXIT_OPERATIONAL];

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
fn command_help_only_advertises_accepted_flags() {
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
fn usage_strings_and_docs_put_dx_flags_after_the_command() {
    for command in Command::value_variants() {
        let command = *command;
        let usage = command.usage();
        let Some(rest) = usage.strip_prefix("Usage: dx ") else {
            panic!(
                "dx {} usage must start with `Usage: dx `: {usage}",
                command.name()
            );
        };
        let slot: Vec<&str> = rest
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .split('|')
            .collect();
        assert!(
            !slot.is_empty() && slot.iter().all(|name| Command::parse(name).is_some()),
            "dx {} usage must name the command before its flags: {usage}",
            command.name()
        );
    }
    let readme =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    assert!(
        readme.contains("flags after the command"),
        "docs/cli/commands/README.md must put dx flags after the command:\n{readme}"
    );
    assert!(
        !readme.contains("flags before the command"),
        "docs/cli/commands/README.md still puts dx flags before the command"
    );
}

fn words_taking_bazel_options(command: Command) -> Vec<String> {
    let mut words = vec![command.name().to_owned()];
    words.extend(required_words(command));
    words.push("--".to_owned());
    words.push("--jobs=1".to_owned());
    words
}

#[test]
fn command_usage_advertises_the_bazel_passthrough_exactly_where_the_parser_accepts_it() {
    for command in Command::value_variants() {
        let command = *command;
        let words = words_taking_bazel_options(command);
        let accepted = parse(&words).is_ok();
        let usage = command.usage();
        let advertised = usage.contains("[-- bazel-options")
            || usage.contains("[-- app-args")
            || usage.contains("[-- bazel-args");
        assert_eq!(
            accepted,
            advertised,
            "dx {} accepts={accepted} advertises={advertised} for `--`: {words:?}\n{usage}",
            command.name()
        );
    }
}

#[test]
fn command_usage_advertises_check_exactly_where_the_parser_accepts_it() {
    for command in Command::value_variants() {
        let command = *command;
        let usage = command.usage();
        let words = vec![command.name(), "--check"];
        let accepted = parse(&strings(&words)).is_ok();
        let advertised = usage.contains("[--check]");
        assert_eq!(
            accepted,
            advertised,
            "dx {} accepts={accepted} advertises={advertised} for --check: {words:?}\n{usage}",
            command.name()
        );
    }
}

#[test]
fn command_usage_advertises_apply_exactly_where_the_parser_accepts_it() {
    for command in Command::value_variants() {
        let command = *command;
        let usage = command.usage();
        let mut words = vec![command.name().to_owned()];
        words.extend(required_words(command));
        if command == Command::Watch {
            words[1] = "fix".to_owned();
        }
        if command == Command::Docs {
            words.push("--serve".to_owned());
        }
        words.push("--apply".to_owned());
        let accepted = parse(&words).is_ok();
        let advertised = usage.contains("[--apply]");
        assert_eq!(
            accepted,
            advertised,
            "dx {} accepts={accepted} advertises={advertised} for --apply: {words:?}\n{usage}",
            command.name()
        );
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
                accepted.extend(
                    super::super::grammar::advertised_flags(command)
                        .iter()
                        .map(|name| format!("--{name}")),
                );
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

fn documented_usage_lines() -> Vec<(String, Command, String)> {
    let mut lines: Vec<(String, Command, String)> = Vec::new();
    for (name, page) in pages() {
        for block in usage_blocks(&page) {
            for line in block {
                let words: Vec<&str> = line.split_whitespace().collect();
                let command = Command::parse(words[1]).unwrap_or_else(|| {
                    panic!("{name}: usage line names an unknown command: {line}")
                });
                lines.push((name.clone(), command, line));
            }
        }
    }
    lines
}

#[test]
fn docs_usage_blocks_document_every_advertised_flag() {
    let all = documented_usage_lines();
    for command in Command::value_variants() {
        let command = *command;
        let documented: Vec<&String> = all
            .iter()
            .filter(|(_, owner, _)| *owner == command)
            .map(|(_, _, line)| line)
            .collect();
        assert!(
            !documented.is_empty(),
            "no docs/cli/commands usage line documents dx {}",
            command.name()
        );
        for flag in flag_tokens(command.usage()) {
            assert!(
                documented
                    .iter()
                    .any(|line| flag_tokens(line).contains(&flag)),
                "dx {} advertises {flag} but no docs usage line states it: {documented:?}",
                command.name()
            );
        }
    }
}

#[test]
fn docs_shell_examples_parse() {
    for (name, page) in pages() {
        for block in shell_blocks(&page) {
            for line in &block {
                let trimmed = line.trim();
                assert!(
                    trimmed.starts_with(DX_PREFIX),
                    "{name}: shell example must run the dx launcher: {line}"
                );
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

fn evidence_paragraph(page: &str) -> Option<String> {
    page.split("\n\n")
        .find(|paragraph| paragraph.contains("tests pass"))
        .map(|paragraph| {
            paragraph
                .split_whitespace()
                .collect::<Vec<&str>>()
                .join(" ")
        })
}

#[test]
fn example_readme_shell_examples_run_dx_or_bazel_on_the_example() {
    for (name, page) in example_readmes() {
        let scope = format!("//examples/{name}/...");
        for block in shell_blocks(&page) {
            for line in &block {
                let trimmed = line.trim();
                if let Some(words) = trimmed.strip_prefix(DX_PREFIX) {
                    let words: Vec<String> = words
                        .split_whitespace()
                        .take_while(|word| *word != ">")
                        .map(ToOwned::to_owned)
                        .collect();
                    let parsed = parse(&words);
                    assert!(
                        parsed.is_ok(),
                        "{name}: example does not parse: {line}\n{parsed:?}"
                    );
                    continue;
                }
                let words: Vec<&str> = trimmed.split_whitespace().collect();
                assert_eq!(
                    words.first(),
                    Some(&"bazel"),
                    "{name}: shell example must run dx or bazel: {line}"
                );
                for word in &words {
                    if word.starts_with("//") {
                        assert_eq!(
                            *word, scope,
                            "{name}: shell example must act on {scope}: {line}"
                        );
                    }
                }
            }
        }
    }
}

fn readme_event_bullet(event: &str) -> String {
    let readme = std::fs::read_to_string(docs_dir().join("README.md")).expect("README.md");
    let start = format!("- `{event}`:");
    let mut lines = readme.lines().skip_while(|line| !line.starts_with(&start));
    let first = lines
        .next()
        .unwrap_or_else(|| panic!("README.md must list the `{event}` event"));
    let mut bullet = first.to_owned();
    for line in lines.take_while(|line| !line.starts_with("- ") && !line.trim().is_empty()) {
        bullet.push(' ');
        bullet.push_str(line.trim());
    }
    bullet
}

#[test]
fn readme_events_list_every_emitted_field() {
    let digest = "0".repeat(64);
    let strings = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    let fixtures: Vec<(&str, serde_json::Value)> = vec![
        (
            "command_started",
            dx_output::command_started("check", "default").expect("command_started"),
        ),
        (
            "operation",
            dx_output::operation_event("check", "plan", Some(&strings(&["//a"])))
                .expect("operation"),
        ),
        (
            "selection",
            dx_output::selection_event(&digest, &digest, &digest).expect("selection"),
        ),
        (
            "status",
            dx_output::status_event(&dx_output::StatusEvent {
                name: "n".to_owned(),
                status: "ok".to_owned(),
                detail: "d".to_owned(),
                hint: "h".to_owned(),
            })
            .expect("status"),
        ),
        (
            "slot",
            dx_output::slot_event(&dx_output::SlotEvent {
                action: "add".to_owned(),
                path: "/slots/1".to_owned(),
                slot: Some(1),
                branch: Some("main".to_owned()),
                holder: Some("agent".to_owned()),
                state: "held".to_owned(),
                reason: Some("dirty".to_owned()),
                base: Some("origin/main".to_owned()),
                server: true,
                disk_bytes: Some(1),
            })
            .expect("slot"),
        ),
        (
            "report",
            dx_output::report_event("junit", "out.xml", true).expect("report"),
        ),
        (
            "summary",
            dx_output::summary_event(&dx_output::SummaryEvent {
                slots: 1,
                servers: 1,
                servers_rss_bytes: 1,
                disk_bytes: Some(1),
                disk_cache_bytes: Some(1),
            }),
        ),
        (
            "notice",
            dx_output::notice_event(&dx_output::NoticeEvent {
                level: "info".to_owned(),
                code: "c".to_owned(),
                message: "m".to_owned(),
                related_command: Some("check".to_owned()),
                scope: Some(strings(&["//a"])),
                path: Some("a.rs".to_owned()),
                language: Some("rust".to_owned()),
                import: Some("x".to_owned()),
            })
            .expect("notice"),
        ),
        (
            "coverage",
            dx_output::coverage_event(&dx_output::CoverageEvent {
                cell: "c".to_owned(),
                covered_lines: 1,
                eligible_lines: 1,
                raw_lines: 1,
                excluded_lines: 0,
                eligible_files: 1,
                support_files: 0,
                excluded_files: 0,
                unsupported_files: 0,
                minimum: Some(80),
                passed: true,
                errors: 0,
                dx_version: "1".to_owned(),
                sha: Some("abc".to_owned()),
                dirty: false,
                collector: Some("bazel 9.0.0".to_owned()),
            })
            .expect("coverage"),
        ),
        (
            "capability",
            dx_output::capability_value(&dx_output::CapabilityEvent {
                name: "n".to_owned(),
                source: "cli-grammar".to_owned(),
                availability: "available".to_owned(),
                detail: "d".to_owned(),
                flags: strings(&["--f"]),
                outputs: strings(&["o"]),
                reports: strings(&["r"]),
                scope_policy: "p".to_owned(),
                effect: "e".to_owned(),
                passthrough: false,
            }),
        ),
        (
            "error",
            dx_output::error_event("c", "m", Some("a.rs"), Some("--f"), Some("plan"))
                .expect("error"),
        ),
    ];
    let mut problems = Vec::new();
    for (name, event) in &fixtures {
        assert!(
            dx_output::EVENTS.contains(name),
            "`{name}` is not a known event"
        );
        assert_eq!(
            event["event"], *name,
            "fixture for `{name}` emits another event"
        );
        let bullet = readme_event_bullet(name);
        let object = event.as_object().expect("event is an object");
        for key in object.keys().filter(|k| *k != "event" && *k != "schema") {
            if !bullet.contains(&format!("`{key}`")) {
                problems.push(format!("`{name}` omits `{key}`"));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "README.md event bullets omit emitted fields: {problems:?}"
    );
}

#[test]
fn user_facing_text_launches_dx_through_the_public_label() {
    const CRATE_LABEL: &str = "//cli/cli:dx";
    const PUBLIC_LABEL: &str = "@rules_dx//:dx";
    let mut texts = pages();
    texts.extend(example_readmes());
    texts.push((
        "README.md".to_owned(),
        std::fs::read_to_string(workspace_root().join("README.md")).expect("README.md"),
    ));
    for (name, text) in texts {
        assert!(
            !text.contains(CRATE_LABEL),
            "{name} must launch dx through {PUBLIC_LABEL}, never the crate label"
        );
        assert!(
            text.contains(PUBLIC_LABEL),
            "{name} must name the public dx launcher"
        );
    }
}

#[test]
fn example_readmes_name_the_tests_they_declare() {
    for (name, page) in example_readmes() {
        let sentence = evidence_paragraph(&page)
            .unwrap_or_else(|| panic!("{name}: the README must name the tests that pass"));
        let (claim, rest) = sentence.split_once(" tests pass (").unwrap_or_else(|| {
            panic!("{name}: the test evidence must read 'tests pass (`a_test`)': {sentence}")
        });
        let listed = rest
            .split_once(')')
            .unwrap_or_else(|| panic!("{name}: the test list never closes: {sentence}"))
            .0;
        let names: Vec<String> = listed
            .split(", ")
            .map(|test| test.trim_matches('`').to_owned())
            .collect();
        let claimed = match claim {
            "Both" => 2,
            other => other
                .strip_prefix("All ")
                .and_then(|count| count.parse::<usize>().ok())
                .unwrap_or_else(|| {
                    panic!("{name}: the test count must read 'Both' or 'All N': {sentence}")
                }),
        };
        assert_eq!(
            claimed,
            names.len(),
            "{name}: the sentence claims {claimed} tests and names {}: {sentence}",
            names.len()
        );
        for test in &names {
            assert!(
                test.ends_with("_test") || test.ends_with("_spec"),
                "{name}: {test} is not a test target name"
            );
        }
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            names.len(),
            "{name}: the README names a test twice: {sentence}"
        );
    }
}

#[test]
fn example_readmes_state_no_target_counts() {
    for (name, page) in example_readmes() {
        for sentence in page.lines().map(str::trim) {
            let words: Vec<&str> = sentence.split_whitespace().collect();
            let counted = words.windows(2).any(|pair| {
                pair[0].chars().all(|ch| ch.is_ascii_digit()) && pair[1].starts_with("target")
            });
            assert!(
                !counted,
                "{name}: drop the target count, it counts the repo lint corpus and wrapper targets: {sentence}"
            );
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
    for command in Command::value_variants() {
        let command = *command;
        let mut supported: Vec<String> = crate::plan::spec(command)
            .reports
            .iter()
            .map(|format| format.name().to_owned())
            .collect();
        supported.sort();
        let mut documented: Vec<String> = documented_usage_lines()
            .iter()
            .filter(|(_, owner, _)| *owner == command)
            .flat_map(|(_, _, line)| report_formats(line))
            .collect();
        documented.sort();
        documented.dedup();
        assert_eq!(
            documented,
            supported,
            "the docs usage line for dx {} must name every --report format the registry allows, \
             and nothing else",
            command.name()
        );
    }
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

fn commands_where(predicate: impl Fn(Command) -> bool) -> Vec<String> {
    let mut names: Vec<String> = Command::value_variants()
        .iter()
        .copied()
        .filter(|command| predicate(*command))
        .map(|command| command.name().to_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn global_flags_page_names_flag_shapes_not_commands() {
    let page =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "Global Flags")
        .map(|(_, body)| body)
        .expect("README has a `## Global Flags` section");
    for token in backticked(&body) {
        let name = token.strip_prefix("dx ").unwrap_or(&token);
        assert!(
            Command::parse(name).is_none(),
            "docs/cli/commands/README.md Global Flags names dx {name}; command pages own flag acceptance"
        );
    }
    for flag in ["--output", "--fail-on", "--report", "--offline", "--frozen"] {
        assert!(
            body.contains(&format!("`{flag}")),
            "docs/cli/commands/README.md Global Flags never names {flag}"
        );
    }
}

#[test]
fn json_output_section_names_every_event_kind() {
    let page =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "JSON Output")
        .map(|(_, body)| body)
        .expect("README has a `## JSON Output` section");
    let mut named = bullet_terms(&body);
    named.sort();
    let mut expected: Vec<String> = dx_output::EVENTS
        .iter()
        .map(|event| (*event).to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        named, expected,
        "docs/cli/commands/README.md JSON Output must give every event one bullet, and no others"
    );
    for field in ["event", "schema"] {
        assert!(
            body.contains(&format!("`{field}`")),
            "docs/cli/commands/README.md JSON Output never names the {field} field every object carries"
        );
    }
    let intro = body
        .split("\n- ")
        .next()
        .expect("JSON Output has prose before its bullets")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let envelope =
        "The stream starts with one `command_started` and ends with one `command_finished`";
    assert!(
        intro.contains(envelope),
        "docs/cli/commands/README.md JSON Output must say \"{envelope}\": {intro}"
    );
}

#[test]
fn global_flags_page_documents_every_banner_option() {
    let page =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "Global Flags")
        .map(|(_, body)| body)
        .expect("README has a `## Global Flags` section");
    let advertised = banner()
        .lines()
        .next()
        .expect("the usage banner opens with its usage line")
        .to_owned();
    for flag in flag_tokens(&advertised) {
        assert!(
            body.contains(&format!("`{flag}")),
            "docs/cli/commands/README.md Global Flags never documents {flag}, which the usage banner advertises: {advertised}"
        );
    }
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
                let stated = if command.supports_json() {
                    format!("`--output {modes}`")
                } else {
                    "text only".to_owned()
                };
                assert!(
                    body.contains(&stated),
                    "{name}: {where_} documents dx {} usage but never states {stated}",
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
            let where_ = format!("{name} section {heading:?}");
            let want: &[i32] = if name == "watch" {
                &[EXIT_PRE_EXEC, EXIT_OPERATIONAL]
            } else {
                &EXIT_TRIPLE
            };
            assert_exit_codes(&where_, &body, want);
        }
    }
}

#[test]
fn the_help_exit_code_clause_matches_the_documented_codes() {
    for command in Command::value_variants() {
        let want: &[i32] = if *command == Command::Watch {
            &[EXIT_PRE_EXEC, EXIT_OPERATIONAL]
        } else {
            &EXIT_TRIPLE
        };
        assert_exit_codes(
            &format!("dx {} --help", command.name()),
            super::exit_codes(*command),
            want,
        );
    }
}

#[test]
fn quality_help_says_bazel_failures_exit_one_like_the_docs() {
    let page = std::fs::read_to_string(docs_dir().join("quality.md")).expect("quality.md");
    let documented = sections(&page)
        .into_iter()
        .map(|(_, body)| body.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|body| body.contains("Exit codes:"))
        .expect("quality.md has an Exit codes: paragraph");
    assert!(
        documented.contains("Bazel failures report `1`"),
        "quality.md must say Bazel failures report `1`"
    );
    for command in [Command::Lint, Command::Typecheck, Command::Format] {
        let help = render_command_help(command);
        assert!(
            !help.contains("preserve Bazel's code"),
            "dx {} --help must not claim Bazel's code is kept",
            command.name()
        );
        assert!(
            help.contains("1 findings, operational failures, and Bazel failures"),
            "dx {} --help must say Bazel failures exit 1",
            command.name()
        );
    }
    for command in [Command::Build, Command::Check] {
        assert!(
            render_command_help(command).contains("preserve Bazel's code"),
            "dx {} --help must keep the Bazel-code clause",
            command.name()
        );
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
fn docs_and_help_never_name_removed_commands() {
    let mut texts: Vec<(String, String)> = pages();
    texts.push(("top help".into(), render_top_help()));
    texts.push(("usage banner".into(), usage_banner()));
    for command in Command::value_variants() {
        texts.push((
            format!("help {}", command.name()),
            render_command_help(*command),
        ));
    }
    for (name, text) in texts {
        let words: Vec<&str> = text
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .filter(|word| !word.is_empty())
            .collect();
        for pair in words.windows(2) {
            if pair[0] == "dx" {
                assert!(
                    !REMOVED_COMMANDS
                        .iter()
                        .any(|(removed, _)| *removed == pair[1]),
                    "{name} names the removed command dx {}",
                    pair[1]
                );
            }
        }
    }
}

#[test]
fn worktree_page_shows_every_allow_rule() {
    let page = std::fs::read_to_string(docs_dir().join("worktree.md")).expect("worktree.md");
    for rule in dx_worktree::setup::allow_rules() {
        assert!(page.contains(&rule), "worktree.md must show {rule}");
    }
}

#[test]
fn allowlisted_commands_are_read_only() {
    for rule in dx_worktree::setup::allow_rules() {
        if rule.starts_with("Bash(export DX_WORKTREE_") {
            continue;
        }
        let name = rule
            .strip_prefix("Bash(tools/dx ")
            .and_then(|r| r.strip_suffix(" *)"))
            .expect("rule shape");
        let command = Command::parse(name).expect("known command");
        assert!(!command.supports_apply(), "{name} takes --apply");
        assert!(parse(&[name]).is_ok(), "{name} must parse");
    }
}

#[test]
fn clean_section_describes_every_clean_flag() {
    let page =
        std::fs::read_to_string(docs_dir().join("check-fix-clean.md")).expect("check-fix-clean.md");
    let (_, body) = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "`dx clean`")
        .expect("check-fix-clean.md has a `dx clean` section");
    let mut prose = String::new();
    let mut fenced = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced {
            prose.push_str(line);
            prose.push('\n');
        }
    }
    let tokens = backticked(&prose);
    for flag in flag_tokens(Command::Clean.usage()) {
        assert!(
            tokens.iter().any(|token| token == &flag),
            "check-fix-clean.md `dx clean` prose never names `{flag}` in backticks"
        );
    }
    assert!(
        shell_blocks(&body)
            .iter()
            .flatten()
            .any(|line| line.contains("--prune-unobserved")),
        "check-fix-clean.md `dx clean` section needs a shell example using `--prune-unobserved`"
    );
}

fn incomplete_result_clause(page: &str, name: &str) -> String {
    let opening = "Results are incomplete when ";
    let flat = page.split_whitespace().collect::<Vec<_>>().join(" ");
    let start = flat
        .find(opening)
        .unwrap_or_else(|| panic!("{name} has no sentence starting {opening:?}"));
    let rest = &flat[start + opening.len()..];
    let end = rest.find(". ").unwrap_or(rest.len());
    rest[..end].to_owned()
}

#[test]
fn check_page_lists_every_incomplete_result_cause() {
    let coverage = std::fs::read_to_string(docs_dir().join("build-test-coverage.md"))
        .expect("build-test-coverage.md");
    let check =
        std::fs::read_to_string(docs_dir().join("check-fix-clean.md")).expect("check-fix-clean.md");
    let expected = incomplete_result_clause(&coverage, "build-test-coverage.md");
    let clause = incomplete_result_clause(&check, "check-fix-clean.md");
    let actual = clause
        .strip_prefix("the build failed, ")
        .unwrap_or_else(|| {
            panic!("check-fix-clean.md must list `the build failed` first, got {clause:?}")
        });
    assert_eq!(
        actual, expected,
        "check-fix-clean.md and build-test-coverage.md list different incomplete-result causes"
    );
}

#[test]
fn coverage_page_names_every_forced_flag() {
    use crate::plan::workflow::COVERAGE_COMBINED_REPORT_FLAG;
    use crate::plan::{
        BEP_FLAG_NAME, CLIPPY_DIAGNOSTICS_FLAG, DOWNLOAD_TOPLEVEL_FLAG, RUSTC_DIAGNOSTICS_FLAG,
        SKIP_INCOMPATIBLE_FLAG,
    };
    let page = std::fs::read_to_string(docs_dir().join("build-test-coverage.md"))
        .expect("build-test-coverage.md");
    let named = backticked(&page);
    let bep_flag = format!("--{BEP_FLAG_NAME}");
    let forced = [
        SKIP_INCOMPATIBLE_FLAG,
        CLIPPY_DIAGNOSTICS_FLAG,
        RUSTC_DIAGNOSTICS_FLAG,
        DOWNLOAD_TOPLEVEL_FLAG,
        COVERAGE_COMBINED_REPORT_FLAG,
        bep_flag.as_str(),
    ];
    for flag in forced {
        assert!(
            named.iter().any(|token| token == flag),
            "build-test-coverage.md never names forced flag `{flag}` in backticks"
        );
    }
}

fn command_index(readme: &str) -> Vec<(Vec<String>, String)> {
    let mut rows: Vec<(Vec<String>, String)> = Vec::new();
    for line in readme.lines() {
        let Some(rest) = line.strip_prefix("- [`dx ") else {
            continue;
        };
        let (_, target) = rest
            .rsplit_once("](")
            .unwrap_or_else(|| panic!("command index bullet has no page link: {line}"));
        let target = target
            .strip_suffix(')')
            .unwrap_or_else(|| panic!("command index bullet has an unclosed link: {line}"));
        let mut commands: Vec<String> = backticked(line)
            .into_iter()
            .filter_map(|token| token.strip_prefix("dx ").map(ToOwned::to_owned))
            .collect();
        assert!(
            !commands.is_empty(),
            "the command index links {target} but names no command: {line}"
        );
        commands.sort();
        rows.push((commands, target.to_owned()));
    }
    rows
}

fn link_targets(page: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = page;
    while let Some(start) = rest.find("](") {
        let tail = &rest[start + 2..];
        let Some(end) = tail.find(')') else {
            break;
        };
        let file = tail[..end].split('#').next().unwrap_or_default();
        if file.ends_with(".md") {
            out.push(file.to_owned());
        }
        rest = &tail[end + 1..];
    }
    out
}

#[test]
fn command_index_names_every_command_exactly_once() {
    let readme =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    let index = command_index(&readme);
    assert!(
        !index.is_empty(),
        "docs/cli/commands/README.md has no command index bullets"
    );
    let named: Vec<String> = index
        .iter()
        .flat_map(|(commands, _)| commands.clone())
        .collect();
    assert_eq!(
        sorted(named),
        commands_where(|_| true),
        "the docs/cli/commands/README.md index must name every dx command exactly once"
    );
    for (commands, target) in &index {
        assert!(
            docs_dir().join(target).is_file(),
            "the command index bullet for dx {} links {target}, which is not a page in \
             docs/cli/commands",
            commands.join(", ")
        );
    }
}

#[test]
fn command_reference_links_every_page_and_only_real_pages() {
    let readme =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    let mut linked = link_targets(&readme);
    linked.sort();
    linked.dedup();
    for target in &linked {
        assert!(
            docs_dir().join(target).is_file(),
            "docs/cli/commands/README.md links {target}, which is not a page in docs/cli/commands"
        );
    }
    assert_eq!(
        sorted(linked),
        sorted(
            pages()
                .iter()
                .map(|(name, _)| format!("{name}.md"))
                .filter(|name| name != "README.md")
                .collect()
        ),
        "docs/cli/commands/README.md must link every page it ships, and nothing else"
    );
}

#[test]
fn help_doc_anchors_resolve() {
    for command in Command::value_variants() {
        let texts = [command.describe(), command.usage(), command.scopes_text()];
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

#[test]
fn github_ci_page_pairs_check_only_with_commands_that_accept_it() {
    let page = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");
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
fn github_ci_page_names_no_workflow_internals() {
    let page = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");
    let label_char = |c: char| c.is_ascii_alphanumeric() || "_.+@=,~/:-".contains(c);
    let label_at = |text: &str, at: usize| {
        let end = text[at..]
            .find(|c: char| !label_char(c))
            .map_or(text.len(), |offset| at + offset);
        text[at..end].to_owned()
    };
    let refresh = dx_audit::advisory::REFRESH_COMMAND;
    let refresh_label = refresh
        .find("@rules_dx//")
        .map(|at| label_at(refresh, at))
        .unwrap_or_else(|| panic!("{refresh} names no @rules_dx label"));
    let allowed = ["@rules_dx//:dx".to_owned(), refresh_label];
    for (at, _) in page.match_indices("@rules_dx//") {
        let label = label_at(&page, at);
        assert!(
            allowed.contains(&label),
            "docs/github-ci.md names the workflow internal {label}; only {allowed:?} are user facing"
        );
    }
    for internal in [
        "tools/ci",
        "python3",
        "diff-cover",
        "tool_resolve",
        "evidence.yml",
        "android.yml",
        "apple.yml",
        "bump.yml",
    ] {
        assert!(
            !page.contains(internal),
            "docs/github-ci.md names the workflow internal {internal}"
        );
    }
}

const CI_EXAMPLES: &[(&str, &str, &[&str])] = &[(
    "examples/gitlab-ci/.gitlab-ci.yml",
    "examples/gitlab-ci/README.md",
    &["build", "check", "coverage", "test"],
)];

fn ci_example_dx_lines(path: &str) -> Vec<String> {
    let text = std::fs::read_to_string(workspace_root().join(path))
        .unwrap_or_else(|error| panic!("{path} must ship as test data: {error}"));
    text.lines()
        .filter(|line| line.contains(DX_PREFIX))
        .map(|line| {
            line.trim()
                .trim_start_matches("- ")
                .trim_end_matches("|| status=$?")
                .trim()
                .to_owned()
        })
        .collect()
}

fn ci_example_invocations(path: &str) -> Vec<Invocation> {
    let lines = ci_example_dx_lines(path);
    assert!(!lines.is_empty(), "{path} runs no dx line");
    lines
        .iter()
        .map(|line| {
            let words: Vec<&str> = line
                .strip_prefix(DX_PREFIX)
                .unwrap_or_else(|| panic!("{path}: dx line must start with {DX_PREFIX}: {line}"))
                .split_whitespace()
                .collect();
            parse(&words)
                .unwrap_or_else(|error| panic!("{path}: dx line does not parse: {line}\n{error:?}"))
        })
        .collect()
}

#[test]
fn gitlab_example_dx_commands_parse() {
    for (pipeline, readme, commands) in CI_EXAMPLES {
        let invocations = ci_example_invocations(pipeline);
        let mut got: Vec<&str> = invocations
            .iter()
            .map(|invocation| invocation.command.name())
            .collect();
        got.sort_unstable();
        assert_eq!(&got, commands, "{pipeline} must run exactly {commands:?}");
        for invocation in invocations.iter().chain(&ci_example_invocations(readme)) {
            let name = invocation.command.name();
            assert!(!invocation.apply, "{pipeline}: dx {name} must not --apply");
            assert_ne!(name, "fix", "{pipeline} must not run dx fix");
            assert!(
                !invocation.reports.is_empty(),
                "{pipeline}: dx {name} writes no report"
            );
            for report in &invocation.reports {
                assert!(
                    report.destination.starts_with("reports/"),
                    "{pipeline}: dx {name} report {} must land under reports/",
                    report.destination
                );
            }
        }
        assert_eq!(
            ci_example_dx_lines(pipeline),
            ci_example_dx_lines(readme),
            "{readme} must list the dx lines of {pipeline} in order"
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
            "NO_COLOR".to_owned(),
            "BUILD_WORKSPACE_DIRECTORY".to_owned(),
            "BUILD_WORKING_DIRECTORY".to_owned(),
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
            "{env} has no matching dx.toml key"
        );
    }
}

#[test]
fn config_file_keys_are_the_hyphen_spellings_the_parser_reads() {
    let text = "[dx]\nworkspace = \"/w\"\ncolor = \"never\"\nfail-on = \"error\"\n";
    let parsed = dx_adopt::defaults::parse_file_text(text).expect("keys parse");
    assert_eq!(parsed.workspace, Some("/w".to_owned()));
    assert_eq!(parsed.color, Some("never".to_owned()));
    assert_eq!(parsed.fail_on, Some("error".to_owned()));
    for (env, _, _) in dx_adopt::defaults::ENV_DEFAULTS {
        let key = dx_adopt::defaults::config_key(env).expect("every default has a key");
        let body = format!("[dx]\n{key} = {}\n", literal_for(&key));
        dx_adopt::defaults::parse_file_text(&body)
            .unwrap_or_else(|error| panic!("docs promise {key} works in a config file: {error}"));
    }
}

#[test]
fn config_file_section_names_the_keys_and_files_the_parser_reads() {
    let page =
        std::fs::read_to_string(docs_dir().join("README.md")).expect("README ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "Config File")
        .map(|(_, body)| body)
        .expect("docs/cli/commands/README.md has a Config File section");
    for name in [
        dx_adopt::defaults::DX_TOML_REL,
        dx_adopt::defaults::DX_LOCAL_TOML_REL,
    ] {
        assert!(
            body.contains(&format!("`{name}`")),
            "the Config File section never names `{name}`, which the loader reads"
        );
    }
    let mut shown: Vec<(String, String)> = fenced_blocks(&page, "```toml")
        .into_iter()
        .flatten()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect();
    shown.sort();
    for (key, literal) in &shown {
        let text = format!("[dx]\n{key} = {literal}\n");
        let parsed = dx_adopt::defaults::parse_file_text(&text).unwrap_or_else(|error| {
            panic!("the Config File example sets {key}, which the parser rejects: {error}")
        });
        assert_ne!(
            parsed,
            dx_adopt::defaults::FileDefaults::default(),
            "the Config File example sets {key}, which the parser ignores"
        );
    }
    for (env, _, _) in dx_adopt::defaults::ENV_DEFAULTS {
        let key = dx_adopt::defaults::config_key(env).expect("every default has a key");
        let literal = literal_for(&key);
        assert!(
            shown.iter().any(|(shown_key, _)| shown_key == key),
            "the Config File example never sets {key}, which the parser reads"
        );
        let hyphenated = format!("[dx]\n{key} = {literal}\n");
        dx_adopt::defaults::parse_file_text(&hyphenated).unwrap_or_else(|error| {
            panic!("the Config File section spells {key} a way the parser rejects: {error}")
        });
    }
    for claim in [
        "live only under `[dx]`",
        "boolean key takes",
        "`true` or `false`",
        "No other file is read for defaults",
        "nearest file to the working directory wins",
    ] {
        assert!(
            body.contains(claim),
            "the Config File section never states: {claim}"
        );
    }
}

fn literal_for(key: &str) -> &'static str {
    match key {
        "quiet" | "verbose" => "true",
        "fail-on" => "\"error\"",
        "color" => "\"never\"",
        "output" => "\"json\"",
        "build-slots" => "\"auto\"",
        _ => "\"/w\"",
    }
}

#[test]
fn tool_path_env_vars_are_documented_where_the_command_needs_them() {
    let cases: Vec<(&str, Option<&str>, &str, &str)> = vec![(
        dx_audit::secrets::TOOL_ENV_VAR,
        Some(dx_audit::secrets::TOOL_LABEL),
        "audit-update.md",
        "dx security",
    )];
    for (var, label, page_name, command) in cases {
        let page = std::fs::read_to_string(docs_dir().join(page_name))
            .unwrap_or_else(|error| panic!("{page_name} ships as test data: {error}"));
        let paragraph = page
            .split("\n\n")
            .find(|block| block.contains(var))
            .unwrap_or_else(|| {
                panic!("{page_name} never names {var}, which {command} needs to find its tool")
            });
        assert!(
            paragraph.contains("absolute"),
            "{page_name} must say {var} takes an absolute path: {paragraph}"
        );
        if let Some(label) = label {
            assert!(
                paragraph.contains(label),
                "{page_name} must name the pinned label {label} next to {var}: {paragraph}"
            );
        }
    }
}

#[test]
fn secrets_config_docs_say_which_file_the_scan_reads() {
    let page_name = "audit-update.md";
    let page = std::fs::read_to_string(docs_dir().join(page_name))
        .unwrap_or_else(|error| panic!("{page_name} ships as test data: {error}"));
    let paragraph = page
        .split("\n\n")
        .find(|block| block.contains(dx_audit::secrets::CONFIG_FILE_NAME))
        .unwrap_or_else(|| {
            panic!(
                "{page_name} never names {}, the only config dx security hands the secrets scan",
                dx_audit::secrets::CONFIG_FILE_NAME
            )
        });
    assert!(
        paragraph.contains("warning"),
        "{page_name} must say a committed config makes the scan warn: {paragraph}"
    );
}

fn skew_bullet_owners(page: &str, lead: &str) -> Vec<String> {
    let section = sections(page)
        .into_iter()
        .find(|(name, _)| name == "Version Skew")
        .map(|(_, body)| body)
        .unwrap_or_else(|| panic!("status-version.md has no `Version Skew` section"));
    let line = section
        .lines()
        .find(|line| line.starts_with(&format!("- {lead}")))
        .unwrap_or_else(|| panic!("the Version Skew section has no `{lead}` bullet"));
    let (_, list) = line
        .split_once(':')
        .expect("the bullet has a lead and a list");
    sorted(backticked(list))
}

#[test]
fn version_skew_page_names_exactly_the_registry_groups() {
    let page =
        std::fs::read_to_string(docs_dir().join("status-version.md")).expect("page ships as data");
    let proceed = skew_bullet_owners(&page, "Runs anyway");
    let warn = skew_bullet_owners(&page, "Warns and runs");
    for (lead, kind, found) in [
        ("Runs anyway", SkewKind::Proceed, &proceed),
        ("Warns and runs", SkewKind::Warn, &warn),
    ] {
        let expected = commands_where(|command| command.meta().skew == kind);
        assert_eq!(
            *found, expected,
            "the `{lead}` bullet must name exactly the commands the registry marks {kind:?}"
        );
    }
    let stray: Vec<String> = skew_bullet_owners(&page, "Stops with exit code")
        .into_iter()
        .filter(|token| Command::parse(token).is_some())
        .collect();
    assert!(
        stray.is_empty(),
        "the stop bullet covers the rest of the registry, so it must name no command: {stray:?}"
    );
    let mut rest: Vec<String> = Command::value_variants()
        .iter()
        .map(|command| command.name().to_owned())
        .filter(|name| !proceed.contains(name) && !warn.contains(name))
        .collect();
    rest.sort();
    assert_eq!(
        commands_where(|command| command.meta().skew == SkewKind::Refuse),
        rest,
        "every command the two bullets do not name must be a SkewKind::Refuse one"
    );
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

fn offline_bzl() -> String {
    std::fs::read_to_string(workspace_root().join("deploy/offline/offline.bzl"))
        .expect("offline.bzl ships as test data")
}

fn offline_accepted_sets() -> Vec<String> {
    let source = offline_bzl();
    let marker = "if set in [";
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("no advisory set list in:\n{source}"));
    let rest = &source[start + marker.len()..];
    let end = rest.find(']').expect("the advisory set list is closed");
    rest[..end]
        .split(',')
        .map(|name| name.trim().trim_matches('"').to_owned())
        .collect()
}

fn offline_wanted_sets() -> Vec<String> {
    let source = offline_bzl();
    let marker = "want one of ";
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("no wanted-set list in:\n{source}"));
    let rest = &source[start + marker.len()..];
    let end = rest.find('"').expect("the wanted-set list is closed");
    rest[..end]
        .split(',')
        .map(|name| name.trim().to_owned())
        .collect()
}

fn bootstrap_accepted_sets() -> Vec<String> {
    let source = std::fs::read_to_string(workspace_root().join(BOOTSTRAP_SOURCE))
        .expect("the offline bootstrap source ships as test data");
    source
        .lines()
        .find(|line| line.contains("ADVISORY_SETS: [&str;"))
        .and_then(|line| line.rsplit_once('[').map(|(_, rest)| rest))
        .and_then(|rest| rest.split_once(']').map(|(head, _)| head))
        .map(|names| {
            names
                .split(',')
                .filter_map(|name| name.split('"').nth(1))
                .map(|name| name.to_owned())
                .collect()
        })
        .expect("the offline bootstrap accepts a fixed advisory set list")
}

#[test]
fn offline_bundle_accepts_every_curated_advisory_set() {
    let want = sorted(
        dx_audit::curator::CURATOR_ADVISORY_SETS
            .iter()
            .map(|set| (*set).to_owned())
            .collect(),
    );
    assert_eq!(
        sorted(offline_accepted_sets()),
        want.clone(),
        "deploy/offline/offline.bzl must vendor every curated advisory set, and nothing else"
    );
    assert_eq!(
        sorted(offline_wanted_sets()),
        want.clone(),
        "the invalid-advisory-set error must name every curated advisory set"
    );
    assert_eq!(
        sorted(bootstrap_accepted_sets()),
        want,
        "the offline bootstrap must install every curated advisory set, and nothing else"
    );
}

const PREP_TARGET: &str = "@rules_dx//cli/advisory_prep";

/// Whether the text continues with a bare ` --` separator, not a longer flag.
fn separator_follows(rest: &str) -> bool {
    rest.strip_prefix(" --")
        .is_some_and(|after| after.chars().next().is_none_or(char::is_whitespace))
}

#[test]
fn the_separator_check_rejects_longer_flags() {
    assert!(separator_follows(" -- --out x"));
    assert!(separator_follows(" --"));
    assert!(!separator_follows(" --workspace . --out x"));
    assert!(!separator_follows(" x"));
}

#[test]
fn docs_name_the_advisory_refresh_command_the_errors_print() {
    let audit = std::fs::read_to_string(docs_dir().join("audit-update.md"))
        .expect("audit page ships as test data");
    let security = sections(&audit)
        .into_iter()
        .find(|(heading, _)| heading == "`dx security` And `dx license`")
        .map(|(_, body)| body)
        .expect("`dx security` section");
    let ci = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");
    let command = dx_audit::advisory::REFRESH_COMMAND;
    for (name, text) in [("audit-update.md", &security), ("github-ci.md", &ci)] {
        assert!(text.contains(command), "{name} never names {command}");
        let runs = format!("bazel run {PREP_TARGET}");
        for (at, _) in text.match_indices(&runs) {
            assert!(
                separator_follows(&text[at + runs.len()..]),
                "{name} runs {PREP_TARGET} without the -- separator"
            );
        }
    }
}

#[test]
fn docs_name_the_uv_resolve_commands() {
    use dx_update::backend::{
        UV_BIN_VAR, UV_LABEL, UV_PYTHON_LABEL, UV_PYTHON_RESOLVE_COMMAND, UV_PYTHON_VAR,
        UV_RESOLVE_COMMAND,
    };
    assert!(
        UV_RESOLVE_COMMAND.contains(UV_LABEL),
        "{UV_RESOLVE_COMMAND}"
    );
    assert!(
        UV_RESOLVE_COMMAND.contains(UV_BIN_VAR),
        "{UV_RESOLVE_COMMAND}"
    );
    assert!(UV_PYTHON_RESOLVE_COMMAND.contains(UV_PYTHON_LABEL));
    assert!(UV_PYTHON_RESOLVE_COMMAND.contains(UV_PYTHON_VAR));
    let audit = std::fs::read_to_string(docs_dir().join("audit-update.md"))
        .expect("audit page ships as test data");
    for command in [UV_RESOLVE_COMMAND, UV_PYTHON_RESOLVE_COMMAND] {
        assert!(
            audit.contains(command),
            "audit-update.md never names {command}"
        );
    }
}

#[test]
fn docs_name_the_gitleaks_resolve_command_the_error_prints() {
    use dx_audit::secrets::{RESOLVE_COMMAND, TOOL_ENV_VAR, TOOL_LABEL};
    assert!(RESOLVE_COMMAND.contains(TOOL_LABEL), "{RESOLVE_COMMAND}");
    assert!(RESOLVE_COMMAND.contains(TOOL_ENV_VAR), "{RESOLVE_COMMAND}");
    let audit = std::fs::read_to_string(docs_dir().join("audit-update.md"))
        .expect("audit page ships as test data");
    let security = sections(&audit)
        .into_iter()
        .find(|(heading, _)| heading == "`dx security` And `dx license`")
        .map(|(_, body)| body)
        .expect("`dx security` section");
    assert!(
        security.contains(RESOLVE_COMMAND),
        "audit-update.md never names {RESOLVE_COMMAND}"
    );
    let ci = std::fs::read_to_string(workspace_root().join("docs/github-ci.md"))
        .expect("docs/github-ci.md ships as test data");
    let runs = "bazel run @rules_dx//cli/tool_resolve";
    for (name, text) in [("audit-update.md", &audit), ("github-ci.md", &ci)] {
        for (at, _) in text.match_indices(runs) {
            assert!(
                separator_follows(&text[at + runs.len()..]),
                "{name} runs tool_resolve without the -- separator"
            );
        }
    }
}

#[test]
fn security_docs_page_names_exactly_the_audited_sets() {
    let page = std::fs::read_to_string(docs_dir().join("audit-update.md"))
        .expect("audit page ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "`dx security` And `dx license`")
        .map(|(_, body)| body)
        .expect("`dx security` section");
    assert_eq!(
        set_names_in_list(&body),
        sorted(
            dx_update::sets::SetId::ALL
                .iter()
                .map(|set| (*set).name())
                .filter(|name| !dx_audit::backend::is_empty_set(name))
                .map(ToOwned::to_owned)
                .collect()
        ),
        "docs/cli/commands/audit-update.md must name every set dx security audits, and nothing else"
    );
}

fn security_scope_table() -> Vec<(String, String)> {
    let page = std::fs::read_to_string(docs_dir().join("audit-update.md"))
        .expect("audit page ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "`dx security` And `dx license`")
        .map(|(_, body)| body)
        .expect("`dx security` section");
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut block: Vec<&str> = Vec::new();
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut fenced = false;
    for line in body.lines() {
        if line.starts_with("```") {
            if fenced && !block.is_empty() {
                blocks.push(std::mem::take(&mut block));
            } else {
                block.clear();
            }
            fenced = !fenced;
            continue;
        }
        if fenced {
            block.push(line);
        }
    }
    for lines in &blocks {
        let parsed: Vec<Option<(String, String)>> = lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let (paths, set) = line.split_once("  ")?;
                let set = set.trim().to_owned();
                dx_update::sets::SetId::parse(&set)?;
                Some((paths.trim().to_owned(), set))
            })
            .collect();
        if parsed.iter().any(Option::is_none) {
            continue;
        }
        for row in parsed.into_iter().flatten() {
            for path in row.0.split(',') {
                rows.push((path.trim().to_owned(), row.1.clone()));
            }
        }
        break;
    }
    assert!(!rows.is_empty(), "the scope table ships");
    rows
}

fn documented_path(path: &str) -> String {
    let trimmed = path.trim_start_matches('/');
    trimmed.strip_suffix("/...").unwrap_or(trimmed).to_owned()
}

fn encloses(prefix: &str, path: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

fn set_names(sets: &[dx_update::sets::SetId]) -> Vec<String> {
    let mut names: Vec<String> = sets.iter().map(|set| (*set).name().to_owned()).collect();
    names.sort();
    names.dedup();
    names
}

#[test]
fn security_docs_scope_table_matches_owning_sets() {
    let rows = security_scope_table();
    for (path, set) in &rows {
        let expected = dx_update::sets::SetId::parse(set).expect("known set");
        assert!(
            dx_update::selector::owning_sets(path).contains(&expected),
            "the scope table says {path} is audited by {set}"
        );
    }
    let mut documented: Vec<String> = rows.iter().map(|(_, set)| set.clone()).collect();
    documented.sort();
    documented.dedup();
    assert_eq!(
        documented,
        sorted(
            dx_update::sets::SetId::names()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect()
        ),
        "the scope table must name every set a scope can own, and nothing else"
    );
    let documented_paths: Vec<String> = rows
        .iter()
        .map(|(path, _)| path.trim_start_matches('/').to_owned())
        .collect();
    for (prefix, _) in dx_update::selector::owning_prefixes() {
        assert!(
            documented_paths
                .iter()
                .any(|path| path == prefix || path == &format!("{prefix}/...")),
            "OWNING_PREFIXES maps {prefix:?}, which the scope table does not name"
        );
    }
}

#[test]
fn security_scope_never_owns_a_set_the_scope_table_does_not_name() {
    let rows = security_scope_table();
    let table: Vec<(String, dx_update::sets::SetId)> = rows
        .iter()
        .map(|(path, set)| {
            (
                documented_path(path),
                dx_update::sets::SetId::parse(set).expect("known set"),
            )
        })
        .collect();
    let mut scopes: Vec<String> = table.iter().map(|(path, _)| path.clone()).collect();
    for (path, _) in &table {
        let mut parent = path.as_str();
        while let Some((head, _)) = parent.rsplit_once('/') {
            parent = head;
            scopes.push(parent.to_owned());
        }
    }
    scopes.sort();
    scopes.dedup();
    for scope in &scopes {
        let under = set_names(
            &table
                .iter()
                .filter(|(path, _)| encloses(scope, path))
                .map(|(_, set)| *set)
                .collect::<Vec<_>>(),
        );
        let near = set_names(
            &table
                .iter()
                .filter(|(path, _)| encloses(scope, path) || encloses(path, scope))
                .map(|(_, set)| *set)
                .collect::<Vec<_>>(),
        );
        let got = set_names(&dx_update::selector::owning_sets(&format!("//{scope}/...")));
        for set in &got {
            assert!(
                near.contains(set),
                "//{scope}/... selects {set}, which the scope table places at {scope}, under it, or above it"
            );
        }
        for set in &under {
            assert!(
                got.contains(set),
                "//{scope}/... must select {set}, which the scope table places under {scope}"
            );
        }
        let plain = set_names(&dx_update::selector::owning_sets(&format!("//{scope}")));
        for set in &plain {
            assert!(
                near.contains(set),
                "//{scope} selects {set}, which no scope table path places at {scope}, under it, or above it"
            );
        }
    }
}

#[test]
fn update_docs_page_names_exactly_the_update_sets() {
    let page = std::fs::read_to_string(docs_dir().join("audit-update.md"))
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
        "docs/cli/commands/audit-update.md must name every dx update set, and nothing else"
    );
}

#[test]
fn bump_docs_page_names_exactly_the_bump_sets() {
    let page = std::fs::read_to_string(docs_dir().join("audit-update.md"))
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
        "docs/cli/commands/audit-update.md must name every dx bump set, and nothing else"
    );
}

#[test]
fn add_docs_and_help_name_exactly_the_add_sets() {
    let page = std::fs::read_to_string(docs_dir().join("audit-update.md"))
        .expect("audit page ships as test data");
    let body = sections(&page)
        .into_iter()
        .find(|(heading, _)| heading == "`dx add`")
        .map(|(_, body)| body)
        .expect("`dx add` section");
    let sets = sorted(
        dx_bump::AddRequest::SETS
            .iter()
            .map(|set| (*set).name().to_owned())
            .collect(),
    );
    assert_eq!(
        set_names_in_list(&body),
        sets,
        "docs/cli/commands/audit-update.md must name every dx add set, and nothing else"
    );
    assert_eq!(
        sorted(scoped_set_list(Command::Add.scopes_text())),
        sets,
        "dx add --help must name every add set, and nothing else"
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
fn unknown_selector_errors_name_every_set() {
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

const SCOPE_PAGE_POLICIES: [(&str, &[&str]); 6] = [
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

fn scope_page_affected_sentence(body: &str, needle: &str) -> Vec<String> {
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let sentence = flat
        .split(". ")
        .find(|sentence| sentence.contains(needle))
        .unwrap_or_else(|| panic!("the `--affected` section has no sentence with {needle:?}"));
    let mut found = named_commands(sentence);
    found.sort();
    found
}

#[test]
fn scope_page_affected_lists_match_the_registry() {
    let page = scope_page();
    let body = scope_page_section(&page, "`--affected`");
    let names = |keep: fn(Command) -> bool| {
        let mut names: Vec<String> = Command::value_variants()
            .iter()
            .copied()
            .filter(|command| keep(*command))
            .map(|command| command.name().to_owned())
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        scope_page_affected_sentence(&body, "accepted by"),
        names(|command| {
            command != Command::Watch && super::super::grammar::accepts_long(command, "affected")
        }),
        "scope-defaults.md must name exactly the commands that accept `--affected`"
    );
    assert_eq!(
        scope_page_affected_sentence(&body, "reject `--affected`"),
        names(|command| {
            command.supports_here()
                && command != Command::Watch
                && !super::super::grammar::accepts_long(command, "affected")
        }),
        "scope-defaults.md must name exactly the `--here` commands that reject `--affected`"
    );
}

fn pipe_runs(text: &str) -> Vec<Vec<String>> {
    let mut runs: Vec<Vec<String>> = Vec::new();
    let mut run: Vec<String> = Vec::new();
    let mut word = String::new();
    for letter in text.chars() {
        if letter.is_ascii_alphanumeric() || matches!(letter, '#' | '+' | '-' | '.') {
            word.push(letter);
            continue;
        }
        if !word.is_empty() {
            run.push(std::mem::take(&mut word));
        }
        if letter != '|' {
            if run.len() > 1 && !run.iter().any(|part| part.starts_with('-')) {
                runs.push(std::mem::take(&mut run));
            }
            run.clear();
        }
    }
    if !word.is_empty() {
        run.push(word);
    }
    if run.len() > 1 && !run.iter().any(|part| part.starts_with('-')) {
        runs.push(run);
    }
    runs
}

fn backticked_runs(text: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = text.chars().collect();
    let mut runs: Vec<Vec<String>> = Vec::new();
    let mut run: Vec<String> = Vec::new();
    let mut gap = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '`' {
            gap.push(chars[index]);
            index += 1;
            continue;
        }
        let start = index + 1;
        let mut end = start;
        while end < chars.len() && chars[end] != '`' {
            end += 1;
        }
        if !run.is_empty() {
            let separator = gap
                .chars()
                .all(|letter| letter.is_whitespace() || ",orand".contains(letter));
            if !separator {
                if run.len() > 1 {
                    runs.push(std::mem::take(&mut run));
                }
                run.clear();
            }
        }
        run.push(chars[start..end].iter().collect());
        gap.clear();
        index = end + 1;
    }
    if run.len() > 1 {
        runs.push(run);
    }
    runs
}

fn slot_lists(text: &str, registry: &[&str]) -> Vec<Vec<String>> {
    let wanted: Vec<String> = registry.iter().map(|name| (*name).to_owned()).collect();
    let mut found: Vec<Vec<String>> = pipe_runs(text)
        .into_iter()
        .chain(backticked_runs(text))
        .filter(|run| run.iter().any(|name| wanted.contains(name)))
        .collect();
    found.sort();
    found.dedup();
    assert!(
        !found.is_empty(),
        "no list of these names is spelled out in:\n{text}"
    );
    for run in &found {
        assert_eq!(
            *run,
            wanted,
            "the list must name every entry of the registry, in order, and nothing else, in:\n{text}"
        );
    }
    found
}

fn bullet_terms(page: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for line in page.lines() {
        let Some(rest) = line.trim_start().strip_prefix("- `") else {
            continue;
        };
        let term: String = rest
            .chars()
            .take_while(|letter| !letter.is_whitespace() && *letter != '`')
            .collect();
        terms.push(term);
    }
    terms
}

#[test]
fn watch_help_and_page_name_exactly_the_watchable_commands() {
    let registry = dx_adopt::WATCHABLE_COMMANDS;
    let command = Command::Watch;
    for text in [command.usage(), command.scopes_text()] {
        slot_lists(text, registry);
    }
    let page = std::fs::read_to_string(docs_dir().join("watch.md")).expect("watch page");
    slot_lists(&page, registry);
}

#[test]
fn completion_help_banner_and_page_name_exactly_the_shells() {
    let registry = super::super::completion::COMPLETION_SHELLS;
    let command = Command::Completion;
    for text in [command.describe(), command.usage(), command.scopes_text()] {
        slot_lists(text, registry);
    }
    let page = std::fs::read_to_string(docs_dir().join("completion.md")).expect("completion page");
    slot_lists(&page, registry);
}

fn versions_pin(name: &str) -> String {
    let source = std::fs::read_to_string(workspace_root().join("modules/versions.bzl"))
        .expect("modules/versions.bzl ships as test data");
    let prefix = format!("\n{name} = \"");
    let start = source
        .find(&prefix)
        .unwrap_or_else(|| panic!("modules/versions.bzl has no {name} pin"))
        + prefix.len();
    let rest = &source[start..];
    let end = rest.find('"').expect("the pin literal is closed");
    rest[..end].to_owned()
}

#[test]
fn status_toolchain_detail_tracks_the_canonical_rust_pins() {
    let rust = versions_pin("RUST_VERSION");
    let rules_rust = versions_pin("FLOOR_RULES_RUST");
    let scratch = dx_test_scratch::scratch("dx-docs-toolchain-");
    std::fs::write(
        scratch.path().join("MODULE.bazel"),
        dx_adopt::render_consumer_module_setup(),
    )
    .expect("module");
    let tools = dx_adopt::StatusCheck {
        name: "tools".to_owned(),
        status: "warn".to_owned(),
        detail: String::new(),
        hint: String::new(),
    };
    let toolchain = dx_adopt::observed_status_checks(scratch.path(), "0.0.0", tools)
        .into_iter()
        .find(|check| check.name == "toolchain")
        .expect("dx status has a toolchain check");
    let expected =
        format!("available: rust {rust} via rules_rust {rules_rust} declared in MODULE.bazel");
    assert_eq!(toolchain.status, "ok", "{}", toolchain.detail);
    assert_eq!(toolchain.detail, expected, "dx status toolchain detail");
    let line = dx_adopt::render_status_text(&[toolchain]);
    let page = std::fs::read_to_string(docs_dir().join("status-version.md"))
        .expect("status page ships as test data");
    assert!(
        page.contains(&line),
        "docs/cli/commands/status-version.md must show the canonical toolchain line: {line}"
    );
}

#[test]
fn forwarding_pages_name_every_rejected_startup_option() {
    for page in ["quality.md", "environment-codegen.md"] {
        let text = std::fs::read_to_string(docs_dir().join(page))
            .unwrap_or_else(|err| panic!("read docs/cli/commands/{page}: {err}"));
        for name in STARTUP_OPTIONS {
            assert!(
                text.contains(&format!("`--{name}`")),
                "docs/cli/commands/{page} never names rejected startup option `--{name}`"
            );
        }
        assert!(
            text.contains("`--test_arg`"),
            "docs/cli/commands/{page} never names rejected `--test_arg`"
        );
        assert!(
            text.contains("[`--bazel-startup-option`](README.md#global-flags)"),
            "docs/cli/commands/{page} never points to `--bazel-startup-option`"
        );
    }
}
