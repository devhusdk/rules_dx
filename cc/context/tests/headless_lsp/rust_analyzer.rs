//! rust-analyzer headless LSP qualification.

use dx_lsp::*;
use dx_testing::resolve_runfiles;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const INIT_SECS: u64 = 90;
const REQUEST_SECS: u64 = 90;
const SHOW_SECS: u64 = 120;
const QUIET_SECS: u64 = 30;
const RUST_PUB_SECS: u64 = 240;

const SUPPORTED: [(&str, &str); 2] = [("rust", "rust-analyzer"), ("c++", "clangd")];

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

const RUST_LIB: &str = r#"mod featured;
mod generated;

pub fn answer() -> i32 {
    featured::featured() + generated::GENERATED_VALUE
}

pub fn broken() -> i32 {
    undefined_helper(21)
}
"#;

const RUST_FEATURED: &str = r#"#[cfg(feature = "mine")]
pub fn featured() -> i32 {
    41
}
"#;

const RUST_CONSUMER_FEATURED: &str = r#"#[cfg(feature = "mine")]
pub fn featured() -> i32 {
    41
}

#[cfg(consumer)]
pub fn tuned() -> i32 {
    7
}
"#;

const RUST_CONSUMER_LIB: &str = r#"mod featured;
mod generated;

pub fn answer() -> i32 {
    featured::featured() + featured::tuned() + generated::GENERATED_VALUE
}
"#;

const RUST_GENERATED: &str = "pub const GENERATED_VALUE: i32 = 21;\n";

fn rust_sysroot(project_rustc: &Path) -> (String, String) {
    let output = Command::new(project_rustc)
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("project rustc must run");
    assert!(
        output.status.success(),
        "rustc --print sysroot must succeed"
    );
    let sysroot = String::from_utf8(output.stdout)
        .expect("sysroot must be utf8")
        .trim()
        .to_string();
    let src = format!("{sysroot}/lib/rustlib/src/library");
    assert!(
        Path::new(&src).join("core/src/lib.rs").exists(),
        "managed sysroot must carry library sources at {src}"
    );
    (sysroot, src)
}

fn rust_project(work: &Path, sysroot: &str, sysroot_src: &str, cfgs: &[&str]) -> Value {
    let root_module = work.join("src/lib.rs").to_string_lossy().into_owned();
    let include = work.join("src").to_string_lossy().into_owned();
    json!({
        "sysroot": sysroot,
        "sysroot_src": sysroot_src,
        "crates": [{
            "display_name": "demo",
            "root_module": root_module,
            "edition": "2021",
            "cfg": cfgs,
            "deps": [],
            "source": {"include_dirs": [include], "exclude_dirs": []},
            "is_workspace_member": true,
            "is_proc_macro": false,
            "target": "x86_64-unknown-linux-gnu",
            "env": {},
        }],
        "runnables": [],
    })
}

fn check_command(check_rustc: &Path, out: &Path, cfgs: &[&str]) -> Vec<String> {
    let mut command = vec![
        check_rustc.to_string_lossy().into_owned(),
        "--error-format=json".to_string(),
        "--crate-type=lib".to_string(),
        "--crate-name=demo".to_string(),
        "--edition=2021".to_string(),
    ];
    for cfg in cfgs {
        command.push(format!("--cfg={cfg}"));
    }
    command.push(format!("--out-dir={}", out.display()));
    command.push("{saved_file}".to_string());
    command
}

fn start_rust(work: &Path) -> Result<(Session, String), Error> {
    let server = spawn_server(&tool("DX_LSP_RUST_ANALYZER"), &[], work)?;
    let root = format!("file://{}", work.display());
    Ok((server, root))
}

fn open_rust(
    server: &mut Session,
    root: &str,
    work: &Path,
    names: &[&str],
) -> Result<Vec<(String, String)>, Error> {
    let mut opened = Vec::new();
    for name in names {
        let path = work.join(name);
        let text = std::fs::read_to_string(&path).expect("staged source must read");
        let uri = format!("{root}/{name}");
        server.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri, "languageId": "rust", "version": 1, "text": text,
            }}),
        )?;
        opened.push((uri, text));
    }
    Ok(opened)
}

fn init_rust(server: &mut Session, root: &str, check: &[String]) -> Result<Value, Error> {
    let options = if check.is_empty() {
        json!({})
    } else {
        json!({"check": {"overrideCommand": check}})
    };
    let id = server.request(
        "initialize",
        json!({"processId": null, "rootUri": root, "capabilities": {},
               "initializationOptions": options}),
    )?;
    let response = server.wait_response(id, "rust initialize", within(INIT_SECS))?;
    if error_code(&response).is_some() {
        return Err(Error::Server(
            error_code(&response).unwrap_or(-1),
            format!("rust initialize failed: {response}"),
        ));
    }
    server.notify("initialized", json!({}))?;
    Ok(response)
}

#[test]
fn rust_reports_check_diagnostic_and_definition() {
    let work = stage("headless-lsp-rust");
    write_text(&work.join("src/lib.rs"), RUST_LIB);
    write_text(&work.join("src/featured.rs"), RUST_FEATURED);
    write_text(&work.join("src/generated.rs"), RUST_GENERATED);
    let (sysroot, sysroot_src) = rust_sysroot(&tool("DX_LSP_RUSTC_PROJECT"));
    write_text(
        &work.join("rust-project.json"),
        &serde_json::to_string(&rust_project(
            &work,
            &sysroot,
            &sysroot_src,
            &["feature=\"mine\""],
        ))
        .expect("project must serialize"),
    );
    let out = work.join("out");
    std::fs::create_dir_all(&out).expect("out dir must exist");
    let check = check_command(&tool("DX_LSP_RUSTC_CHECK"), &out, &["feature=\"mine\""]);

    let (mut server, root) = start_rust(&work).expect("rust-analyzer must start");
    let init = init_rust(&mut server, &root, &check).expect("rust initialize must succeed");
    assert_eq!(
        init.pointer("/result/serverInfo/name")
            .and_then(Value::as_str),
        Some("rust-analyzer"),
        "server identity must be rust-analyzer: {init}",
    );
    eprintln!(
        "rust-analyzer version: {}",
        init.pointer("/result/serverInfo/version")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    );
    let opened = open_rust(
        &mut server,
        &root,
        &work,
        &["src/lib.rs", "src/featured.rs", "src/generated.rs"],
    )
    .expect("didOpen must send");
    let lib_uri = opened[0].0.clone();
    let lib_text = opened[0].1.clone();
    server
        .wait_publication(&lib_uri, false, "diagnostics", within(RUST_PUB_SECS))
        .expect("workspace must load");
    server
        .notify(
            "textDocument/didSave",
            json!({"textDocument": {"uri": lib_uri}}),
        )
        .expect("didSave must send");

    let params = server
        .wait_publication(&lib_uri, true, "diagnostics", within(RUST_PUB_SECS))
        .expect("rust diagnostic must arrive");
    let diags = params["diagnostics"]
        .as_array()
        .expect("diagnostics must be an array");
    assert!(
        diags
            .iter()
            .any(|d| d.get("source") == Some(&json!("rustc"))
                && d.get("code") == Some(&json!("E0425"))
                && d.pointer("/message")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .contains("undefined_helper")),
        "managed check must report the deliberate E0425: {diags:?}",
    );

    let (line, character) = position_of(&lib_text, "featured::featured", 0);
    let target = definition(
        &mut server,
        &lib_uri,
        line,
        character + "featured::".len() as u32,
    )
    .expect("definition must answer");
    assert_eq!(
        location_uri(&target["result"])
            .as_deref()
            .map(|uri| uri.ends_with("src/featured.rs")),
        Some(true),
        "feature-gated definition must land in featured.rs: {target}",
    );
    let (gen_line, gen_character) = position_of(&lib_text, "GENERATED_VALUE", 0);
    let generated = definition(&mut server, &lib_uri, gen_line, gen_character)
        .expect("generated definition must answer");
    assert_eq!(
        location_uri(&generated["result"])
            .as_deref()
            .map(|uri| uri.ends_with("src/generated.rs")),
        Some(true),
        "generated-source definition must land in generated.rs: {generated}",
    );

    for (uri, _) in opened.iter().skip(1) {
        let latest = server.publications(uri).expect("pubs must read");
        assert!(
            latest.iter().all(|p| p["diagnostics"]
                .as_array()
                .map(Vec::is_empty)
                .unwrap_or(true)),
            "supporting files must stay clean: {latest:?}",
        );
    }
    let fresh = std::fs::read_to_string(work.join("src/lib.rs")).expect("source must reread");
    assert_eq!(fresh, RUST_LIB, "check mode must preserve source bytes");
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
    server.shutdown().expect("rust shutdown must succeed");
}

#[test]
fn rust_consumer_flags_and_renamed_repo() {
    let work = stage("renamed-consumer");
    write_text(&work.join("src/lib.rs"), RUST_CONSUMER_LIB);
    write_text(&work.join("src/featured.rs"), RUST_CONSUMER_FEATURED);
    write_text(&work.join("src/generated.rs"), RUST_GENERATED);
    let (sysroot, sysroot_src) = rust_sysroot(&tool("DX_LSP_RUSTC_PROJECT"));
    write_text(
        &work.join("rust-project.json"),
        &serde_json::to_string(&rust_project(
            &work,
            &sysroot,
            &sysroot_src,
            &["feature=\"mine\"", "consumer"],
        ))
        .expect("project must serialize"),
    );
    let out = work.join("out");
    std::fs::create_dir_all(&out).expect("out dir must exist");
    let check = check_command(
        &tool("DX_LSP_RUSTC_CHECK"),
        &out,
        &["feature=\"mine\"", "consumer"],
    );

    let (mut server, root) = start_rust(&work).expect("rust-analyzer must start");
    init_rust(&mut server, &root, &check).expect("rust initialize must succeed");
    let opened = open_rust(
        &mut server,
        &root,
        &work,
        &["src/lib.rs", "src/featured.rs", "src/generated.rs"],
    )
    .expect("didOpen must send");
    let lib_uri = opened[0].0.clone();
    let lib_text = opened[0].1.clone();
    server
        .wait_publication(&lib_uri, false, "diagnostics", within(RUST_PUB_SECS))
        .expect("workspace must load");
    server
        .notify(
            "textDocument/didSave",
            json!({"textDocument": {"uri": lib_uri}}),
        )
        .expect("didSave must send");

    let deadline = Instant::now() + Duration::from_secs(QUIET_SECS);
    while Instant::now() < deadline {
        let pubs = server.publications(&lib_uri).expect("pubs must read");
        assert!(
            pubs.iter().all(|p| p["diagnostics"]
                .as_array()
                .map(Vec::is_empty)
                .unwrap_or(true)),
            "consumer fixture must stay clean: {pubs:?}",
        );
        std::thread::sleep(Duration::from_secs(1));
    }
    let last = server
        .publications(&lib_uri)
        .expect("pubs must read")
        .pop()
        .expect("rust pub must exist");
    assert!(
        last["diagnostics"]
            .as_array()
            .map(Vec::is_empty)
            .unwrap_or(false),
        "clean consumer fixture must pass: {:?}",
        last["diagnostics"],
    );

    let (line, character) = position_of(&lib_text, "featured::tuned", 0);
    let target = definition(
        &mut server,
        &lib_uri,
        line,
        character + "featured::".len() as u32,
    )
    .expect("consumer definition must answer");
    assert_eq!(
        location_uri(&target["result"])
            .as_deref()
            .map(|uri| uri.ends_with("src/featured.rs")),
        Some(true),
        "consumer flag definition must land in featured.rs: {target}",
    );
    server.shutdown().expect("rust shutdown must succeed");
}

#[test]
fn rust_wrong_target_config_is_explicit() {
    let work = stage("headless-lsp-rust-wrong-config");
    write_text(&work.join("src/lib.rs"), RUST_LIB);
    write_text(&work.join("src/featured.rs"), RUST_FEATURED);
    write_text(&work.join("src/generated.rs"), RUST_GENERATED);
    let (sysroot, sysroot_src) = rust_sysroot(&tool("DX_LSP_RUSTC_PROJECT"));
    write_text(
        &work.join("rust-project.json"),
        &serde_json::to_string(&rust_project(&work, &sysroot, &sysroot_src, &[]))
            .expect("project must serialize"),
    );
    let out = work.join("out");
    std::fs::create_dir_all(&out).expect("out dir must exist");
    let check = check_command(&tool("DX_LSP_RUSTC_CHECK"), &out, &[]);

    let (mut server, root) = start_rust(&work).expect("rust-analyzer must start");
    init_rust(&mut server, &root, &check).expect("rust initialize must succeed");
    let opened = open_rust(
        &mut server,
        &root,
        &work,
        &["src/lib.rs", "src/featured.rs"],
    )
    .expect("didOpen must send");
    let lib_uri = opened[0].0.clone();
    let lib_text = opened[0].1.clone();
    server
        .wait_publication(&lib_uri, false, "diagnostics", within(RUST_PUB_SECS))
        .expect("workspace must load");
    server
        .notify(
            "textDocument/didSave",
            json!({"textDocument": {"uri": lib_uri}}),
        )
        .expect("didSave must send");

    let params = server
        .wait_publication(&lib_uri, true, "diagnostics", within(RUST_PUB_SECS))
        .expect("rust diagnostic must arrive");
    let diags = params["diagnostics"]
        .as_array()
        .expect("diagnostics must be an array");
    assert!(
        diags.iter().any(|d| d.get("code") == Some(&json!("E0425"))
            && d.pointer("/message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("featured")),
        "missing feature must surface as an explicit E0425: {diags:?}",
    );
    let (line, character) = position_of(&lib_text, "featured::featured", 0);
    let target = definition(
        &mut server,
        &lib_uri,
        line,
        character + "featured::".len() as u32,
    )
    .expect("definition must answer");
    assert_eq!(
        target.get("result"),
        Some(&json!([])),
        "inactive feature must have no definition target: {target}",
    );
    server.shutdown().expect("rust shutdown must succeed");
}

#[test]
fn rust_unavailable_check_command_is_explicit() {
    let work = stage("headless-lsp-rust-no-check");
    write_text(&work.join("src/lib.rs"), RUST_LIB);
    let (sysroot, sysroot_src) = rust_sysroot(&tool("DX_LSP_RUSTC_PROJECT"));
    write_text(
        &work.join("rust-project.json"),
        &serde_json::to_string(&rust_project(
            &work,
            &sysroot,
            &sysroot_src,
            &["feature=\"mine\""],
        ))
        .expect("project must serialize"),
    );
    let check = vec![
        "/nonexistent/dx-check".to_string(),
        "{saved_file}".to_string(),
    ];

    let (mut server, root) = start_rust(&work).expect("rust-analyzer must start");
    init_rust(&mut server, &root, &check).expect("rust initialize must succeed");
    let opened = open_rust(&mut server, &root, &work, &["src/lib.rs"]).expect("didOpen must send");
    server
        .wait_publication(&opened[0].0, false, "diagnostics", within(RUST_PUB_SECS))
        .expect("workspace must load");
    server
        .notify(
            "textDocument/didSave",
            json!({"textDocument": {"uri": opened[0].0}}),
        )
        .expect("didSave must send");
    let message = server
        .wait_show_message("/nonexistent/dx-check", "showMessage", within(SHOW_SECS))
        .expect("unavailable check must be reported");
    assert!(
        message
            .pointer("/params/message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .contains("Failed to spawn"),
        "missing check binary must fail explicitly: {message}",
    );
    server
        .shutdown()
        .expect("server must stay usable after the failure");
}

#[test]
fn rust_malformed_project_is_explicit() {
    let work = stage("headless-lsp-rust-malformed");
    write_text(&work.join("src/lib.rs"), RUST_LIB);
    write_text(&work.join("rust-project.json"), "{not json");
    let check: Vec<String> = vec![];

    let (mut server, root) = start_rust(&work).expect("rust-analyzer must start");
    init_rust(&mut server, &root, &check).expect("rust initialize must succeed");
    server
        .wait_show_message("Failed to load workspace", "showMessage", within(SHOW_SECS))
        .expect("malformed project must be reported");
    server
        .shutdown()
        .expect("server must stay usable after the failure");
}

#[test]
fn qualified_servers_are_pinned() {
    assert_eq!(
        SUPPORTED,
        [("rust", "rust-analyzer"), ("c++", "clangd")],
        "only rust-analyzer and clangd are qualified headless servers",
    );
}
