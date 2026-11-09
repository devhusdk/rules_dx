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
        assert!(invocation.targets.is_empty());
    }
}

#[test]
fn affected_conflicts_with_scopes_and_here() {
    assert_eq!(
        parse(&strings(&["test", "--affected", "//a:b"])),
        Err(ArgsError::ConflictingAffected)
    );
    assert_eq!(
        parse(&strings(&["test", "//a:b", "--affected"])),
        Err(ArgsError::ConflictingAffected)
    );
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
