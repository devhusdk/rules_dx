//! Asserts the staged shared directory layout and evaluates its asset natively.

use std::path::PathBuf;

const PACKAGE: &str = "examples/shared-rust/app";

fn staged(name: &str) -> PathBuf {
    let path = dx_testing::resolve_runfiles(&format!("{PACKAGE}/{name}"));
    assert!(path.exists(), "missing staged file: {name}");
    path
}

#[test]
fn index_references_the_entry_module() {
    let html = std::fs::read_to_string(staged("index.html")).expect("readable index");
    assert!(
        html.contains("./shared_core.js"),
        "index must load the entry"
    );
    assert!(
        html.contains("id=\"proof\""),
        "index must keep the proof node"
    );
    assert!(
        html.contains("id=\"failure\""),
        "index must keep the failure node"
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
        manifest.contains("\"entry\":\"shared_core.js\""),
        "unexpected entry"
    );
    assert!(
        manifest.contains("\"wasm\":\"shared_core_bg.wasm\""),
        "unexpected wasm"
    );
}

#[test]
fn staged_queries_evaluate_natively() {
    let queries = std::fs::read_to_string(staged("assets/queries.txt")).expect("readable queries");
    let values: Vec<i64> = queries
        .lines()
        .map(|line| shared_core::eval_expr(line).expect("asset line evaluates"))
        .collect();
    assert_eq!(values, vec![14, 2, 1], "unexpected asset evaluation");
}
