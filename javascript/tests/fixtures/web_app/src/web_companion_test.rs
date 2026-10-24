//! Asserts bundler-style bindgen companion modules are staged with the entry.

const PACKAGE: &str = "javascript/tests/fixtures/web_app/app_bundler";

fn staged(name: &str) -> std::path::PathBuf {
    let path = dx_testing::resolve_runfiles(&format!("{PACKAGE}/{name}"));
    assert!(path.exists(), "missing staged file: {name}");
    path
}

#[test]
fn entry_import_of_the_companion_resolves() {
    let entry = std::fs::read_to_string(staged("wasm_hello_bundler.js")).expect("readable entry");
    assert!(
        entry.contains("./wasm_hello_bundler_bg.js"),
        "entry must import the companion"
    );
    let companion =
        std::fs::read_to_string(staged("wasm_hello_bundler_bg.js")).expect("readable companion");
    assert!(companion.contains("greet"), "companion lost the bindings");
}

#[test]
fn companion_snippet_imports_resolve() {
    let companion =
        std::fs::read_to_string(staged("wasm_hello_bundler_bg.js")).expect("readable companion");
    assert!(companion.contains("./snippets/"), "snippet import missing");
    assert!(staged("snippets").is_dir(), "snippets must be staged");
}
