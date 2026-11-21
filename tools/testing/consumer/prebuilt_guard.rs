//! The rules shared by the prebuilt guards: the allowlist, the compiled rules_dx binaries of a
//! `cquery --output=label_kind`, the `rules_dx` targets dx commands run and the hub resolvers, the aspects `dx check` and `dx docs` pick, the wrapper
//! package whose `deps` carry them, the `file://` URL of a stub, and the stub `python_tools` bundle.

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
/// applies to (none: every language).
#[derive(Debug, Default, PartialEq)]
pub struct Allowlist {
    pub labels: BTreeMap<String, BTreeSet<String>>,
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

/// Reads `label <canonical label> [<lang>...]` lines; each needs a `#` reason.
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
            _ => {
                return Err(format!(
                    "line {}: want `label <label> [<lang>...]`: {line}",
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

/// Drops from `found` each `java_binary` whose `sources` cquery output, for its label, compiles no
/// rules_dx source.
pub fn drop_jar_launchers(
    found: &mut BTreeMap<String, BTreeSet<String>>,
    mut sources: impl FnMut(&str) -> Result<String, String>,
) -> Result<(), String> {
    let mut launchers = Vec::new();
    for (label, lines) in found.iter() {
        if java_binary_only(lines) && !compiles_rules_dx_sources(&sources(label)?) {
            launchers.push(label.clone());
        }
    }
    for label in launchers {
        found.remove(&label);
    }
    Ok(())
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

/// `label` as rules_dx as the root module names it; the inverse of [`consumer_label`] for rules_dx
/// labels.
pub fn root_label(label: &str) -> String {
    if let Some(rest) = label.strip_prefix("@@rules_dx+//") {
        format!("@@//{rest}")
    } else if let Some(rest) = label.strip_prefix("@@rules_dx++") {
        format!("@@+{rest}")
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
        .copied()
        .flat_map(dx_families::resolvers)
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

/// One tool of the `python_tools` bundle: its hub name, its `bin/` launcher, and the platform
/// keys whose bundle leaves it out.
#[derive(Debug, PartialEq)]
pub struct BundleTool {
    pub name: String,
    pub launcher: String,
    pub missing: BTreeSet<String>,
}

impl BundleTool {
    /// The canonical label of the hub target that serves the tool.
    pub fn hub_target(&self) -> String {
        format!("@@{PREBUILT_HUB_REPO}//:python_tool_{}", self.name)
    }
}

/// Reads `<name> <launcher> [<missing platform key>...]` lines of `//cli/prebuilt:python_hub_targets`.
pub fn bundle_tools(text: &str) -> Result<Vec<BundleTool>, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(
            |line| match line.split_whitespace().collect::<Vec<_>>()[..] {
                [name, launcher, ref missing @ ..] => Ok(BundleTool {
                    name: name.to_owned(),
                    launcher: launcher.to_owned(),
                    missing: missing.iter().map(|key| (*key).to_owned()).collect(),
                }),
                _ => Err(format!("want `<name> <launcher> [<missing>...]`: {line}")),
            },
        )
        .collect()
}

/// A `tar.gz` with an executable `bin/<launcher>` stub per tool, plus `bin/<launcher>.cmd` on
/// Windows, where the hub runs the `.cmd` launcher.
pub fn stub_bundle(tools: &[BundleTool], windows: bool) -> std::io::Result<Vec<u8>> {
    let mut tar = dx_archive::TarWriter::new();
    for tool in tools {
        let launcher = format!("bin/{}", tool.launcher);
        tar.add_file(&launcher, format!("stub {}\n", tool.name).as_bytes(), 0o755)?;
        if windows {
            tar.add_file(
                &format!("{launcher}.cmd"),
                format!("@echo stub {}\r\n", tool.name).as_bytes(),
                0o755,
            )?;
        }
    }
    tar.finish()
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

/// The `@platforms` os and cpu of a registry platform key such as `windows_arm64`.
pub fn release_platform(key: &str) -> Option<(&str, &str)> {
    let (os, cpu) = key.split_once('_')?;
    Some((os, if cpu == "arm64" { "aarch64" } else { cpu }))
}

/// The `.bzl` file of the `dx_target_guard` package: rule `target_root`, whose `deps` are split
/// over `<rules_dx>//tools/platforms:<key>` for each of its `keys`.
pub fn target_guard_bzl(rules_dx: &str) -> String {
    format!(
        "_PLATFORMS = Label(\"{rules_dx}//tools/platforms:BUILD.bazel\")\n\n\
         def _platforms_impl(_settings, attr):\n    return {{key: {{\"//command_line_option:platforms\": [str(_PLATFORMS.same_package_label(key))]}} for key in attr.keys}}\n\n\
         _platforms = transition(\n    implementation = _platforms_impl,\n    inputs = [],\n    outputs = [\"//command_line_option:platforms\"],\n)\n\n\
         def _root_impl(_ctx):\n    return []\n\n\
         target_root = rule(\n    implementation = _root_impl,\n    attrs = {{\n        \"keys\": attr.string_list(),\n        \"deps\": attr.label_list(cfg = _platforms),\n    }},\n)\n"
    )
}

/// One target-platform tool of the `dx_target_guard` package: its registry keys and hub targets.
pub struct TargetRoot {
    pub tool: String,
    pub keys: Vec<String>,
    pub targets: Vec<String>,
}

/// The `BUILD.bazel` of the `dx_target_guard` package: one `target_root` per tool, and `targets`
/// over all of them.
pub fn target_guard_build(roots: &[TargetRoot]) -> String {
    let quoted = |items: &[String]| -> String {
        items
            .iter()
            .map(|item| format!("        \"{item}\",\n"))
            .collect()
    };
    let mut build = String::from("load(\":defs.bzl\", \"target_root\")\n");
    for root in roots {
        build.push_str(&format!(
            "\ntarget_root(\n    name = \"{}\",\n    testonly = True,\n    keys = [\n{}    ],\n    deps = [\n{}    ],\n)\n",
            root.tool,
            quoted(&root.keys),
            quoted(&root.targets)
        ));
    }
    let names: Vec<String> = roots.iter().map(|root| format!(":{}", root.tool)).collect();
    build.push_str(&format!(
        "\nfilegroup(\n    name = \"targets\",\n    testonly = True,\n    srcs = [\n{}    ],\n)\n",
        quoted(&names)
    ));
    build
}

/// The canonical name of the repo that downloads `tool` for the registry platform `key`.
pub fn target_repo(tool: &str, key: &str) -> Option<String> {
    let (os, cpu) = release_platform(key)?;
    Some(format!(
        "rules_dx++dx_prebuilt+dx_{tool}_prebuilt_{os}_{cpu}"
    ))
}

/// One line per (tool, key) of `roots` whose download repo no target in the `label_kind` cquery
/// output comes from.
pub fn missing_target_repos(cquery: &str, roots: &[TargetRoot]) -> Vec<String> {
    let repos: BTreeSet<&str> = cquery
        .lines()
        .filter_map(|line| {
            line.split_whitespace()
                .nth(2)?
                .strip_prefix("@@")?
                .split_once("//")
        })
        .map(|(repo, _)| repo)
        .collect();
    roots
        .iter()
        .flat_map(|root| {
            root.keys
                .iter()
                .map(move |key| (root.tool.as_str(), key.as_str()))
        })
        .filter_map(|(tool, key)| match target_repo(tool, key) {
            Some(repo) if repos.contains(repo.as_str()) => None,
            Some(repo) => Some(format!("{tool} on {key} does not select @@{repo}")),
            None => Some(format!("{tool}: {key} is no registry platform key")),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_keys_map_to_platforms_constraints() {
        assert_eq!(release_platform("linux_x86_64"), Some(("linux", "x86_64")));
        assert_eq!(
            release_platform("windows_arm64"),
            Some(("windows", "aarch64"))
        );
        assert_eq!(release_platform("any"), None);
        assert_eq!(
            target_repo("runtime", "macos_arm64").as_deref(),
            Some("rules_dx++dx_prebuilt+dx_runtime_prebuilt_macos_aarch64")
        );
    }

    #[test]
    fn the_target_guard_splits_each_tool_over_its_keys() {
        let bzl = target_guard_bzl("@rules_dx");
        assert!(
            bzl.starts_with("_PLATFORMS = Label(\"@rules_dx//tools/platforms:BUILD.bazel\")\n")
        );
        assert!(bzl.contains("[str(_PLATFORMS.same_package_label(key))]"));
        assert!(bzl.contains("\"deps\": attr.label_list(cfg = _platforms)"));
        let roots = [TargetRoot {
            tool: "runtime".to_owned(),
            keys: vec!["linux_x86_64".to_owned(), "windows_arm64".to_owned()],
            targets: vec!["@@rules_dx++dx_prebuilt+dx_prebuilt//:runtime".to_owned()],
        }];
        let build = target_guard_build(&roots);
        assert!(build.starts_with("load(\":defs.bzl\", \"target_root\")\n"));
        assert!(build.contains(
            "name = \"runtime\",\n    testonly = True,\n    keys = [\n        \"linux_x86_64\",\n        \"windows_arm64\",\n    ],\n    deps = [\n        \"@@rules_dx++dx_prebuilt+dx_prebuilt//:runtime\",\n    ],"
        ));
        assert!(build.contains(
            "name = \"targets\",\n    testonly = True,\n    srcs = [\n        \":runtime\",\n    ],"
        ));
        let cquery = "native_binary rule @@rules_dx++dx_prebuilt+dx_prebuilt//:runtime_linux_x86_64 (abc)\n\
                      source file @@rules_dx++dx_prebuilt+dx_runtime_prebuilt_linux_x86_64//file:runtime (null)\n";
        assert_eq!(
            missing_target_repos(cquery, &roots),
            [
                "runtime on windows_arm64 does not select @@rules_dx++dx_prebuilt+dx_runtime_prebuilt_windows_aarch64"
            ]
        );
    }

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

    fn launchers() -> BTreeMap<String, BTreeSet<String>> {
        BTreeMap::from([
            (
                "@@rules_dx+//t:detekt".to_owned(),
                BTreeSet::from(["java_binary rule @@rules_dx+//t:detekt (abc)".to_owned()]),
            ),
            (
                "@@rules_dx+//t:javac_check".to_owned(),
                BTreeSet::from(["java_binary rule @@rules_dx+//t:javac_check (abc)".to_owned()]),
            ),
            (
                "@@rules_dx+//cli/cli:dx".to_owned(),
                BTreeSet::from(["rust_binary rule @@rules_dx+//cli/cli:dx (abc)".to_owned()]),
            ),
        ])
    }

    #[test]
    fn drop_jar_launchers_keeps_binaries_with_rules_dx_sources() {
        let mut found = launchers();
        let mut asked = Vec::new();
        drop_jar_launchers(&mut found, |label| {
            asked.push(label.to_owned());
            Ok(if label.ends_with(":javac_check") {
                "@@rules_dx+//t:javac_check_lib (abc)\n".to_owned()
            } else {
                "@@rules_java+//toolchains:singlejar (exec)\n".to_owned()
            })
        })
        .expect("no cquery error");
        assert_eq!(
            found.keys().collect::<Vec<_>>(),
            ["@@rules_dx+//cli/cli:dx", "@@rules_dx+//t:javac_check"]
        );
        assert_eq!(
            asked,
            ["@@rules_dx+//t:detekt", "@@rules_dx+//t:javac_check"]
        );
    }

    #[test]
    fn drop_jar_launchers_passes_up_a_cquery_error() {
        let mut found = launchers();
        assert_eq!(
            drop_jar_launchers(&mut found, |_| Err("cquery failed".to_owned())),
            Err("cquery failed".to_owned())
        );
        assert_eq!(found, launchers());
    }

    #[test]
    fn root_label_inverts_consumer_label() {
        let fixtures = "+_repo_rules+dx_api_fixtures";
        for root in [
            "@@//java/private/quality/tools/kotlinc_check:kotlinc_check",
            "@@+dx_prebuilt+dx_prebuilt//:javac_check",
            "@@rules_java+//toolchains:singlejar",
        ] {
            assert_eq!(root_label(&consumer_label(root, fixtures)), root);
        }
        assert_eq!(
            root_label("@@rules_dx++dx_prebuilt+dx_prebuilt//:javac_check"),
            "@@+dx_prebuilt+dx_prebuilt//:javac_check"
        );
    }

    #[test]
    fn bundle_tools_read_the_launcher_and_missing_platforms() {
        let tools =
            bundle_tools("ansible_lint_tool ansible_lint windows_arm64\nyamllint yamllint\n\n")
                .expect("parses");
        assert_eq!(
            tools,
            [
                BundleTool {
                    name: "ansible_lint_tool".to_owned(),
                    launcher: "ansible_lint".to_owned(),
                    missing: BTreeSet::from(["windows_arm64".to_owned()]),
                },
                BundleTool {
                    name: "yamllint".to_owned(),
                    launcher: "yamllint".to_owned(),
                    missing: BTreeSet::new(),
                },
            ]
        );
        assert_eq!(
            tools[1].hub_target(),
            "@@rules_dx++dx_prebuilt+dx_prebuilt//:python_tool_yamllint"
        );
        assert!(bundle_tools("yamllint\n").is_err());
    }

    #[test]
    fn the_stub_bundle_has_a_launcher_per_tool() {
        let tools = bundle_tools("cfn_lint cfn_lint\nyamllint yamllint\n").expect("parses");
        let names = |windows| {
            let archive = stub_bundle(&tools, windows).expect("writes");
            let mut names = Vec::new();
            dx_archive::visit(
                std::io::Cursor::new(archive),
                dx_archive::Format::TarGz,
                dx_archive::Limits::default(),
                |name, _, _| {
                    names.push(name.to_owned());
                    Ok(())
                },
            )
            .expect("reads");
            names
        };
        assert_eq!(names(false), ["bin/cfn_lint", "bin/yamllint"]);
        assert_eq!(
            names(true),
            [
                "bin/cfn_lint",
                "bin/cfn_lint.cmd",
                "bin/yamllint",
                "bin/yamllint.cmd"
            ]
        );
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
                "@rules_dx//dx:env",
                "@rules_dx//dx:codegen",
                "@rules_dx//tools/docs/site:scope_tools",
                "@rules_dx//go/private/tools:go",
                "@rules_dx//go/private/ide:gopackagesdriver",
                "@rules_dx//rust/private/ide:discover",
                "@rules_dx//rust/private/ide:flycheck",
            ]
        );
        assert!(command_roots("", &[]).contains(&"//:dx".to_owned()));
    }

    #[test]
    fn resolver_roots_name_the_hub_resolvers_of_the_languages() {
        assert_eq!(
            resolver_roots(&["go", "zig", "perl", "d", "zig", "dart", "csharp"]),
            [
                "@rules_dx//csharp/private/tools/lock",
                "@rules_dx//d/private/tools/lock",
                "@rules_dx//dart/private/tools/lock",
                "@rules_dx//perl/private/tools/lock",
                "@rules_dx//zig/private/tools/lock",
            ]
        );
        assert!(resolver_roots(&["go", "kotlin"]).is_empty());
    }

    #[test]
    fn allowlist_lines_need_a_kind_and_a_reason() {
        let list = allowlist(
            "# header\nlabel @@rules_dx+//a:b  # why\nlabel @@rules_dx+//l:c lua  # lua only\n\n",
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
        assert!(allowlist("untagged python_tools # why\n").is_err());
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
