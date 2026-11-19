//! A consumer whose root module pins every registry tool for the host compiles no rules_dx tool.
//!
//! The test writes a stub file per registry tool of the host, adds one `dx_prebuilt.asset`
//! (or `dx_gazelle.asset`) per tool to a consumer of the `<lang>/tests/consumer` fixtures of the
//! `DX_PREBUILT_GUARD_LANGS` languages (the first at the root, each later one under `<lang>/`),
//! and runs `bazel cquery 'deps(...)'` over the fixtures and the hub target of every pinned tool.
//! A jar tool gets `os = "any"` and `cpu = "any"`, and its hub target is a root only when a
//! language is a JVM language. cquery follows exec and target configurations alike. Any compiled binary that a
//! rules_dx repository declares fails the test unless `prebuilt_guard_allowlist.txt` names it;
//! the `java_binary` the `dx_prebuilt` hub wraps around a downloaded jar compiles nothing.
//!
//! `cquery --aspects` and `aquery --aspects` leave out the tool targets of command-line aspects,
//! while cquery follows aspects a rule attribute carries. So the test also writes the
//! `dx_prebuilt_guard` package: its `check` and `docs` targets depend on every non-manual rule
//! of the consumer through a `deps` attribute that carries the aspects `dx check` and `dx docs`
//! pick for the consumer's files.
//!
//! cquery only analyzes, so the stubs never run; their sha256 is all the hubs check.

use std::collections::{BTreeMap, BTreeSet};

use dx_nested::Case;
use dx_plan::args::Command;
use dx_plan::plan::registry::{Consumer, aspects_for};

/// The rule kind prefixes of compiled binaries; shell stubs and `native_binary` stay out.
const COMPILED: &[&str] = &[
    "cc_", "gazelle_", "go_", "haskell_", "java_", "kt_jvm_", "rust_", "scala_",
];

/// The target name crate_universe gives a crate's build script binary.
const BUILD_SCRIPT: &str = ":_bs_";

/// The repository of the `dx_prebuilt` hub.
const PREBUILT_HUB_REPO: &str = "rules_dx++dx_prebuilt+dx_prebuilt";

/// The consumer languages whose fixture roots the jar tools.
const JVM_LANGS: &[&str] = &["java", "kotlin", "scala"];

/// Registry tools served by `dx_gazelle` instead of `dx_prebuilt`.
const GAZELLE_TOOLS: &[&str] = &["gazelle"];

/// The allowlist: binary labels the consumer may compile, each with the consumer languages it
/// applies to (none: every language), and registry tools no hub takes as an asset tag.
#[derive(Debug, Default, PartialEq)]
struct Allowlist {
    labels: BTreeMap<String, BTreeSet<String>>,
    untagged: BTreeSet<String>,
}

impl Allowlist {
    /// The labels a consumer of `langs` may compile.
    fn labels_for(&self, langs: &[&str]) -> BTreeSet<String> {
        self.labels
            .iter()
            .filter(|(_, only)| only.is_empty() || langs.iter().any(|lang| only.contains(*lang)))
            .map(|(label, _)| label.clone())
            .collect()
    }
}

/// Reads `label <canonical label> [<lang>...]` and `untagged <tool>` lines; each needs a `#` reason.
fn allowlist(text: &str) -> Result<Allowlist, String> {
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

/// The host as a registry platform key such as `linux_x86_64`, and as `@platforms` os and cpu.
fn host() -> Result<(String, &'static str, &'static str), String> {
    let (key_os, os) = match std::env::consts::OS {
        "linux" => ("linux", "linux"),
        "macos" => ("macos", "macos"),
        "windows" => ("windows", "windows"),
        other => return Err(format!("no registry platform for os {other}")),
    };
    let (key_cpu, cpu) = match std::env::consts::ARCH {
        "x86_64" => ("x86_64", "x86_64"),
        "aarch64" => ("arm64", "aarch64"),
        other => return Err(format!("no registry platform for cpu {other}")),
    };
    Ok((format!("{key_os}_{key_cpu}"), os, cpu))
}

/// The registry tools released for `key`, plus every tool released as a jar for any platform.
fn host_tools(registry: &str, key: &str) -> BTreeMap<String, String> {
    registry
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (tool, platform) = (fields.next()?, fields.next()?);
            (platform == key || platform == "any").then(|| (tool.to_owned(), platform.to_owned()))
        })
        .collect()
}

/// The extension that serves `tool`.
fn hub(tool: &str) -> &'static str {
    if GAZELLE_TOOLS.contains(&tool) {
        "dx_gazelle"
    } else {
        "dx_prebuilt"
    }
}

/// The hub targets of `tool`, by canonical name so the consumer needs no `use_repo`. The `go`
/// registry tool is the dx-go multicall, served as one hub target per name in `go_names`.
fn hub_targets(tool: &str, go_names: &str) -> Vec<String> {
    let hub = hub(tool);
    let names: Vec<&str> = if tool == "go" {
        go_names.split_whitespace().collect()
    } else {
        vec![tool]
    };
    names
        .iter()
        .map(|name| format!("@@rules_dx++{hub}+{hub}//:{name}"))
        .collect()
}

/// The `asset` tag of one stub, on the hub that serves `tool`.
fn asset_tag(tool: &str, os: &str, cpu: &str, url: &str, sha256: &str) -> String {
    let hub = hub(tool);
    format!(
        "{hub}.asset(name = \"{tool}\", os = \"{os}\", cpu = \"{cpu}\", url = \"{url}\", sha256 = \"{sha256}\")\n"
    )
}

/// Whether a consumer of `langs` roots the hub target of a tool released for `platform`.
fn rooted(platform: &str, langs: &[&str]) -> bool {
    platform != "any" || langs.iter().any(|lang| JVM_LANGS.contains(lang))
}

/// The rules_dx-declared compiled binaries in `cquery --output=label_kind` lines, each with its lines.
fn compiled_binaries(cquery: &str) -> BTreeMap<String, BTreeSet<String>> {
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

/// The problems: compiled binaries not allowlisted, and allowlisted labels no longer compiled.
fn problems(found: &BTreeMap<String, BTreeSet<String>>, allowed: &BTreeSet<String>) -> Vec<String> {
    let mut problems: Vec<String> = found
        .iter()
        .filter(|(label, _)| !allowed.contains(*label))
        .flat_map(|(_, lines)| lines.iter().map(|line| format!("compiles {line}")))
        .collect();
    problems.extend(
        allowed
            .iter()
            .filter(|label| !found.contains_key(*label))
            .map(|label| format!("no longer compiled, remove it from the allowlist: {label}")),
    );
    problems
}

/// The aspects `dx check` (format, lint, typecheck) and `dx docs` pass for a consumer with
/// `extensions`, each list in command order and without repeats.
fn guard_aspects(extensions: &BTreeSet<String>) -> (Vec<&'static str>, Vec<&'static str>) {
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
/// `deps` carry the `check` and `docs` aspects (`<package>:<file>%<name>` labels in rules_dx).
fn wrapper_bzl(check: &[&str], docs: &[&str]) -> String {
    let mut loads = String::new();
    let mut rules = String::from("def _root_impl(_ctx):\n    return []\n");
    for (prefix, aspects) in [("check", check), ("docs", docs)] {
        let mut names = Vec::new();
        for (index, aspect) in aspects.iter().enumerate() {
            let (file, name) = aspect.split_once('%').unwrap_or((aspect, ""));
            let alias = format!("{prefix}_{index}");
            loads.push_str(&format!(
                "load(\"@rules_dx//{file}\", {alias} = \"{name}\")\n"
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

/// The `BUILD.bazel` of the `dx_prebuilt_guard` package: `check` and `docs` over `targets`.
fn wrapper_build(targets: &[&str]) -> String {
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

fn runfile(var: &str) -> String {
    let path = dx_testing::rlocation(dx_testing::env_text(var).trim())
        .unwrap_or_else(|error| panic!("{var}: {error}"));
    dx_testing::read_text(&path)
}

#[test]
fn a_consumer_with_every_host_pin_compiles_no_rules_dx_tool() {
    let langs = dx_testing::env_text("DX_PREBUILT_GUARD_LANGS");
    let langs: Vec<&str> = langs.split_whitespace().collect();
    assert!(
        !langs.is_empty(),
        "DX_PREBUILT_GUARD_LANGS names no language"
    );
    let named = langs.join(" ");
    let list = allowlist(&runfile("DX_PREBUILT_GUARD_ALLOWLIST")).unwrap_or_else(|error| {
        panic!("prebuilt_guard_allowlist.txt: {error}");
    });
    let (key, os, cpu) = host().unwrap_or_else(|error| panic!("{error}"));
    let tools = host_tools(&runfile("DX_PREBUILT_REGISTRY"), &key);
    let go_names = runfile("DX_PREBUILT_GO_HUB_TARGETS");
    let stubs = std::path::PathBuf::from(dx_testing::env_text("TEST_TMPDIR").trim()).join("stubs");
    let mut tags = String::new();
    let mut roots = vec!["//...".to_owned()];
    for (tool, platform) in &tools {
        if list.untagged.contains(tool) {
            continue;
        }
        let (os, cpu) = if platform == "any" {
            ("any", "any")
        } else {
            (os, cpu)
        };
        let stub = stubs.join(tool);
        dx_testing::write_text(&stub, &format!("stub {tool}\n"));
        let sha256 = dx_testing::sha256_file(&stub).unwrap_or_else(|error| panic!("{error}"));
        let url = format!("file://{}", stub.to_string_lossy().replace('\\', "/"));
        tags.push_str(&asset_tag(tool, os, cpu, &url, &sha256));
        if rooted(platform, &langs) {
            roots.extend(hub_targets(tool, &go_names));
        }
    }
    let extra = format!(
        "dx_prebuilt = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_prebuilt\")\n\
         dx_gazelle = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_gazelle\")\n{tags}"
    );
    let case = Case::languages_consumer(&format!("{}_prebuilt_guard", langs[0]), &langs, &extra)
        .unwrap_or_else(|error| panic!("{error}"));
    let targets = case.bazel(&[
        "query",
        "--color=no",
        "--curses=no",
        "--output=label",
        "kind(rule, //...) except attr(tags, \"\\bmanual\\b\", //...)",
    ]);
    assert!(
        targets.status.success(),
        "bazel query: {}",
        targets.combined()
    );
    let targets: Vec<&str> = targets
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    assert!(
        !targets.is_empty(),
        "bazel query found no rule in the {named} consumer"
    );
    for lang in &langs[1..] {
        let package = format!("//{lang}:");
        assert!(
            targets.iter().any(|target| target.starts_with(&package)),
            "bazel query found no {package} rule in the {named} consumer"
        );
    }
    let (check, docs) = guard_aspects(&dx_adopt::file_extensions(case.root()));
    let wrapper = case.root().join("dx_prebuilt_guard");
    dx_testing::write_text(&wrapper.join("defs.bzl"), &wrapper_bzl(&check, &docs));
    dx_testing::write_text(&wrapper.join("BUILD.bazel"), &wrapper_build(&targets));
    let query = format!("deps({})", roots.join(" + "));
    let run = case.bazel(&[
        "cquery",
        "--color=no",
        "--curses=no",
        "--consistent_labels",
        "--nocheck_visibility",
        "--output=label_kind",
        &query,
    ]);
    assert!(run.status.success(), "bazel cquery: {}", run.combined());
    for label in ["@@//dx_prebuilt_guard:check", "@@//dx_prebuilt_guard:docs"] {
        assert!(
            run.stdout
                .lines()
                .any(|line| line.split_whitespace().nth(2) == Some(label)),
            "bazel cquery left out {label}"
        );
    }
    let found = compiled_binaries(&run.stdout);
    let problems = problems(&found, &list.labels_for(&langs));
    assert!(
        problems.is_empty(),
        "{named} consumer with host pins for {}:\n{}",
        tools.keys().cloned().collect::<Vec<_>>().join(", "),
        problems.join("\n")
    );
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
fn host_tools_keep_the_host_key_and_jars() {
    let registry = "dx linux_x86_64 dx-linux-x86_64\ndx macos_arm64 dx-macos-arm64\njmh_generate any dx-jmh-generate-any.jar\nhlint linux_arm64 dx-hlint-linux-arm64\n";
    assert_eq!(
        host_tools(registry, "linux_x86_64"),
        BTreeMap::from([
            ("dx".to_owned(), "linux_x86_64".to_owned()),
            ("jmh_generate".to_owned(), "any".to_owned()),
        ])
    );
}

#[test]
fn asset_tags_go_to_the_hub_that_serves_the_tool() {
    assert!(
        asset_tag("gazelle", "linux", "x86_64", "file:///g", "ab")
            .starts_with("dx_gazelle.asset(name = \"gazelle\"")
    );
    assert!(
        asset_tag("selene", "linux", "x86_64", "file:///s", "ab")
            .starts_with("dx_prebuilt.asset(name = \"selene\"")
    );
    let go_names = "dx_errcheck\ndx_govet\n";
    assert_eq!(
        hub_targets("gazelle", go_names),
        ["@@rules_dx++dx_gazelle+dx_gazelle//:gazelle"]
    );
    assert_eq!(
        hub_targets("dx", go_names),
        ["@@rules_dx++dx_prebuilt+dx_prebuilt//:dx"]
    );
    assert_eq!(
        hub_targets("go", go_names),
        [
            "@@rules_dx++dx_prebuilt+dx_prebuilt//:dx_errcheck",
            "@@rules_dx++dx_prebuilt+dx_prebuilt//:dx_govet",
        ]
    );
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
fn only_jvm_consumers_root_the_jar_tools() {
    assert!(rooted("linux_x86_64", &["go"]));
    assert!(rooted("linux_x86_64", &["java"]));
    assert!(!rooted("any", &["go"]));
    assert!(rooted("any", &["java"]));
    assert!(rooted("any", &["kotlin"]));
    assert!(rooted("any", &["scala"]));
    assert!(rooted("any", &["go", "scala"]));
}

#[test]
fn the_wrapper_carries_each_aspect_on_its_deps() {
    let bzl = wrapper_bzl(
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
