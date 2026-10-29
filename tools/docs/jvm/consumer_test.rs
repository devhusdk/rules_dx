//! Builds a JVM API site through `dx docs` in a consumer that renames rules_dx.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_test_scratch::scratch;
use dx_testing::{read, resolve_runfiles, rules_dx_source, sha256_file, write, Run};

const MODULE: &str = r#"module(name = "renamed_jvm_docs_consumer")

bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "dxr")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

# The dotnet toolchain rules_dx registers needs the patched rules_dotnet; overrides of a
# dependency apply only in the root module.
single_version_override(
    module_name = "rules_dotnet",
    patch_strip = 1,
    patches = [
        "//patches/rules_dotnet:reproducible.patch",
        "//patches/rules_dotnet:fsharp_signature.patch",
        "//patches/rules_dotnet:sdk_versions.patch",
    ],
    version = "0.22.2",
)

bazel_dep(name = "rules_kotlin", version = "RULES_KOTLIN")
bazel_dep(name = "rules_rust", version = "0.75.0")
bazel_dep(name = "rules_scala", version = "RULES_SCALA")

# rules_dx imports these repos from the rust extension, so the consumer pins the same version.
rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    rustfmt_version = "1.98.1",
    versions = ["1.98.1"],
)
use_repo(rust, "rust_toolchains")

register_toolchains("@rust_toolchains//:all")

scala_config = use_extension("@rules_scala//scala/extensions:config.bzl", "scala_config")
scala_config.settings(scala_version = "2.13.18")
use_repo(scala_config, "rules_scala_config")

scala_deps = use_extension("@rules_scala//scala/extensions:deps.bzl", "scala_deps")
scala_deps.scala()

register_toolchains("@dxr//scala/toolchain:semanticdb")
"#;

const ROOT_BUILD: &str = r#"load("@dxr//tools/docs/site:site.bzl", "docs_site")

docs_site(
    name = "site",
    book_toml = "book.toml",
    prose = ["prose.md"],
    shards = [
        "//java:api",
        "//kotlin:api",
        "//scala:api",
    ],
    source_url = "https://example.com/consumer/blob/main/",
)
"#;

const JAVA_BUILD: &str = r#"load("@dxr//java/rules:defs.bzl", "java_library")
load("@dxr//tools/docs/jvm:defs.bzl", "java_doc_shard")

java_library(
    name = "lib",
    srcs = ["dx/consumer/shapes/Shape.java"],
)

java_doc_shard(
    name = "api",
    lib = ":lib",
    package = "shapes",
    visibility = ["//visibility:public"],
)
"#;

const KOTLIN_BUILD: &str = r#"load("@dxr//kotlin/rules:defs.bzl", "kotlin_library")
load("@dxr//tools/docs/jvm:defs.bzl", "kotlin_doc_shard")

kotlin_library(
    name = "lib",
    srcs = ["dx/consumer/shapes/Shape.kt"],
)

kotlin_doc_shard(
    name = "api",
    lib = ":lib",
    package = "shapes",
    visibility = ["//visibility:public"],
)
"#;

const SCALA_BUILD: &str = r#"load("@dxr//scala/rules:defs.bzl", "scala_library")
load("@dxr//tools/docs/jvm:defs.bzl", "scala_doc_shard")

scala_library(
    name = "lib",
    srcs = ["dx/consumer/shapes/Shape.scala"],
)

scala_doc_shard(
    name = "api",
    lib = ":lib",
    package = "shapes",
    visibility = ["//visibility:public"],
)
"#;

const BOOK_TOML: &str =
    "[book]\ntitle = \"consumer\"\nauthors = [\"consumer\"]\nlanguage = \"en\"\nsrc = \"src\"\n";

const PROSE: &str = "# Consumer\n\nThe JVM API reference of the shapes packages.\n";

const JAVA_SHAPE: &str = r#"package dx.consumer.shapes;

/** A closed figure that can report its area. */
public interface Shape {
  /**
   * Returns the area of this shape.
   *
   * @return the area in square units
   */
  double area();
}
"#;

const KOTLIN_SHAPE: &str = r#"package dx.consumer.shapes

/** A closed figure that can report its area. */
interface Shape {
    /** Returns the area of this shape in square units. */
    fun area(): Double
}
"#;

const SCALA_SHAPE: &str = r#"package dx.consumer.shapes

/** A closed figure that can report its area. */
trait Shape {

  /** Returns the area of this shape in square units. */
  def area: Double
}
"#;

const SOURCES: [(&str, &str); 3] = [
    ("java/dx/consumer/shapes/Shape.java", JAVA_SHAPE),
    ("kotlin/dx/consumer/shapes/Shape.kt", KOTLIN_SHAPE),
    ("scala/dx/consumer/shapes/Shape.scala", SCALA_SHAPE),
];

const DOTNET_PATCHES: [&str; 3] = [
    "reproducible.patch",
    "fsharp_signature.patch",
    "sdk_versions.patch",
];

const OFFLINE: [&str; 3] = [
    "--nofetch",
    "--repository_disable_download",
    "--sandbox_default_allow_network=false",
];

/// The version a `single_version_override` of the rules_dx root forces; overrides do not
/// reach a consumer, which must ask for the same version.
fn override_pin(text: &str, name: &str) -> String {
    let marker = format!("module_name = \"{name}\",\n    version = \"");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} override missing"))
        + marker.len();
    let end = text[start..].find('"').expect("closing quote") + start;
    text[start..end].to_owned()
}

/// A consumer sets up fresh: the build keeps the old `bazel-bin/site`, so each case is its own.
fn consumer(name: &str) -> Case {
    let rules = rules_dx_source();
    let module = read(&rules.join("MODULE.bazel"));
    let root = nested::case(name);
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE
            .replace("RULES_DX", &rules.display().to_string())
            .replace("RULES_KOTLIN", &override_pin(&module, "rules_kotlin"))
            .replace("RULES_SCALA", &override_pin(&module, "rules_scala")),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\nbuild --java_runtime_version=remotejdk_21\nbuild --tool_java_runtime_version=remotejdk_21\n");
    write(&root.join("patches/rules_dotnet/BUILD.bazel"), "");
    for patch in DOTNET_PATCHES {
        let from = rules.join("patches/rules_dotnet").join(patch);
        write(&root.join("patches/rules_dotnet").join(patch), &read(&from));
    }
    write(&root.join("BUILD.bazel"), ROOT_BUILD);
    write(&root.join("book.toml"), BOOK_TOML);
    write(&root.join("prose.md"), PROSE);
    write(&root.join("java/BUILD.bazel"), JAVA_BUILD);
    write(&root.join("kotlin/BUILD.bazel"), KOTLIN_BUILD);
    write(&root.join("scala/BUILD.bazel"), SCALA_BUILD);
    for (path, text) in SOURCES {
        write(&root.join(path), text);
    }
    root
}

fn docs(root: &Case, extra: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home_dir = scratch("jvm-docs-home");
    let home = home_dir.path().to_string_lossy().into_owned();
    let mut args = vec!["docs", "--workspace", workspace.as_str(), "//:site"];
    if !extra.is_empty() {
        args.push("--");
        args.extend_from_slice(extra);
    }
    root.run(
        &resolve_runfiles(&rel),
        &args,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn hashes(root: &Path) -> BTreeMap<PathBuf, String> {
    SOURCES
        .iter()
        .map(|(path, _)| {
            let file = root.join(path);
            let hash = sha256_file(&file).unwrap_or_else(|error| panic!("{error}"));
            (file, hash)
        })
        .collect()
}

fn site(root: &Case) -> PathBuf {
    root.join("bazel-bin/site")
}

/// Every rendered page under one language directory of the site, joined.
fn pages(root: &Case, language: &str) -> String {
    let dir = site(root).join("api").join(language);
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
fn dx_docs_renders_java_kotlin_and_scala_in_a_renamed_consumer() {
    let root = consumer("jvm_docs_render");
    let before = hashes(&root);
    let built = docs(&root, &[]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    assert_eq!(hashes(&root), before, "dx docs changed a source");

    for language in ["java", "kotlin", "scala"] {
        let text = pages(&root, language);
        assert!(
            text.contains("A closed figure that can report its area."),
            "{language} page misses the type doc\n{text}"
        );
        assert!(
            text.contains("Returns the area of this shape"),
            "{language} page misses the method doc\n{text}"
        );
    }
    let index = read(&site(&root).join("searchindex.js"));
    for language in ["java", "kotlin", "scala"] {
        assert!(
            index.contains(&format!("api/{language}/")),
            "the search index never names a {language} page"
        );
    }

    let rerun = docs(&root, &OFFLINE);
    assert_eq!(rerun.status.code(), Some(0), "{}", rerun.combined());
    assert_eq!(hashes(&root), before, "the rerun changed a source");
}

#[test]
fn a_java_type_error_fails_without_a_page() {
    let root = consumer("jvm_docs_java_type_error");
    let broken = JAVA_SHAPE.replace("double area();", "Missing area();");
    write(&root.join("java/dx/consumer/shapes/Shape.java"), &broken);
    let before = hashes(&root);
    let built = docs(&root, &[]);
    assert_ne!(built.status.code(), Some(0), "{}", built.combined());
    assert!(
        built.combined().contains("Shape.java"),
        "{}",
        built.combined()
    );
    assert!(
        !site(&root).join("api/java").exists(),
        "a failed build left Java pages behind"
    );
    assert_eq!(hashes(&root), before, "dx docs changed a source");
}
