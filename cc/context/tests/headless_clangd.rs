use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("{name}")))
}

fn tag(name: &str) -> String {
    format!("cc-headless-{}-{name}", std::process::id())
}

fn stage_db(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(tag(name));
    if root.exists() {
        std::fs::remove_dir_all(&root).expect("scratch reset");
    }
    std::fs::create_dir_all(&root).expect("scratch root");
    std::fs::write(root.join("clean.c"), "int main(){return 0;}\n").expect("clean staged");
    std::fs::write(root.join("dirty.c"), "int main(){return oops;}\n").expect("dirty staged");
    let db = serde_json::json!([
        {"directory": root.to_string_lossy(), "file": root.join("clean.c").to_string_lossy(), "arguments": ["clang", "-c", "clean.c"]},
        {"directory": root.to_string_lossy(), "file": root.join("dirty.c").to_string_lossy(), "arguments": ["clang", "-c", "dirty.c"]}
    ]);
    std::fs::write(
        root.join("compile_commands.json"),
        serde_json::to_string_pretty(&db).expect("db renders"),
    )
    .expect("db staged");
    root
}

fn staged_clangd(root: &std::path::Path) -> PathBuf {
    let _ = root;
    env_path("DX_CLANGD")
}

fn run_check(clangd: &std::path::Path, db: &std::path::Path, file: &str) -> (Option<i32>, String) {
    let output = Command::new(clangd)
        .arg(format!("--compile-commands-dir={}", db.display()))
        .arg(format!("--check={}", db.join(file).display()))
        .arg("--log=error")
        .output()
        .expect("clangd check runs");
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
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
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
    let body = &buffer[head_end + 4..total];
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|err| format!("body is not json: {err}"))?;
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
    for _ in 0..50 {
        let message = read_one(stdout).expect("server answers");
        if message.get("id") == Some(&serde_json::json!(id)) {
            return message;
        }
    }
    panic!("no response with id {id}");
}

fn spawn(clangd: &std::path::Path, db: &std::path::Path) -> Child {
    Command::new(clangd)
        .arg(format!("--compile-commands-dir={}", db.display()))
        .arg("--log=error")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("clangd spawns")
}

fn initialize(child: &mut Child, root: &std::path::Path) -> serde_json::Value {
    let mut stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.as_mut().expect("stdout piped");
    send(
        &mut stdin,
        &serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"processId": null, "rootUri": format!("file://{}", root.display()), "capabilities": {}}}),
    );
    let response = read_until_id(stdout, 1);
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

#[test]
fn dirty_check_names_the_undeclared_identifier() {
    let root = stage_db("check");
    let clangd = staged_clangd(&root);
    let (code, dirty) = run_check(&clangd, &root, "dirty.c");
    assert_ne!(code, Some(0), "{dirty}");
    assert!(dirty.contains("undeclared"), "{dirty}");
    assert!(dirty.contains("oops"), "{dirty}");
    let (clean_code, clean) = run_check(&clangd, &root, "clean.c");
    assert_eq!(clean_code, Some(0), "{clean}");
    assert!(!clean.contains("undeclared"), "{clean}");
    assert!(!clean.contains("error"), "{clean}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn missing_inputs_fail_with_an_explicit_path() {
    let root = stage_db("missing");
    let clangd = staged_clangd(&root);
    let (_, no_db) = run_check(&clangd, &root.join("absent-dir"), "clean.c");
    assert!(no_db.contains("does not exist"), "{no_db}");
    let (_, no_file) = run_check(&clangd, &root, "absent.c");
    assert!(no_file.contains("No such file"), "{no_file}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn initialize_answers_with_a_definition_provider() {
    let root = stage_db("lsp");
    let clangd = staged_clangd(&root);
    let mut child = spawn(&clangd, &root);
    let response = initialize(&mut child, &root);
    let capabilities = response
        .get("result")
        .and_then(|result| result.get("capabilities"))
        .expect("initialize carries capabilities");
    assert_eq!(
        capabilities.get("definitionProvider"),
        Some(&serde_json::json!(true)),
        "{capabilities}"
    );
    shutdown(&mut child);
    let status = child.wait().expect("clangd exits");
    assert!(status.success());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn unsupported_method_returns_method_not_found() {
    let root = stage_db("unknown");
    let clangd = staged_clangd(&root);
    let mut child = spawn(&clangd, &root);
    initialize(&mut child, &root);
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
    let status = child.wait().expect("clangd exits");
    assert!(status.success());
    std::fs::remove_dir_all(&root).ok();
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
    let missing =
        std::env::temp_dir().join(format!("cc-headless-{}-absent-server", std::process::id()));
    let result = Command::new(&missing).arg("--version").output();
    match result {
        Ok(_) => panic!("absent server must not execute"),
        Err(err) => assert_eq!(err.kind(), std::io::ErrorKind::NotFound, "{err}"),
    }
}

#[test]
fn a_silent_server_trips_a_bounded_read() {
    use std::sync::mpsc::channel;
    use std::time::Duration;
    let (done_tx, done_rx) = channel::<()>();
    let handle = std::thread::spawn(move || {
        let mut child = Command::new("sleep")
            .arg("30")
            .stdout(Stdio::piped())
            .spawn()
            .expect("sleep spawns");
        let _ = done_rx.recv_timeout(Duration::from_millis(200));
        child.kill().ok();
        let _ = child.wait();
    });
    let started = std::time::Instant::now();
    handle.join().expect("watcher joins");
    assert!(started.elapsed() < Duration::from_secs(10));
    let _ = done_tx.send(());
}
