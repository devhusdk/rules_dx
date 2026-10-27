use std::path::Path;

use dx_process::ForwardError;

use crate::args::Profile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionContext {
    configured: bool,
    profile: Option<Profile>,
    options: Vec<String>,
    startup: Vec<String>,
}

fn selection_options(options: &[String]) -> Vec<String> {
    let mut kept = Vec::with_capacity(options.len());
    let mut index = 0;
    while index < options.len() {
        let arg = &options[index];
        if dx_process::is_test_binary_arg(arg) {
            index += if arg.contains('=') { 1 } else { 2 };
        } else {
            kept.push(arg.clone());
            index += 1;
        }
    }
    kept
}

/// Options `bazel query` accepts that change which packages and repositories load.
/// `--config` is left out on purpose: Bazel expands `--config=X` for `query`
/// only from `always`, `common` and `query` rc sections, so a `build:`-only
/// config would make the query fail.
const QUERY_OPTIONS: &[&str] = &[
    "override_repository",
    "override_module",
    "repo_env",
    "registry",
    "lockfile_mode",
    "vendor_dir",
    "distdir",
    "repository_cache",
    "repo_contents_cache",
    "downloader_config",
    "check_direct_dependencies",
    "ignore_dev_dependency",
    "allow_yanked_versions",
    "enable_platform_specific_config",
    "check_bazel_compatibility",
    "keep_going",
    "loading_phase_threads",
    "deleted_packages",
    "package_path",
];

fn query_option_name(arg: &str) -> Option<&'static str> {
    let name = dx_process::flag_name(arg)?;
    let bare = name.strip_prefix("no").unwrap_or(&name);
    QUERY_OPTIONS
        .iter()
        .copied()
        .find(|allowed| *allowed == name || *allowed == bare)
}

fn query_options(options: &[String]) -> Vec<String> {
    let mut kept = Vec::with_capacity(options.len());
    let mut index = 0;
    while index < options.len() {
        let arg = &options[index];
        let consumed = match options.get(index + 1) {
            Some(next) if !arg.contains('=') && !next.starts_with('-') => 2,
            _ => 1,
        };
        if query_option_name(arg).is_some() {
            kept.extend(options[index..index + consumed].iter().cloned());
        }
        index += consumed;
    }
    kept
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

    pub fn unconfigured_view(&self) -> SelectionContext {
        SelectionContext {
            configured: false,
            ..self.clone()
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
        crate::args::filter_profile_configs(&self.options, self.profile)
    }

    #[cfg(test)]
    pub(crate) fn startup(&self) -> &[String] {
        &self.startup
    }

    fn verb_argv(&self, verb: &str, required: &[String]) -> Result<Vec<String>, ForwardError> {
        let filtered = self.filtered_options()?;
        let options = if verb == "query" {
            query_options(&filtered)
        } else {
            crate::plan::profile_options(self.profile, &filtered)
        };
        crate::plan::bazel_argv(verb, &options, required, &[], &[], &self.startup)
    }

    pub fn selection_argv(
        &self,
        verb: &str,
        required: &[String],
        expr: &str,
    ) -> Result<Vec<String>, ForwardError> {
        let mut argv = self.verb_argv(verb, required)?;
        argv.push("--".to_owned());
        argv.push(expr.to_owned());
        Ok(argv)
    }

    pub(crate) fn selection_query_file_argv(
        &self,
        verb: &str,
        query_file: &Path,
    ) -> Result<Vec<String>, ForwardError> {
        let mut argv = self.verb_argv(verb, &[])?;
        argv.push(format!("--query_file={}", query_file.display()));
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
    use dx_testing::cli::strings;

    #[test]
    fn unconfigured_selection_uses_query_without_options() {
        let selection = SelectionContext::unconfigured(&[]);
        assert!(!selection.configured());
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
    fn query_file_argv_ends_with_the_file_instead_of_an_expression() {
        let file = Path::new("/tmp/q.txt");
        let unconfigured = SelectionContext::unconfigured(&strings(&["--output_base=/o"]));
        assert_eq!(
            unconfigured
                .selection_query_file_argv("query", file)
                .expect("argv"),
            strings(&[
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "--output_base=/o",
                "query",
                "--query_file=/tmp/q.txt",
            ])
        );
        let configured = SelectionContext::for_test_execution(
            Profile::Dev,
            &strings(&["--test_arg=--exact", "--config=ci"]),
            &[],
        );
        let argv = configured
            .selection_query_file_argv("cquery", file)
            .expect("argv");
        assert_eq!(argv[3], "cquery");
        let dev = argv
            .iter()
            .position(|arg| arg == "--config=dx_dev")
            .expect("profile config");
        let ci = argv
            .iter()
            .position(|arg| arg == "--config=ci")
            .expect("consumer config");
        assert!(dev < ci);
        assert!(!argv.iter().any(|arg| arg.contains("test_arg")), "{argv:?}");
        assert!(!argv.contains(&"--".to_owned()), "{argv:?}");
        assert_eq!(
            argv.last().map(String::as_str),
            Some("--query_file=/tmp/q.txt")
        );
    }

    #[test]
    fn configured_tests_selection_uses_cquery_with_options() {
        let selection = SelectionContext::for_tests_configured(
            &strings(&["--platforms=//:x", "--config=ci"]),
            &strings(&["--output_base=/tmp/a"]),
        );
        assert!(selection.configured());
        let argv = selection
            .selection_argv(
                "cquery",
                &["--output=label_kind".to_owned()],
                "tests(//...)",
            )
            .expect("argv");
        assert_eq!(argv[4], "cquery");
        assert_eq!(argv[5..7], strings(&["--platforms=//:x", "--config=ci"]));
        assert_eq!(argv[7..9], crate::plan::BAZEL_SETTINGS);
        assert_eq!(argv[9], "--output=label_kind");
        assert!(argv.contains(&"--output_base=/tmp/a".to_owned()));
        let detail = selection.redacted_detail();
        assert!(detail.contains("configured"));
        assert!(detail.contains("--platforms=//:x"));
    }

    #[test]
    fn selection_drops_separated_and_equals_test_args() {
        let options = strings(&[
            "--test_arg",
            "--exact",
            "--config=ci",
            "--test_arg=--nocapture",
            "--test_arg",
            "",
            "--test_arg",
            "--test_arg",
            "--platforms=//:x",
            "--test_arg",
        ]);
        let selection = SelectionContext::for_tests_configured(&options, &[]);
        assert_eq!(
            selection.filtered_options().expect("options"),
            strings(&["--config=ci", "--platforms=//:x"])
        );
        let unconfigured = SelectionContext::unconfigured_with_options(&options, &[]);
        assert_eq!(
            unconfigured.filtered_options().expect("options"),
            strings(&["--config=ci", "--platforms=//:x"])
        );
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
    fn selection_redacts_headers_env_values_and_url_credentials() {
        let selection = SelectionContext::for_tests_configured(
            &strings(&[
                "--remote_header=Authorization=Bearer hunter2",
                "--bes_header",
                "x-api=hunter2",
                "--action_env=API_KEY=hunter2",
                "--test_env",
                "DB=hunter2",
                "--repo_env=HOME",
                "--remote_cache=grpcs://ci:hunter2@cache.example",
                "--define=API_TOKEN=hunter2",
                "--//pkg:token=hunter2",
                "--config=ci",
            ]),
            &[],
        );
        let detail = selection.redacted_detail();
        assert!(!detail.contains("hunter2"), "{detail}");
        for kept in [
            "--remote_header",
            "--bes_header",
            "--action_env=API_KEY=(value)",
            "--test_env DB=(value)",
            "--repo_env=HOME",
            "--remote_cache=grpcs://(credentials)@cache.example",
            "--define=API_TOKEN=(value)",
            "--//pkg:token",
            "--config=ci",
        ] {
            assert!(detail.contains(kept), "{kept}: {detail}");
        }
    }

    fn query_test_options() -> Vec<String> {
        strings(&[
            "--config=ci",
            "--override_repository=a=/p",
            "--config",
            "ci",
            "--test_filter=x",
            "--repo_env=K=V",
            "--test_filter",
            "x",
            "--test_tag_filters=a",
            "--keep_going",
            "--platforms=//:x",
            "--override_module",
            "m=/p",
            "--jobs=2",
            "--nobuild_tests_only",
            "--config=dx_dev",
        ])
    }

    #[test]
    fn query_verb_keeps_only_repository_options() {
        let selection =
            SelectionContext::for_test_execution(Profile::Dev, &query_test_options(), &[])
                .unconfigured_view();
        let expected = strings(&[
            "--override_repository=a=/p",
            "--repo_env=K=V",
            "--keep_going",
            "--override_module",
            "m=/p",
        ]);
        let argv = selection
            .selection_argv("query", &[], "tests(//...)")
            .expect("argv");
        assert_eq!(argv[3], "query");
        assert_eq!(argv[4..9], expected, "{argv:?}");
        for dropped in [
            "--config=ci",
            "--config",
            "--config=dx_dev",
            "--test_filter=x",
            "--test_filter",
            "--test_tag_filters=a",
            "--platforms=//:x",
            "--jobs=2",
            "--nobuild_tests_only",
            "ci",
            "x",
        ] {
            assert!(!argv.contains(&dropped.to_owned()), "{dropped}: {argv:?}");
        }
        let file_argv = selection
            .selection_query_file_argv("query", Path::new("/tmp/q.txt"))
            .expect("argv");
        assert_eq!(file_argv[4..9], expected, "{file_argv:?}");
        assert!(!file_argv.contains(&"--config=dx_dev".to_owned()));
    }

    #[test]
    fn cquery_verb_keeps_every_option_and_the_profile() {
        let options = query_test_options();
        let selection =
            SelectionContext::for_test_execution(Profile::Dev, &options[..options.len() - 1], &[]);
        let argv = selection
            .selection_argv("cquery", &[], "tests(//...)")
            .expect("argv");
        assert!(argv.contains(&"--config=dx_dev".to_owned()), "{argv:?}");
        for kept in &options[..options.len() - 1] {
            assert!(argv.contains(kept), "{kept}: {argv:?}");
        }
    }

    #[test]
    fn query_verb_still_rejects_a_conflicting_profile() {
        let selection = SelectionContext::for_test_execution(
            Profile::Dev,
            &strings(&["--config=dx_release"]),
            &[],
        )
        .unconfigured_view();
        let error = selection
            .selection_argv("query", &[], "tests(//...)")
            .expect_err("conflict");
        assert!(matches!(error, ForwardError::ConflictingOption { .. }));
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
