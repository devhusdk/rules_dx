use super::super::{ArgsError, Command};
use super::parse;
use dx_testing::cli::strings;

const AFFECTED: [Command; 9] = [
    Command::Build,
    Command::Test,
    Command::Coverage,
    Command::Lint,
    Command::Typecheck,
    Command::Format,
    Command::Check,
    Command::Security,
    Command::License,
];

#[test]
fn affected_is_accepted_by_the_scope_taking_commands() {
    for command in AFFECTED {
        let words = [command.name(), "--affected"];
        let invocation = parse(&strings(&words)).expect("parses");
        assert!(invocation.affected, "{}", command.name());
        assert_eq!(invocation.targets, [] as [String; 0]);
    }
}

#[test]
fn affected_takes_scopes_on_the_graph_commands() {
    for command in [
        Command::Build,
        Command::Test,
        Command::Coverage,
        Command::Lint,
        Command::Typecheck,
        Command::Format,
        Command::Check,
    ] {
        let name = command.name();
        for words in [
            [name, "--affected", "//a/..."],
            [name, "//a/...", "--affected"],
        ] {
            let invocation = parse(&strings(&words)).expect("parses");
            assert!(invocation.affected, "{name}");
            assert_eq!(invocation.targets, ["//a/..."], "{name}");
        }
    }
    for command in [Command::Security, Command::License, Command::Deploy] {
        assert_eq!(
            parse(&strings(&[command.name(), "--affected", "//a:b"])),
            Err(ArgsError::ConflictingAffected),
            "{}",
            command.name()
        );
    }
}

#[test]
fn affected_conflicts_with_here() {
    assert_eq!(
        parse(&strings(&["lint", "--affected", "--here"])),
        Err(ArgsError::AffectedWithHere)
    );
    assert!(parse(&strings(&["test", "--affected=yes"])).is_err());
}

#[test]
fn the_grammar_accepts_affected_exactly_for_the_affected_owners() {
    use crate::args::command::owners::AFFECTED_OWNERS;
    use crate::args::grammar::accepts_long;
    use clap::ValueEnum;
    for command in Command::value_variants() {
        assert_eq!(
            accepts_long(*command, "affected"),
            AFFECTED_OWNERS.contains(command) || *command == Command::Watch,
            "{}",
            command.name()
        );
    }
}

#[test]
fn scoped_commands_run_everything_without_a_scope() {
    for command in AFFECTED.iter().copied() {
        let name = command.name();
        let bare = parse(&strings(&[name])).expect("bare");
        assert!(!bare.affected && bare.targets.is_empty(), "{name}");
        let typed = parse(&strings(&[name, "--affected"])).expect("typed");
        assert!(typed.affected, "{name}");
        assert!(!parse(&strings(&[name, "//..."])).expect("scoped").affected);
        assert!(!parse(&strings(&[name, "--here"])).expect("here").affected);
    }
}

#[test]
fn since_needs_affected() {
    let got = parse(&strings(&["test", "--affected", "--since", "HEAD~1"])).expect("since");
    assert_eq!(got.since.as_deref(), Some("HEAD~1"));
    assert!(matches!(
        parse(&strings(&["test", "--since", "HEAD~1"])),
        Err(ArgsError::SinceNeedsAffected)
    ));
}

#[test]
fn deploy_takes_affected_and_since_but_no_label() {
    let invocation = parse(&strings(&["deploy", "--affected", "--since=main"])).expect("parses");
    assert!(invocation.affected);
    assert_eq!(invocation.targets, [] as [String; 0]);
    assert_eq!(
        parse(&strings(&["deploy", "--affected", "//a:b"])),
        Err(ArgsError::ConflictingAffected)
    );
    assert!(parse(&strings(&["deploy", "--since=main"])).is_err());
}

#[test]
fn base_needs_affected_and_excludes_since() {
    let got = parse(&strings(&[
        "test",
        "--affected",
        "--base",
        "origin/develop",
    ]))
    .expect("base");
    assert_eq!(got.base.as_deref(), Some("origin/develop"));
    assert_eq!(got.since, None);
    assert!(matches!(
        parse(&strings(&["test", "--base", "origin/develop"])),
        Err(ArgsError::BaseNeedsAffected)
    ));
    assert!(parse(&strings(&["test", "--affected", "--base=a", "--since=b"])).is_err());
    assert!(parse(&strings(&["test", "--affected", "--base="])).is_err());
}

#[test]
fn lint_scope_needs_affected_and_only_lint_and_check_take_it() {
    for command in ["lint", "check"] {
        let closure =
            parse(&strings(&[command, "--affected", "--lint-scope=closure"])).expect("closure");
        assert!(closure.lint_closure, "{command}");
        let direct =
            parse(&strings(&[command, "--affected", "--lint-scope", "direct"])).expect("direct");
        assert!(!direct.lint_closure, "{command}");
        let default = parse(&strings(&[command, "--affected"])).expect("default");
        assert!(!default.lint_closure, "{command}");
        assert_eq!(
            parse(&strings(&[command, "--lint-scope=closure"])),
            Err(ArgsError::LintScopeNeedsAffected)
        );
        assert!(parse(&strings(&[command, "--affected", "--lint-scope=all"])).is_err());
    }
    for command in ["test", "format", "typecheck"] {
        assert!(parse(&strings(&[command, "--affected", "--lint-scope=closure"])).is_err());
    }
}
