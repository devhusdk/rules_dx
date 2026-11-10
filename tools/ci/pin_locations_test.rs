//! Every download carries a digest, every digest lives in a pins.json, no pin is written twice and no reference is mutable, outside tools/ci/pin_locations_allowlist.txt.

#[path = "guard_rules/scan.rs"]
mod scan;

use std::collections::{BTreeMap, BTreeSet};

use scan::{Counts, Masked, add, calls, is_skipped, is_starlark, masked};
use starlark_syntax as _;
use yaml_serde as _;

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/ci/pin_locations_allowlist.txt";

const NO_DIGEST: &str = "no-digest";
const OUTSIDE: &str = "digest-outside-pins";
const DUPLICATE: &str = "duplicate-pin";
const MUTABLE: &str = "mutable-ref";
const KINDS: [&str; 4] = [NO_DIGEST, OUTSIDE, DUPLICATE, MUTABLE];

/// Calls that download and take a `sha256` or `integrity` argument.
const DOWNLOADS: [&str; 6] = [
    "http_archive",
    "http_file",
    "http_jar",
    "archive_override",
    ".download",
    ".download_and_extract",
];

/// Calls that fetch a git ref.
const GIT_FETCHES: [&str; 3] = ["git_override", "git_repository", "new_git_repository"];

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 1000;

/// True when `value` is absent or an empty literal.
fn is_empty_value(value: Option<&str>) -> bool {
    matches!(value, None | Some("" | "\"\"" | "''" | "None"))
}

fn is_hex(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

fn base64_value(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some(u32::from(byte - b'A')),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// The lowercase hex of a 43 character base64 SHA-256, or None when it is not base64.
fn sri_hex(base64: &[u8]) -> Option<String> {
    let mut bits = 0u32;
    let mut held = 0;
    let mut out = String::new();
    for byte in base64 {
        bits = (bits << 6) | base64_value(*byte)?;
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push_str(&format!("{:02x}", (bits >> held) & 0xff));
        }
    }
    Some(out)
}

/// Every SHA-256 literal in `text` as lowercase hex: a run of exactly 64 hex characters, or `sha256-` and 43 base64 characters and `=`.
fn digests(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at..].starts_with(b"sha256-") && bytes.len() >= at + 51 && bytes[at + 50] == b'=' {
            if let Some(hex) = sri_hex(&bytes[at + 7..at + 50]) {
                out.push(hex);
                at += 51;
                continue;
            }
        }
        if bytes[at].is_ascii_alphanumeric() {
            let start = at;
            while at < bytes.len() && bytes[at].is_ascii_alphanumeric() {
                at += 1;
            }
            let run = &bytes[start..at];
            if run.len() == 64 && run.iter().all(|byte| is_hex(*byte)) {
                out.push(text[start..at].to_ascii_lowercase());
            }
        } else {
            at += 1;
        }
    }
    out
}

/// Download calls in `source` with no `sha256` or `integrity` argument, or with an empty one.
fn downloads_without_digest(source: &Masked) -> usize {
    DOWNLOADS
        .iter()
        .flat_map(|name| calls(source, name))
        .filter(|call| {
            is_empty_value(call.keyword("sha256")) && is_empty_value(call.keyword("integrity"))
        })
        .count()
}

/// True when `text` holds `@sha256:` and 64 hex characters.
fn has_image_digest(text: &str) -> bool {
    text.split("@sha256:")
        .skip(1)
        .any(|rest| rest.len() >= 64 && rest.bytes().take(64).all(is_hex))
}

/// Mutable references in Starlark: an `oci.pull` with no digest, a git fetch by branch, a `docker://` string with no digest and a `:latest` string.
fn mutable_starlark(source: &Masked) -> usize {
    let pulls = calls(source, ".pull")
        .iter()
        .filter(|call| {
            !call
                .keyword("digest")
                .is_some_and(|value| value.contains("sha256:"))
        })
        .count();
    let branches = GIT_FETCHES
        .iter()
        .flat_map(|name| calls(source, name))
        .filter(|call| !is_empty_value(call.keyword("branch")))
        .count();
    let strings = source
        .code
        .lines()
        .filter(|line| {
            (line.contains("docker://") && !has_image_digest(line))
                || line.contains(":latest\"")
                || line.contains("tag = \"latest\"")
        })
        .count();
    pulls + branches + strings
}

/// Container images in a bazelrc with no digest.
fn mutable_bazelrc(text: &str) -> usize {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter(|line| line.contains("docker://") || line.contains("container-image="))
        .filter(|line| !has_image_digest(line))
        .count()
}

/// Workflow `uses:` lines that are not a 40 hex commit, and `image:` and `container:` scalars with no digest.
fn mutable_workflow(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let line = line.trim_start();
            let line = line.strip_prefix("- ").unwrap_or(line).trim_start();
            let line = line.split(" #").next().unwrap_or(line).trim_end();
            if let Some(target) = line.strip_prefix("uses:") {
                let target = target.trim().trim_matches(['"', '\'']);
                if target.starts_with("./") {
                    return false;
                }
                if target.starts_with("docker://") {
                    return !has_image_digest(target);
                }
                return !target
                    .rsplit_once('@')
                    .is_some_and(|(_, commit)| commit.len() == 40 && commit.bytes().all(is_hex));
            }
            ["image:", "container:"]
                .iter()
                .filter_map(|key| line.strip_prefix(key))
                .any(|value| !value.trim().is_empty() && !has_image_digest(value))
        })
        .count()
}

/// The `name version` pair of every pins.json entry that carries a `version` string.
fn pin_pairs(value: &serde_json::Value, key: Option<&str>, out: &mut BTreeSet<String>) {
    let Some(object) = value.as_object() else {
        return;
    };
    if let (Some(name), Some(version)) = (key, object.get("version").and_then(|v| v.as_str())) {
        out.insert(format!("{name} {version}"));
    }
    for (name, child) in object {
        pin_pairs(child, Some(name), out);
    }
}

/// The hits in `files`, a `(path, text)` list.
fn scan_files(files: &[(String, String)]) -> Counts {
    let mut counts = Counts::new();
    let mut digest_files: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    let mut pair_files: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for (path, text) in files {
        let name = path.rsplit('/').next().unwrap_or(path);
        if path == ".bazelrc" {
            add(&mut counts, path, MUTABLE, mutable_bazelrc(text));
            continue;
        }
        if path.starts_with(".github/workflows/") {
            add(&mut counts, path, MUTABLE, mutable_workflow(text));
            continue;
        }
        if is_skipped(path) {
            continue;
        }
        let pins = name == "pins.json";
        let rust = name.ends_with(".rs");
        if !pins && !rust && !is_starlark(path) {
            continue;
        }
        let found = digests(text);
        for digest in &found {
            digest_files.entry(digest.clone()).or_default().insert(path);
        }
        if pins {
            let mut pairs = BTreeSet::new();
            let value: serde_json::Value =
                serde_json::from_str(text).unwrap_or_else(|error| panic!("{path}: {error}"));
            pin_pairs(&value, None, &mut pairs);
            for pair in pairs {
                pair_files.entry(pair).or_default().insert(path);
            }
            continue;
        }
        add(&mut counts, path, OUTSIDE, found.len());
        if !rust {
            let source = masked(text);
            add(
                &mut counts,
                path,
                NO_DIGEST,
                downloads_without_digest(&source),
            );
            add(&mut counts, path, MUTABLE, mutable_starlark(&source));
        }
    }
    for holders in digest_files.values().chain(pair_files.values()) {
        if holders.len() > 1 {
            for path in holders {
                add(&mut counts, path, DUPLICATE, 1);
            }
        }
    }
    counts
}

#[test]
fn pins_have_one_location_outside_the_allowlist() {
    let root = dx_testing::source_snapshot();
    let files = scan::tree(&root);
    assert!(
        files.len() >= MIN_FILES,
        "only {} files in the snapshot",
        files.len()
    );
    for needed in [
        ".bazelrc",
        ".github/workflows/ci.yml",
        "deploy/private/release/pins.json",
    ] {
        assert!(
            files.iter().any(|(path, _)| path == needed),
            "the snapshot has no {needed}"
        );
    }
    assert!(
        files.iter().any(|(path, _)| path.ends_with(".rs")),
        "the snapshot has no .rs file"
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

const HEX_A: &str = "f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158";
const HEX_B: &str = "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0";

fn fixture(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

fn hits(files: &[(&str, &str)]) -> String {
    scan::render(&scan_files(&fixture(files)))
}

#[test]
fn a_download_without_a_digest_is_counted() {
    let source = "http_archive(\n    name = \"a\",\n    urls = [\"https://example.com/a.tar.gz\"],\n)\n\
        http_file(name = \"b\", sha256 = \"\", url = \"https://example.com/b\")\n\
        def _impl(rctx):\n    rctx.download(url = rctx.attr.url, output = \"c\")\n    rctx.download_and_extract(url = rctx.attr.url, sha256 = rctx.attr.sha256)\n\
        # http_jar(name = \"commented\")\n\
        def http_jar(name):\n    pass\n\
        archive_override(module_name = \"m\", integrity = PINS[\"m\"])\n";
    assert_eq!(
        hits(&[("x/repos.bzl", source)]),
        "x/repos.bzl no-digest 3\n"
    );
}

#[test]
fn a_digest_outside_pins_json_is_counted() {
    let bzl = format!(
        "PIN = \"{HEX_A}\"\nSRI = \"sha256-V8zDK7vQBcq3W8xSREBSU1r2kXiduiuQFtXFBkDWiz0=\"\n"
    );
    let rust = format!("const PIN: &str = \"{HEX_B}\";\n");
    let pins =
        format!("{{\"archives\": {{\"tool\": {{\"sha256\": \"{HEX_A}\", \"version\": \"1\"}}}}}}");
    let test_file = format!("PIN = \"{HEX_B}\"\n");
    assert_eq!(
        hits(&[
            ("x/pin.bzl", &bzl),
            ("x/src/lib.rs", &rust),
            ("x/tests/pin.bzl", &test_file),
            (
                "y/BUILD.bazel",
                "filegroup(name = \"commit\", srcs = [\"0123456789abcdef0123456789abcdef01234567\"])\n"
            ),
            (
                "z/pins.json",
                &pins.replace(HEX_A, HEX_B).replace("tool", "other")
            ),
        ]),
        "x/pin.bzl digest-outside-pins 2\nx/src/lib.rs digest-outside-pins 1\nx/src/lib.rs duplicate-pin 1\nz/pins.json duplicate-pin 1\n"
    );
}

#[test]
fn a_digest_or_a_name_and_version_in_two_files_is_counted() {
    let pins = |name: &str, hex: &str| {
        format!("{{\"archives\": {{\"{name}\": {{\"sha256\": \"{hex}\", \"version\": \"1.2\"}}}}}}")
    };
    assert_eq!(
        hits(&[
            ("a/pins.json", &pins("tool", HEX_A)),
            ("b/pins.json", &pins("tool", HEX_B))
        ]),
        "a/pins.json duplicate-pin 1\nb/pins.json duplicate-pin 1\n"
    );
    assert_eq!(
        hits(&[
            ("a/pins.json", &pins("one", HEX_A)),
            ("b/pins.json", &pins("two", HEX_A))
        ]),
        "a/pins.json duplicate-pin 1\nb/pins.json duplicate-pin 1\n"
    );
    assert_eq!(
        hits(&[
            ("a/pins.json", &pins("one", HEX_A)),
            ("b/pins.json", &pins("two", HEX_B))
        ]),
        ""
    );
    let sri = "PIN = \"sha256-V8zDK7vQBcq3W8xSREBSU1r2kXiduiuQFtXFBkDWiz0=\"\n";
    let hex = "PIN = \"57ccc32bbbd005cab75bcc52444052535af691789dba2b9016d5c50640d68b3d\"\n";
    assert!(hits(&[("a.bzl", sri), ("b.bzl", hex)]).contains("b.bzl duplicate-pin 1\n"));
}

#[test]
fn a_mutable_reference_is_counted() {
    let module = format!(
        "oci.pull(name = \"pinned\", digest = \"sha256:{HEX_A}\", image = \"gcr.io/a\")\n\
        oci.pull(name = \"floating\", image = \"gcr.io/b\", tag = \"latest\")\n\
        git_override(module_name = \"m\", remote = \"https://example.com/m.git\", branch = \"main\")\n\
        git_override(module_name = \"n\", remote = \"https://example.com/n.git\", commit = \"0123456789abcdef0123456789abcdef01234567\")\n\
        IMAGE = \"docker://gcr.io/c\"\n"
    );
    let workflow = "jobs:\n  a:\n    container: ubuntu:24.04\n    steps:\n      - uses: actions/checkout@v4\n      - uses: actions/checkout@0123456789abcdef0123456789abcdef01234567 # v4\n      - uses: ./.github/actions/setup\n      - uses: owner/repo/.github/workflows/x.yml@main\n";
    let bazelrc = format!(
        "common:rbe --remote_default_exec_properties=container-image=docker://gcr.io/a@sha256:{HEX_A}\ncommon:other --remote_default_exec_properties=container-image=docker://gcr.io/a:latest\n"
    );
    let found = scan_files(&fixture(&[
        ("MODULE.bazel", &module),
        (".github/workflows/ci.yml", workflow),
        (".bazelrc", &bazelrc),
    ]));
    let count = |path: &str| found.get(&(path.to_owned(), MUTABLE.to_owned())).copied();
    assert_eq!(count("MODULE.bazel"), Some(4), "{found:?}");
    assert_eq!(count(".github/workflows/ci.yml"), Some(3), "{found:?}");
    assert_eq!(count(".bazelrc"), Some(1), "{found:?}");
}

#[test]
fn the_allowlist_only_shrinks() {
    let found = scan_files(&fixture(&[(
        "x/repos.bzl",
        "http_archive(name = \"a\")\nhttp_archive(name = \"b\")\n",
    )]));
    let check = |allowlist: &str| scan::problems(&found, allowlist, &KINDS, ALLOWLIST);
    assert_eq!(check("x/repos.bzl no-digest 2\n"), Vec::<String>::new());
    for (allowlist, needle) in [
        ("", "allows none"),
        ("x/repos.bzl no-digest 1\n", "allows 1"),
        ("x/repos.bzl no-digest 3\n", "lower the line"),
        (
            "x/repos.bzl no-digest 2\ny/repos.bzl no-digest 1\n",
            "spare line",
        ),
        (
            "x/repos.bzl mutable-ref 1\nx/repos.bzl no-digest 2\n",
            "spare line",
        ),
        (
            "x/repos.bzl no-digest 2\nx/repos.bzl no-digest 2\n",
            "not sorted",
        ),
        ("x/repos.bzl unknown 2\n", "is not `<path> <kind> <count>`"),
        (
            "x/repos.bzl no-digest 0\n",
            "is not `<path> <kind> <count>`",
        ),
        (
            "x/repos.bzl  no-digest 2\n",
            "is not `<path> <kind> <count>`",
        ),
        (
            "# x\nx/repos.bzl no-digest 2\n",
            "is not `<path> <kind> <count>`",
        ),
    ] {
        let problems = check(allowlist);
        assert!(
            problems.iter().any(|problem| problem.contains(needle)),
            "{allowlist:?} {needle}: {problems:?}"
        );
    }
}
