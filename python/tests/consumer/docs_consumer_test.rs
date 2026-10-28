//! Builds a Python API site through `dx docs` in a consumer that renames rules_dx.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, sha256_file, write, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "dxr")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "platforms", version = "1.1.0")
bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_jvm_external", version = "7.1")
bazel_dep(name = "rules_kotlin", version = "2.4.21")
bazel_dep(name = "rules_python", version = "RULES_PYTHON")
bazel_dep(name = "aspect_rules_py", version = "ASPECT_RULES_PY")

python_interpreters = use_extension("@aspect_rules_py//py:extensions.bzl", "python_interpreters")
python_interpreters.toolchain(python_version = "3.12")
use_repo(python_interpreters, "python_interpreters")

register_toolchains("@python_interpreters//:all")

python = use_extension("@rules_python//python/extensions:python.bzl", "python")
python.toolchain(python_version = "3.12")
use_repo(python, "python_3_12")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    rustfmt_version = "1.98.1",
    versions = ["1.98.1"],
)

MAVEN_BLOCK
"#;

const BUILD: &str = r#"load("@dxr//python/rules:defs.bzl", "python_library")
load("@dxr//tools/docs/python:defs.bzl", "python_docs")

python_library(
    name = "money",
    srcs = ["src/money/__init__.py"],
    imports = ["src"],
)

python_docs(
    name = "site",
    book_toml = "book.toml",
    library = ":money",
    package = "money",
    prose = ["prose.md"],
    source_url = "https://example.com/consumer/blob/main/",
)

python_docs(
    name = "numpy_site",
    book_toml = "book.toml",
    docstyle = "numpy",
    library = ":money",
    package = "money",
    prose = ["prose.md"],
    source_url = "https://example.com/consumer/blob/main/",
)
"#;

const BOOK_TOML: &str =
    "[book]\ntitle = \"consumer\"\nauthors = [\"consumer\"]\nlanguage = \"en\"\nsrc = \"src\"\n";

const PROSE: &str = "# Consumer\n\nThe Python API reference of the money package.\n";

const MONEY: &str = "\"\"\"Keeps accounts.\"\"\"\n\n\nclass Account:\n    \"\"\"Holds the balance of one owner.\"\"\"\n\n    def __init__(self, owner: str) -> None:\n        self.owner = owner\n\n\ndef new_account(owner: str) -> Account:\n    \"\"\"Opens an empty account.\n\n    Parameters\n    ----------\n    owner : str\n        The name of the account holder.\n    \"\"\"\n    return Account(owner)\n";

const BROKEN: &str = "def broken(:\n    pass\n";

fn pin(text: &str, name: &str) -> String {
    let marker = format!("{name} = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} missing from versions.bzl"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

fn consumer(name: &str) -> Case {
    let rules = rules_dx_source();
    let versions = read(&rules.join("modules/versions.bzl"));
    let root = nested::case(name);
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("MAVEN_BLOCK", nested::CONSUMER_MAVEN)
            .replace("RULES_DX", &rules.display().to_string())
            .replace("RULES_PYTHON", &pin(&versions, "FLOOR_RULES_PYTHON"))
            .replace("ASPECT_RULES_PY", &pin(&versions, "FLOOR_ASPECT_RULES_PY")),
    );
    nested::seed_rules_dx_lock(&root, &rules);
    write(&root.join(".bazelrc"), &dx_testing::preset_rc(&rules));
    nested::stage_consumer_maven_lock(&root);
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("book.toml"), BOOK_TOML);
    write(&root.join("prose.md"), PROSE);
    write(&root.join("src/money/__init__.py"), MONEY);
    root
}

fn dx(root: &Case, extra: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("python-docs-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut args = vec!["docs", "--workspace", workspace.as_str()];
    args.extend_from_slice(extra);
    root.run(
        &resolve_runfiles(&rel),
        &args,
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

fn site(root: &Case, name: &str) -> PathBuf {
    root.join(format!("bazel-bin/{name}"))
}

/// Every rendered Python page, joined.
fn py_pages(site: &Path) -> String {
    let dir = site.join("api/python");
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
fn dx_docs_renders_the_public_api_without_touching_sources() {
    let root = consumer("python_docs_render");
    let before = hashes(&root.join("src"));
    let built = dx(&root, &["//:site"]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    assert_eq!(
        hashes(&root.join("src")),
        before,
        "dx docs changed a source"
    );
    let checked = dx(&root, &["--check", "//:site"]);
    assert_eq!(checked.status.code(), Some(0), "{}", checked.combined());

    let pages = py_pages(&site(&root, "site"));
    assert!(pages.contains("new_account"), "{pages}");
    assert!(pages.contains("Opens an empty account"), "{pages}");
    assert!(
        pages.contains("href=\"https://example.com/consumer/blob/main/src/money/__init__.py#L"),
        "{pages}"
    );
    let index = read(&site(&root, "site").join("searchindex.js"));
    assert!(
        index.contains("api/python/"),
        "the search index never names a Python page"
    );
}

#[test]
fn numpy_docstyle_renders_parameter_docs() {
    let root = consumer("python_docs_numpy");
    let built = dx(&root, &["//:numpy_site"]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    let pages = py_pages(&site(&root, "numpy_site"));
    assert!(pages.contains("Opens an empty account"), "{pages}");
    assert!(pages.contains("The name of the account holder."), "{pages}");
}

#[test]
fn a_package_that_does_not_parse_fails_without_a_stub_page() {
    let root = consumer("python_docs_broken");
    write(&root.join("src/money/__init__.py"), BROKEN);
    let built = dx(&root, &["//:site"]);
    assert_ne!(built.status.code(), Some(0), "{}", built.combined());
    assert!(
        !site(&root, "site").join("api/python").exists(),
        "a failed build left Python pages behind"
    );
}
