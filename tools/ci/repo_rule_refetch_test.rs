//! No repository rule or module extension refetches: none is configure or local, reads the environment, watches a path or omits reproducible metadata, outside tools/ci/repo_rule_refetch_allowlist.txt.

#[path = "guard_rules/scan.rs"]
mod scan;

use scan::{CallArgs, Counts, Masked, add, calls, is_skipped, is_starlark, masked};
use serde_json as _;
use starlark_syntax as _;
use yaml_serde as _;

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/ci/repo_rule_refetch_allowlist.txt";

const CONFIGURE: &str = "configure";
const LOCAL: &str = "local";
const ENVIRON: &str = "environ";
const NOT_REPRODUCIBLE: &str = "not-reproducible";
const WATCH: &str = "watch";
const KINDS: [&str; 5] = [CONFIGURE, LOCAL, ENVIRON, NOT_REPRODUCIBLE, WATCH];

/// The file of the `@dx_test_tags_sources` rule, which watches the tree it snapshots.
const SNAPSHOT_RULE: &str = "tools/ci/repos.bzl";

/// Each definition call and the metadata call its implementation returns.
const DEFINITIONS: [(&str, &str); 2] = [
    ("repository_rule", ".repo_metadata"),
    ("module_extension", ".extension_metadata"),
];

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 1000;

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Assignments of `value` to the keyword `key` in `mask`.
fn count_keyword(mask: &str, key: &str, value: &str) -> usize {
    mask.match_indices(key)
        .filter(|(at, _)| !mask[..*at].bytes().last().is_some_and(is_ident))
        .filter(|(at, _)| {
            mask[at + key.len()..]
                .trim_start_matches(' ')
                .strip_prefix('=')
                .is_some_and(|rest| {
                    let rest = rest.trim_start_matches(' ');
                    rest.strip_prefix(value).is_some_and(|after| {
                        !value.bytes().last().is_some_and(is_ident)
                            || !after.bytes().next().is_some_and(is_ident)
                    })
                })
        })
        .count()
}

/// The body of the top-level `def name(` in `source`, strings blanked, or None when it is defined elsewhere.
fn body<'a>(source: &'a Masked, name: &str) -> Option<&'a str> {
    let header = format!("def {name}(");
    let start = source
        .mask
        .match_indices(&header)
        .map(|(at, _)| at)
        .find(|at| *at == 0 || source.mask.as_bytes()[at - 1] == b'\n')?;
    let rest = &source.mask[start..];
    let mut end = 0;
    for (index, line) in rest.split_inclusive('\n').enumerate() {
        if index > 0 && !line.trim().is_empty() && !line.starts_with([' ', '\t', ')']) {
            break;
        }
        end += line.len();
    }
    Some(&rest[..end])
}

/// The implementation named by a `repository_rule` or `module_extension` call.
fn implementation(call: &CallArgs<'_>) -> String {
    call.keyword("implementation").map_or_else(
        || {
            let end = call.mask.find(',').unwrap_or(call.mask.len());
            call.code[..end].trim().to_owned()
        },
        str::to_owned,
    )
}

/// Definitions in `source` whose implementation does not return reproducible metadata.
fn not_reproducible(source: &Masked) -> usize {
    DEFINITIONS
        .iter()
        .map(|(definition, metadata)| {
            calls(source, definition)
                .iter()
                .filter(|call| {
                    let Some(body) = body(source, &implementation(call)) else {
                        return true;
                    };
                    let body = Masked {
                        code: body.to_owned(),
                        mask: body.to_owned(),
                    };
                    !calls(&body, metadata)
                        .iter()
                        .any(|call| call.keyword("reproducible") == Some("True"))
                })
                .count()
        })
        .sum()
}

/// Watches in `source`: `.watch(`, `.watch_tree(` and `.path(` on a label of the main repository or of the calling package.
fn watches(source: &Masked) -> usize {
    let labels = calls(source, ".path")
        .iter()
        .filter(|call| {
            let argument = call.code.trim_start();
            ["\"//", "\"@//", "Label(\"//", "Label(\"@//", "Label(\":"]
                .iter()
                .any(|prefix| argument.starts_with(prefix))
        })
        .count();
    calls(source, ".watch").len() + calls(source, ".watch_tree").len() + labels
}

/// The hits in `files`, a `(path, text)` list.
fn scan_files(files: &[(String, String)]) -> Counts {
    let mut counts = Counts::new();
    for (path, text) in files {
        if !path.ends_with(".bzl") || !is_starlark(path) || is_skipped(path) {
            continue;
        }
        let source = masked(text);
        add(
            &mut counts,
            path,
            CONFIGURE,
            count_keyword(&source.mask, "configure", "True"),
        );
        add(
            &mut counts,
            path,
            LOCAL,
            count_keyword(&source.mask, "local", "True"),
        );
        let environ = count_keyword(&source.mask, "environ", "[")
            + calls(&source, ".getenv").len()
            + source.mask.matches(".os.environ").count();
        add(&mut counts, path, ENVIRON, environ);
        add(
            &mut counts,
            path,
            NOT_REPRODUCIBLE,
            not_reproducible(&source),
        );
        if path != SNAPSHOT_RULE {
            add(&mut counts, path, WATCH, watches(&source));
        }
    }
    counts
}

#[test]
fn repository_rules_do_not_refetch_outside_the_allowlist() {
    let root = dx_testing::source_snapshot();
    let files = scan::tree(&root);
    assert!(
        files.len() >= MIN_FILES,
        "only {} files in the snapshot",
        files.len()
    );
    assert!(
        files.iter().any(|(path, _)| path == SNAPSHOT_RULE),
        "the snapshot has no {SNAPSHOT_RULE}"
    );
    let found = scan_files(&files);
    let allowlist = dx_testing::read_runfiles(ALLOWLIST);
    let problems = scan::problems(&found, &allowlist, &KINDS, ALLOWLIST);
    assert!(
        problems.is_empty(),
        "{}\n\nwanted {ALLOWLIST}:\n{}",
        problems.join("\n"),
        scan::render(&found)
    );
}

const GOOD: &str = "def _impl(rctx):\n    rctx.file(\"BUILD.bazel\", \"\")\n    return rctx.repo_metadata(reproducible = True)\n\ngood = repository_rule(\n    implementation = _impl,\n    attrs = {\"local\": attr.bool()},\n)\n\ndef _ext(mctx):\n    good(name = \"a\")\n    return mctx.extension_metadata(\n        reproducible = True,\n    )\n\next = module_extension(implementation = _ext)\n";

fn hits(files: &[(&str, &str)]) -> String {
    let files: Vec<(String, String)> = files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect();
    scan::render(&scan_files(&files))
}

#[test]
fn a_reproducible_rule_and_extension_have_no_hit() {
    assert_eq!(hits(&[("x/repos.bzl", GOOD)]), "");
}

#[test]
fn configure_and_local_are_counted() {
    let source = GOOD.replace(
        "    implementation = _impl,\n",
        "    implementation = _impl,\n    configure = True,\n    local=True,\n",
    );
    assert_eq!(
        hits(&[("x/repos.bzl", &source)]),
        "x/repos.bzl configure 1\nx/repos.bzl local 1\n"
    );
    let off = GOOD.replace(
        "    implementation = _impl,\n",
        "    implementation = _impl,\n    configure = False,\n    # local = True\n",
    );
    assert_eq!(hits(&[("x/repos.bzl", &off)]), "");
}

#[test]
fn environment_reads_are_counted() {
    let source = GOOD
        .replace(
            "    implementation = _impl,\n",
            "    implementation = _impl,\n    environ = [\"HOME\"],\n",
        )
        .replace(
            "    rctx.file(\"BUILD.bazel\", \"\")\n",
            "    rctx.getenv(\"HOME\")\n    rctx.os.environ.get(\"PATH\")\n",
        );
    assert_eq!(hits(&[("x/repos.bzl", &source)]), "x/repos.bzl environ 3\n");
}

#[test]
fn missing_reproducible_metadata_is_counted() {
    for (from, to) in [
        ("    return rctx.repo_metadata(reproducible = True)\n", ""),
        (
            "rctx.repo_metadata(reproducible = True)",
            "rctx.repo_metadata(reproducible = False)",
        ),
        (
            "    return mctx.extension_metadata(\n        reproducible = True,\n    )\n",
            "    return None\n",
        ),
        (
            "implementation = _ext",
            "implementation = _loaded_elsewhere",
        ),
    ] {
        assert_eq!(
            hits(&[("x/repos.bzl", &GOOD.replace(from, to))]),
            "x/repos.bzl not-reproducible 1\n",
            "{from:?}"
        );
    }
    let positional = GOOD.replace(
        "module_extension(implementation = _ext)",
        "module_extension(_ext)",
    );
    assert_eq!(hits(&[("x/repos.bzl", &positional)]), "");
}

#[test]
fn watches_are_counted_outside_the_snapshot_rule() {
    let source = GOOD.replace(
        "    rctx.file(\"BUILD.bazel\", \"\")\n",
        "    rctx.watch(rctx.attr.src)\n    rctx.watch_tree(\"dir\")\n    rctx.path(Label(\"//:MODULE.bazel\"))\n    rctx.path(\"out\")\n",
    );
    assert_eq!(hits(&[("x/repos.bzl", &source)]), "x/repos.bzl watch 3\n");
    assert_eq!(hits(&[(SNAPSHOT_RULE, &source)]), "");
    assert_eq!(hits(&[("x/tests/repos.bzl", &source)]), "");
}

#[test]
fn the_allowlist_only_shrinks() {
    let source = GOOD.replace(
        "    implementation = _impl,\n",
        "    implementation = _impl,\n    configure = True,\n",
    );
    let files = vec![("x/repos.bzl".to_owned(), source)];
    let found = scan_files(&files);
    let check = |allowlist: &str| scan::problems(&found, allowlist, &KINDS, ALLOWLIST);
    assert_eq!(check("x/repos.bzl configure 1\n"), Vec::<String>::new());
    for (allowlist, needle) in [
        ("", "allows none"),
        ("x/repos.bzl configure 2\n", "lower the line"),
        (
            "x/repos.bzl configure 1\nx/repos.bzl local 1\n",
            "spare line",
        ),
        (
            "x/repos.bzl local 1\nx/repos.bzl configure 1\n",
            "not sorted",
        ),
        ("x/repos.bzl refetch 1\n", "is not `<path> <kind> <count>`"),
    ] {
        let problems = check(allowlist);
        assert!(
            problems.iter().any(|problem| problem.contains(needle)),
            "{allowlist:?} {needle}: {problems:?}"
        );
    }
}
