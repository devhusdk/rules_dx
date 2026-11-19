//! A consumer whose root module pins every registry tool for the host compiles no rules_dx tool.
//!
//! The test writes a stub file per registry tool of the host, adds one `dx_prebuilt.asset`
//! (or `dx_gazelle.asset`) per tool to a consumer of the `<lang>/tests/consumer` fixtures of the
//! `DX_PREBUILT_GUARD_LANGS` languages (the first at the root, each later one under `<lang>/`),
//! and runs `bazel cquery 'deps(...)'` over the fixtures, the `@rules_dx` targets dx commands run
//! and the hub target of every pinned tool.
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

use std::collections::BTreeMap;

use dx_nested::Case;
use prebuilt_guard::{
    allowlist, command_roots, compiled_binaries, guard_aspects, problems, stub_url, wrapper_build,
    wrapper_bzl,
};

/// The consumer languages whose fixture roots the jar tools.
const JVM_LANGS: &[&str] = &["java", "kotlin", "scala"];

/// Registry tools served by `dx_gazelle` instead of `dx_prebuilt`.
const GAZELLE_TOOLS: &[&str] = &["gazelle"];

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
    roots.extend(command_roots("@rules_dx", &langs));
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
        let url = stub_url(&stub);
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
    for label in ["@@//dx_prebuilt_guard:check", "@@//dx_prebuilt_guard:docs"]
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
fn only_jvm_consumers_root_the_jar_tools() {
    assert!(rooted("linux_x86_64", &["go"]));
    assert!(rooted("linux_x86_64", &["java"]));
    assert!(!rooted("any", &["go"]));
    assert!(rooted("any", &["java"]));
    assert!(rooted("any", &["kotlin"]));
    assert!(rooted("any", &["scala"]));
    assert!(rooted("any", &["go", "scala"]));
}
