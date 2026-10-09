//! Asserts the staged shared-app browser directory layout and its relocation.

use std::path::{Path, PathBuf};

const PACKAGE: &str = "rust/tests/fixtures/shared_app/shared_browser";

fn staged(name: &str) -> PathBuf {
    let path = dx_testing::resolve_runfiles(&format!("{PACKAGE}/{name}"));
    assert!(path.exists(), "missing staged file: {name}");
    path
}

fn copy_tree(source: &Path, dest: &Path) {
    std::fs::create_dir_all(dest).expect("create dest");
    let mut entries: Vec<_> = std::fs::read_dir(source)
        .expect("read source")
        .map(|entry| entry.expect("dir entry"))
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let target = dest.join(entry.file_name());
        if std::fs::metadata(entry.path())
            .expect("file metadata")
            .is_dir()
        {
            copy_tree(&entry.path(), &target);
        } else {
            let bytes = std::fs::read(entry.path()).expect("read file");
            std::fs::write(target, bytes).expect("write file");
        }
    }
}

#[test]
fn index_references_the_entry_module() {
    let html = std::fs::read_to_string(staged("index.html")).expect("readable index");
    assert!(
        html.contains("./shared_web.js"),
        "index must load the entry"
    );
    assert!(
        html.contains("id=\"proof\""),
        "index must keep the proof node"
    );
    assert!(
        html.contains("id=\"count\""),
        "index must keep the count node"
    );
    assert!(
        html.contains("id=\"asset\""),
        "index must keep the asset node"
    );
}

#[test]
fn manifest_describes_the_outputs() {
    let manifest = std::fs::read_to_string(staged("manifest.json")).expect("readable manifest");
    assert!(
        manifest.contains("\"entry\":\"shared_web.js\""),
        "unexpected entry"
    );
    assert!(
        manifest.contains("\"wasm\":\"shared_web_bg.wasm\""),
        "unexpected wasm"
    );
    assert!(
        manifest.contains("\"threads\":false"),
        "unexpected threads flag"
    );
}

#[test]
fn entry_exposes_the_shared_core() {
    let js = std::fs::read_to_string(staged("shared_web.js")).expect("readable entry");
    assert!(js.contains("greet"), "entry does not expose greet");
    assert!(js.contains("Counter"), "entry does not expose Counter");
}

#[test]
fn wasm_has_module_magic() {
    let bytes = std::fs::read(staged("shared_web_bg.wasm")).expect("readable wasm");
    assert!(bytes.len() > 8, "wasm is suspiciously small");
    assert_eq!(&bytes[0..4], b"\0asm", "wasm is missing the module magic");
    assert_eq!(
        &bytes[4..8],
        &[1, 0, 0, 0],
        "wasm has an unexpected version"
    );
}

#[test]
fn asset_serves_declared_content() {
    let asset = std::fs::read_to_string(staged("assets/greeting.txt")).expect("readable asset");
    assert_eq!(asset, "shared browser asset\n", "unexpected asset content");
}

#[test]
fn declarations_are_not_served() {
    let root = staged("index.html").parent().expect("parent").to_path_buf();
    assert!(
        !root.join("shared_web.d.ts").exists(),
        "types must not stage"
    );
}

#[test]
fn tree_relocates_to_a_plain_directory() {
    let root = staged("index.html").parent().expect("parent").to_path_buf();
    let dest = dx_testing::mkscratch("shared-app").expect("scratch dir");
    copy_tree(&root, &dest);
    assert!(
        dest.join("index.html").is_file(),
        "relocated index is missing"
    );
    assert!(
        dest.join("shared_web.js").is_file(),
        "relocated entry is missing"
    );
    let bytes = std::fs::read(dest.join("shared_web_bg.wasm")).expect("relocated wasm");
    assert_eq!(&bytes[0..4], b"\0asm", "relocated wasm lost its magic");
    assert_eq!(
        std::fs::read_to_string(dest.join("assets/greeting.txt")).expect("relocated asset"),
        "shared browser asset\n",
        "relocated asset changed",
    );
}
