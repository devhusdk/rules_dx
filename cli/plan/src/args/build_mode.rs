use super::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    Debug,
    Dev,
    Release,
}

impl BuildMode {
    pub fn name(self) -> &'static str {
        match self {
            BuildMode::Debug => "debug",
            BuildMode::Dev => "dev",
            BuildMode::Release => "release",
        }
    }

    pub fn config(self) -> &'static str {
        match self {
            BuildMode::Debug => "dx_build_debug",
            BuildMode::Dev => "dx_build_dev",
            BuildMode::Release => "dx_build_release",
        }
    }

    pub fn config_flag(self) -> String {
        format!("--config={}", self.config())
    }

    pub fn default_for(command: Command) -> Self {
        if command.meta().default_release {
            BuildMode::Release
        } else {
            BuildMode::Dev
        }
    }

    pub fn parse_attr(value: &str) -> Option<Self> {
        match value {
            "debug" => Some(BuildMode::Debug),
            "dev" => Some(BuildMode::Dev),
            "release" => Some(BuildMode::Release),
            _ => None,
        }
    }
}

pub const DX_BUILD_MODE_ENV: &str = "DX_BUILD_MODE";

pub fn resolve_build_mode(
    flag: Option<BuildMode>,
    attr: Option<BuildMode>,
    default: BuildMode,
) -> BuildMode {
    flag.or(attr).unwrap_or(default)
}

fn is_dx_build_mode_config(value: &str) -> bool {
    [BuildMode::Debug, BuildMode::Dev, BuildMode::Release]
        .iter()
        .any(|build_mode| build_mode.config() == value)
}

pub(crate) fn filter_build_mode_configs(
    options: &[String],
    build_mode: Option<BuildMode>,
) -> Result<Vec<String>, dx_error::DxError> {
    let Some(build_mode) = build_mode else {
        return Ok(options.to_vec());
    };
    let required = build_mode.config_flag();
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
        if consumed == 2 && Some(build_mode.config()) == value.as_deref() {
            index += consumed;
            continue;
        }
        if let Some(value) = value
            && is_dx_build_mode_config(&value)
        {
            return Err(dx_process::conflicting_option("config".to_owned()));
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
    use super::super::{ArgsError, assert_usage, parse};
    use super::*;
    use clap::ValueEnum;
    use dx_testing::cli::strings;

    #[test]
    fn build_mode_vocabulary_maps_to_shared_configs() {
        assert_eq!(BuildMode::Debug.name(), "debug");
        assert_eq!(BuildMode::Dev.name(), "dev");
        assert_eq!(BuildMode::Release.name(), "release");
        assert_eq!(BuildMode::Debug.config(), "dx_build_debug");
        assert_eq!(BuildMode::Dev.config(), "dx_build_dev");
        assert_eq!(BuildMode::Release.config(), "dx_build_release");
        assert_eq!(BuildMode::Debug.config_flag(), "--config=dx_build_debug");
        assert_eq!(BuildMode::Dev.config_flag(), "--config=dx_build_dev");
        assert_eq!(
            BuildMode::Release.config_flag(),
            "--config=dx_build_release"
        );
        assert_eq!(DX_BUILD_MODE_ENV, "DX_BUILD_MODE");
    }

    #[test]
    fn build_mode_attr_parses_deploy_vocabulary() {
        assert_eq!(BuildMode::parse_attr("debug"), Some(BuildMode::Debug));
        assert_eq!(BuildMode::parse_attr("dev"), Some(BuildMode::Dev));
        assert_eq!(BuildMode::parse_attr("release"), Some(BuildMode::Release));
        assert_eq!(BuildMode::parse_attr("staging"), None);
        assert_eq!(BuildMode::parse_attr(""), None);
    }

    #[test]
    fn build_mode_precedence_is_flag_over_attr_over_default() {
        assert_eq!(
            resolve_build_mode(
                Some(BuildMode::Debug),
                Some(BuildMode::Release),
                BuildMode::Dev
            ),
            BuildMode::Debug
        );
        assert_eq!(
            resolve_build_mode(None, Some(BuildMode::Release), BuildMode::Dev),
            BuildMode::Release
        );
        assert_eq!(
            resolve_build_mode(None, None, BuildMode::Dev),
            BuildMode::Dev
        );
        assert_eq!(
            resolve_build_mode(None, None, BuildMode::Release),
            BuildMode::Release
        );
    }
    #[test]
    fn build_mode_flags_parse_on_build_run_test() {
        for command in ["build", "run", "test"] {
            let bare = parse(&strings(&[command])).expect("bare parse");
            assert!(!bare.debug);
            assert!(!bare.release);
            assert_eq!(bare.build_mode_flag(), None);
            assert_eq!(bare.build_mode(), BuildMode::Dev);
            let debug = parse(&strings(&[command, "--debug"])).expect("debug parse");
            assert!(debug.debug);
            assert!(!debug.release);
            assert_eq!(debug.build_mode_flag(), Some(BuildMode::Debug));
            assert_eq!(debug.build_mode(), BuildMode::Debug);
            let release = parse(&strings(&[command, "--release"])).expect("release parse");
            assert!(!release.debug);
            assert!(release.release);
            assert_eq!(release.build_mode_flag(), Some(BuildMode::Release));
            assert_eq!(release.build_mode(), BuildMode::Release);
        }
        let bare = parse(&strings(&["deploy"])).expect("bare deploy parse");
        assert_eq!(bare.build_mode_flag(), None);
        assert_eq!(bare.build_mode(), BuildMode::Release);
        let debug = parse(&strings(&["deploy", "--debug"])).expect("deploy debug parse");
        assert_eq!(debug.build_mode_flag(), Some(BuildMode::Debug));
        assert_eq!(debug.build_mode(), BuildMode::Debug);
        let release = parse(&strings(&["deploy", "--release"])).expect("deploy release parse");
        assert_eq!(release.build_mode_flag(), Some(BuildMode::Release));
        assert_eq!(release.build_mode(), BuildMode::Release);
        let got = parse(&strings(&["build", "--debug"])).expect("parse");
        assert_eq!(got.build_mode_flag(), Some(BuildMode::Debug));
        let got = parse(&strings(&["test", "--release", "//a:t"])).expect("parse");
        assert_eq!(got.build_mode_flag(), Some(BuildMode::Release));
    }

    #[test]
    fn build_mode_flags_reject_conflicts_and_foreign_commands() {
        for command in Command::value_variants() {
            let command = *command;
            if !crate::args::command::owners::BUILD_MODE_OWNERS.contains(&command) {
                continue;
            }
            let words = [command.name(), "--debug", "--release"];
            assert_usage(words, parse(&strings(&words)).unwrap_err(), &["--debug"]);
        }
        for words in [
            vec!["coverage", "--debug"],
            vec!["coverage", "--release"],
            vec!["lint", "--debug"],
            vec!["check", "--release"],
            vec!["generate", "--debug"],
            vec!["sync", "--release"],
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
    fn filter_build_mode_configs_drops_the_selected_build_mode_and_rejects_others() {
        let options = strings(&[
            "--config=dx_build_dev",
            "--config",
            "dx_build_dev",
            "--keep",
        ]);
        let kept = filter_build_mode_configs(&options, Some(BuildMode::Dev)).expect("kept");
        assert_eq!(kept, strings(&["--keep"]));
        assert!(filter_build_mode_configs(&options, Some(BuildMode::Release)).is_err());
        assert_eq!(
            filter_build_mode_configs(&options, None).expect("all"),
            options
        );
    }
}
