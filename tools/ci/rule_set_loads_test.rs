use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use dx_guard::allowlist::{Counts, REMOVE, add, render};
use dx_guard::masked::{calls, masked};
use dx_guard::snapshot::{is_skipped, tree};
use dx_guard::starlark::{exprs, is_bazel_file, parse, string};
use serde_json as _;
use starlark_syntax::syntax::ast::{AstExpr, ExprP};
use yaml_serde as _;

const ALLOWLIST: &str = "tools/ci/rule_set_loads_allowlist.txt";

const FOREIGN: &str = "foreign-load";
const FOREIGN_LABEL: &str = "foreign-label";
const REACHES: &str = "reaches-rule-set";
const KINDS: [&str; 3] = [FOREIGN, FOREIGN_LABEL, REACHES];

const PACKAGE_NAMES: [&str; 2] = ["__pkg__", "__subpackages__"];

const BUILTIN_PACKAGES: [&str; 3] = ["visibility", "conditions", "command_line_option"];

const RULE_SETS: [(&str, &[&str]); 3] = [
    (
        "php",
        &[
            "php/BUILD.bazel",
            "php/defs.bzl",
            "php/extensions.bzl",
            "php/private/BUILD.bazel",
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

const RULES_DX_ALLOW: [&str; 9] = [
    "//cli/prebuilt:launcher.bzl",
    "//core:cells.bzl",
    "//core:entry.bzl",
    "//core:fuzz_bench.bzl",
    "//core:pins.bzl",
    "//core:platform.bzl",
    "//core:wrapper.bzl",
    "//core/quality:sources.bzl",
    "//rust:defs.bzl",
];

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

const MIN_FILES: usize = 40;

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

fn first_literal(args: &str) -> Option<&str> {
    let args = args.trim_start();
    let quote = args.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let rest = args.get(1..)?;
    rest.get(..rest.find(quote)?)
}

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

fn file_of(label: &str) -> Option<String> {
    let rest = label.strip_prefix("//")?;
    Some(match rest.split_once(':') {
        Some(("", name)) => name.to_owned(),
        Some((package, name)) => format!("{package}/{name}"),
        None => rest.to_owned(),
    })
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn loads(path: &str, text: &str) -> Vec<String> {
    let source = masked(text);
    calls(&source, "load")
        .iter()
        .filter_map(|call| first_literal(call.code))
        .map(|label| normalize(label, dir_of(path)))
        .collect()
}

fn label_strings(path: &str, text: &str) -> Vec<String> {
    let module = parse(path, text).unwrap_or_else(|error| panic!("{error}"));
    let all = exprs(module.statement());
    let operands: HashSet<*const AstExpr> = all
        .iter()
        .filter_map(|expr| {
            if let ExprP::Op(lhs, _, rhs) = &expr.node {
                Some([&**lhs as *const AstExpr, &**rhs as *const AstExpr])
            } else {
                None
            }
        })
        .flatten()
        .collect();
    all.into_iter()
        .filter(|expr| !operands.contains(&(*expr as *const AstExpr)))
        .filter_map(|expr| string(expr).map(str::to_owned))
        .collect()
}

fn foreign_label(lang: &str, s: &str) -> bool {
    if s.chars()
        .any(|c| c.is_whitespace() || matches!(c, '%' | '{' | '$'))
    {
        return false;
    }
    let Some((repo, rest)) = s.split_once("//") else {
        return false;
    };
    let repo = if repo.is_empty() {
        repo
    } else if let Some(name) = repo.strip_prefix("@@").or_else(|| repo.strip_prefix('@')) {
        name
    } else {
        return false;
    };
    if repo.contains(['@', '/', ':']) {
        return false;
    }
    let (package, name) = rest.split_once(':').unwrap_or((rest, ""));
    if PACKAGE_NAMES.contains(&name) || BUILTIN_PACKAGES.contains(&package) {
        return false;
    }
    if repo.starts_with("dx_") {
        return true;
    }
    if !repo.is_empty() && repo != "rules_dx" {
        return false;
    }
    let build = if package.is_empty() {
        "BUILD.bazel".to_owned()
    } else {
        format!("{package}/BUILD.bazel")
    };
    rule_set_of(&build) != Some(lang)
}

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
        if !is_bazel_file(path) || is_skipped(path) {
            continue;
        }
        for label in loads(path, text) {
            if !allowed(lang, &label) {
                add(&mut counts, path, FOREIGN, 1);
                details.push(format!("{path} loads {label}"));
            }
        }
        for label in label_strings(path, text) {
            if foreign_label(lang, &label) {
                add(&mut counts, path, FOREIGN_LABEL, 1);
                details.push(format!("{path} names {label}"));
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
    let files = tree(&root).unwrap_or_else(|error| panic!("{}: {error}", root.display()));
    let scanned = files
        .iter()
        .filter(|(path, _)| rule_set_of(path).is_some() && is_bazel_file(path))
        .count();
    assert!(
        scanned >= MIN_FILES,
        "only {scanned} rule-set files in the snapshot"
    );
    for needed in [
        "php/defs.bzl",
        "lua/defs.bzl",
        "qt/defs.bzl",
        "rust/defs.bzl",
    ] {
        assert!(
            files.iter().any(|(path, _)| path == needed),
            "the snapshot has no {needed}"
        );
    }
    let (found, details) = scan_files(&files);
    let allowlist = dx_testing::read_runfiles(ALLOWLIST);
    let problems = dx_guard::allowlist::check(&found, &allowlist, &KINDS, REMOVE);
    assert!(
        problems.is_empty(),
        "{}\n\nloads:\n{}\n\nwanted {ALLOWLIST}:\n{}",
        problems.join("\n"),
        details.join("\n"),
        render(&found)
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
    assert_eq!(render(&counts), "php/private/php.bzl foreign-load 2\n");
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
        (
            "php/private/quality/php.bzl",
            "load(\"//core:x.bzl\", \"x\")\n",
        ),
        ("php/tests/t.bzl", "load(\"//core:x.bzl\", \"x\")\n"),
    ]));
    assert!(counts.is_empty());
}

#[test]
fn a_stale_allowlist_line_is_named() {
    let (counts, _) = scan_files(&fixture(&[(
        "lua/defs.bzl",
        "load(\"//lua/private:lua.bzl\", \"l\")\n",
    )]));
    let problems =
        dx_guard::allowlist::check(&counts, "lua/defs.bzl foreign-load 1\n", &KINDS, REMOVE);
    assert_eq!(problems.len(), 1);
    assert!(problems[0] == "lua/defs.bzl foreign-load 1: none left; delete the allowlist line");
}

#[test]
fn a_new_violation_is_named() {
    let (counts, _) = scan_files(&fixture(&[(
        "qt/defs.bzl",
        "load(\"//core:x.bzl\", \"x\")\n",
    )]));
    let problems = dx_guard::allowlist::check(&counts, "", &KINDS, REMOVE);
    assert_eq!(
        problems,
        ["qt/defs.bzl foreign-load 1: 0 allowed; remove the new one"]
    );
}

#[test]
fn a_closure_that_reaches_a_rule_set_prints_the_chain() {
    let (counts, details) = scan_files(&fixture(&[
        ("rust/defs.bzl", "load(\"//core:x.bzl\", \"x\")\n"),
        ("core/x.bzl", "load(\"//lua/private:lua.bzl\", \"l\")\n"),
        ("lua/private/lua.bzl", ""),
    ]));
    assert_eq!(render(&counts), "rust/defs.bzl reaches-rule-set 1\n");
    assert_eq!(
        details,
        ["rust/defs.bzl reaches rust/defs.bzl -> core/x.bzl -> lua/private/lua.bzl"]
    );
}

#[test]
fn a_foreign_label_string_is_counted_and_named() {
    let source = "load(\"//lua/private:providers.bzl\", \"L\")\n\
        X = Label(\"//lua/private/coverage:libs\")\n\
        Y = [\"@dx_luacov//:sources\", \"@rules_dx//core:x\"]\n\
        Z = \"//lua:toolchain_type\"\n\
        V = [\"//visibility:public\", \"//core:__pkg__\", \"@platforms//os:linux\"]\n\
        T = \"//%s:x\" % \"core\"\n\
        U = \"//core\" + \":x\"\n\
        W = \"//conditions:default\"\n";
    let (counts, details) = scan_files(&fixture(&[("lua/private/lua.bzl", source)]));
    assert_eq!(render(&counts), "lua/private/lua.bzl foreign-label 3\n");
    assert_eq!(
        details,
        [
            "lua/private/lua.bzl names //lua/private/coverage:libs",
            "lua/private/lua.bzl names @dx_luacov//:sources",
            "lua/private/lua.bzl names @rules_dx//core:x"
        ]
    );
}

#[test]
fn a_root_package_label_is_foreign() {
    let (counts, details) = scan_files(&fixture(&[(
        "qt/BUILD.bazel",
        "filegroup(name = \"x\", srcs = [\"//:module_srcs\"])\n",
    )]));
    assert_eq!(render(&counts), "qt/BUILD.bazel foreign-label 1\n");
    assert_eq!(details, ["qt/BUILD.bazel names //:module_srcs"]);
}
