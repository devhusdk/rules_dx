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
                "quality:families/cc.bzl%real_cc_lint_aspect",
                "quality:real_aspects.bzl%real_dockerfile_lint_aspect",
                "quality:real_aspects.bzl%real_make_lint_aspect",
                "quality:real_aspects.bzl%real_cmake_lint_aspect",
                "quality:real_aspects.bzl%real_terraform_lint_aspect",
                "quality:families/dotnet.bzl%real_dotnet_lint_aspect",
                "quality:families/generation.bzl%real_capnp_lint_aspect",
                "quality:families/generation.bzl%real_flatbuffers_lint_aspect",
                "quality:families/generation.bzl%real_thrift_lint_aspect",
                "quality:families/generation.bzl%real_avro_lint_aspect",
                "quality:real_aspects.bzl%real_go_lint_aspect",
                "quality:real_aspects.bzl%real_lint_aspect",
                "quality:families/js.bzl%real_js_lint_aspect",
                "quality:families/python.bzl%real_python_lint_aspect",
                "quality:families/generation.bzl%real_protobuf_lint_aspect",
                "quality:families/powershell.bzl%real_powershell_lint_aspect",
                "quality:families/jvm.bzl%real_jvm_lint_aspect",
                "quality:families/ruby.bzl%real_ruby_lint_aspect",
                "quality:families/rust.bzl%real_rust_lint_aspect",
                "quality:families/jvm.bzl%real_scala_lint_aspect",
                "quality:real_aspects.bzl%real_shell_lint_aspect",
                "quality:real_aspects.bzl%real_text_lint_aspect",
                "quality:real_aspects.bzl%real_html_template_lint_aspect",
                "quality:real_aspects.bzl%real_yaml_lint_aspect",
                "quality:real_aspects.bzl%real_nix_lint_aspect",
                "quality:families/lua.bzl%real_lua_lint_aspect",
                "quality:families/ocaml.bzl%real_ocaml_lint_aspect",
                "quality:real_aspects.bzl%real_kubernetes_lint_aspect",
                "quality:real_aspects.bzl%real_ansible_lint_aspect",
                "quality:real_aspects.bzl%real_cloudformation_lint_aspect",
                "quality:real_aspects.bzl%real_bicep_lint_aspect",
                "quality:real_aspects.bzl%real_sql_lint_aspect",
                "quality:real_aspects.bzl%real_rego_lint_aspect",
                "quality:real_aspects.bzl%real_jsonnet_lint_aspect",
                "quality:families/fortran.bzl%real_fortran_lint_aspect",
                "quality:real_aspects.bzl%real_typst_lint_aspect",
                "quality:real_aspects.bzl%real_xml_lint_aspect",
                "quality:families/haskell.bzl%real_haskell_lint_aspect",
                "quality:real_aspects.bzl%real_latex_lint_aspect",
                "quality:families/beam.bzl%real_erlang_lint_aspect",
                "quality:families/beam.bzl%real_elixir_lint_aspect",
                "quality:families/php.bzl%real_php_lint_aspect",
                "quality:families/dart.bzl%real_dart_lint_aspect",
                "quality:families/d.bzl%real_d_lint_aspect",
                "quality:families/perl.bzl%real_perl_lint_aspect",
                "quality:families/swift.bzl%real_swift_lint_aspect",
                "quality:families/zig.bzl%real_zig_lint_aspect",
                "quality:families/julia.bzl%real_julia_lint_aspect",
                "quality:real_aspects.bzl%real_pkl_lint_aspect",
            ],
            probe: Some("quality:real_aspects.bzl%real_lint_probe_aspect"),
            reports: &[StandardFormat::Sarif],
        },
        Command::Unused => CommandSpec {
            command,
            aspects: &[
                "quality:families/js.bzl%real_js_unused_aspect",
                "quality:families/python.bzl%real_python_unused_aspect",
                "quality:families/ruby.bzl%real_ruby_unused_aspect",
                "quality:families/beam.bzl%real_erlang_unused_aspect",
                "quality:families/beam.bzl%real_elixir_unused_aspect",
                "quality:families/cc.bzl%real_cc_unused_aspect",
                "quality:families/php.bzl%real_php_unused_aspect",
                "quality:families/go.bzl%real_go_unused_aspect",
            ],
            probe: None,
            reports: &[],
        },
        Command::Typecheck => CommandSpec {
            command,
            aspects: &[
                "quality:real_aspects.bzl%real_typecheck_aspect",
                "quality:real_aspects.bzl%real_go_typecheck_aspect",
                "quality:families/js.bzl%real_js_typecheck_aspect",
                "quality:families/jvm.bzl%real_jvm_typecheck_aspect",
                "quality:families/d.bzl%real_d_typecheck_aspect",
                "quality:families/fortran.bzl%real_fortran_typecheck_aspect",
                "quality:families/zig.bzl%real_zig_typecheck_aspect",
                "quality:families/ocaml.bzl%real_ocaml_typecheck_aspect",
                "quality:families/haskell.bzl%real_haskell_typecheck_aspect",
                "quality:families/rust.bzl%real_rust_typecheck_aspect",
                "quality:families/dotnet.bzl%real_dotnet_typecheck_aspect",
                "quality:real_aspects.bzl%real_rego_typecheck_aspect",
                "quality:families/generation.bzl%real_capnp_typecheck_aspect",
                "quality:families/generation.bzl%real_flatbuffers_typecheck_aspect",
                "quality:families/generation.bzl%real_thrift_typecheck_aspect",
                "quality:families/generation.bzl%real_avro_typecheck_aspect",
                "quality:real_aspects.bzl%real_cue_typecheck_aspect",
                "quality:families/cc.bzl%real_cc_typecheck_aspect",
                "quality:real_aspects.bzl%real_pkl_typecheck_aspect",
                "quality:real_aspects.bzl%real_terraform_typecheck_aspect",
                "quality:families/beam.bzl%real_erlang_typecheck_aspect",
                "quality:families/php.bzl%real_php_typecheck_aspect",
                "quality:families/julia.bzl%real_julia_typecheck_aspect",
                "quality:families/lua.bzl%real_lua_typecheck_aspect",
                "quality:families/ruby.bzl%real_ruby_typecheck_aspect",
                "quality:families/dart.bzl%real_dart_typecheck_aspect",
                "quality:families/python.bzl%real_python_typecheck_aspect",
            ],
            probe: Some("quality:real_aspects.bzl%real_typecheck_probe_aspect"),
            reports: &[StandardFormat::Sarif],
        },
        Command::Format => CommandSpec {
            command,
            aspects: &[
                "quality:families/cc.bzl%real_cc_format_aspect",
                "quality:real_aspects.bzl%real_cue_format_aspect",
                "quality:families/dotnet.bzl%real_dotnet_format_aspect",
                "quality:real_aspects.bzl%real_format_aspect",
                "quality:families/fortran.bzl%real_fortran_format_aspect",
                "quality:real_aspects.bzl%real_go_format_aspect",
                "quality:families/haskell.bzl%real_haskell_format_aspect",
                "quality:families/js.bzl%real_js_format_aspect",
                "quality:real_aspects.bzl%real_jsonnet_format_aspect",
                "quality:families/jvm.bzl%real_jvm_format_aspect",
                "quality:families/lua.bzl%real_lua_format_aspect",
                "quality:real_aspects.bzl%real_bicep_format_aspect",
                "quality:real_aspects.bzl%real_cmake_format_aspect",
                "quality:real_aspects.bzl%real_make_format_aspect",
                "quality:real_aspects.bzl%real_nix_format_aspect",
                "quality:families/ocaml.bzl%real_ocaml_format_aspect",
                "quality:real_aspects.bzl%real_pkl_format_aspect",
                "quality:families/generation.bzl%real_protobuf_format_aspect",
                "quality:families/powershell.bzl%real_powershell_format_aspect",
                "quality:real_aspects.bzl%real_rego_format_aspect",
                "quality:families/ruby.bzl%real_ruby_format_aspect",
                "quality:families/rust.bzl%real_rust_format_aspect",
                "quality:families/jvm.bzl%real_scala_format_aspect",
                "quality:real_aspects.bzl%real_shell_format_aspect",
                "quality:real_aspects.bzl%real_terraform_format_aspect",
                "quality:real_aspects.bzl%real_typst_format_aspect",
                "quality:real_aspects.bzl%real_latex_format_aspect",
                "quality:real_aspects.bzl%real_html_template_format_aspect",
                "quality:real_aspects.bzl%real_sql_format_aspect",
                "quality:real_aspects.bzl%real_yaml_format_aspect",
                "quality:families/beam.bzl%real_erlang_format_aspect",
                "quality:families/beam.bzl%real_elixir_format_aspect",
                "quality:families/dart.bzl%real_dart_format_aspect",
                "quality:families/d.bzl%real_d_format_aspect",
                "quality:families/perl.bzl%real_perl_format_aspect",
                "quality:families/python.bzl%real_python_format_aspect",
                "quality:families/julia.bzl%real_julia_format_aspect",
                "quality:families/swift.bzl%real_swift_format_aspect",
                "quality:families/zig.bzl%real_zig_format_aspect",
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
        Command::Clean => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Env => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Bench
        | Command::Init
        | Command::Upgrade
        | Command::Worktree
        | Command::Explain
        | Command::Fuzz
        | Command::Mutate
        | Command::Release
        | Command::Status
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
            reports: &[StandardFormat::Sarif],
        },
        Command::Update => CommandSpec {
            command,
            aspects: &[],
            probe: None,
            reports: &[],
        },
        Command::Docs => CommandSpec {
            command,
            aspects: &["quality:real_aspects.bzl%real_docs_aspect"],
            probe: None,
            reports: &[],
        },
    }
}

/// The aspects of `command` whose family module the consumer has. A family without a `module`
/// is always kept; the probe is not filtered.
pub fn aspects_for(command: Command, installed_modules: &[String]) -> Vec<&'static str> {
    aspects_for_families(command, dx_families::families(), installed_modules)
}

fn aspects_for_families(
    command: Command,
    families: &[dx_families::Family],
    installed_modules: &[String],
) -> Vec<&'static str> {
    let missing: Vec<&str> = families
        .iter()
        .filter(|family| {
            family
                .module
                .as_ref()
                .is_some_and(|module| !installed_modules.contains(module))
        })
        .flat_map(|family| family.aspects.iter().map(String::as_str))
        .collect();
    spec(command)
        .aspects
        .iter()
        .copied()
        .filter(|aspect| {
            let name = aspect.rsplit_once('%').map_or(*aspect, |(_, name)| name);
            !missing.contains(&name)
        })
        .collect()
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
    labels.push(dx_codegen::aspect_label(super::rules_dx_repo()));
    labels.push(dx_env_plan::aspect_label(super::rules_dx_repo()));
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
        assert!(labels.contains(&dx_codegen::aspect_label(super::super::rules_dx_repo())));
        assert!(labels.contains(&dx_env_plan::aspect_label(super::super::rules_dx_repo())));
        let mut sorted = labels.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(labels, sorted);
    }

    #[test]
    fn labels_point_at_the_file_that_defines_the_aspect() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let entry = spec(*command);
            for label in entry.aspects {
                let (file, name) = label.rsplit_once('%').expect("label has a name");
                let family = dx_families::families()
                    .iter()
                    .find(|family| family.aspects.iter().any(|aspect| aspect == name));
                let expected = match family {
                    Some(family) if family.name != "core" => {
                        format!("quality:families/{}.bzl", family.name)
                    }
                    _ => "quality:real_aspects.bzl".to_owned(),
                };
                assert_eq!(file, expected, "{label}");
            }
        }
    }

    fn families_json(python_module: &str) -> Vec<dx_families::Family> {
        let python_module = if python_module.is_empty() {
            String::new()
        } else {
            format!(r#""module": "{python_module}","#)
        };
        dx_families::parse(&format!(
            r#"{{"families": {{
                "python": {{{python_module} "aspects": ["real_python_lint_aspect"]}},
                "jvm": {{"module": "rules_dx_jvm", "aspects": ["real_jvm_lint_aspect"]}},
                "rust": {{"module": "rules_dx_rust", "aspects": ["real_rust_lint_aspect"]}},
                "core": {{"aspects": ["real_lint_aspect"]}}
            }}}}"#
        ))
        .expect("families")
    }

    #[test]
    fn aspects_for_drops_aspects_of_families_that_are_not_installed() {
        let installed = ["rules_dx_python".to_owned()];
        let lint =
            aspects_for_families(Command::Lint, &families_json("rules_dx_python"), &installed);
        let has = |name: &str| {
            lint.iter()
                .any(|aspect| aspect.ends_with(&format!("%{name}")))
        };
        assert!(has("real_lint_aspect"));
        assert!(has("real_python_lint_aspect"));
        assert!(has("real_cc_lint_aspect"));
        assert!(!has("real_jvm_lint_aspect"));
        assert!(!has("real_rust_lint_aspect"));
    }

    #[test]
    fn aspects_for_keeps_families_without_a_module() {
        let lint = aspects_for_families(Command::Lint, &families_json(""), &[]);
        assert!(lint.contains(&"quality:families/python.bzl%real_python_lint_aspect"));
        assert!(!lint.contains(&"quality:families/jvm.bzl%real_jvm_lint_aspect"));
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
                "quality:families/cc.bzl%real_cc_lint_aspect",
                "quality:real_aspects.bzl%real_dockerfile_lint_aspect",
                "quality:real_aspects.bzl%real_make_lint_aspect",
                "quality:real_aspects.bzl%real_cmake_lint_aspect",
                "quality:real_aspects.bzl%real_terraform_lint_aspect",
                "quality:families/dotnet.bzl%real_dotnet_lint_aspect",
                "quality:families/generation.bzl%real_capnp_lint_aspect",
                "quality:families/generation.bzl%real_flatbuffers_lint_aspect",
                "quality:families/generation.bzl%real_thrift_lint_aspect",
                "quality:families/generation.bzl%real_avro_lint_aspect",
                "quality:real_aspects.bzl%real_go_lint_aspect",
                "quality:real_aspects.bzl%real_lint_aspect",
                "quality:families/js.bzl%real_js_lint_aspect",
                "quality:families/python.bzl%real_python_lint_aspect",
                "quality:families/generation.bzl%real_protobuf_lint_aspect",
                "quality:families/powershell.bzl%real_powershell_lint_aspect",
                "quality:families/jvm.bzl%real_jvm_lint_aspect",
                "quality:families/ruby.bzl%real_ruby_lint_aspect",
                "quality:families/rust.bzl%real_rust_lint_aspect",
                "quality:families/jvm.bzl%real_scala_lint_aspect",
                "quality:real_aspects.bzl%real_shell_lint_aspect",
                "quality:real_aspects.bzl%real_text_lint_aspect",
                "quality:real_aspects.bzl%real_html_template_lint_aspect",
                "quality:real_aspects.bzl%real_yaml_lint_aspect",
                "quality:real_aspects.bzl%real_nix_lint_aspect",
                "quality:families/lua.bzl%real_lua_lint_aspect",
                "quality:families/ocaml.bzl%real_ocaml_lint_aspect",
                "quality:real_aspects.bzl%real_kubernetes_lint_aspect",
                "quality:real_aspects.bzl%real_ansible_lint_aspect",
                "quality:real_aspects.bzl%real_cloudformation_lint_aspect",
                "quality:real_aspects.bzl%real_bicep_lint_aspect",
                "quality:real_aspects.bzl%real_sql_lint_aspect",
                "quality:real_aspects.bzl%real_rego_lint_aspect",
                "quality:real_aspects.bzl%real_jsonnet_lint_aspect",
                "quality:families/fortran.bzl%real_fortran_lint_aspect",
                "quality:real_aspects.bzl%real_typst_lint_aspect",
                "quality:real_aspects.bzl%real_xml_lint_aspect",
                "quality:families/haskell.bzl%real_haskell_lint_aspect",
                "quality:real_aspects.bzl%real_latex_lint_aspect",
                "quality:families/beam.bzl%real_erlang_lint_aspect",
                "quality:families/beam.bzl%real_elixir_lint_aspect",
                "quality:families/php.bzl%real_php_lint_aspect",
                "quality:families/dart.bzl%real_dart_lint_aspect",
                "quality:families/d.bzl%real_d_lint_aspect",
                "quality:families/perl.bzl%real_perl_lint_aspect",
                "quality:families/swift.bzl%real_swift_lint_aspect",
                "quality:families/zig.bzl%real_zig_lint_aspect",
                "quality:families/julia.bzl%real_julia_lint_aspect",
                "quality:real_aspects.bzl%real_pkl_lint_aspect",
            ]
        );
        assert_eq!(lint.reports, &[StandardFormat::Sarif]);
        let typecheck = spec(Command::Typecheck);
        assert_eq!(
            typecheck.aspects,
            &[
                "quality:real_aspects.bzl%real_typecheck_aspect",
                "quality:real_aspects.bzl%real_go_typecheck_aspect",
                "quality:families/js.bzl%real_js_typecheck_aspect",
                "quality:families/jvm.bzl%real_jvm_typecheck_aspect",
                "quality:families/d.bzl%real_d_typecheck_aspect",
                "quality:families/fortran.bzl%real_fortran_typecheck_aspect",
                "quality:families/zig.bzl%real_zig_typecheck_aspect",
                "quality:families/ocaml.bzl%real_ocaml_typecheck_aspect",
                "quality:families/haskell.bzl%real_haskell_typecheck_aspect",
                "quality:families/rust.bzl%real_rust_typecheck_aspect",
                "quality:families/dotnet.bzl%real_dotnet_typecheck_aspect",
                "quality:real_aspects.bzl%real_rego_typecheck_aspect",
                "quality:families/generation.bzl%real_capnp_typecheck_aspect",
                "quality:families/generation.bzl%real_flatbuffers_typecheck_aspect",
                "quality:families/generation.bzl%real_thrift_typecheck_aspect",
                "quality:families/generation.bzl%real_avro_typecheck_aspect",
                "quality:real_aspects.bzl%real_cue_typecheck_aspect",
                "quality:families/cc.bzl%real_cc_typecheck_aspect",
                "quality:real_aspects.bzl%real_pkl_typecheck_aspect",
                "quality:real_aspects.bzl%real_terraform_typecheck_aspect",
                "quality:families/beam.bzl%real_erlang_typecheck_aspect",
                "quality:families/php.bzl%real_php_typecheck_aspect",
                "quality:families/julia.bzl%real_julia_typecheck_aspect",
                "quality:families/lua.bzl%real_lua_typecheck_aspect",
                "quality:families/ruby.bzl%real_ruby_typecheck_aspect",
                "quality:families/dart.bzl%real_dart_typecheck_aspect",
                "quality:families/python.bzl%real_python_typecheck_aspect",
            ]
        );
        assert_eq!(typecheck.reports, &[StandardFormat::Sarif]);
        let format = spec(Command::Format);
        assert_eq!(
            format.aspects,
            &[
                "quality:families/cc.bzl%real_cc_format_aspect",
                "quality:real_aspects.bzl%real_cue_format_aspect",
                "quality:families/dotnet.bzl%real_dotnet_format_aspect",
                "quality:real_aspects.bzl%real_format_aspect",
                "quality:families/fortran.bzl%real_fortran_format_aspect",
                "quality:real_aspects.bzl%real_go_format_aspect",
                "quality:families/haskell.bzl%real_haskell_format_aspect",
                "quality:families/js.bzl%real_js_format_aspect",
                "quality:real_aspects.bzl%real_jsonnet_format_aspect",
                "quality:families/jvm.bzl%real_jvm_format_aspect",
                "quality:families/lua.bzl%real_lua_format_aspect",
                "quality:real_aspects.bzl%real_bicep_format_aspect",
                "quality:real_aspects.bzl%real_cmake_format_aspect",
                "quality:real_aspects.bzl%real_make_format_aspect",
                "quality:real_aspects.bzl%real_nix_format_aspect",
                "quality:families/ocaml.bzl%real_ocaml_format_aspect",
                "quality:real_aspects.bzl%real_pkl_format_aspect",
                "quality:families/generation.bzl%real_protobuf_format_aspect",
                "quality:families/powershell.bzl%real_powershell_format_aspect",
                "quality:real_aspects.bzl%real_rego_format_aspect",
                "quality:families/ruby.bzl%real_ruby_format_aspect",
                "quality:families/rust.bzl%real_rust_format_aspect",
                "quality:families/jvm.bzl%real_scala_format_aspect",
                "quality:real_aspects.bzl%real_shell_format_aspect",
                "quality:real_aspects.bzl%real_terraform_format_aspect",
                "quality:real_aspects.bzl%real_typst_format_aspect",
                "quality:real_aspects.bzl%real_latex_format_aspect",
                "quality:real_aspects.bzl%real_html_template_format_aspect",
                "quality:real_aspects.bzl%real_sql_format_aspect",
                "quality:real_aspects.bzl%real_yaml_format_aspect",
                "quality:families/beam.bzl%real_erlang_format_aspect",
                "quality:families/beam.bzl%real_elixir_format_aspect",
                "quality:families/dart.bzl%real_dart_format_aspect",
                "quality:families/d.bzl%real_d_format_aspect",
                "quality:families/perl.bzl%real_perl_format_aspect",
                "quality:families/python.bzl%real_python_format_aspect",
                "quality:families/julia.bzl%real_julia_format_aspect",
                "quality:families/swift.bzl%real_swift_format_aspect",
                "quality:families/zig.bzl%real_zig_format_aspect",
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
        let docs = spec(Command::Docs);
        assert_eq!(docs.aspects, &["quality:real_aspects.bzl%real_docs_aspect"]);
        assert!(docs.probe.is_none());
        assert!(docs.reports.is_empty());
        let run = spec(Command::Run);
        assert!(run.aspects.is_empty());
        assert!(run.reports.is_empty());
        let generate = spec(Command::Generate);
        assert!(generate.aspects.is_empty());
        assert!(generate.reports.is_empty());
        let clean = spec(Command::Clean);
        assert!(clean.aspects.is_empty());
        assert!(clean.reports.is_empty());
        let env = spec(Command::Env);
        assert!(env.aspects.is_empty());
        assert!(env.reports.is_empty());
        assert!(Command::Env.is_managed());
        for command in [Command::Security, Command::License] {
            let entry = spec(command);
            assert!(entry.aspects.is_empty());
        }
        assert_eq!(spec(Command::Security).reports, &[StandardFormat::Sarif]);
        assert_eq!(spec(Command::License).reports, &[StandardFormat::Sarif]);
        let update = spec(Command::Update);
        assert!(update.aspects.is_empty());
        assert!(update.reports.is_empty());
    }

    #[test]
    fn standard_format_names_and_parsing_are_inverses() {
        for format in [
            StandardFormat::Sarif,
            StandardFormat::Junit,
            StandardFormat::Lcov,
            StandardFormat::Sources,
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
