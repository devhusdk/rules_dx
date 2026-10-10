use super::ResolveError;
use crate::args::Profile;

/// How a resolved selection relates to the Bazel configuration it runs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionProvenance {
    Configured,
    Unconfigured,
}

impl SelectionProvenance {
    /// Short stable name used in plans, events, and user documentation.
    pub fn name(self) -> &'static str {
        match self {
            SelectionProvenance::Configured => "configured",
            SelectionProvenance::Unconfigured => "unconfigured",
        }
    }
}

/// The Bazel configuration a workflow selection resolves under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionContext {
    startup_options: Vec<String>,
    config_options: Vec<String>,
    provenance: SelectionProvenance,
}

impl SelectionContext {
    /// Ownership and inspection selections that never evaluate a configuration.
    pub fn unconfigured(startup_options: &[String]) -> Self {
        SelectionContext {
            startup_options: startup_options.to_vec(),
            config_options: Vec::new(),
            provenance: SelectionProvenance::Unconfigured,
        }
    }

    /// Runnable selections sharing the run execution configuration.
    pub fn for_run(profile: Profile, startup_options: &[String]) -> Self {
        SelectionContext {
            startup_options: startup_options.to_vec(),
            config_options: vec![profile.config_flag()],
            provenance: SelectionProvenance::Configured,
        }
    }

    /// Startup options shared by selection and execution invocations.
    pub fn startup_options(&self) -> &[String] {
        &self.startup_options
    }

    /// Execution configuration options the selection query evaluates under.
    pub fn config_options(&self) -> &[String] {
        &self.config_options
    }

    /// Whether the selection evaluated a configuration.
    pub fn provenance(&self) -> SelectionProvenance {
        self.provenance
    }

    /// One-line disclosure of how the selection was resolved.
    pub fn describe(&self) -> String {
        match self.provenance {
            SelectionProvenance::Configured => format!(
                "configured (cquery under {})",
                self.config_options.join(" ")
            ),
            SelectionProvenance::Unconfigured => "unconfigured (query)".to_owned(),
        }
    }

    /// Bazel argv running a discovery expression under the execution configuration.
    pub fn cquery_argv(&self, expression: &str) -> Vec<String> {
        let mut argv = dx_process::startup_argv(&self.startup_options);
        argv.push("cquery".to_owned());
        argv.extend(self.config_options.iter().cloned());
        argv.push("--".to_owned());
        argv.push(expression.to_owned());
        argv
    }
}

/// Parse configured query output, dropping the per-target configuration suffix.
pub(crate) fn parse_configured_labels(
    stdout: &[u8],
    label: &str,
) -> Result<Vec<String>, ResolveError> {
    let text = std::str::from_utf8(stdout).map_err(|_| ResolveError::QueryFailed {
        label: label.to_owned(),
        detail: "query output is not UTF-8".to_owned(),
    })?;
    let mut labels: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| match line.split_once(" (") {
            Some((head, _)) => head.to_owned(),
            None => line.to_owned(),
        })
        .collect();
    labels.sort();
    labels.dedup();
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::strings;

    #[test]
    fn run_context_carries_the_execution_config_flag() {
        for (profile, flag) in [
            (Profile::Debug, "--config=dx_debug"),
            (Profile::Dev, "--config=dx_dev"),
            (Profile::Release, "--config=dx_release"),
        ] {
            let selection = SelectionContext::for_run(profile, &[]);
            assert_eq!(selection.provenance(), SelectionProvenance::Configured);
            assert_eq!(selection.config_options(), &[flag.to_owned()]);
            assert_eq!(
                selection.cquery_argv("kind('rule', //...)"),
                strings(&[
                    "bazel",
                    "--nohome_rc",
                    "--nosystem_rc",
                    "cquery",
                    flag,
                    "--",
                    "kind('rule', //...)",
                ])
            );
            assert!(selection.describe().contains(flag), "{}", selection.describe());
        }
    }

    #[test]
    fn run_context_keeps_startup_options_before_the_verb() {
        let startup = vec!["--output_base=/tmp/a".to_owned()];
        let selection = SelectionContext::for_run(Profile::Dev, &startup);
        assert_eq!(
            selection.cquery_argv("//app:bin"),
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "--output_base=/tmp/a",
                "cquery",
                "--config=dx_dev",
                "--",
                "//app:bin",
            ])
        );
    }

    #[test]
    fn unconfigured_context_queries_without_configuration() {
        let selection = SelectionContext::unconfigured(&["--output_base=/tmp/a".to_owned()]);
        assert_eq!(selection.provenance(), SelectionProvenance::Unconfigured);
        assert!(selection.config_options().is_empty());
        assert_eq!(selection.describe(), "unconfigured (query)");
        assert_eq!(
            selection.startup_options(),
            &["--output_base=/tmp/a".to_owned()]
        );
    }

    #[test]
    fn provenance_names_are_stable() {
        assert_eq!(SelectionProvenance::Configured.name(), "configured");
        assert_eq!(SelectionProvenance::Unconfigured.name(), "unconfigured");
    }

    #[test]
    fn configured_output_drops_the_configuration_suffix() {
        let parsed = parse_configured_labels(
            b"//app:two (62a8b64)\n//app:one (532c333)\n//app:one (9f1d2aa)\n",
            "owners",
        )
        .expect("parse");
        assert_eq!(parsed, strings(&["//app:one", "//app:two"]));
    }

    #[test]
    fn configured_output_keeps_plain_labels() {
        let parsed = parse_configured_labels(b"//app:bin\n\n  \n", "owners").expect("parse");
        assert_eq!(parsed, strings(&["//app:bin"]));
    }

    #[test]
    fn configured_output_rejects_non_utf8() {
        let err = parse_configured_labels(&[0xff, 0xfe], "owners").expect_err("non-utf8");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: "owners".to_owned(),
                detail: "query output is not UTF-8".to_owned(),
            }
        );
    }
}
