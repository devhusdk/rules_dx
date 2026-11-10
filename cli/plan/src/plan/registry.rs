use super::registry_gen;
use super::rules_dx_label;
use crate::args::Command;
use crate::reports::StandardFormat;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

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

/// The labels of the aspect files without a language, then those of every language's `quality`
/// entry (and, for `docs`, its `docs` entry) in `lang.json`.
fn with_languages(
    command: &'static str,
    probes: bool,
    core: &'static [&'static str],
) -> &'static [&'static str] {
    type Key = (&'static str, bool);
    static ALL: OnceLock<BTreeMap<Key, &'static [&'static str]>> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        let mut all = BTreeMap::new();
        for (command, probes, core) in [
            ("lint", false, registry_gen::LINT_ASPECTS),
            ("lint", true, registry_gen::LINT_PROBES),
            ("typecheck", false, registry_gen::TYPECHECK_ASPECTS),
            ("typecheck", true, registry_gen::TYPECHECK_PROBES),
            ("format", false, registry_gen::FORMAT_ASPECTS),
            ("format", true, registry_gen::FORMAT_PROBES),
            ("docs", false, registry_gen::DOCS_ASPECTS),
        ] {
            let mut labels: Vec<&'static str> = core.to_vec();
            for language in dx_families::quality_languages() {
                labels.extend(
                    language
                        .labels(command, probes)
                        .into_iter()
                        .map(|label| &*Box::leak(label.into_boxed_str())),
                );
            }
            if command == "docs" {
                for language in dx_families::docs_languages() {
                    labels.extend(
                        language
                            .labels()
                            .into_iter()
                            .map(|label| &*Box::leak(label.into_boxed_str())),
                    );
                }
            }
            all.insert((command, probes), &*Box::leak(labels.into_boxed_slice()));
        }
        all
    });
    all.get(&(command, probes)).copied().unwrap_or(core)
}

pub fn spec(command: Command) -> CommandSpec {
    match command {
        Command::Lint => CommandSpec {
            command,
            aspects: with_languages("lint", false, registry_gen::LINT_ASPECTS),
            probes: with_languages("lint", true, registry_gen::LINT_PROBES),
            reports: &[StandardFormat::Sarif],
        },
        Command::Typecheck => CommandSpec {
            command,
            aspects: with_languages("typecheck", false, registry_gen::TYPECHECK_ASPECTS),
            probes: with_languages("typecheck", true, registry_gen::TYPECHECK_PROBES),
            reports: &[StandardFormat::Sarif],
        },
        Command::Format => CommandSpec {
            command,
            aspects: with_languages("format", false, registry_gen::FORMAT_ASPECTS),
            probes: with_languages("format", true, registry_gen::FORMAT_PROBES),
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
            aspects: with_languages("docs", false, registry_gen::DOCS_ASPECTS),
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

/// Whether the file that defines `label` loads no language rule set or loads one the consumer has.
fn wanted(label: &str, consumer: Consumer) -> bool {
    let file = label.split_once('%').map_or(label, |(file, _)| file);
    let extensions = dx_families::quality_languages()
        .iter()
        .find(|language| language.bzl == file)
        .map(|language| &language.file_extensions)
        .or_else(|| {
            dx_families::docs_languages()
                .iter()
                .find(|language| language.bzl == file)
                .map(|language| &language.file_extensions)
        });
    extensions.is_none_or(|extensions| {
        extensions
            .iter()
            .any(|extension| consumer.extensions.contains(extension))
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
                "@rules_dx//core/quality:families/core_probe.bzl%real_core_{probe}_probe_aspect"
            )));
        }
        assert!(
            labels.contains(&"@rules_dx//core/quality:families/core.bzl%real_lint_aspect".to_owned())
        );
        assert!(
            labels
                .contains(&"@rules_dx//core/quality:families/core.bzl%real_typecheck_aspect".to_owned())
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
                if file.starts_with("tools/docs") {
                    continue;
                }
                let family = dx_families::families()
                    .iter()
                    .find(|family| family.aspects.iter().any(|aspect| aspect == name));
                match family {
                    Some(family) if family.name != "core" => {
                        let in_quality = format!("core/quality:families/{}.bzl", family.name);
                        let in_language = format!("javascript/private/quality:{}.bzl", family.name);
                        assert!(file == in_quality || file == in_language, "{label}");
                    }
                    _ => assert!(
                        file.starts_with("core/quality:families/")
                            || file.contains("/private/quality:"),
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

    fn families_json() -> Vec<dx_families::Family> {
        dx_families::parse(
            r#"{"families": {
                "python": {"aspects": ["real_python_lint_aspect"]},
                "jvm": {"module": "rules_dx_jvm", "aspects": ["real_jvm_lint_aspect"]},
                "rust": {"module": "rules_dx_rust", "aspects": ["real_rust_lint_aspect"]},
                "core": {"aspects": ["real_lint_aspect"]}
            }}"#,
        )
        .expect("families")
    }

    #[test]
    fn aspects_for_drops_aspects_of_families_that_are_not_installed() {
        let installed = ["rules_dx_rust".to_owned()];
        let extensions = extensions(&[".c", ".py", ".rs"]);
        let lint = aspects_for_families(
            Command::Lint,
            &families_json(),
            consumer(&installed, &extensions),
        );
        let has = |name: &str| {
            lint.iter()
                .any(|aspect| aspect.ends_with(&format!("%{name}")))
        };
        assert!(has("real_lint_aspect"));
        assert!(has("real_python_lint_aspect"));
        assert!(has("real_cc_lint_aspect"));
        assert!(has("real_rust_lint_aspect"));
        assert!(!has("real_jvm_lint_aspect"));
    }

    #[test]
    fn aspects_for_keeps_families_without_a_module() {
        let extensions = extensions(&[".py", ".java"]);
        let lint = aspects_for_families(
            Command::Lint,
            &families_json(),
            consumer(&[], &extensions),
        );
        assert!(lint.contains(&"core/quality:families/python.bzl%real_python_lint_aspect"));
        assert!(!lint.contains(&"core/quality:families/jvm.bzl%real_jvm_lint_aspect"));
    }

    #[test]
    fn probes_for_lists_the_core_probe_and_the_installed_families_probes() {
        let installed = ["rules_dx_rust".to_owned()];
        let extensions = extensions(&[".py", ".rs", ".swift"]);
        let probes = probes_for(Command::Lint, consumer(&installed, &extensions));
        assert!(probes.contains(&"core/quality:families/core_probe.bzl%real_core_lint_probe_aspect"));
        assert!(probes.contains(&"core/quality:families/python.bzl%real_python_lint_probe_aspect"));
        assert!(probes.contains(&"rust/private/quality:rust.bzl%real_rust_lint_probe_aspect"));
        assert!(probes.contains(&"core/quality:families/swift.bzl%real_swift_lint_probe_aspect"));
        assert!(probes_for(Command::Docs, consumer(&installed, &extensions)).is_empty());
    }

    #[test]
    fn go_only_consumer_docs_selects_the_core_aspect_and_go() {
        let extensions = extensions(&[".go", ".json", ".md"]);
        let labels = aspects_for(Command::Docs, consumer(&[], &extensions));
        assert_eq!(
            labels,
            [
                "tools/docs:aspect.bzl%real_docs_aspect",
                "tools/docs/go:aspect.bzl%real_go_docs_aspect"
            ]
        );
    }

    #[test]
    fn go_only_consumer_selects_core_and_go() {
        let extensions = extensions(&[".go", ".json", ".md"]);
        for command in [Command::Lint, Command::Typecheck, Command::Format] {
            let entry = spec(command);
            let labels = aspects_for(command, consumer(&[], &extensions));
            let probes = probes_for(command, consumer(&[], &extensions));
            for label in labels.iter().chain(&probes) {
                let file = label.split_once('%').expect("label has a name").0;
                let language = dx_families::quality_languages()
                    .iter()
                    .find(|language| language.bzl == file);
                assert!(
                    language.is_none_or(|language| language.bzl == "core/quality:families/go.bzl"),
                    "{command:?}: {label}"
                );
            }
            assert!(labels.iter().any(|label| label.contains("families/go.bzl")));
            assert!(labels.iter().any(|label| label.contains("families/core.bzl")));
            assert!(labels.len() < entry.aspects.len());
        }
        let probes = probes_for(Command::Lint, consumer(&[], &extensions));
        assert_eq!(
            probes,
            vec![
                "core/quality:families/core_probe.bzl%real_core_lint_probe_aspect",
                "core/quality:families/go.bzl%real_go_lint_probe_aspect",
            ]
        );
    }

    #[test]
    fn every_non_core_aspect_label_maps_to_one_language_file() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let entry = spec(*command);
            for label in entry.aspects.iter().chain(entry.probes) {
                let file = label.split_once('%').expect("label has a name").0;
                let owners = dx_families::quality_languages()
                    .iter()
                    .filter(|language| language.bzl == file)
                    .count();
                assert!(owners <= 1, "{label}");
            }
            let mut sorted: Vec<&str> = entry.aspects.iter().chain(entry.probes).copied().collect();
            sorted.sort();
            let before = sorted.len();
            sorted.dedup();
            assert_eq!(before, sorted.len(), "{command:?} lists a label twice");
        }
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
                    dx_families::quality_languages()
                        .iter()
                        .any(|row| row.file_extensions.contains(extension)),
                    "{} extension {extension} is in no lang.json quality entry",
                    family.name
                );
            }
        }
    }

    #[test]
    fn every_language_file_has_aspects() {
        use clap::ValueEnum;
        for language in dx_families::quality_languages() {
            let used = Command::value_variants().iter().any(|command| {
                let entry = spec(*command);
                entry.aspects.iter().chain(entry.probes).any(|label| {
                    label.split_once('%').map(|(file, _)| file) == Some(language.bzl.as_str())
                })
            });
            assert!(used, "{} names no aspect", language.bzl);
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
                    .strip_prefix("core/quality:families/")
                    .and_then(|rest| rest.split_once(".bzl"))
                    .map(|(file, _)| file)
                else {
                    continue;
                };
                let bzl = format!("core/quality:families/{file}.bzl");
                if !dx_families::quality_languages()
                    .iter()
                    .any(|language| language.bzl == bzl)
                {
                    continue;
                }
                let probe =
                    format!("core/quality:families/{file}.bzl%real_{file}_{capability}_probe_aspect");
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

    fn assert_core_then_languages(entry: &CommandSpec) {
        let core = match entry.command {
            Command::Lint => (registry_gen::LINT_ASPECTS, registry_gen::LINT_PROBES),
            Command::Typecheck => (registry_gen::TYPECHECK_ASPECTS, registry_gen::TYPECHECK_PROBES),
            _ => (registry_gen::FORMAT_ASPECTS, registry_gen::FORMAT_PROBES),
        };
        assert_eq!(&entry.aspects[..core.0.len()], core.0);
        assert_eq!(&entry.probes[..core.1.len()], core.1);
        let language_aspects: Vec<&str> = entry.aspects[core.0.len()..].to_vec();
        assert!(!language_aspects.is_empty());
        for label in language_aspects.iter().chain(&entry.probes[core.1.len()..]) {
            let file = label.split_once('%').expect("label has a name").0;
            assert!(
                dx_families::quality_languages()
                    .iter()
                    .any(|language| language.bzl == file),
                "{label} is in no lang.json quality entry"
            );
        }
    }

    #[test]
    fn registry_entries_match_command_contract() {
        let lint = spec(Command::Lint);
        assert_core_then_languages(&lint);
        assert_eq!(lint.reports, &[StandardFormat::Sarif]);
        let typecheck = spec(Command::Typecheck);
        assert_core_then_languages(&typecheck);
        assert_eq!(typecheck.reports, &[StandardFormat::Sarif]);
        let format = spec(Command::Format);
        assert_core_then_languages(&format);
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
        assert_eq!(docs.aspects[0], "tools/docs:aspect.bzl%real_docs_aspect");
        assert!(docs.aspects.contains(&"tools/docs/go:aspect.bzl%real_go_docs_aspect"));
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
