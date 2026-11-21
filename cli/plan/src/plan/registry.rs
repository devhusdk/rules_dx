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
    #[must_use]
    pub fn accepts_report(&self, format: &str) -> bool {
        StandardFormat::parse(format).is_some_and(|parsed| self.reports.contains(&parsed))
    }
}

fn leak(label: String) -> &'static str {
    Box::leak(label.into_boxed_str())
}

/// The core labels of `command`: those of `core/quality/core_aspects.json`, in file order.
fn core_labels(command: &str, probes: bool) -> Vec<String> {
    if command == "docs" {
        return dx_families::core_docs()
            .iter()
            .flat_map(dx_families::DocsFiles::labels)
            .collect();
    }
    dx_families::core_quality()
        .iter()
        .flat_map(|files| files.labels(command, probes))
        .collect()
}

/// The labels of the aspect files in core, then those of every language's `quality` entry (and,
/// for `docs`, its `docs` entry) in `lang.json`.
fn with_languages(command: &'static str, probes: bool) -> &'static [&'static str] {
    type Key = (&'static str, bool);
    static ALL: OnceLock<BTreeMap<Key, &'static [&'static str]>> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        let mut all = BTreeMap::new();
        for (command, probes) in [
            ("lint", false),
            ("lint", true),
            ("typecheck", false),
            ("typecheck", true),
            ("format", false),
            ("format", true),
            ("docs", false),
        ] {
            let mut labels: Vec<&'static str> =
                core_labels(command, probes).into_iter().map(leak).collect();
            for language in dx_families::quality_languages() {
                labels.extend(language.labels(command, probes).into_iter().map(leak));
            }
            if command == "docs" {
                for language in dx_families::docs_languages() {
                    labels.extend(language.labels().into_iter().map(leak));
                }
            }
            all.insert((command, probes), &*Box::leak(labels.into_boxed_slice()));
        }
        all
    });
    all.get(&(command, probes)).copied().unwrap_or_default()
}

#[must_use]
#[expect(
    clippy::match_same_arms,
    reason = "one arm per command keeps each command spec independent"
)]
#[expect(
    clippy::too_many_lines,
    reason = "one linear sequence of steps read top to bottom"
)]
pub fn spec(command: Command) -> CommandSpec {
    match command {
        Command::Lint => CommandSpec {
            command,
            aspects: with_languages("lint", false),
            probes: with_languages("lint", true),
            reports: &[StandardFormat::Sarif],
        },
        Command::Typecheck => CommandSpec {
            command,
            aspects: with_languages("typecheck", false),
            probes: with_languages("typecheck", true),
            reports: &[StandardFormat::Sarif],
        },
        Command::Format => CommandSpec {
            command,
            aspects: with_languages("format", false),
            probes: with_languages("format", true),
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
        | Command::Fetch
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
            aspects: with_languages("docs", false),
            probes: &[],
            reports: &[],
        },
    }
}

/// What a consumer has: the extensions of the files in its workspace.
#[derive(Debug, Clone, Copy)]
pub struct Consumer<'a> {
    pub extensions: &'a BTreeSet<String>,
}

/// The aspects of `command` that fit the consumer: those of core files and of the languages whose
/// files it has.
#[must_use]
pub fn aspects_for(command: Command, consumer: Consumer<'_>) -> Vec<&'static str> {
    spec(command)
        .aspects
        .iter()
        .copied()
        .filter(|aspect| wanted(aspect, consumer))
        .collect()
}

/// The probes of `command` that fit the consumer, chosen as its aspects are. Empty for a command
/// without a probe.
#[must_use]
pub fn probes_for(command: Command, consumer: Consumer<'_>) -> Vec<&'static str> {
    spec(command)
        .probes
        .iter()
        .copied()
        .filter(|probe| wanted(probe, consumer))
        .collect()
}

/// Whether the file that defines `label` is a core file or one the consumer's `lang.json` entry
/// asks for.
fn wanted(label: &str, consumer: Consumer<'_>) -> bool {
    let file = label.split_once('%').map_or(label, |(file, _)| file);
    if let Some(language) = quality_owner(file) {
        return language.wanted(consumer.extensions);
    }
    dx_families::docs_languages()
        .iter()
        .find(|language| language.bzl == file)
        .is_none_or(|language| {
            language
                .file_extensions
                .iter()
                .any(|extension| consumer.extensions.contains(extension))
        })
}

/// The language whose files decide whether the aspects of `file` run: the first whose `quality`
/// entry names aspects or probes in it. A language that only shares a core file, as asciidoc shares
/// `core.bzl`, decides nothing, so that file's aspects run for every consumer.
fn quality_owner(file: &str) -> Option<&'static dx_families::QualityFiles> {
    dx_families::quality_languages().iter().find(|language| {
        language.bzl == file && (!language.aspects.is_empty() || !language.probes.is_empty())
    })
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
    labels.extend(super::managed::managed_aspects());
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
            labels
                .contains(&"@rules_dx//core/quality:families/core.bzl%real_lint_aspect".to_owned())
        );
        assert!(labels.contains(
            &"@rules_dx//core/quality:families/core.bzl%real_typecheck_aspect".to_owned()
        ));
        assert!(labels.contains(&dx_codegen::aspect_label(super::super::rules_dx_repo())));
        assert!(labels.contains(&dx_env_plan::aspect_label(super::super::rules_dx_repo())));
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(labels, sorted);
    }

    #[test]
    fn labels_point_at_the_file_that_defines_the_aspect() {
        use clap::ValueEnum;
        for command in Command::value_variants() {
            let entry = spec(*command);
            for label in entry.aspects {
                let (file, _) = label.rsplit_once('%').expect("label has a name");
                if file.starts_with("tools/docs") {
                    continue;
                }
                assert!(
                    file.starts_with("core/quality:families/")
                        || file.contains("/private/quality:"),
                    "{label}"
                );
            }
        }
    }

    fn extensions(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn consumer(extensions: &BTreeSet<String>) -> Consumer<'_> {
        Consumer { extensions }
    }

    #[test]
    fn probes_for_lists_the_core_probe_and_the_present_families_probes() {
        let extensions = extensions(&[".py", ".rs", ".swift"]);
        let probes = probes_for(Command::Lint, consumer(&extensions));
        assert!(
            probes.contains(&"core/quality:families/core_probe.bzl%real_core_lint_probe_aspect")
        );
        assert!(
            probes.contains(&"python/private/quality:python.bzl%real_python_lint_probe_aspect")
        );
        assert!(probes.contains(&"rust/private/quality:rust.bzl%real_rust_lint_probe_aspect"));
        assert!(probes.contains(&"swift/private/quality:swift.bzl%real_swift_lint_probe_aspect"));
        assert_eq!(
            probes_for(Command::Docs, consumer(&extensions)),
            [] as [&str; 0]
        );
    }

    #[test]
    fn go_only_consumer_docs_selects_the_core_aspect_and_go() {
        let extensions = extensions(&[".go", ".json", ".md"]);
        let labels = aspects_for(Command::Docs, consumer(&extensions));
        assert_eq!(
            labels,
            [
                "tools/docs:aspect.bzl%real_docs_aspect",
                "tools/docs/go:aspect.bzl%real_go_docs_aspect"
            ]
        );
    }

    #[test]
    fn java_only_consumer_docs_select_only_the_java_aspect() {
        let extensions = extensions(&[".java"]);
        let labels = aspects_for(Command::Docs, consumer(&extensions));
        assert_eq!(
            labels,
            [
                "tools/docs:aspect.bzl%real_docs_aspect",
                "tools/docs/jvm:aspect.bzl%real_jvm_docs_aspect"
            ]
        );
    }

    #[test]
    fn scala_and_kotlin_consumer_docs_select_their_own_aspects() {
        let scala = extensions(&[".scala"]);
        let labels = aspects_for(Command::Docs, consumer(&scala));
        assert!(labels.contains(&"tools/docs/scala:aspect.bzl%real_scala_docs_aspect"));
        assert!(!labels.contains(&"tools/docs/jvm:aspect.bzl%real_jvm_docs_aspect"));
        let kotlin = extensions(&[".kt"]);
        let labels = aspects_for(Command::Docs, consumer(&kotlin));
        assert!(labels.contains(&"tools/docs/kotlin:aspect.bzl%real_kotlin_docs_aspect"));
        assert!(!labels.contains(&"tools/docs/jvm:aspect.bzl%real_jvm_docs_aspect"));
    }

    #[test]
    fn a_language_sharing_the_core_file_does_not_gate_it() {
        let extensions = extensions(&[".go"]);
        let lint = aspects_for(Command::Lint, consumer(&extensions));
        assert!(lint.contains(&"core/quality:families/core.bzl%real_lint_aspect"));
        assert!(quality_owner("core/quality:families/core.bzl").is_none());
    }

    #[test]
    fn go_only_consumer_selects_core_and_go() {
        let extensions = extensions(&[".go", ".json", ".md"]);
        for command in [Command::Lint, Command::Typecheck, Command::Format] {
            let entry = spec(command);
            let labels = aspects_for(command, consumer(&extensions));
            let probes = probes_for(command, consumer(&extensions));
            for label in labels.iter().chain(&probes) {
                let file = label.split_once('%').expect("label has a name").0;
                let language = quality_owner(file);
                assert!(
                    language.is_none_or(|language| language.bzl == "go/private/quality:go.bzl"
                        || language.file_extensions.iter().any(|ext| ext == ".json")),
                    "{command:?}: {label}"
                );
            }
            assert!(
                labels
                    .iter()
                    .any(|label| label.contains("go/private/quality:go.bzl"))
            );
            assert!(
                labels
                    .iter()
                    .any(|label| label.contains("families/core.bzl"))
            );
            assert!(labels.len() < entry.aspects.len());
        }
        let probes = probes_for(Command::Lint, consumer(&extensions));
        assert_eq!(
            probes,
            vec![
                "core/quality:families/core_probe.bzl%real_core_lint_probe_aspect",
                "cloudformation/private/quality:cloudformation.bzl%real_cloudformation_lint_probe_aspect",
                "go/private/quality:go.bzl%real_go_lint_probe_aspect",
                "kubernetes/private/quality:kubernetes.bzl%real_kubernetes_lint_probe_aspect",
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
            sorted.sort_unstable();
            let before = sorted.len();
            sorted.dedup();
            assert_eq!(before, sorted.len(), "{command:?} lists a label twice");
        }
    }

    #[test]
    fn core_table_lists_only_core_files() {
        let labels: Vec<String> = [
            ("lint", false),
            ("lint", true),
            ("typecheck", false),
            ("typecheck", true),
            ("format", false),
            ("format", true),
            ("docs", false),
        ]
        .into_iter()
        .flat_map(|(command, probes)| core_labels(command, probes))
        .collect();
        assert!(labels.len() > 20, "{labels:?}");
        for label in &labels {
            assert!(
                label.starts_with("core/quality:families/") || label.starts_with("tools/docs:"),
                "{label} belongs in its lang.json quality entry"
            );
        }
    }

    #[test]
    fn languages_without_their_files_are_not_selected() {
        let extensions = extensions(&[".go"]);
        let lint = aspects_for(Command::Lint, consumer(&extensions));
        assert!(
            !lint
                .contains(&"kubernetes/private/quality:kubernetes.bzl%real_kubernetes_lint_aspect")
        );
        assert!(
            !lint
                .contains(&"dockerfile/private/quality:dockerfile.bzl%real_dockerfile_lint_aspect")
        );
        assert!(!lint.contains(&"make/private/quality:make.bzl%real_make_lint_aspect"));
        assert!(!lint.contains(&"sql/private/quality:sql.bzl%real_sql_lint_aspect"));
    }

    #[test]
    fn a_yaml_file_selects_the_yaml_languages() {
        let yaml = extensions(&[".yaml"]);
        let lint = aspects_for(Command::Lint, consumer(&yaml));
        for label in [
            "ansible/private/quality:ansible.bzl%real_ansible_lint_aspect",
            "cloudformation/private/quality:cloudformation.bzl%real_cloudformation_lint_aspect",
            "kubernetes/private/quality:kubernetes.bzl%real_kubernetes_lint_aspect",
        ] {
            assert!(lint.contains(&label), "{label}");
        }
    }

    #[test]
    fn a_file_name_selects_its_language() {
        let make = extensions(&["Makefile"]);
        let lint = aspects_for(Command::Lint, consumer(&make));
        assert!(lint.contains(&"make/private/quality:make.bzl%real_make_lint_aspect"));
        let cmake = extensions(&["CMakeLists.txt"]);
        let format = aspects_for(Command::Format, consumer(&cmake));
        assert!(format.contains(&"cmake/private/quality:cmake.bzl%real_cmake_format_aspect"));
        for mark in ["Dockerfile", "Containerfile", ".dockerfile"] {
            let docker = extensions(&[mark]);
            let lint = aspects_for(Command::Lint, consumer(&docker));
            assert!(lint.contains(
                &"dockerfile/private/quality:dockerfile.bzl%real_dockerfile_lint_aspect"
            ));
        }
    }

    #[test]
    fn a_language_outside_the_core_table_is_selected_by_its_files() {
        for (extension, label) in [
            (
                ".bicep",
                "bicep/private/quality:bicep.bzl%real_bicep_lint_aspect",
            ),
            (
                ".rego",
                "rego/private/quality:rego.bzl%real_rego_lint_aspect",
            ),
            (".sql", "sql/private/quality:sql.bzl%real_sql_lint_aspect"),
            (
                ".tf",
                "terraform/private/quality:terraform.bzl%real_terraform_lint_aspect",
            ),
        ] {
            let extensions = extensions(&[extension]);
            let lint = aspects_for(Command::Lint, consumer(&extensions));
            assert!(lint.contains(&label), "{label}");
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
                if quality_owner(&bzl).is_none() {
                    continue;
                }
                let probe = format!(
                    "core/quality:families/{file}.bzl%real_{file}_{capability}_probe_aspect"
                );
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
        let command = match entry.command {
            Command::Lint => "lint",
            Command::Typecheck => "typecheck",
            Command::Security
            | Command::License
            | Command::Format
            | Command::Generate
            | Command::Build
            | Command::Test
            | Command::Coverage
            | Command::Bench
            | Command::Run
            | Command::Deploy
            | Command::Check
            | Command::Clean
            | Command::Update
            | Command::Sync
            | Command::Init
            | Command::Upgrade
            | Command::Status
            | Command::Watch
            | Command::Completion
            | Command::Docs
            | Command::Worktree
            | Command::Fuzz
            | Command::Config
            | Command::Fetch => "format",
        };
        let core = (core_labels(command, false), core_labels(command, true));
        assert_eq!(&entry.aspects[..core.0.len()], core.0);
        assert_eq!(&entry.probes[..core.1.len()], core.1);
        let language_aspects: Vec<&str> = entry.aspects[core.0.len()..].to_vec();
        assert_ne!(language_aspects, [] as [&str; 0]);
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
        assert_eq!(format.reports, []);
        let build = spec(Command::Build);
        assert_eq!(build.aspects, [] as [String; 0]);
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
        assert!(
            docs.aspects
                .contains(&"tools/docs/go:aspect.bzl%real_go_docs_aspect")
        );
        assert_eq!(docs.probes, [] as [String; 0]);
        assert_eq!(docs.reports, []);
        let run = spec(Command::Run);
        assert_eq!(run.aspects, [] as [String; 0]);
        assert_eq!(run.reports, []);
        let generate = spec(Command::Generate);
        assert_eq!(generate.aspects, [] as [String; 0]);
        assert_eq!(generate.reports, []);
        let clean = spec(Command::Clean);
        assert_eq!(clean.aspects, [] as [String; 0]);
        assert_eq!(clean.reports, []);
        let env = spec(Command::Sync);
        assert_eq!(env.aspects, [] as [String; 0]);
        assert_eq!(env.reports, []);
        assert!(Command::Sync.is_managed());
        for command in [Command::Security, Command::License] {
            let entry = spec(command);
            assert_eq!(entry.aspects, [] as [String; 0]);
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
        assert_eq!(update.aspects, [] as [String; 0]);
        assert_eq!(update.reports, []);
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
