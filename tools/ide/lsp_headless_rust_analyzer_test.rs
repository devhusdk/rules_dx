use std::path::PathBuf;
use std::time::{Duration, Instant};

use dx_lsp::*;

fn toolchain_pick(variable: &str, suffix: &str) -> PathBuf {
    let rels = std::env::var(variable)
        .unwrap_or_else(|_| panic!("{variable} must list managed toolchain files"));
    let mut found = Vec::new();
    for rel in rels.split_whitespace() {
        let path = dx_testing::resolve_runfiles(rel);
        if path.ends_with(suffix) {
            found.push(path);
        }
    }
    assert!(
        found.len() == 1,
        "{variable} must carry exactly one {suffix}, saw {}",
        found.len()
    );
    found.pop().expect("exactly one match")
}

const RUST_LIB: &str = "mod typed;\n\n#[cfg(feature = \"dx_probe\")]\npub fn feature_helper(value: i32) -> i32 {\n    value * 2\n}\n\npub fn base_helper(value: i32) -> i32 {\n    value + 1\n}\n\npub fn describe(value: i32) -> i32 {\n    let stepped = base_helper(value);\n    #[cfg(feature = \"dx_probe\")]\n    let stepped = feature_helper(stepped);\n    stepped\n}\n";

const RUST_TYPED: &str =
    "pub fn typed() -> i32 {\n    let mismatch: i32 = \"not an integer\";\n    mismatch\n}\n";

const RUST_BROKEN: &str = "fn broken( {\n    let x =\n}\n";

struct RustFixture {
    _scratch: dx_test_scratch::TempDir,
    root: PathBuf,
    lib_uri: String,
    lib_text: String,
    typed_uri: String,
    broken_uri: String,
}

fn stage_rust(tag: &str, feature: bool) -> RustFixture {
    let root_tmp = dx_test_scratch::scratch(tag);
    let root = root_tmp.path().to_path_buf();
    let src = root.join("src");
    let lib = src.join("lib.rs");
    write_text(&lib, RUST_LIB);
    let typed = src.join("typed.rs");
    write_text(&typed, RUST_TYPED);
    let broken = src.join("broken.rs");
    write_text(&broken, RUST_BROKEN);
    let cfg: Vec<&str> = if feature {
        vec!["feature=\"dx_probe\""]
    } else {
        Vec::new()
    };
    let project = serde_json::json!({
        "crates": [{
            "display_name": "dx_probe",
            "root_module": lib.to_string_lossy(),
            "edition": "2021",
            "deps": [],
            "cfg": cfg,
            "is_workspace_member": true,
        }],
    });
    let rendered = serde_json::to_string_pretty(&project).expect("rust project serializes");
    write_text(&root.join("rust-project.json"), &format!("{rendered}\n"));
    RustFixture {
        lib_uri: file_uri(&lib),
        lib_text: RUST_LIB.to_owned(),
        typed_uri: file_uri(&typed),
        broken_uri: file_uri(&broken),
        root,
        _scratch: root_tmp,
    }
}

#[test]
fn rust_project_definition_resolves_through_selected_features() {
    let analyzer = toolchain_pick("DX_LSP_TOOLCHAIN_FILES", "bin/rust-analyzer");
    let fixture = stage_rust("lsp-rust-feature", true);
    let vars = scoped_env(&fixture.root);
    require_context(&fixture.root, "rust-project.json").expect("generated context exists");
    let mut session = Session::spawn(&analyzer, &[], &vars).expect("rust-analyzer starts");
    let deadline = Instant::now() + Duration::from_secs(240);
    session
        .initialize(
            &file_uri(&fixture.root),
            "rust-analyzer initialize",
            deadline,
        )
        .expect("rust-analyzer initializes");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.lib_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": fixture.lib_text,
                },
            }),
        )
        .expect("rust-analyzer opens the fixture");
    let result = poll_definition(
        &mut session,
        &fixture.lib_uri,
        &fixture.lib_text,
        "base_helper(value)",
        "rust-analyzer definition",
        deadline,
    )
    .expect("rust-analyzer answers definition");
    let target = first_location(&result, "base helper definition");
    assert_eq!(
        target.get("uri").and_then(serde_json::Value::as_str),
        Some(fixture.lib_uri.as_str()),
        "definition resolves through the project context"
    );
    let offset = fixture
        .lib_text
        .find("feature_helper(stepped)")
        .expect("fixture holds the feature use");
    let (line, character) = offset_to_position(&fixture.lib_text, offset + 2);
    let id = session
        .request(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": {"uri": fixture.lib_uri},
                "position": {"line": line, "character": character},
            }),
        )
        .expect("hover sends");
    let response = session
        .wait_response(id, "rust-analyzer hover", deadline)
        .expect("rust-analyzer answers hover");
    let result = ok_result(&response).expect("hover answers");
    let text = result
        .pointer("/contents/value")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("hover text, saw {result}"));
    assert!(
        text.contains("dx_probe") && text.contains("feature_helper"),
        "hover carries the project identity: {text}"
    );
    session.shutdown().expect("rust-analyzer exits cleanly");
}

#[test]
fn rust_project_hides_feature_gated_items_without_the_feature() {
    let analyzer = toolchain_pick("DX_LSP_TOOLCHAIN_FILES", "bin/rust-analyzer");
    let fixture = stage_rust("lsp-rust-no-feature", false);
    let vars = scoped_env(&fixture.root);
    require_context(&fixture.root, "rust-project.json").expect("generated context exists");
    let mut session = Session::spawn(&analyzer, &[], &vars).expect("rust-analyzer starts");
    let deadline = Instant::now() + Duration::from_secs(240);
    session
        .initialize(
            &file_uri(&fixture.root),
            "rust-analyzer initialize",
            deadline,
        )
        .expect("rust-analyzer initializes");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.lib_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": fixture.lib_text,
                },
            }),
        )
        .expect("rust-analyzer opens the fixture");
    let result = poll_definition(
        &mut session,
        &fixture.lib_uri,
        &fixture.lib_text,
        "base_helper(value)",
        "rust-analyzer definition",
        deadline,
    )
    .expect("rust-analyzer answers definition");
    let target = first_location(&result, "base helper definition");
    assert_eq!(
        target.get("uri").and_then(serde_json::Value::as_str),
        Some(fixture.lib_uri.as_str()),
        "ungated items still resolve"
    );
    let result = definition_uri(
        &mut session,
        &fixture.lib_uri,
        &fixture.lib_text,
        "feature_helper(stepped)",
        "rust-analyzer gated definition",
        deadline,
    )
    .expect("rust-analyzer answers gated definition");
    assert!(
        result == serde_json::Value::Null || result == serde_json::json!([]),
        "without the feature the gated item hides: {result}"
    );
    session.shutdown().expect("rust-analyzer exits cleanly");
}

#[test]
fn rust_project_syntax_diagnostic_publishes_without_check_runner() {
    let analyzer = toolchain_pick("DX_LSP_TOOLCHAIN_FILES", "bin/rust-analyzer");
    let fixture = stage_rust("lsp-rust-syntax", true);
    let vars = scoped_env(&fixture.root);
    require_context(&fixture.root, "rust-project.json").expect("generated context exists");
    let mut session = Session::spawn(&analyzer, &[], &vars).expect("rust-analyzer starts");
    let deadline = Instant::now() + Duration::from_secs(240);
    session
        .initialize(
            &file_uri(&fixture.root),
            "rust-analyzer initialize",
            deadline,
        )
        .expect("rust-analyzer initializes");
    let broken = std::fs::read_to_string(fixture.root.join("src").join("broken.rs"))
        .expect("broken fixture reads");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.broken_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": broken,
                },
            }),
        )
        .expect("rust-analyzer opens the broken file");
    let diagnostics = session
        .wait_diagnostics(&fixture.broken_uri, "rust-analyzer syntax", deadline)
        .expect("rust-analyzer publishes syntax diagnostics");
    let items = diagnostics.as_array().expect("diagnostics array");
    assert!(
        items
            .iter()
            .any(|item| { item.get("code") == Some(&serde_json::json!("syntax-error")) }),
        "syntax error diagnostic, saw {diagnostics}"
    );
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.typed_uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": RUST_TYPED,
                },
            }),
        )
        .expect("rust-analyzer opens the typed file");
    let quiet = Instant::now() + Duration::from_secs(60);
    let err = session
        .wait_diagnostics(&fixture.typed_uri, "native type diagnostics", quiet)
        .expect_err("without a check runner there are no native type diagnostics");
    assert!(matches!(err, Error::Timeout(_)), "unexpected {err}");
    session.shutdown().expect("rust-analyzer exits cleanly");
}

#[test]
fn rust_project_malformed_json_is_explicit() {
    let analyzer = toolchain_pick("DX_LSP_TOOLCHAIN_FILES", "bin/rust-analyzer");
    let fixture = stage_rust("lsp-rust-malformed", true);
    write_text(&fixture.root.join("rust-project.json"), "{not json");
    let vars = scoped_env(&fixture.root);
    let mut session = Session::spawn(&analyzer, &[], &vars).expect("rust-analyzer starts");
    let deadline = Instant::now() + Duration::from_secs(240);
    session
        .initialize(
            &file_uri(&fixture.root),
            "rust-analyzer initialize",
            deadline,
        )
        .expect("rust-analyzer initializes");
    let quiet = Instant::now() + Duration::from_secs(30);
    let err = session
        .wait_diagnostics(&fixture.lib_uri, "malformed project", quiet)
        .expect_err("a malformed project publishes no diagnostics");
    let Error::Timeout(detail) = err else {
        panic!("unexpected {err}");
    };
    assert!(
        detail.contains("Failed to deserialize json file"),
        "malformed project must be reported on stderr: {detail}"
    );
    session.shutdown().expect("server stays usable");
}
