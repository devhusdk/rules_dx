//! Builds TypeScript API docs in a renamed consumer repository through `dx docs`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_testing::nested::{self, Case};
use dx_testing::{mkscratch, resolve_runfiles, serde_json, Run};

const MODULE: &str = r#"module(name = "renamed_ledger_consumer")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "aspect_rules_js", version = "3.5.2")

npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(
    name = "ledger_npm",
    npmrc = "//:.npmrc",
    pnpm_lock = "//:pnpm-lock.yaml",
    verify_node_modules_ignored = "//:.bazelignore",
)
use_repo(npm, "ledger_npm")
"#;

const ROOT_BUILD: &str = r#"load("@ledger_npm//:defs.bzl", "npm_link_all_packages")

npm_link_all_packages(name = "node_modules")

exports_files(["package.json"])
"#;

const PACKAGE_JSON: &str = r#"{
  "name": "ledger-workspace",
  "private": true,
  "packageManager": "pnpm@10.34.5",
  "devDependencies": {
    "csstype": "3.2.3"
  }
}
"#;

const LOCK: &str = r#"lockfileVersion: '9.0'

settings:
  autoInstallPeers: true
  excludeLinksFromLockfile: false

importers:

  .:
    devDependencies:
      csstype:
        specifier: 3.2.3
        version: 3.2.3

packages:

  csstype@3.2.3:
    resolution: {integrity: sha512-z1HGKcYy2xA8AGQfwrn0PAy+PB7X/GSj3UVJW9qKyn43xWa+gl5nXmU4qqLMRzWVLFC8KusUX8T/0kCiOYpAIQ==}

snapshots:

  csstype@3.2.3: {}
"#;

const WORKSPACE: &str = "packages:\n  - .\nallowBuilds: {}\n";

const NPMRC: &str = "hoist=false\n";

const LIB_BUILD: &str = r#"load("@rules_dx//tools/docs/typedoc:defs.bzl", "typedoc_site")

typedoc_site(
    name = "api_docs",
    srcs = glob(["src/*.ts"]),
    entry_points = ["src/index.ts"],
    package = "ledger",
    source_url = "https://example.test/ledger/blob/main/{file}#L{line}",
    tsconfig = "tsconfig.json",
    deps = ["//:node_modules/csstype"],
)

typedoc_site(
    name = "documented_docs",
    srcs = glob(["src/*.ts"]),
    entry_points = ["src/index.ts"],
    options = ["--excludeNotDocumented"],
    package = "ledger",
    tsconfig = "tsconfig.json",
    deps = ["//:node_modules/csstype"],
)

typedoc_site(
    name = "missing_entry_docs",
    srcs = glob(["src/*.ts"]),
    entry_points = ["src/absent.ts"],
    package = "ledger",
    tsconfig = "tsconfig.json",
    deps = ["//:node_modules/csstype"],
)

typedoc_site(
    name = "missing_dependency_docs",
    srcs = glob(["src/*.ts"]),
    entry_points = ["src/index.ts"],
    package = "ledger",
    tsconfig = "tsconfig.json",
)
"#;

const TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true,
    "types": []
  },
  "include": ["src/*.ts"]
}
"#;

const INDEX_TS: &str = r#"export { Ledger } from "./ledger";
export { addAmounts, Currency } from "./amount";
export type { Amount } from "./amount";
export { badge } from "./badge";
"#;

const AMOUNT_TS: &str = r#"/** A currency the ledger can hold. */
export enum Currency {
  /** United States dollar. */
  Usd = "USD",
  /** Euro. */
  Eur = "EUR",
}

/** A whole number of minor units in one currency. */
export type Amount = { minor: number; currency: Currency };

/**
 * Adds two amounts of the same currency.
 * @param left - The first amount.
 * @param right - The second amount.
 * @returns The sum.
 */
export function addAmounts(left: Amount, right: Amount): Amount {
  if (left.currency !== right.currency) {
    throw new Error("currency mismatch");
  }
  return { minor: left.minor + right.minor, currency: left.currency };
}
"#;

const LEDGER_TS: &str = r#"import type { Amount } from "./amount";

/** An append-only list of amounts. */
export class Ledger {
  private entries: Amount[] = [];

  /** Records one amount. */
  record(amount: Amount): void {
    this.entries.push(amount);
  }

  /**
   * Counts the recorded amounts.
   * @returns The number of entries.
   */
  count(): number {
    return this.entries.length;
  }

  private audit(): void {}
}
"#;

const BADGE_TS: &str = r#"import type { Property } from "csstype";

/** Describes a colored badge. */
export interface Badge {
  /** The badge background. */
  background: Property.BackgroundColor;
}

/** Builds a badge. */
export function badge(background: Property.BackgroundColor): Badge {
  return { background };
}
"#;

const SOURCES: [(&str, &str); 4] = [
    ("lib/src/index.ts", INDEX_TS),
    ("lib/src/amount.ts", AMOUNT_TS),
    ("lib/src/ledger.ts", LEDGER_TS),
    ("lib/src/badge.ts", BADGE_TS),
];

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("tools/docs/typedoc/tests/consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(5)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join(".bazelignore"), "node_modules\n");
    write(&root.join(".npmrc"), NPMRC);
    write(&root.join("package.json"), PACKAGE_JSON);
    write(&root.join("pnpm-lock.yaml"), LOCK);
    write(&root.join("pnpm-workspace.yaml"), WORKSPACE);
    write(&root.join("BUILD.bazel"), ROOT_BUILD);
    write(&root.join("lib/BUILD.bazel"), LIB_BUILD);
    write(&root.join("lib/tsconfig.json"), TSCONFIG);
    for (path, text) in SOURCES {
        write(&root.join(path), text);
    }
    root
}

fn docs(root: &Case, target: &str) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("typedoc-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    root.run(
        &resolve_runfiles(&rel),
        &["docs", "--workspace", workspace.as_str(), target],
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

fn site(root: &Case, name: &str) -> PathBuf {
    let base = root.bazel(&["info", "bazel-bin"]);
    assert!(base.status.success(), "{}", base.combined());
    PathBuf::from(base.stdout.trim()).join("lib").join(name)
}

fn hashes(root: &Path) -> BTreeMap<String, Vec<u8>> {
    SOURCES
        .iter()
        .map(|(path, _)| {
            (
                (*path).to_owned(),
                std::fs::read(root.join(path)).unwrap_or_default(),
            )
        })
        .collect()
}

fn search_docs(site: &Path) -> (Vec<serde_json::Value>, Vec<String>) {
    let raw = read(&site.join("searchindex.js"));
    let json = raw
        .split("JSON.parse('")
        .nth(1)
        .and_then(|rest| rest.rsplit_once("'));"))
        .map(|(json, _)| json.replace("\\'", "'").replace("\\\\", "\\"))
        .unwrap_or_else(|| panic!("no embedded index in {raw}"));
    let index: serde_json::Value =
        serde_json::from_str(&json).unwrap_or_else(|error| panic!("index JSON: {error}"));
    let docs = index["index"]["documentStore"]["docs"]
        .as_object()
        .unwrap_or_else(|| panic!("no docs"))
        .values()
        .cloned()
        .collect();
    let urls = index["doc_urls"]
        .as_array()
        .unwrap_or_else(|| panic!("no urls"))
        .iter()
        .map(|url| url.as_str().unwrap_or_default().to_owned())
        .collect();
    (docs, urls)
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Vec<&'a str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(open) {
        let tail = &rest[start + open.len()..];
        let Some(end) = tail.find(close) else { break };
        found.push(&tail[..end]);
        rest = &tail[end..];
    }
    found
}

#[test]
fn dx_docs_renders_the_public_api_with_links_search_and_untouched_sources() {
    let root = consumer();
    let before = hashes(&root);
    let run = docs(&root, "//lib:api_docs");
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    assert_eq!(hashes(&root), before, "dx docs changed sources");

    let tree = site(&root, "api_docs");
    let page = read(&tree.join("api.html"));
    for needle in [
        "function addAmounts(left: Amount, right: Amount): Amount",
        "class Ledger",
        "record(amount: Amount): void",
        "count(): number",
        "function badge(background: Property.BackgroundColor): Badge",
        "background: Property.BackgroundColor",
        "Usd = \"USD\"",
        "type Amount = { currency: Currency; minor: number }",
        "<p>Adds two amounts of the same currency.</p>",
        "https://example.test/ledger/blob/main/lib/src/amount.ts#L",
    ] {
        assert!(page.contains(needle), "missing {needle:?} in {page}");
    }
    assert!(
        !page.contains("audit") && !page.contains("Ledger.entries"),
        "private member leaked"
    );

    let anchors: Vec<&str> = between(&page, " id=\"", "\"");
    let links = between(&page, "href=\"#", "\"");
    assert!(links.len() > 10, "{} links", links.len());
    for link in links {
        assert!(anchors.contains(&link), "dangling link #{link}");
    }

    let (docs, urls) = search_docs(&tree);
    let add = docs
        .iter()
        .find(|doc| doc["title"] == "addAmounts")
        .unwrap_or_else(|| panic!("no search record for addAmounts"));
    let body = add["body"].as_str().unwrap_or_default();
    assert!(
        body.contains("Adds two amounts of the same currency."),
        "{body}"
    );
    assert!(
        body.contains("function addAmounts(left: Amount, right: Amount): Amount"),
        "{body}"
    );
    for doc in &docs {
        let at: usize = doc["id"]
            .as_str()
            .unwrap_or_default()
            .parse()
            .unwrap_or(usize::MAX);
        let fragment = urls[at].strip_prefix("api.html#").unwrap_or_default();
        assert!(
            anchors.contains(&fragment),
            "dangling search url {}",
            urls[at]
        );
    }
}

#[test]
fn editing_a_doc_comment_changes_the_rebuilt_site() {
    let root = consumer();
    let first = docs(&root, "//lib:api_docs");
    assert_eq!(first.status.code(), Some(0), "{}", first.combined());
    let tree = site(&root, "api_docs");
    assert!(read(&tree.join("api.html")).contains("Records one amount."));

    write(
        &root.join("lib/src/ledger.ts"),
        &LEDGER_TS.replace("Records one amount.", "Appends one amount."),
    );
    let second = docs(&root, "//lib:api_docs");
    assert_eq!(second.status.code(), Some(0), "{}", second.combined());
    let page = read(&tree.join("api.html"));
    assert!(page.contains("Appends one amount.") && !page.contains("Records one amount."));
}

#[test]
fn a_typedoc_option_variation_changes_the_extracted_set() {
    let root = consumer();
    write(
        &root.join("lib/src/index.ts"),
        &format!("{INDEX_TS}\nexport function undocumented(): void {{}}\n"),
    );
    let full = docs(&root, "//lib:api_docs");
    assert_eq!(full.status.code(), Some(0), "{}", full.combined());
    let strict = docs(&root, "//lib:documented_docs");
    assert_eq!(strict.status.code(), Some(0), "{}", strict.combined());
    let all = read(&site(&root, "api_docs").join("api.html"));
    let documented = read(&site(&root, "documented_docs").join("api.html"));
    assert!(all.contains("function undocumented(): void"));
    assert!(!documented.contains("function undocumented"));
    assert!(documented.contains("function addAmounts"));
}

#[test]
fn failing_extractions_exit_nonzero_and_leave_no_site() {
    for (name, edit) in [
        ("missing_entry_docs", None),
        ("missing_dependency_docs", None),
        (
            "api_docs",
            Some(("lib/src/badge.ts", "export function badge(background: Property.BackgroundColor): Badge {\n  return { background: 1 };\n}\n")),
        ),
    ] {
        let root = consumer();
        let clean = root.bazel(&["clean"]);
        assert!(clean.status.success(), "{}", clean.combined());
        if let Some((path, tail)) = edit {
            let text = read(&root.join(path));
            let head = text.split("/** Builds a badge. */").next().unwrap_or_default().to_owned();
            write(&root.join(path), &format!("{head}{tail}"));
        }
        let target = format!("//lib:{name}");
        let run = docs(&root, &target);
        assert_ne!(run.status.code(), Some(0), "{name}: {}", run.combined());
        let tree = site(&root, name);
        assert!(!tree.exists(), "{name}: stub site at {tree:?}");
        assert!(!tree.with_file_name(format!("{name}_api.md")).exists(), "{name}: stub page");
    }
}
