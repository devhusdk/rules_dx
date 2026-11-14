//! Rule-set files load only same-language files, a fixed set of external repos and `RULES_DX_ALLOW`, and nothing in `RULES_DX_ALLOW` reaches them, outside tools/ci/rule_set_loads_allowlist.txt.

#[path = "guard_rules/scan.rs"]
#[expect(dead_code, reason = "the test uses a subset of the shared module")]
mod scan;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use scan::{add, calls, is_skipped, is_starlark, masked, Counts};
use starlark_syntax as _;
use yaml_serde as _;

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/ci/rule_set_loads_allowlist.txt";

const FOREIGN: &str = "foreign-load";
const REACHES: &str = "reaches-rule-set";
const KINDS: [&str; 2] = [FOREIGN, REACHES];

/// Path prefixes of the files that become the `rules_<lang>` modules, per language. A prefix ending in `/` is a directory; any other is one file.
const RULE_SETS: [(&str, &[&str]); 3] = [
    (
        "php",
        &[
            "php/BUILD.bazel",
            "php/defs.bzl",
            "php/extensions.bzl",
            "php/private/BUILD.bazel",
            "php/private/defs.bzl",
            "php/private/php.bzl",
            "php/private/providers.bzl",
            "php/private/lock/",
            "php/private/toolchain/",
        ],
    ),
    (
        "lua",
        &[
            "lua/BUILD.bazel",
            "lua/defs.bzl",
            "lua/extensions.bzl",
            "lua/private/BUILD.bazel",
            "lua/private/defs.bzl",
            "lua/private/lua.bzl",
            "lua/private/providers.bzl",
            "lua/private/lock/",
            "lua/private/runtime/",
            "lua/private/toolchain/",
        ],
    ),
    (
        "qt",
        &[
            "qt/BUILD.bazel",
            "qt/defs.bzl",
            "qt/extensions.bzl",
            "qt/private/",
        ],
    ),
];

/// The only rules_dx files a rule-set file may load, as `//` labels.
const RULES_DX_ALLOW: [&str; 3] = [
    "//core:wrapper.bzl",
    "//core/quality:sources.bzl",
    "//rust:defs.bzl",
];

/// External repos a rule-set file may load from.
const EXTERNAL_ALLOW: [&str; 8] = [
    "bazel_lib",
    "bazel_skylib",
    "bazel_tools",
    "crates",
    "llvm",
    "platforms",
    "rules_cc",
    "rules_shell",
];

/// Fewer scanned rule-set files than this means the sweep lost the tree.
const MIN_FILES: usize = 40;

/// The language that owns `path` as a rule-set file.
fn rule_set_of(path: &str) -> Option<&'static str> {
    RULE_SETS.iter().find_map(|(lang, prefixes)| {
        prefixes
            .iter()
            .any(|prefix| {
                if prefix.ends_with('/') {
                    path.starts_with(prefix)
                } else {
                    path == *prefix
                }
            })
            .then_some(*lang)
    })
}

/// The first string literal of a `load(` call.
fn first_literal(args: &str) -> Option<&str> {
    let args = args.trim_start();
    let quote = args.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let rest = &args[1..];
    Some(&rest[..rest.find(quote)?])
}

/// A load label as `//pkg:file` for rules_dx files and relative loads, or `@repo//pkg:file` for other repos.
fn normalize(label: &str, dir: &str) -> String {
    if let Some(rest) = label.strip_prefix(':') {
        return format!("//{dir}:{rest}");
    }
    for own in ["@@rules_dx//", "@rules_dx//"] {
        if let Some(rest) = label.strip_prefix(own) {
            return format!("//{rest}");
        }
    }
    label.to_owned()
}

/// The repo path of an in-repo `//pkg:file` label.
fn file_of(label: &str) -> Option<String> {
    let rest = label.strip_prefix("//")?;
    Some(match rest.split_once(':') {
        Some(("", name)) => name.to_owned(),
        Some((package, name)) => format!("{package}/{name}"),
        None => rest.to_owned(),
    })
}

/// The directory of a repo path.
fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Normalised labels loaded by `text`, whose file lives in `path`.
fn loads(path: &str, text: &str) -> Vec<String> {
    let source = masked(text);
    calls(&source, "load")
        .iter()
        .filter_map(|call| first_literal(call.code))
        .map(|label| normalize(label, dir_of(path)))
        .collect()
}

/// True when a rule-set file of `lang` may load `label`.
fn allowed(lang: &str, label: &str) -> bool {
    if let Some(rest) = label.strip_prefix("//") {
        let package = rest.split(':').next().unwrap_or(rest);
        let top = package.split('/').next().unwrap_or(package);
        return top == lang || RULES_DX_ALLOW.contains(&label);
    }
    label
        .strip_prefix("@@")
        .or_else(|| label.strip_prefix('@'))
        .and_then(|rest| rest.split("//").next())
        .is_some_and(|repo| EXTERNAL_ALLOW.contains(&repo))
}

/// The hits in `files`, a `(path, text)` list, and a line per hit that names the file and the load.
fn scan_files(files: &[(String, String)]) -> (Counts, Vec<String>) {
    let mut counts = Counts::new();
    let mut details = Vec::new();
    let texts: BTreeMap<&str, &str> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    for (path, text) in files {
        let Some(lang) = rule_set_of(path) else {
            continue;
        };
        if !is_starlark(path) || is_skipped(path) {
            continue;
        }
        for label in loads(path, text) {
            if !allowed(lang, &label) {
                add(&mut counts, path, FOREIGN, 1);
                details.push(format!("{path} loads {label}"));
            }
        }
    }
    for root in RULES_DX_ALLOW.iter().filter_map(|label| file_of(label)) {
        let mut parents: BTreeMap<String, String> = BTreeMap::new();
        let mut queue = VecDeque::from([root.clone()]);
        let mut seen = BTreeSet::from([root.clone()]);
        while let Some(path) = queue.pop_front() {
            let Some(text) = texts.get(path.as_str()) else {
                continue;
            };
            for label in loads(&path, text) {
                let Some(next) = file_of(&label) else {
                    continue;
                };
                if !seen.insert(next.clone()) {
                    continue;
                }
                parents.insert(next.clone(), path.clone());
                if rule_set_of(&next).is_some() {
                    let mut chain = vec![next.clone()];
                    let mut at = &next;
                    while let Some(parent) = parents.get(at) {
                        chain.push(parent.clone());
                        at = parent;
                    }
                    chain.reverse();
                    add(&mut counts, &root, REACHES, 1);
                    details.push(format!("{root} reaches {}", chain.join(" -> ")));
                } else {
                    queue.push_back(next);
                }
            }
        }
    }
    (counts, details)
}

#[test]
fn rule_set_files_load_only_what_the_allowlist_permits() {
    let root = dx_testing::source_snapshot();
    let files = scan::tree(&root);
    let scanned = files
        .iter()
        .filter(|(path, _)| rule_set_of(path).is_some() && is_starlark(path))
        .count();
    assert!(
        scanned >= MIN_FILES,
        "only {scanned} rule-set files in the snapshot"
    );
    for needed in ["php/defs.bzl", "lua/defs.bzl", "qt/defs.bzl", "rust/defs.bzl"] {
        assert!(
            files.iter().any(|(path, _)| path == needed),
            "the snapshot has no {needed}"
        );
    }
    let (found, details) = scan_files(&files);
    let allowlist = dx_testing::read_runfiles(ALLOWLIST);
    let problems = scan::problems(&found, &allowlist, &KINDS, ALLOWLIST);
    assert!(
        problems.is_empty(),
        "{}\n\nloads:\n{}\n\nwanted {ALLOWLIST}:\n{}",
        problems.join("\n"),
        details.join("\n"),
        scan::render(&found)
    );
}

fn fixture(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

#[test]
fn a_disallowed_load_is_counted_and_named() {
    let source = "load(\n    \"@rules_dx//core:x.bzl\",\n    x = \"x\",\n)\n\
        load(\"//php/private:providers.bzl\", \"P\")\n\
        load(\":php.bzl\", \"p\")\n\
        load(\"@rules_cc//cc:defs.bzl\", \"cc_library\")\n\
        load(\"@rules_dx//rust:defs.bzl\", \"rust_library\")\n\
        # load(\"//core:commented.bzl\", \"c\")\n\
        load(\"//tools:t.bzl\", \"t\")\n";
    let (counts, details) = scan_files(&fixture(&[("php/private/php.bzl", source)]));
    assert_eq!(
        scan::render(&counts),
        "php/private/php.bzl foreign-load 2\n"
    );
    assert_eq!(
        details,
        [
            "php/private/php.bzl loads //core:x.bzl",
            "php/private/php.bzl loads //tools:t.bzl"
        ]
    );
}

#[test]
fn files_outside_the_rule_sets_are_not_scanned() {
    let (counts, _) = scan_files(&fixture(&[
        ("php/private/quality/php.bzl", "load(\"//core:x.bzl\", \"x\")\n"),
        ("php/tests/t.bzl", "load(\"//core:x.bzl\", \"x\")\n"),
    ]));
    assert!(counts.is_empty());
}

#[test]
fn a_stale_allowlist_line_is_named() {
    let (counts, _) = scan_files(&fixture(&[("lua/defs.bzl", "load(\"//lua/private:lua.bzl\", \"l\")\n")]));
    let problems = scan::problems(
        &counts,
        "lua/defs.bzl foreign-load 1\n",
        &KINDS,
        ALLOWLIST,
    );
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("spare line `lua/defs.bzl foreign-load`"));
}

#[test]
fn a_new_violation_is_named() {
    let (counts, _) = scan_files(&fixture(&[("qt/defs.bzl", "load(\"//core:x.bzl\", \"x\")\n")]));
    let problems = scan::problems(&counts, "", &KINDS, ALLOWLIST);
    assert_eq!(
        problems,
        ["qt/defs.bzl has 1 foreign-load, tools/ci/rule_set_loads_allowlist.txt allows none"]
    );
}

#[test]
fn a_closure_that_reaches_a_rule_set_prints_the_chain() {
    let (counts, details) = scan_files(&fixture(&[
        ("rust/defs.bzl", "load(\"//core:x.bzl\", \"x\")\n"),
        ("core/x.bzl", "load(\"//lua/private:lua.bzl\", \"l\")\n"),
        ("lua/private/lua.bzl", ""),
    ]));
    assert_eq!(scan::render(&counts), "rust/defs.bzl reaches-rule-set 1\n");
    assert_eq!(
        details,
        ["rust/defs.bzl reaches rust/defs.bzl -> core/x.bzl -> lua/private/lua.bzl"]
    );
}
