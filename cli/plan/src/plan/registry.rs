use super::registry_gen;
use super::rules_dx_label;
use crate::args::Command;
use crate::reports::StandardFormat;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub command: Command,
    pub aspects: &'static [&'static str],
    /// The cheap aspects that name the shards each target needs; see `select_aspects`. The first is
    /// the core family's, then one per family file.
    pub probes: &'static [&'static str],
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
            aspects: registry_gen::LINT_ASPECTS,
            probes: registry_gen::LINT_PROBES,
            reports: &[StandardFormat::Sarif],
        },
        Command::Typecheck => CommandSpec {
            command,
            aspects: registry_gen::TYPECHECK_ASPECTS,
            probes: registry_gen::TYPECHECK_PROBES,
            reports: &[StandardFormat::Sarif],
        },
        Command::Format => CommandSpec {
            command,
            aspects: registry_gen::FORMAT_ASPECTS,
            probes: registry_gen::FORMAT_PROBES,
            reports: &[],
        },
        Command::Generate => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[],
        },
        Command::Build => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[StandardFormat::Failures],
        },
        Command::Test => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[
                StandardFormat::Junit,
                StandardFormat::Failures,
                StandardFormat::Results,
            ],
        },
        Command::Coverage => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
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
            probes: &[],
            reports: &[],
        },
        Command::Deploy => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[],
        },
        Command::Check => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[StandardFormat::Sarif],
        },
        Command::Clean => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[],
        },
        Command::Sync => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[],
        },
        Command::Bench
        | Command::Init
        | Command::Upgrade
        | Command::Worktree
        | Command::Fuzz
        | Command::Status
        | Command::Config
        | Command::Watch
        | Command::Completion => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[],
        },
        Command::Security => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[StandardFormat::Sarif],
        },
        Command::License => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[
                StandardFormat::Sarif,
                StandardFormat::Spdx,
                StandardFormat::Cyclonedx,
            ],
        },
        Command::Update => CommandSpec {
            command,
            aspects: &[],
            probes: &[],
            reports: &[],
        },
        Command::Docs => CommandSpec {
            command,
            aspects: registry_gen::DOCS_ASPECTS,
            probes: &[],
            reports: &[],
        },
    }
}

/// What a consumer has: its `bazel_dep` names and the extensions of the files in its workspace.
#[derive(Debug, Clone, Copy)]
pub struct Consumer<'a> {
    pub modules: &'a [String],
    pub extensions: &'a BTreeSet<String>,
}

/// The aspect files that load no language rule set, so every consumer gets them.
const CORE_FILES: &[&str] = &[
    "quality:families/ansible.bzl",
    "quality:families/bicep.bzl",
    "quality:families/cloudformation.bzl",
    "quality:families/cmake.bzl",
    "quality:families/core.bzl",
    "quality:families/core_probe.bzl",
    "quality:families/cue.bzl",
    "quality:families/dockerfile.bzl",
    "quality:families/html_template.bzl",
    "quality:families/jsonnet.bzl",
    "quality:families/kubernetes.bzl",
    "quality:families/latex.bzl",
    "quality:families/make.bzl",
    "quality:families/nix.bzl",
    "quality:families/rego.bzl",
    "quality:families/shell.bzl",
    "quality:families/sql.bzl",
    "quality:families/terraform.bzl",
    "quality:families/text.bzl",
    "quality:families/typst.bzl",
    "quality:families/xml.bzl",
    "quality:families/yaml.bzl",
];

/// The aspect files that load a language rule set, with the file extensions that make a workspace
/// need them. Moves to lang.json (#10079).
const LANGUAGES: &[(&str, &[&str])] = &[
    (
        "quality:families/beam.bzl",
        &[".erl", ".ex", ".exs", ".hrl"],
    ),
    (
        "quality:families/cc.bzl",
        &[
            ".c", ".cc", ".cpp", ".cu", ".cuh", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".inl",
            ".ipp", ".m", ".mm", ".qml",
        ],
    ),
    ("quality:families/d.bzl", &[".d"]),
    ("quality:families/dart.bzl", &[".dart"]),
    (
        "quality:families/dotnet.bzl",
        &[".cs", ".csproj", ".fs", ".fsproj", ".fsx"],
    ),
    (
        "quality:families/fortran.bzl",
        &[".f", ".f03", ".f08", ".f90", ".f95"],
    ),
    (
        "quality:families/generation.bzl",
        &[".avsc", ".capnp", ".fbs", ".proto", ".thrift"],
    ),
    ("quality:families/go.bzl", &[".go"]),
    ("quality:families/haskell.bzl", &[".hs"]),
    (
        "quality:families/js.bzl",
        &[
            ".astro", ".cjs", ".cts", ".js", ".jsx", ".mdx", ".mjs", ".mts", ".svelte", ".ts",
            ".tsx", ".vue",
        ],
    ),
    ("quality:families/julia.bzl", &[".jl"]),
    (
        "quality:families/jvm.bzl",
        &[".java", ".kt", ".kts", ".sc", ".scala"],
    ),
    ("quality:families/lua.bzl", &[".lua"]),
    ("quality:families/ocaml.bzl", &[".ml", ".mli"]),
    ("quality:families/perl.bzl", &[".pl", ".pm"]),
    ("quality:families/php.bzl", &[".php"]),
    ("quality:families/pkl.bzl", &[".pkl"]),
    (
        "quality:families/powershell.bzl",
        &[".ps1", ".psd1", ".psm1"],
    ),
    ("quality:families/python.bzl", &[".py", ".pyi"]),
    ("quality:families/ruby.bzl", &[".rb", ".rbs"]),
    ("quality:families/rust.bzl", &[".rs"]),
    ("quality:families/swift.bzl", &[".swift"]),
    ("quality:families/zig.bzl", &[".zig"]),
];

/// The aspects of `command` that fit the consumer: those of core files and of the languages whose
/// files it has, when their family module is installed. A family without a `module` is always kept.
pub fn aspects_for(command: Command, consumer: Consumer) -> Vec<&'static str> {
    aspects_for_families(command, dx_families::families(), consumer)
}

fn aspects_for_families(
    command: Command,
    families: &[dx_families::Family],
    consumer: Consumer,
) -> Vec<&'static str> {
    let missing = missing_aspects(families, consumer.modules);
    spec(command)
        .aspects
        .iter()
        .copied()
        .filter(|aspect| !missing.contains(&label_name(aspect)) && wanted(aspect, consumer))
        .collect()
}

/// The probes of `command` that fit the consumer, chosen as its aspects are. Empty for a command
/// without a probe.
pub fn probes_for(command: Command, consumer: Consumer) -> Vec<&'static str> {
    probes_for_families(command, dx_families::families(), consumer)
}

fn probes_for_families(
    command: Command,
    families: &[dx_families::Family],
    consumer: Consumer,
) -> Vec<&'static str> {
    let missing = missing_aspects(families, consumer.modules);
    spec(command)
        .probes
        .iter()
        .copied()
        .filter(|probe| !missing.contains(&label_name(probe)) && wanted(probe, consumer))
        .collect()
}

/// Whether the file that defines `label` is a core file or one of a language the consumer has.
fn wanted(label: &str, consumer: Consumer) -> bool {
    let file = label.split_once('%').map_or(label, |(file, _)| file);
    CORE_FILES.contains(&file)
        || LANGUAGES.iter().any(|(bzl, extensions)| {
            *bzl == file
                && extensions
                    .iter()
                    .any(|extension| consumer.extensions.contains(*extension))
        })
}

fn label_name(label: &str) -> &str {
    label.rsplit_once('%').map_or(label, |(_, name)| name)
}

fn missing_aspects<'a>(
    families: &'a [dx_families::Family],
    installed_modules: &[String],
) -> Vec<&'a str> {
    families
        .iter()
        .filter(|family| {
            family
                .module
                .as_ref()
                .is_some_and(|module| !installed_modules.contains(module))
        })
        .flat_map(|family| family.aspects.iter().map(String::as_str))
        .collect()
}

/// Every aspect label dx passes to Bazel.
pub fn all_aspect_labels() -> Vec<String> {
    use clap::ValueEnum;
    let mut labels: Vec<String> = Command::value_variants()
        .iter()
        .flat_map(|command| {
            let entry = spec(*command);
            entry
                .aspects
                .iter()
                .copied()
                .chain(entry.probes.iter().copied())
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
                "@rules_dx//quality:families/core_probe.bzl%real_core_{probe}_probe_aspect"
            )));
        }
        assert!(
            labels.contains(&"@rules_dx//quality:families/core.bzl%real_lint_aspect".to_owned())
        );
        assert!(
            labels
                .contains(&"@rules_dx//quality:families/core.bzl%real_typecheck_aspect".to_owned())
        );
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
                match family {
                    Some(family) if family.name != "core" => {
                        assert_eq!(
                            file,
                            format!("quality:families/{}.bzl", family.name),
                            "{label}"
                        );
                    }
                    _ => assert!(
                        file.starts_with("quality:families/") || file == "tools/docs:aspect.bzl",
                        "{label}"
                    ),
                }
            }
        }
    }

    fn extensions(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn consumer<'a>(modules: &'a [String], extensions: &'a BTreeSet<String>) -> Consumer<'a> {
        Consumer {
            modules,
            extensions,
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
        let extensions = extensions(&[".c", ".py", ".rs"]);
        let lint = aspects_for_families(
            Command::Lint,
            &families_json("rules_dx_python"),
            consumer(&installed, &extensions),
        );
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
        let extensions = extensions(&[".py", ".java"]);
        let lint = aspects_for_families(
            Command::Lint,
            &families_json(""),
            consumer(&[], &extensions),
        );
        assert!(lint.contains(&"quality:families/python.bzl%real_python_lint_aspect"));
        assert!(!lint.contains(&"quality:families/jvm.bzl%real_jvm_lint_aspect"));
    }

    #[test]
    fn probes_for_lists_the_core_probe_and_the_installed_families_probes() {
        let installed = ["rules_dx_python".to_owned(), "rules_dx_rust".to_owned()];
        let extensions = extensions(&[".py", ".rs", ".erl"]);
        let probes = probes_for(Command::Lint, consumer(&installed, &extensions));
        assert!(probes.contains(&"quality:families/core_probe.bzl%real_core_lint_probe_aspect"));
        assert!(probes.contains(&"quality:families/python.bzl%real_python_lint_probe_aspect"));
        assert!(probes.contains(&"quality:families/rust.bzl%real_rust_lint_probe_aspect"));
        assert!(!probes.contains(&"quality:families/beam.bzl%real_beam_lint_probe_aspect"));
        assert!(probes_for(Command::Docs, consumer(&installed, &extensions)).is_empty());
    }

    #[test]
    fn go_only_consumer_selects_core_and_go() {
        let extensions = extensions(&[".go", ".json", ".md"]);
        let lint = aspects_for(Command::Lint, consumer(&[], &extensions));
        assert_eq!(
            lint,
            vec![
                "quality:families/dockerfile.bzl%real_dockerfile_lint_aspect",
                "quality:families/make.bzl%real_make_lint_aspect",
                "quality:families/cmake.bzl%real_cmake_lint_aspect",
                "quality:families/terraform.bzl%real_terraform_lint_aspect",
                "quality:families/go.bzl%real_go_lint_aspect",
                "quality:families/core.bzl%real_lint_aspect",
                "quality:families/shell.bzl%real_shell_lint_aspect",
                "quality:families/text.bzl%real_text_lint_aspect",
                "quality:families/html_template.bzl%real_html_template_lint_aspect",
                "quality:families/yaml.bzl%real_yaml_lint_aspect",
                "quality:families/nix.bzl%real_nix_lint_aspect",
                "quality:families/kubernetes.bzl%real_kubernetes_lint_aspect",
                "quality:families/ansible.bzl%real_ansible_lint_aspect",
                "quality:families/cloudformation.bzl%real_cloudformation_lint_aspect",
                "quality:families/bicep.bzl%real_bicep_lint_aspect",
                "quality:families/sql.bzl%real_sql_lint_aspect",
                "quality:families/rego.bzl%real_rego_lint_aspect",
                "quality:families/jsonnet.bzl%real_jsonnet_lint_aspect",
                "quality:families/typst.bzl%real_typst_lint_aspect",
                "quality:families/xml.bzl%real_xml_lint_aspect",
                "quality:families/latex.bzl%real_latex_lint_aspect",
            ]
        );
        let probes = probes_for(Command::Lint, consumer(&[], &extensions));
        assert_eq!(
            probes,
            vec![
                "quality:families/core_probe.bzl%real_core_lint_probe_aspect",
                "quality:families/go.bzl%real_go_lint_probe_aspect",
            ]
        );
    }

    /// Family extensions no aspect file checks.
    const UNCHECKED_EXTENSIONS: &[&str] = &[".aidl", ".plist"];

    #[test]
    fn language_rows_cover_family_extensions() {
        for family in dx_families::families() {
            if family.module.is_none() {
                continue;
            }
            for extension in &family.file_extensions {
                if UNCHECKED_EXTENSIONS.contains(&extension.as_str()) {
                    continue;
                }
                assert!(
                    LANGUAGES
                        .iter()
                        .any(|(_, row)| row.contains(&extension.as_str())),
                    "{} extension {extension} is in no LANGUAGES row",
                    family.name
                );
            }
        }
    }

    #[test]
    fn every_non_core_aspect_file_has_a_language_row() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            if *command == Command::Docs {
                // #10116
                continue;
            }
            let entry = spec(*command);
            for label in entry.aspects.iter().chain(entry.probes) {
                let file = label.split_once('%').map_or(*label, |(file, _)| file);
                let core = CORE_FILES.contains(&file);
                let language = LANGUAGES.iter().any(|(bzl, _)| *bzl == file);
                assert!(
                    core != language,
                    "{label}: core {core}, language row {language}"
                );
            }
        }
    }

    #[test]
    fn every_family_with_shard_aspects_has_a_probe_for_that_capability() {
        for (command, capability) in [
            (Command::Lint, "lint"),
            (Command::Format, "format"),
            (Command::Typecheck, "typecheck"),
        ] {
            let entry = spec(command);
            for aspect in entry.aspects {
                let Some(file) = aspect
                    .strip_prefix("quality:families/")
                    .and_then(|rest| rest.split_once(".bzl"))
                    .map(|(file, _)| file)
                else {
                    continue;
                };
                if CORE_FILES.contains(&format!("quality:families/{file}.bzl").as_str()) {
                    continue;
                }
                let probe =
                    format!("quality:families/{file}.bzl%real_{file}_{capability}_probe_aspect");
                assert!(
                    entry.probes.contains(&probe.as_str()),
                    "{command:?}: {aspect} has no probe {probe}"
                );
            }
        }
    }

    #[test]
    fn registry_aspects_are_relative_to_rules_dx() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let entry = spec(*command);
            for aspect in entry
                .aspects
                .iter()
                .copied()
                .chain(entry.probes.iter().copied())
            {
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
                "quality:families/dockerfile.bzl%real_dockerfile_lint_aspect",
                "quality:families/make.bzl%real_make_lint_aspect",
                "quality:families/cmake.bzl%real_cmake_lint_aspect",
                "quality:families/terraform.bzl%real_terraform_lint_aspect",
                "quality:families/dotnet.bzl%real_dotnet_lint_aspect",
                "quality:families/generation.bzl%real_capnp_lint_aspect",
                "quality:families/generation.bzl%real_flatbuffers_lint_aspect",
                "quality:families/generation.bzl%real_thrift_lint_aspect",
                "quality:families/generation.bzl%real_avro_lint_aspect",
                "quality:families/go.bzl%real_go_lint_aspect",
                "quality:families/core.bzl%real_lint_aspect",
                "quality:families/js.bzl%real_js_lint_aspect",
                "quality:families/python.bzl%real_python_lint_aspect",
                "quality:families/generation.bzl%real_protobuf_lint_aspect",
                "quality:families/powershell.bzl%real_powershell_lint_aspect",
                "quality:families/jvm.bzl%real_jvm_lint_aspect",
                "quality:families/ruby.bzl%real_ruby_lint_aspect",
                "quality:families/rust.bzl%real_rust_lint_aspect",
                "quality:families/jvm.bzl%real_scala_lint_aspect",
                "quality:families/shell.bzl%real_shell_lint_aspect",
                "quality:families/text.bzl%real_text_lint_aspect",
                "quality:families/html_template.bzl%real_html_template_lint_aspect",
                "quality:families/yaml.bzl%real_yaml_lint_aspect",
                "quality:families/nix.bzl%real_nix_lint_aspect",
                "quality:families/lua.bzl%real_lua_lint_aspect",
                "quality:families/ocaml.bzl%real_ocaml_lint_aspect",
                "quality:families/kubernetes.bzl%real_kubernetes_lint_aspect",
                "quality:families/ansible.bzl%real_ansible_lint_aspect",
                "quality:families/cloudformation.bzl%real_cloudformation_lint_aspect",
                "quality:families/bicep.bzl%real_bicep_lint_aspect",
                "quality:families/sql.bzl%real_sql_lint_aspect",
                "quality:families/rego.bzl%real_rego_lint_aspect",
                "quality:families/jsonnet.bzl%real_jsonnet_lint_aspect",
                "quality:families/fortran.bzl%real_fortran_lint_aspect",
                "quality:families/typst.bzl%real_typst_lint_aspect",
                "quality:families/xml.bzl%real_xml_lint_aspect",
                "quality:families/haskell.bzl%real_haskell_lint_aspect",
                "quality:families/latex.bzl%real_latex_lint_aspect",
                "quality:families/beam.bzl%real_erlang_lint_aspect",
                "quality:families/beam.bzl%real_elixir_lint_aspect",
                "quality:families/php.bzl%real_php_lint_aspect",
                "quality:families/dart.bzl%real_dart_lint_aspect",
                "quality:families/d.bzl%real_d_lint_aspect",
                "quality:families/perl.bzl%real_perl_lint_aspect",
                "quality:families/swift.bzl%real_swift_lint_aspect",
                "quality:families/zig.bzl%real_zig_lint_aspect",
                "quality:families/julia.bzl%real_julia_lint_aspect",
                "quality:families/pkl.bzl%real_pkl_lint_aspect",
            ]
        );
        assert_eq!(lint.reports, &[StandardFormat::Sarif]);
        let typecheck = spec(Command::Typecheck);
        assert_eq!(
            typecheck.aspects,
            &[
                "quality:families/core.bzl%real_typecheck_aspect",
                "quality:families/go.bzl%real_go_typecheck_aspect",
                "quality:families/js.bzl%real_js_typecheck_aspect",
                "quality:families/jvm.bzl%real_jvm_typecheck_aspect",
                "quality:families/d.bzl%real_d_typecheck_aspect",
                "quality:families/fortran.bzl%real_fortran_typecheck_aspect",
                "quality:families/zig.bzl%real_zig_typecheck_aspect",
                "quality:families/ocaml.bzl%real_ocaml_typecheck_aspect",
                "quality:families/haskell.bzl%real_haskell_typecheck_aspect",
                "quality:families/rust.bzl%real_rust_typecheck_aspect",
                "quality:families/dotnet.bzl%real_dotnet_typecheck_aspect",
                "quality:families/rego.bzl%real_rego_typecheck_aspect",
                "quality:families/generation.bzl%real_capnp_typecheck_aspect",
                "quality:families/generation.bzl%real_flatbuffers_typecheck_aspect",
                "quality:families/generation.bzl%real_thrift_typecheck_aspect",
                "quality:families/generation.bzl%real_avro_typecheck_aspect",
                "quality:families/generation.bzl%real_protobuf_typecheck_aspect",
                "quality:families/cue.bzl%real_cue_typecheck_aspect",
                "quality:families/cc.bzl%real_cc_typecheck_aspect",
                "quality:families/pkl.bzl%real_pkl_typecheck_aspect",
                "quality:families/terraform.bzl%real_terraform_typecheck_aspect",
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
                "quality:families/cue.bzl%real_cue_format_aspect",
                "quality:families/dotnet.bzl%real_dotnet_format_aspect",
                "quality:families/core.bzl%real_format_aspect",
                "quality:families/fortran.bzl%real_fortran_format_aspect",
                "quality:families/go.bzl%real_go_format_aspect",
                "quality:families/haskell.bzl%real_haskell_format_aspect",
                "quality:families/js.bzl%real_js_format_aspect",
                "quality:families/jsonnet.bzl%real_jsonnet_format_aspect",
                "quality:families/jvm.bzl%real_jvm_format_aspect",
                "quality:families/lua.bzl%real_lua_format_aspect",
                "quality:families/bicep.bzl%real_bicep_format_aspect",
                "quality:families/cmake.bzl%real_cmake_format_aspect",
                "quality:families/make.bzl%real_make_format_aspect",
                "quality:families/nix.bzl%real_nix_format_aspect",
                "quality:families/kubernetes.bzl%real_kubernetes_format_aspect",
                "quality:families/ocaml.bzl%real_ocaml_format_aspect",
                "quality:families/pkl.bzl%real_pkl_format_aspect",
                "quality:families/generation.bzl%real_protobuf_format_aspect",
                "quality:families/powershell.bzl%real_powershell_format_aspect",
                "quality:families/rego.bzl%real_rego_format_aspect",
                "quality:families/ruby.bzl%real_ruby_format_aspect",
                "quality:families/rust.bzl%real_rust_format_aspect",
                "quality:families/jvm.bzl%real_scala_format_aspect",
                "quality:families/shell.bzl%real_shell_format_aspect",
                "quality:families/terraform.bzl%real_terraform_format_aspect",
                "quality:families/typst.bzl%real_typst_format_aspect",
                "quality:families/latex.bzl%real_latex_format_aspect",
                "quality:families/html_template.bzl%real_html_template_format_aspect",
                "quality:families/sql.bzl%real_sql_format_aspect",
                "quality:families/yaml.bzl%real_yaml_format_aspect",
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
        assert_eq!(docs.aspects, &["tools/docs:aspect.bzl%real_docs_aspect"]);
        assert!(docs.probes.is_empty());
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
        let env = spec(Command::Sync);
        assert!(env.aspects.is_empty());
        assert!(env.reports.is_empty());
        assert!(Command::Sync.is_managed());
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
                StandardFormat::Cyclonedx
            ]
        );
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
            StandardFormat::Spdx,
            StandardFormat::Cyclonedx,
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
            StandardFormat::Spdx,
            StandardFormat::Cyclonedx,
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
