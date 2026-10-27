//! Builds a Go API site through `dx docs` in a consumer that renames rules_dx.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read, resolve_runfiles, rules_dx_source, sha256_file, write, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "dxr")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_go", version = "RULES_GO")
bazel_dep(name = "rules_rust", version = "0.74.0")

go_sdk = use_extension("@rules_go//go:extensions.bzl", "go_sdk")
go_sdk.download(version = "GO_VERSION")
"#;

const BUILD: &str = r#"load("@dxr//tools/docs/go:defs.bzl", "go_doc_shard")
load("@dxr//tools/docs/site:site.bzl", "docs_site")
load("@rules_go//go:def.bzl", "go_library")

go_library(
    name = "money",
    srcs = ["money/money.go"],
    importpath = "example.com/consumer/money",
)

go_doc_shard(
    name = "api",
    library = ":money",
)

docs_site(
    name = "site",
    book_toml = "book.toml",
    prose = ["prose.md"],
    shards = [":api"],
    source_url = "https://example.com/consumer/blob/main/",
)
"#;

const BOOK_TOML: &str =
    "[book]\ntitle = \"consumer\"\nauthors = [\"consumer\"]\nlanguage = \"en\"\nsrc = \"src\"\n";

const PROSE: &str = "# Consumer\n\nThe Go API reference of the money package.\n";

const MONEY: &str = "// Package money keeps accounts.\npackage money\n\n// Account holds the balance of one owner.\ntype Account struct {\n\t// Owner names the holder.\n\tOwner string\n}\n\n// NewAccount opens an empty account.\nfunc NewAccount(owner string) *Account {\n\treturn &Account{Owner: owner}\n}\n";

const BROKEN: &str = "package money\n\nfunc Broken( {\n";

fn pin(text: &str, name: &str) -> String {
    let marker = format!("{name}\", version = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} missing"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

fn consumer(name: &str) -> Case {
    let rules = rules_dx_source();
    let module = read(&rules.join("MODULE.bazel"));
    let go = {
        let at = module.find("go_sdk.download(").expect("go_sdk.download");
        let rest = &module[at..];
        let from = rest.find("\"go1.").expect("go sdk") + 3;
        let rest = &rest[from..];
        let end = rest
            .find(".darwin")
            .or_else(|| rest.find(".linux"))
            .expect("sdk name");
        rest[..end].to_owned()
    };
    let root = nested::case(name);
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("RULES_DX", &rules.display().to_string())
            .replace("RULES_GO", &pin(&module, "bazel_dep(name = \"rules_go"))
            .replace("GO_VERSION", &go),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("book.toml"), BOOK_TOML);
    write(&root.join("prose.md"), PROSE);
    write(&root.join("money/money.go"), MONEY);
    root
}

fn docs(root: &Case) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("go-docs-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    root.run(
        &resolve_runfiles(&rel),
        &["docs", "--workspace", workspace.as_str(), "//:site"],
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn hashes(dir: &Path) -> BTreeMap<PathBuf, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap_or_else(|error| panic!("{next:?}: {error}")) {
            let path = entry
                .unwrap_or_else(|error| panic!("entry: {error}"))
                .path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let hash = sha256_file(&path).unwrap_or_else(|error| panic!("{error}"));
                out.insert(path, hash);
            }
        }
    }
    out
}

fn site(root: &Case) -> PathBuf {
    root.join("bazel-bin/site")
}

/// Every rendered Go page, joined.
fn go_pages(root: &Case) -> String {
    let dir = site(root).join("api/go");
    let mut text = String::new();
    let mut stack = vec![dir.clone()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap_or_else(|error| panic!("{next:?}: {error}")) {
            let path = entry
                .unwrap_or_else(|error| panic!("entry: {error}"))
                .path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "html") {
                text.push_str(&read(&path));
            }
        }
    }
    text
}

#[test]
fn dx_docs_renders_the_exported_api_without_touching_sources() {
    let root = consumer("go_docs_render");
    let before = hashes(&root.join("money"));
    let built = docs(&root);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    assert_eq!(
        hashes(&root.join("money")),
        before,
        "dx docs changed a source"
    );

    let pages = go_pages(&root);
    assert!(pages.contains("NewAccount"), "{pages}");
    assert!(pages.contains("opens an empty account"), "{pages}");
    assert!(
        pages.contains("href=\"https://example.com/consumer/blob/main/money/money.go#L"),
        "{pages}"
    );
    let index = read(&site(&root).join("searchindex.js"));
    assert!(
        index.contains("api/go/"),
        "the search index never names a Go page"
    );
}

#[test]
fn a_doc_edit_shows_up_after_a_rebuild() {
    let root = consumer("go_docs_rebuild");
    let built = docs(&root);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    write(
        &root.join("money/money.go"),
        &MONEY.replace("opens an empty account", "opens a fresh account"),
    );
    let rebuilt = docs(&root);
    assert_eq!(rebuilt.status.code(), Some(0), "{}", rebuilt.combined());
    let pages = go_pages(&root);
    assert!(pages.contains("opens a fresh account"), "{pages}");
    assert!(!pages.contains("opens an empty account"), "{pages}");
}

#[test]
fn a_package_that_does_not_parse_fails_without_a_stub_page() {
    let root = consumer("go_docs_broken");
    write(&root.join("money/money.go"), BROKEN);
    let built = docs(&root);
    assert_ne!(built.status.code(), Some(0), "{}", built.combined());
    assert!(
        built.combined().contains("money/money.go"),
        "{}",
        built.combined()
    );
    assert!(
        !site(&root).join("api/go").exists(),
        "a failed build left Go pages behind"
    );
}
