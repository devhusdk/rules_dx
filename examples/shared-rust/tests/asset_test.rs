//! Asserts the staged browser layout and loads its resources natively.

use std::path::PathBuf;

const PACKAGE: &str = "examples/shared-rust/app";

fn staged(name: &str) -> PathBuf {
    let path = dx_testing::resolve_runfiles(&format!("{PACKAGE}/{name}"));
    assert!(path.exists(), "missing staged file: {name}");
    path
}

#[test]
fn index_imports_the_generated_interface() {
    let html = std::fs::read_to_string(staged("index.html")).expect("readable index");
    for needle in [
        "./shared_tasks.js",
        "WebBoard",
        "id=\"tasks\"",
        "id=\"summary\"",
        "id=\"asset\"",
    ] {
        assert!(html.contains(needle), "index must keep {needle}");
    }
}

#[test]
fn manifest_describes_the_outputs() {
    let manifest = std::fs::read_to_string(staged("manifest.json")).expect("readable manifest");
    assert!(
        manifest.contains("\"entry\":\"shared_tasks.js\""),
        "unexpected entry: {manifest}"
    );
    assert!(
        manifest.contains("\"wasm\":\"shared_tasks_bg.wasm\""),
        "unexpected wasm: {manifest}"
    );
    let wasm = std::fs::read(staged("shared_tasks_bg.wasm")).expect("readable wasm");
    assert_eq!(&wasm[..4], b"\0asm", "staged module is not Wasm");
    let glue = std::fs::read_to_string(staged("shared_tasks.js")).expect("readable glue");
    assert!(
        glue.contains("class WebBoard"),
        "generated glue must export WebBoard"
    );
}

#[test]
fn staged_resources_load_natively() {
    let board = std::fs::read_to_string(staged("assets/board.txt")).expect("readable board");
    let board = shared_tasks::Board::load(&board).expect("staged board loads");
    assert_eq!(
        board.summary().to_string(),
        "tasks=4 done=1 open=3 assets=1"
    );
    let notes = std::fs::read(staged("assets/notes.txt")).expect("readable notes");
    assert_eq!(
        shared_tasks::asset_report("notes.txt", &notes),
        "asset notes.txt bytes=41 adler32:3d6a0e9d"
    );
}
