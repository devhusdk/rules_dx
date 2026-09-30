use super::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Debug,
    Dev,
    Release,
}

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Profile::Debug => "debug",
            Profile::Dev => "dev",
            Profile::Release => "release",
        }
    }

    pub fn config(self) -> &'static str {
        match self {
            Profile::Debug => "dx_debug",
            Profile::Dev => "dx_dev",
            Profile::Release => "dx_release",
        }
    }

    pub fn config_flag(self) -> String {
        format!("--config={}", self.config())
    }

    pub fn default_for(command: Command) -> Self {
        if command.meta().default_release {
            Profile::Release
        } else {
            Profile::Dev
        }
    }

    pub fn parse_attr(value: &str) -> Option<Self> {
        match value {
            "debug" => Some(Profile::Debug),
            "dev" => Some(Profile::Dev),
            "release" => Some(Profile::Release),
            _ => None,
        }
    }
}

pub const DX_PROFILE_ENV: &str = "DX_PROFILE";

pub fn resolve_profile(flag: Option<Profile>, attr: Option<Profile>, default: Profile) -> Profile {
    flag.or(attr).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::super::{assert_usage, parse, ArgsError};
    use super::*;
    use clap::ValueEnum;

    #[test]
    fn profile_vocabulary_maps_to_shared_configs() {
        assert_eq!(Profile::Debug.name(), "debug");
        assert_eq!(Profile::Dev.name(), "dev");
        assert_eq!(Profile::Release.name(), "release");
        assert_eq!(Profile::Debug.config(), "dx_debug");
        assert_eq!(Profile::Dev.config(), "dx_dev");
        assert_eq!(Profile::Release.config(), "dx_release");
        assert_eq!(Profile::Debug.config_flag(), "--config=dx_debug");
        assert_eq!(Profile::Dev.config_flag(), "--config=dx_dev");
        assert_eq!(Profile::Release.config_flag(), "--config=dx_release");
        assert_eq!(DX_PROFILE_ENV, "DX_PROFILE");
    }

    #[test]
    fn profile_attr_parses_deploy_vocabulary() {
        assert_eq!(Profile::parse_attr("debug"), Some(Profile::Debug));
        assert_eq!(Profile::parse_attr("dev"), Some(Profile::Dev));
        assert_eq!(Profile::parse_attr("release"), Some(Profile::Release));
        assert_eq!(Profile::parse_attr("staging"), None);
        assert_eq!(Profile::parse_attr(""), None);
    }

    #[test]
    fn profile_precedence_is_flag_over_attr_over_default() {
        assert_eq!(
            resolve_profile(Some(Profile::Debug), Some(Profile::Release), Profile::Dev),
            Profile::Debug
        );
        assert_eq!(
            resolve_profile(None, Some(Profile::Release), Profile::Dev),
            Profile::Release
        );
        assert_eq!(resolve_profile(None, None, Profile::Dev), Profile::Dev);
        assert_eq!(
            resolve_profile(None, None, Profile::Release),
            Profile::Release
        );
    }

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn profile_flags_parse_on_build_run_test() {
        for command in ["build", "run", "test"] {
            let bare = parse(&args(&[command])).expect("bare parse");
            assert!(!bare.debug);
            assert!(!bare.release);
            assert_eq!(bare.profile_flag(), None);
            assert_eq!(bare.profile(), Profile::Dev);
            let debug = parse(&args(&[command, "--debug"])).expect("debug parse");
            assert!(debug.debug);
            assert!(!debug.release);
            assert_eq!(debug.profile_flag(), Some(Profile::Debug));
            assert_eq!(debug.profile(), Profile::Debug);
            let release = parse(&args(&[command, "--release"])).expect("release parse");
            assert!(!release.debug);
            assert!(release.release);
            assert_eq!(release.profile_flag(), Some(Profile::Release));
            assert_eq!(release.profile(), Profile::Release);
        }
        let bare = parse(&args(&["deploy"])).expect("bare deploy parse");
        assert_eq!(bare.profile_flag(), None);
        assert_eq!(bare.profile(), Profile::Release);
        let debug = parse(&args(&["deploy", "--debug"])).expect("deploy debug parse");
        assert_eq!(debug.profile_flag(), Some(Profile::Debug));
        assert_eq!(debug.profile(), Profile::Debug);
        let release = parse(&args(&["deploy", "--release"])).expect("deploy release parse");
        assert_eq!(release.profile_flag(), Some(Profile::Release));
        assert_eq!(release.profile(), Profile::Release);
        let got = parse(&args(&["--debug", "build"])).expect("parse");
        assert_eq!(got.profile_flag(), Some(Profile::Debug));
        let got = parse(&args(&["test", "--release", "//a:t"])).expect("parse");
        assert_eq!(got.profile_flag(), Some(Profile::Release));
    }

    #[test]
    fn profile_flags_reject_conflicts_and_foreign_commands() {
        for command in Command::value_variants() {
            let command = *command;
            if !command.supports_profile() {
                continue;
            }
            let words = [command.name(), "--debug", "--release"];
            assert_usage(&words, parse(&args(&words)).unwrap_err(), &["--debug"]);
        }
        for words in [
            vec!["coverage", "--debug"],
            vec!["coverage", "--release"],
            vec!["lint", "--debug"],
            vec!["check", "--release"],
            vec!["generate", "--debug"],
            vec!["codegen", "--release"],
            vec!["status", "--debug"],
            vec!["docs", "--debug"],
            vec!["docs", "--release"],
        ] {
            assert!(
                matches!(
                    parse(&args(&words)),
                    Err(ArgsError::UnsupportedOption { .. })
                ),
                "words: {words:?}"
            );
        }
    }
}
