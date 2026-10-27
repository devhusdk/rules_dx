//! Builds a PowerShell API site through `dx docs` in a consumer that renames rules_dx.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, resolve_runfiles, sha256_file, Run};

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_dx", version = "0.0.0", repo_name = "dxr")
local_path_override(module_name = "rules_dx", path = "RULES_DX")

bazel_dep(name = "rules_rust", version = "0.74.0")
"#;

const BUILD: &str = r#"load("@dxr//tools/docs/powershell:defs.bzl", "powershell_doc_shard")
load("@dxr//tools/docs/site:site.bzl", "docs_site")

powershell_doc_shard(
    name = "api",
    srcs = glob(["Ledger/**"]),
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

const PROSE: &str = "# Consumer\n\nThe PowerShell API reference of the Ledger module.\n";

const PSD1: &str = "@{\n    RootModule        = 'Ledger.psm1'\n    ModuleVersion     = '2.0.0'\n    GUID              = '1b0c3c36-8a55-4f43-9d5e-0e9b0a1c2d3e'\n    FunctionsToExport = @('Add-LedgerEntry', 'Get-LedgerTotal')\n}\n";

const PSM1: &str = "Set-Content -Path \"$env:TEST_TMPDIR/imported\" -Value 'imported'\nthrow 'imported'\n\nforeach ($file in Get-ChildItem -Path \"$PSScriptRoot/Public\" -Filter *.ps1) {\n    . $file.FullName\n}\n";

const BROKEN_PSM1: &str = "function Get-Broken {\n    param([string]$Name\n    $Name\n}\n";

const ADD_PS1: &str = "function Add-LedgerEntry {\n    <#\n    .SYNOPSIS\n    Adds one entry to a ledger.\n\n    .PARAMETER Amount\n    The amount to add.\n    #>\n    [CmdletBinding()]\n    param(\n        [Parameter(Mandatory)]\n        [int]$Amount\n    )\n    $Amount\n}\n";

const TOTAL_PS1: &str = "function Get-LedgerTotal {\n    <#\n    .SYNOPSIS\n    Sums a ledger.\n    #>\n    param()\n    0\n}\n";

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
    let marker = resolve_runfiles("quality/tests/powershell_docs_consumer_test.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(3)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> Case {
    let rules = rules_dx_root();
    let root = nested::case("powershell_docs_consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("book.toml"), BOOK_TOML);
    write(&root.join("prose.md"), PROSE);
    write(&root.join("Ledger/Ledger.psd1"), PSD1);
    write(&root.join("Ledger/Ledger.psm1"), PSM1);
    write(&root.join("Ledger/Public/Add-LedgerEntry.ps1"), ADD_PS1);
    write(&root.join("Ledger/Public/Get-LedgerTotal.ps1"), TOTAL_PS1);
    root
}

fn docs(root: &Case) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let home = mkscratch("powershell-docs-home").unwrap_or_else(|error| panic!("scratch: {error}"));
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

fn api_pages(root: &Case) -> Vec<String> {
    let dir = site(root).join("api/powershell/Ledger");
    let mut pages: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{dir:?}: {error}"))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("entry: {error}"))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    pages.sort();
    pages
}

fn assert_no_outputs(root: &Case) {
    assert!(
        !site(root).join("api/powershell").exists(),
        "a failed build left PowerShell pages behind"
    );
    assert!(
        !root.join("bazel-bin/api.ir.pb").exists(),
        "a failed build left a shard behind"
    );
}

#[test]
fn dx_docs_renders_the_exported_functions_without_touching_sources() {
    let root = consumer();
    let before = hashes(&root.join("Ledger"));
    let built = docs(&root);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    assert_eq!(
        hashes(&root.join("Ledger")),
        before,
        "dx docs changed a source"
    );

    assert_eq!(
        api_pages(&root),
        ["Add-LedgerEntry.html", "Get-LedgerTotal.html"]
    );
    let add = read(&site(&root).join("api/powershell/Ledger/Add-LedgerEntry.html"));
    assert!(
        add.contains("Add-LedgerEntry [-Amount] &lt;int&gt; [&lt;CommonParameters&gt;]"),
        "{add}"
    );
    assert!(add.contains("Adds one entry to a ledger."), "{add}");
    assert!(
        add.contains(
            "href=\"https://example.com/consumer/blob/main/Ledger/Public/Add-LedgerEntry.ps1#L1\""
        ),
        "{add}"
    );
    let index = read(&site(&root).join("searchindex.js"));
    assert!(
        index.contains("api/powershell/Ledger/Add-LedgerEntry.html"),
        "the search index never names the PowerShell page"
    );
    let checked = root.bazel(&["build", "//:site_docs"]);
    assert!(checked.status.success(), "{}", checked.combined());
}

#[test]
fn functions_to_export_decides_the_page_set() {
    let root = consumer();
    let built = docs(&root);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    write(
        &root.join("Ledger/Ledger.psd1"),
        &PSD1.replace(
            "@('Add-LedgerEntry', 'Get-LedgerTotal')",
            "@('Get-Ledger*')",
        ),
    );
    let rebuilt = docs(&root);
    assert_eq!(rebuilt.status.code(), Some(0), "{}", rebuilt.combined());
    assert_eq!(api_pages(&root), ["Get-LedgerTotal.html"]);
}

#[test]
fn a_module_that_does_not_parse_fails_without_a_stub_page() {
    let root = consumer();
    write(&root.join("Ledger/Ledger.psm1"), BROKEN_PSM1);
    let built = docs(&root);
    assert_ne!(built.status.code(), Some(0), "{}", built.combined());
    assert!(
        built
            .combined()
            .contains("docs_powershell: Ledger/Ledger.psm1:2:"),
        "{}",
        built.combined()
    );
    assert_no_outputs(&root);
}

#[test]
fn a_missing_manifest_fails_without_a_stub_page() {
    let root = consumer();
    write(
        &root.join("BUILD.bazel"),
        &BUILD.replace(
            "    srcs = glob([\"Ledger/**\"]),\n",
            "    srcs = glob([\"Ledger/**\"]),\n    manifest = \"Ledger/Missing.psd1\",\n",
        ),
    );
    let built = docs(&root);
    assert_ne!(built.status.code(), Some(0), "{}", built.combined());
    assert!(
        built.combined().contains("Missing.psd1"),
        "{}",
        built.combined()
    );
    assert_no_outputs(&root);
}
