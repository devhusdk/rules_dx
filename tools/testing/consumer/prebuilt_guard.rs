//! The rules shared by the prebuilt guards: the allowlist, the compiled rules_dx binaries of a
//! `cquery --output=label_kind`, the `rules_dx` targets dx commands run and the hub resolvers, the aspects `dx check` and `dx docs` pick, the wrapper
//! package whose `deps` carry them, and the `file://` URL of a stub.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dx_plan::args::Command;
use dx_plan::plan::registry::{Consumer, aspects_for};

/// The rule kind prefixes of compiled binaries; shell stubs and `native_binary` stay out.
pub const COMPILED: &[&str] = &[
    "cc_", "gazelle_", "go_", "haskell_", "java_", "kt_jvm_", "rust_", "scala_",
];

/// The target name crate_universe gives a crate's build script binary.
pub const BUILD_SCRIPT: &str = ":_bs_";

/// The repository of the `dx_prebuilt` hub.
pub const PREBUILT_HUB_REPO: &str = "rules_dx++dx_prebuilt+dx_prebuilt";

/// The allowlist: binary labels the consumer may compile, each with the consumer languages it
/// applies to (none: every language), and registry tools no hub takes as an asset tag.
#[derive(Debug, Default, PartialEq)]
pub struct Allowlist {
    pub labels: BTreeMap<String, BTreeSet<String>>,
    pub untagged: BTreeSet<String>,
}

impl Allowlist {
    /// The labels a consumer of `langs` may compile.
    pub fn labels_for(&self, langs: &[&str]) -> BTreeSet<String> {
        self.labels
            .iter()
            .filter(|(_, only)| only.is_empty() || langs.iter().any(|lang| only.contains(*lang)))
            .map(|(label, _)| label.clone())
            .collect()
    }
}

/// Reads `label <canonical label> [<lang>...]` and `untagged <tool>` lines; each needs a `#` reason.
pub fn allowlist(text: &str) -> Result<Allowlist, String> {
    let mut list = Allowlist::default();
    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (entry, reason) = line.split_once('#').unwrap_or((line, ""));
        if reason.trim().is_empty() {
            return Err(format!("line {}: no `# reason`: {line}", number + 1));
        }
        match entry.split_whitespace().collect::<Vec<_>>()[..] {
            ["label", label, ref langs @ ..] => {
                list.labels.insert(
                    label.to_owned(),
                    langs.iter().map(|lang| (*lang).to_owned()).collect(),
                );
            }
            ["untagged", tool] => {
                list.untagged.insert(tool.to_owned());
            }
            _ => {
                return Err(format!(
                    "line {}: want `label <label> [<lang>...]` or `untagged <tool>`: {line}",
                    number + 1
                ));
            }
        }
    }
    Ok(list)
}

/// The rules_dx-declared compiled binaries in `cquery --output=label_kind` lines, each with its lines.
pub fn compiled_binaries(cquery: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in cquery.lines() {
        let Some((kind, rest)) = line.split_once(" rule ") else {
            continue;
        };
        let bare = kind.trim().trim_start_matches('_');
        if !bare.ends_with("_binary") || !COMPILED.iter().any(|prefix| bare.starts_with(prefix)) {
            continue;
        }
        let label = rest.split_whitespace().next().unwrap_or_default();
        // A crate build script comes with the crate's rust_library; the tool that pulls the
        // crate in is reported on its own, so the scripts would only bury it.
        if label.ends_with(BUILD_SCRIPT) {
            continue;
        }
        let repo = label
            .trim_start_matches('@')
            .split("//")
            .next()
            .unwrap_or_default();
        if bare == "java_binary" && repo == PREBUILT_HUB_REPO {
            continue;
        }
        if repo.starts_with("rules_dx+") {
            found
                .entry(label.to_owned())
                .or_default()
                .insert(line.trim().to_owned());
        }
    }
    found
}

/// The `srcs` query whose labels are the source-compiled JVM rules `label` depends on.
pub fn jvm_sources_query(label: &str) -> String {
    format!("attr(srcs, \"\\[.+\\]\", kind(\"(java|kt_jvm|scala)_\", deps({label})))")
}

/// Whether `cquery --output=label` lines of [`jvm_sources_query`] name a rules_dx rule.
pub fn compiles_rules_dx_sources(cquery: &str) -> bool {
    cquery
        .lines()
        .any(|line| line.trim().trim_start_matches('@').starts_with("rules_dx+"))
}

/// Whether every line of a found binary is a `java_binary`, the one kind that may only launch jars.
pub fn java_binary_only(lines: &BTreeSet<String>) -> bool {
    lines
        .iter()
        .all(|line| line.trim_start_matches('_').starts_with("java_binary "))
}

/// The problems: compiled binaries not allowlisted, and allowlisted labels no longer compiled.
pub fn problems(
    found: &BTreeMap<String, BTreeSet<String>>,
    allowed: &BTreeSet<String>,
) -> Vec<String> {
    let mut problems = unlisted(found, allowed);
    problems.extend(stale(found, allowed));
    problems
}

/// One problem per line of a compiled binary that `allowed` leaves out.
pub fn unlisted(
    found: &BTreeMap<String, BTreeSet<String>>,
    allowed: &BTreeSet<String>,
) -> Vec<String> {
    found
        .iter()
        .filter(|(label, _)| !allowed.contains(*label))
        .flat_map(|(_, lines)| lines.iter().map(|line| format!("compiles {line}")))
        .collect()
}

/// One problem per allowlisted label that `found` no longer holds.
pub fn stale(
    found: &BTreeMap<String, BTreeSet<String>>,
    allowed: &BTreeSet<String>,
) -> Vec<String> {
    allowed
        .iter()
        .filter(|label| !found.contains_key(*label))
        .map(|label| format!("no longer compiled, remove it from the allowlist: {label}"))
        .collect()
}

/// `label` as a consumer of rules_dx names it, for a label of rules_dx as the root module:
/// `fixtures`, the canonical repo of the consumer fixtures, is the consumer's main repo, the main
/// repo is `rules_dx+`, and a repo of a root-module extension or repo rule is under `rules_dx+`.
pub fn consumer_label(label: &str, fixtures: &str) -> String {
    if let Some(rest) = label.strip_prefix(&format!("@@{fixtures}//")) {
        format!("//{rest}")
    } else if let Some(rest) = label
        .strip_prefix("@@//")
        .or_else(|| label.strip_prefix("//"))
    {
        format!("@@rules_dx+//{rest}")
    } else if let Some(rest) = label.strip_prefix("@@+") {
        format!("@@rules_dx++{rest}")
    } else {
        label.to_owned()
    }
}

/// A `cquery --output=label_kind` line with its label mapped by `consumer_label`.
pub fn consumer_line(line: &str, fixtures: &str) -> String {
    let Some((kind, rest)) = line.split_once(" rule ") else {
        return line.to_owned();
    };
    let (label, tail) = rest.split_once(' ').unwrap_or((rest, ""));
    format!("{kind} rule {} {tail}", consumer_label(label, fixtures))
        .trim_end()
        .to_owned()
}

/// The aspects `dx check` (format, lint, typecheck) and `dx docs` pass for a consumer with
/// `extensions`, each list in command order and without repeats.
pub fn guard_aspects(extensions: &BTreeSet<String>) -> (Vec<&'static str>, Vec<&'static str>) {
    let consumer = Consumer { extensions };
    let mut check: Vec<&'static str> = Vec::new();
    for command in [Command::Format, Command::Lint, Command::Typecheck] {
        for aspect in aspects_for(command, consumer) {
            if !check.contains(&aspect) {
                check.push(aspect);
            }
        }
    }
    (check, aspects_for(Command::Docs, consumer))
}

/// The `.bzl` file of the `dx_prebuilt_guard` package: rules `check_root` and `docs_root`, whose
/// `deps` carry the `check` and `docs` aspects (`<package>:<file>%<name>` labels in rules_dx, the
/// repo `rules_dx` names).
pub fn wrapper_bzl(rules_dx: &str, check: &[&str], docs: &[&str]) -> String {
    let mut loads = String::new();
    let mut rules = String::from("def _root_impl(_ctx):\n    return []\n");
    for (prefix, aspects) in [("check", check), ("docs", docs)] {
        let mut names = Vec::new();
        for (index, aspect) in aspects.iter().enumerate() {
            let (file, name) = aspect.split_once('%').unwrap_or((aspect, ""));
            let alias = format!("{prefix}_{index}");
            loads.push_str(&format!(
                "load(\"{rules_dx}//{file}\", {alias} = \"{name}\")\n"
            ));
            names.push(alias);
        }
        rules.push_str(&format!(
            "\n{prefix}_root = rule(\n    implementation = _root_impl,\n    attrs = {{\"deps\": attr.label_list(aspects = [{}])}},\n)\n",
            names.join(", ")
        ));
    }
    format!("{loads}\n{rules}")
}

/// The `repo` targets dx commands run for every language and for `langs`, as cquery roots.
pub fn command_roots(repo: &str, langs: &[&str]) -> Vec<String> {
    dx_families::COMMAND_TARGETS
        .iter()
        .copied()
        .chain(
            langs
                .iter()
                .flat_map(|lang| dx_families::language_commands(lang)),
        )
        .map(|label| format!("{repo}//{}", dx_families::repo_relative(label)))
        .collect()
}

/// The `setup.hubs[].resolver` labels of `langs`, which `dx update` and `dx generate` run, as
/// cquery roots.
pub fn resolver_roots(langs: &[&str]) -> Vec<String> {
    let roots: BTreeSet<String> = langs
        .iter()
        .flat_map(|lang| dx_families::resolvers(lang))
        .collect();
    roots.into_iter().collect()
}

/// The `file://` URL of `path`, with `/` separators and `file:///C:/` for a drive path.
pub fn stub_url(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

/// The `BUILD.bazel` of the `dx_prebuilt_guard` package: `check` and `docs` over `targets`.
pub fn wrapper_build(targets: &[&str]) -> String {
    let deps: String = targets
        .iter()
        .map(|target| format!("        \"{target}\",\n"))
        .collect();
    let mut build = String::from("load(\":defs.bzl\", \"check_root\", \"docs_root\")\n");
    for name in ["check", "docs"] {
        build.push_str(&format!(
            "\n{name}_root(\n    name = \"{name}\",\n    testonly = True,\n    deps = [\n{deps}    ],\n)\n"
        ));
    }
    build
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_java_binary_without_rules_dx_sources_launches_jars() {
        assert_eq!(
            jvm_sources_query("@@rules_dx+//t:detekt"),
            "attr(srcs, \"\\[.+\\]\", kind(\"(java|kt_jvm|scala)_\", deps(@@rules_dx+//t:detekt)))"
        );
        assert!(java_binary_only(&BTreeSet::from([
            "java_binary rule @@rules_dx+//t:detekt (abc)".to_owned(),
            "java_binary rule @@rules_dx+//t:detekt (exec)".to_owned(),
        ])));
        assert!(!java_binary_only(&BTreeSet::from([
            "rust_binary rule @@rules_dx+//cli/cli:dx (abc)".to_owned(),
        ])));
        assert!(!compiles_rules_dx_sources(
            "@@rules_java+//toolchains:singlejar (exec)\n"
        ));
        assert!(compiles_rules_dx_sources(
            "@@rules_dx+//java/private/quality/tools/javac_check:javac_check_lib (abc)\n"
        ));
    }

    #[test]
    fn stub_url_has_three_slashes_before_a_drive() {
        assert_eq!(
            stub_url(Path::new(r"C:\t\stubs\go")),
            "file:///C:/t/stubs/go"
        );
        assert_eq!(
            stub_url(Path::new(r"C:/t/_tmp\stubs\go")),
            "file:///C:/t/_tmp/stubs/go"
        );
        assert_eq!(stub_url(Path::new("/tmp/stubs/go")), "file:///tmp/stubs/go");
    }

    #[test]
    fn command_roots_name_the_shared_and_language_targets() {
        assert_eq!(
            command_roots("@rules_dx", &["go", "rust"]),
            [
                "@rules_dx//dx:generate",
                "@rules_dx//dx:generate_check",
                "@rules_dx//deploy/private/release:cosign",
                "@rules_dx//core/quality/artifacts:gitleaks",
                "@rules_dx//core/quality/artifacts:trivy",
                "@rules_dx//:dx",
                "@rules_dx//go/private/tools:go",
            ]
        );
        assert_eq!(
            command_roots("", &[]).last().map(String::as_str),
            Some("//:dx")
        );
    }

    #[test]
    fn resolver_roots_name_the_hub_resolvers_of_the_languages() {
        assert_eq!(
            resolver_roots(&["go", "zig", "d", "zig"]),
            [
                "@rules_dx//d/private/tools/lock",
                "@rules_dx//zig/private/tools/lock"
            ]
        );
        assert!(resolver_roots(&["go", "kotlin"]).is_empty());
    }

    #[test]
    fn allowlist_lines_need_a_kind_and_a_reason() {
        let list = allowlist(
            "# header\nlabel @@rules_dx+//a:b  # why\nlabel @@rules_dx+//l:c lua  # lua only\nuntagged avro_union # jar\n\n",
        )
        .expect("parses");
        assert_eq!(
            list,
            Allowlist {
                labels: BTreeMap::from([
                    ("@@rules_dx+//a:b".to_owned(), BTreeSet::new()),
                    (
                        "@@rules_dx+//l:c".to_owned(),
                        BTreeSet::from(["lua".to_owned()])
                    ),
                ]),
                untagged: BTreeSet::from(["avro_union".to_owned()]),
            }
        );
        assert_eq!(
            list.labels_for(&["go"]),
            BTreeSet::from(["@@rules_dx+//a:b".to_owned()])
        );
        assert_eq!(
            list.labels_for(&["lua"]),
            BTreeSet::from(["@@rules_dx+//a:b".to_owned(), "@@rules_dx+//l:c".to_owned()])
        );
        assert_eq!(
            list.labels_for(&["go", "lua"]),
            BTreeSet::from(["@@rules_dx+//a:b".to_owned(), "@@rules_dx+//l:c".to_owned()])
        );
        assert!(allowlist("label @@rules_dx+//a:b\n").is_err());
        assert!(allowlist("tool x # why\n").is_err());
    }

    #[test]
    fn only_rules_dx_compiled_binaries_count() {
        let cquery = "\
    rust_binary rule @@rules_dx+//cli/cli:dx (abc)
    go_binary rule @@rules_dx++dx_gazelle+dx_gazelle//:gazelle (exec)
    native_binary rule @@rules_dx++dx_prebuilt+dx_prebuilt//:selene_linux_x86_64 (abc)
    sh_binary rule @@rules_dx+//tools:stub (abc)
    go_binary rule @@rules_go+//go/tools/builders:builder (exec)
    go_library rule @@rules_dx+//gazelle/go:go (abc)
    rust_binary rule @@rules_dx++crate+dx_crates__anyhow-1.0.104//:_bs_ (abc)
    java_binary rule @@rules_dx++dx_prebuilt+dx_prebuilt//:javac_check (exec)
    java_binary rule @@rules_dx+//java/private/quality/tools/javac_check:javac_check_upstream (exec)
    source file @@rules_dx+//cli/cli:main.rs (null)
    ";
        let found = compiled_binaries(cquery);
        assert_eq!(
            found.keys().cloned().collect::<Vec<_>>(),
            [
                "@@rules_dx++dx_gazelle+dx_gazelle//:gazelle",
                "@@rules_dx+//cli/cli:dx",
                "@@rules_dx+//java/private/quality/tools/javac_check:javac_check_upstream",
            ]
        );
        let allowed = BTreeSet::from([
            "@@rules_dx+//cli/cli:dx".to_owned(),
            "@@rules_dx+//gone:tool".to_owned(),
        ]);
        assert_eq!(
            problems(&found, &allowed),
            [
                "compiles go_binary rule @@rules_dx++dx_gazelle+dx_gazelle//:gazelle (exec)",
                "compiles java_binary rule @@rules_dx+//java/private/quality/tools/javac_check:javac_check_upstream (exec)",
                "no longer compiled, remove it from the allowlist: @@rules_dx+//gone:tool",
            ]
        );
    }

    #[test]
    fn the_wrapper_carries_each_aspect_on_its_deps() {
        let bzl = wrapper_bzl(
            "@rules_dx",
            &["go/private/quality:go.bzl%real_go_lint_aspect"],
            &["tools/docs/go:aspect.bzl%real_go_docs_aspect"],
        );
        assert_eq!(
            bzl,
            "load(\"@rules_dx//go/private/quality:go.bzl\", check_0 = \"real_go_lint_aspect\")\n\
             load(\"@rules_dx//tools/docs/go:aspect.bzl\", docs_0 = \"real_go_docs_aspect\")\n\
             \n\
             def _root_impl(_ctx):\n    return []\n\
             \n\
             check_root = rule(\n    implementation = _root_impl,\n    attrs = {\"deps\": attr.label_list(aspects = [check_0])},\n)\n\
             \n\
             docs_root = rule(\n    implementation = _root_impl,\n    attrs = {\"deps\": attr.label_list(aspects = [docs_0])},\n)\n"
        );
        let build = wrapper_build(&["//a:b"]);
        assert!(build.contains(
            "    name = \"check\",\n    testonly = True,\n    deps = [\n        \"//a:b\",\n    ],"
        ));
        assert!(build.contains("docs_root(\n    name = \"docs\""));
    }

    #[test]
    fn guard_aspects_follow_the_consumer_languages() {
        let go = BTreeSet::from([".go".to_owned()]);
        let (check, docs) = guard_aspects(&go);
        assert!(check.contains(&"go/private/quality:go.bzl%real_go_lint_aspect"));
        assert!(docs.contains(&"tools/docs/go:aspect.bzl%real_go_docs_aspect"));
        assert!(!check.iter().chain(&docs).any(|label| label.contains("jvm")));
        let java = BTreeSet::from([".java".to_owned()]);
        let (check, docs) = guard_aspects(&java);
        assert!(
            check
                .iter()
                .any(|label| label.ends_with("%real_jvm_typecheck_aspect"))
        );
        assert!(
            docs.iter()
                .any(|label| label.ends_with("%real_jvm_docs_aspect"))
        );
    }

    #[test]
    fn root_mode_labels_map_to_consumer_labels() {
        let fixtures = "+_repo_rules+dx_api_fixtures";
        assert_eq!(
            consumer_label(
                "@@+_repo_rules+dx_api_fixtures//rego/tests/consumer:greet",
                fixtures
            ),
            "//rego/tests/consumer:greet"
        );
        assert_eq!(
            consumer_label("@@//cli/runtime:dx_runtime", fixtures),
            "@@rules_dx+//cli/runtime:dx_runtime"
        );
        assert_eq!(
            consumer_label("//cli/cli:dx", fixtures),
            "@@rules_dx+//cli/cli:dx"
        );
        assert_eq!(
            consumer_label("@@+dx_prebuilt+dx_prebuilt//:javac_check", fixtures),
            "@@rules_dx++dx_prebuilt+dx_prebuilt//:javac_check"
        );
        assert_eq!(
            consumer_label("@@rules_go+//go/tools/builders:builder", fixtures),
            "@@rules_go+//go/tools/builders:builder"
        );
        assert_eq!(
            consumer_line(
                "rust_binary rule @@//cli/runtime:dx_runtime (abc)",
                fixtures
            ),
            "rust_binary rule @@rules_dx+//cli/runtime:dx_runtime (abc)"
        );
        assert_eq!(
            consumer_line("source file @@//a:b.rs (null)", fixtures),
            "source file @@//a:b.rs (null)"
        );
        let found = compiled_binaries(&consumer_line(
            "java_binary rule @@+dx_prebuilt+dx_prebuilt//:javac_check (exec)",
            fixtures,
        ));
        assert!(found.is_empty());
    }

    #[test]
    fn stale_and_unlisted_split_the_problems() {
        let found = BTreeMap::from([(
            "@@rules_dx+//a:b".to_owned(),
            BTreeSet::from(["rust_binary rule @@rules_dx+//a:b (abc)".to_owned()]),
        )]);
        let allowed = BTreeSet::from(["@@rules_dx+//gone:tool".to_owned()]);
        assert_eq!(
            unlisted(&found, &allowed),
            ["compiles rust_binary rule @@rules_dx+//a:b (abc)"]
        );
        assert_eq!(
            stale(&found, &allowed),
            ["no longer compiled, remove it from the allowlist: @@rules_dx+//gone:tool"]
        );
    }
}
