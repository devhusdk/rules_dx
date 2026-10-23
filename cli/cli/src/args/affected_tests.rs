use super::super::{ArgsError, Command};
use super::parse;
use crate::test_support::strings;

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
        assert!(command.supports_affected());
    }
}

#[test]
fn affected_is_rejected_by_generate_and_docs() {
    for name in ["generate", "docs"] {
        assert!(parse(&strings(&[name, "--affected"])).is_err(), "{name}");
    }
    assert!(!Command::Generate.supports_affected());
    assert!(!Command::Docs.supports_affected());
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
