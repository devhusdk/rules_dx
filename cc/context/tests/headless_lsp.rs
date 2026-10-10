//! Headless LSP qualification for the managed language servers.
//!
//! A small test client speaks the upstream JSON-RPC protocol directly to the
//! selected managed binaries: rust-analyzer with a rustc check command for
//! Rust, and clangd with an authoritative compilation database for C. Only
//! these two servers are qualified here; every other language server stays
//! unqualified and must not be advertised as a supported headless path.

use cc_context::{AqueryGraph, DATABASE_FILE};
use dx_testing::{self, resolve_runfiles};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SUPPORTED: [(&str, &str); 2] = [("rust", "rust-analyzer"), ("c++", "clangd")];

const INIT_SECS: u64 = 90;
const RUST_PUB_SECS: u64 = 240;
const CLANGD_PUB_SECS: u64 = 120;
const REQUEST_SECS: u64 = 90;
const SHOW_SECS: u64 = 120;
const QUIET_SECS: u64 = 30;

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

const CLANGD_SENTINEL: &str = "\nint dx_headless_sentinel = dx_undeclared_sentinel;\n";

fn decode_frame(buffer: &[u8]) -> Result<Option<(Value, usize)>, String> {
    let Some(head_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(None);
    };
    let mut length: Option<usize> = None;
    for line in buffer[..head_end].split(|byte| *byte == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.len() > b"content-length:".len()
            && line[..b"content-length:".len()].eq_ignore_ascii_case(b"content-length:")
        {
            let text = std::str::from_utf8(line[b"content-length:".len()..].trim_ascii())
                .map_err(|err| format!("content-length is not ascii: {err}"))?;
            length = Some(
                text.parse()
                    .map_err(|err| format!("content-length is not a number: {err}"))?,
            );
        }
    }
    let length = length.ok_or_else(|| "frame has no content-length".to_string())?;
    let body_at = head_end + 4;
    if buffer.len() < body_at + length {
        return Ok(None);
    }
    let value: Value = serde_json::from_slice(&buffer[body_at..body_at + length])
        .map_err(|err| format!("frame body is not json: {err}"))?;
    Ok(Some((value, body_at + length)))
}

fn encode_message(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("lsp message must serialize");
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(&body);
    out
}

fn error_code(value: &Value) -> Option<i64> {
    value.get("error")?.get("code")?.as_i64()
}

struct Server {
    child: Child,
    messages: Arc<Mutex<Vec<Value>>>,
    frame_errors: Arc<Mutex<Vec<String>>>,
    next_id: i64,
    stderr_path: PathBuf,
}

impl Server {
    fn spawn(
        binary: &Path,
        args: &[String],
        cwd: &Path,
        home: &Path,
        tmp: &Path,
        stderr_name: &str,
    ) -> Result<Self, String> {
        let stderr_path = tmp.join(stderr_name);
        let stderr_file =
            std::fs::File::create(&stderr_path).map_err(|err| format!("open stderr: {err}"))?;
        let mut child = Command::new(binary)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr_file)
            .env_clear()
            .env("HOME", home)
            .env("PATH", "/usr/bin:/bin")
            .env("TMPDIR", tmp)
            .spawn()
            .map_err(|err| format!("spawn {}: {err}", binary.display()))?;
        let stdout = child.stdout.take().expect("piped stdout must exist");
        let messages = Arc::new(Mutex::new(Vec::new()));
        let frame_errors = Arc::new(Mutex::new(Vec::new()));
        let reader_messages = Arc::clone(&messages);
        let reader_errors = Arc::clone(&frame_errors);
        std::thread::spawn(move || {
            read_frames(stdout, &reader_messages, &reader_errors);
        });
        Ok(Self {
            child,
            messages,
            frame_errors,
            next_id: 1,
            stderr_path,
        })
    }

    fn send(&mut self, value: &Value) -> Result<(), String> {
        use std::io::Write;
        let bytes = encode_message(value);
        self.child
            .stdin
            .as_mut()
            .expect("piped stdin must exist")
            .write_all(&bytes)
            .map_err(|err| format!("write to server: {err}"))?;
        Ok(())
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    fn request(&mut self, method: &str, params: Value) -> Result<i64, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        Ok(id)
    }

    fn snapshot(&self) -> Result<Vec<Value>, String> {
        if let Some(err) = self.frame_errors.lock().expect("errors lock").first() {
            return Err(format!("server sent a malformed frame: {err}"));
        }
        Ok(self.messages.lock().expect("messages lock").clone())
    }

    fn take_response(&self, id: i64) -> Result<Option<Value>, String> {
        if let Some(err) = self.frame_errors.lock().expect("errors lock").first() {
            return Err(format!("server sent a malformed frame: {err}"));
        }
        let mut messages = self.messages.lock().expect("messages lock");
        if let Some(at) = messages.iter().position(|m| m.get("id") == Some(&json!(id))) {
            return Ok(Some(messages.remove(at)));
        }
        Ok(None)
    }

    fn wait_response(&self, id: i64, secs: u64) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            if let Some(response) = self.take_response(id)? {
                return Ok(response);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(format!(
            "no response for request {id} within {secs}s; stderr: {}",
            stderr_tail(&self.stderr_path)
        ))
    }

    fn pubs_for(&self, uri: &str) -> Result<Vec<Value>, String> {
        Ok(self
            .snapshot()?
            .into_iter()
            .filter(|m| {
                m.get("method") == Some(&json!("textDocument/publishDiagnostics"))
                    && m
                        .pointer("/params/uri")
                        .and_then(Value::as_str)
                        == Some(uri)
            })
            .collect())
    }

    fn wait_pub(
        &self,
        uri: &str,
        secs: u64,
        want_nonempty: bool,
    ) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            for params in self.pubs_for(uri)? {
                let empty = params
                    .pointer("/diagnostics")
                    .and_then(Value::as_array)
                    .map(Vec::is_empty)
                    .unwrap_or(true);
                if !want_nonempty || !empty {
                    return Ok(params);
                }
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "no {} diagnostics for {uri} within {secs}s; stderr: {}",
                    if want_nonempty { "non-empty" } else { "any" },
                    stderr_tail(&self.stderr_path)
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn wait_show_message(&self, needle: &str, secs: u64) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            for message in self.snapshot()? {
                let is_show = message.get("method") == Some(&json!("window/showMessage"));
                let text = message
                    .pointer("/params/message")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if is_show && text.contains(needle) {
                    return Ok(message);
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Err(format!(
            "no window/showMessage containing {needle:?} within {secs}s; stderr: {}",
            stderr_tail(&self.stderr_path)
        ))
    }

    fn shutdown(&mut self) -> Result<(), String> {
        let id = self.request("shutdown", Value::Null)?;
        let response = self.wait_response(id, REQUEST_SECS)?;
        if error_code(&response).is_some() {
            return Err(format!("shutdown failed: {response}"));
        }
        self.notify("exit", Value::Null)?;
        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_frames(
    mut stdout: std::process::ChildStdout,
    messages: &Arc<Mutex<Vec<Value>>>,
    errors: &Arc<Mutex<Vec<String>>>,
) {
    let mut pending: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match stdout.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                pending.extend_from_slice(&chunk[..n]);
                loop {
                    match decode_frame(&pending) {
                        Ok(None) => break,
                        Ok(Some((value, used))) => {
                            pending.drain(..used);
                            messages.lock().expect("messages lock").push(value);
                        }
                        Err(err) => {
                            errors.lock().expect("errors lock").push(err);
                            pending.clear();
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn stderr_tail(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut tail = String::new();
    for line in text.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev() {
        tail.push_str(line);
        tail.push('\n');
    }
    tail
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

fn write_text(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent dirs must exist");
    }
    std::fs::write(path, text.as_bytes()).expect("fixture write must succeed");
}

fn position_of(text: &str, needle: &str, occurrence: usize) -> (u32, u32) {
    let mut seen = 0;
    let mut offset = 0;
    let at = loop {
        match text[offset..].find(needle) {
            Some(found) => {
                if seen == occurrence {
                    break offset + found;
                }
                seen += 1;
                offset += found + needle.len();
            }
            None => panic!("{needle:?} occurrence {occurrence} not found"),
        }
    };
    let line = text[..at].matches('\n').count() as u32;
    let character = (at - text[..at].rfind('\n').map(|at| at + 1).unwrap_or(0)) as u32;
    (line, character)
}

fn rust_sysroot(project_rustc: &Path) -> (String, String) {
    let output = Command::new(project_rustc)
        .arg("--print")
        .arg("sysroot")
        .output()
        .expect("project rustc must run");
    assert!(output.status.success(), "rustc --print sysroot must succeed");
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

fn start_rust(work: &Path) -> Result<(Server, String), String> {
    let server = Server::spawn(
        &tool("DX_LSP_RUST_ANALYZER"),
        &[],
        work,
        &work.join("home"),
        work,
        "rust-analyzer-stderr.log",
    )?;
    let root = format!("file://{}", work.display());
    Ok((server, root))
}

fn open_rust(
    server: &mut Server,
    root: &str,
    work: &Path,
    names: &[&str],
) -> Result<Vec<(String, String)>, String> {
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

fn init_rust(server: &mut Server, root: &str, check: &[String]) -> Result<Value, String> {
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
    let response = server.wait_response(id, INIT_SECS)?;
    if error_code(&response).is_some() {
        return Err(format!("rust initialize failed: {response}"));
    }
    server.notify("initialized", json!({}))?;
    Ok(response)
}

fn definition(
    server: &mut Server,
    uri: &str,
    line: u32,
    character: u32,
) -> Result<Value, String> {
    let id = server.request(
        "textDocument/definition",
        json!({"textDocument": {"uri": uri},
               "position": {"line": line, "character": character}}),
    )?;
    server.wait_response(id, REQUEST_SECS)
}

fn definition_uri(result: &Value) -> Option<String> {
    if let Some(first) = result.get("result")?.as_array()?.first() {
        if let Some(uri) = first.get("uri").and_then(Value::as_str) {
            return Some(uri.to_string());
        }
        return first
            .get("targetUri")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    None
}

#[test]
fn qualified_servers_are_pinned() {
    assert_eq!(
        SUPPORTED,
        [("rust", "rust-analyzer"), ("c++", "clangd")],
        "only rust-analyzer and clangd are qualified headless servers",
    );
}

#[test]
fn frame_round_trip_and_batch() {
    let first = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}});
    let second = json!({"jsonrpc": "2.0", "method": "initialized", "params": {}});
    let mut stream = encode_message(&first);
    stream.extend_from_slice(&encode_message(&second));
    let (back, used) = decode_frame(&stream)
        .expect("frame must decode")
        .expect("buffer holds a frame");
    assert_eq!(back, first);
    let (again, _) = decode_frame(&stream[used..])
        .expect("second frame must decode")
        .expect("buffer holds a second frame");
    assert_eq!(again, second);
    let lower = format!("content-length: {}\r\n\r\n", serde_json::to_vec(&first).unwrap().len());
    let mut raw = lower.into_bytes();
    raw.extend_from_slice(&serde_json::to_vec(&first).unwrap());
    let (cased, _) = decode_frame(&raw)
        .expect("lowercase header must decode")
        .expect("buffer holds a frame");
    assert_eq!(cased, first);
}

#[test]
fn frame_rejects_malformed() {
    assert!(
        decode_frame(b"{\"jsonrpc\":\"2.0\"}")
            .expect("headerless bytes are an incomplete frame")
            .is_none()
    );
    assert!(decode_frame(b"Content-Length: 7\r\n\r\n{\"a\":}").is_err());
    let mut partial = format!("Content-Length: 100\r\n\r\n").into_bytes();
    partial.extend_from_slice(b"{\"jsonrpc\":");
    assert!(decode_frame(&partial).expect("short body must not fail").is_none());
    let mut bad = format!("Content-Length: 4\r\n\r\n").into_bytes();
    bad.extend_from_slice(b"nope");
    assert!(decode_frame(&bad).is_err());
    assert!(decode_frame(b"Content-Length: nope\r\n\r\n{}").is_err());
}

#[test]
fn error_value_inspection() {
    let not_found = json!({"jsonrpc": "2.0", "id": 7,
        "error": {"code": -32601, "message": "method not found"}});
    assert_eq!(error_code(&not_found), Some(-32601));
    let ok = json!({"jsonrpc": "2.0", "id": 7, "result": []});
    assert_eq!(error_code(&ok), None);
}

#[test]
fn spawn_missing_binary_fails() {
    let result = Command::new("/nonexistent/dx-lsp-probe").output();
    assert!(result.is_err(), "missing server binary must fail to spawn");
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
        &serde_json::to_string(&rust_project(&work, &sysroot, &sysroot_src, &["feature=\"mine\""]))
            .expect("project must serialize"),
    );
    let out = work.join("out");
    std::fs::create_dir_all(&out).expect("out dir must exist");
    let check = check_command(&tool("DX_LSP_RUSTC_CHECK"), &out, &["feature=\"mine\""]);

    let (mut server, root) = start_rust(&work).expect("rust-analyzer must start");
    let init = init_rust(&mut server, &root, &check).expect("rust initialize must succeed");
    assert_eq!(
        init.pointer("/result/serverInfo/name").and_then(Value::as_str),
        Some("rust-analyzer"),
        "server identity must be rust-analyzer: {init}",
    );
    eprintln!(
        "rust-analyzer version: {}",
        init.pointer("/result/serverInfo/version").and_then(Value::as_str).unwrap_or("unknown")
    );
    let opened = open_rust(&mut server, &root, &work, &["src/lib.rs", "src/featured.rs", "src/generated.rs"])
        .expect("didOpen must send");
    let lib_uri = opened[0].0.clone();
    let lib_text = opened[0].1.clone();
    server
        .notify("textDocument/didSave", json!({"textDocument": {"uri": lib_uri}}))
        .expect("didSave must send");

    let params = server.wait_pub(&lib_uri, RUST_PUB_SECS, true).expect("rust diagnostic must arrive");
    let diags = params["diagnostics"].as_array().expect("diagnostics must be an array");
    assert!(
        diags.iter().any(|d| d.get("source") == Some(&json!("rustc"))
            && d.get("code") == Some(&json!("E0425"))
            && d.pointer("/message").and_then(Value::as_str).unwrap_or("").contains("undefined_helper")),
        "managed check must report the deliberate E0425: {diags:?}",
    );

    let (line, character) = position_of(&lib_text, "featured::featured", 0);
    let target = definition(&mut server, &lib_uri, line, character + "featured::".len() as u32)
        .expect("definition must answer");
    assert_eq!(
        definition_uri(&target).as_deref().map(|uri| uri.ends_with("src/featured.rs")),
        Some(true),
        "feature-gated definition must land in featured.rs: {target}",
    );
    let (gen_line, gen_character) = position_of(&lib_text, "GENERATED_VALUE", 0);
    let generated = definition(&mut server, &lib_uri, gen_line, gen_character)
        .expect("generated definition must answer");
    assert_eq!(
        definition_uri(&generated).as_deref().map(|uri| uri.ends_with("src/generated.rs")),
        Some(true),
        "generated-source definition must land in generated.rs: {generated}",
    );

    for (uri, _) in opened.iter().skip(1) {
        let latest = server.pubs_for(uri).expect("pubs must read");
        assert!(
            latest.iter().all(|p| p["diagnostics"].as_array().map(Vec::is_empty).unwrap_or(true)),
            "supporting files must stay clean: {latest:?}",
        );
    }
    let fresh = std::fs::read_to_string(work.join("src/lib.rs")).expect("source must reread");
    assert_eq!(fresh, RUST_LIB, "check mode must preserve source bytes");
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
    let check = check_command(&tool("DX_LSP_RUSTC_CHECK"), &out, &["feature=\"mine\"", "consumer"]);

    let (mut server, root) = start_rust(&work).expect("rust-analyzer must start");
    init_rust(&mut server, &root, &check).expect("rust initialize must succeed");
    let opened = open_rust(&mut server, &root, &work, &["src/lib.rs", "src/featured.rs", "src/generated.rs"])
        .expect("didOpen must send");
    let lib_uri = opened[0].0.clone();
    let lib_text = opened[0].1.clone();
    server
        .notify("textDocument/didSave", json!({"textDocument": {"uri": lib_uri}}))
        .expect("didSave must send");

    let params = server.wait_pub(&lib_uri, RUST_PUB_SECS, false).expect("rust pub must arrive");
    assert!(
        params["diagnostics"].as_array().map(Vec::is_empty).unwrap_or(false),
        "clean consumer fixture must pass: {:?}",
        params["diagnostics"],
    );
    let deadline = Instant::now() + Duration::from_secs(QUIET_SECS);
    while Instant::now() < deadline {
        let pubs = server.pubs_for(&lib_uri).expect("pubs must read");
        assert!(
            pubs.iter().all(|p| p["diagnostics"].as_array().map(Vec::is_empty).unwrap_or(true)),
            "consumer fixture must stay clean: {pubs:?}",
        );
        std::thread::sleep(Duration::from_secs(1));
    }

    let (line, character) = position_of(&lib_text, "featured::tuned", 0);
    let target = definition(&mut server, &lib_uri, line, character + "featured::".len() as u32)
        .expect("consumer definition must answer");
    assert_eq!(
        definition_uri(&target).as_deref().map(|uri| uri.ends_with("src/featured.rs")),
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
    let opened = open_rust(&mut server, &root, &work, &["src/lib.rs", "src/featured.rs"])
        .expect("didOpen must send");
    let lib_uri = opened[0].0.clone();
    let lib_text = opened[0].1.clone();
    server
        .notify("textDocument/didSave", json!({"textDocument": {"uri": lib_uri}}))
        .expect("didSave must send");

    let params = server.wait_pub(&lib_uri, RUST_PUB_SECS, true).expect("rust diagnostic must arrive");
    let diags = params["diagnostics"].as_array().expect("diagnostics must be an array");
    assert!(
        diags.iter().any(|d| d.get("code") == Some(&json!("E0425"))
            && d.pointer("/message").and_then(Value::as_str).unwrap_or("").contains("featured")),
        "missing feature must surface as an explicit E0425: {diags:?}",
    );
    let (line, character) = position_of(&lib_text, "featured::featured", 0);
    let target = definition(&mut server, &lib_uri, line, character + "featured::".len() as u32)
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
        &serde_json::to_string(&rust_project(&work, &sysroot, &sysroot_src, &["feature=\"mine\""]))
            .expect("project must serialize"),
    );
    let check = vec!["/nonexistent/dx-check".to_string(), "{saved_file}".to_string()];

    let (mut server, root) = start_rust(&work).expect("rust-analyzer must start");
    init_rust(&mut server, &root, &check).expect("rust initialize must succeed");
    let opened = open_rust(&mut server, &root, &work, &["src/lib.rs"]).expect("didOpen must send");
    server
        .notify("textDocument/didSave", json!({"textDocument": {"uri": opened[0].0}}))
        .expect("didSave must send");
    let message = server
        .wait_show_message("/nonexistent/dx-check", SHOW_SECS)
        .expect("unavailable check must be reported");
    assert!(
        message.pointer("/params/message").and_then(Value::as_str).unwrap_or("").contains("Failed to spawn"),
        "missing check binary must fail explicitly: {message}",
    );
    server.shutdown().expect("server must stay usable after the failure");
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
        .wait_show_message("Failed to load workspace", SHOW_SECS)
        .expect("malformed project must be reported");
    server.shutdown().expect("server must stay usable after the failure");
}

fn clangd_database(work: &Path) -> (String, PathBuf, String) {
    let record = std::fs::read_to_string(tool("DX_LSP_AQUERY")).expect("aquery record must read");
    let graph = AqueryGraph::parse(&record).expect("aquery record must parse");
    let commands = graph.compile_commands(Path::new("/")).expect("compile commands must decode");
    assert!(!commands.is_empty(), "record must hold a compile action");
    let target = commands[0].target.clone();
    let label = target.strip_suffix("_upstream").unwrap_or(&target).to_string();
    let source = commands[0].source.clone();
    let entries = cc_context::database(&commands, work, &label, &[source.clone()])
        .expect("authoritative database must build");
    assert_eq!(entries.len(), 1, "record must hold one entry");
    let header = commands[0]
        .inputs
        .iter()
        .find(|input| input.ends_with("generated/limits.h"))
        .expect("record must declare the generated header")
        .clone();
    write_text(&work.join(&source), &std::fs::read_to_string(tool("DX_LSP_PROBE_C")).expect("probe must read"));
    write_text(&work.join(&header), &std::fs::read_to_string(tool("DX_LSP_LIMITS_H")).expect("header must read"));
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

fn start_clangd(work: &Path) -> Result<Server, String> {
    Server::spawn(
        &tool("DX_LSP_CLANGD"),
        &[
            format!("--compile-commands-dir={}", work.display()),
            "--log=error".to_string(),
        ],
        work,
        &work.join("home"),
        work,
        "clangd-stderr.log",
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
    let init = server.wait_response(id, INIT_SECS).expect("clangd initialize must succeed");
    assert_eq!(
        init.pointer("/result/capabilities/definitionProvider"),
        Some(&json!(true)),
        "clangd must offer definitions: {init}",
    );
    server.notify("initialized", json!({})).expect("initialized must send");
    server
        .notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": probe_uri, "languageId": "c", "version": 1, "text": clean}}),
        )
        .expect("didOpen must send");

    let params = server.wait_pub(&probe_uri, CLANGD_PUB_SECS, false).expect("clangd pub must arrive");
    let diags = params["diagnostics"].as_array().expect("diagnostics must be an array");
    assert!(
        diags.iter().all(|d| d.get("severity") != Some(&json!(1))),
        "clean probe must carry no error: {diags:?}",
    );

    let (line, character) = position_of(&clean, "DX_ZERO_RESULT", 1);
    let target = definition(&mut server, &probe_uri, line, character).expect("definition must answer");
    let header_text = std::fs::read_to_string(tool("DX_LSP_LIMITS_H")).expect("header must reread");
    let (define_line, _) = position_of(&header_text, "#define DX_ZERO_RESULT", 0);
    let uri = definition_uri(&target).expect("definition must land somewhere");
    assert!(uri.ends_with("generated/limits.h"), "generated definition must land in limits.h: {uri}");
    assert_eq!(
        target.pointer("/result/0/range/start/line").and_then(Value::as_u64),
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
    let params = server.wait_pub(&probe_uri, CLANGD_PUB_SECS, true).expect("dirty diagnostic must arrive");
    let diags = params["diagnostics"].as_array().expect("diagnostics must be an array");
    assert!(
        diags.iter().any(|d| d.get("severity") == Some(&json!(1))
            && d.pointer("/message").and_then(Value::as_str).unwrap_or("").contains("dx_undeclared_sentinel")),
        "clangd must report the deliberate error: {diags:?}",
    );

    let unknown = server.request("dx/doesNotExist", json!({})).expect("request must send");
    let response = server.wait_response(unknown, REQUEST_SECS).expect("error must arrive");
    assert_eq!(error_code(&response), Some(-32601), "unsupported method must fail: {response}");

    let slow = server
        .request("workspace/symbol", json!({"query": "classify"}))
        .expect("request must send");
    server
        .notify("$/cancelRequest", json!({"id": slow}))
        .expect("cancel must send");
    let cancelled = server.wait_response(slow, REQUEST_SECS).expect("cancelled request must settle");
    if let Some(code) = error_code(&cancelled) {
        assert_eq!(code, -32800, "cancelled request must report cancellation: {cancelled}");
    }
    let (again_line, again_character) = position_of(&clean, "DX_ZERO_RESULT", 1);
    let alive = definition(&mut server, &probe_uri, again_line, again_character)
        .expect("server must stay alive after cancel");
    assert!(
        definition_uri(&alive).map(|uri| uri.ends_with("generated/limits.h")).unwrap_or(false),
        "server must answer after cancel: {alive}",
    );

    std::fs::remove_file(work.join(&header_rel)).expect("header removal must succeed");
    server
        .notify("textDocument/didClose", json!({"textDocument": {"uri": probe_uri}}))
        .expect("didClose must send");
    server
        .notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": probe_uri, "languageId": "c", "version": 3, "text": dirty}}),
        )
        .expect("reopen must send");
    let deadline = Instant::now() + Duration::from_secs(CLANGD_PUB_SECS);
    let stale = loop {
        let pubs = server.pubs_for(&probe_uri).expect("pubs must read");
        let found = pubs.iter().rev().find(|p| {
            p["diagnostics"].as_array().map(|ds| {
                ds.iter().any(|d| {
                    d.pointer("/message").and_then(Value::as_str).unwrap_or("").contains("not found")
                })
            }).unwrap_or(false)
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
        stale["diagnostics"].as_array().map(|ds| ds.iter().any(|d| {
            d.pointer("/message").and_then(Value::as_str).unwrap_or("").contains("limits.h")
        })).unwrap_or(false),
        "stale header diagnostic must name limits.h: {stale}",
    );

    let on_disk = std::fs::read_to_string(&probe_path).expect("staged probe must reread");
    assert_eq!(on_disk, clean, "check mode must preserve source bytes");
    server.shutdown().expect("clangd shutdown must succeed");
}
