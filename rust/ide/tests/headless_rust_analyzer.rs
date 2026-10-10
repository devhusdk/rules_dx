use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};

use dx_testing::serde_json;

fn analyzer() -> PathBuf {
    let rel = std::env::var("DX_RUST_ANALYZER").expect("DX_RUST_ANALYZER");
    dx_testing::resolve_runfiles(&rel)
}

fn tag(name: &str) -> String {
    format!("rust-headless-{}-{name}", std::process::id())
}

fn stage_project(name: &str, main_rs: &str) -> PathBuf {
    let root = std::env::temp_dir().join(tag(name));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("scratch reset");
    }
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("scratch root");
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"headless_probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .expect("manifest staged");
    std::fs::write(src.join("main.rs"), main_rs).expect("source staged");
    root
}

fn toolchain_path(name: &str) -> PathBuf {
    let rel = std::env::var(name).expect(name);
    dx_testing::resolve_runfiles(&rel)
}

fn run_diagnostics(dir: &std::path::Path) -> (Option<i32>, String) {
    let cargo = toolchain_path("DX_CARGO");
    let rustc = toolchain_path("DX_RUSTC");
    let mut paths = vec![
        cargo
            .parent()
            .expect("cargo dir")
            .to_string_lossy()
            .into_owned(),
        rustc
            .parent()
            .expect("rustc dir")
            .to_string_lossy()
            .into_owned(),
    ];
    if let Ok(host) = std::env::var("PATH") {
        paths.push(host);
    }
    let output = Command::new(analyzer())
        .arg("diagnostics")
        .arg(dir)
        .env("PATH", paths.join(":"))
        .env("RUSTUP_AUTO_INSTALL", "0")
        .output()
        .expect("diagnostics runs");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.code(), text)
}

fn encode(value: &serde_json::Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("message renders");
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(&body);
    out
}

fn parse_frame(buffer: &[u8]) -> Result<(serde_json::Value, usize), String> {
    let head_end = buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "missing header terminator".to_string())?;
    let head = std::str::from_utf8(&buffer[..head_end])
        .map_err(|err| format!("header is not utf8: {err}"))?;
    let mut length: Option<usize> = None;
    for line in head.split("\r\n") {
        if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            let parsed: usize = rest
                .trim()
                .parse()
                .map_err(|_| "content-length is not a number".to_string())?;
            length = Some(parsed);
        }
    }
    let length = length.ok_or_else(|| "missing content-length".to_string())?;
    let total = head_end + 4 + length;
    if buffer.len() < total {
        return Err(format!(
            "truncated body: need {total} bytes, have {}",
            buffer.len()
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&buffer[head_end + 4..total])
        .map_err(|err| format!("body is not json: {err}"))?;
    Ok((value, total))
}

fn read_one(stdout: &mut impl Read) -> Result<serde_json::Value, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1];
    loop {
        match stdout.read(&mut chunk) {
            Ok(0) => return Err("server closed the stream".to_string()),
            Ok(_) => buffer.extend_from_slice(&chunk),
            Err(err) => return Err(format!("read failed: {err}")),
        }
        if let Ok((value, consumed)) = parse_frame(&buffer) {
            if consumed <= buffer.len() {
                return Ok(value);
            }
        }
        if buffer.len() > 8 * 1024 * 1024 {
            return Err("no complete frame in 8MiB".to_string());
        }
    }
}

fn send(stdin: &mut ChildStdin, value: &serde_json::Value) {
    use std::io::Write;
    stdin.write_all(&encode(value)).expect("send frame");
    stdin.flush().expect("flush frame");
}

fn read_until_id(stdout: &mut impl Read, id: i64) -> serde_json::Value {
    for _ in 0..80 {
        let message = read_one(stdout).expect("server answers");
        if message.get("id") == Some(&serde_json::json!(id)) {
            return message;
        }
    }
    panic!("no response with id {id}");
}

fn spawn() -> Child {
    Command::new(analyzer())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("rust-analyzer spawns")
}

fn initialize(child: &mut Child) -> serde_json::Value {
    let mut stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.as_mut().expect("stdout piped");
    send(
        &mut stdin,
        &serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"processId": null, "rootUri": null, "capabilities": {}}}),
    );
    let response = read_until_id(stdout, 1);
    send(
        &mut stdin,
        &serde_json::json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    );
    child.stdin.replace(stdin);
    response
}

fn shutdown(child: &mut Child) {
    let mut stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.as_mut().expect("stdout piped");
    send(
        &mut stdin,
        &serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": null}),
    );
    let response = read_until_id(stdout, 2);
    assert_eq!(response.get("id"), Some(&serde_json::json!(2)));
    use std::io::Write;
    let exit = serde_json::json!({"jsonrpc": "2.0", "method": "exit"});
    let body = serde_json::to_vec(&exit).expect("exit renders");
    let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    framed.extend_from_slice(&body);
    stdin.write_all(&framed).expect("send exit");
    stdin.flush().expect("flush exit");
    drop(stdin);
}

const DIRTY_MAIN: &str = "fn main() {\n    let x: i32 = \"not an int\";\n    let _ = x;\n}\n";
const CLEAN_MAIN: &str = "fn main() {\n    let x: i32 = 1;\n    let _ = x + 1;\n}\n";

#[test]
fn dirty_diagnostics_name_the_type_error() {
    let root = stage_project("dirty", DIRTY_MAIN);
    let (code, text) = run_diagnostics(&root);
    assert_ne!(code, Some(0), "{text}");
    assert!(text.contains("E0308"), "{text}");
    assert!(text.contains("expected i32"), "{text}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn clean_diagnostics_stay_quiet() {
    let root = stage_project("clean", CLEAN_MAIN);
    let (code, text) = run_diagnostics(&root);
    assert_eq!(code, Some(0), "{text}");
    assert!(!text.contains("E0308"), "{text}");
    assert!(!text.contains("diagnostic error"), "{text}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn missing_manifest_fails_explicitly() {
    let absent = std::env::temp_dir().join(tag("absent-manifest"));
    if absent.exists() {
        std::fs::remove_dir_all(&absent).expect("scratch reset");
    }
    let (code, text) = run_diagnostics(&absent);
    assert_ne!(code, Some(0), "{text}");
    assert!(text.contains("No such file"), "{text}");
}

#[test]
fn initialize_answers_with_capabilities() {
    let mut child = spawn();
    let response = initialize(&mut child);
    let capabilities = response
        .get("result")
        .and_then(|result| result.get("capabilities"))
        .expect("initialize carries capabilities");
    assert!(
        capabilities.get("completionProvider").is_some(),
        "{capabilities}"
    );
    shutdown(&mut child);
    let _ = child.wait();
}

#[test]
fn unsupported_request_returns_an_error() {
    let mut child = spawn();
    initialize(&mut child);
    let mut stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.as_mut().expect("stdout piped");
    send(
        &mut stdin,
        &serde_json::json!({"jsonrpc": "2.0", "id": 7, "method": "workspace/unknownMethod", "params": {}}),
    );
    let response = read_until_id(stdout, 7);
    assert_eq!(
        response.get("error").and_then(|error| error.get("code")),
        Some(&serde_json::json!(-32601)),
        "{response}"
    );
    child.stdin.replace(stdin);
    shutdown(&mut child);
    let _ = child.wait();
}

#[test]
fn framing_rejects_malformed_input() {
    let good = encode(&serde_json::json!({"jsonrpc": "2.0", "id": 1}));
    let (value, consumed) = parse_frame(&good).expect("valid frame");
    assert_eq!(value.get("id"), Some(&serde_json::json!(1)));
    assert_eq!(consumed, good.len());
    assert!(parse_frame(b"{\"jsonrpc\":\"2.0\"}").is_err());
    assert!(parse_frame(b"Content-Length: not-a-number\r\n\r\n{}").is_err());
    assert!(parse_frame(b"Content-Length: 100\r\n\r\n{}").is_err());
    assert!(parse_frame(b"Content-Length: 3\r\n\r\n{}").is_err());
    assert!(parse_frame(b"Content-Length: 4\r\n\r\n").is_err());
}

#[test]
fn unavailable_server_fails_instead_of_falling_back() {
    let missing = std::env::temp_dir().join(format!(
        "rust-headless-{}-absent-server",
        std::process::id()
    ));
    match Command::new(&missing).arg("--version").output() {
        Ok(_) => panic!("absent server must not execute"),
        Err(err) => assert_eq!(err.kind(), std::io::ErrorKind::NotFound, "{err}"),
    }
}
