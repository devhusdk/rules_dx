use std::ffi::OsStr;

use super::command::Command;
use super::completion::COMPLETION_SHELLS;
use super::grammar::{Cli, VALUE_OPTIONS};
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

pub(crate) fn help_command_in<S: AsRef<OsStr>>(args: &[S]) -> Option<Command> {
    let mut index = 0;
    while index < args.len() {
        let raw = args[index].as_ref();
        let arg = raw.to_str()?;
        if arg == "--" {
            return None;
        }
        if arg.starts_with('-') {
            let name = arg.split_once('=').map_or(arg, |(name, _)| name);
            if !arg.contains('=') && VALUE_OPTIONS.contains(&name) {
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
            let name = arg.split_once('=').map_or(arg, |(name, _)| name);
            if !arg.contains('=') && VALUE_OPTIONS.contains(&name) {
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
            let name = arg.split_once('=').map_or(arg, |(name, _)| name);
            if !arg.contains('=') && VALUE_OPTIONS.contains(&name) {
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
    out.push_str("  RUST_LOG=<filter>\n");
    out.push_str("      Override --verbose and --log-level.\n");
    out.push_str("  NO_COLOR=<any>\n");
    out.push_str("      Disable color output.\n");
    out.push_str("  BUILD_WORKSPACE_DIRECTORY=<dir>\n");
    out.push_str("      Workspace start under `bazel run`.\n");
    out.push_str(&format!(
        "  {}\n  {}\n      Same defaults as the DX_ variables, below the environment.\n      An unknown key is a usage error.\n",
        dx_adopt::defaults::CONFIG_TOML_REL,
        dx_adopt::defaults::CONFIG_REL
    ));
    out
}

const EXIT_CODES: &str = "Exit codes: 0 success; 2 usage/scope/owner errors; 1 operational failures; Bazel-authoritative failures preserve Bazel's code.";

pub(crate) fn per_command_flags(command: Command) -> &'static str {
    command.flags()
}

pub fn usage_banner() -> String {
    let commands = Command::pipe_list();
    let shells = COMPLETION_SHELLS.join("|");
    format!(
        "usage: dx <{commands}> [--workspace DIR] [--dry-run] [--quiet] [--verbose|-v] \
[--log-level error|warn|info|debug|trace] [--color auto|always|never] \
[--output text|diff|json] [--report <format>=<destination>]... \
[--fail-on info|warning|error] [--min-coverage 0-100 (coverage only)] \
[scope ...] [-- command-options...]\n\
flags go after the command: `dx lint --check //...`. \
per-command flags: clean --bazel|--prune-unobserved (also run `bazel clean`; default never touches Bazel outputs; \
distinct from `dx bazel` passthrough; --prune-unobserved prunes generations no observation protects); owners|deps|why --configured (cquery); \
coverage --min-coverage; build|run|test|deploy --debug|--release; \
version --check|--pin|--rollback; docs --check|--serve|--port|--host|--open; \
completion <{shells}> [--check] (no shell with --check verifies all). \
--check is per-command only (quality/version/update/docs/completion/check|fix; status rejects --check; \
see `dx <command> --help`). fix applies without rerun (run `dx check` to validate). \
no dx doctor; use `dx status` for diagnostics. \
see `dx help <command>` or `dx <command> --help`."
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
    format!(
        "Output: --output {}; {}",
        output_modes(command),
        report_clause(command)
    )
}

/// The flags that belong to some commands only, in the order help lists them.
const COMMAND_FLAGS: &[&str] = &[
    "--report",
    "--fail-on",
    "--min-coverage",
    "--check",
    "--apply",
    "--debug",
    "--release",
    "--bazel",
    "--prune-unobserved",
    "--pin",
    "--rollback",
    "--configured",
    "--from",
    "--to",
    "--here",
    "--serve",
    "--port",
    "--host",
    "--open",
    "--offline",
];

const PASSTHROUGH: &str = "-- <bazel-options>";

/// The flags the grammar gives this command's help, read off its clap subcommand.
pub(crate) fn advertised_flags(command: Command) -> Vec<&'static str> {
    let names = super::grammar::advertised_flags(command);
    COMMAND_FLAGS
        .iter()
        .copied()
        .filter(|flag| {
            let long = flag.trim_start_matches('-');
            names.iter().any(|name| name == long)
        })
        .collect()
}

/// Whether `dx <command> -- <bazel-options>` passes them through.
pub(crate) fn advertises_passthrough(command: Command) -> bool {
    accepts_bazel_options(command) && super::grammar::advertises_passthrough(command)
}

/// The flags the grammar names for some command but not this one.
pub(crate) fn rejected_flags(command: Command) -> Vec<&'static str> {
    let mut flags: Vec<&'static str> = COMMAND_FLAGS
        .iter()
        .copied()
        .filter(|flag| !advertised_flags(command).contains(flag))
        .collect();
    if !advertises_passthrough(command) {
        flags.push(PASSTHROUGH);
    }
    flags
}

/// Whether `dx <command> -- <bazel-options>` passes them through.
pub(crate) fn accepts_bazel_options(command: Command) -> bool {
    if command.is_adoption() {
        return false;
    }
    !matches!(
        command,
        Command::Security
            | Command::License
            | Command::Clean
            | Command::Update
            | Command::Bump
            | Command::Migrate
            | Command::Upgrade
    )
}

pub(crate) fn rejected_line(command: Command) -> String {
    format!("Rejected: {}.", rejected_flags(command).join(", "))
}

/// The prose clap appends to each command's long help.
pub(crate) fn after_long_help(command: Command) -> String {
    let mut out = String::new();
    out.push_str(per_command_flags(command));
    out.push_str("\n\n");
    out.push_str(command.scopes_text());
    out.push_str(&format!("\n{EXIT_CODES}"));
    out.push('\n');
    out.push_str(&output_line(command));
    out.push('\n');
    out.push_str(&rejected_line(command));
    out.push('\n');
    out.push_str(ENV_LINE);
    out
}

const ENV_LINE: &str = "Environment: RUST_LOG=<filter> overrides --verbose and --log-level; NO_COLOR=<any> disables color; \
BUILD_WORKSPACE_DIRECTORY=<dir> sets the workspace start. `dx --help` lists every DX_ default.";

pub(crate) fn render_command_help(command: Command) -> String {
    let root = super::grammar::cli_command();
    let mut out = String::new();
    if let Some(sub) = root.find_subcommand(command.name()) {
        out.push_str(&format!(
            "dx {} - {}\n\n",
            command.name(),
            command.describe()
        ));
        out.push_str(&sub.clone().render_long_help().to_string());
    } else {
        out.push_str(&format!(
            "dx {} - {}\n\n{}\n",
            command.name(),
            command.describe(),
            command.usage()
        ));
    }
    out.push('\n');
    out
}

#[cfg(test)]
#[path = "docs_parity_tests.rs"]
mod docs_parity_tests;

#[cfg(test)]
#[path = "fixture_pins_tests.rs"]
mod fixture_pins_tests;

#[cfg(test)]
mod tests {
    use super::super::{assert_usage, parse, ArgsError, Command};
    use super::render_command_help;
    use crate::test_support::strings;

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
                "--log-level",
                "Exit codes",
                "RUST_LOG",
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
            text.contains(dx_adopt::defaults::CONFIG_TOML_REL),
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
            ("bazel_help.golden", render_command_help(Command::Bazel)),
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
                "RUST_LOG",
                "NO_COLOR",
                "BUILD_WORKSPACE_DIRECTORY",
                "--workspace",
            ] {
                assert!(text.contains(needle), "{argv:?}: missing {needle:?}");
            }
        }
    }

    #[test]
    fn per_command_help_names_owned_flags_and_reconciles_bazel_naming() {
        let clean = match parse(&strings(&["clean", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("clean --help: want Help, got {other:?}"),
        };
        assert!(
            clean.contains("clean [--apply] [--dry-run] [--bazel] [--prune-unobserved]"),
            "clean usage:\n{clean}"
        );
        assert!(clean.contains("--apply"), "clean flags:\n{clean}");
        assert!(clean.contains("--bazel"), "clean flags:\n{clean}");
        assert!(
            clean.contains("--prune-unobserved"),
            "clean flags:\n{clean}"
        );
        assert!(
            clean.contains("distinct from `dx bazel`"),
            "clean disambiguation:\n{clean}"
        );
        assert!(
            clean.contains("never touches Bazel outputs") || clean.contains("never Bazel outputs"),
            "clean surprise:\n{clean}"
        );
        let fix_help = match parse(&strings(&["fix", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("fix --help: want Help, got {other:?}"),
        };
        assert!(fix_help.contains("no rerun"), "fix surprise:\n{fix_help}");
        assert!(
            fix_help.contains("dx check"),
            "fix rerun guidance:\n{fix_help}"
        );
        let check_help = match parse(&strings(&["check", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("check --help: want Help, got {other:?}"),
        };
        assert!(
            check_help.contains("non-mutating"),
            "check mode:\n{check_help}"
        );
        for command in ["owners", "deps", "why"] {
            let text = match parse(&strings(&[command, "--help"])) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("{command} --help: want Help, got {other:?}"),
            };
            assert!(text.contains("[--configured]"), "{command} usage:\n{text}");
            assert!(
                text.contains("distinct from `dx clean --bazel`"),
                "{command} disambiguation:\n{text}"
            );
        }
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
        assert!(version.contains("--pin"), "version flags:\n{version}");
        for argv in [["lint", "--help"], ["status", "--help"]] {
            let text = match parse(&strings(&argv)) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("{argv:?}: want Help, got {other:?}"),
            };
            assert!(text.contains("Per-command flags:"), "{argv:?}:\n{text}");
        }
        let bazel_help = render_command_help(Command::Bazel);
        assert!(
            bazel_help.contains("Per-command flags:"),
            "bazel help:\n{bazel_help}"
        );
    }

    #[test]
    fn help_value_option_payload_is_not_a_command() {
        let text = match parse(&strings(&["lint", "--output", "bazel", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("want Help, got {other:?}"),
        };
        assert!(text.contains("--output"));
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
            Command::Bump => owned(&["rust:serde", "1.0.0"]),
            Command::Migrate | Command::Upgrade => owned(&["--from", "0.1.0", "--to", "0.2.0"]),
            Command::Why => owned(&["README.md", "//:all"]),
            Command::Completion => owned(&["bash"]),
            Command::Hooks => owned(&["install"]),
            Command::Watch => owned(&["build"]),
            Command::Owners | Command::Deps => owned(&["//:all"]),
            Command::New => owned(&["rust"]),
            _ => Vec::new(),
        }
    }

    fn accepts_output(command: Command, mode: &str) -> bool {
        if command == Command::Bazel {
            return false;
        }
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
            assert_eq!(
                line,
                format!("Output: --output {}; {report}", modes.join("|")),
                "dx {} output line",
                command.name()
            );
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
            "--log-level",
            "--verbose",
            "-v",
            "RUST_LOG",
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
        for needle in [
            "--check",
            "status_pin_mismatch",
            "command_started",
            "no dx doctor",
        ] {
            assert!(
                text.contains(needle),
                "status help missing {needle:?}:\n{text}"
            );
        }
    }

    #[test]
    fn completion_help_names_check_verification() {
        let text = match parse(&strings(&["completion", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("want Help, got {other:?}"),
        };
        assert!(text.contains("--check"), "completion help:\n{text}");
        assert!(
            text.contains("bash|zsh|fish|powershell"),
            "completion help:\n{text}"
        );
    }
}
