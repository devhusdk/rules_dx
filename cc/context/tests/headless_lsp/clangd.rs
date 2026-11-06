//! clangd headless LSP qualification.

use cc_context::{AqueryGraph, DATABASE_FILE};
use dx_lsp::*;
use dx_testing::resolve_runfiles;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const INIT_SECS: u64 = 90;
const REQUEST_SECS: u64 = 90;
const CLANGD_PUB_SECS: u64 = 120;
fn within(secs: u64) -> Instant {
    Instant::now() + Duration::from_secs(secs)
}

fn tool(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a runfile"));
    resolve_runfiles(&rel)
}

fn stage(prefix: &str) -> PathBuf {
    let dir = dx_testing::mkscratch(prefix).expect("scratch dir must exist");
    std::fs::create_dir_all(dir.join("home")).expect("home dir must exist");
    dir
}

fn spawn_server(binary: &Path, args: &[String], work: &Path) -> Result<Session, Error> {
    let mut command = Command::new(binary);
    command
        .args(args)
        .current_dir(work)
        .env_clear()
        .env("HOME", work.join("home"))
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", work);
    Session::from_command(command)
}

fn definition(server: &mut Session, uri: &str, line: u32, character: u32) -> Result<Value, Error> {
    let mut response = Value::Null;
    for _ in 0..50 {
        let id = server.request(
            "textDocument/definition",
            json!({"textDocument": {"uri": uri},
                   "position": {"line": line, "character": character}}),
        )?;
        response = server.wait_response(id, "definition", within(REQUEST_SECS))?;
        if error_code(&response) != Some(-32801) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(response)
}

const CLANGD_SENTINEL: &str = "\nint dx_headless_sentinel = dx_undeclared_sentinel;\n";

fn gnu_identity() -> cc_context::CompilerIdentity {
    cc_context::CompilerIdentity {
        configuration: "k8-fastbuild".to_string(),
        compiler: "clang".to_string(),
        family: "clang".to_string(),
        driver: cc_context::Driver::Gnu,
        abi: "itanium".to_string(),
        target: "x86_64-linux-gnu".to_string(),
    }
}

fn clangd_database(work: &Path) -> (String, PathBuf, String) {
    let record = std::fs::read_to_string(tool("DX_LSP_AQUERY")).expect("aquery record must read");
    let graph = AqueryGraph::parse(&record).expect("aquery record must parse");
    let commands = graph
        .compile_commands(Path::new("/"), &[gnu_identity()])
        .expect("compile commands must decode");
    assert!(!commands.is_empty(), "record must hold a compile action");
    let target = commands[0].target.clone();
    let label = target
        .strip_suffix("_upstream")
        .unwrap_or(&target)
        .to_string();
    let source = commands[0].source.clone();
    let entries = cc_context::database(&commands, work, &label, std::slice::from_ref(&source))
        .expect("authoritative database must build");
    assert_eq!(entries.len(), 1, "record must hold one entry");
    let header = commands[0]
        .inputs
        .iter()
        .find(|input| input.ends_with("generated/limits.h"))
        .expect("record must declare the generated header")
        .clone();
    write_text(
        &work.join(&source),
        &std::fs::read_to_string(tool("DX_LSP_PROBE_C")).expect("probe must read"),
    );
    write_text(
        &work.join(&header),
        &std::fs::read_to_string(tool("DX_LSP_LIMITS_H")).expect("header must read"),
    );
    let mut arguments = entries[0].arguments.clone();
    assert!(!arguments.is_empty(), "entry must carry argv");
    arguments[0] = tool("DX_LSP_CLANG").to_string_lossy().into_owned();
    let database = vec![json!({
        "directory": work.to_string_lossy(),
        "file": work.join(&source).to_string_lossy(),
        "arguments": arguments,
    })];
    write_text(
        &work.join(DATABASE_FILE),
        &serde_json::to_string_pretty(&database).expect("database must serialize"),
    );
    let uri = format!("file://{}/{}", work.display(), source);
    (uri, work.join(&source), header)
}

fn start_clangd(work: &Path) -> Result<Session, Error> {
    spawn_server(
        &tool("DX_LSP_CLANGD"),
        &[
            format!("--compile-commands-dir={}", work.display()),
            "--log=error".to_string(),
        ],
        work,
    )
}

#[test]
fn clangd_reports_diagnostic_and_definition() {
    let work = stage("headless-lsp-clangd");
    let (probe_uri, probe_path, header_rel) = clangd_database(&work);
    let clean = std::fs::read_to_string(&probe_path).expect("staged probe must read");

    let mut server = start_clangd(&work).expect("clangd must start");
    let id = server
        .request("initialize", json!({"processId": null, "rootUri": format!("file://{}", work.display()), "capabilities": {}}))
        .expect("initialize must send");
    let init = server
        .wait_response(id, "response", within(INIT_SECS))
        .expect("clangd initialize must succeed");
    assert_eq!(
        init.pointer("/result/capabilities/definitionProvider"),
        Some(&json!(true)),
        "clangd must offer definitions: {init}",
    );
    server
        .notify("initialized", json!({}))
        .expect("initialized must send");
    server
        .notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": probe_uri, "languageId": "c", "version": 1, "text": clean}}),
        )
        .expect("didOpen must send");

    let params = server
        .wait_publication(&probe_uri, false, "diagnostics", within(CLANGD_PUB_SECS))
        .expect("clangd pub must arrive");
    let diags = params["diagnostics"]
        .as_array()
        .expect("diagnostics must be an array");
    assert!(
        diags.iter().all(|d| d.get("severity") != Some(&json!(1))),
        "clean probe must carry no error: {diags:?}",
    );

    let (line, character) = position_of(&clean, "DX_ZERO_RESULT", 0);
    let target =
        definition(&mut server, &probe_uri, line, character).expect("definition must answer");
    let header_text = std::fs::read_to_string(tool("DX_LSP_LIMITS_H")).expect("header must reread");
    let (define_line, _) = position_of(&header_text, "#define DX_ZERO_RESULT", 0);
    let uri = location_uri(&target["result"]).expect("definition must land somewhere");
    assert!(
        uri.ends_with("generated/limits.h"),
        "generated definition must land in limits.h: {uri}"
    );
    assert_eq!(
        target
            .pointer("/result/0/range/start/line")
            .and_then(Value::as_u64),
        Some(define_line as u64),
        "definition must land on the generated define: {target}",
    );

    let dirty = format!("{clean}{CLANGD_SENTINEL}");
    server
        .notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": probe_uri, "version": 2},
                   "contentChanges": [{"text": dirty}]}),
        )
        .expect("didChange must send");
    let params = server
        .wait_publication(&probe_uri, true, "diagnostics", within(CLANGD_PUB_SECS))
        .expect("dirty diagnostic must arrive");
    let diags = params["diagnostics"]
        .as_array()
        .expect("diagnostics must be an array");
    assert!(
        diags.iter().any(|d| d.get("severity") == Some(&json!(1))
            && d.pointer("/message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("dx_undeclared_sentinel")),
        "clangd must report the deliberate error: {diags:?}",
    );

    let unknown = server
        .request("dx/doesNotExist", json!({}))
        .expect("request must send");
    let response = server
        .wait_response(unknown, "response", within(REQUEST_SECS))
        .expect("error must arrive");
    assert_eq!(
        error_code(&response),
        Some(-32601),
        "unsupported method must fail: {response}"
    );

    let slow = server
        .request("workspace/symbol", json!({"query": "classify"}))
        .expect("request must send");
    server
        .notify("$/cancelRequest", json!({"id": slow}))
        .expect("cancel must send");
    let cancelled = server
        .wait_response(slow, "response", within(REQUEST_SECS))
        .expect("cancelled request must settle");
    if let Some(code) = error_code(&cancelled) {
        assert_eq!(
            code, -32800,
            "cancelled request must report cancellation: {cancelled}"
        );
    }
    let (again_line, again_character) = position_of(&clean, "DX_ZERO_RESULT", 0);
    let alive = definition(&mut server, &probe_uri, again_line, again_character)
        .expect("server must stay alive after cancel");
    assert!(
        location_uri(&alive["result"])
            .map(|uri| uri.ends_with("generated/limits.h"))
            .unwrap_or(false),
        "server must answer after cancel: {alive}",
    );

    std::fs::remove_file(work.join(&header_rel)).expect("header removal must succeed");
    server
        .notify(
            "textDocument/didClose",
            json!({"textDocument": {"uri": probe_uri}}),
        )
        .expect("didClose must send");
    server
        .notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": probe_uri, "languageId": "c", "version": 3, "text": dirty}}),
        )
        .expect("reopen must send");
    let deadline = Instant::now() + Duration::from_secs(CLANGD_PUB_SECS);
    let stale = loop {
        let pubs = server.publications(&probe_uri).expect("pubs must read");
        let found = pubs.iter().rev().find(|p| {
            p["diagnostics"]
                .as_array()
                .map(|ds| {
                    ds.iter().any(|d| {
                        d.pointer("/message")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .contains("not found")
                    })
                })
                .unwrap_or(false)
        });
        if let Some(params) = found {
            break params.clone();
        }
        if Instant::now() >= deadline {
            panic!("missing generated header must be explicit; pubs: {pubs:?}");
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(
        stale["diagnostics"]
            .as_array()
            .map(|ds| ds.iter().any(|d| {
                d.pointer("/message")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .contains("limits.h")
            }))
            .unwrap_or(false),
        "stale header diagnostic must name limits.h: {stale}",
    );

    let on_disk = std::fs::read_to_string(&probe_path).expect("staged probe must reread");
    assert_eq!(on_disk, clean, "check mode must preserve source bytes");
    server.shutdown().expect("clangd shutdown must succeed");
}
