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

fn is_dx_profile_config(value: &str) -> bool {
    [Profile::Debug, Profile::Dev, Profile::Release]
        .iter()
        .any(|profile| profile.config() == value)
}

pub(crate) fn filter_profile_configs(
    options: &[String],
    profile: Option<Profile>,
) -> Result<Vec<String>, dx_process::ForwardError> {
    let Some(profile) = profile else {
        return Ok(options.to_vec());
    };
    let required = profile.config_flag();
    let mut kept = Vec::with_capacity(options.len());
    let mut index = 0;
    while index < options.len() {
        let arg = &options[index];
        if arg == &required {
            index += 1;
            continue;
        }
        let (value, consumed) = if let Some(value) = arg.strip_prefix("--config=") {
            (Some(value.to_owned()), 1)
        } else if arg == "--config" {
            match options.get(index + 1) {
                Some(value) => (Some(value.clone()), 2),
                None => (None, 1),
            }
        } else {
            kept.push(arg.clone());
            index += 1;
            continue;
        };
        if consumed == 2 && Some(profile.config()) == value.as_deref() {
            index += consumed;
            continue;
        }
        if let Some(value) = value {
            if is_dx_profile_config(&value) {
                return Err(dx_process::ForwardError::ConflictingOption {
                    flag: "config".to_owned(),
                });
            }
        }
        for offset in 0..consumed {
            kept.push(options[index + offset].clone());
        }
        index += consumed;
    }
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::super::{assert_usage, parse, ArgsError};
    use super::*;
    use clap::ValueEnum;
    use dx_testing::cli::strings;

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
    #[test]
    fn profile_flags_parse_on_build_run_test() {
        for command in ["build", "run", "test"] {
            let bare = parse(&strings(&[command])).expect("bare parse");
            assert!(!bare.debug);
            assert!(!bare.release);
            assert_eq!(bare.profile_flag(), None);
            assert_eq!(bare.profile(), Profile::Dev);
            let debug = parse(&strings(&[command, "--debug"])).expect("debug parse");
            assert!(debug.debug);
            assert!(!debug.release);
            assert_eq!(debug.profile_flag(), Some(Profile::Debug));
            assert_eq!(debug.profile(), Profile::Debug);
            let release = parse(&strings(&[command, "--release"])).expect("release parse");
            assert!(!release.debug);
            assert!(release.release);
            assert_eq!(release.profile_flag(), Some(Profile::Release));
            assert_eq!(release.profile(), Profile::Release);
        }
        let bare = parse(&strings(&["deploy"])).expect("bare deploy parse");
        assert_eq!(bare.profile_flag(), None);
        assert_eq!(bare.profile(), Profile::Release);
        let debug = parse(&strings(&["deploy", "--debug"])).expect("deploy debug parse");
        assert_eq!(debug.profile_flag(), Some(Profile::Debug));
        assert_eq!(debug.profile(), Profile::Debug);
        let release = parse(&strings(&["deploy", "--release"])).expect("deploy release parse");
        assert_eq!(release.profile_flag(), Some(Profile::Release));
        assert_eq!(release.profile(), Profile::Release);
        let got = parse(&strings(&["build", "--debug"])).expect("parse");
        assert_eq!(got.profile_flag(), Some(Profile::Debug));
        let got = parse(&strings(&["test", "--release", "//a:t"])).expect("parse");
        assert_eq!(got.profile_flag(), Some(Profile::Release));
    }

    #[test]
    fn profile_flags_reject_conflicts_and_foreign_commands() {
        for command in Command::value_variants() {
            let command = *command;
            if !crate::args::command::owners::PROFILE_OWNERS.contains(&command) {
                continue;
            }
            let words = [command.name(), "--debug", "--release"];
            assert_usage(&words, parse(&strings(&words)).unwrap_err(), &["--debug"]);
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
                    parse(&strings(&words)),
                    Err(ArgsError::UnsupportedOption { .. })
                ),
                "words: {words:?}"
            );
        }
    }

    #[test]
    fn filter_profile_configs_drops_the_selected_profile_and_rejects_others() {
        let options = strings(&["--config=dx_dev", "--config", "dx_dev", "--keep"]);
        let kept = filter_profile_configs(&options, Some(Profile::Dev)).expect("kept");
        assert_eq!(kept, strings(&["--keep"]));
        assert!(filter_profile_configs(&options, Some(Profile::Release)).is_err());
        assert_eq!(
            filter_profile_configs(&options, None).expect("all"),
            options
        );
    }
}
