use std::path::{Path, PathBuf};

use clap::ValueEnum;

use super::super::{Command, WorkflowVerb};
use crate::plan::spec;

fn workspace_root() -> PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    Path::new(&root).join(workspace)
}

fn fixture(fixture_dir: &str) -> String {
    let path = workspace_root()
        .join("cli/cli/tests/fixtures")
        .join(fixture_dir)
        .join("pins.bzl");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn pin_list(pins: &str, name: &str) -> Vec<String> {
    let open = format!("{name} = [");
    let start = pins
        .find(&open)
        .unwrap_or_else(|| panic!("the fixture pins declare no {name}"))
        + open.len();
    let tail = &pins[start..];
    let end = tail
        .find(']')
        .unwrap_or_else(|| panic!("{name} opens a list it never closes"));
    tail[..end]
        .split(',')
        .filter_map(|entry| {
            entry
                .trim()
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .map(ToOwned::to_owned)
        })
        .collect()
}

fn pin_count(pins: &str, name: &str) -> usize {
    let needle = format!("{name} = ");
    let start = pins
        .find(&needle)
        .unwrap_or_else(|| panic!("the fixture pins declare no {name}"))
        + needle.len();
    let digits: String = pins[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("the fixture pins set {name} to no number"))
}

fn commands() -> Vec<&'static str> {
    Command::value_variants()
        .iter()
        .map(|command| command.name())
        .collect()
}

fn sorted(names: Vec<String>) -> Vec<String> {
    let mut names = names;
    names.sort();
    names
}

#[test]
fn command_count_pins_match_the_registry() {
    let count = commands().len();
    for (fixture_dir, name) in [
        ("strict_parsing", "STRICT_COMMAND_COUNT"),
        ("help_goldens", "HELP_COMMAND_COUNT"),
    ] {
        assert_eq!(
            pin_count(&fixture(fixture_dir), name),
            count,
            "{fixture_dir}/pins.bzl {name} must be the {count} commands dx parses"
        );
    }
}

#[test]
fn watchable_pins_match_the_watchable_set() {
    let pins = fixture("cli_execution_gaps");
    let watchable: Vec<String> = dx_adopt::watch::WATCHABLE_COMMANDS
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    assert_eq!(
        sorted(pin_list(&pins, "WATCHABLE_COMMANDS")),
        sorted(watchable.clone()),
        "the fixture pins must name every command dx watch wraps, and nothing else"
    );
    assert_eq!(
        pin_count(&pins, "WATCHABLE_COUNT"),
        watchable.len(),
        "WATCHABLE_COUNT must count the commands the fixture pins list"
    );
}

#[test]
fn not_watchable_pins_name_every_other_command() {
    let pins = fixture("cli_execution_gaps");
    let watchable: Vec<&str> = dx_adopt::watch::WATCHABLE_COMMANDS.to_vec();
    let mut documented = pin_list(&pins, "NOT_WATCHABLE_COMMANDS");
    for name in &documented {
        assert!(
            Command::parse(name).is_some(),
            "the fixture pins name {name}, which is not a dx command"
        );
        assert!(
            !watchable.contains(&name.as_str()),
            "the fixture pins call {name} not watchable, but dx watch wraps it"
        );
    }
    documented.sort();
    let wanted: Vec<String> = commands()
        .into_iter()
        .filter(|name| !watchable.contains(name))
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(
        documented,
        sorted(wanted.clone()),
        "the fixture pins must name every command dx watch refuses, and nothing else"
    );
    assert_eq!(
        pin_count(&pins, "NOT_WATCHABLE_COUNT"),
        wanted.len(),
        "NOT_WATCHABLE_COUNT must count the commands the fixture pins list"
    );
}

#[test]
fn report_pins_match_the_registry() {
    let pins = fixture("cli_execution_gaps");
    for (name, command) in [
        ("REPORT_LINT", Command::Lint),
        ("REPORT_TYPECHECK", Command::Typecheck),
        ("REPORT_TEST", Command::Test),
        ("REPORT_COVERAGE", Command::Coverage),
        ("REPORT_CHECK", Command::Check),
        ("REPORT_FIX", Command::Fix),
        ("REPORT_AUDIT", Command::License),
    ] {
        let mut wanted: Vec<String> = spec(command)
            .reports
            .iter()
            .map(|format| format.name().to_owned())
            .collect();
        wanted.sort();
        assert_eq!(
            sorted(pin_list(&pins, name)),
            wanted,
            "{name} must name every --report format dx {} accepts, and nothing else",
            command.name()
        );
    }
    let mut without: Vec<String> = Command::value_variants()
        .iter()
        .filter(|command| spec(**command).reports.is_empty())
        .map(|command| (*command.name()).to_owned())
        .collect();
    without.sort();
    assert_eq!(
        sorted(pin_list(&pins, "REPORT_NONE")),
        without,
        "REPORT_NONE must name every command that rejects --report, and nothing else"
    );
}

#[test]
fn profile_pins_match_the_profile_commands() {
    let pins = fixture("build_profiles");
    let mut wanted: Vec<String> = Command::value_variants()
        .iter()
        .filter(|command| command.supports_profile())
        .map(|command| (*command.name()).to_owned())
        .collect();
    wanted.sort();
    assert_eq!(
        sorted(pin_list(&pins, "PROFILE_COMMANDS")),
        wanted,
        "the fixture pins must name every command that takes --debug or --release, and nothing else"
    );
    assert_eq!(
        pin_list(&pins, "PROFILE_FLAGS"),
        vec!["--debug".to_owned(), "--release".to_owned()],
        "the fixture pins must name the two profile flags dx takes"
    );
}

#[test]
fn workflow_verbs_match_the_registry() {
    let mut verbs: Vec<String> = Command::value_variants()
        .iter()
        .filter_map(|command| command.workflow_verb().map(WorkflowVerb::name))
        .map(ToOwned::to_owned)
        .collect();
    verbs.sort();
    assert_eq!(
        verbs,
        vec![
            "build".to_owned(),
            "coverage".to_owned(),
            "run".to_owned(),
            "test".to_owned()
        ],
        "only the four workflow verbs carry a build profile"
    );
}

#[test]
fn pinned_counts_match_the_lists_they_count() {
    let cases: Vec<(&str, &str, &str)> = vec![
        (
            "cli_execution_gaps",
            "WATCHABLE_COMMANDS",
            "WATCHABLE_COUNT",
        ),
        (
            "cli_execution_gaps",
            "NOT_WATCHABLE_COMMANDS",
            "NOT_WATCHABLE_COUNT",
        ),
    ];
    for (fixture_dir, list, count) in cases {
        let pins = fixture(fixture_dir);
        assert_eq!(
            pin_count(&pins, count),
            pin_list(&pins, list).len(),
            "{fixture_dir}/pins.bzl {count} must count the {list} it pins"
        );
    }
}
