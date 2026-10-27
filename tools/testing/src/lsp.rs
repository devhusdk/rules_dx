//! A small headless LSP client for tests: framing, a reader thread and timed waits.

use std::io::{BufRead, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const MAX_HEADER_BYTES: usize = 65536;
const MAX_BODY_BYTES: usize = 1 << 30;

/// Frames one JSON message with a `Content-Length` header.
pub fn encode(message: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(message).expect("lsp message serializes");
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(&body);
    out
}

fn header_length(head: &[u8]) -> Result<usize, String> {
    let text = std::str::from_utf8(&head[..head.len() - 4])
        .map_err(|err| format!("lsp headers are not UTF-8: {err}"))?;
    for line in text.split("\r\n") {
        let (name, value) = line.split_once(':').unwrap_or((line, ""));
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("lsp content length is not a number: {}", value.trim()));
        }
    }
    Err("lsp headers carry no Content-Length".to_owned())
}

/// Reads one framed message, refusing oversized headers and bodies.
pub fn read_message(reader: &mut impl BufRead) -> Result<Value, String> {
    let mut head: Vec<u8> = Vec::new();
    loop {
        if head.len() > MAX_HEADER_BYTES {
            return Err("lsp headers exceed 64 KiB".to_owned());
        }
        let mut byte = [0u8; 1];
        reader
            .read_exact(&mut byte)
            .map_err(|err| format!("lsp stream ended inside headers: {err}"))?;
        head.push(byte[0]);
        if head.len() >= 4 && head[head.len() - 4..] == *b"\r\n\r\n" {
            break;
        }
    }
    let length = header_length(&head)?;
    if length > MAX_BODY_BYTES {
        return Err(format!("lsp content length {length} exceeds 1 GiB"));
    }
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|_| format!("lsp body truncated: expected {length} bytes"))?;
    serde_json::from_slice(&body).map_err(|err| format!("lsp body is not JSON: {err}"))
}

/// Answers a server-to-client request, or returns `None` when `message` is not one.
/// `workspace/configuration` gets the matching section of `settings`; anything else gets `null`.
pub fn reply_to_server_request(message: &Value, settings: &Value) -> Option<Value> {
    let method = message.get("method")?.as_str()?;
    let id = message.get("id")?.clone();
    let result = if method == "workspace/configuration" {
        let items = message["params"]["items"].as_array().cloned();
        Value::Array(
            items
                .unwrap_or_default()
                .iter()
                .map(|item| {
                    let section = item["section"].as_str().unwrap_or("");
                    section
                        .split('.')
                        .filter(|part| !part.is_empty())
                        .try_fold(settings, |at, part| at.get(part))
                        .cloned()
                        .unwrap_or(Value::Null)
                })
                .collect(),
        )
    } else {
        Value::Null
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// One running language server with a reader thread behind a channel.
pub struct Session {
    stdin: std::process::ChildStdin,
    events: Receiver<Result<Value, String>>,
    next_id: i64,
    child: Child,
    settings: Value,
    stderr: Arc<Mutex<Vec<u8>>>,
    /// Every message seen so far, in order.
    pub log: Vec<Value>,
}

impl Session {
    /// Starts `command` with piped stdio; `settings` answers `workspace/configuration`.
    pub fn spawn(mut command: Command, settings: Value) -> Result<Session, String> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|err| format!("cannot start {command:?}: {err}"))?;
        let stdin = child.stdin.take().expect("lsp child keeps stdin");
        let stdout = child.stdout.take().expect("lsp child keeps stdout");
        let mut stderr = child.stderr.take().expect("lsp child keeps stderr");
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&captured);
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(n) = std::io::Read::read(&mut stderr, &mut chunk) {
                if n == 0 {
                    break;
                }
                if let Ok(mut all) = sink.lock() {
                    all.extend_from_slice(&chunk[..n]);
                    let excess = all.len().saturating_sub(65536);
                    all.drain(..excess);
                }
            }
        });
        let (sender, events) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            loop {
                let message = read_message(&mut reader);
                let done = message.is_err();
                if sender.send(message).is_err() || done {
                    break;
                }
            }
        });
        Ok(Session {
            stdin,
            events,
            next_id: 0,
            child,
            settings,
            stderr: captured,
            log: Vec::new(),
        })
    }

    fn send(&mut self, message: &Value) -> Result<(), String> {
        self.stdin
            .write_all(&encode(message))
            .and_then(|()| self.stdin.flush())
            .map_err(|err| format!("cannot write lsp message: {err}"))
    }

    /// Sends a notification.
    pub fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send(&json!({"jsonrpc": "2.0", "method": method, "params": params}))
    }

    /// Sends a request and returns its id.
    pub fn request(&mut self, method: &str, params: Value) -> Result<i64, String> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        Ok(id)
    }

    /// Reads messages until `pred` accepts one, answering server requests on the way.
    /// Returns `Ok(None)` on timeout.
    pub fn wait_for(
        &mut self,
        timeout: Duration,
        mut pred: impl FnMut(&Value) -> bool,
    ) -> Result<Option<Value>, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let message = match self.events.recv_timeout(left) {
                Ok(message) => message?,
                Err(RecvTimeoutError::Timeout) => return Ok(None),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("lsp server closed its output".to_owned())
                }
            };
            if let Some(reply) = reply_to_server_request(&message, &self.settings) {
                self.send(&reply)?;
            }
            self.log.push(message.clone());
            if pred(&message) {
                return Ok(Some(message));
            }
        }
    }

    /// Sends a request and waits for its response; `Ok(None)` on timeout.
    pub fn call(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Option<Value>, String> {
        let id = self.request(method, params)?;
        self.wait_for(timeout, |m| {
            m.get("id") == Some(&json!(id)) && m.get("method").is_none()
        })
    }

    /// The last 64 KiB the server wrote to stderr.
    pub fn stderr(&self) -> String {
        let all = self
            .stderr
            .lock()
            .map(|all| all.clone())
            .unwrap_or_default();
        String::from_utf8_lossy(&all).into_owned()
    }

    /// Asks the server to exit, then kills it if it lingers.
    pub fn shutdown(&mut self) {
        if self
            .call("shutdown", Value::Null, Duration::from_secs(20))
            .is_ok()
        {
            let _ = self.notify("exit", Value::Null);
        }
        self.kill();
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trips() {
        let message = json!({"jsonrpc": "2.0", "method": "x", "params": {"k": [1, 2]}});
        let bytes = encode(&message);
        assert_eq!(read_message(&mut &bytes[..]).expect("read"), message);
    }

    #[test]
    fn missing_length_is_rejected() {
        let err = read_message(&mut &b"Other: 1\r\n\r\n{}"[..]).expect_err("no length");
        assert!(err.contains("Content-Length"), "{err}");
    }

    #[test]
    fn truncated_body_is_rejected() {
        let err = read_message(&mut &b"Content-Length: 10\r\n\r\n{}"[..]).expect_err("short");
        assert!(err.contains("truncated"), "{err}");
    }

    #[test]
    fn invalid_json_is_rejected() {
        let err = read_message(&mut &b"Content-Length: 3\r\n\r\nabc"[..]).expect_err("json");
        assert!(err.contains("not JSON"), "{err}");
    }

    #[test]
    fn configuration_reply_maps_each_section() {
        let request = json!({"jsonrpc": "2.0", "id": 7, "method": "workspace/configuration",
            "params": {"items": [{"section": "a.b"}, {"section": "missing"}]}});
        let reply = reply_to_server_request(&request, &json!({"a": {"b": 1}})).expect("reply");
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["result"], json!([1, null]));
    }

    #[test]
    fn other_requests_get_null_and_notifications_none() {
        let request = json!({"id": "s", "method": "client/registerCapability"});
        let reply = reply_to_server_request(&request, &Value::Null).expect("reply");
        assert_eq!(reply["result"], Value::Null);
        assert!(reply_to_server_request(&json!({"method": "n"}), &Value::Null).is_none());
    }

    #[test]
    fn missing_binary_is_reported() {
        let err = Session::spawn(Command::new("/nonexistent/lsp-server"), Value::Null)
            .err()
            .expect("spawn fails");
        assert!(err.contains("cannot start"), "{err}");
    }
}
