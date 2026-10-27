//! Asserts a real bundler (esbuild) resolves the staged bindgen companion modules.

const OUT: &str = "javascript/tests/fixtures/web_app/bundle_esbuild";

fn output(name: &str) -> String {
    let path = dx_testing::resolve_runfiles(&format!("{OUT}/{name}"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("unreadable {name}: {e}"))
}

#[test]
fn bundle_inlines_the_companion() {
    let bundle = output("wasm_hello_bundler.js");
    assert!(
        !bundle.contains("./wasm_hello_bundler_bg.js"),
        "companion must be inlined"
    );
    assert!(bundle.contains("greet"), "bundle lost the bindings");
    assert!(
        bundle.contains("wasm_hello_bundler_bg.wasm"),
        "bundle must still reference the wasm"
    );
}

#[test]
fn metafile_inputs_stay_in_the_staged_tree() {
    let meta = output("meta.json");
    assert!(
        meta.contains("app_bundler/wasm_hello_bundler_bg.js"),
        "companion must resolve from the staged tree"
    );
    assert!(
        meta.contains("app_bundler/snippets/"),
        "snippets must resolve from the staged tree"
    );
    assert!(
        !meta.contains("rust/tests/fixtures/wasm_bindgen/"),
        "inputs must not escape the staged tree"
    );
}
