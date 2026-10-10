use crate::args::{Command, Invocation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkewDisposition {
    Proceed,
    Warn,
    Refuse,
}

pub fn disposition(invocation: &Invocation, skewed: bool) -> SkewDisposition {
    if !skewed {
        return SkewDisposition::Proceed;
    }
    match governing_command(invocation).meta().skew {
        crate::args::command::SkewKind::Proceed => SkewDisposition::Proceed,
        crate::args::command::SkewKind::Warn => SkewDisposition::Warn,
        crate::args::command::SkewKind::Refuse => {
            if invocation.dry_run {
                SkewDisposition::Warn
            } else {
                SkewDisposition::Refuse
            }
        }
    }
}

fn governing_command(invocation: &Invocation) -> Command {
    if invocation.command != Command::Watch {
        return invocation.command;
    }
    invocation
        .targets
        .first()
        .and_then(|wrapped| Command::parse(wrapped))
        .unwrap_or(invocation.command)
}

pub fn diagnostic(pin: &str) -> String {
    format!(
        "version skew: binary {} pin {pin} module {}; fix with `dx version --pin {} --apply` or `dx version --rollback --apply`",
        dx_adopt::DX_VERSION,
        dx_adopt::MODULE_VERSION,
        dx_adopt::MODULE_VERSION,
    )
}

pub fn read_pin(workspace: &std::path::Path) -> String {
    dx_adopt::read_version_pin(workspace).unwrap_or_default()
}

pub fn is_skewed(pin: &str) -> bool {
    let pin = pin.trim();
    !pin.is_empty() && !dx_adopt::version_pin_matches_module(pin, dx_adopt::MODULE_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::command::SkewKind;
    use clap::ValueEnum;

    const SKEWED: bool = true;
    const CLEAN: bool = false;

    const PROCEED: &[Command] = &[
        Command::Version,
        Command::Status,
        Command::Completion,
        Command::Capabilities,
    ];

    const WARN: &[Command] = &[
        Command::Check,
        Command::Security,
        Command::License,
        Command::Owners,
        Command::Deps,
        Command::Why,
        Command::Verify,
        Command::Rerun,
        Command::Tests,
    ];

    const REFUSE: &[Command] = &[
        Command::Build,
        Command::Test,
        Command::Coverage,
        Command::Run,
        Command::Deploy,
        Command::Generate,
        Command::Fix,
        Command::Format,
        Command::Lint,
        Command::Typecheck,
        Command::Clean,
        Command::Update,
        Command::Bump,
        Command::Migrate,
        Command::New,
        Command::Upgrade,
        Command::Codegen,
        Command::Env,
        Command::Setup,
        Command::Init,
        Command::Hooks,
        Command::Watch,
        Command::Docs,
        Command::Bazel,
    ];

    fn stub(command: Command, dry_run: bool, wrapped: Option<&str>) -> Invocation {
        let mut invocation = crate::args::parse(&["status"]).expect("status");
        invocation.command = command;
        invocation.dry_run = dry_run;
        invocation.targets = match wrapped {
            Some(wrapped) => vec![wrapped.to_owned(), "//...".to_owned()],
            None => vec!["//...".to_owned()],
        };
        invocation
    }

    fn of(command: Command) -> Invocation {
        stub(command, false, None)
    }

    fn dry_run(command: Command) -> Invocation {
        stub(command, true, None)
    }

    fn watched(wrapped: &str) -> Invocation {
        stub(Command::Watch, false, Some(wrapped))
    }

    fn with_skew(kind: SkewKind) -> Vec<Command> {
        let mut found: Vec<Command> = Command::value_variants()
            .into_iter()
            .copied()
            .filter(|command| command.meta().skew == kind)
            .collect();
        found.sort_by_key(|command| command.name());
        found
    }

    fn sorted(list: &[Command]) -> Vec<Command> {
        let mut found = list.to_vec();
        found.sort_by_key(|command| command.name());
        found
    }

    #[test]
    fn clean_tree_proceeds_on_every_command() {
        for command in Command::value_variants() {
            assert_eq!(
                disposition(&of(*command), CLEAN),
                SkewDisposition::Proceed,
                "{command:?}"
            );
        }
    }

    #[test]
    fn repair_path_stays_usable_on_skew() {
        for command in PROCEED {
            assert_eq!(
                disposition(&of(*command), SKEWED),
                SkewDisposition::Proceed,
                "{command:?}"
            );
        }
    }

    #[test]
    fn read_only_commands_warn_on_skew() {
        for command in WARN {
            assert_eq!(
                disposition(&of(*command), SKEWED),
                SkewDisposition::Warn,
                "{command:?}"
            );
        }
    }

    #[test]
    fn mutating_commands_refuse_on_skew() {
        for command in REFUSE {
            assert_eq!(
                disposition(&of(*command), SKEWED),
                SkewDisposition::Refuse,
                "{command:?}"
            );
        }
    }

    #[test]
    fn the_three_lists_are_exactly_the_registry_skew_kinds() {
        assert_eq!(
            with_skew(SkewKind::Proceed),
            sorted(PROCEED),
            "the PROCEED list must be every command the registry marks Proceed"
        );
        assert_eq!(
            with_skew(SkewKind::Warn),
            sorted(WARN),
            "the WARN list must be every command the registry marks Warn"
        );
        assert_eq!(
            with_skew(SkewKind::Refuse),
            sorted(REFUSE),
            "the REFUSE list must be every command the registry marks Refuse"
        );
    }

    #[test]
    fn dry_run_downgrades_refusal_to_warning() {
        assert_eq!(
            disposition(&dry_run(Command::Build), SKEWED),
            SkewDisposition::Warn
        );
        assert_eq!(
            disposition(&dry_run(Command::Generate), SKEWED),
            SkewDisposition::Warn
        );
    }

    #[test]
    fn diagnostic_names_three_versions_and_fix() {
        let message = diagnostic("9.9.9");
        assert!(message.contains(dx_adopt::DX_VERSION), "{message}");
        assert!(message.contains("9.9.9"), "{message}");
        assert!(message.contains(dx_adopt::MODULE_VERSION), "{message}");
        assert!(message.contains("dx version --pin"), "{message}");
        assert!(message.contains("dx version --rollback"), "{message}");
    }

    #[test]
    fn missing_and_empty_pins_are_not_skew() {
        assert!(!is_skewed(""));
        assert!(!is_skewed("   "));
    }

    #[test]
    fn matching_pin_is_not_skew() {
        assert!(!is_skewed(dx_adopt::MODULE_VERSION));
    }

    #[test]
    fn mismatched_pin_is_skew() {
        assert!(is_skewed("9.9.9"));
    }

    #[test]
    fn read_pin_round_trips_through_workspace() {
        let scratch = dx_test_scratch::scratch("dx-skew-pin-");
        let dir = scratch.path().to_path_buf();
        let _ = std::fs::create_dir_all(dir.join(".dx"));
        std::fs::write(dir.join(".dx/version"), "9.9.9\n").expect("write pin");
        let pin = read_pin(&dir);
        assert_eq!(pin, "9.9.9");
        assert!(is_skewed(&pin));
    }

    #[test]
    fn read_pin_defaults_to_empty_off_workspace() {
        let scratch = dx_test_scratch::scratch("dx-skew-nopin-");
        let dir = scratch.path().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);
        assert_eq!(read_pin(&dir), "");
    }

    #[test]
    fn watch_answers_with_the_wrapped_commands_skew_kind() {
        for wrapped in dx_adopt::WATCHABLE_COMMANDS {
            let watched = disposition(&watched(wrapped), SKEWED);
            let bare = disposition(&of(Command::parse(wrapped).expect(wrapped)), SKEWED);
            assert_eq!(
                watched, bare,
                "watch {wrapped} must answer with {wrapped}'s own skew kind"
            );
        }
    }

    #[test]
    fn watch_check_warns_where_check_warns() {
        assert_eq!(
            disposition(&of(Command::Check), SKEWED),
            SkewDisposition::Warn
        );
        assert_eq!(
            disposition(&watched("check"), SKEWED),
            SkewDisposition::Warn,
            "watch must not refuse what the command it wraps does not"
        );
    }

    #[test]
    fn watch_build_still_refuses_on_skew() {
        assert_eq!(
            disposition(&watched("build"), SKEWED),
            SkewDisposition::Refuse
        );
    }

    #[test]
    fn watch_of_a_label_keeps_watchs_own_kind() {
        assert_eq!(
            disposition(&watched("//nope"), SKEWED),
            SkewDisposition::Refuse,
            "watch itself is mutating, so an unwrappable target still refuses"
        );
    }

    #[test]
    fn only_watch_defers_to_the_command_it_wraps() {
        for command in Command::value_variants() {
            if *command == Command::Watch {
                continue;
            }
            assert_eq!(
                disposition(&watched(command.name()), SKEWED),
                disposition(&of(*command), SKEWED),
                "watch {command:?} must answer for {command:?} itself"
            );
        }
    }
}
