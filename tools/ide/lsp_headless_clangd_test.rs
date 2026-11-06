use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dx_lsp::*;

fn tool_path(variable: &str) -> PathBuf {
    let rel =
        std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must name the managed tool"));
    dx_testing::resolve_runfiles(&rel)
}

const CPP_HEADER: &str = "inline int generated_helper(int value) {\n    return value * 2;\n}\n";

const CPP_MAIN: &str = "#include \"probe_generated.h\"\n\nint main() {\n    int value = generated_helper(21);\n    int broken = value + unknown_identifier;\n    return broken;\n}\n";

struct CppFixture {
    _scratch: dx_test_scratch::TempDir,
    root: PathBuf,
    directory: PathBuf,
    main_uri: String,
    header_uri: String,
    main_text: String,
}

fn stage_cpp(tag: &str, clang: &Path) -> CppFixture {
    let root_tmp = dx_test_scratch::scratch(tag);
    let root = root_tmp.path().to_path_buf();
    let gen_dir = root.join("gen");
    let header = gen_dir.join("probe_generated.h");
    write_text(&header, CPP_HEADER);
    let main = root.join("main.cc");
    write_text(&main, CPP_MAIN);
    let entry = cc_context::Entry {
        directory: root.to_string_lossy().into_owned(),
        file: main.to_string_lossy().into_owned(),
        arguments: vec![
            clang.to_string_lossy().into_owned(),
            "-c".to_owned(),
            main.to_string_lossy().into_owned(),
            "-o".to_owned(),
            root.join("main.o").to_string_lossy().into_owned(),
            "-I".to_owned(),
            gen_dir.to_string_lossy().into_owned(),
            "-std=c++17".to_owned(),
        ],
    };
    let database = serde_json::to_string_pretty(&vec![entry]).expect("compile commands serialize");
    write_text(
        &root.join(cc_context::DATABASE_FILE),
        &format!("{database}\n"),
    );
    CppFixture {
        main_uri: file_uri(&main),
        header_uri: file_uri(&header),
        main_text: CPP_MAIN.to_owned(),
        directory: root.clone(),
        root,
        _scratch: root_tmp,
    }
}

#[test]
fn clangd_reports_diagnostic_and_definition_for_generated_sources() {
    let clang = tool_path("DX_LSP_CLANG");
    let clangd = tool_path("DX_LSP_CLANGD");
    let fixture = stage_cpp("lsp-clangd", &clang);
    let vars = scoped_env(&fixture.root);
    require_context(&fixture.directory, cc_context::DATABASE_FILE)
        .expect("generated context exists");
    let dir_arg = format!("--compile-commands-dir={}", fixture.directory.display());
    let mut session = Session::spawn(&clangd, &[dir_arg.as_str()], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(90);
    session
        .initialize(&file_uri(&fixture.directory), "clangd initialize", deadline)
        .expect("clangd initializes");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": fixture.main_uri,
                    "languageId": "cpp",
                    "version": 1,
                    "text": fixture.main_text,
                },
            }),
        )
        .expect("clangd opens the fixture");
    let diagnostics = session
        .wait_diagnostics(&fixture.main_uri, "clangd diagnostics", deadline)
        .expect("clangd publishes the semantic diagnostic");
    let items = diagnostics.as_array().expect("diagnostics array");
    let offset = fixture
        .main_text
        .find("unknown_identifier")
        .expect("fixture holds the error");
    let (error_line, _) = offset_to_position(&fixture.main_text, offset);
    let hit = items
        .iter()
        .find(|item| {
            item.get("code") == Some(&serde_json::json!("undeclared_var_use"))
                && item
                    .get("message")
                    .and_then(serde_json::Value::as_str)
                    .map(|message| message.contains("unknown_identifier"))
                    .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("undeclared identifier diagnostic, saw {diagnostics}"));
    assert_eq!(
        hit.pointer("/range/start/line"),
        Some(&serde_json::json!(error_line)),
        "diagnostic lands on the error line"
    );
    let result = definition_uri(
        &mut session,
        &fixture.main_uri,
        &fixture.main_text,
        "generated_helper(21)",
        "clangd definition",
        deadline,
    )
    .expect("clangd answers definition");
    let target = first_location(&result, "generated helper definition");
    assert_eq!(
        target.get("uri").and_then(serde_json::Value::as_str),
        Some(fixture.header_uri.as_str()),
        "definition lands in the generated header"
    );
    session.shutdown().expect("clangd exits cleanly");
}

#[test]
fn clangd_missing_context_is_explicit() {
    let empty_tmp = dx_test_scratch::scratch("lsp-clangd-empty");
    let empty = empty_tmp.path().to_path_buf();
    let err =
        require_context(&empty, cc_context::DATABASE_FILE).expect_err("missing context must fail");
    assert!(matches!(err, Error::MissingContext(_)), "unexpected {err}");
    let clang = tool_path("DX_LSP_CLANG");
    let staged = stage_cpp("lsp-clangd-bare", &clang);
    let bare_tmp = dx_test_scratch::scratch("lsp-clangd-bare-tree");
    let bare = bare_tmp.path().to_path_buf();
    let main = bare.join("main.cc");
    write_text(&main, &staged.main_text);
    let main_uri = file_uri(&main);
    let vars = scoped_env(&bare);
    let clangd = tool_path("DX_LSP_CLANGD");
    let dir_arg = format!("--compile-commands-dir={}", empty.display());
    let mut session = Session::spawn(&clangd, &[dir_arg.as_str()], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(90);
    session
        .initialize(&file_uri(&bare), "clangd initialize", deadline)
        .expect("clangd initializes");
    session
        .notify(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "cpp",
                    "version": 1,
                    "text": staged.main_text,
                },
            }),
        )
        .expect("clangd opens the fixture");
    let result = definition_uri(
        &mut session,
        &main_uri,
        &staged.main_text,
        "generated_helper(21)",
        "clangd definition without context",
        deadline,
    )
    .expect("clangd answers definition");
    assert_eq!(
        result,
        serde_json::json!([]),
        "without the generated context the cross-file fact vanishes"
    );
    session.shutdown().expect("clangd exits cleanly");
}

#[test]
fn clangd_unsupported_request_reports_method_not_found() {
    let root_tmp = dx_test_scratch::scratch("lsp-clangd-errors");
    let root = root_tmp.path().to_path_buf();
    let vars = scoped_env(&root);
    let clangd = tool_path("DX_LSP_CLANGD");
    let mut session = Session::spawn(&clangd, &[], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    session
        .initialize(&file_uri(&root), "clangd initialize", deadline)
        .expect("clangd initializes");
    let id = session
        .request("workspace/dxProbeDoesNotExist", serde_json::json!({}))
        .expect("request sends");
    let response = session
        .wait_response(id, "unknown method answer", deadline)
        .expect("server answers unknown method");
    let err = ok_result(&response).expect_err("unknown method must fail");
    assert!(matches!(err, Error::Server(-32601, _)), "unexpected {err}");
    session.shutdown().expect("clangd exits cleanly");
}

#[test]
fn clangd_wrong_target_reports_server_error() {
    let root_tmp = dx_test_scratch::scratch("lsp-clangd-target");
    let root = root_tmp.path().to_path_buf();
    let vars = scoped_env(&root);
    let clangd = tool_path("DX_LSP_CLANGD");
    let mut session = Session::spawn(&clangd, &[], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    session
        .initialize(&file_uri(&root), "clangd initialize", deadline)
        .expect("clangd initializes");
    let id = session
        .request(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": {"uri": "file:///never-opened-dx-probe.cc"},
                "position": {"line": 0, "character": 0},
            }),
        )
        .expect("request sends");
    let response = session
        .wait_response(id, "unopened file answer", deadline)
        .expect("server answers wrong target");
    let err = ok_result(&response).expect_err("unopened file must fail");
    assert!(matches!(err, Error::Server(_, _)), "unexpected {err}");
    session.shutdown().expect("clangd exits cleanly");
}

#[test]
fn lsp_wait_timeout_reports_cancellation() {
    let root_tmp = dx_test_scratch::scratch("lsp-quiet");
    let root = root_tmp.path().to_path_buf();
    let vars = scoped_env(&root);
    let clangd = tool_path("DX_LSP_CLANGD");
    let mut session = Session::spawn(&clangd, &[], &vars).expect("clangd starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    session
        .initialize(&file_uri(&root), "clangd initialize", deadline)
        .expect("clangd initializes");
    let quiet = Instant::now() + Duration::from_millis(200);
    let err = session
        .wait_diagnostics(
            "file:///never-opened-dx-probe.cc",
            "quiet diagnostics",
            quiet,
        )
        .expect_err("quiet server must time out");
    assert!(matches!(err, Error::Timeout(_)), "unexpected {err}");
    session.shutdown().expect("clangd exits cleanly");
}
