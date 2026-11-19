//! A consumer whose root module pins every registry tool for the host compiles no rules_dx tool.
//!
//! The test writes a stub file per registry tool of the host, adds one `dx_prebuilt.asset`
//! (or `dx_gazelle.asset`) per tool to the consumer of `<lang>/tests/consumer`, and runs
//! `bazel cquery 'deps(...)'` over the fixture and the hub target of every pinned tool. cquery
//! follows exec and target configurations alike. Any compiled binary that a rules_dx repository
//! declares fails the test unless `prebuilt_guard_allowlist.txt` names it.
//!
//! cquery only analyzes, so the stubs never run; their sha256 is all the hubs check.

use std::collections::{BTreeMap, BTreeSet};

use dx_nested::Case;

/// The rule kind prefixes of compiled binaries; shell stubs and `native_binary` stay out.
const COMPILED: &[&str] = &[
    "cc_", "gazelle_", "go_", "haskell_", "java_", "kt_jvm_", "rust_", "scala_",
];

/// The target name crate_universe gives a crate's build script binary.
const BUILD_SCRIPT: &str = ":_bs_";

/// Registry tools served by `dx_gazelle` instead of `dx_prebuilt`.
const GAZELLE_TOOLS: &[&str] = &["gazelle"];

/// The allowlist: binary labels the consumer may compile, and registry tools no hub takes as an asset tag.
#[derive(Debug, Default, PartialEq)]
struct Allowlist {
    labels: BTreeSet<String>,
    untagged: BTreeSet<String>,
}

/// Reads `label <canonical label>` and `untagged <tool>` lines; each needs a `#` reason.
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
            ["label", label] => list.labels.insert(label.to_owned()),
            ["untagged", tool] => list.untagged.insert(tool.to_owned()),
            _ => {
                return Err(format!(
                    "line {}: want `label <label>` or `untagged <tool>`: {line}",
                    number + 1
                ));
            }
        };
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

/// The hub target of `tool`, by canonical name so the consumer needs no `use_repo`.
fn hub_target(tool: &str) -> String {
    let hub = hub(tool);
    format!("@@rules_dx++{hub}+{hub}//:{tool}")
}

/// The `asset` tag of one stub, on the hub that serves `tool`.
fn asset_tag(tool: &str, os: &str, cpu: &str, url: &str, sha256: &str) -> String {
    let hub = hub(tool);
    format!(
        "{hub}.asset(name = \"{tool}\", os = \"{os}\", cpu = \"{cpu}\", url = \"{url}\", sha256 = \"{sha256}\")\n"
    )
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

fn runfile(var: &str) -> String {
    let path = dx_testing::rlocation(dx_testing::env_text(var).trim())
        .unwrap_or_else(|error| panic!("{var}: {error}"));
    dx_testing::read_text(&path)
}

#[test]
fn a_consumer_with_every_host_pin_compiles_no_rules_dx_tool() {
    let lang = dx_testing::env_text("DX_PREBUILT_GUARD_LANG");
    let lang = lang.trim();
    let list = allowlist(&runfile("DX_PREBUILT_GUARD_ALLOWLIST")).unwrap_or_else(|error| {
        panic!("prebuilt_guard_allowlist.txt: {error}");
    });
    let (key, os, cpu) = host().unwrap_or_else(|error| panic!("{error}"));
    let tools = host_tools(&runfile("DX_PREBUILT_REGISTRY"), &key);
    let stubs = std::path::PathBuf::from(dx_testing::env_text("TEST_TMPDIR").trim()).join("stubs");
    let mut tags = String::new();
    let mut roots = vec!["//...".to_owned()];
    let mut untagged = Vec::new();
    for (tool, platform) in &tools {
        if list.untagged.contains(tool) {
            continue;
        }
        if platform == "any" {
            untagged.push(format!(
                "registry tool {tool} ships for any platform and no hub takes an asset tag for it"
            ));
            continue;
        }
        let stub = stubs.join(tool);
        dx_testing::write_text(&stub, &format!("stub {tool}\n"));
        let sha256 = dx_testing::sha256_file(&stub).unwrap_or_else(|error| panic!("{error}"));
        let url = format!("file://{}", stub.to_string_lossy().replace('\\', "/"));
        tags.push_str(&asset_tag(tool, os, cpu, &url, &sha256));
        roots.push(hub_target(tool));
    }
    assert!(untagged.is_empty(), "{}", untagged.join("\n"));
    let extra = format!(
        "dx_prebuilt = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_prebuilt\")\n\
         dx_gazelle = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_gazelle\")\n{tags}"
    );
    let case = Case::language_consumer(&format!("{lang}_prebuilt_guard"), lang, &extra)
        .unwrap_or_else(|error| panic!("{error}"));
    let query = format!("deps({})", roots.join(" + "));
    let run = case.bazel(&[
        "cquery",
        "--color=no",
        "--curses=no",
        "--consistent_labels",
        "--output=label_kind",
        &query,
    ]);
    assert!(run.status.success(), "bazel cquery: {}", run.combined());
    let found = compiled_binaries(&run.stdout);
    let problems = problems(&found, &list.labels);
    assert!(
        problems.is_empty(),
        "{lang} consumer with host pins for {}:\n{}",
        tools.keys().cloned().collect::<Vec<_>>().join(", "),
        problems.join("\n")
    );
}

#[test]
fn allowlist_lines_need_a_kind_and_a_reason() {
    let list = allowlist("# header\nlabel @@rules_dx+//a:b  # why\nuntagged avro_union # jar\n\n")
        .expect("parses");
    assert_eq!(
        list,
        Allowlist {
            labels: BTreeSet::from(["@@rules_dx+//a:b".to_owned()]),
            untagged: BTreeSet::from(["avro_union".to_owned()]),
        }
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
    assert_eq!(
        hub_target("gazelle"),
        "@@rules_dx++dx_gazelle+dx_gazelle//:gazelle"
    );
    assert_eq!(hub_target("dx"), "@@rules_dx++dx_prebuilt+dx_prebuilt//:dx");
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
source file @@rules_dx+//cli/cli:main.rs (null)
";
    let found = compiled_binaries(cquery);
    assert_eq!(
        found.keys().cloned().collect::<Vec<_>>(),
        [
            "@@rules_dx++dx_gazelle+dx_gazelle//:gazelle",
            "@@rules_dx+//cli/cli:dx"
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
            "no longer compiled, remove it from the allowlist: @@rules_dx+//gone:tool",
        ]
    );
}
