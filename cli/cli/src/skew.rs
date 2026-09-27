use crate::args::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkewDisposition {
    Proceed,
    Warn,
    Refuse,
}

pub fn disposition(command: Command, dry_run: bool, skewed: bool) -> SkewDisposition {
    if !skewed {
        return SkewDisposition::Proceed;
    }
    match command.meta().skew {
        crate::args::command::SkewKind::Proceed => SkewDisposition::Proceed,
        crate::args::command::SkewKind::Warn => SkewDisposition::Warn,
        crate::args::command::SkewKind::Refuse => {
            if dry_run {
                SkewDisposition::Warn
            } else {
                SkewDisposition::Refuse
            }
        }
    }
}

pub fn diagnostic(pin: &str) -> String {
    format!(
        "version skew: binary {} pin {pin} module {}; fix with `dx version --pin {}` or `dx version --rollback`",
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

    const SKEWED: bool = true;
    const CLEAN: bool = false;

    #[test]
    fn clean_tree_proceeds_on_every_command() {
        for command in [
            Command::Build,
            Command::Generate,
            Command::Update,
            Command::Bump,
            Command::Check,
            Command::Security,
            Command::License,
            Command::Owners,
            Command::Version,
            Command::Status,
            Command::Completion,
            Command::Bazel,
        ] {
            assert_eq!(
                disposition(command, false, CLEAN),
                SkewDisposition::Proceed,
                "{command:?}"
            );
        }
    }

    #[test]
    fn repair_path_stays_usable_on_skew() {
        for command in [Command::Version, Command::Status, Command::Completion] {
            assert_eq!(
                disposition(command, false, SKEWED),
                SkewDisposition::Proceed,
                "{command:?}"
            );
        }
    }

    #[test]
    fn read_only_commands_warn_on_skew() {
        for command in [
            Command::Check,
            Command::Security,
            Command::License,
            Command::Owners,
            Command::Deps,
            Command::Why,
        ] {
            assert_eq!(
                disposition(command, false, SKEWED),
                SkewDisposition::Warn,
                "{command:?}"
            );
        }
    }

    #[test]
    fn mutating_commands_refuse_on_skew() {
        for command in [
            Command::Build,
            Command::Test,
            Command::Coverage,
            Command::Run,
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
        ] {
            assert_eq!(
                disposition(command, false, SKEWED),
                SkewDisposition::Refuse,
                "{command:?}"
            );
        }
    }

    #[test]
    fn dry_run_downgrades_refusal_to_warning() {
        assert_eq!(
            disposition(Command::Build, true, SKEWED),
            SkewDisposition::Warn
        );
        assert_eq!(
            disposition(Command::Generate, true, SKEWED),
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
}
