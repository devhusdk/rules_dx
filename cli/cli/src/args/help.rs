use std::ffi::OsStr;

use super::command::Command;
use super::grammar::{Cli, VALUE_OPTIONS};
use super::{suggest, ArgsError};

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
            None => {
                let suggestion = suggest::suggest_command(&word);
                Some(ArgsError::UnknownCommand {
                    command: word,
                    suggestion,
                })
            }
        },
    }
}

pub(crate) fn render_top_help() -> String {
    use clap::{CommandFactory, ValueEnum};
    let mut out = String::new();
    if let Some(about) = Cli::command().get_about() {
        out.push_str(&format!("dx - {about}\n\n"));
    }
    out.push_str("Commands:\n");
    for command in Command::value_variants() {
        out.push_str(&format!(
            "  {:<12} {}\n",
            command.name(),
            command.describe()
        ));
    }
    out.push('\n');
    out.push_str(&Cli::command().render_long_help().to_string());
    out.push_str(&render_env_help());
    out
}

fn render_env_help() -> String {
    let mut out = String::new();
    out.push_str("\nEnvironment:\n");
    out.push_str("  RUST_LOG=<filter>\n");
    out.push_str("      Override --verbose and --log-level.\n");
    out.push_str("  NO_COLOR=<any>\n");
    out.push_str("      Disable color output.\n");
    out.push_str("  BUILD_WORKSPACE_DIRECTORY=<dir>\n");
    out.push_str("      Workspace start under `bazel run`.\n");
    out
}

pub(crate) fn per_command_flags(command: Command) -> &'static str {
    match command {
        Command::Check => {
            "Per-command flags: --check/--fail-on/--report pass through per phase (check only; non-mutating umbrella over format+lint+typecheck+generate, stop-on-first-failure)."
        }
        Command::Fix => {
            "Per-command flags: --check/--fail-on/--report pass through per phase (fix only; mutating by default with no rerun, run `dx check` to validate)."
        }
        Command::Clean => {
            "Per-command flags: --bazel (also run `bazel clean` after pruning; default never touches Bazel outputs; distinct from `dx bazel`, which forwards raw args; --output text|json only, diff has no patch)."
        }
        Command::Owners | Command::Deps | Command::Why => {
            "Per-command flags: --configured (use `bazel cquery` instead of `bazel query`; distinct from `dx clean --bazel`, which forwards `bazel clean`; --output text|json only, diff has no patch; JSON reuses the status envelope with one status event per label)."
        }
        Command::Coverage => {
            "Per-command flags: --min-coverage <0-100> (coverage only; collects without enforcing when absent)."
        }
        Command::Build | Command::Test | Command::Run | Command::Deploy => {
            "Per-command flags: --debug | --release (build/run/test/deploy only; mutually exclusive; bare invocation means dev, except deploy means release)."
        }
        Command::Version => {
            "Per-command flags: --check (drift check), --pin <version>, --rollback (version only; --pin and --rollback conflict; --output text|json only, diff has no patch; JSON reuses the status envelope)."
        }
        Command::Migrate => {
            "Per-command flags: --from <version> --to <version> (migrate only; both Cargo semver, upgrade-only gate)."
        }
        Command::New => {
            "Per-command flags: none (<language> [name]; rust|python|javascript|typescript|go|java|kotlin|scala|csharp|fsharp|c|cc|cpp; absent-only, no --force)."
        }
        Command::Upgrade => {
            "Per-command flags: --from <version> --to <version> (upgrade only; pin+migrate+setup composition with recovery pointer)."
        }
        Command::Security | Command::License => {
            "Per-command flags: --offline/--frozen (cache-only, no network fetches), --fail-on info|warning|error, --report sarif|spdx (security/license only; --check and `-- --bazel-options` do not apply; --output diff has no patch)."
        }
        Command::Update => {
            "Per-command flags: --offline/--frozen (cache-only, no network fetches), --check (preset stale gate; selectors ignored) (update only; --fail-on/--report and `-- --bazel-options` do not apply; --output diff has no patch)."
        }
        Command::Bump => {
            "Per-command flags: --offline/--frozen (cache-only, no network fetches; exactly one `set:package` plus version; --check/--fail-on/--report and `-- --bazel-options` do not apply; --output diff has no patch)."
        }
        Command::Bazel => {
            "Per-command flags: none (raw Bazel forwarding; dx-owned options must precede the command word and most are rejected)."
        }
        Command::Lint | Command::Typecheck | Command::Format | Command::Generate => {
            "Per-command flags: --check/--fail-on/--report (quality only; --here for cwd scope; --output text|diff|json; unsupported uses fail with `option \"--flag\" is not supported by dx <command>`)."
        }
        Command::Codegen | Command::Env | Command::Setup => {
            "Per-command flags: none (repository-wide or one exact // or @ label; --check/--fail-on/--report/--output diff and version/clean/inspect/migrate flags do not apply; --output text|json only; unsupported uses fail with `option \"--flag\" is not supported by dx <command>`)."
        }
        Command::Init => {
            "Per-command flags: none (optional [module-name]; --check/--fail-on/--report/--output json|diff and `-- --bazel-options` do not apply; unsupported uses fail with `option \"--flag\" is not supported by dx <command>`)."
        }
        Command::Hooks => {
            "Per-command flags: none (verbs install|uninstall|status|run [pre-commit|pre-push]; --check/--fail-on/--report/--output json|diff and `-- --bazel-options` do not apply; unsupported uses fail with `option \"--flag\" is not supported by dx <command>`)."
        }
        Command::Status => {
            "Per-command flags: none (no scopes; --output text|json only, diff has no patch; --check/--fail-on/--report/--pin/--rollback/--configured and `-- --bazel-options` do not apply; unsupported uses fail with `option \"--flag\" is not supported by dx <command>`; JSON streams command_started, one status event per check (name, status, detail, hint), optional status_pin_mismatch error, command_finished; no dx doctor, use dx status, see docs/cli/commands/status-version.md#failure-explainer)."
        }
        Command::Watch => {
            "Per-command flags: wrapped-command flags pass through per iteration (watch only wraps build|test|run|lint|typecheck|format|check|fix; local only, refuses CI; unsupported uses fail with `option \"--flag\" is not supported by dx <command>`)."
        }
        Command::Completion => {
            "Per-command flags: [--check] verifies without writing (exactly one <shell> bash|zsh|fish|powershell without --check; zero shells checks all, one checks that shell with --check; unknown shells fail with unknown-shell; --output json|diff and `-- --bazel-options` do not apply)."
        }
        Command::Docs => {
            "Per-command flags: --check/--serve/--port/--host/--open (docs only; --check validates without rendering, --serve previews the last build locally, --port/--host/--open require --serve; --output text|json only, diff has no patch)."
        }
    }
}

pub(crate) fn render_command_help(command: Command) -> String {
    let usage = match command {
        Command::Clean => "Usage: dx [global-options] clean [--dry-run] [--bazel]",
        Command::Bazel => "Usage: dx [global-options] bazel [-- bazel-args ...]",
        Command::Run => "Usage: dx [global-options] run [--debug|--release] <target> [-- app-args ...]",
        Command::Deploy => "Usage: dx [global-options] deploy [--debug|--release] <label> [-- app-args ...]",
        Command::Build | Command::Test => {
            "Usage: dx [global-options] build|test [--here] [--debug|--release] [scope ...] [-- bazel-options ...]"
        }
        Command::Coverage => {
            "Usage: dx [global-options] coverage [--here] [--min-coverage 0-100] [scope ...] [-- bazel-options ...]"
        }
        Command::Owners => "Usage: dx [global-options] owners [--configured] <scope> ...",
        Command::Deps => "Usage: dx [global-options] deps [--configured] <scope> ...",
        Command::Why => "Usage: dx [global-options] why [--configured] <file> <label>",
        Command::Version => {
            "Usage: dx [global-options] version [--check] [--pin <version>|--rollback]"
        }
        Command::Update => "Usage: dx [global-options] update [--offline|--frozen] [selector ...]",
        Command::Bump => "Usage: dx [global-options] bump [--offline|--frozen] <set:package> <version>",
        Command::Security => {
            "Usage: dx [global-options] security [--offline|--frozen] [--here] [scope ...]"
        }
        Command::License => {
            "Usage: dx [global-options] license [--offline|--frozen] [--here] [scope ...]"
        }
        Command::Migrate => {
            "Usage: dx [global-options] migrate --from <version> --to <version> [scope ...]"
        }
        Command::New => "Usage: dx [global-options] new <language> [name]",
        Command::Upgrade => {
            "Usage: dx [global-options] upgrade --from <version> --to <version>"
        }
        Command::Lint | Command::Typecheck | Command::Format | Command::Generate => {
            "Usage: dx [global-options] lint|typecheck|format|generate [--here] [scope ...] [-- bazel-options ...]"
        }
        Command::Check | Command::Fix => {
            "Usage: dx [global-options] check|fix [--here] [scope ...] [-- bazel-options ...]"
        }
        Command::Codegen | Command::Env | Command::Setup => {
            "Usage: dx [global-options] codegen|env|setup [<label>] [-- bazel-options ...]"
        }
        Command::Init => "Usage: dx [global-options] init [module-name]",
        Command::Hooks => {
            "Usage: dx [global-options] hooks <install|uninstall|status|run [pre-commit|pre-push]>"
        }
        Command::Status => "Usage: dx [global-options] status",
        Command::Watch => {
            "Usage: dx [global-options] watch <build|test|run|lint|typecheck|format|check|fix> [scope ...] [-- bazel-options ...]"
        }
        Command::Completion => {
            "Usage: dx [global-options] completion [<shell> bash|zsh|fish|powershell] [--check]"
        }
        Command::Docs => {
            "Usage: dx [global-options] docs [--check] [--serve [--port <n>] [--host <addr>] [--open]] [--here] [scope ...]"
        }
    };
    let scopes = match command {
        Command::Clean => "Scopes: none (clean takes no scopes).",
        Command::Bazel => "Scopes: none (raw Bazel forwarding; no dx scope resolution).",
        Command::Deploy => "Scopes: exactly one main-workspace label (//pkg:target); patterns (//...), multiple labels, and file/path scopes are usage failures.",
        Command::Run => "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Requires a scope (empty scope is a usage error); file/dir scopes need exactly one runnable.",
        Command::Update => "Scopes: dependency-set/package/target selectors (cargo|npm|maven|nuget|go, set:package, labels/paths); bare run updates all sets.",
        Command::Bump => "Scopes: exactly one `set:package` plus one new version (bazel|cargo|github-actions|go|maven|npm|nuget); never batch.",
        Command::Security => "Scopes: dependency-set/package/target selectors; bare run audits //... (secrets plus vulnerabilities). Pass --here (--cwd alias) for the current directory tree instead; --here cannot be combined with explicit scopes and never changes the no-flag default.",
        Command::License => "Scopes: dependency-set/package/target selectors; bare run audits //... (license policy plus SPDX inventory). Pass --here (--cwd alias) for the current directory tree instead; --here cannot be combined with explicit scopes and never changes the no-flag default.",
        Command::Migrate => "Scopes: explicit Bazel labels/patterns or workspace-relative files/dirs reusing generation scope resolution; external scopes rejected. No scope selects //....",
        Command::Lint
        | Command::Typecheck
        | Command::Format
        | Command::Generate
        | Command::Build
        | Command::Test
        | Command::Coverage
        | Command::Check
        | Command::Fix => "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied. Pass --here (--cwd alias) for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default.",
        Command::Codegen | Command::Env | Command::Setup => "Scopes: none for repository-wide canonical selection, or exactly one exact // or @ label; patterns, paths, and multiple labels are usage failures (see docs/cli/commands/environment-codegen-setup.md).",
        Command::Docs => "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Bare scope selects the repository docs site (//docs/site:user_site, //docs/site:user_site_aggregate in --check). Pass --here (--cwd alias) for the current directory tree instead (//path/...; //... at the root); --here cannot be combined with explicit scopes and never changes the no-flag default.",
        Command::Init => "Scopes: optional single module name (defaults to my_project when absent); Bazel labels/patterns are not scopes; extra positionals are usage failures.",
        Command::New => "Scopes: <language> plus optional project name (defaults to my_project); unknown languages fail with the supported list; extra positionals are usage failures.",
        Command::Upgrade => "Scopes: none (repository-wide pin+migrate+setup composition; --from/--to required, positional scopes rejected).",
        Command::Hooks => "Scopes: verb install|uninstall|status|run (run requires pre-commit|pre-push); no Bazel scopes; `-- --bazel-options` does not apply.",
        Command::Status => "Scopes: none (status takes no scopes).",
        Command::Watch => "Scopes: wrapped command plus its scopes, re-resolved each iteration (local only, refuses CI=true; only build|test|run|lint|typecheck|format|check|fix are watchable).",
        Command::Completion => "Scopes: exactly one shell (bash|zsh|fish|powershell) without --check, zero (all shells) or one with --check; unknown shells fail with unknown-shell.",
        _ => "Scopes: explicit Bazel labels/patterns (//..., //pkg:target, @repo//...), or workspace-relative files/dirs resolved via Bazel query. Graph-scope commands select //... when no scope is supplied; other commands follow per-command defaults (see docs/cli/commands/README.md#scope-defaults).",
    };
    let mut out = String::new();
    out.push_str(&format!(
        "dx {} - {}\n\n",
        command.name(),
        command.describe()
    ));
    out.push_str(usage);
    out.push('\n');
    out.push_str(per_command_flags(command));
    out.push_str("\n\n");
    out.push_str(scopes);
    out.push_str("\nExit codes: 0 success; 2 usage/scope/owner errors; 1 operational failures; Bazel-authoritative failures preserve Bazel's code.");
    out.push_str(
        "\nOutput: --output text|diff|json; --report <format>=<destination> (repeatable).",
    );
    out.push_str("\n\n");
    out.push_str(&render_top_help());
    out
}

#[cfg(test)]
mod tests {
    use super::super::{parse, ArgsError, Command};
    use super::render_command_help;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn top_level_help_lists_commands_and_flags() {
        for flag in ["--help", "-h"] {
            let text = match parse(&args(&[flag])) {
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
    fn per_command_help_covers_usage_scopes_exits_and_output() {
        for argv in [
            vec!["lint", "--help"],
            vec!["--help", "lint"],
            vec!["clean", "-h"],
        ] {
            let text = match parse(&args(&argv)) {
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
        let clean = match parse(&args(&["clean", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("clean --help: want Help, got {other:?}"),
        };
        assert!(
            clean.contains("clean [--dry-run] [--bazel]"),
            "clean usage:\n{clean}"
        );
        assert!(clean.contains("--bazel"), "clean flags:\n{clean}");
        assert!(
            clean.contains("distinct from `dx bazel`"),
            "clean disambiguation:\n{clean}"
        );
        assert!(
            clean.contains("never touches Bazel outputs") || clean.contains("never Bazel outputs"),
            "clean surprise:\n{clean}"
        );
        let fix_help = match parse(&args(&["fix", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("fix --help: want Help, got {other:?}"),
        };
        assert!(fix_help.contains("no rerun"), "fix surprise:\n{fix_help}");
        assert!(
            fix_help.contains("dx check"),
            "fix rerun guidance:\n{fix_help}"
        );
        let check_help = match parse(&args(&["check", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("check --help: want Help, got {other:?}"),
        };
        assert!(
            check_help.contains("non-mutating"),
            "check mode:\n{check_help}"
        );
        for command in ["owners", "deps", "why"] {
            let text = match parse(&args(&[command, "--help"])) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("{command} --help: want Help, got {other:?}"),
            };
            assert!(text.contains("[--configured]"), "{command} usage:\n{text}");
            assert!(
                text.contains("distinct from `dx clean --bazel`"),
                "{command} disambiguation:\n{text}"
            );
        }
        let coverage = match parse(&args(&["coverage", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("coverage --help: want Help, got {other:?}"),
        };
        assert!(
            coverage.contains("--min-coverage"),
            "coverage flags:\n{coverage}"
        );
        let build = match parse(&args(&["build", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("build --help: want Help, got {other:?}"),
        };
        assert!(build.contains("--debug"), "build flags:\n{build}");
        let version = match parse(&args(&["version", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("version --help: want Help, got {other:?}"),
        };
        assert!(version.contains("--pin"), "version flags:\n{version}");
        for argv in [["lint", "--help"], ["status", "--help"]] {
            let text = match parse(&args(&argv)) {
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
        let text = match parse(&args(&["--output", "bazel", "--help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("want Help, got {other:?}"),
        };
        assert!(text.contains("--output"));
    }

    #[test]
    fn help_verb_redirects_to_generated_help() {
        let top = match parse(&args(&["help"])) {
            Err(ArgsError::Help { text }) => text,
            other => panic!("help: want Help, got {other:?}"),
        };
        assert!(top.contains("Commands:"), "help:\n{top}");
        assert!(top.contains("bash|zsh|fish|powershell"), "help:\n{top}");
        for command in ["lint", "status", "completion"] {
            let verb = match parse(&args(&["help", command])) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("help {command}: want Help, got {other:?}"),
            };
            let flag = match parse(&args(&[command, "--help"])) {
                Err(ArgsError::Help { text }) => text,
                other => panic!("{command} --help: want Help, got {other:?}"),
            };
            assert_eq!(verb, flag, "help {command} must match --help");
        }
        assert!(matches!(
            parse(&args(&["help", "bogus"])),
            Err(ArgsError::UnknownCommand { .. })
        ));
    }

    #[test]
    fn top_help_names_completion_shells() {
        let text = match parse(&args(&["--help"])) {
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
        let text = match parse(&args(&["--help"])) {
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
        let text = match parse(&args(&["status", "--help"])) {
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
        let text = match parse(&args(&["completion", "--help"])) {
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
