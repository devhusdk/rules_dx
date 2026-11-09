use std::ffi::OsStr;

use super::ArgsError;
use super::command::Command;
use super::grammar::{Cli, option_at};

/// Index of the word after the flag `arg` at `index`, skipping its separate value word.
fn skip_flag<S: AsRef<OsStr>>(args: &[S], index: usize, arg: &str) -> usize {
    let next = args.get(index + 1).and_then(|next| next.as_ref().to_str());
    let used_next = option_at(arg, next).is_some_and(|option| option.used_next);
    index + 1 + usize::from(used_next)
}

pub fn help_command_in<S: AsRef<OsStr>>(args: &[S]) -> Option<Command> {
    let mut index = 0;
    while index < args.len() {
        let raw = args[index].as_ref();
        let arg = raw.to_str()?;
        if arg == "--" {
            return None;
        }
        if arg.starts_with('-') {
            index = skip_flag(args, index, arg);
            continue;
        }
        return Command::parse(arg);
    }
    None
}

pub(crate) fn help_verb_error_in<S: AsRef<OsStr>>(args: &[S]) -> Option<ArgsError> {
    let mut index = 0;
    let mut help_at: Option<usize> = None;
    while index < args.len() {
        let raw = args[index].as_ref();
        let Some(arg) = raw.to_str() else {
            break;
        };
        if arg == "--" {
            break;
        }
        if arg.starts_with('-') {
            index = skip_flag(args, index, arg);
            continue;
        }
        if arg == "help" {
            help_at = Some(index);
        }
        break;
    }
    let help_at = help_at?;
    let mut target: Option<String> = None;
    let mut scan = help_at + 1;
    while scan < args.len() {
        let raw = args[scan].as_ref();
        let Some(arg) = raw.to_str() else {
            target = Some(raw.to_string_lossy().into_owned());
            break;
        };
        if arg == "--" {
            break;
        }
        if arg.starts_with('-') {
            scan = skip_flag(args, scan, arg);
            continue;
        }
        target = Some(arg.to_owned());
        break;
    }
    match target {
        None => Some(ArgsError::Help {
            text: render_top_help(),
        }),
        Some(word) if word == "help" => Some(ArgsError::Help {
            text: render_top_help(),
        }),
        Some(word) => match Command::parse(&word) {
            Some(command) => Some(ArgsError::Help {
                text: render_command_help(command),
            }),
            None => Some(ArgsError::Usage {
                text: unknown_command_text(&word),
            }),
        },
    }
}

/// Ask clap to render the failure for a word that names no command.
pub(crate) fn unknown_command_text(word: &str) -> String {
    use clap::CommandFactory;
    let root = Cli::command();
    let names: Vec<String> = root
        .get_subcommands()
        .map(|sub| sub.get_name().to_owned())
        .collect();
    let mut out = root.try_get_matches_from(["dx", word]).err().map_or_else(
        || format!("unknown command {word:?}"),
        |error| super::tokenizer::without_usage(&error.render().to_string()),
    );
    out.push_str(&format!("\nAvailable commands: {}", names.join("|")));
    out
}

pub(crate) fn render_top_help() -> String {
    use clap::CommandFactory;
    let mut out = String::new();
    if let Some(about) = Cli::command().get_about() {
        out.push_str(&format!("dx - {about}\n\n"));
    }
    out.push_str(&Cli::command().render_long_help().to_string());
    out.push_str(&render_env_help());
    out
}

fn render_env_help() -> String {
    use dx_adopt::defaults::ENV_DEFAULTS;
    let mut out = String::new();
    out.push_str("\nEnvironment:\n");
    for (env, flag, shape) in ENV_DEFAULTS {
        out.push_str(&format!("  {env}=<{shape}>\n"));
        out.push_str(&format!("      Default for {flag}.\n"));
    }
    out.push_str(
        "  <auto|n|off> sizes build slots from memory, takes exactly n (1-1024), or takes none.\n",
    );
    out.push_str(
        "  <duration> is seconds, or a number ending in s, m or h. 0 fails at once when no slot is free.\n",
    );
    out.push_str("  NO_COLOR=<non-empty>\n");
    out.push_str("      Disable color output.\n");
    out.push_str("  BUILD_WORKSPACE_DIRECTORY=<dir>\n");
    out.push_str("      Workspace start under `bazel run`.\n");
    out.push_str("  BUILD_WORKING_DIRECTORY=<dir>\n");
    out.push_str("      Current directory under `bazel run`, for `--here` and a relative `worktree release <path>`.\n");
    out.push_str("  In CI the DX_ variables are ignored; flags still win.\n");
    out
}

/// The exit-code line for one command's long help.
pub(crate) fn exit_codes(command: Command) -> &'static str {
    if command == Command::Watch {
        "Exit codes: 2 usage errors, not watchable, or CI=true; 1 operational failures such as watch_failed; 141 when stdout closes; 129 after SIGHUP, 130 after Ctrl-C, 143 after SIGTERM."
    } else if matches!(
        command,
        Command::Lint | Command::Typecheck | Command::Format
    ) {
        "Exit codes: 0 success; 2 usage/scope/owner errors; 1 findings, operational failures, and Bazel failures (Bazel's code is not kept)."
    } else if !runs_bazel(command) {
        "Exit codes: 0 success; 2 usage/scope/owner errors; 1 operational failures."
    } else {
        "Exit codes: 0 success; 2 usage/scope/owner errors; 1 operational failures; Bazel-authoritative failures preserve Bazel's code."
    }
}

/// Whether `dx <command>` can launch Bazel and so preserve its exit code.
pub fn runs_bazel(command: Command) -> bool {
    !matches!(
        command,
        Command::Security
            | Command::License
            | Command::Clean
            | Command::Update
            | Command::Upgrade
            | Command::Completion
            | Command::Init
            | Command::Worktree
    )
}

pub fn usage_banner() -> String {
    let commands = Command::pipe_list();
    format!(
        "usage: dx <{commands}> [flags] [scope ...] [-- bazel-options ...]\n\
see `dx <command> --help`."
    )
}

pub(crate) fn output_modes(command: Command) -> String {
    let mut modes = vec!["text"];
    if command.supports_json() {
        modes.push("json");
    }
    modes.join("|")
}

fn report_clause(command: Command) -> String {
    let formats = crate::reports::format_names(crate::plan::spec(command).reports).join("|");
    if formats.is_empty() {
        return "--report has no standard format for this command.".to_owned();
    }
    format!("--report {formats}=<destination> (repeatable).")
}

pub(crate) fn output_line(command: Command) -> String {
    if !command.supports_json() {
        return format!("Output: text only; {}", report_clause(command));
    }
    let streamed = if command.output_caps().streamed {
        " (json streams events while the command runs)"
    } else {
        ""
    };
    format!(
        "Output: --output {}{streamed}; {}",
        output_modes(command),
        report_clause(command)
    )
}

/// Example invocations shown in each command's long help, without the leading `dx`.
const EXAMPLES: &[(Command, &[&str])] = &[
    (
        Command::Security,
        &["security", "security --offline --fail-on warning"],
    ),
    (Command::License, &["license", "license --affected"]),
    (
        Command::Lint,
        &["lint --affected", "lint --apply //pkg/..."],
    ),
    (
        Command::Typecheck,
        &["typecheck --affected", "typecheck //pkg/..."],
    ),
    (
        Command::Format,
        &["format --apply", "format --affected --diff"],
    ),
    (
        Command::Generate,
        &["generate", "generate --apply //pkg/...", "generate --affected"],
    ),
    (
        Command::Build,
        &[
            "build //...",
            "build --release //app -- --local_resources=cpu=4",
        ],
    ),
    (Command::Test, &["test --affected", "test //pkg/..."]),
    (
        Command::Coverage,
        &[
            "coverage //pkg/...",
            "coverage --min-coverage 80 --report lcov=lcov.info",
        ],
    ),
    (
        Command::Bench,
        &[
            "bench //...",
            "bench //app:parse_bench --output=json",
            "bench --compare main --fail-on=regression:20",
        ],
    ),
    (
        Command::Run,
        &["run //app", "run --release //app -- --port 8080"],
    ),
    (
        Command::Deploy,
        &["deploy //app:push", "deploy --apply //app:push"],
    ),
    (Command::Check, &["check --affected", "check --apply"]),
    (Command::Clean, &["clean", "clean --bazel"]),
    (Command::Update, &["update --outdated", "update --apply"]),
    (Command::Sync, &["sync", "sync //proto:api"]),
    (Command::Init, &["init", "init --apply"]),
    (Command::Upgrade, &["upgrade", "upgrade --apply --to 0.2.0"]),
    (Command::Status, &["status", "status --output json"]),
    (
        Command::Watch,
        &["watch test //pkg/...", "watch lint --affected"],
    ),
    (Command::Completion, &["completion bash", "completion zsh"]),
    (Command::Docs, &["docs", "docs --serve --open"]),
    (
        Command::Worktree,
        &["worktree add my-branch", "worktree list --disk"],
    ),
    (
        Command::Fuzz,
        &["fuzz //pkg:parser", "fuzz --budget 5m //pkg/..."],
    ),
    (
        Command::Config,
        &["config", "config hlint", "config diff hlint"],
    ),
];

fn examples_text(command: Command) -> String {
    let mut out = String::from("Examples:\n");
    for (owner, lines) in EXAMPLES {
        if *owner == command {
            for line in *lines {
                out.push_str(&format!("  dx {line}\n"));
            }
        }
    }
    out
}

/// The prose clap appends to each command's long help.
pub(crate) fn after_long_help(command: Command) -> String {
    let mut out = examples_text(command);
    out.push_str(command.scopes_text());
    out.push('\n');
    out.push_str(exit_codes(command));
    out.push('\n');
    out.push_str(&output_line(command));
    out.push('\n');
    out.push_str(ENV_LINE);
    out
}

const ENV_LINE: &str = "Environment: NO_COLOR=<non-empty> disables color; \
BUILD_WORKSPACE_DIRECTORY=<dir> sets the workspace start. `dx --help` lists every DX_ default.";

pub(crate) fn render_command_help(command: Command) -> String {
    let root = super::grammar::cli_command();
    let mut out = String::new();
    if let Some(sub) = root.find_subcommand(command.name()) {
        out = sub.clone().render_long_help().to_string();
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::super::{ArgsError, Command, assert_usage, parse};
    use super::{render_command_help, usage_banner};
    use dx_testing::cli::strings;

    #[test]
    fn the_help_command_scan_skips_the_value_after_a_short_cluster() {
        use super::super::Command;
        assert_eq!(
            super::help_command_in(&["-qo", "json", "lint"]),
            Some(Command::Lint)
        );
        assert_eq!(super::help_command_in(&["-qo", "lint"]), None);
    }

    #[test]
    fn help_scan_skips_short_cluster_value_words() {
        assert!(super::help_verb_error_in(&["-qo", "json", "help"]).is_some());
        assert!(super::help_verb_error_in(&["-qo", "json", "lint"]).is_none());
    }

    #[test]
    fn every_command_has_two_examples_that_parse() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let lines: Vec<&&str> = super::EXAMPLES
                .iter()
                .filter(|(owner, _)| owner == command)
                .flat_map(|(_, lines)| lines.iter())
                .collect();
            assert!(lines.len() >= 2, "{} needs two examples", command.name());
            for line in lines {
                let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
                let outcome = parse(&words);
                assert!(
                    !matches!(outcome, Err(ArgsError::Usage { .. })),
                    "stale example `dx {line}`: {outcome:?}"
                );
            }
        }
    }

    #[test]
    fn every_command_has_a_help_subcommand() {
        use clap::ValueEnum;
        let root = super::super::grammar::cli_command();
        for command in Command::value_variants() {
            assert!(
                root.find_subcommand(command.name()).is_some(),
                "{}",
                command.name()
            );
        }
    }

    #[test]
    fn top_level_help_lists_commands_and_flags() {
        for flag in ["--help", "-h"] {
            let text = match parse(&strings(&[flag])) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("{flag}: want Help, got {other:?}"),
            };
            for needle in [
                "dx",
                "lint",
                "build",
                "--output",
                "Exit codes",
                "NO_COLOR",
                "BUILD_WORKSPACE_DIRECTORY",
            ] {
                assert!(text.contains(needle), "{flag}: missing {needle:?}");
            }
            for removed in ["mutate", "explain", "env"] {
                let prefix = format!("  {removed} ");
                assert!(
                    !text.lines().any(|line| line.starts_with(&prefix)),
                    "{flag}: removed command {removed:?} is listed"
                );
            }
        }
    }

    #[test]
    fn top_help_environment_names_every_env_default() {
        let text = super::render_top_help();
        for (env, flag, shape) in dx_adopt::defaults::ENV_DEFAULTS {
            assert!(
                text.contains(&format!("  {env}=<{shape}>")),
                "top help never names {env}"
            );
            assert!(
                text.contains(&format!("Default for {flag}.")),
                "top help never says {env} defaults {flag}"
            );
        }
    }

    #[test]
    fn rendered_help_matches_the_goldens() {
        let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
        let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
        let dir = std::path::Path::new(&root)
            .join(workspace)
            .join("cli/cli/tests/fixtures/help_goldens");
        let rendered = [
            ("build_help.golden", render_command_help(Command::Build)),
            ("clean_help.golden", render_command_help(Command::Clean)),
            ("docs_help.golden", render_command_help(Command::Docs)),
            ("lint_help.golden", render_command_help(Command::Lint)),
            ("top_help.golden", super::render_top_help()),
        ];
        for (name, text) in &rendered {
            let expected =
                std::fs::read_to_string(dir.join(name)).expect("golden ships as test data");
            assert_eq!(*text, expected, "{name} no longer matches dx help");
        }
        let mut shipped: Vec<String> = std::fs::read_dir(&dir)
            .expect("readable goldens dir")
            .map(|entry| {
                entry
                    .expect("readable dir entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name.ends_with(".golden"))
            .collect();
        shipped.sort();
        let mut covered: Vec<String> = rendered
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect();
        covered.sort();
        assert_eq!(
            shipped, covered,
            "every shipped help golden is asserted against rendered dx help"
        );
    }

    #[test]
    fn per_command_help_covers_usage_scopes_exits_and_output() {
        for argv in [
            vec!["lint", "--help"],
            vec!["--help", "lint"],
            vec!["clean", "-h"],
        ] {
            let text = match parse(&strings(&argv)) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("{argv:?}: want Help, got {other:?}"),
            };
            let command = argv
                .iter()
                .find(|word| Command::parse(word).is_some())
                .expect("command");
            for needle in [
                command,
                "Usage:",
                "Scopes:",
                "Exit codes:",
                "Output:",
                "Environment:",
                "NO_COLOR",
                "BUILD_WORKSPACE_DIRECTORY",
                "--workspace",
            ] {
                assert!(text.contains(needle), "{argv:?}: missing {needle:?}");
            }
        }
    }

    #[test]
    fn reject_scope_commands_do_not_render_a_targets_positional() {
        use super::super::command::COMMANDS;
        for entry in COMMANDS.iter().filter(|entry| {
            entry.scope_policy == "reject" && !["worktree", "config"].contains(&entry.name)
        }) {
            let text = render_command_help(entry.command);
            assert!(
                !text.contains("[TARGETS]"),
                "{} rejects scopes but help renders [TARGETS]:\n{text}",
                entry.name
            );
        }
    }

    #[test]
    fn per_command_help_names_owned_flags_and_reconciles_bazel_naming() {
        let clean = match parse(&strings(&["clean", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("clean --help: want Help, got {other:?}"),
        };
        assert!(
            clean.contains("clean [--bazel | --expunge]"),
            "clean usage:\n{clean}"
        );
        assert!(clean.contains("--expunge"), "clean flags:\n{clean}");
        assert!(!clean.contains("--apply"), "clean has no apply:\n{clean}");
        let apply_help = |command: Command| {
            super::super::grammar::subcommand_args(command)
                .get_arguments()
                .find(|arg| arg.get_id() == "apply")
                .and_then(|arg| arg.get_help())
                .map(ToString::to_string)
        };
        for command in [
            Command::Check,
            Command::Lint,
            Command::Deploy,
            Command::Init,
            Command::Upgrade,
        ] {
            assert_eq!(
                apply_help(command).as_deref(),
                Some("Apply the changes instead of only showing them")
            );
        }
        let check_help = match parse(&strings(&["check", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("check --help: want Help, got {other:?}"),
        };
        assert!(check_help.contains("--apply"), "check apply:\n{check_help}");
        assert!(
            check_help.contains("verifies with a read-only check"),
            "check verification guidance:\n{check_help}"
        );
        let coverage = match parse(&strings(&["coverage", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("coverage --help: want Help, got {other:?}"),
        };
        assert!(
            coverage.contains("--min-coverage"),
            "coverage flags:\n{coverage}"
        );
        let build = match parse(&strings(&["build", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("build --help: want Help, got {other:?}"),
        };
        assert!(build.contains("--debug"), "build flags:\n{build}");
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let text = render_command_help(*command);
            assert_eq!(
                text.matches(command.describe()).count(),
                1,
                "{} describe must appear once:\n{text}",
                command.name()
            );
            assert!(
                !text.contains("Per-command flags"),
                "{}:\n{text}",
                command.name()
            );
        }
    }

    #[test]
    fn help_value_option_payload_is_not_a_command() {
        let text = match parse(&strings(&["lint", "--workspace", "bazel", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("want Help, got {other:?}"),
        };
        assert!(text.contains("--workspace"));
    }

    #[test]
    fn help_verb_redirects_to_generated_help() {
        let top = match parse(&strings(&["help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("help: want Help, got {other:?}"),
        };
        assert!(top.contains("Commands:"), "help:\n{top}");
        assert!(top.contains("bash|zsh|fish|powershell"), "help:\n{top}");
        for command in ["lint", "status", "completion"] {
            let verb = match parse(&strings(&["help", command])) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("help {command}: want Help, got {other:?}"),
            };
            let flag = match parse(&strings(&[command, "--help"])) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("{command} --help: want Help, got {other:?}"),
            };
            assert_eq!(verb, flag, "help {command} must match --help");
        }
        assert_usage(
            ["help", "bogus"],
            parse(&strings(&["help", "bogus"])).unwrap_err(),
            &["bogus"],
        );
    }

    fn completing_words(command: Command) -> Vec<String> {
        let owned = |words: &[&str]| words.iter().map(|word| (*word).to_owned()).collect();
        match command {
            Command::Upgrade => owned(&["--to", "0.2.0"]),
            Command::Completion => owned(&["bash"]),
            Command::Watch => owned(&["lint"]),
            _ => Vec::new(),
        }
    }

    fn accepts_output(command: Command, mode: &str) -> bool {
        let mut words = vec![command.name().to_owned()];
        words.push(format!("--output={mode}"));
        words.extend(completing_words(command));
        parse(&words).is_ok()
    }

    #[test]
    fn output_line_names_only_accepted_modes_and_registry_reports() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let command = *command;
            let help = render_command_help(command);
            let line = help
                .lines()
                .find(|line| line.starts_with("Output: "))
                .unwrap_or_else(|| panic!("dx {} help has no Output line", command.name()));
            let mut modes = vec!["text"];
            {
                let mode = "json";
                if accepts_output(command, mode) {
                    modes.push(mode);
                }
            }
            let reports = crate::reports::format_names(crate::plan::spec(command).reports);
            let report = if reports.is_empty() {
                "--report has no standard format for this command.".to_owned()
            } else {
                format!("--report {}=<destination> (repeatable).", reports.join("|"))
            };
            let want = if accepts_output(command, "text") {
                {
                let streamed = if command.output_caps().streamed {
                    " (json streams events while the command runs)"
                } else {
                    ""
                };
                format!("Output: --output {}{streamed}; {report}", modes.join("|"))
            }
            } else {
                format!("Output: text only; {report}")
            };
            assert_eq!(line, want, "dx {} output line", command.name());
        }
    }

    #[test]
    fn top_help_names_completion_shells() {
        let text = match parse(&strings(&["--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("want Help, got {other:?}"),
        };
        assert!(
            text.contains("bash|zsh|fish|powershell"),
            "top help must list completion shells:\n{text}"
        );
    }

    #[test]
    fn top_help_documents_the_environment() {
        let text = match parse(&strings(&["--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("want Help, got {other:?}"),
        };
        for needle in ["NO_COLOR", "BUILD_WORKSPACE_DIRECTORY", "Environment:"] {
            assert!(
                text.contains(needle),
                "top help missing verbosity/env {needle:?}:\n{text}"
            );
        }
    }

    #[test]
    fn status_help_hints_rejected_flags_and_ndjson_shape() {
        let text = match parse(&strings(&["status", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("want Help, got {other:?}"),
        };
        assert!(text.contains("Usage: dx status"), "status help:\n{text}");
    }

    #[test]
    fn usage_banner_is_two_lines_ending_with_the_see_hint() {
        let banner = usage_banner();
        assert!(banner.starts_with("usage: dx <"), "{banner}");
        assert_eq!(banner.lines().count(), 2, "{banner}");
        assert!(banner.ends_with("see `dx <command> --help`."), "{banner}");
    }

    #[test]
    fn completion_help_names_shells_without_check() {
        let text = match parse(&strings(&["completion", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("want Help, got {other:?}"),
        };
        assert!(!text.contains("--check]"), "completion help:\n{text}");
        assert!(
            text.contains("bash|zsh|fish|powershell"),
            "completion help:\n{text}"
        );
    }
}
