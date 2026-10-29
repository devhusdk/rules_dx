use super::rules_dx_label;
use crate::args::Command;
use crate::reports::StandardFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub command: Command,
    pub aspects: &'static [&'static str],
    /// The cheap aspect that names the shards each target needs; see `select_aspects`.
    pub probe: Option<&'static str>,
    pub reports: &'static [StandardFormat],
}

impl CommandSpec {
    pub fn accepts_report(&self, format: &str) -> bool {
        StandardFormat::parse(format).is_some_and(|parsed| self.reports.contains(&parsed))
    }
}

pub fn spec(command: Command) -> CommandSpec {
    match command {
        Command::Lint => CommandSpec {
            command,
            aspects: &[
                "quality:real_aspects.bzl%real_cc_lint_aspect",
                "quality:real_aspects.bzl%real_dockerfile_lint_aspect",
                "quality:real_aspects.bzl%real_make_lint_aspect",
                "quality:real_aspects.bzl%real_cmake_lint_aspect",
                "quality:real_aspects.bzl%real_terraform_lint_aspect",
                "quality:real_aspects.bzl%real_dotnet_lint_aspect",
                "quality:real_aspects.bzl%real_capnp_lint_aspect",
                "quality:real_aspects.bzl%real_flatbuffers_lint_aspect",
                "quality:real_aspects.bzl%real_thrift_lint_aspect",
                "quality:real_aspects.bzl%real_go_lint_aspect",
                "quality:real_aspects.bzl%real_lint_aspect",
                "quality:real_aspects.bzl%real_js_lint_aspect",
                "quality:real_aspects.bzl%real_python_lint_aspect",
                "quality:real_aspects.bzl%real_protobuf_lint_aspect",
                "quality:real_aspects.bzl%real_powershell_lint_aspect",
                "quality:real_aspects.bzl%real_jvm_lint_aspect",
                "quality:real_aspects.bzl%real_ruby_lint_aspect",
                "quality:real_aspects.bzl%real_rust_lint_aspect",
                "quality:real_aspects.bzl%real_scala_lint_aspect",
                "quality:real_aspects.bzl%real_shell_lint_aspect",
                "quality:real_aspects.bzl%real_text_lint_aspect",
                "quality:real_aspects.bzl%real_html_template_lint_aspect",
                "quality:real_aspects.bzl%real_yaml_lint_aspect",
                "quality:real_aspects.bzl%real_nix_lint_aspect",
                "quality:real_aspects.bzl%real_lua_lint_aspect",
                "quality:real_aspects.bzl%real_ocaml_lint_aspect",
                "quality:real_aspects.bzl%real_kubernetes_lint_aspect",
                "quality:real_aspects.bzl%real_ansible_lint_aspect",
                "quality:real_aspects.bzl%real_cloudformation_lint_aspect",
                "quality:real_aspects.bzl%real_bicep_lint_aspect",
                "quality:real_aspects.bzl%real_sql_lint_aspect",
                "quality:real_aspects.bzl%real_rego_lint_aspect",
                "quality:real_aspects.bzl%real_jsonnet_lint_aspect",
                "quality:real_aspects.bzl%real_fortran_lint_aspect",
                "quality:real_aspects.bzl%real_xml_lint_aspect",
                "quality:real_aspects.bzl%real_haskell_lint_aspect",
            ],
            probe: Some("quality:real_aspects.bzl%real_lint_probe_aspect"),
            reports: &[StandardFormat::Sarif],
        },
        Command::Typecheck => CommandSpec {
            command,
            aspects: &[
                "quality:real_aspects.bzl%real_typecheck_aspect",
                "quality:real_aspects.bzl%real_go_typecheck_aspect",
                "quality:real_aspects.bzl%real_js_typecheck_aspect",
                "quality:real_aspects.bzl%real_jvm_typecheck_aspect",
                "quality:real_aspects.bzl%real_d_typecheck_aspect",
                "quality:real_aspects.bzl%real_fortran_typecheck_aspect",
                "quality:real_aspects.bzl%real_zig_typecheck_aspect",
                "quality:real_aspects.bzl%real_ocaml_typecheck_aspect",
                "quality:real_aspects.bzl%real_rust_typecheck_aspect",
                "quality:real_aspects.bzl%real_dotnet_typecheck_aspect",
                "quality:real_aspects.bzl%real_rego_typecheck_aspect",
                "quality:real_aspects.bzl%real_capnp_typecheck_aspect",
                "quality:real_aspects.bzl%real_flatbuffers_typecheck_aspect",
                "quality:real_aspects.bzl%real_thrift_typecheck_aspect",
                "quality:real_aspects.bzl%real_cue_typecheck_aspect",
                "quality:real_aspects.bzl%real_cc_typecheck_aspect",
                "quality:real_aspects.bzl%real_pkl_typecheck_aspect",
                "quality:real_aspects.bzl%real_terraform_typecheck_aspect",
            ],
            probe: Some("quality:real_aspects.bzl%real_typecheck_probe_aspect"),
            reports: &[StandardFormat::Sarif],
        },
        Command::Format => CommandSpec {
            command,
            aspects: &[
                "quality:real_aspects.bzl%real_cc_format_aspect",
                "quality:real_aspects.bzl%real_cue_format_aspect",
                "quality:real_aspects.bzl%real_dotnet_format_aspect",
                "quality:real_aspects.bzl%real_format_aspect",
                "quality:real_aspects.bzl%real_fortran_format_aspect",
                "quality:real_aspects.bzl%real_go_format_aspect",
                "quality:real_aspects.bzl%real_haskell_format_aspect",
                "quality:real_aspects.bzl%real_js_format_aspect",
                "quality:real_aspects.bzl%real_jsonnet_format_aspect",
                "quality:real_aspects.bzl%real_jvm_format_aspect",
                "quality:real_aspects.bzl%real_lua_format_aspect",
                "quality:real_aspects.bzl%real_bicep_format_aspect",
                "quality:real_aspects.bzl%real_cmake_format_aspect",
                "quality:real_aspects.bzl%real_make_format_aspect",
                "quality:real_aspects.bzl%real_nix_format_aspect",
                "quality:real_aspects.bzl%real_ocaml_format_aspect",
                "quality:real_aspects.bzl%real_pkl_format_aspect",
                "quality:real_aspects.bzl%real_protobuf_format_aspect",
                "quality:real_aspects.bzl%real_powershell_format_aspect",
                "quality:real_aspects.bzl%real_rego_format_aspect",
                "quality:real_aspects.bzl%real_ruby_format_aspect",
                "quality:real_aspects.bzl%real_rust_format_aspect",
                "quality:real_aspects.bzl%real_scala_format_aspect",
                "quality:real_aspects.bzl%real_shell_format_aspect",
                "quality:real_aspects.bzl%real_terraform_format_aspect",
                "quality:real_aspects.bzl%real_typst_format_aspect",
                "quality:real_aspects.bzl%real_latex_format_aspect",
                "quality:real_aspects.bzl%real_html_template_format_aspect",
                "quality:real_aspects.bzl%real_sql_format_aspect",
                "quality:real_aspects.bzl%real_yaml_format_aspect",
            ],
            probe: Some("quality:real_aspects.bzl%real_format_probe_aspect"),
            reports: &[],
        },
        Command::Generate => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Build => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[StandardFormat::Failures],
        },
        Command::Test => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[
                StandardFormat::Junit,
                StandardFormat::Failures,
                StandardFormat::Results,
            ],
        },
        Command::Coverage => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[
                StandardFormat::Lcov,
                StandardFormat::Sources,
                StandardFormat::Failures,
                StandardFormat::Results,
            ],
        },
        Command::Run => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Deploy => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Check => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[StandardFormat::Sarif],
        },
        Command::Fix => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[StandardFormat::Sarif],
        },
        Command::Clean => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Codegen | Command::Env => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Init
        | Command::Upgrade
        | Command::Rollback
        | Command::Worktree
        | Command::Explain
        | Command::Install
        | Command::Status
        | Command::Version
        | Command::Watch
        | Command::Completion => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Security => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[StandardFormat::Sarif],
        },
        Command::License => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[
                StandardFormat::Sarif,
                StandardFormat::Spdx,
                StandardFormat::Notice,
            ],
        },
        Command::Update => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Bump | Command::Add => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Migrate => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Docs => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
    }
}

/// Every aspect label dx passes to Bazel.
pub fn all_aspect_labels() -> Vec<String> {
    use clap::ValueEnum;
    let mut labels: Vec<String> = Command::value_variants()
        .iter()
        .flat_map(|command| {
            let entry = spec(*command);
            entry.aspects.iter().copied().chain(entry.probe)
        })
        .map(rules_dx_label)
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
        for probe in ["lint", "format", "typecheck"] {
            assert!(labels.contains(&format!(
                "@rules_dx//quality:real_aspects.bzl%real_{probe}_probe_aspect"
            )));
        }
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
    fn registry_aspects_are_relative_to_rules_dx() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let entry = spec(*command);
            for aspect in entry.aspects.iter().copied().chain(entry.probe) {
                assert!(!aspect.starts_with('@'), "{command:?}: {aspect}");
            }
        }
    }

    #[test]
    fn registry_entries_match_command_contract() {
        let lint = spec(Command::Lint);
        assert_eq!(
            lint.aspects,
            &[
                "quality:real_aspects.bzl%real_cc_lint_aspect",
                "quality:real_aspects.bzl%real_dockerfile_lint_aspect",
                "quality:real_aspects.bzl%real_make_lint_aspect",
                "quality:real_aspects.bzl%real_cmake_lint_aspect",
                "quality:real_aspects.bzl%real_terraform_lint_aspect",
                "quality:real_aspects.bzl%real_dotnet_lint_aspect",
                "quality:real_aspects.bzl%real_capnp_lint_aspect",
                "quality:real_aspects.bzl%real_flatbuffers_lint_aspect",
                "quality:real_aspects.bzl%real_thrift_lint_aspect",
                "quality:real_aspects.bzl%real_go_lint_aspect",
                "quality:real_aspects.bzl%real_lint_aspect",
                "quality:real_aspects.bzl%real_js_lint_aspect",
                "quality:real_aspects.bzl%real_python_lint_aspect",
                "quality:real_aspects.bzl%real_protobuf_lint_aspect",
                "quality:real_aspects.bzl%real_powershell_lint_aspect",
                "quality:real_aspects.bzl%real_jvm_lint_aspect",
                "quality:real_aspects.bzl%real_ruby_lint_aspect",
                "quality:real_aspects.bzl%real_rust_lint_aspect",
                "quality:real_aspects.bzl%real_scala_lint_aspect",
                "quality:real_aspects.bzl%real_shell_lint_aspect",
                "quality:real_aspects.bzl%real_text_lint_aspect",
                "quality:real_aspects.bzl%real_html_template_lint_aspect",
                "quality:real_aspects.bzl%real_yaml_lint_aspect",
                "quality:real_aspects.bzl%real_nix_lint_aspect",
                "quality:real_aspects.bzl%real_lua_lint_aspect",
                "quality:real_aspects.bzl%real_ocaml_lint_aspect",
                "quality:real_aspects.bzl%real_kubernetes_lint_aspect",
                "quality:real_aspects.bzl%real_ansible_lint_aspect",
                "quality:real_aspects.bzl%real_cloudformation_lint_aspect",
                "quality:real_aspects.bzl%real_bicep_lint_aspect",
                "quality:real_aspects.bzl%real_sql_lint_aspect",
                "quality:real_aspects.bzl%real_rego_lint_aspect",
                "quality:real_aspects.bzl%real_jsonnet_lint_aspect",
                "quality:real_aspects.bzl%real_fortran_lint_aspect",
                "quality:real_aspects.bzl%real_xml_lint_aspect",
                "quality:real_aspects.bzl%real_haskell_lint_aspect",
            ]
        );
        assert_eq!(lint.reports, &[StandardFormat::Sarif]);
        let typecheck = spec(Command::Typecheck);
        assert_eq!(
            typecheck.aspects,
            &[
                "quality:real_aspects.bzl%real_typecheck_aspect",
                "quality:real_aspects.bzl%real_go_typecheck_aspect",
                "quality:real_aspects.bzl%real_js_typecheck_aspect",
                "quality:real_aspects.bzl%real_jvm_typecheck_aspect",
                "quality:real_aspects.bzl%real_d_typecheck_aspect",
                "quality:real_aspects.bzl%real_fortran_typecheck_aspect",
                "quality:real_aspects.bzl%real_zig_typecheck_aspect",
                "quality:real_aspects.bzl%real_ocaml_typecheck_aspect",
                "quality:real_aspects.bzl%real_rust_typecheck_aspect",
                "quality:real_aspects.bzl%real_dotnet_typecheck_aspect",
                "quality:real_aspects.bzl%real_rego_typecheck_aspect",
                "quality:real_aspects.bzl%real_capnp_typecheck_aspect",
                "quality:real_aspects.bzl%real_flatbuffers_typecheck_aspect",
                "quality:real_aspects.bzl%real_thrift_typecheck_aspect",
                "quality:real_aspects.bzl%real_cue_typecheck_aspect",
                "quality:real_aspects.bzl%real_cc_typecheck_aspect",
                "quality:real_aspects.bzl%real_pkl_typecheck_aspect",
                "quality:real_aspects.bzl%real_terraform_typecheck_aspect",
            ]
        );
        assert_eq!(typecheck.reports, &[StandardFormat::Sarif]);
        let format = spec(Command::Format);
        assert_eq!(
            format.aspects,
            &[
                "quality:real_aspects.bzl%real_cc_format_aspect",
                "quality:real_aspects.bzl%real_cue_format_aspect",
                "quality:real_aspects.bzl%real_dotnet_format_aspect",
                "quality:real_aspects.bzl%real_format_aspect",
                "quality:real_aspects.bzl%real_fortran_format_aspect",
                "quality:real_aspects.bzl%real_go_format_aspect",
                "quality:real_aspects.bzl%real_haskell_format_aspect",
                "quality:real_aspects.bzl%real_js_format_aspect",
                "quality:real_aspects.bzl%real_jsonnet_format_aspect",
                "quality:real_aspects.bzl%real_jvm_format_aspect",
                "quality:real_aspects.bzl%real_lua_format_aspect",
                "quality:real_aspects.bzl%real_bicep_format_aspect",
                "quality:real_aspects.bzl%real_cmake_format_aspect",
                "quality:real_aspects.bzl%real_make_format_aspect",
                "quality:real_aspects.bzl%real_nix_format_aspect",
                "quality:real_aspects.bzl%real_ocaml_format_aspect",
                "quality:real_aspects.bzl%real_pkl_format_aspect",
                "quality:real_aspects.bzl%real_protobuf_format_aspect",
                "quality:real_aspects.bzl%real_powershell_format_aspect",
                "quality:real_aspects.bzl%real_rego_format_aspect",
                "quality:real_aspects.bzl%real_ruby_format_aspect",
                "quality:real_aspects.bzl%real_rust_format_aspect",
                "quality:real_aspects.bzl%real_scala_format_aspect",
                "quality:real_aspects.bzl%real_shell_format_aspect",
                "quality:real_aspects.bzl%real_terraform_format_aspect",
                "quality:real_aspects.bzl%real_typst_format_aspect",
                "quality:real_aspects.bzl%real_latex_format_aspect",
                "quality:real_aspects.bzl%real_html_template_format_aspect",
                "quality:real_aspects.bzl%real_sql_format_aspect",
                "quality:real_aspects.bzl%real_yaml_format_aspect",
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
                StandardFormat::Sources,
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
        }
        assert_eq!(spec(Command::Security).reports, &[StandardFormat::Sarif]);
        assert_eq!(
            spec(Command::License).reports,
            &[
                StandardFormat::Sarif,
                StandardFormat::Spdx,
                StandardFormat::Notice
            ]
        );
        let update = spec(Command::Update);
        assert!(update.aspects.is_empty());
        assert!(update.reports.is_empty());
        for command in [Command::Bump, Command::Add] {
            let spec = spec(command);
            assert!(spec.aspects.is_empty(), "{command:?}");
            assert!(spec.reports.is_empty(), "{command:?}");
        }
        let migrate = spec(Command::Migrate);
        assert!(migrate.aspects.is_empty());
        assert!(migrate.reports.is_empty());
    }

    #[test]
    fn standard_format_names_and_parsing_are_inverses() {
        for format in [
            StandardFormat::Sarif,
            StandardFormat::Junit,
            StandardFormat::Lcov,
            StandardFormat::Sources,
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
            StandardFormat::Sources,
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
}
