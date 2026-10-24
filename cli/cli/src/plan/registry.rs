use crate::args::Command;
use crate::reports::StandardFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub command: Command,
    pub aspects: &'static [&'static str],
    pub reports: &'static [StandardFormat],
}

impl CommandSpec {
    pub fn accepts_report(&self, format: &str) -> bool {
        StandardFormat::parse(format).is_some_and(|parsed| self.reports.contains(&parsed))
    }
}

pub fn keep_sorted_support_state() -> &'static str {
    "qualified"
}

pub fn keep_sorted_cli_gap(aspects: &[&str]) -> Option<&'static str> {
    if aspects
        .iter()
        .any(|aspect| aspect.contains("real_text_lint"))
    {
        None
    } else {
        Some("support: keep_sorted lint is qualified but has no CLI lint aspect (missing real_text_lint_aspect)")
    }
}

pub fn spec(command: Command) -> CommandSpec {
    match command {
        Command::Lint => CommandSpec {
            command,
            aspects: &[
                "@rules_dx//quality:real_aspects.bzl%real_cc_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_dotnet_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_js_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_python_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_protobuf_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_powershell_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_jvm_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_ruby_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_rust_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_scala_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_shell_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_text_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_html_template_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_yaml_lint_aspect",
            ],
            reports: &[StandardFormat::Sarif],
        },
        Command::Typecheck => CommandSpec {
            command,
            aspects: &[
                "@rules_dx//quality:real_aspects.bzl%real_typecheck_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_rust_typecheck_aspect",
            ],
            reports: &[StandardFormat::Sarif],
        },
        Command::Format => CommandSpec {
            command,
            aspects: &[
                "@rules_dx//quality:real_aspects.bzl%real_cc_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_cue_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_dotnet_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_js_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_jsonnet_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_jvm_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_protobuf_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_ruby_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_rust_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_scala_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_terraform_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_html_template_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_yaml_format_aspect",
            ],
            reports: &[],
        },
        Command::Generate => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Build => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Failures],
        },
        Command::Test => CommandSpec {
            command,
            aspects: &[],
            reports: &[
                StandardFormat::Junit,
                StandardFormat::Failures,
                StandardFormat::Results,
            ],
        },
        Command::Coverage => CommandSpec {
            command,
            aspects: &[],
            reports: &[
                StandardFormat::Lcov,
                StandardFormat::Failures,
                StandardFormat::Results,
            ],
        },
        Command::Run => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Deploy => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Check => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Sarif],
        },
        Command::Fix => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Sarif],
        },
        Command::ListTests => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Clean => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Codegen | Command::Env => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Init
        | Command::New
        | Command::Upgrade
        | Command::Rollback
        | Command::Worktree
        | Command::Status
        | Command::Version
        | Command::Watch
        | Command::Completion => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Security => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Sarif],
        },
        Command::License => CommandSpec {
            command,
            aspects: &[],
            reports: &[StandardFormat::Sarif, StandardFormat::Spdx],
        },
        Command::Update => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Bump => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Migrate => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
        Command::Docs => CommandSpec {
            command,
            aspects: &[],
            reports: &[],
        },
    }
}

/// Every aspect label dx passes to Bazel.
pub fn all_aspect_labels() -> Vec<String> {
    use clap::ValueEnum;
    let mut labels: Vec<String> = Command::value_variants()
        .iter()
        .flat_map(|command| spec(*command).aspects.iter())
        .map(|aspect| (*aspect).to_owned())
        .collect();
    labels.push(dx_codegen::CODEGEN_ASPECT.to_owned());
    labels.push(dx_env_plan::ENV_ASPECT.to_owned());
    labels.sort();
    labels.dedup();
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_aspect_labels_cover_registry_and_managed_aspects() {
        let labels = all_aspect_labels();
        assert!(labels.contains(&"@rules_dx//quality:real_aspects.bzl%real_lint_aspect".to_owned()));
        assert!(labels
            .contains(&"@rules_dx//quality:real_aspects.bzl%real_typecheck_aspect".to_owned()));
        assert!(labels.contains(&dx_codegen::CODEGEN_ASPECT.to_owned()));
        assert!(labels.contains(&dx_env_plan::ENV_ASPECT.to_owned()));
        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(labels, sorted);
    }

    #[test]
    fn registry_entries_match_command_contract() {
        let lint = spec(Command::Lint);
        assert_eq!(
            lint.aspects,
            &[
                "@rules_dx//quality:real_aspects.bzl%real_cc_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_dotnet_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_js_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_python_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_protobuf_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_powershell_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_jvm_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_ruby_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_rust_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_scala_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_shell_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_text_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_html_template_lint_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_yaml_lint_aspect",
            ]
        );
        assert_eq!(lint.reports, &[StandardFormat::Sarif]);
        let typecheck = spec(Command::Typecheck);
        assert_eq!(
            typecheck.aspects,
            &[
                "@rules_dx//quality:real_aspects.bzl%real_typecheck_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_rust_typecheck_aspect",
            ]
        );
        assert_eq!(typecheck.reports, &[StandardFormat::Sarif]);
        let format = spec(Command::Format);
        assert_eq!(
            format.aspects,
            &[
                "@rules_dx//quality:real_aspects.bzl%real_cc_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_cue_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_dotnet_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_js_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_jsonnet_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_jvm_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_protobuf_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_ruby_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_rust_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_scala_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_terraform_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_html_template_format_aspect",
                "@rules_dx//quality:real_aspects.bzl%real_yaml_format_aspect",
            ]
        );
        assert!(format.reports.is_empty());
        let build = spec(Command::Build);
        assert!(build.aspects.is_empty());
        assert_eq!(build.reports, &[StandardFormat::Failures]);
        let test = spec(Command::Test);
        assert_eq!(
            test.reports,
            &[
                StandardFormat::Junit,
                StandardFormat::Failures,
                StandardFormat::Results
            ]
        );
        let coverage = spec(Command::Coverage);
        assert_eq!(
            coverage.reports,
            &[
                StandardFormat::Lcov,
                StandardFormat::Failures,
                StandardFormat::Results
            ]
        );
        let run = spec(Command::Run);
        assert!(run.aspects.is_empty());
        assert!(run.reports.is_empty());
        let generate = spec(Command::Generate);
        assert!(generate.aspects.is_empty());
        assert!(generate.reports.is_empty());
        let clean = spec(Command::Clean);
        assert!(clean.aspects.is_empty());
        assert!(clean.reports.is_empty());
        for command in [Command::Codegen, Command::Env] {
            let entry = spec(command);
            assert!(entry.aspects.is_empty());
            assert!(entry.reports.is_empty());
            assert!(command.is_managed());
        }
        for command in [Command::Security, Command::License] {
            let entry = spec(command);
            assert!(entry.aspects.is_empty());
            assert!(command.is_audit_update());
        }
        assert_eq!(spec(Command::Security).reports, &[StandardFormat::Sarif]);
        assert_eq!(
            spec(Command::License).reports,
            &[StandardFormat::Sarif, StandardFormat::Spdx]
        );
        let update = spec(Command::Update);
        assert!(update.aspects.is_empty());
        assert!(update.reports.is_empty());
        assert!(Command::Update.is_audit_update());
        let bump = spec(Command::Bump);
        assert!(bump.aspects.is_empty());
        assert!(bump.reports.is_empty());
        assert!(Command::Bump.is_audit_update());
        let migrate = spec(Command::Migrate);
        assert!(migrate.aspects.is_empty());
        assert!(migrate.reports.is_empty());
        assert!(!Command::Migrate.is_audit_update());
    }

    #[test]
    fn standard_format_names_and_parsing_are_inverses() {
        for format in [
            StandardFormat::Sarif,
            StandardFormat::Junit,
            StandardFormat::Lcov,
            StandardFormat::Spdx,
            StandardFormat::Failures,
            StandardFormat::Results,
        ] {
            assert_eq!(StandardFormat::parse(format.name()), Some(format));
        }
        for text in ["sraif", "SARIF", "Junit", "", "sarif "] {
            assert_eq!(StandardFormat::parse(text), None, "{text:?} must not parse");
        }
    }

    #[test]
    fn every_standard_format_reaches_at_least_one_command() {
        use clap::ValueEnum;
        for format in [
            StandardFormat::Sarif,
            StandardFormat::Junit,
            StandardFormat::Lcov,
            StandardFormat::Spdx,
            StandardFormat::Failures,
            StandardFormat::Results,
        ] {
            assert!(
                Command::value_variants()
                    .iter()
                    .any(|command| spec(*command).accepts_report(format.name())),
                "no command accepts the {format:?} format"
            );
        }
    }

    #[test]
    fn keep_sorted_cli_gap_is_explicit() {
        assert_eq!(keep_sorted_support_state(), "qualified");
        let lint = spec(Command::Lint);
        assert_eq!(keep_sorted_cli_gap(lint.aspects), None);
        assert!(
            lint.aspects
                .iter()
                .any(|aspect| aspect.contains("real_text_lint")),
            "lint wires the text aspect"
        );
        assert_eq!(
            keep_sorted_cli_gap(&["@rules_dx//quality:real_aspects.bzl%real_text_lint_aspect"]),
            None
        );
    }
}
