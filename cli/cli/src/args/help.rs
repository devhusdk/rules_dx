use std::ffi::OsStr;

use super::command::Command;
use super::grammar::{consumes_next, Cli};
use super::ArgsError;

fn skip_value_payload<S: AsRef<OsStr>>(args: &[S], index: usize) -> usize {
    match args.get(index + 1) {
        Some(next) => {
            let raw = next.as_ref();
            let is_flag =
                raw == OsStr::new("--") || raw.to_str().is_some_and(|text| text.starts_with("--"));
            if is_flag {
                index + 1
            } else {
                index + 2
            }
        }
        None => index + 1,
    }
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
            let next = args.get(index + 1).and_then(|next| next.as_ref().to_str());
            if consumes_next(arg, next) {
                index = skip_value_payload(args, index);
                continue;
            }
            index += 1;
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
            let next = args.get(index + 1).and_then(|next| next.as_ref().to_str());
            if consumes_next(arg, next) {
                index = skip_value_payload(args, index);
                continue;
            }
            index += 1;
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
            let next = args.get(scan + 1).and_then(|next| next.as_ref().to_str());
            if consumes_next(arg, next) {
                scan = skip_value_payload(args, scan);
                continue;
            }
            scan += 1;
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
    use dx_adopt::defaults::{ENV_DEFAULTS, FALSEY, TRUTHY};
    let mut out = String::new();
    out.push_str("\nEnvironment:\n");
    for (env, flag, shape) in ENV_DEFAULTS {
        out.push_str(&format!("  {env}=<{shape}>\n"));
        out.push_str(&format!("      Default for {flag}.\n"));
    }
    out.push_str(&format!(
        "  <bool> is on for {} and off for {}.\n      Anything else is a usage error. An empty value is unset.\n",
        TRUTHY.join("|"),
        FALSEY.join("|")
    ));
    out.push_str("  NO_COLOR=<any>\n");
    out.push_str("      Disable color output.\n");
    out.push_str("  BUILD_WORKSPACE_DIRECTORY=<dir>\n");
    out.push_str("      Workspace start under `bazel run`.\n");
    out.push_str("  BUILD_WORKING_DIRECTORY=<dir>\n");
    out.push_str("      Current directory under `bazel run`.\n");
    out.push_str(&format!(
        "  {} (committed) and {} (local-only, gitignored) carry the same defaults as the DX_ variables, below the environment.\n      Local beats committed. In CI the local file and DX_ variables are ignored; flags still win.\n      An unknown key is a usage error.\n",
        dx_adopt::defaults::DX_TOML_REL,
        dx_adopt::defaults::DX_LOCAL_TOML_REL,
    ));
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
pub(crate) fn runs_bazel(command: Command) -> bool {
    !matches!(
        command,
        Command::Security
            | Command::License
            | Command::Clean
            | Command::Update
            | Command::Bump
            | Command::Add
            | Command::Migrate
            | Command::Upgrade
            | Command::Version
            | Command::Completion
            | Command::Init
            | Command::New
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
    if command.supports_diff() {
        modes.push("diff");
    }
    if command.supports_json() {
        modes.push("json");
    }
    if command == Command::Worktree {
        modes.push("path");
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
    format!(
        "Output: --output {}; {}",
        output_modes(command),
        report_clause(command)
    )
}

/// The prose clap appends to each command's long help.
pub(crate) fn after_long_help(command: Command) -> String {
    let mut out = String::new();
    out.push_str(command.scopes_text());
    out.push('\n');
    out.push_str(exit_codes(command));
    out.push('\n');
    out.push_str(&output_line(command));
    out.push('\n');
    out.push_str(ENV_LINE);
    out
}

const ENV_LINE: &str = "Environment: NO_COLOR=<any> disables color; \
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
#[path = "docs_parity_tests.rs"]
mod docs_parity_tests;

#[cfg(test)]
#[path = "quality_docs_parity_tests.rs"]
mod quality_docs_parity_tests;

#[cfg(test)]
mod tests {
    use super::super::{assert_usage, parse, ArgsError, Command};
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
                "--workspace",
                "--output",
                "--fail-on",
                "Exit codes",
                "NO_COLOR",
                "BUILD_WORKSPACE_DIRECTORY",
            ] {
                assert!(text.contains(needle), "{flag}: missing {needle:?}");
            }
        }
    }

    #[test]
    fn help_process_exits_zero_with_brand() {
        let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
        let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
        let binary = std::path::Path::new(&root)
            .join(workspace)
            .join("cli/cli/dx");
        assert_cmd::Command::new(binary)
            .arg("--help")
            .assert()
            .success()
            .stdout(predicates::str::contains("Run Bazel workflows"));
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
        assert!(
            text.contains(dx_adopt::defaults::DX_TOML_REL),
            "top help never names the config file"
        );
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
        for entry in COMMANDS
            .iter()
            .filter(|entry| entry.scope_policy == "reject" && entry.name != "worktree")
        {
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
            clean.contains("clean [--check] [--apply] [--prune-unobserved]"),
            "clean usage:\n{clean}"
        );
        assert!(clean.contains("--apply"), "clean flags:\n{clean}");
        assert!(
            clean.contains("--prune-unobserved"),
            "clean flags:\n{clean}"
        );
        assert!(
            clean.contains("never touches Bazel outputs") || clean.contains("never Bazel outputs"),
            "clean surprise:\n{clean}"
        );
        let fix_help = match parse(&strings(&["fix", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("fix --help: want Help, got {other:?}"),
        };
        assert!(fix_help.contains("--apply"), "fix surprise:\n{fix_help}");
        assert!(
            fix_help.contains("Write fixes to source files"),
            "fix apply help:\n{fix_help}"
        );
        let apply_help = |command: Command| {
            super::super::grammar::subcommand_args(command)
                .get_arguments()
                .find(|arg| arg.get_id() == "apply")
                .and_then(|arg| arg.get_help())
                .map(ToString::to_string)
        };
        assert_eq!(
            apply_help(Command::Fix).as_deref(),
            Some("Write fixes to source files")
        );
        assert_eq!(
            apply_help(Command::Clean).as_deref(),
            Some("Prune unselected managed state")
        );
        assert_eq!(
            apply_help(Command::Lint).as_deref(),
            Some("Write fixes to source files")
        );
        assert_eq!(
            apply_help(Command::Deploy).as_deref(),
            Some("Authorize the managed mutation or effect")
        );
        assert!(
            fix_help.contains("verifies with a read-only check"),
            "fix verification guidance:\n{fix_help}"
        );
        let check_help = match parse(&strings(&["check", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("check --help: want Help, got {other:?}"),
        };
        assert!(
            check_help.contains("non-mutating"),
            "check mode:\n{check_help}"
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
        let version = match parse(&strings(&["version", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("version --help: want Help, got {other:?}"),
        };
        assert!(version.contains("--check"), "version flags:\n{version}");
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
            &["help", "bogus"],
            parse(&strings(&["help", "bogus"])).unwrap_err(),
            &["bogus"],
        );
    }

    fn completing_words(command: Command) -> Vec<String> {
        let owned = |words: &[&str]| words.iter().map(|word| (*word).to_owned()).collect();
        match command {
            Command::Bump | Command::Add => owned(&["rust:serde", "1.0.0"]),
            Command::Migrate | Command::Upgrade => owned(&["--from", "0.1.0", "--to", "0.2.0"]),
            Command::Completion => owned(&["bash"]),
            Command::Watch => owned(&["lint"]),
            Command::New => owned(&["rust"]),
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
            for mode in ["diff", "json"] {
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
            if command == Command::Worktree {
                modes.push("path");
            }
            let want = if accepts_output(command, "text") {
                format!("Output: --output {}; {report}", modes.join("|"))
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
    fn top_help_documents_verbosity_and_environment() {
        let text = match parse(&strings(&["--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("want Help, got {other:?}"),
        };
        for needle in [
            "--verbose",
            "-v",
            "NO_COLOR",
            "BUILD_WORKSPACE_DIRECTORY",
            "Environment:",
        ] {
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
        assert!(!text.contains("--check"), "status help:\n{text}");
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
