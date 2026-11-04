//! Headless LSP qualification for the managed language servers.
//!
//! A small test client speaks the upstream JSON-RPC protocol directly to the
//! selected managed binaries: rust-analyzer with a rustc check command for
//! Rust, and clangd with an authoritative compilation database for C. Only
//! these two servers are qualified here; every other language server stays
//! unqualified and must not be advertised as a supported headless path.

use dx_testing::{self, resolve_runfiles};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const SUPPORTED: [(&str, &str); 2] = [("rust", "rust-analyzer"), ("c++", "clangd")];

pub const INIT_SECS: u64 = 90;
pub const REQUEST_SECS: u64 = 90;
pub const SHOW_SECS: u64 = 120;
pub const QUIET_SECS: u64 = 30;

pub fn decode_frame(buffer: &[u8]) -> Result<Option<(Value, usize)>, String> {
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

pub fn encode_message(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).expect("lsp message must serialize");
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(&body);
    out
}

pub fn error_code(value: &Value) -> Option<i64> {
    value.get("error")?.get("code")?.as_i64()
}

pub struct Server {
    child: Child,
    messages: Arc<Mutex<Vec<Value>>>,
    frame_errors: Arc<Mutex<Vec<String>>>,
    next_id: i64,
    stderr_path: PathBuf,
}

impl Server {
    pub fn spawn(
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

    pub fn send(&mut self, value: &Value) -> Result<(), String> {
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

    pub fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    pub fn request(&mut self, method: &str, params: Value) -> Result<i64, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        Ok(id)
    }

    pub fn snapshot(&self) -> Result<Vec<Value>, String> {
        if let Some(err) = self.frame_errors.lock().expect("errors lock").first() {
            return Err(format!("server sent a malformed frame: {err}"));
        }
        Ok(self.messages.lock().expect("messages lock").clone())
    }

    pub fn take_response(&self, id: i64) -> Result<Option<Value>, String> {
        if let Some(err) = self.frame_errors.lock().expect("errors lock").first() {
            return Err(format!("server sent a malformed frame: {err}"));
        }
        let mut messages = self.messages.lock().expect("messages lock");
        if let Some(at) = messages
            .iter()
            .position(|m| m.get("id") == Some(&json!(id)))
        {
            return Ok(Some(messages.remove(at)));
        }
        Ok(None)
    }

    pub fn wait_response(&self, id: i64, secs: u64) -> Result<Value, String> {
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

    pub fn pubs_for(&self, uri: &str) -> Result<Vec<Value>, String> {
        Ok(self
            .snapshot()?
            .into_iter()
            .filter(|m| {
                m.get("method") == Some(&json!("textDocument/publishDiagnostics"))
                    && m.pointer("/params/uri").and_then(Value::as_str) == Some(uri)
            })
            .filter_map(|m| m.get("params").cloned())
            .collect())
    }

    pub fn wait_pub(&self, uri: &str, secs: u64, want_nonempty: bool) -> Result<Value, String> {
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

    pub fn wait_show_message(&self, needle: &str, secs: u64) -> Result<Value, String> {
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

    pub fn shutdown(&mut self) -> Result<(), String> {
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

pub fn read_frames(
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

pub fn stderr_tail(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut tail = String::new();
    for line in text
        .lines()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        tail.push_str(line);
        tail.push('\n');
    }
    tail
}

pub fn tool(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must name a runfile"));
    resolve_runfiles(&rel)
}

pub fn stage(prefix: &str) -> PathBuf {
    let dir = dx_testing::mkscratch(prefix).expect("scratch dir must exist");
    std::fs::create_dir_all(dir.join("home")).expect("home dir must exist");
    dir
}

pub fn write_text(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("parent dirs must exist");
    }
    std::fs::write(path, text.as_bytes()).expect("fixture write must succeed");
}

pub fn position_of(text: &str, needle: &str, occurrence: usize) -> (u32, u32) {
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

pub fn definition(
    server: &mut Server,
    uri: &str,
    line: u32,
    character: u32,
) -> Result<Value, String> {
    let mut response = Value::Null;
    for _ in 0..50 {
        let id = server.request(
            "textDocument/definition",
            json!({"textDocument": {"uri": uri},
                   "position": {"line": line, "character": character}}),
        )?;
        response = server.wait_response(id, REQUEST_SECS)?;
        if error_code(&response) != Some(-32801) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(response)
}

pub fn definition_uri(result: &Value) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let lower = format!(
            "content-length: {}\r\n\r\n",
            serde_json::to_vec(&first).unwrap().len()
        );
        let mut raw = lower.into_bytes();
        raw.extend_from_slice(&serde_json::to_vec(&first).unwrap());
        let (cased, _) = decode_frame(&raw)
            .expect("lowercase header must decode")
            .expect("buffer holds a frame");
        assert_eq!(cased, first);
    }

    #[test]
    fn frame_rejects_malformed() {
        assert!(decode_frame(b"{\"jsonrpc\":\"2.0\"}")
            .expect("headerless bytes are an incomplete frame")
            .is_none());
        assert!(decode_frame(b"Content-Length: 6\r\n\r\n{\"a\":}").is_err());
        let mut partial = b"Content-Length: 100\r\n\r\n".to_vec();
        partial.extend_from_slice(b"{\"jsonrpc\":");
        assert!(decode_frame(&partial)
            .expect("short body must not fail")
            .is_none());
        let mut bad = b"Content-Length: 4\r\n\r\n".to_vec();
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
}
