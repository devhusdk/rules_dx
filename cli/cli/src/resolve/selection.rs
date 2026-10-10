use dx_process::ForwardError;

use crate::args::Profile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionContext {
    configured: bool,
    profile: Option<Profile>,
    options: Vec<String>,
    startup: Vec<String>,
}

fn is_dx_profile_config(value: &str) -> bool {
    [Profile::Debug, Profile::Dev, Profile::Release]
        .iter()
        .any(|profile| profile.config() == value)
}

fn filter_configs(
    options: &[String],
    profile: Option<Profile>,
) -> Result<Vec<String>, ForwardError> {
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
                return Err(ForwardError::ConflictingOption {
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

fn selection_options(options: &[String]) -> Vec<String> {
    options
        .iter()
        .filter(|arg| !dx_process::is_test_binary_arg(arg))
        .cloned()
        .collect()
}

impl SelectionContext {
    pub fn unconfigured(startup: &[String]) -> Self {
        SelectionContext {
            configured: false,
            profile: None,
            options: Vec::new(),
            startup: startup.to_vec(),
        }
    }

    pub fn unconfigured_with_options(bazel_options: &[String], startup: &[String]) -> Self {
        SelectionContext {
            configured: false,
            profile: None,
            options: selection_options(bazel_options),
            startup: startup.to_vec(),
        }
    }

    pub fn for_tests_configured(bazel_options: &[String], startup: &[String]) -> Self {
        SelectionContext {
            configured: true,
            profile: None,
            options: selection_options(bazel_options),
            startup: startup.to_vec(),
        }
    }

    pub fn for_test_execution(
        profile: Profile,
        bazel_options: &[String],
        startup: &[String],
    ) -> Self {
        SelectionContext {
            configured: true,
            profile: Some(profile),
            options: selection_options(bazel_options),
            startup: startup.to_vec(),
        }
    }

    pub fn configured(&self) -> bool {
        self.configured
    }

    pub fn query_verb(&self) -> &'static str {
        if self.configured {
            "cquery"
        } else {
            "query"
        }
    }

    pub fn provenance(&self) -> &'static str {
        if self.configured {
            "configured"
        } else {
            "unconfigured"
        }
    }

    pub fn filtered_options(&self) -> Result<Vec<String>, ForwardError> {
        filter_configs(&self.options, self.profile)
    }

    pub fn selection_argv(
        &self,
        verb: &str,
        required: &[String],
        expr: &str,
    ) -> Result<Vec<String>, ForwardError> {
        let mut full_required = Vec::new();
        if let Some(profile) = self.profile {
            full_required.push(profile.config_flag());
        }
        full_required.extend(required.iter().cloned());
        let filtered = self.filtered_options()?;
        let mut argv = dx_process::build_workflow_argv(
            verb,
            &filtered,
            &full_required,
            &[],
            &[],
            &self.startup,
        )?;
        argv.push("--".to_owned());
        argv.push(expr.to_owned());
        Ok(argv)
    }

    pub fn redacted_detail(&self) -> String {
        let mut parts = Vec::new();
        if let Some(profile) = self.profile {
            parts.push(profile.config_flag());
        }
        let filtered = self.filtered_options().unwrap_or_default();
        let (mut safe, withheld) = crate::plan::quality::partition_provenance_options(&filtered);
        safe.extend(withheld);
        parts.extend(safe);
        if parts.is_empty() {
            self.provenance().to_owned()
        } else {
            format!("{} ({})", self.provenance(), parts.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::strings;

    #[test]
    fn unconfigured_selection_uses_query_without_options() {
        let selection = SelectionContext::unconfigured(&[]);
        assert!(!selection.configured());
        assert_eq!(selection.query_verb(), "query");
        assert_eq!(selection.provenance(), "unconfigured");
        let argv = selection
            .selection_argv("query", &[], "kind('rule', //...)")
            .expect("argv");
        assert_eq!(
            argv,
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "query",
                "--",
                "kind('rule', //...)",
            ])
        );
        assert_eq!(selection.redacted_detail(), "unconfigured");
    }

    #[test]
    fn configured_tests_selection_uses_cquery_with_options() {
        let selection = SelectionContext::for_tests_configured(
            &strings(&["--platforms=//:x", "--config=ci"]),
            &strings(&["--output_base=/tmp/a"]),
        );
        assert!(selection.configured());
        assert_eq!(selection.query_verb(), "cquery");
        let argv = selection
            .selection_argv(
                "cquery",
                &["--output=label_kind".to_owned()],
                "tests(//...)",
            )
            .expect("argv");
        assert_eq!(argv[4], "cquery");
        assert!(argv.contains(&"--platforms=//:x".to_owned()));
        assert!(argv.contains(&"--config=ci".to_owned()));
        assert!(argv.contains(&"--output_base=/tmp/a".to_owned()));
        assert!(argv.contains(&"--output=label_kind".to_owned()));
        let detail = selection.redacted_detail();
        assert!(detail.contains("configured"));
        assert!(detail.contains("--platforms=//:x"));
    }

    #[test]
    fn test_execution_selection_pins_profile_config() {
        let selection =
            SelectionContext::for_test_execution(Profile::Dev, &strings(&["--config=ci"]), &[]);
        let argv = selection
            .selection_argv("cquery", &[], "tests(//...)")
            .expect("argv");
        let dev = argv
            .iter()
            .position(|arg| arg == "--config=dx_dev")
            .expect("profile reaches selection");
        let ci = argv
            .iter()
            .position(|arg| arg == "--config=ci")
            .expect("consumer config reaches selection");
        assert!(dev < ci);
    }

    #[test]
    fn selection_drops_test_binary_args() {
        let selection = SelectionContext::for_tests_configured(
            &strings(&["--test_arg=--exact", "--config=ci"]),
            &[],
        );
        let argv = selection
            .selection_argv("cquery", &[], "tests(//...)")
            .expect("argv");
        assert!(!argv.iter().any(|arg| arg.contains("test_arg")));
        assert!(argv.contains(&"--config=ci".to_owned()));
    }

    #[test]
    fn selection_rejects_conflicting_profile_config() {
        let selection = SelectionContext::for_test_execution(
            Profile::Dev,
            &strings(&["--config=dx_release"]),
            &[],
        );
        let error = selection
            .selection_argv("cquery", &[], "tests(//...)")
            .expect_err("conflict");
        assert!(matches!(error, ForwardError::ConflictingOption { .. }));
    }

    #[test]
    fn selection_redacts_secrets_from_detail() {
        let selection =
            SelectionContext::for_tests_configured(&strings(&["--token=abc", "--config=ci"]), &[]);
        let detail = selection.redacted_detail();
        assert!(!detail.contains("abc"));
        assert!(detail.contains("--token"));
        assert!(detail.contains("--config=ci"));
        let argv = selection
            .selection_argv("cquery", &[], "tests(//...)")
            .expect("argv");
        assert!(argv.contains(&"--token=abc".to_owned()));
    }

    #[test]
    fn startup_options_reach_selection_argv() {
        let selection =
            SelectionContext::for_tests_configured(&[], &strings(&["--output_base=/tmp/a"]));
        let argv = selection
            .selection_argv("cquery", &[], "tests(//...)")
            .expect("argv");
        assert_eq!(
            argv[..5],
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "--output_base=/tmp/a",
                "cquery",
            ])
        );
    }
}
