//! Every .bzl file is loaded, every fixture file and Rust file is named by a target or a test, every filegroup and exported file has a user, and every directory belongs to a package, outside tools/ci/unreferenced_files_allowlist.txt.

use std::collections::{BTreeMap, BTreeSet};

use serde_json as _;
use shared::{Counts, Piece, Segment, add, glob_match, is_starlark};
use starlark_syntax as _;
use yaml_serde as _;

#[path = "guards/shared.rs"]
mod shared;

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/ci/unreferenced_files_allowlist.txt";

/// The kinds an allowlist line may name.
const KINDS: [&str; 5] = [
    "empty-package",
    "orphan-target",
    "unbuilt-rust",
    "unloaded-bzl",
    "unreferenced-fixture",
];

/// Globs of the trees that hold fixtures.
const FIXTURE_TREES: [&str; 6] = [
    "**/tests/fixtures/**",
    "core/quality/testdata/**",
    "tools/ci/*_fixtures/**",
    "tools/docs/*/fixture/**",
    "tools/testing/fixtures/**",
    "tools/testing/hermetic/consumers/**",
];

/// Calls whose first argument names a .bzl file.
const LOADERS: [&str; 3] = ["load(", "use_extension(", "use_repo_rule("];

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 3000;

fn ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The directory of `path`, `""` at the root.
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn join(dir: &str, rel: &str) -> String {
    if dir.is_empty() {
        rel.to_owned()
    } else {
        format!("{dir}/{rel}")
    }
}

/// `path` relative to its ancestor `dir`.
fn below<'a>(path: &'a str, dir: &str) -> &'a str {
    if dir.is_empty() {
        path
    } else {
        &path[dir.len() + 1..]
    }
}

/// `dir` and every directory above it, the root `""` last.
fn ancestors(dir: &str) -> Vec<&str> {
    let mut out = vec![dir];
    let mut current = dir;
    while !current.is_empty() {
        current = parent(current);
        out.push(current);
    }
    out
}

/// `dir/rel` with `.` and `..` folded, or `None` when it leaves the tree.
fn normal(dir: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|part| !part.is_empty()).collect();
    for part in rel.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(parts.join("/"))
}

fn is_build(path: &str) -> bool {
    matches!(base(path), "BUILD" | "BUILD.bazel")
}

fn in_fixture_tree(path: &str) -> bool {
    FIXTURE_TREES.iter().any(|tree| glob_match(tree, path))
}

/// Whether `path` is `<lang>/defs.bzl` or `<lang>/extensions.bzl`.
fn public_entry(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    matches!(parts.as_slice(), [_, "defs.bzl" | "extensions.bzl"])
}

/// Every `pkg:name` that a `//pkg:name` or `//pkg` label in `text` spells.
fn labels_in(text: &str, out: &mut BTreeSet<String>) {
    let bytes = text.as_bytes();
    for (start, _) in text.match_indices("//") {
        if start > 0 && matches!(bytes[start - 1], b':' | b'/') {
            continue;
        }
        let rest = &text[start + 2..];
        let package_end = rest
            .find(|letter: char| !(letter.is_ascii_alphanumeric() || "_./+-".contains(letter)))
            .unwrap_or(rest.len());
        let package = rest[..package_end].trim_end_matches('.');
        let name = match rest[package_end..].strip_prefix(':') {
            Some(tail) => {
                let end = tail
                    .find(|letter: char| !(letter.is_ascii_alphanumeric() || "_./+=,@~#-".contains(letter)))
                    .unwrap_or(tail.len());
                tail[..end].trim_end_matches(['.', ','])
            }
            None => base(package),
        };
        if !name.is_empty() {
            out.insert(format!("{package}:{name}"));
        }
    }
}

/// The segments between the parentheses of each `name(` call.
fn call_bodies(segments: &[Segment], name: &str) -> Vec<Vec<Segment>> {
    let mut out = Vec::new();
    let mut body: Option<(i32, Vec<Segment>)> = None;
    let mut depth = 0_i32;
    for segment in segments {
        if segment.piece != Piece::Code {
            if let Some((_, parts)) = body.as_mut() {
                parts.push(segment.clone());
            }
            continue;
        }
        let text = segment.text.as_str();
        let mut from = 0;
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'(' {
                let head = &text[..index];
                let opens = head.ends_with(name)
                    && (head.len() == name.len() || !ident_byte(head.as_bytes()[head.len() - name.len() - 1]));
                if body.is_none() && opens {
                    body = Some((depth, Vec::new()));
                    from = index + 1;
                }
                depth += 1;
            } else if byte == b')' {
                depth -= 1;
                if body.as_ref().is_some_and(|(open, _)| *open == depth) {
                    let (_, mut parts) = body.take().expect("an open call");
                    parts.push(Segment {
                        piece: Piece::Code,
                        text: text[from..index].to_owned(),
                    });
                    out.push(parts);
                }
            }
        }
        if let Some((_, parts)) = body.as_mut() {
            parts.push(Segment {
                piece: Piece::Code,
                text: text[from..].to_owned(),
            });
        }
    }
    out
}

/// The string that follows the code ending in `lead` in `body`.
fn string_after<'a>(body: &'a [Segment], lead: &str) -> Option<&'a str> {
    body.windows(2).find_map(|pair| {
        let tail = pair[0].text.trim_end();
        let leads = pair[0].piece == Piece::Code && (tail.ends_with(lead) || tail.ends_with(&lead.replace(' ', "")));
        (leads && pair[1].piece != Piece::Comment).then_some(pair[1].text.as_str())
    })
}

/// What the tree says about which paths something names.
#[derive(Default)]
struct Index {
    known: BTreeSet<String>,
    exact: BTreeMap<String, BTreeMap<String, usize>>,
    build_exact: BTreeMap<String, BTreeMap<String, usize>>,
    globs: BTreeMap<String, Vec<String>>,
    labels: BTreeSet<String>,
    rust_paths: BTreeSet<String>,
    mods: BTreeSet<String>,
    loads: BTreeSet<String>,
}

impl Index {
    fn new(files: &BTreeMap<String, String>, paths: &[String]) -> Self {
        let mut index = Self::default();
        for path in paths {
            index.known.insert(path.clone());
            for dir in ancestors(parent(path)) {
                if !dir.is_empty() && !index.known.insert(dir.to_owned()) {
                    break;
                }
            }
        }
        for (path, text) in files {
            if is_starlark(path) {
                index.starlark(path, text);
            } else if path.ends_with(".rs") {
                index.rust(path, text);
            }
        }
        index
    }

    /// The nearest directory at or above `dir` that holds one of `names`.
    fn nearest(&self, dir: &str, names: &[&str]) -> String {
        ancestors(dir)
            .into_iter()
            .find(|candidate| names.iter().any(|name| self.known.contains(&join(candidate, name))))
            .unwrap_or("")
            .to_owned()
    }

    fn starlark(&mut self, path: &str, text: &str) {
        let dir = parent(path);
        let segments = shared::starlark_segments(text);
        let workspace = self.nearest(dir, &["MODULE.bazel"]);
        let package = self.nearest(dir, &["BUILD.bazel", "BUILD"]);
        for (position, segment) in segments.iter().enumerate() {
            if segment.piece == Piece::Code || segment.piece == Piece::Comment {
                continue;
            }
            let literal = segment.text.as_str();
            labels_in(literal, &mut self.labels);
            if literal.contains('*') {
                self.globs.entry(dir.to_owned()).or_default().push(literal.to_owned());
            }
            *self.exact.entry(dir.to_owned()).or_default().entry(literal.to_owned()).or_default() += 1;
            if is_build(path) {
                *self.build_exact.entry(dir.to_owned()).or_default().entry(literal.to_owned()).or_default() += 1;
            }
            let before = position.checked_sub(1).map_or("", |at| segments[at].text.trim_end());
            if !LOADERS.iter().any(|loader| before.ends_with(loader)) {
                continue;
            }
            if let Some((_, label)) = literal.split_once("//") {
                let file = label.replacen(':', "/", 1);
                self.loads.insert(join(&workspace, file.trim_start_matches('/')));
                if literal.starts_with('@') {
                    self.loads.insert(file.trim_start_matches('/').to_owned());
                }
            } else {
                self.loads.insert(join(&package, literal.trim_start_matches(':')));
            }
        }
    }

    fn rust(&mut self, path: &str, text: &str) {
        let dir = parent(path);
        let stem = base(path).trim_end_matches(".rs");
        let segments = shared::rust_segments(text);
        for (position, segment) in segments.iter().enumerate() {
            match segment.piece {
                Piece::Comment => {}
                Piece::Code => {
                    let words: Vec<&str> = segment
                        .text
                        .split(|letter: char| !(letter.is_ascii_alphanumeric() || letter == '_' || letter == ';'))
                        .filter(|word| !word.is_empty())
                        .collect();
                    for pair in words.windows(2) {
                        let Some(name) = pair[1].strip_suffix(';') else { continue };
                        if pair[0] != "mod" {
                            continue;
                        }
                        let name = name.trim_start_matches("r#");
                        for home in [dir.to_owned(), join(dir, stem)] {
                            self.mods.insert(join(&home, &format!("{name}.rs")));
                            self.mods.insert(join(&home, &format!("{name}/mod.rs")));
                        }
                    }
                }
                Piece::Text | Piece::Doc => {
                    let literal = segment.text.trim_start_matches("./").trim_start_matches("_main/");
                    labels_in(literal, &mut self.labels);
                    let before = position.checked_sub(1).map_or("", |at| segments[at].text.trim_end());
                    if before.ends_with("#[path =") || before.ends_with("#[path=") {
                        self.mods.extend(normal(dir, literal));
                    }
                    let pathlike = literal.len() < 200
                        && (literal.contains('/') || literal.contains('.'))
                        && !literal.contains(|letter: char| letter.is_whitespace() || "{}\\\"".contains(letter));
                    if !pathlike {
                        continue;
                    }
                    for home in ancestors(dir) {
                        if let Some(named) = normal(home, literal) {
                            if self.known.contains(&named) {
                                self.rust_paths.insert(named);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Whether a label, or a string or glob in a BUILD or .bzl file above `path`, names it or a directory above it.
    fn starlark_names(&self, path: &str) -> bool {
        let home = parent(path);
        let mut named: Vec<&str> = vec![path];
        named.extend(ancestors(home).into_iter().filter(|dir| !dir.is_empty()));
        for package in ancestors(home) {
            let exact = self.exact.get(package);
            let globs = self.globs.get(package);
            for target in &named {
                if target.len() <= package.len() {
                    continue;
                }
                let rel = below(target, package);
                if self.labels.contains(&format!("{package}:{rel}")) {
                    return true;
                }
                if exact.is_some_and(|map| {
                    map.contains_key(rel) || map.contains_key(&format!(":{rel}")) || map.contains_key(&format!("{rel}/"))
                }) {
                    return true;
                }
            }
            let rel = below(path, package);
            if globs.is_some_and(|list| list.iter().any(|glob| glob_match(glob.trim_start_matches(':'), rel))) {
                return true;
            }
        }
        false
    }

    /// Whether a Rust string names `path` or a directory above it below the top level.
    fn rust_names(&self, path: &str) -> bool {
        self.rust_paths.contains(path)
            || ancestors(parent(path))
                .into_iter()
                .any(|dir| dir.contains('/') && self.rust_paths.contains(dir))
    }

    fn referenced(&self, path: &str) -> bool {
        self.starlark_names(path) || self.rust_names(path) || self.mods.contains(path)
    }

    /// How many filegroups and exported files of the BUILD file at `path` have no user.
    fn orphans(&self, path: &str, text: &str) -> usize {
        let package = parent(path);
        let segments = shared::starlark_segments(text);
        let mut names: Vec<String> = Vec::new();
        for body in call_bodies(&segments, "filegroup") {
            let tagged = body.iter().any(|part| part.piece == Piece::Text && part.text == "corpus");
            if let (Some(name), false) = (string_after(&body, "name ="), tagged) {
                names.push(name.to_owned());
            }
        }
        for body in call_bodies(&segments, "exports_files") {
            let mut listing = true;
            for part in &body {
                if part.piece == Piece::Code && part.text.contains("visibility") {
                    listing = false;
                }
                if part.piece == Piece::Text && listing {
                    names.push(part.text.clone());
                }
            }
        }
        let used = |name: &String| {
            let own = self.build_exact.get(package).and_then(|map| map.get(name)).copied().unwrap_or(0);
            own >= 2
                || self.labels.contains(&format!("{package}:{name}"))
                || self.exact.get(package).is_some_and(|map| map.contains_key(&format!(":{name}")))
        };
        names.iter().filter(|name| !used(name)).count()
    }
}

/// Counts every hit of every kind; `files` holds the Starlark and Rust text, `paths` every tracked path.
fn scan(files: &BTreeMap<String, String>, paths: &[String]) -> Counts {
    let index = Index::new(files, paths);
    let mut counts = Counts::new();
    let mut directories: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for path in paths {
        directories.entry(parent(path)).or_default().push(path);
        let fixture = in_fixture_tree(path);
        if path.ends_with(".bzl") {
            let loaded = index.loads.contains(path) || index.labels.contains(&format!("{}:{}", parent(path), base(path)));
            add(&mut counts, path, "unloaded-bzl", usize::from(!loaded && !public_entry(path)));
        }
        if fixture && !is_build(path) {
            add(&mut counts, path, "unreferenced-fixture", usize::from(!index.referenced(path)));
        }
        if path.ends_with(".rs") && !fixture {
            let built = index.starlark_names(path) || index.mods.contains(path);
            add(&mut counts, path, "unbuilt-rust", usize::from(!built));
        }
        if is_build(path) && index.nearest(parent(path), &["MODULE.bazel"]).is_empty() {
            if let Some(text) = files.get(path) {
                add(&mut counts, path, "orphan-target", index.orphans(path, text));
            }
        }
    }
    for (dir, members) in directories {
        let packaged = dir.is_empty() || members.iter().any(|path| is_build(path));
        if packaged || in_fixture_tree(&join(dir, "x")) {
            continue;
        }
        let reached = members.iter().any(|path| index.referenced(path));
        add(&mut counts, dir, "empty-package", usize::from(!reached));
    }
    counts
}

#[test]
fn no_unreferenced_file_outside_the_allowlist() {
    let files = shared::tree(&dx_testing::source_snapshot());
    assert!(files.len() >= MIN_FILES, "only {} files in the snapshot", files.len());
    let paths = dx_testing::source_paths();
    shared::settle(&scan(&files, &paths), "DX_ALLOWLIST", ALLOWLIST, &KINDS);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hits(entries: &[(&str, &str)]) -> Vec<(String, String, usize)> {
        let files: BTreeMap<String, String> = entries
            .iter()
            .filter(|(path, _)| is_starlark(path) || path.ends_with(".rs"))
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect();
        let mut paths: Vec<String> = entries.iter().map(|(path, _)| (*path).to_owned()).collect();
        paths.sort();
        scan(&files, &paths)
            .into_iter()
            .map(|((path, kind), count)| (path, kind, count))
            .collect()
    }

    fn hit(path: &str, kind: &str, count: usize) -> (String, String, usize) {
        (path.to_owned(), kind.to_owned(), count)
    }

    #[test]
    fn a_bzl_file_nobody_loads_is_detected() {
        let found = hits(&[
            ("MODULE.bazel", "x = use_extension(\"//a:extensions.bzl\", \"x\")\ny = use_repo_rule(\"//a/private:repo.bzl\", \"y\")\n"),
            ("a/BUILD.bazel", "load(\":used.bzl\", \"u\")\nload(\"//a/private:deep.bzl\", \"d\")\nload(\"@rules_dx//a/private:named.bzl\", \"n\")\n"),
            ("a/defs.bzl", ""),
            ("a/extensions.bzl", ""),
            ("a/used.bzl", ""),
            ("a/unused.bzl", "# load(\":unused.bzl\")\n"),
            ("a/private/BUILD.bazel", ""),
            ("a/private/deep.bzl", "load(\":sibling.bzl\", \"s\")\n"),
            ("a/private/named.bzl", ""),
            ("a/private/repo.bzl", ""),
            ("a/private/sibling.bzl", ""),
            ("a/private/defs.bzl", ""),
        ]);
        assert_eq!(
            found,
            [hit("a/private/defs.bzl", "unloaded-bzl", 1), hit("a/unused.bzl", "unloaded-bzl", 1)]
        );
    }

    #[test]
    fn a_fixture_file_no_target_or_test_names_is_detected() {
        let build = "t(srcs = [\"fixtures/listed.txt\"], data = glob([\"fixtures/globbed/**\"]) + [\"fixtures/consumer\"])\nexports_files([\"fixtures/exported.txt\"])\nu(data = [\"//go/tests/fixtures/pkg:labelled.txt\", \":fixtures/exported.txt\"])\n";
        let test = "const A: &str = include_str!(\"fixtures/included.txt\");\nfn f() { rlocation(\"_main/go/tests/fixtures/looked/up.txt\"); dir(\"go/tests/fixtures/tree\"); }\n";
        let found = hits(&[
            ("go/tests/BUILD.bazel", build),
            ("go/tests/a_test.rs", test),
            ("go/tests/fixtures/listed.txt", ""),
            ("go/tests/fixtures/globbed/deep/a.txt", ""),
            ("go/tests/fixtures/consumer/MODULE.bazel", ""),
            ("go/tests/fixtures/consumer/src/main.go", ""),
            ("go/tests/fixtures/exported.txt", ""),
            ("go/tests/fixtures/pkg/BUILD.bazel", ""),
            ("go/tests/fixtures/pkg/labelled.txt", ""),
            ("go/tests/fixtures/pkg/stray.txt", ""),
            ("go/tests/fixtures/included.txt", ""),
            ("go/tests/fixtures/looked/up.txt", ""),
            ("go/tests/fixtures/tree/deep/a.txt", ""),
            ("go/tests/fixtures/stray.txt", ""),
            ("tools/ci/x_fixtures/stray.yml", ""),
            ("tools/ci/BUILD.bazel", "t(srcs = [\"x_test.rs\"])\n"),
        ]);
        assert_eq!(
            found,
            [
                hit("go/tests/a_test.rs", "unbuilt-rust", 1),
                hit("go/tests/fixtures/pkg/stray.txt", "unreferenced-fixture", 1),
                hit("go/tests/fixtures/stray.txt", "unreferenced-fixture", 1),
                hit("tools/ci/x_fixtures/stray.yml", "unreferenced-fixture", 1),
            ]
        );
    }

    #[test]
    fn a_filegroup_or_exported_file_without_a_user_is_detected() {
        let build = "filegroup(\n    name = \"used\",\n    srcs = [\"a.txt\"],\n)\nfilegroup(name = \"local\", srcs = [\"a.txt\"])\nfilegroup(name = \"orphan\", srcs = [\"a.txt\"])\nfilegroup(name = \"corpus_x\", srcs = [\"a.txt\"], tags = [\"corpus\"])\nexports_files([\"a.txt\", \"b.txt\", \"c.txt\"], visibility = [\"//visibility:public\"])\nt(data = [\":local\", \"b.txt\"])\n";
        let found = hits(&[
            ("a/BUILD.bazel", build),
            ("a/a.txt", ""),
            ("a/b.txt", ""),
            ("a/c.txt", ""),
            ("b/BUILD.bazel", "t(data = [\"//a:used\"], env = {\"X\": \"$(rootpath //a:a.txt)\"})\n"),
            ("b/nested/MODULE.bazel", ""),
            ("b/nested/BUILD.bazel", "filegroup(name = \"inner\")\n"),
        ]);
        assert_eq!(found, [hit("a/BUILD.bazel", "orphan-target", 2)]);
    }

    #[test]
    fn a_rust_file_no_srcs_or_mod_reaches_is_detected() {
        let found = hits(&[
            ("a/BUILD.bazel", "t(srcs = [\"src/lib.rs\"] + glob([\"tests/*.rs\"]))\n"),
            ("a/src/lib.rs", "mod one;\npub mod two;\n#[path = \"odd/three.rs\"]\nmod three;\n// mod four;\n"),
            ("a/src/one.rs", "mod inner;\n"),
            ("a/src/one/inner.rs", ""),
            ("a/src/two/mod.rs", ""),
            ("a/src/odd/three.rs", ""),
            ("a/src/four.rs", ""),
            ("a/tests/t.rs", ""),
        ]);
        assert_eq!(found, [hit("a/src/four.rs", "unbuilt-rust", 1)]);
    }

    #[test]
    fn a_directory_no_package_reaches_is_detected() {
        let found = hits(&[
            ("a/BUILD.bazel", "t(srcs = glob([\"globbed/**\"]) + [\"listed/a.txt\"])\n"),
            ("a/globbed/deep/a.txt", ""),
            ("a/listed/a.txt", ""),
            ("a/stray/a.txt", ""),
            ("a/stray/deep/b.txt", ""),
            ("b/a.txt", ""),
            ("top.txt", ""),
        ]);
        assert_eq!(
            found,
            [
                hit("a/stray", "empty-package", 1),
                hit("a/stray/deep", "empty-package", 1),
                hit("b", "empty-package", 1),
            ]
        );
    }

    #[test]
    fn labels_calls_and_paths_are_read() {
        let mut labels = BTreeSet::new();
        labels_in("$(rootpath //a/b:c.txt) @rules_dx//d //e/f. https://x.test/y", &mut labels);
        assert_eq!(labels.into_iter().collect::<Vec<_>>(), ["a/b:c.txt", "d:d", "e/f:f"]);
        assert_eq!(normal("a/b", "../c/./d.rs").as_deref(), Some("a/c/d.rs"));
        assert_eq!(normal("a", "../../c"), None);
        assert_eq!(ancestors("a/b"), ["a/b", "a", ""]);
        assert!(in_fixture_tree("go/tests/fixtures/a") && in_fixture_tree("tools/docs/x/fixture/a"));
        assert!(!in_fixture_tree("go/tests/a") && !in_fixture_tree("tools/ci/a.rs"));
    }
}
