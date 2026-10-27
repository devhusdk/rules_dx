use super::super::{ArgsError, Command};
use super::parse;
use dx_testing::cli::strings;

const AFFECTED: [Command; 11] = [
    Command::Build,
    Command::Test,
    Command::Coverage,
    Command::ListTests,
    Command::Lint,
    Command::Typecheck,
    Command::Format,
    Command::Check,
    Command::Fix,
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
