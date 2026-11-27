//! A consumer whose root module pins every registry tool for the host compiles no rules_dx tool.
//!
//! The test writes a stub file per registry tool of the host, adds one `dx_prebuilt.asset`
//! (or `dx_gazelle.asset`) per tool to a consumer of the `<lang>/tests/consumer` fixtures of the
//! languages the `var` environment variable names (the first at the root, each later one under `<lang>/`),
//! and runs `bazel cquery 'deps(...)'` over the fixtures, the `@rules_dx` targets dx commands run
//! and the hub target of every pinned tool.
//! A tool of `DX_PREBUILT_TARGET_TOOLS` runs on the target platform, so it gets one tag per
//! registry key instead, and the test writes the `dx_target_guard` package: its `targets` depend
//! on the hub target of each such tool through a split transition over
//! `@rules_dx//tools/platforms:<key>` for each registry key of the tool. A target tool that does
//! not select the download of each of its keys fails the test.
//! A jar tool gets `os = "any"` and `cpu = "any"`, and its hub target is a root only when a
//! language is a JVM language. cquery follows exec and target configurations alike. Any compiled binary that a
//! rules_dx repository declares fails the test unless `prebuilt_guard_allowlist.txt` names it;
//! the `java_binary` the `dx_prebuilt` hub wraps around a downloaded jar compiles nothing, and
//! so does a rules_dx `java_binary` when neither it nor a rules_dx JVM rule under it has `srcs`.
//!
//! `cquery --aspects` and `aquery --aspects` leave out the tool targets of command-line aspects,
//! while cquery follows aspects a rule attribute carries. So the test also writes the
//! `dx_prebuilt_guard` package: its `check` and `docs` targets depend on every non-manual rule
//! of the consumer through a `deps` attribute that carries the aspects `dx check` and `dx docs`
//! pick for the consumer's files.
//!
//! The `python_tools` stub is a `tar.gz` with a `bin/` launcher per bundled tool, so the hub's
//! `python_tool_<name>` targets analyze the pinned bundle path.
//!
//! cquery only analyzes, so the stubs never run; their sha256 is all the hubs check.

#![expect(
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test helper: a failed case fails the test"
)]

use std::collections::BTreeMap;

use dx_nested::Case;
use prebuilt_guard::{
    BundleTool, DOCS_FOREIGN, TargetRoot, allowlist, bundle_tools, command_roots,
    compiled_binaries, drop_jar_launchers, drop_rule_set_repos, foreign_docs_labels, guard_aspects,
    jvm_sources_query, missing_target_repos, problems, release_platform, resolver_roots,
    stale_repos, stub_bundle, stub_url, target_guard_build, target_guard_bzl, wrapper_build,
    wrapper_bzl,
};

/// The consumer languages whose fixture roots the jar tools.
const JVM_LANGS: &[&str] = &["java", "kotlin", "scala"];

/// Registry tools served by `dx_gazelle` instead of `dx_prebuilt`.
const GAZELLE_TOOLS: &[&str] = &["gazelle"];

/// The host as a registry platform key such as `linux_x86_64`.
fn host() -> Result<String, String> {
    let os = match std::env::consts::OS {
        os @ ("linux" | "macos" | "windows") => os,
        other => return Err(format!("no registry platform for os {other}")),
    };
    let cpu = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        other => return Err(format!("no registry platform for cpu {other}")),
    };
    Ok(format!("{os}_{cpu}"))
}

/// The registry keys of each tool in `names`.
fn target_keys(registry: &str, names: &str) -> BTreeMap<String, Vec<String>> {
    let names: Vec<&str> = names.split_whitespace().collect();
    let mut keys: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in registry.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(tool), Some(key)) = (fields.next(), fields.next())
            && names.contains(&tool)
        {
            keys.entry(tool.to_owned())
                .or_default()
                .push(key.to_owned());
        }
    }
    keys
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

/// The names the hub serves for one registry tool, one per line.
struct HubNames {
    dx: String,
    go: String,
}

/// The hub targets of `tool` on platform `key`, by canonical name so the consumer needs no
/// `use_repo`. The `dx` registry tool is also served as `dx:<name>` per name in `names.dx`. The `go`
/// registry tool is the dx-go multicall, served as one hub target per name in `names.go`;
/// `python_tools` is served as one `python_tool_<name>` per bundled tool not missing on `key`.
fn hub_targets(tool: &str, key: &str, names: &HubNames, bundle: &[BundleTool]) -> Vec<String> {
    let hub = hub(tool);
    match tool {
        "dx" => std::iter::once(format!("@@rules_dx++{hub}+{hub}//:dx"))
            .chain(
                names
                    .dx
                    .split_whitespace()
                    .map(|name| format!("@@rules_dx++{hub}+{hub}//dx:{name}")),
            )
            .collect(),
        "go" => names
            .go
            .split_whitespace()
            .map(|name| format!("@@rules_dx++{hub}+{hub}//:{name}"))
            .collect(),
        "python_tools" => bundle
            .iter()
            .filter(|tool| !tool.missing.contains(key))
            .map(BundleTool::hub_target)
            .collect(),
        _ => vec![format!("@@rules_dx++{hub}+{hub}//:{tool}")],
    }
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

fn runfile(var: &str) -> String {
    let path = dx_testing::rlocation(dx_testing::env_text(var).trim())
        .unwrap_or_else(|error| panic!("{var}: {error}"));
    dx_testing::read_text(&path)
}

pub fn compiles_no_rules_dx_tool(var: &str) {
    let langs = dx_testing::env_text(var);
    let langs: Vec<&str> = langs.split_whitespace().collect();
    assert!(!langs.is_empty(), "{var} names no language");
    let named = langs.join(" ");
    let list = allowlist(&runfile("DX_PREBUILT_GUARD_ALLOWLIST")).unwrap_or_else(|error| {
        panic!("prebuilt_guard_allowlist.txt: {error}");
    });
    let key = host().unwrap_or_else(|error| panic!("{error}"));
    let (os, cpu) = release_platform(&key).unwrap_or_else(|| panic!("host key {key}"));
    let registry = runfile("DX_PREBUILT_REGISTRY");
    let tools = host_tools(&registry, &key);
    let targeted = target_keys(&registry, &runfile("DX_PREBUILT_TARGET_TOOLS"));
    let names = HubNames {
        dx: runfile("DX_PREBUILT_DX_HUB_TARGETS"),
        go: runfile("DX_PREBUILT_GO_HUB_TARGETS"),
    };
    let bundle = bundle_tools(&runfile("DX_PREBUILT_PYTHON_HUB_TARGETS"))
        .unwrap_or_else(|error| panic!("python_hub_targets.txt: {error}"));
    let stubs = std::path::PathBuf::from(dx_testing::env_text("TEST_TMPDIR").trim()).join("stubs");
    let mut tags = String::new();
    let mut roots = vec!["//...".to_owned()];
    roots.extend(command_roots("@rules_dx", &langs));
    roots.extend(resolver_roots(&langs));
    for (tool, platform) in &tools {
        if targeted.contains_key(tool) {
            continue;
        }
        let (os, cpu) = if platform == "any" {
            ("any", "any")
        } else {
            (os, cpu)
        };
        let stub = stubs.join(tool);
        if tool == "python_tools" {
            let archive = stub_bundle(&bundle, os == "windows")
                .unwrap_or_else(|error| panic!("stub python_tools bundle: {error}"));
            std::fs::create_dir_all(&stubs)
                .and_then(|()| std::fs::write(&stub, archive))
                .unwrap_or_else(|error| panic!("write {}: {error}", stub.display()));
        } else {
            dx_testing::write_text(&stub, &format!("stub {tool}\n"));
        }
        let sha256 = dx_testing::sha256_file(&stub).unwrap_or_else(|error| panic!("{error}"));
        let url = stub_url(&stub);
        tags.push_str(&asset_tag(tool, os, cpu, &url, &sha256));
        if rooted(platform, &langs) {
            roots.extend(hub_targets(tool, &key, &names, &bundle));
        }
    }
    let mut target_roots = Vec::new();
    for (tool, keys) in &targeted {
        let stub = stubs.join(tool);
        dx_testing::write_text(&stub, &format!("stub {tool}\n"));
        let sha256 = dx_testing::sha256_file(&stub).unwrap_or_else(|error| panic!("{error}"));
        let url = stub_url(&stub);
        for target_key in keys {
            let (os, cpu) = release_platform(target_key)
                .unwrap_or_else(|| panic!("{tool}: registry key {target_key}"));
            tags.push_str(&asset_tag(tool, os, cpu, &url, &sha256));
        }
        if keys.contains(&key) {
            roots.extend(hub_targets(tool, &key, &names, &bundle));
        }
        target_roots.push(TargetRoot {
            tool: tool.clone(),
            keys: keys.clone(),
            targets: hub_targets(tool, &key, &names, &bundle),
        });
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
        "--nocheck_bzl_visibility",
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
    dx_testing::write_text(
        &wrapper.join("defs.bzl"),
        &wrapper_bzl("@rules_dx", &check, &docs),
    );
    dx_testing::write_text(&wrapper.join("BUILD.bazel"), &wrapper_build(&targets));
    let target_guard = case.root().join("dx_target_guard");
    dx_testing::write_text(
        &target_guard.join("defs.bzl"),
        &target_guard_bzl("@rules_dx"),
    );
    dx_testing::write_text(
        &target_guard.join("BUILD.bazel"),
        &target_guard_build(&target_roots),
    );
    let query = format!("deps({})", roots.join(" + "));
    let run = case.bazel(&[
        "cquery",
        "--color=no",
        "--curses=no",
        "--consistent_labels",
        "--nocheck_visibility",
        "--nocheck_bzl_visibility",
        "--output=label_kind",
        &query,
    ]);
    assert!(run.status.success(), "bazel cquery: {}", run.combined());
    let command_labels: Vec<String> = command_roots("@@rules_dx+", &langs);
    for label in [
        "@@//dx_prebuilt_guard:check",
        "@@//dx_prebuilt_guard:docs",
        "@@//dx_target_guard:targets",
    ]
    .into_iter()
    .chain(command_labels.iter().map(String::as_str))
    {
        assert!(
            run.stdout
                .lines()
                .any(|line| line.split_whitespace().nth(2) == Some(label)),
            "bazel cquery left out {label}"
        );
    }
    let mut found = compiled_binaries(&run.stdout);
    let rule_set_repos = list.repos_for(&langs);
    let matched_repos = drop_rule_set_repos(&mut found, &rule_set_repos);
    drop_jar_launchers(&mut found, |label| {
        let sources = case.bazel(&[
            "cquery",
            "--color=no",
            "--curses=no",
            "--consistent_labels",
            "--nocheck_visibility",
            "--nocheck_bzl_visibility",
            "--output=label",
            &jvm_sources_query(label),
        ]);
        if sources.status.success() {
            Ok(sources.stdout)
        } else {
            Err(format!("bazel cquery: {}", sources.combined()))
        }
    })
    .unwrap_or_else(|error| panic!("{error}"));
    let mut problems = problems(&found, &list.labels_for(&langs));
    problems.extend(stale_repos(&rule_set_repos, &matched_repos));
    problems.extend(missing_target_repos(&run.stdout, &target_roots));
    if let [lang] = langs.as_slice()
        && let Some((_, fragments)) = DOCS_FOREIGN.iter().find(|(name, _)| name == lang)
    {
        let docs_only = format!(
            "deps(@@//dx_prebuilt_guard:docs) except deps({})",
            targets.join(" + ")
        );
        let docs = case.bazel(&[
            "cquery",
            "--color=no",
            "--curses=no",
            "--consistent_labels",
            "--nocheck_visibility",
            "--nocheck_bzl_visibility",
            "--output=label",
            &docs_only,
        ]);
        assert!(docs.status.success(), "bazel cquery: {}", docs.combined());
        problems.extend(
            foreign_docs_labels(&docs.stdout, fragments)
                .into_iter()
                .map(|label| format!("{lang} docs reach {label}")),
        );
    }
    assert!(
        problems.is_empty(),
        "{named} consumer with host pins for {} and target pins for {}:\n{}",
        tools.keys().cloned().collect::<Vec<_>>().join(", "),
        targeted.keys().cloned().collect::<Vec<_>>().join(", "),
        problems.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let names = HubNames {
            dx: "lua_lock\nperl_lock\n".to_owned(),
            go: "dx_errcheck\ndx_govet\n".to_owned(),
        };
        let bundle =
            bundle_tools("ansible_lint_tool ansible_lint windows_arm64\nyamllint yamllint\n")
                .expect("parses");
        let key = "linux_x86_64";
        assert_eq!(
            hub_targets("gazelle", key, &names, &bundle),
            ["@@rules_dx++dx_gazelle+dx_gazelle//:gazelle"]
        );
        assert_eq!(
            hub_targets("dx", key, &names, &bundle),
            [
                "@@rules_dx++dx_prebuilt+dx_prebuilt//:dx",
                "@@rules_dx++dx_prebuilt+dx_prebuilt//dx:lua_lock",
                "@@rules_dx++dx_prebuilt+dx_prebuilt//dx:perl_lock",
            ]
        );
        assert_eq!(
            hub_targets("python_tools", key, &names, &bundle),
            [
                "@@rules_dx++dx_prebuilt+dx_prebuilt//:python_tool_ansible_lint_tool",
                "@@rules_dx++dx_prebuilt+dx_prebuilt//:python_tool_yamllint",
            ]
        );
        assert_eq!(
            hub_targets("python_tools", "windows_arm64", &names, &bundle),
            ["@@rules_dx++dx_prebuilt+dx_prebuilt//:python_tool_yamllint"]
        );
        assert_eq!(
            hub_targets("go", key, &names, &bundle),
            [
                "@@rules_dx++dx_prebuilt+dx_prebuilt//:dx_errcheck",
                "@@rules_dx++dx_prebuilt+dx_prebuilt//:dx_govet",
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
    fn target_keys_list_every_registry_key_of_the_named_tools() {
        let registry = "dx linux_x86_64 dx-linux-x86_64\nkcov linux_arm64 dx-kcov-linux-arm64\nkcov linux_x86_64 dx-kcov-linux-x86_64\nruntime windows_arm64 dx-runtime-windows-arm64\n";
        assert_eq!(
            target_keys(registry, "kcov\nruntime\n"),
            BTreeMap::from([
                (
                    "kcov".to_owned(),
                    vec!["linux_arm64".to_owned(), "linux_x86_64".to_owned()]
                ),
                ("runtime".to_owned(), vec!["windows_arm64".to_owned()]),
            ])
        );
    }

    #[test]
    fn the_host_is_a_registry_key() {
        let key = host().unwrap();
        assert!(release_platform(&key).is_some(), "{key}");
    }
}
